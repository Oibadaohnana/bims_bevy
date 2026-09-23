//! The world, and the one loop that advances it.
//!
//! # One clock
//!
//! There is a single [`World::step`] and it moves everything by exactly
//! [`data::STEP_MINUTES`]. The ship's flight, what the crew have seen, and —
//! when they arrive — the crew themselves, construction and health all happen
//! inside it, in a fixed order, at the same instant. Nothing in this crate may
//! grow a clock of its own or a loop of its own; a second one is two
//! simulations that will disagree, and the failure reads as a ship that is in
//! two places.
//!
//! The order inside a step is written out in [`World::step`]. The crew are
//! in it now — spawned at their bunks when the world opens, stepped in the
//! fifth stage — and the last two stages are **empty on purpose**: they are
//! where construction and health go, and they are there so that the shape of
//! the loop is decided while it is still small.
//!
//! # Where the ship is
//!
//! Two numbers and a rule. [`Ship::anchor`] is where design tile (0, 0) sits
//! in the system, [`Ship::heading`] is which way the ship is pointing, and the
//! ship's *position* — the thing a trip moves and the camera is centred on —
//! is the **centre of mass**, worked out from those two through
//! [`flight::angle::rotate_design`].
//!
//! That way round rather than the other because of what
//! [`World::on_ship_changed`] has to promise: welding a shelf to the stern
//! moves the centre of mass, and it must not move the *hull*. If the position
//! were stored and the anchor derived, every wall anybody built would shove
//! the whole ship sideways through space.
//!
//! # What changes a ship's mass
//!
//! Three things, and this is the list to check anything new against: trading
//! while docked, food eaten or grown, and crew joining or leaving. Nothing is
//! burnt in flight: the engines run on the reactor. Construction and deconstruction move materials from the
//! hold into the hull and back — `shipdesign::materials` is that contract —
//! and change the centre of mass and the inertia without changing the total.

use economy::{Money, Storage, footprint, storage};
use flight::{Dynamics, Phase, Plan, PlanError, Target, angle};
use physics::ResourceId;
use shipdesign::parts::{PartKind, Rotation};
use shipdesign::research::{Node as ResearchNode, Research};
use shipdesign::{CARGO_SLOTS, ShipDesign, design_hash};
use worldgen::math::{DVec2, dvec2};
use worldgen::{Galaxy, GalaxyType, Node, StarSystem};

use bims::combat::{
    ArmourKind, Item, LOOT_CELLS, LootCell, PACK_CELLS, Sentry, Tier, Weapon, WeaponKind,
};
use bims::game::Container;
use bims::health::Part;
use bims::order::CrewOrder;
use bims::sight::Stance;

use crate::armour::{self, FetchKind, LootSource, Piece, Where};
use crate::build::{self, BuildSite, SiteRefusal};
use crate::class::{self, Class, Progress, Side, Talent};
use crate::commander::{Aura, Commander, SquadAsk, SquadKind, SquadOrder};
use crate::crew::{Aboard, Residents};
use crate::data;
use crate::deploy::{self, Deck, DeployKind, Deployable, Kit};
use crate::droid::{self as droidplan, Infestation};
use crate::event::{Refusal, WorldEvent};
use crate::frame::{self, Frame};
use crate::grid::{Grid, Kept, Wanted};
use crate::medic::Medic;
use crate::memory::{self, Grave, Losses, SystemMemory};
use crate::mercenary::{self, Hired, Offer};
use crate::mining::{self, MiningSite};
use crate::orders::Standing;
use crate::plunder::{self, Plunder};
use crate::raid::{self, Raid, Raids};
use crate::speed::{self, Speed};
use crate::station::{Berth, Station, enemies_of};
use crate::surface::{self, Surface};
use crate::tank::Tank;

/// What a player can ask the world to do.
///
/// Every one of them carries the slot that sent it, because every one of them
/// can be sent by anybody: there is one ship and the crew fly it together.
/// Which of them a player is *allowed* to send is [`World::can_command`], and
/// today the answer is always yes.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Command {
    /// Fly there. While a trip is under way this is a redirect: the ship stops
    /// first and then sets off again.
    Confirm {
        slot: u32,
        target: Target,
    },
    /// Stop.
    Abort {
        slot: u32,
    },
    /// Charge the hyperdrive for a jump to another star — see
    /// [`crate::jump`]. From the helm, holding, with a working drive.
    Jump {
        slot: u32,
        star: u32,
    },
    /// Land on the planet the ship is holding over — see
    /// [`crate::surface`]. From the helm, holding in a rocky planet's or an
    /// ice world's frame, nothing under construction: a slide to the
    /// point straight over it and a descent onto the pad, and the ship is
    /// docked at the settlement there.
    Land {
        slot: u32,
    },
    SetSpeed {
        slot: u32,
        speed: Speed,
    },
    Buy {
        slot: u32,
        resource: ResourceId,
        units: u32,
    },
    Sell {
        slot: u32,
        resource: ResourceId,
        units: u32,
    },
    /// Keep so many of `resource` made — see [`World::set_craft_target`].
    /// A command rather than a setting because the benches work to it and
    /// two players' ships have to agree about what the benches are doing.
    SetCraftTarget {
        slot: u32,
        resource: ResourceId,
        units: u32,
    },
    /// Mark a rock tile at the mining site to be mined, or unmark a marked
    /// one — `(x, y)` in the ship's design tiles, where the site's rocks
    /// are. A command because the crew work to the marks and every
    /// player's ship has to agree about which rocks are wanted.
    MarkRock {
        slot: u32,
        x: i32,
        y: i32,
    },
    /// Take every mark off.
    ClearMarks {
        slot: u32,
    },
    /// Lay out a part to be built: a construction site at `origin`, turned
    /// by `rotation`, for the crew to carry the materials to and put
    /// together — see [`crate::build`]. A command because the crew work to
    /// the sites and every player's ship has to agree about what is being
    /// built where. Refused while the ship is not at rest, and where the
    /// part would not go.
    PlaceSite {
        slot: u32,
        kind: PartKind,
        origin: (u32, u32),
        rotation: Rotation,
    },
    /// Take a site away again. Whatever was carried to it was only ever
    /// spoken for and is the hold's again.
    CancelSite {
        slot: u32,
        site: u32,
    },
    /// Put what is in cell `cell` of crew member `who`'s pack into a
    /// container: the hold's count goes up by one and, for a piece of
    /// armour, the piece is `Where::Hold` with the health it had. Wants
    /// the Bim within [`data::REACH`] of a container that takes the thing
    /// — see [`World::container_takes`] — and room in its class. A broken
    /// piece is refused; discard it. Commands rather than room orders,
    /// all six of these, because the hold is the world's and every
    /// player's ship has to agree about what is in it.
    Stow {
        slot: u32,
        who: u32,
        cell: u32,
    },
    /// Take something out of a container into `who`'s pack: one piece by
    /// its id, or one unit of a resource — for an armour resource, the
    /// least damaged piece of that kind. Reach as for a stow, and a free
    /// cell in the pack.
    Fetch {
        slot: u32,
        who: u32,
        kind: FetchKind,
    },
    /// Put on what is in a pack cell: a piece goes on the part it is cut
    /// for and what was worn there comes back into the cell; a weapon
    /// swaps with the hand the same way. Wants no container — it is all
    /// in the pack already.
    Equip {
        slot: u32,
        who: u32,
        cell: u32,
    },
    /// Take off what is worn on a part, into the first free pack cell.
    Unequip {
        slot: u32,
        who: u32,
        part: Part,
    },
    /// Throw away what is in a pack cell, for good. What a broken piece
    /// is for.
    Discard {
        slot: u32,
        who: u32,
        cell: u32,
    },
    /// Take one thing off a body into `who`'s pack: `cell` is a
    /// `bims::combat::LootCell` code — the body's pack, then what it
    /// wears on the head, the body and the legs, then the weapon in its
    /// hand. The body has to be *down* — dead or out cold, asked when this
    /// lands and not when the window opened — and `who` alive, awake,
    /// aboard and within [`data::REACH`] of it, with a free cell in the
    /// pack. A piece stripped off one of the station's people becomes a
    /// piece of the world's with the health it had; a weapon goes into
    /// the pack, to be equipped from there. Nothing goes *onto* a body.
    Loot {
        slot: u32,
        who: u32,
        source: LootSource,
        cell: u32,
    },
    /// Hire the mercenary that is resident `resident` of the station the
    /// ship is tied to, crew member `who` doing the hiring: the ship
    /// docked there, the body a mercenary for hire and on its feet, `who`
    /// alive, awake, aboard and within [`data::REACH`] of it, a free bunk
    /// aboard, and the first month's fee in hand. The body walks out of
    /// the station's room into the crew's and the fee out of the money;
    /// see [`crate::mercenary`].
    Hire {
        slot: u32,
        who: u32,
        resident: u32,
    },
    /// Finish off resident `resident` of the station the ship is tied to,
    /// lying out cold, crew member `who` doing it: the station hostile —
    /// a downed crewmate or a friend's is never finished — the body alive
    /// and out cold, `who` alive, awake, aboard and with a weapon in its
    /// hand. The walk over and the shots or the swings are the room's
    /// (`Game::execute`); when the hands come off, the body is dead in its
    /// own room and `WorldEvent::Executed` is said.
    Execute {
        slot: u32,
        who: u32,
        resident: u32,
    },
    /// Take a stack off the shelf of the enemy's station the ship is tied
    /// to — slot `id` of its grid ([`World::plunder_alongside`]) — into
    /// crew member `who`'s pack: the ship docked with the rooms joined,
    /// else `NotDocked`; the station's people enemies (`World::stance`
    /// hostile — a raider's are; a friend's or a stranger's shelf is
    /// bought from across the desk), else `NotHostile`; such a slot, else
    /// `NotAboard`; `who` alive, awake, aboard and within [`data::REACH`]
    /// of one of the *station's* shelves on the joined deck, else
    /// `OutOfReach`; a free cell in the pack, else `PackFull`. As many
    /// of the stack come as the pack has cells for, one to a cell the
    /// way a fetch lands one, and the shelf keeps the rest. See
    /// [`crate::plunder`].
    Plunder {
        slot: u32,
        who: u32,
        id: u32,
    },
    /// Take the research key off the research desk of the station the
    /// ship is tied to, into crew member `who`'s pack, whichever tier
    /// lies there (`World::station_keys`: tier one on a friend's desk,
    /// tier two on every enemy's): the ship docked, a key there, `who`
    /// alive, awake, aboard and within [`data::REACH`] of that desk, and
    /// two cells free in the pack, one over the other — a key is that
    /// big. Nothing about whose desk it is or whether its people stand:
    /// a key is taken at a hostile dock in the middle of the fight. The
    /// desk is bare after; the key is stowed in the crew's own desk from
    /// the pack like anything else, and consumed there by an `Unlock`.
    TakeKey {
        slot: u32,
        who: u32,
    },
    /// Consume the research key in the crew's research desk to open the
    /// lock on node `node` (`shipdesign::research::Node`'s code) — one
    /// key, one node, and a key of **the node's own tier**
    /// (`Research::key_wanted`): a desk holding only the other tier's
    /// key is refused `NoKey` and that key stays. Wants a powered
    /// research desk aboard; a node open already, or with no lock, is
    /// refused, so no key is spent for nothing.
    Unlock {
        slot: u32,
        node: u32,
    },
    /// Queue node `node` of the research tree (`shipdesign::research::Node`'s
    /// code) for the ship's AI, with whatever it needs that is not yet
    /// known or queued ahead of it (`Research::enqueue`): the AI goes
    /// onto the head of the queue whenever it is idle and the desk has
    /// power, so a node queued while it is idle is begun the same step.
    /// Wants a research desk aboard, and refuses a node planned already
    /// or one whose chain is behind a key. A command because every
    /// player's ship has to agree about what its crew know, and what
    /// they will know next.
    Research {
        slot: u32,
        node: u32,
    },
    /// Take the AI off whatever it is on: what was put in is lost, and
    /// whatever was queued that needed it comes off the queue too. The AI
    /// goes onto what is left of the queue the same step.
    CancelResearch {
        slot: u32,
    },
    /// Take node `node` off the research queue, and with it everything
    /// queued that needed it. A node not in the queue is refused.
    Dequeue {
        slot: u32,
        node: u32,
    },
    /// Whether the crew combine matching gear at the workbench: with `on`,
    /// whenever the hold has two of a kind at the same tier and a
    /// workbench is aboard, the two go onto the bench and come off as one
    /// of the next tier a day of work later — `World::upgrade`. Never
    /// refused; off is off, and an upgrade already begun finishes.
    SetAutoUpgrade {
        slot: u32,
        on: bool,
    },
    /// Begin the day's work on the pair in the workbench's two input
    /// slots — the button in the bench's window. Refused `NoWorkbench`
    /// with none aboard, `BenchBusy` while work is under way or the
    /// output slot is still full, `NoPair` unless the two slots hold two
    /// of a kind at one tier below three. With the tick box on the world
    /// presses it itself — `tend_bench`.
    Upgrade {
        slot: u32,
    },
    /// Put what is in `who`'s pack cell into the workbench's first free
    /// input slot. Reach as for a stow, but to the workbench itself
    /// (`OutOfReach`, or `NoWorkbench` with none aboard); `NoRoom` with
    /// both input slots full, `BenchBusy` while work is under way,
    /// `NoPair` for a thing that would not pair with what is in the other
    /// slot, or a tier-three thing, `Broken` for a broken piece. Nothing
    /// in the hold moves: the thing goes from the pack to the bench.
    StowOnBench {
        slot: u32,
        who: u32,
        cell: u32,
    },
    /// Move a slot of the lockers' grid — `World::lockers`, by the slot's
    /// id — to column `x`, row `y`, `turned` a quarter round or not: what
    /// a drag in the armoury window asks, and the R key over a thing
    /// there. Refused `NoRoom` when it would not lie there — off the grid,
    /// or over another slot — or there is no such slot. Wants nobody in
    /// reach: it is tidying, and nothing leaves the lockers. A command
    /// because the grid is in the checksum: every player's ship has to
    /// agree about where the rifle lies.
    Arrange {
        slot: u32,
        class: u32,
        id: u32,
        x: u32,
        y: u32,
        turned: bool,
    },
    /// Move a thing across `who`'s pack: the one kept in `cell` — or
    /// reaching over it — so its corner is in `to`, `turned` a quarter
    /// round or not; a drag in the inventory window. The pack is the
    /// room's, but a piece's `Where::Pack` is in the checksum, so it is a
    /// command like the rest. Refused `NoRoom` when it would not lie
    /// there, `NotAboard` for nothing in the cell or no such crew member.
    Repack {
        slot: u32,
        who: u32,
        cell: u32,
        to: u32,
        turned: bool,
    },
    /// An order to the crew's room — a click on the deck, a walk, a row
    /// of a fixture's menu, a box on the Management tab — see
    /// [`bims::order::CrewOrder`]. A command because the crew's positions
    /// are in the checksum: every player's ship has to agree about who
    /// walked where, so nothing reaches into the room except through
    /// here. Three of them the world answers itself — `ToHelm`, `ToDesk`,
    /// `StandDown` — since the room does not know where the helm is;
    /// the rest go to `Game::order`. A walk with no way there is
    /// `Refused` with `DoorLocked` or `NoWayThere`; every other order
    /// shows its answer on the deck and says nothing.
    Crew {
        slot: u32,
        order: CrewOrder,
    },
    /// The same order given with Shift held (feature 69): it waits its
    /// turn on the crew member's queue behind what it is on rather than
    /// displacing it — `Game::order_later`. A walk with no way there is
    /// refused the same way; the rest are dropped without a word when
    /// their turn comes and they cannot be begun.
    CrewLater {
        slot: u32,
        order: CrewOrder,
    },
    /// Walk that player's own crew member to the helm, to stand there
    /// until sent elsewhere — what a Confirm from the strip does first.
    /// Nothing with no helm or no way to it.
    ToHelm {
        slot: u32,
    },
    /// Walk that player's own crew member to the station's trading desk.
    /// Nothing with no desk.
    ToDesk {
        slot: u32,
    },
    /// Choose that player's class (feature 74, `crate::class`): what its
    /// own crew member is. Allowed until the ship first leaves its
    /// berth, refused `ClassLocked` after. An engineer sets out with
    /// [`crate::deploy::SANDBAG_CHARGES`] sandbag kits and
    /// [`crate::deploy::SENTRY_CHARGES`] sentry kits in its
    /// pack, and a class put back to none takes them out again.
    SetClass {
        slot: u32,
        class: Class,
    },
    /// Pick a talent for that player's own crew member: one side of a
    /// pick level it has reached and not picked at yet. Refused
    /// `NotAnEngineer` without a class, `NotAPickLevel`, `LevelNotReached`
    /// or `AlreadyPicked` otherwise; a pick is never changed.
    PickTalent {
        slot: u32,
        level: u32,
        side: Side,
    },
    /// Send that player's own engineer to lay `kit` on the tile `(x, y)`
    /// of the crew's room — a room tile, which the world puts on the ship's
    /// deck or the station's by where it lies. Wants the Bim fit to act,
    /// an engineer, the kit in its pack, for a sentry the third level and
    /// a sentry short of its limit, and a tile of reachable deck floor
    /// that is not a door, holds no blocking part and no deployable
    /// (`CantDeployThere`). The Bim walks beside it and works there —
    /// see `crate::deploy`.
    Deploy {
        slot: u32,
        kit: Kit,
        x: i32,
        y: i32,
    },
    /// Take deployable `id` back into that player's own engineer's pack
    /// as a kit: the engineer fit to act and within [`data::REACH`] of it,
    /// with room in the pack. Counted as a re-used kit.
    PackUp {
        slot: u32,
        id: u32,
    },
    /// Begin a repair at the workbench — the engineer's *armourer*
    /// talent: a damaged piece in the bench's first input slot, the
    /// second empty, [`crate::deploy::ARMOUR_REPAIR_METAL`] in the hold,
    /// and that player's own engineer with the talent, who alone works
    /// the session. The piece comes out in the output slot with
    /// [`crate::class::ARMOUR_REPAIR_PER_METAL`] back on it, capped at
    /// its tier's full health.
    Repair {
        slot: u32,
    },
    /// Brace that player's own soldier, or stand it easy (feature 75,
    /// `crate::class`): braced, it holds where it stands — no errands,
    /// no running, shooting at [`class::BRACE_ACCURACY`] the odds — until
    /// this with `on` false, an order that moves it, or going down.
    /// Refused `NotASoldier` for anybody else and `OutOfReach` for one
    /// not fit to act.
    Brace {
        slot: u32,
        on: bool,
    },
    /// Throw a grenade from that player's own soldier's pack at the tile
    /// `(x, y)` of the crew's room — a room tile like a deploy's. Wants
    /// the soldier fit to act, at [`class::GRENADE_LEVEL`], a grenade in
    /// the pack, [`class::GRENADE_COOLDOWN`] past its last throw, and a
    /// tile of deck within its range with nothing opaque between
    /// (`World::can_throw`). The grenade leaves the pack at once and
    /// bursts its fuse later.
    Throw {
        slot: u32,
        x: i32,
        y: i32,
    },
    /// Link that player's own medic's heal beam to crew member `patient`
    /// — a player's Bim or a mercenary, never an enemy, never itself —
    /// or unlink with `None` (feature 76, `crate::class`,
    /// `crate::medic`). Wants the medic fit to act and the patient
    /// alive, within [`class::HEAL_BEAM_RANGE`] tiles and in its sight
    /// (`World::can_beam`). Linked, the patient's wounds and traumas do
    /// not bleed and its blood comes back at [`class::HEAL_BEAM_BLOOD`]
    /// an hour; the medic walks but does not fire. With *double link* a
    /// second patient is held beside the first, and a third takes the
    /// first's place.
    Beam {
        slot: u32,
        patient: Option<u32>,
    },
    /// Trigger that player's own medic's surge: for
    /// [`class::SURGE_MINUTES`] the medic and every linked patient take
    /// nothing from any hit. Wants the medic fit to act, at
    /// [`class::SURGE_LEVEL`], linked, and the charge full
    /// (`World::can_surge`); the charge empties.
    Surge {
        slot: u32,
    },
    /// Stand that player's own tank as a wall, or stand it down (feature
    /// 77, `crate::class`, `crate::tank`): with it on he walks at
    /// [`class::BULWARK_PACE`] and a crewmate within
    /// [`class::BULWARK_REACH`] of him that he stands between and the
    /// shooter is in cover against the shot. Refused `NotATank` for
    /// anybody else and `OutOfReach` for one not fit to act.
    Bulwark {
        slot: u32,
        on: bool,
    },
    /// That player's own tank taunts: for [`class::TAUNT_MINUTES`] of
    /// the clock every enemy within [`class::TAUNT_RADIUS`] that can see
    /// him, with him in its weapon's reach, fires at him before any
    /// nearer target. Wants the tank fit to act, at
    /// [`class::TAUNT_LEVEL`], and [`class::TAUNT_COOLDOWN`] past his
    /// last taunt (`World::can_taunt`).
    Taunt {
        slot: u32,
    },
    /// Send that player's own commander's **squad** — every crew member
    /// no player is steering, within [`class::SQUAD_RANGE`] tiles of him
    /// (the whole room with *long reach*) — after an enemy, back to a
    /// tile, or to stand its ground (feature 78, `crate::class`,
    /// `crate::commander`). Wants the commander fit to act
    /// (`World::can_squad`) and, for an attack, an enemy of the station
    /// alongside; a squad order works with the alarm and without it.
    /// The same order given again releases the squad, and so does the
    /// commander going down. It never moves, holds or aims a Bim a
    /// player steers.
    Squad {
        slot: u32,
        order: SquadAsk,
    },
    /// That player's own commander rallies: for
    /// [`class::RALLY_MINUTES`] of the clock every friendly Bim in his
    /// aura — a player's own included — shoots at
    /// [`class::RALLY_AIM`] and does not run at all. Wants the
    /// commander fit to act, at [`class::RALLY_LEVEL`], and
    /// [`class::RALLY_COOLDOWN`] past his last rally
    /// (`World::can_rally`).
    Rally {
        slot: u32,
    },
    /// That player's **standing order** to the bots that follow them
    /// (feature 84, `crate::orders`): fight their way to a tile of the
    /// crew's room and hold it, fall back to the ship, or go back to
    /// keeping to the player's side. Wants the player fit to act
    /// ([`World::can_order`]) and, for an attack, a tile of the deck.
    /// The same order given again puts them back to following. It is not
    /// a class's — every player has these two, whatever they are — and
    /// it never moves a Bim a player steers.
    Orders {
        slot: u32,
        order: Standing,
    },
    /// Take crew member `who` up into that player's own **medic's**
    /// arms, or set down whatever it is carrying with `None` (feature
    /// 86, [`crate::mercenary`]). Wants the carrier fit to act and
    /// either a medic of the class or a hired field medic
    /// ([`World::can_carry`]); the body has to be a crewmate that is out
    /// cold, in a dying state or bleeding, within
    /// [`bims::game::CARRY_REACH`] tiles, and in nobody else's arms. Carried,
    /// a body walks nowhere of its own and the carrier holds its fire
    /// and walks at [`bims::game::CARRY_PACE`] — the point of it being
    /// to get somebody out of the fire and treat them where it is
    /// quiet. A set down with empty arms does nothing and says so.
    Carry {
        slot: u32,
        who: Option<u32>,
    },
}

/// What the ship is doing.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ShipState {
    /// Alongside a station, where money works. The centre of mass sits on the
    /// station's own position.
    Docked { station: u32 },
    /// Stopped, somewhere. Not docked: a station with no airlock is somewhere
    /// to hold beside rather than somewhere to go aboard.
    Holding,
    /// Under way. The plan is the whole of the trip, and `departed` is the
    /// clock reading it began at — the two of them together are the ship's
    /// position at any moment, worked out rather than accumulated.
    Travelling { plan: Plan, departed: f64 },
    /// A trip confirmed at a station. The ship is still at its berth with
    /// the rooms joined, the station's people are going ashore and its own
    /// coming back aboard, and it casts off once they have — or once
    /// [`data::CASTING_OFF_LIMIT`] has gone by since `since`, whoever is
    /// still on the wrong side. `Ship::pending` is where it is going.
    CastingOff { station: u32, since: f64 },
    /// Pushing off the berth, straight out of the station's door: from
    /// `from`, `along` the way the door opens, for
    /// [`data::UNDOCK_MINUTES`] from `began`. The trip is planned once the
    /// ship is clear, from where it has got to, so the turn towards the
    /// target is the plan's own align phase.
    Undocking {
        station: u32,
        from: DVec2,
        along: DVec2,
        began: f64,
    },
    /// Coming alongside, over [`data::DOCK_MINUTES`] from `began`: from
    /// where the trip ended to `hold`, the point in front of the station's
    /// door the push-off ends at, turning onto the berth's heading on the
    /// way; then straight in along the door's line to the berth, and
    /// tied up at the end of it.
    Docking {
        station: u32,
        from: DVec2,
        from_heading: f64,
        hold: DVec2,
        began: f64,
    },
    /// The hyperdrive charging for a jump to `star`, since `began` on the
    /// clock, the ship holding where it is. [`data::JUMP_CHARGE_MINUTES`]
    /// later it is in that system — `World::jump`; an Abort before then
    /// leaves it holding. See [`crate::jump`].
    Charging { star: u32, began: f64 },
}

impl ShipState {
    /// The number that crosses the wasm boundary.
    pub fn code(&self) -> u32 {
        match self {
            ShipState::Docked { .. } => 0,
            ShipState::Holding => 1,
            ShipState::Travelling { .. } => 2,
            ShipState::CastingOff { .. } => 3,
            ShipState::Undocking { .. } => 4,
            ShipState::Docking { .. } => 5,
            ShipState::Charging { .. } => 6,
        }
    }

    /// The station the ship is tied up at with the rooms joined — docked,
    /// or casting off and not yet clear of the berth. What the painter
    /// draws the airlocks mated for.
    pub fn alongside(&self) -> Option<u32> {
        match self {
            ShipState::Docked { station } | ShipState::CastingOff { station, .. } => Some(*station),
            _ => None,
        }
    }

    /// The station the ship is at, tied up or on its way on or off the
    /// berth. What the local frame and the residents' room are about.
    pub fn station(&self) -> Option<u32> {
        match self {
            ShipState::Docked { station }
            | ShipState::CastingOff { station, .. }
            | ShipState::Undocking { station, .. }
            | ShipState::Docking { station, .. } => Some(*station),
            _ => None,
        }
    }
}

/// Nought to one, with no jolt at either end: how far along a push-off or
/// a docking the ship is at `t` of `over` minutes.
fn eased(t: f64, over: f64) -> f64 {
    let t = (t / over).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The ship: the design, where it is, and what it is doing.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ship {
    /// **The live ship.** The play phase changes this one; there is no second
    /// copy anywhere and no editor format that gets converted into it.
    pub design: ShipDesign,
    pub crew_count: u32,
    /// Worked out from the design, and only ever by
    /// [`World::on_ship_changed`].
    pub dynamics: Dynamics,
    /// Where design tile (0, 0) is, in system coordinates. See the module
    /// note: this is stored and the position is derived, not the other way
    /// about.
    pub anchor: DVec2,
    pub heading: f64,
    pub state: ShipState,
    /// Who set the destination. The route line on the map is drawn in their
    /// colour, which is the whole of what it is for.
    pub destination_set_by: Option<u32>,
    pub frame: Frame,
    /// Where a redirect is going once the ship has stopped, or where a
    /// departure is going once the ship is clear of its berth. Only ever
    /// the **latest** confirmed target: two Confirms in one step leave one.
    pub pending: Option<(u32, Target)>,
    /// What is in the batteries, in power units — never more than what
    /// the wired batteries hold. Full when the world opens, and moved by
    /// [`World::run_power`] alone; see [`Power`].
    pub charge: f64,
}

impl Ship {
    /// Which centre of mass the hull is hung from at this instant.
    ///
    /// A trip keeps the one it was planned with, so a design change halfway
    /// does not move a ship that is already flying — see
    /// [`World::on_ship_changed`].
    fn live_centre(&self) -> DVec2 {
        match &self.state {
            ShipState::Travelling { plan, .. } => plan.dynamics.centre_of_mass,
            _ => self.dynamics.centre_of_mass,
        }
    }

    /// Where the ship is: its centre of mass, in system coordinates.
    pub fn position(&self) -> DVec2 {
        self.anchor
            .add(angle::rotate_design(self.live_centre(), self.heading))
    }

    /// Put the centre of mass there, by moving the anchor under it.
    fn set_position(&mut self, at: DVec2) {
        self.anchor = at.sub(angle::rotate_design(self.live_centre(), self.heading));
    }
}

/// Why a world could not be started.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StartError {
    /// The star or the station the game was told to start at is not in this
    /// galaxy. The lobby names both, and a world does **not** fall back to
    /// some other dock when they are wrong: two players handed different
    /// spawns would be two players in two places, and a wrong one is a bug
    /// to show rather than to paper over.
    NoSuchStation,
    /// The accepted design does not describe a ship that can exist.
    NotAShip(flight::DynamicsError),
}

/// One star system, the ship in it, and the clock they both run on.
///
/// Not `Clone` and not `PartialEq`: the room aboard is neither, and a
/// world is compared by its [`World::checksum`] — which is the comparison
/// two machines will make, and the one a test should make too.
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct World {
    pub clock_minutes: f64,
    /// How many steps have been taken. The stamp a command carries, and the
    /// one number that is an integer rather than a float — a count of steps
    /// is the only honest way to say "this command applies *there*".
    pub steps: u64,
    pub galaxy_seed: u64,
    pub galaxy_type: GalaxyType,
    pub star_id: u32,
    pub system: StarSystem,
    /// What is left of the pool the ship was designed against. It buys
    /// nothing away from a station.
    pub money: Money,
    pub ship: Ship,
    /// The people aboard, and the room they live in: the room's whole
    /// simulation, laid out on this ship, one Bim per player at their own
    /// bunk from the moment the world opened. See [`crate::crew`].
    pub aboard: Aboard,
    /// Every station of the system as a place: a hull on a grid at a
    /// position, with a door the ship docks by. Built once, in id order.
    /// See [`crate::station`].
    pub stations: Vec<Station>,
    /// Every landable body of the system as a place — its settlement, a
    /// station the ship lands at rather than docks by, found through
    /// [`World::station`] by [`surface::surface_id`]. Rolled at the
    /// start, in body order; built when first asked for. See
    /// [`crate::surface`].
    pub surfaces: Vec<Surface>,
    /// The station the ship is near, if it is near one, as a room: the
    /// room's whole simulation again, laid out on that station with the
    /// people who live there in it, opened when the ship comes within
    /// [`data::RESIDENTS_RANGE`] of the hull and closed when it leaves. One
    /// at a time — the nearest. A derelict's room has nobody in it. See
    /// [`crate::crew`].
    pub residents: Option<Residents>,
    /// Which stations the machines hold (feature 83, [`crate::droid`]),
    /// one [`Infestation`] each, **sorted by station id** so a checksum
    /// over them means something. The crisis step will put stations on
    /// this; until then the only things that do are the `droids` and
    /// `droids_planet` probes ([`World::infest_for_probe`]). A station
    /// here has no people at all — [`World::people_of`] is nought for one
    /// — and the room laid out on it holds machines instead.
    /// In `world_checksum` whole.
    pub infested: Vec<Infestation>,
    /// The machines standing in the residents' room, waiting to be put
    /// there the first step it is open: a wave laid at a dock the room
    /// is not built for yet. Drained by `settle_droids`. Not saved and
    /// not hashed — it is empty by the end of every step it is filled in.
    #[cfg_attr(feature = "serde", serde(skip))]
    droids_to_post: Vec<bims::droid::Droid>,
    /// What tier the machines come at: one, bar the probes
    /// (`BIMS_DROID_TIER`). In `world_checksum`, since it is the size of
    /// the fight.
    droid_tier: Tier,
    /// How long after a wave is spent the next arrives, in minutes of
    /// the world's clock: [`data::DROID_REINFORCE_MINUTES`], bar the
    /// probes, which shorten it to a minute so a wave can be watched
    /// arriving. In `world_checksum` for the same reason.
    droid_reinforce: f64,
    /// The most a wave ever is: [`data::DROID_WAVE_MAX`], bar the probes,
    /// which raise it to measure what a bigger wave costs a frame
    /// (`BIMS_DROID_WAVE`). In `world_checksum` with the other two.
    droid_wave_max: u32,
    /// A wave forced to a size by a probe (`BIMS_DROID_WAVE`), whatever
    /// the formula and the cap say: the measurements' dial, `None` in
    /// the game. In `world_checksum` with the rest.
    droid_wave_forced: Option<u32>,
    /// How many waves a held station has all told, forced by a probe
    /// (`BIMS_DROID_WAVES`, and three on the `droids` commands) whatever
    /// the formula says; `None` in the game. **Neither saved nor
    /// hashed**, unlike the three above: it is read once, at the crew's
    /// first dock, and what it decides is `Infestation::waves_left`,
    /// which is both.
    #[cfg_attr(feature = "serde", serde(skip))]
    droid_waves_forced: Option<u32>,
    /// The ship's power over its live networks, worked out from the parts
    /// once per change to them — `on_ship_changed` — rather than once a
    /// step: it is a union-find over every tile of the grid, and the
    /// parts change when something is built and not otherwise.
    power_budget: shipdesign::PowerBudget,
    /// What the crew have seen, shared between all of them and never
    /// forgotten. Sorted, so a checksum over it means something.
    pub discovered: Vec<Node>,
    /// How many of each resource the crew are to keep made, by
    /// `ResourceId` — the player's standing instruction to the benches,
    /// the way the manager's stew target is to the galley. A recipe is on
    /// offer while the hold has fewer of its output than this. Nought until
    /// somebody asks: a workshop that started the game smelting the ore
    /// down would move every probe that pins the first morning.
    pub craft_targets: [u32; CARGO_SLOTS],
    /// Each crew member's body, by slot — `crates/health`: the dose and
    /// what it has done. Stage 8 of the step. Only the suit doses anybody
    /// yet: a walk outside is exposure at [`data::SUIT_INTENSITY`], and
    /// under cover the dose comes off. The room's own hunger and sleep are
    /// not in here; see the crate's note.
    pub health: Vec<health::HealthState>,
    /// One per player, in slot order. The world runs at the slowest of them.
    pub speed_requests: Vec<Speed>,
    /// The asteroids about every belt the ship has held station at, one
    /// site a belt in belt order, laid out the first time the ship came
    /// to rest there and kept — a mined tile stays mined. See
    /// [`crate::mining`].
    pub sites: Vec<MiningSite>,
    /// Bumped every time a site's rocks change, so the room rebuilds its
    /// outside grid then and only then.
    pub site_version: u64,
    /// What the walk under way has mined so far — rock, ore, galvum — to
    /// be said all at once when the Bim comes back in.
    walk_tally: (u32, u32, u32),
    /// The parts laid out to be built and not built yet, in the order they
    /// were laid out — which is the order the crew take them in. See
    /// [`crate::build`]. In `world_checksum` whole.
    pub builds: Vec<BuildSite>,
    /// The next site's id. Only ever climbs, like a part's.
    pub next_site: u32,
    /// The station the crew set out from: the one place they are at home,
    /// and friendly unless it is on the list below.
    pub home: u32,
    /// The star `home` is at. A jump takes the ship to a system with ids of
    /// its own, and home is only home while the ship is at its star.
    pub home_star: u32,
    /// The stations whose people are enemies, sorted by id: every station
    /// the generator rolled hostile (`StationBlueprint::hostile`) bar
    /// `home`, from [`World::start`], and whatever [`World::set_hostile`]
    /// — the `combat` command — has put on or taken off since. Everywhere
    /// else is neutral. See [`World::stance`]. In `world_checksum` with
    /// `home`.
    pub hostile: Vec<u32>,
    /// How many more enemies every hostile station arms over
    /// [`crate::station::enemies_of`]: nought, bar the `combat` command's
    /// arena ([`World::arena_dock_for_probe`], [`data::ARENA_GARRISON`]).
    /// In `world_checksum` with `hostile`, since it is the size of the fight.
    pub reinforcements: u32,
    /// The mercenaries hired: which crew member each is, what a month of
    /// them costs and when it next falls due. See [`crate::mercenary`].
    /// In `world_checksum` whole.
    pub hired: Vec<Hired>,
    /// How many mercenaries every friendly station has for hire at the
    /// least, whatever the roll said: nought, bar the `test` command
    /// ([`World::mercenary_for_probe`]). Not in the checksum: the crowd in
    /// a station's room never is.
    pub least_mercenaries: u32,
    /// Which crew members the world has already said are down, by slot,
    /// so [`WorldEvent::CrewDown`] is said once — the step it happens —
    /// and not every step after. See [`World::casualties`].
    crew_down: Vec<bool>,
    /// Which crew members were locked in a melee last step, by slot, so
    /// [`WorldEvent::Locked`] is said the step a lock forms and not every
    /// step it holds. See [`World::melee_locks`].
    crew_locked: Vec<bool>,
    /// Every piece of armour aboard, in id order: in the hold, in a pack,
    /// or on a body. **The hold's count of each armour resource is always
    /// the number of these `at == Hold` of that kind** — see
    /// [`crate::armour`] for the split and [`World::settle_pieces`] for
    /// what keeps it. In `world_checksum` whole.
    pub pieces: Vec<Piece>,
    /// The next piece's id. Only ever climbs, like a site's.
    pub next_piece: u32,
    /// What the ship and its hold were worth when the world opened —
    /// [`World::worth`] at step nought — fixed for the whole game. Every
    /// half of it the crew's worth has grown by since doubles the enemies
    /// an enemy station puts up; see [`crate::station::enemies_of`] and
    /// [`World::people_of`]. Not in `world_checksum`: it is a function of
    /// the design the world started on, which two clients share.
    pub start_worth: Money,
    /// What the crew know how to build and make, and what the AI is on —
    /// `shipdesign::research`. In `world_checksum` whole.
    pub research: Research,
    /// Which tier of research key each station's desk still has on it —
    /// nought for none, one, two — by index into `stations`: what the
    /// blueprint rolled (`Station::key`; tier one on a friend's desk,
    /// tier two on every enemy's), the spawn's tier one always, until a
    /// crew member takes it. In `world_checksum` whole.
    pub station_keys: Vec<u8>,
    /// The weapons in the hold, each with its tier, sorted by kind and
    /// tier. **The hold's count of each weapon resource is always the
    /// number of these of that kind** — [`World::settle_guns`] holds it
    /// the way `settle_pieces` holds the pieces'. A weapon in a pack or a
    /// hand is the room's `Item::Weapon`, tier and all, and the world
    /// keeps no copy of it. In `world_checksum` whole.
    pub guns: Vec<Weapon>,
    /// Whether the crew combine two of a kind at the same tier into one
    /// of the next whenever there is a pair — the Management tab's tick
    /// box, `Command::SetAutoUpgrade`. Off at the start. In
    /// `world_checksum`.
    pub auto_upgrade: bool,
    /// The workbench's three slots — two things going in, one coming out
    /// — what is in somebody's arms on the way to or from it, and the
    /// day's work under way on it. In `world_checksum` whole. See
    /// [`Workbench`].
    pub bench: Workbench,
    /// The shelves, the cold stores and the lockers as grids, in
    /// [`World::GRID_CLASSES`] order: where every stack, piece and gun
    /// lies and which way round, as far as they fit.
    /// [`World::settle_grids`] holds them the way `settle_pieces` holds
    /// the pieces; see [`crate::grid`]. In `world_checksum` whole.
    pub grids: [Grid; 3],
    /// Every lamp a fight has damaged, by where it hangs, with what it has
    /// left. A room is built afresh at every dock, undock and relayout,
    /// and this is what puts the damage back on its lamps, and what
    /// carries a hit on the crew's deck to the same lamp on the residents'
    /// ([`World::sync_lamps`]). In `world_checksum`, the health to a
    /// hundredth like a piece of armour's.
    pub lamps: Vec<LampDamage>,
    /// Every part on a live network, by id ascending
    /// (`shipdesign::powered_parts`), worked out beside the power budget
    /// at every change to the ship rather than once a lamp a step. What
    /// [`World::powered`] and [`World::sync_lamp_power`] read.
    powered_parts: Vec<u32>,
    /// Whether the ship was browned out at the end of the last step: what
    /// [`World::run_brownout`] compares against to say `Brownout` and
    /// `PowerRestored` once each way. Derived — `Power::brownout` off the
    /// charge and the budget, both of which are hashed — so not in the
    /// checksum itself.
    browned_out: bool,
    /// Steps the cold store has gone without power towards the next
    /// share of the food spoiling — [`data::SPOIL_STEPS`] an hour — and
    /// nought whenever it has power. An integer, in `world_checksum`
    /// after the lamps, so two clients lose the same crate on the same
    /// step.
    pub cold_store_out: u64,
    /// What a probe told the reactors to make instead of what they make
    /// (`throttle_reactors_for_probe`), kept across `on_ship_changed`.
    /// Never set by the game.
    probe_supply: Option<f64>,
    /// The raids: when the next falls due, and the one under way — a
    /// raider closing on the ship, or tied to it. See [`crate::raid`]. In
    /// `world_checksum` whole: two clients are raided together or not at
    /// all.
    pub raids: Raids,
    /// Whether the run is over: no crew member standing — dead or out
    /// cold, every one — said once as [`WorldEvent::CrewLost`] and kept,
    /// since the app ends the run on it. In `world_checksum`.
    pub lost: bool,
    /// Every enemy's shelf the crew have been alongside, by the station's
    /// id, as they have left it — laid out once, the first time the rooms
    /// are joined at a station whose people are enemies, and kept: a
    /// shelf plundered stays plundered. A raider's goes with the raider
    /// at the cast-off, and a jump leaves them all behind with the
    /// system. See [`crate::plunder`]. In `world_checksum` whole.
    pub plunder: Vec<Plunder>,
    /// What every station of this system has lost to the crew — its own
    /// people dead, its mercenaries hired away or dead — by the station's
    /// id, sorted. Added to whenever a station's room is closed
    /// ([`World::close_residents`]) and taken off the crowd the room is
    /// opened with again ([`World::people_of`], [`World::mercenaries_of`]),
    /// so a station raided stays raided. See [`crate::memory`]. In
    /// `world_checksum` whole.
    pub losses: Vec<Losses>,
    /// Every body lying on a station of this system, by the station's id,
    /// sorted — where it fell, what is still on it and what it looked
    /// like (feature 85). Filed when a station's room closes
    /// ([`World::close_residents`]) and laid back out when it opens
    /// ([`Residents::open`]), so the dead of a fight are still on the
    /// deck when the crew come back to it. A jump leaves them behind
    /// with the system, and finds them again on the way back. See
    /// [`crate::memory`]. In `world_checksum` whole.
    pub graves: Vec<Grave>,
    /// Which of this system's nodes the ship has actually been at —
    /// docked, landed, or holding beside — sorted the way
    /// [`World::discovered`] is (feature 85). What the map marks as
    /// somewhere the crew have already been; a jump files it with the
    /// system, so a chart of a system met twice says so. In
    /// `world_checksum`.
    pub visited: Vec<Node>,
    /// Every system the ship has jumped out of, as it was left: the
    /// fields above that belong to the system rather than the ship —
    /// the hostile list, the keys, the sites, the plunder, the stations'
    /// lamps, the chart and the losses — filed by star at the jump out
    /// and put back at the jump in. See [`crate::memory`]. In
    /// `world_checksum` whole.
    pub memories: Vec<SystemMemory>,
    /// Each player's class, by slot (feature 74, `crate::class`): what
    /// its own crew member is. Chosen before the game opens and kept
    /// with the start; `Command::SetClass` may change it until the ship
    /// first leaves its berth. In `world_checksum`.
    pub classes: Vec<Class>,
    /// Each crew member's way through its class, by index: experience
    /// and the picks made. Only a player's own gains any. In
    /// `world_checksum` whole.
    pub progress: Vec<Progress>,
    /// Whether the ship has ever left its first berth: what locks the
    /// classes. In `world_checksum`.
    pub undocked_once: bool,
    /// Every deployable laid and standing, in id order: an engineer's
    /// sandbags and sentries, on the ship's deck or a station's. See
    /// [`crate::deploy`]. In `world_checksum` whole.
    pub deployables: Vec<Deployable>,
    /// The next deployable's id. Only ever climbs, like a site's.
    pub next_deployable: u32,
    /// Kits each crew member has taken back — packed up, salvaged — and
    /// not laid again yet, by index: a deploy uses one of these before a
    /// fresh kit, and gives no experience for it. In `world_checksum`.
    pub reused_kits: Vec<u32>,
    /// When the cooldown on each crew member's next charge of each kit
    /// began, in clock minutes, or `None` for one not running (feature
    /// 88) — one entry a [`Kit`], by its code. `World::restock_kits`
    /// keeps it, and it is in `world_checksum`: a charge waiting is a
    /// different fight from one in the pack.
    pub kit_timers: Vec<[Option<f64>; Kit::ALL.len()]>,
    /// When each crew member last threw a grenade, in clock minutes, or
    /// never (feature 75): what the throw's cooldown is read against.
    /// In `world_checksum`.
    pub last_throw: Vec<Option<f64>>,
    /// Each crew member's medic state, by index (feature 76,
    /// `crate::medic`): who its beam holds, its surge's charge, and its
    /// field surgery this fight. Empty for anybody but a player's medic.
    /// In `world_checksum` whole.
    pub medics: Vec<Medic>,
    /// Each crew member's tank state, by index (feature 77,
    /// `crate::tank`): when he last taunted, which is the whole of it.
    /// Empty for anybody but a player's tank. In `world_checksum`.
    pub tanks: Vec<Tank>,
    /// Each crew member's commander state, by index (feature 78,
    /// `crate::commander`): when he last rallied, which is the whole of
    /// it. Empty for anybody but a player's commander. In
    /// `world_checksum`.
    pub commanders: Vec<Commander>,
    /// The one squad order the crew are under, while they are under one
    /// (feature 78): whose it is, what it is, and which crew members it
    /// reaches. `None` with none. In `world_checksum`.
    pub squad: Option<SquadOrder>,
    /// What each player's bots are under, by player slot (feature 84,
    /// `crate::orders`): [`Standing::Follow`] for a slot that has said
    /// nothing, which is every slot until somebody presses a key. As
    /// long as there are players, and in `world_checksum`.
    pub standing: Vec<Standing>,
}

/// A lamp a fight has damaged, remembered by where it hangs: which
/// station's design it is in — `None` for the ship's own — and its tile
/// there, with what it has left of `bims::sight::LAMP_HEALTH`; nought is
/// out, for good. See [`World::lamps`].
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LampDamage {
    pub station: Option<u32>,
    pub tile: (u32, u32),
    pub health: f32,
}

/// An upgrade under way at the workbench: the two of a kind and a tier in
/// its input slots becoming one of the next tier over
/// [`data::UPGRADE_SESSIONS`] hours of work. Progress is whole sessions
/// and the world's, so a Bim that leaves the bench between them loses
/// nothing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Upgrade {
    /// What is being made, as the resource it counts as in the hold.
    pub resource: ResourceId,
    /// The tier it comes out at.
    pub to: Tier,
    /// Sessions done, of [`data::UPGRADE_SESSIONS`].
    pub done: u32,
}

impl Upgrade {
    /// Whether the day's work is done.
    pub fn complete(&self) -> bool {
        self.done >= data::UPGRADE_SESSIONS
    }
}

/// The workbench as a container: three slots, the first two for the pair
/// going in and the third for what comes out, each holding a gun or a
/// piece of armour as the room would carry it — a piece on the bench is
/// *not* in [`World::pieces`], the way one in a pack is not in the hold;
/// it is pushed back when it is taken off. `carrying` is the thing in a
/// crew member's arms on the way to or from the bench (`Kind::Ferry`),
/// out of the hold and not yet on it, and `work` the day's work under
/// way on the pair. The bench with the slots is the first workbench in
/// bench order ([`World::workbench`]); a second workbench is a bench to
/// make things at and nothing more.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Workbench {
    pub slots: [Option<Item>; Workbench::SLOTS],
    pub carrying: Option<Item>,
    /// Whether what is in the arms is on its way back to the lockers rather
    /// than to the bench: the one thing about a carry the order does not
    /// say once the thing is off its slot.
    pub back: bool,
    pub work: Option<Upgrade>,
    /// The engineer repairing the piece in the first slot, by crew index,
    /// while a repair is under way (feature 74, *armourer*): the one Bim
    /// the session is offered to. `None` with no repair on.
    pub repair: Option<u32>,
}

impl Workbench {
    /// Three: two in, one out.
    pub const SLOTS: usize = 3;
    /// The two input slots.
    pub const IN: [usize; 2] = [0, 1];
    /// The output slot.
    pub const OUT: usize = 2;

    /// Whether nothing is on the bench, in the arms or under way.
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|s| s.is_none())
            && self.carrying.is_none()
            && self.work.is_none()
            && self.repair.is_none()
    }

    /// Whether anybody is at work on what is on the bench: an upgrade's
    /// day, or a repair.
    pub fn busy(&self) -> bool {
        self.work.is_some() || self.repair.is_some()
    }

    /// The first empty input slot, if either is.
    pub fn free_in(&self) -> Option<usize> {
        Workbench::IN.into_iter().find(|&i| self.slots[i].is_none())
    }

    /// What the input slots hold, as a pair to go up a tier — the same
    /// kind at the same tier below three — or why not.
    pub fn pair(&self) -> Result<(ResourceId, Tier), Refusal> {
        let (Some(a), Some(b)) = (self.slots[0], self.slots[1]) else {
            return Err(Refusal::NoPair);
        };
        let (Some(resource), Some(tier)) = (armour::resource_of_item(a), tier_of(a)) else {
            return Err(Refusal::NoPair);
        };
        if armour::resource_of_item(b) != Some(resource) || tier_of(b) != Some(tier) {
            return Err(Refusal::NoPair);
        }
        if tier.next().is_none() {
            return Err(Refusal::NoPair);
        }
        Ok((resource, tier))
    }

    /// Whether a thing could go into an input slot now: a free slot, no
    /// work under way, and — with the other slot filled — the same kind at
    /// the same tier as what is there, below tier three.
    pub fn takes(&self, item: Item) -> Result<usize, Refusal> {
        if self.busy() {
            return Err(Refusal::BenchBusy);
        }
        let (Some(resource), Some(tier)) = (armour::resource_of_item(item), tier_of(item)) else {
            return Err(Refusal::NoPair);
        };
        // A damaged piece goes onto an empty bench at any tier: a repair
        // (feature 74) wants no pair, and a tier-three piece has nowhere
        // to go up but can still be mended.
        let mending = matches!(item, Item::Armour(p) if p.health < p.stats().health)
            && self.slots.iter().all(|s| s.is_none());
        if tier.next().is_none() && !mending {
            return Err(Refusal::NoPair);
        }
        let Some(free) = self.free_in() else {
            return Err(Refusal::NoRoom);
        };
        let other = Workbench::IN.into_iter().find_map(|i| self.slots[i]);
        if let Some(other) = other
            && (armour::resource_of_item(other) != Some(resource) || tier_of(other) != Some(tier))
        {
            return Err(Refusal::NoPair);
        }
        Ok(free)
    }
}

/// A gun's or a piece's tier; a stack and a key have none.
fn tier_of(item: Item) -> Option<Tier> {
    match item {
        Item::Weapon(w) => Some(w.tier),
        Item::Armour(p) => Some(p.tier),
        Item::Stack(_) | Item::Key(_) => None,
    }
}

/// The room's `Order.recipe` for one session of an upgrade at the
/// workbench: not a recipe — past every row of `shipdesign::RECIPES`, which
/// a test pins — so `finish_craft` knows it for what it is.
pub const UPGRADE_ORDER: u32 = 1_000;

impl World {
    /// Open a world with the accepted ship docked at a station.
    ///
    /// `star_id` and `station_id` are the spawn the lobby chose — see
    /// [`StartError::NoSuchStation`] for why a wrong one is an error and not
    /// a fallback. [`spawn`] picks one for the simulation, which has no lobby.
    ///
    /// `money` is what was left of the design phase's pool. It is not
    /// converted into anything and it is not spent at Accept: it is what the
    /// crew have in hand, and it is spendable only while docked.
    pub fn start(
        design: ShipDesign,
        money: Money,
        players: u32,
        seed: u64,
        galaxy_type: GalaxyType,
        star_id: u32,
        station_id: u32,
    ) -> Result<World, StartError> {
        World::start_with_crew(
            design,
            money,
            players,
            players,
            seed,
            galaxy_type,
            star_id,
            station_id,
        )
    }

    /// [`World::start`] with a crew of `crew` aboard, of whom the first
    /// `players` are players: the rest are crew members nobody steers —
    /// they take no orders and ask for no speed, so a world for one player
    /// runs at whatever that one asks. Slot *i* is still Bim *i*. The
    /// `combat` command opens on fourteen crew this way, one a player —
    /// more than the ship has bunks: the room takes the world's count
    /// (`Game::with_layout`), and the ones past the bunks stand on the deck.
    #[allow(clippy::too_many_arguments)]
    pub fn start_with_crew(
        design: ShipDesign,
        money: Money,
        players: u32,
        crew: u32,
        seed: u64,
        galaxy_type: GalaxyType,
        star_id: u32,
        station_id: u32,
    ) -> Result<World, StartError> {
        let players = players.max(1);
        let crew = crew.max(players);
        let galaxy = Galaxy::new(seed, galaxy_type);
        let system = galaxy.system(star_id).ok_or(StartError::NoSuchStation)?;
        let mut stations = Station::all_of(&system);
        let Some(home) = stations.iter_mut().find(|s| s.id == station_id) else {
            return Err(StartError::NoSuchStation);
        };
        // The spawn is the hub whatever its seed rolled, the way it is home
        // whatever its stance rolled: a crew's first dock is the familiar
        // one, and every fixture test and the arena are built on it. The
        // other five plans are what a crew meets elsewhere
        // (`station::Plan`).
        if home.plan != crate::station::Plan::Hub {
            home.replan(crate::station::Plan::Hub);
        }
        // And its desk leans only the way its kind does: the local roll
        // is forced to nothing, so an opening pool buys the same at a
        // kind of station whatever the seed rolled, and the design
        // phase's desk (`ship::Session::design`) and this one agree.
        home.bias = economy::market::Bias::NONE;
        // Whose people are enemies: what the generator rolled, except that
        // the spawn is home whatever it rolled — a crew cannot start at an
        // enemy's, and the lobby does not offer one, but a world handed
        // one is a world that opens at home rather than one that opens
        // under fire. Sorted, since `stance` binary-searches it.
        let mut hostile: Vec<u32> = stations
            .iter()
            .filter(|s| s.hostile && s.id != station_id)
            .map(|s| s.id)
            .collect();
        // And the planets' settlements, rolled here rather than by the
        // generator (`crate::surface`), on the same list.
        let surfaces = Surface::all_of(&system, seed);
        hostile.extend(surfaces.iter().filter(|s| s.hostile).map(|s| s.id));
        hostile.sort_unstable();

        // What the crew set out with, for the enemies to be scaled against.
        let start_worth = shipdesign::Budget::spent(&design);
        let dynamics = flight::dynamics(&design, crew).map_err(StartError::NotAShip)?;
        let aboard = Aboard::new(&design, crew, seed);
        let station_keys: Vec<u8> = stations
            .iter()
            .map(|s| if s.id == station_id { 1 } else { s.key })
            .collect();
        let design_for_charge = design.clone();
        let ship = Ship {
            design,
            crew_count: crew,
            dynamics,
            anchor: DVec2::ZERO,
            heading: 0.0,
            state: ShipState::Docked {
                station: station_id,
            },
            destination_set_by: None,
            frame: Frame::Local(Node::Station(station_id)),
            pending: None,
            // Full: the ship has been sitting at a station's dock.
            charge: shipdesign::power_budget(&design_for_charge).storage,
        };

        let mut world = World {
            clock_minutes: 0.0,
            steps: 0,
            galaxy_seed: seed,
            galaxy_type,
            star_id,
            system,
            money,
            ship,
            aboard,
            stations,
            surfaces,
            residents: None,
            infested: Vec::new(),
            droids_to_post: Vec::new(),
            droid_tier: Tier::One,
            droid_reinforce: data::DROID_REINFORCE_MINUTES,
            droid_wave_max: data::DROID_WAVE_MAX,
            droid_wave_forced: None,
            droid_waves_forced: None,
            power_budget: shipdesign::power_budget(&design_for_charge),
            discovered: Vec::new(),
            craft_targets: [0; CARGO_SLOTS],
            health: vec![health::HealthState::new(); crew as usize],
            // Everybody starts at real time. Anything else would have the
            // world already moving before the first player had looked at it.
            speed_requests: vec![Speed::Real; players as usize],
            sites: Vec::new(),
            site_version: 0,
            walk_tally: (0, 0, 0),
            builds: Vec::new(),
            next_site: 1,
            home: station_id,
            home_star: star_id,
            hostile,
            reinforcements: 0,
            hired: Vec::new(),
            least_mercenaries: 0,
            crew_down: vec![false; crew as usize],
            crew_locked: vec![false; crew as usize],
            pieces: Vec::new(),
            next_piece: 1,
            start_worth,
            research: Research::new(),
            // The spawn has a key whatever it rolled: the first key is
            // how the research loop is learnt, and a crew that had to fly
            // for it would learn nothing at the start.
            station_keys,
            guns: Vec::new(),
            auto_upgrade: false,
            bench: Workbench::default(),
            grids: [Grid::default(), Grid::default(), Grid::default()],
            lamps: Vec::new(),
            powered_parts: shipdesign::powered_parts(&design_for_charge),
            browned_out: false,
            cold_store_out: 0,
            probe_supply: None,
            raids: Raids::new(seed),
            lost: false,
            plunder: Vec::new(),
            losses: Vec::new(),
            graves: Vec::new(),
            visited: Vec::new(),
            memories: Vec::new(),
            classes: vec![Class::None; players as usize],
            progress: vec![Progress::default(); crew as usize],
            undocked_once: false,
            deployables: Vec::new(),
            next_deployable: 1,
            reused_kits: vec![0; crew as usize],
            kit_timers: vec![[None; Kit::ALL.len()]; crew as usize],
            last_throw: vec![None; crew as usize],
            medics: vec![Medic::default(); crew as usize],
            tanks: vec![Tank::default(); crew as usize],
            commanders: vec![Commander::default(); crew as usize],
            squad: None,
            standing: vec![Standing::Follow; players as usize],
        };

        // Whatever armour the design was accepted carrying is so many
        // whole pieces in the hold from the first step, and everything in
        // the locker class is laid out on the lockers' grid.
        world.settle_pieces();
        world.settle_guns();
        world.settle_grids();
        // The whole system, charted. The crew picked this dock off the
        // lobby's chart of this very system — every planet and every station
        // on it — and a map that then hid what they had just been looking at
        // would be a map with nothing on it to fly to. Discovery is for what
        // the chart does not show: the systems beyond this one, when there
        // is a way there, and anything a sweep turns up on the way.
        world.discovered = world.system.nodes();
        world.discovered.sort_by_key(node_key);
        // Alongside from the first step: the ship at the station's door, and
        // whoever lives there already up and about.
        world.dock_at(station_id);
        world.settle_residents();
        Ok(world)
    }

    /// Forget the chart: only the dock is known, plus whatever the sensors
    /// reach from it. For probes of discovery, which otherwise have nothing
    /// left to discover in a system that opens charted.
    pub fn uncharted_for_probe(&mut self) {
        self.discovered.clear();
        if let ShipState::Docked { station } = self.ship.state {
            self.discovered.push(Node::Station(station));
        }
        let here = self.ship.position();
        self.discover_along(here, here, &mut Vec::new());
    }

    // --- the step ---------------------------------------------------------

    /// Advance the world by exactly one [`data::STEP_MINUTES`].
    ///
    /// **The order below is the contract.** Later steps add their systems at
    /// the numbered places and nowhere else; what they must not do is
    /// introduce a second clock or a second loop, because then there are two
    /// answers to "what time is it aboard".
    ///
    /// Commands go first so that a command stamped for this step lands before
    /// anything moves — a Confirm and the departure it causes are the same
    /// instant, not one step apart.
    pub fn step(&mut self, commands: &[Command]) -> Vec<WorldEvent> {
        let mut events = Vec::new();

        // 1. What the players asked for, in the order it arrived.
        for &command in commands {
            self.apply(command, &mut events);
        }

        // 2. The clock. Everything below reads it; nothing below sets it.
        self.clock_minutes += data::STEP_MINUTES;
        self.steps += 1;
        //    And what the clock brought due: the hired hands' month.
        self.pay_wages(&mut events);

        // 3. Flight — and the two moves either side of a trip that are not
        //    flown: pushing off the berth and coming alongside.
        let was = self.ship.position();
        self.fly(&mut events);
        self.cast_off(&mut events);
        self.come_alongside(&mut events);
        //    And the jump, which is not flown either: a charge read off the
        //    clock, and then another system altogether — see `crate::jump`.
        //    A jump replaces the chart, so what follows discovers afresh
        //    from where the ship landed.
        let jumped = self.charge_jump(&mut events);
        let was = if jumped { self.ship.position() } else { was };
        //    And the raiders, who come to the ship while it holds: contact
        //    at the edge of the radar, and a docking when they arrive —
        //    `dock_at` again, as at any berth. See `crate::raid`.
        self.run_raid(&mut events);
        let now = self.ship.position();
        //    And whether the ship has ever left its berth, which is what
        //    locks the classes (feature 74).
        if !matches!(self.ship.state, ShipState::Docked { .. }) {
            self.undocked_once = true;
        }

        // 4. What that brought into range.
        self.discover_along(was, now, &mut events);
        self.settle_frame(&mut events);
        self.settle_residents();
        //    And, at a station the machines hold (feature 83), the wave
        //    that is aboard put into the room `settle_residents` just
        //    opened — and the clock that brings the next one. Before the
        //    rooms are stepped, so a wave landing this step fights this
        //    step.
        self.settle_droids();
        self.droid_waves(&mut events);
        self.settle_site();

        // 5. Crew: the room's own update, aboard, on this clock. Bims live
        //    in ship-design coordinates and the ship's position, rotation and
        //    acceleration do not reach them. See `crates/world/src/crew.rs`.
        //    The station's residents, when the ship is near one, are the
        //    same room again on the same clock.
        //
        //    Away from a berth the ship wants somebody at the helm, and the
        //    room offers that as a job. Not while casting off: the crew are
        //    being walked home then, and the helm can wait until the ship is
        //    clear. Docked, the post the job set is lifted.
        let wants_helm = !matches!(
            self.ship.state,
            ShipState::Docked { .. } | ShipState::CastingOff { .. }
        );
        let seat = if wants_helm { self.helm_spot() } else { None };
        self.aboard.set_helm(seat);
        //    And how many of the crew are players' own: a room is built
        //    afresh at every dock and undock and starts at one, so it is
        //    told every step, before it moves anybody (`bims::order`).
        self.aboard.room.set_players(self.players());
        //    And what the benches are wanted for, worked out fresh from the
        //    hold, the targets and the power; then, after the step, what
        //    they finished, moved through the hold.
        //    First, with the tick box on, a pair of matching gear goes onto
        //    the workbench — out of the hold now — so the orders below can
        //    have a Bim work at it.
        let ferries = self.tend_bench(&mut events);
        self.aboard.room.set_ferries(ferries);
        let orders = self.craft_orders();
        self.aboard.room.set_craft_orders(orders);
        //    The sickbay's shelf is the hold's too: the bandages to hand
        //    are the hold's count, and the fibre on the cold store's shelf
        //    is the hold's count — the room keeps neither of its own
        //    aboard. What the room used and what it grew are read back
        //    after the step, below.
        //    A hired field medic fills its pack back up out of the hold
        //    first (feature 86), so the count the room is handed below
        //    is the one it will treat with this step.
        self.restock_field_medics();
        //    And every crew member fills its pack back up with dressings
        //    out of the hold out of combat (feature 87): a bandage is a
        //    thing now, spent out of the pack of whoever winds it.
        self.restock_bandages();
        self.hand_the_room_the_hold_s_medicine();
        let eva = self.eva_offer();
        self.aboard.room.set_eva(eva);
        //    And the construction sites, what each still wants, and who may
        //    go out to one beyond the hull. What the room did about them is
        //    read in stage 7.
        let builds = self.build_orders();
        let suit_ok = self.suit_ok();
        self.aboard.room.set_build_orders(builds, suit_ok);
        //    And the engineers' work (feature 74): what their talents do
        //    to the working steps, which of them keep at a deploy under
        //    fire, the sandbags laid as cover on both rooms, and the
        //    sentries on the crew's deck to be fired there. What the fight
        //    did to them is read back after `visit`.
        //    And an engineer's charges: a spent sandbag or sentry comes
        //    back into its pack on its own cooldown (feature 88), before
        //    the boxes at the foot of the screen are read.
        self.restock_kits();
        self.hand_the_room_the_engineers();
        //    And what each class wears (feature 81): drawing only, said
        //    every step because a class is chosen, a crew member joins
        //    and a save is read without anything else telling the room.
        self.hand_the_room_the_outfits();
        //    And the medics' (feature 76): every beam checked and the
        //    patients' blood held, before the soldiers' skills, since a
        //    medic beaming holds its fire through them.
        self.hand_the_room_the_medics(&mut events);
        //    And which crew members are hired **field medics** (feature
        //    86), whose business under arms is the fallen: said every
        //    step like the squad's orders, since it is the contract that
        //    knows and a save reads the contract back.
        self.hand_the_room_the_field_medics();
        //    And the tanks' (feature 77): the walls standing among the
        //    crew, before the skills, which read the bulwark off the room.
        self.hand_the_room_the_tanks();
        //    And the commander's (feature 78): the squad order pruned and
        //    handed over, before the skills, which read *focus fire* and
        //    *stand ground* off it.
        self.hand_the_room_the_squad();
        //    And every player's own two standing orders (feature 84),
        //    which the crew's bots read after the squad's: an order to
        //    the squad is a commander's and outranks the standing one.
        self.hand_the_room_the_standing();
        self.hand_the_room_the_soldiers();
        self.aboard.step();
        if let Some(residents) = &mut self.residents {
            residents.aboard.step();
        }
        self.visit(&mut events);
        self.settle_deployables(&mut events);
        self.settle_bursts(&mut events);
        self.sync_lamps();
        self.casualties(&mut events);
        let downed = self.experience(&mut events);
        self.settle_rampage(&downed);
        self.settle_medics(&mut events);
        self.settle_tanks(&mut events);
        self.melee_locks(&mut events);
        //    What the fight did to the raid: the boarders re-posted at the
        //    gangway after it, or all down and the raider a derelict; and
        //    the run over with nobody standing.
        self.settle_raid(&mut events);
        self.check_lost(&mut events);
        //    And what the fight did to the armour: the pieces in packs and
        //    on bodies are the room's, and the world's copies are read
        //    back after the step so the checksum sees them as they are.
        self.mirror_pieces(&mut events);
        self.take_the_room_s_medicine();
        for recipe in self.aboard.room.take_crafted() {
            self.finish_craft(recipe, &mut events);
        }
        self.finish_upgrade(&mut events);
        for at in self.aboard.room.take_mined() {
            self.finish_tile(at);
        }
        for _ in 0..self.aboard.room.take_walks() {
            self.finish_walk(&mut events);
        }

        // 6. Power: what the reactors made this step against what the
        //    wired consumers drew, into or out of the batteries. What is
        //    running in a brownout is `World::powered`, read by whatever
        //    draws — the benches, the research desk, the hyperdrive — and
        //    what the brownout does to the rest is `run_brownout`: the
        //    lamps dark, the bay asleep, the cold store's food spoiling.
        self.run_power();
        self.run_brownout(&mut events);
        //    And the AI's research, which runs on the research desk's power.
        self.run_research(&mut events);

        // 7. Construction: what the crew did at the sites this step — a load
        //    taken off a shelf, a load put down, a load given up, a part put
        //    together — moved through the hold. Building goes through
        //    `shipdesign::materials`, out of what is aboard, and asks
        //    `World::can_modify_part` first. See `crate::build`.
        for (site, resource, _units, who) in self.aboard.room.take_picked() {
            self.finish_pick(site, resource, who);
        }
        for site in self.aboard.room.take_dropped() {
            self.finish_drop(site);
        }
        for site in self.aboard.room.take_returned() {
            self.finish_return(site);
        }
        //    And the workbench's carries, the same three ways.
        for ferry in self.aboard.room.take_ferry_picked() {
            self.finish_ferry_pick(ferry);
        }
        for ferry in self.aboard.room.take_ferry_dropped() {
            self.finish_ferry_drop(ferry);
        }
        for ferry in self.aboard.room.take_ferry_returned() {
            self.finish_ferry_return(ferry);
        }
        for (site, who) in self.aboard.room.take_built() {
            self.finish_build(site, who, &mut events);
        }
        //    And the kits laid, each a deployable put down.
        for (who, at, sentry) in self.aboard.room.take_deployed() {
            self.finish_deploy(who, at, sentry, &mut events);
        }

        // 8. Health and radiation: each crew member's body, dosed while it
        //    is outside in a suit and sheltered otherwise. The design's
        //    exposure map — `shipdesign::exposure` — is the other input
        //    and is not wired yet.
        self.run_health(&mut events);

        events
    }

    /// Everything a command can do. Split out from [`World::step`] so the
    /// order of the step reads as an order rather than as a wall of matches.
    fn apply(&mut self, command: Command, events: &mut Vec<WorldEvent>) {
        let slot = match command {
            Command::Confirm { slot, .. }
            | Command::Abort { slot }
            | Command::Jump { slot, .. }
            | Command::Land { slot }
            | Command::SetSpeed { slot, .. }
            | Command::Buy { slot, .. }
            | Command::Sell { slot, .. }
            | Command::SetCraftTarget { slot, .. }
            | Command::MarkRock { slot, .. }
            | Command::ClearMarks { slot }
            | Command::PlaceSite { slot, .. }
            | Command::CancelSite { slot, .. }
            | Command::Stow { slot, .. }
            | Command::Fetch { slot, .. }
            | Command::Equip { slot, .. }
            | Command::Unequip { slot, .. }
            | Command::Discard { slot, .. }
            | Command::Loot { slot, .. }
            | Command::Hire { slot, .. }
            | Command::Execute { slot, .. }
            | Command::TakeKey { slot, .. }
            | Command::Plunder { slot, .. }
            | Command::Unlock { slot, .. }
            | Command::Research { slot, .. }
            | Command::CancelResearch { slot }
            | Command::Dequeue { slot, .. }
            | Command::SetAutoUpgrade { slot, .. }
            | Command::Upgrade { slot }
            | Command::StowOnBench { slot, .. }
            | Command::Arrange { slot, .. }
            | Command::Repack { slot, .. }
            | Command::Crew { slot, .. }
            | Command::CrewLater { slot, .. }
            | Command::ToHelm { slot }
            | Command::ToDesk { slot }
            | Command::SetClass { slot, .. }
            | Command::PickTalent { slot, .. }
            | Command::Deploy { slot, .. }
            | Command::PackUp { slot, .. }
            | Command::Repair { slot }
            | Command::Brace { slot, .. }
            | Command::Throw { slot, .. }
            | Command::Beam { slot, .. }
            | Command::Surge { slot }
            | Command::Bulwark { slot, .. }
            | Command::Taunt { slot }
            | Command::Squad { slot, .. }
            | Command::Rally { slot }
            | Command::Carry { slot, .. }
            | Command::Orders { slot, .. } => slot,
        };

        match command {
            // Speed is not an order to the ship, so it does not go through the
            // helm: a player who is nowhere near the controls may still say
            // they want to watch this bit slowly.
            Command::SetSpeed { speed, .. } => self.request_speed(slot, speed),
            Command::Confirm { target, .. } => {
                if !self.can_command(slot) {
                    events.push(refused(slot, Refusal::NotAtTheHelm));
                    return;
                }
                self.confirm(slot, target, events);
            }
            Command::Abort { .. } => {
                if !self.can_command(slot) {
                    events.push(refused(slot, Refusal::NotAtTheHelm));
                    return;
                }
                self.give_up(slot, events);
            }
            Command::Jump { star, .. } => {
                if !self.can_command(slot) {
                    events.push(refused(slot, Refusal::NotAtTheHelm));
                    return;
                }
                self.begin_jump(slot, star, events);
            }
            Command::Land { .. } => {
                if !self.can_command(slot) {
                    events.push(refused(slot, Refusal::NotAtTheHelm));
                    return;
                }
                self.land(slot, events);
            }
            Command::Buy {
                resource, units, ..
            } => self.buy(slot, resource, units, events),
            Command::Sell {
                resource, units, ..
            } => self.sell(slot, resource, units, events),
            Command::SetCraftTarget {
                resource, units, ..
            } => self.set_craft_target(resource, units),
            Command::MarkRock { x, y, .. } => self.mark_rock(x, y),
            Command::ClearMarks { .. } => self.clear_marks(),
            Command::PlaceSite {
                kind,
                origin,
                rotation,
                ..
            } => self.place_site(slot, kind, origin, rotation, events),
            Command::CancelSite { site, .. } => self.cancel_site(slot, site, events),
            Command::Stow { who, cell, .. } => self.stow(slot, who, cell, events),
            Command::Fetch { who, kind, .. } => self.fetch(slot, who, kind, events),
            Command::Equip { who, cell, .. } => self.equip(slot, who, cell, events),
            Command::Unequip { who, part, .. } => self.unequip(slot, who, part, events),
            Command::Discard { who, cell, .. } => self.discard(slot, who, cell, events),
            Command::Loot {
                who, source, cell, ..
            } => self.loot(slot, who, source, cell, events),
            Command::Hire { who, resident, .. } => self.hire(slot, who, resident, events),
            Command::Execute { who, resident, .. } => self.execute(slot, who, resident, events),
            Command::TakeKey { who, .. } => self.take_key(slot, who, events),
            Command::Plunder { who, id, .. } => self.plunder(slot, who, id, events),
            Command::Unlock { node, .. } => self.unlock(slot, node, events),
            Command::Research { node, .. } => self.research(slot, node, events),
            Command::CancelResearch { .. } => self.cancel_research(events),
            Command::Dequeue { node, .. } => self.dequeue(slot, node, events),
            Command::SetAutoUpgrade { on, .. } => self.auto_upgrade = on,
            Command::Upgrade { .. } => {
                if let Err(why) = self.begin_upgrade(events) {
                    events.push(refused(slot, why));
                }
            }
            Command::StowOnBench { who, cell, .. } => self.stow_on_bench(slot, who, cell, events),
            Command::Arrange {
                class,
                id,
                x,
                y,
                turned,
                ..
            } => self.arrange(slot, class, id, x, y, turned, events),
            Command::Repack {
                who,
                cell,
                to,
                turned,
                ..
            } => self.repack(slot, who, cell, to, turned, events),
            Command::Crew { order, .. } => {
                // A room built since the last step starts at one player.
                self.aboard.room.set_players(self.players());
                // A player's own order to a squad member takes it out of
                // the squad order until the next one (feature 78).
                self.take_the_ordered_out_of_squad(slot, order);
                let code = self.aboard.room.order(slot, order);
                // A walk with no way there is the one order that is said:
                // the room's `ORDER_LOCKED` and `ORDER_NOWHERE`.
                if let Some(why) = walk_refusal(code) {
                    events.push(refused(slot, why));
                }
            }
            Command::CrewLater { order, .. } => {
                self.aboard.room.set_players(self.players());
                self.take_the_ordered_out_of_squad(slot, order);
                let code = self.aboard.room.order_later(slot, order);
                if let Some(why) = walk_refusal(code) {
                    events.push(refused(slot, why));
                }
            }
            Command::ToHelm { .. } => {
                self.order_to_helm(slot);
            }
            Command::ToDesk { .. } => {
                self.walk_to_desk(slot);
            }
            Command::SetClass { class, .. } => {
                if let Err(why) = self.set_class(slot, class) {
                    events.push(refused(slot, why));
                }
            }
            Command::PickTalent { level, side, .. } => self.pick_talent(slot, level, side, events),
            Command::Deploy { kit, x, y, .. } => {
                if let Err(why) = self.deploy(slot, kit, (x, y)) {
                    events.push(refused(slot, why));
                }
            }
            Command::PackUp { id, .. } => self.pack_up(slot, id, events),
            Command::Repair { .. } => {
                if let Err(why) = self.begin_repair(slot, events) {
                    events.push(refused(slot, why));
                }
            }
            Command::Brace { on, .. } => match self.brace(slot, on) {
                Ok(()) => events.push(WorldEvent::Braced { who: slot, on }),
                Err(why) => events.push(refused(slot, why)),
            },
            Command::Throw { x, y, .. } => match self.throw(slot, (x, y)) {
                Ok(()) => events.push(WorldEvent::Thrown { who: slot }),
                Err(why) => events.push(refused(slot, why)),
            },
            Command::Beam { patient, .. } => match self.beam(slot, patient) {
                Ok(()) => events.push(WorldEvent::Beamed { who: slot, patient }),
                Err(why) => events.push(refused(slot, why)),
            },
            Command::Surge { .. } => match self.surge(slot) {
                Ok(()) => events.push(WorldEvent::Surged { who: slot }),
                Err(why) => events.push(refused(slot, why)),
            },
            Command::Bulwark { on, .. } => match self.bulwark(slot, on) {
                Ok(()) => events.push(WorldEvent::Bulwarked { who: slot, on }),
                Err(why) => events.push(refused(slot, why)),
            },
            Command::Taunt { .. } => match self.taunt(slot) {
                Ok(()) => events.push(WorldEvent::Taunted { who: slot }),
                Err(why) => events.push(refused(slot, why)),
            },
            Command::Squad { order, .. } => match self.squad_order(slot, order) {
                Ok(kind) => events.push(WorldEvent::Squadded {
                    who: slot,
                    kind: kind.map_or(u32::MAX, |k| k.code()),
                }),
                Err(why) => events.push(refused(slot, why)),
            },
            Command::Rally { .. } => match self.rally(slot) {
                Ok(()) => events.push(WorldEvent::Rallied { who: slot }),
                Err(why) => events.push(refused(slot, why)),
            },
            Command::Orders { order, .. } => match self.give_orders(slot, order) {
                Ok(kind) => events.push(WorldEvent::Ordered { who: slot, kind }),
                Err(why) => events.push(refused(slot, why)),
            },
            Command::Carry { who, .. } => match self.carry(slot, who) {
                Ok(patient) => events.push(WorldEvent::Carried { who: slot, patient }),
                Err(why) => events.push(refused(slot, why)),
            },
        }
    }

    // --- flying ------------------------------------------------------------

    /// Where the trip has got to, and what to do when it is over.
    fn fly(&mut self, events: &mut Vec<WorldEvent>) {
        let ShipState::Travelling { plan, departed } = &self.ship.state else {
            return;
        };
        let (plan, departed) = (plan.clone(), *departed);
        let elapsed = self.clock_minutes - departed;
        let total = plan.duration();
        let state = flight::state_at(&plan, elapsed.min(total));

        // Heading first: the position is worked out through it.
        self.ship.heading = state.heading;
        self.ship.set_position(state.position);
        if elapsed < total {
            return;
        }
        self.finish(&plan, events);
    }

    /// A plan has run out. Put the ship down, and pick up a redirect if one
    /// is waiting. Nothing comes out of the hold: the trip ran on the
    /// reactor, and the power stage charged it as it went.
    fn finish(&mut self, plan: &Plan, events: &mut Vec<WorldEvent>) {
        let station = plan
            .docks
            .then(|| match plan.target {
                Target::Station(id) => Some(id),
                _ => None,
            })
            .flatten();

        // The approach ends a station radius short of the berth, and going
        // alongside closes the gap — a slide and a turn over
        // `DOCK_MINUTES`, ending airlock to airlock. A station with no door
        // to dock by is simply where the ship is put.
        let hold = station.and_then(|id| self.hold_point(id));
        self.ship.state = match (station, hold) {
            (Some(id), Some(hold)) => ShipState::Docking {
                station: id,
                from: self.ship.position(),
                from_heading: self.ship.heading,
                hold,
                began: self.clock_minutes,
            },
            (Some(id), None) => ShipState::Docked { station: id },
            (None, _) => ShipState::Holding,
        };
        if let ShipState::Docked { station: id } = self.ship.state {
            self.dock_at(id);
        }
        if plan.aborting {
            self.ship.destination_set_by = None;
        }
        self.on_ship_changed();

        if plan.aborting {
            // A redirect: the stop was only ever the first half of it.
            if let Some((slot, target)) = self.ship.pending.take() {
                self.set_off(slot, target, events);
                return;
            }
        } else if let ShipState::Docking { station, .. } = self.ship.state {
            events.push(WorldEvent::Docking { station });
        } else {
            events.push(WorldEvent::Arrived { station });
        }
    }

    // --- the jump --------------------------------------------------------------

    /// Whether the ship can jump: a hyperdrive bolted to an engine on a live
    /// network — `shipdesign::hyperdrive::ready` — and the ship not browned
    /// out, since the drive is an optional consumer like the smelter.
    pub fn hyperdrive_ready(&self) -> bool {
        shipdesign::hyperdrive::ready(&self.ship.design) && self.powered(PartKind::Hyperdrive)
    }

    /// How far along a charge is, nought to one, and the star it is for.
    /// `None` unless the drive is charging. For the strip.
    pub fn jump_charge(&self) -> Option<(u32, f64)> {
        match self.ship.state {
            ShipState::Charging { star, began } => Some((
                star,
                ((self.clock_minutes - began) / data::JUMP_CHARGE_MINUTES).clamp(0.0, 1.0),
            )),
            _ => None,
        }
    }

    /// The galaxy this world is a system of, generated afresh: the stars
    /// and, per star, the system — what the galaxy chart in the map shows.
    /// Cheap for the stars; a system is generated when asked for.
    pub fn galaxy(&self) -> Galaxy {
        Galaxy::new(self.galaxy_seed, self.galaxy_type)
    }

    /// A Jump: start the charge, or say why not. Wants the ship holding —
    /// not docked, with the rooms joined and the station's people aboard,
    /// and not under way — a working drive, and a star that is another
    /// star.
    fn begin_jump(&mut self, slot: u32, star: u32, events: &mut Vec<WorldEvent>) {
        if !matches!(self.ship.state, ShipState::Holding) {
            events.push(refused(slot, Refusal::NotHolding));
            return;
        }
        if !self.hyperdrive_ready() {
            events.push(refused(slot, Refusal::NoHyperdrive));
            return;
        }
        if star == self.star_id {
            events.push(refused(slot, Refusal::SameStar));
            return;
        }
        if self.galaxy().star(star).is_none() {
            events.push(refused(slot, Refusal::NoSuchStar));
            return;
        }
        if self.under_construction() {
            events.push(refused(slot, Refusal::UnderConstruction));
            return;
        }
        self.ship.destination_set_by = Some(slot);
        self.ship.state = ShipState::Charging {
            star,
            began: self.clock_minutes,
        };
        events.push(WorldEvent::Charging { slot, star });
    }

    /// The charge, read off the clock: nothing until it is done, and then
    /// the jump. `true` when the ship has just landed somewhere else.
    fn charge_jump(&mut self, events: &mut Vec<WorldEvent>) -> bool {
        let ShipState::Charging { star, began } = self.ship.state else {
            return false;
        };
        if self.clock_minutes - began < data::JUMP_CHARGE_MINUTES {
            return false;
        }
        // The drive has to still be there and working when it fires: a
        // brownout or a deconstruction in the twenty seconds leaves the
        // ship where it was.
        if !self.hyperdrive_ready() {
            self.ship.state = ShipState::Holding;
            self.ship.destination_set_by = None;
            events.push(WorldEvent::JumpFailed);
            return false;
        }
        self.jump(star, events)
    }

    /// Put the ship in another system. **The one place a system is
    /// replaced**, so this is the list of what belongs to one: the chart,
    /// the stations and their people, the hostile list, the mining site,
    /// the keys on the stations' desks, the local frame. The ship, the crew,
    /// the hold, the sites on the deck, the research, the money and the
    /// hired hands come along. It lands holding, in empty space
    /// (`jump::landing_point`), pointing the way it was, with only what its
    /// own sensors reach on the chart.
    fn jump(&mut self, star: u32, events: &mut Vec<WorldEvent>) -> bool {
        let Some(system) = self.galaxy().system(star) else {
            self.ship.state = ShipState::Holding;
            self.ship.destination_set_by = None;
            events.push(WorldEvent::JumpFailed);
            return false;
        };
        let at = crate::jump::landing_point(&system);
        // The system left behind, as it was left — its station's room
        // closed first, so its dead are counted — filed under its star.
        self.close_residents();
        self.remember_system();
        self.star_id = star;
        self.system = system;
        self.stations = Station::all_of(&self.system);
        self.surfaces = Surface::all_of(&self.system, self.galaxy_seed);
        // A raider closing on the ship is left behind with the system.
        if matches!(self.raids.state, Raid::Closing { .. }) {
            self.raids.state = Raid::Quiet;
            events.push(WorldEvent::RaidCancelled);
        }
        // The system arrived at: as the crew left it, if they have been
        // here, else as the generator rolled it.
        if !self.recall_system(star) {
            let mut hostile: Vec<u32> = self
                .stations
                .iter()
                .filter(|s| s.hostile)
                .map(|s| s.id)
                .chain(self.surfaces.iter().filter(|s| s.hostile).map(|s| s.id))
                .collect();
            hostile.sort_unstable();
            self.hostile = hostile;
            self.station_keys = self.stations.iter().map(|s| s.key).collect();
            self.reinforcements = 0;
            self.sites.clear();
            self.plunder.clear();
            self.lamps.retain(|d| d.station.is_none());
            self.discovered.clear();
            self.losses.clear();
            self.graves.clear();
            self.visited.clear();
        }
        self.site_version += 1;
        self.ship.state = ShipState::Holding;
        self.ship.destination_set_by = None;
        self.ship.pending = None;
        self.ship.set_position(at);
        self.ship.frame = Frame::Space;
        events.push(WorldEvent::Jumped { star });
        true
    }

    /// The two halves of leaving a station. Waiting at the berth for
    /// everybody to be on their own side of the airlock — the station's
    /// people are sent ashore and the crew called back every step, and
    /// nobody is teleported — and then the push-off, read off the clock.
    /// The trip is planned from wherever the push-off ends; a plan that
    /// will not go leaves the ship holding there, as it would anywhere.
    fn cast_off(&mut self, events: &mut Vec<WorldEvent>) {
        match self.ship.state.clone() {
            ShipState::CastingOff { station, since } => {
                self.aboard.send_everybody_home(&self.ship.design);
                let overdue = self.clock_minutes - since >= data::CASTING_OFF_LIMIT;
                if !self.aboard.everybody_home(&self.ship.design) && !overdue {
                    return;
                }
                let from = self.ship.position();
                // Off a planet the ship climbs to the point straight over it,
                // whichever way the settlement's gate opens: up, and the
                // pad's few tiles west of the planet's middle taken out.
                let surface = surface::surface_body(station);
                let along = match surface.and_then(|body| self.above(body)) {
                    Some(over) => {
                        let up = over.sub(from);
                        let len = up.length();
                        if len > 0.0 {
                            up.scale(1.0 / len)
                        } else {
                            dvec2(0.0, 1.0)
                        }
                    }
                    None => self.way_out(station).unwrap_or(DVec2 { x: 0.0, y: 1.0 }),
                };
                self.unjoin_rooms();
                self.ship.state = ShipState::Undocking {
                    station,
                    from,
                    along,
                    began: self.clock_minutes,
                };
                // Pushing off a raider is the end of it: the derelict is
                // gone from the world, and its people with it.
                if raid::raider_index(station).is_some() {
                    self.raids.state = Raid::Quiet;
                    self.hostile.retain(|&id| id != station);
                    self.plunder.retain(|p| p.station != station);
                    self.losses.retain(|l| l.station != station);
                }
                let slot = self.ship.pending.map(|(slot, _)| slot).unwrap_or(0);
                events.push(match surface {
                    Some(body) => WorldEvent::LiftedOff { body },
                    None => WorldEvent::Undocking { slot },
                });
            }
            ShipState::Undocking {
                station,
                from,
                along,
                began,
            } => {
                let t = self.clock_minutes - began;
                // A lift-off climbs to where a trip to the planet ends, and
                // takes its own time over it.
                let over_planet = surface::surface_body(station).and_then(|body| self.above(body));
                let (reach, over) = match over_planet {
                    Some(up) => (from.distance(up), data::LIFT_MINUTES),
                    None => (self.undock_distance(), data::UNDOCK_MINUTES),
                };
                let reach = reach * eased(t, over);
                self.ship.set_position(from.add(along.scale(reach)));
                if t < over {
                    return;
                }
                self.ship.state = ShipState::Holding;
                match self.ship.pending.take() {
                    Some((slot, target)) => self.set_off(slot, target, events),
                    None => self.ship.destination_set_by = None,
                }
            }
            _ => {}
        }
    }

    /// How far the ship pushes off before it turns: its own span, so a
    /// turn in place clears the station's hull whichever way it goes.
    fn undock_distance(&self) -> f64 {
        self.ship.design.build_area as f64 * shipdesign::TILE as f64
    }

    /// The last stretch of a trip to a station, read off the clock, in two
    /// halves of [`data::DOCK_MINUTES`]: to the hold point in front of the
    /// door, turning onto the berth's heading on the way, and then straight
    /// in along the door's line — the push-off backwards. Tied up at the
    /// end: `dock_at` sets the ship down exactly and joins the rooms.
    fn come_alongside(&mut self, events: &mut Vec<WorldEvent>) {
        let ShipState::Docking {
            station,
            from,
            from_heading,
            hold,
            began,
        } = self.ship.state.clone()
        else {
            return;
        };
        let t = self.clock_minutes - began;
        // A landing is the same two halves, over the planet then down
        // onto the pad, taking longer.
        let surface = surface::surface_body(station);
        let over = if surface.is_some() {
            data::LAND_MINUTES
        } else {
            data::DOCK_MINUTES
        };
        let half = over / 2.0;
        if let Some(berth) = self.berth_at(station) {
            if t < half {
                let f = eased(t, half);
                let turn = angle::shortest(from_heading, berth.heading);
                self.ship.heading = angle::wrap(from_heading + turn * f);
                self.ship.set_position(from.add(hold.sub(from).scale(f)));
            } else {
                let f = eased(t - half, half);
                self.ship.heading = berth.heading;
                self.ship
                    .set_position(hold.add(berth.position.sub(hold).scale(f)));
            }
        }
        if t < over {
            return;
        }
        self.ship.state = ShipState::Docked { station };
        self.dock_at(station);
        events.push(match surface {
            Some(body) => WorldEvent::Landed { body },
            None => WorldEvent::Arrived {
                station: Some(station),
            },
        });
    }

    /// A Confirm. From rest it is a departure; under way it is a redirect,
    /// which is a stop followed by a departure.
    fn confirm(&mut self, slot: u32, target: Target, events: &mut Vec<WorldEvent>) {
        if let Some(node) = target.node()
            && !self.discovered.contains(&node)
        {
            events.push(WorldEvent::PlanFailed {
                slot,
                error: PlanError::TargetUndiscovered,
            });
            return;
        }
        // The ship does not move while it is built on — see `crate::build`.
        // Asked of a ship at rest only: one already under way is being
        // redirected, and nothing is built on it in the meantime anyway.
        if self.at_rest() && self.under_construction() {
            events.push(refused(slot, Refusal::UnderConstruction));
            return;
        }

        match self.ship.state.clone() {
            ShipState::Travelling { plan, departed } => {
                self.ship.pending = Some((slot, target));
                self.ship.destination_set_by = Some(slot);
                if plan.aborting {
                    // Already stopping. Only the latest target is kept, and it
                    // has just been kept; there is nothing else to do.
                    return;
                }
                self.begin_abort(&plan, self.clock_minutes - departed);
                events.push(WorldEvent::Aborted { slot });
            }
            ShipState::Docked { station } => {
                // Refused now rather than after everybody has been sent
                // ashore for nothing: what would not fly from the berth
                // will not fly from a ship's length outside it either.
                if let Err(error) = self.plan_from_here(target) {
                    events.push(WorldEvent::PlanFailed { slot, error });
                    return;
                }
                self.ship.pending = Some((slot, target));
                self.ship.destination_set_by = Some(slot);
                self.ship.state = ShipState::CastingOff {
                    station,
                    since: self.clock_minutes,
                };
                events.push(WorldEvent::CastingOff { slot });
            }
            // Still leaving: only where to is changed.
            ShipState::CastingOff { .. } | ShipState::Undocking { .. } => {
                self.ship.pending = Some((slot, target));
                self.ship.destination_set_by = Some(slot);
            }
            ShipState::Docking { .. } => {
                events.push(refused(slot, Refusal::ComingAlongside));
            }
            // A trip called while the drive charges is the charge given up
            // for it: the drive is the ship's other way of going somewhere,
            // and the helm holds one intention at a time.
            ShipState::Charging { .. } => {
                self.ship.state = ShipState::Holding;
                self.set_off(slot, target, events);
            }
            ShipState::Holding => self.set_off(slot, target, events),
        }
    }

    /// An Abort. Unlike a redirect it leaves nothing pending. At the berth
    /// it is the departure called off — the ship stays tied up; pushing
    /// off, the push-off finishes and the ship holds there.
    fn give_up(&mut self, slot: u32, events: &mut Vec<WorldEvent>) {
        let under_way = match &self.ship.state {
            ShipState::Charging { .. } => {
                // The charge called off: the ship was holding, and holds.
                self.ship.state = ShipState::Holding;
                self.ship.destination_set_by = None;
                events.push(WorldEvent::Aborted { slot });
                return;
            }
            ShipState::Travelling { plan, departed } => Some((plan.clone(), *departed)),
            ShipState::CastingOff { station, .. } => {
                self.ship.state = ShipState::Docked { station: *station };
                self.ship.pending = None;
                self.ship.destination_set_by = None;
                events.push(WorldEvent::Aborted { slot });
                return;
            }
            ShipState::Undocking { .. } => {
                self.ship.pending = None;
                self.ship.destination_set_by = None;
                events.push(WorldEvent::Aborted { slot });
                return;
            }
            _ => None,
        };
        let Some((plan, departed)) = under_way else {
            events.push(refused(slot, Refusal::NotTravelling));
            return;
        };
        self.ship.pending = None;
        if plan.aborting {
            return;
        }
        self.begin_abort(&plan, self.clock_minutes - departed);
        events.push(WorldEvent::Aborted { slot });
    }

    fn begin_abort(&mut self, plan: &Plan, elapsed: f64) {
        let stop = flight::abort(plan, elapsed);
        self.ship.state = ShipState::Travelling {
            plan: stop,
            departed: self.clock_minutes,
        };
    }

    /// Plan a trip from where the ship is standing, and go.
    fn set_off(&mut self, slot: u32, target: Target, events: &mut Vec<WorldEvent>) {
        match self.plan_from_here(target) {
            Ok(plan) => {
                self.ship.destination_set_by = Some(slot);
                self.ship.pending = None;
                self.ship.state = ShipState::Travelling {
                    plan,
                    departed: self.clock_minutes,
                };
                self.unjoin_rooms();
                self.leave_site(events);
                events.push(WorldEvent::Departed { slot });
            }
            Err(error) => {
                self.ship.pending = None;
                self.ship.destination_set_by = None;
                events.push(WorldEvent::PlanFailed { slot, error });
            }
        }
    }

    fn plan_from_here(&self, target: Target) -> Result<Plan, PlanError> {
        let at = self
            .target_position(target)
            .ok_or(PlanError::TargetUndiscovered)?;
        flight::plan_trip(
            &self.ship.dynamics,
            self.ship.position(),
            self.ship.heading,
            target,
            at,
        )
    }

    /// Where a target is in the system.
    pub fn target_position(&self, target: Target) -> Option<DVec2> {
        match target {
            Target::Point(at) => Some(at),
            // A station is flown to by its door, not by its middle: the
            // trip is aimed at the hold point in front of it and ends a
            // station radius short of that, and coming alongside covers
            // the rest — so the ship never comes to rest inside a station.
            Target::Station(id) => self
                .hold_point(id)
                .or_else(|| self.system.absolute_position(Node::Station(id))),
            other => self.system.absolute_position(other.node()?),
        }
    }

    // --- stations ------------------------------------------------------------

    /// A station of the system by id — or a planet's settlement by its
    /// [`surface::surface_id`], built the first time it is asked for. Every
    /// place the ship can be tied up at is found here, so everything that
    /// works at a station works on a surface.
    pub fn station(&self, id: u32) -> Option<&Station> {
        self.stations
            .iter()
            .find(|s| s.id == id)
            .or_else(|| self.surface_of(id).map(Surface::station))
            .or_else(|| self.raids.station().filter(|s| s.id == id))
    }

    /// The settlement whose station id that is, if it is one's.
    fn surface_of(&self, id: u32) -> Option<&Surface> {
        surface::surface_body(id).and_then(|body| self.surface(body))
    }

    /// The settlement on `body`, if the body has ground to stand one on.
    pub fn surface(&self, body: u32) -> Option<&Surface> {
        self.surfaces.iter().find(|s| s.body == body)
    }

    /// The planet the ship is on the ground of — tied up at its settlement,
    /// or casting off from it — if it is on one.
    pub fn landed(&self) -> Option<u32> {
        self.ship.state.alongside().and_then(surface::surface_body)
    }

    /// The planet the ship is coming down onto, and how far along the
    /// landing is, nought to one — the whole of it, the slide over the
    /// planet and the descent — while it is landing.
    pub fn landing(&self) -> Option<(u32, f64)> {
        let ShipState::Docking { station, began, .. } = self.ship.state else {
            return None;
        };
        let body = surface::surface_body(station)?;
        let done = ((self.clock_minutes - began) / data::LAND_MINUTES).clamp(0.0, 1.0);
        Some((body, done))
    }

    /// The planet the ship is climbing away from, and how far along the
    /// lift-off is, nought to one, while it is lifting off.
    pub fn lifting(&self) -> Option<(u32, f64)> {
        let ShipState::Undocking { station, began, .. } = self.ship.state else {
            return None;
        };
        let body = surface::surface_body(station)?;
        let done = ((self.clock_minutes - began) / data::LIFT_MINUTES).clamp(0.0, 1.0);
        Some((body, done))
    }

    /// Whether the ship can land where it is: holding — not docked, not
    /// under way, not charging — in the frame of a planet with ground,
    /// and nothing under construction. The planet, or why not.
    pub fn can_land(&self) -> Result<u32, Refusal> {
        if !matches!(self.ship.state, ShipState::Holding) {
            return Err(Refusal::NotHolding);
        }
        let body = match self.ship.frame {
            Frame::Local(Node::Body(body)) if self.surface(body).is_some() => body,
            _ => return Err(Refusal::NoPlanetHere),
        };
        if self.under_construction() {
            return Err(Refusal::UnderConstruction);
        }
        Ok(body)
    }

    /// The point straight over a planet a landing comes down from and a
    /// lift-off climbs to: [`data::LANDING_HEIGHT`] north of it, where a
    /// trip to the body ends.
    fn above(&self, body: u32) -> Option<DVec2> {
        let at = self.system.absolute_position(Node::Body(body))?;
        Some(at.add(dvec2(0.0, data::LANDING_HEIGHT)))
    }

    /// A Land. The landing is a docking at the settlement — the same
    /// state, the same slide-and-turn then straight in (`come_alongside`),
    /// over [`data::LAND_MINUTES`] rather than a docking's — with the hold
    /// point straight over the planet, so the ship approaches from the
    /// top and comes down onto the pad; `dock_at` at the end of it ties
    /// the ship up and joins the rooms as at any berth.
    fn land(&mut self, slot: u32, events: &mut Vec<WorldEvent>) {
        let body = match self.can_land() {
            Ok(body) => body,
            Err(why) => {
                events.push(refused(slot, why));
                return;
            }
        };
        let Some(hold) = self.above(body) else {
            events.push(refused(slot, Refusal::NoPlanetHere));
            return;
        };
        self.ship.state = ShipState::Docking {
            station: surface::surface_id(body),
            from: self.ship.position(),
            from_heading: self.ship.heading,
            hold,
            began: self.clock_minutes,
        };
        self.ship.destination_set_by = Some(slot);
        self.leave_site(events);
        events.push(WorldEvent::Landing { body });
    }

    // --- raids ------------------------------------------------------------

    /// Whether the ship is holding where a raid could find it: at rest,
    /// tied to nothing, charging nothing.
    fn holding(&self) -> bool {
        matches!(self.ship.state, ShipState::Holding)
    }

    /// The raiders, once a step — see [`crate::raid`]. A raid falls due
    /// on the schedule and comes at the first hold on or after it, once
    /// the crew have left home: contact at the edge of the radar, every
    /// player's speed put back to 1×. A raider that has closed finds the
    /// ship holding where it was and docks to it — or finds it gone and
    /// the raid is off.
    fn run_raid(&mut self, events: &mut Vec<WorldEvent>) {
        // Leaving home is any state that is not tied up: the first push
        // off the start station's berth.
        if !matches!(
            self.ship.state,
            ShipState::Docked { .. } | ShipState::CastingOff { .. }
        ) {
            self.raids.left_home = true;
        }
        match self.raids.state.clone() {
            Raid::Quiet => {
                let minute = self.clock_minutes.floor() as u64;
                if !self.raids.left_home || minute < self.raids.due || !self.holding() {
                    return;
                }
                // A ship with no port has nothing a raider can dock by: no
                // raid, and the schedule waits.
                if shipdesign::port(&self.ship.design).is_none() {
                    return;
                }
                let n = self.raids.next;
                let boarders = raid::boarders_of(
                    self.aboard.crew_count(),
                    self.worth(),
                    self.start_worth,
                    self.days_gone(),
                );
                let range = self.detection_range();
                let at = self.ship.position();
                let from = at.add(Raids::bearing(self.galaxy_seed, n).scale(range));
                // Whole minutes out, rounded up, so the arrival is a
                // minute of the clock like the due date.
                let minutes = (range / data::RAIDER_SPEED).ceil().max(1.0);
                self.raids.state = Raid::Closing {
                    n,
                    boarders,
                    from,
                    at,
                    began: self.clock_minutes,
                    arrives: self.clock_minutes + minutes,
                };
                self.raids.next = n + 1;
                self.raids.due = minute + Raids::gap(self.galaxy_seed, n + 1);
                // The reset: everybody back to real time, once. Not a
                // veto — anybody may raise it again.
                for request in &mut self.speed_requests {
                    *request = Speed::Real;
                }
                events.push(WorldEvent::RaidContact {
                    boarders,
                    minutes: minutes as u32,
                });
            }
            Raid::Closing {
                n,
                boarders,
                at,
                arrives,
                ..
            } => {
                if self.clock_minutes < arrives {
                    return;
                }
                // Arrived. The ship holding where it was — a hold is a
                // stop, so anywhere else is a ship that left — or the raid
                // is off.
                let moved = self.ship.position().distance(at) > shipdesign::TILE as f64;
                if !self.holding() || moved {
                    self.raids.state = Raid::Quiet;
                    events.push(WorldEvent::RaidCancelled);
                    return;
                }
                let station = Raids::build_raider(
                    self.galaxy_seed,
                    n,
                    boarders,
                    &self.ship.design,
                    self.ship.dynamics.centre_of_mass,
                    self.ship.position(),
                );
                let id = station.id;
                self.raids.state = Raid::Docked {
                    station,
                    boarders,
                    breached: false,
                    repelled: false,
                };
                // Its people are enemies from the first step it is there.
                if let Err(i) = self.hostile.binary_search(&id) {
                    self.hostile.insert(i, id);
                }
                self.leave_site(events);
                self.ship.state = ShipState::Docked { station: id };
                self.ship.destination_set_by = None;
                self.ship.pending = None;
                self.dock_at(id);
                // Nobody asked it aboard: the airlock is locked in its
                // face, and its boarders have to force it.
                self.seal_against_raid();
                self.on_ship_changed();
                events.push(WorldEvent::RaidBoarded { boarders });
            }
            Raid::Docked { .. } => {}
        }
    }

    /// The ship's airlock as a door of the crew's room: the one the
    /// port's opening is in, through the ship's shift on a joined deck.
    /// `None` without a port, or while the room is the ship's alone and
    /// the door is not in it.
    pub fn ship_airlock_door(&self) -> Option<usize> {
        let port = shipdesign::port(&self.ship.design)?;
        let (px, py) = port.centre;
        let at = self.aboard.offset.add(dvec2(px, py));
        self.aboard
            .room
            .door_index_at(bims::math::vec2(at.x as f32, at.y as f32))
    }

    /// Lock the ship's airlock against a raider the step it ties up: the
    /// crew's lock, the one the panel sets, so the panel unlocks it too.
    /// The boarders posted at the gangway find it shut and force it
    /// (`bims::game::Game::breach`); `sync_doors` carries the lock into
    /// their room next step. Nothing without a port, or with the door
    /// already locked.
    fn seal_against_raid(&mut self) {
        let Some(door) = self.ship_airlock_door() else {
            return;
        };
        if self.aboard.room.ship_door_is_locked(door) {
            return;
        }
        self.aboard
            .room
            .order_door_now(door, bims::door::Order::Lock);
    }

    /// How far the raider's boarders have got with forcing the ship's
    /// airlock, nought to one, while they are at it — the bar over the
    /// door, for the warning. `None` when nobody is heaving at it: not
    /// raided, the door not locked, or the boarders not there yet.
    pub fn raid_forcing(&self) -> Option<f32> {
        if !self.raided() {
            return None;
        }
        let door = self.ship_airlock_door()?;
        self.aboard.room.door_states().get(door)?.smash
    }

    /// Whether the ship's airlock still holds a raider's boarders out:
    /// tied to a raider, and the lock not yet given.
    pub fn raid_airlock_holds(&self) -> bool {
        matches!(
            self.raids.state,
            Raid::Docked {
                breached: false,
                ..
            }
        ) && self.raided()
    }

    /// How many minutes of the clock until a closing raider arrives, if
    /// one is closing — the warning's countdown. Never below nought.
    pub fn raid_minutes_left(&self) -> Option<f64> {
        match &self.raids.state {
            Raid::Closing { arrives, .. } => Some((arrives - self.clock_minutes).max(0.0)),
            _ => None,
        }
    }

    /// The raid after the fight, while the raider is tied up: the boarders
    /// posted at the gangway again — a fight drops every post — the
    /// airlock's lock gone said once (`RaidBreached`, whether they forced
    /// it or the crew opened it), and, every one of them down, the raider
    /// a derelict, said once.
    fn settle_raid(&mut self, events: &mut Vec<WorldEvent>) {
        let airlock_locked = self
            .ship_airlock_door()
            .is_some_and(|door| self.aboard.room.ship_door_is_locked(door));
        let Raid::Docked {
            boarders,
            breached,
            repelled,
            ..
        } = &mut self.raids.state
        else {
            return;
        };
        let Some(residents) = &mut self.residents else {
            return;
        };
        // Tied up, not casting off: then the boarders are being sent
        // ashore, and a post at the gangway would send them back.
        if !matches!(self.ship.state, ShipState::Docked { .. }) || *repelled {
            return;
        }
        if !*breached && !airlock_locked {
            *breached = true;
            events.push(WorldEvent::RaidBreached {
                boarders: *boarders,
            });
        }
        let all_down =
            (0..residents.aboard.count() as usize).all(|who| residents.aboard.room.is_down(who));
        if all_down {
            *repelled = true;
            events.push(WorldEvent::RaidRepelled);
            return;
        }
        residents.post_boarders(&self.ship.design);
    }

    /// The end: nobody of the crew standing — alive and awake — and the
    /// run is over, said once and kept.
    fn check_lost(&mut self, events: &mut Vec<WorldEvent>) {
        if self.lost {
            return;
        }
        let room = &self.aboard.room;
        let standing = (0..self.aboard.crew_count() as usize)
            .any(|who| room.is_alive(who) && !room.is_unconscious(who));
        if !standing {
            self.lost = true;
            events.push(WorldEvent::CrewLost);
        }
    }

    /// A raid this instant, for looking at: off the berth and holding a
    /// little way out, the next raid brought forward to now and contact
    /// made — and, with `dock`, the raider arrived without the closing,
    /// tied to the ship with the boarders posted and on their way.
    /// `BIMS_RAID=1` and `BIMS_RAID=contact` in the app. `None`, and
    /// nothing moved, when the ship has no port for a raider to dock by.
    pub fn raid_for_probe(&mut self, dock: bool) -> Option<Vec<WorldEvent>> {
        self.hold_off_for_probe()?;
        self.raid_now_for_probe();
        let mut events = Vec::new();
        self.run_raid(&mut events);
        if !dock {
            return matches!(self.raids.state, Raid::Closing { .. }).then_some(events);
        }
        if let Raid::Closing { arrives, .. } = &mut self.raids.state {
            *arrives = self.clock_minutes;
        }
        self.run_raid(&mut events);
        self.raided().then_some(events)
    }

    /// The raid under way, if one is: for the painter and the tests.
    pub fn raid(&self) -> &Raid {
        &self.raids.state
    }

    /// Where the raider is while it closes — for the map and the window.
    pub fn raid_contact(&self) -> Option<DVec2> {
        self.raids.contact(self.clock_minutes)
    }

    /// Whether the ship is tied to a raider.
    pub fn raided(&self) -> bool {
        self.ship
            .state
            .station()
            .is_some_and(|id| raid::raider_index(id).is_some())
    }

    /// Off the berth and holding a little way out in open space, where a
    /// raider can find the ship: what every staged raid starts from.
    /// `None`, and nothing moved, when the ship has no port for a raider
    /// to dock by.
    fn hold_off_for_probe(&mut self) -> Option<()> {
        shipdesign::port(&self.ship.design)?;
        self.undock_for_probe();
        // Clear of the berth it was at, so the raider is not laid across
        // the station's hull: a little way out into open space.
        let out = self.ship.position().add(dvec2(0.0, -30_000.0));
        self.put_for_probe(out);
        Some(())
    }

    /// A raid on its way, for watching it come: off the berth and holding
    /// a little way out, and the next raid brought forward to `minutes`
    /// of the clock from now — contact at the first step on or after that
    /// minute, and the raider then closing at its own pace. The `raid`
    /// command in the app. False, and nothing moved, when the ship has
    /// no port for a raider to dock by.
    pub fn raid_coming_for_probe(&mut self, minutes: u64) -> bool {
        if self.hold_off_for_probe().is_none() {
            return false;
        }
        self.raid_due_for_probe(minutes);
        true
    }

    /// Bring the next raid forward to now: the schedule's next minute is
    /// this one, so the next holding step is contact. For the probes and
    /// the tests, which cannot wait a day.
    pub fn raid_now_for_probe(&mut self) {
        self.raid_due_for_probe(0);
    }

    /// Bring the next raid forward to `minutes` of the clock from now —
    /// this minute and that many on — the crew counted as having left
    /// home. The first holding step on or after it is contact.
    pub fn raid_due_for_probe(&mut self, minutes: u64) {
        self.raids.left_home = true;
        self.raids.due = self.clock_minutes.floor() as u64 + minutes;
    }

    /// What the ship and everything in its hold are worth now, in whole
    /// euros: every part's price and every unit of cargo at its **book
    /// value** — `shipdesign::Budget::spent`, `economy::trade_price` a
    /// unit, the same everywhere; a valuation, not what any desk would
    /// pay. The crew's money in hand is not in it: net worth here is the
    /// ship and its contents, which is what an enemy sizes up.
    pub fn worth(&self) -> Money {
        shipdesign::Budget::spent(&self.ship.design)
    }

    /// How many whole days the game has run: `clock_minutes` — elapsed
    /// time since the world opened, not the crew's calendar
    /// ([`World::day`]) — over a day, floored, and read in whole minutes
    /// first the way the raids' schedule is, so a server catching up
    /// counts the same day. The enemies' base grows by one every
    /// [`data::ENEMIES_DAYS`] of it (`station::base_by_day`).
    pub fn days_gone(&self) -> u32 {
        let minutes = self.clock_minutes.floor() as u64;
        (minutes / (time::DAY as u64)) as u32
    }

    /// How many people a station's room is opened with: the people who
    /// live there, or, at an enemy's, the enemies — [`enemies_of`] the
    /// crew's number and worth against [`World::start_worth`], and the
    /// days gone by ([`World::days_gone`]) — **less its dead**
    /// ([`World::losses`]): the people the crew have killed there stay
    /// dead, so a station raided opens with its survivors and one emptied
    /// opens empty. Nobody on a derelict either way. Asked when the room
    /// opens, so a station keeps the crowd it was reached with until the
    /// ship has gone and come back.
    pub fn people_of(&self, station: &Station) -> u32 {
        // A station the machines hold has no people at all (feature 83):
        // whoever lived there is gone, and what the room is opened with
        // is a wave of droids (`World::settle_droids`).
        if self.is_droid_held(station.id) {
            return 0;
        }
        if station.residents() == 0 {
            return 0;
        }
        // A raider carries the boarders it was rolled with — `boarders_of`
        // at contact, kept as its population — whatever the crew are
        // worth by the time it arrives.
        if raid::raider_index(station.id).is_some() {
            return station.residents();
        }
        let crowd = match self.stance(station.id) {
            Stance::Hostile => enemies_of(
                self.aboard.crew_count(),
                self.worth(),
                self.start_worth,
                self.days_gone(),
            )
            .saturating_add(self.reinforcements)
            .min(data::ENEMIES_MAX),
            Stance::Friendly | Stance::Neutral => station.residents(),
        };
        crowd.saturating_sub(self.losses_at(station.id).dead)
    }

    /// How many mercenaries for hire live at a station, on top of
    /// [`World::people_of`]: none at an enemy's or on a derelict, else
    /// [`mercenary::how_many`] the crew's worth against
    /// [`World::start_worth`] off the station's seed — at least
    /// `least_mercenaries` for the `test` command — **less those gone**
    /// ([`World::losses`]): one hired onto the crew, or dead, is not there
    /// to hire again. Asked when the room opens, like the people, so a
    /// station keeps its offer until the ship has gone and come back.
    pub fn mercenaries_of(&self, station: &Station) -> u32 {
        if self.is_droid_held(station.id) {
            return 0;
        }
        if station.residents() == 0 || self.stance(station.id) == Stance::Hostile {
            return 0;
        }
        mercenary::how_many(self.worth(), self.start_worth, station.map_seed)
            .max(self.least_mercenaries)
            .saturating_sub(self.losses_at(station.id).mercenaries)
    }

    /// Where this ship docks at that station: airlock to airlock, outside
    /// its hull. `None` for a station that is not there or has no door.
    pub fn berth_at(&self, id: u32) -> Option<Berth> {
        self.station(id)?
            .berth(&self.ship.design, self.ship.dynamics.centre_of_mass)
    }

    /// The point in front of that station's door where a push-off ends and
    /// a docking begins its last straight run: the berth, a ship's span out
    /// along the way the door opens. `None` where there is no berth.
    pub fn hold_point(&self, id: u32) -> Option<DVec2> {
        // A settlement's is the point straight over its planet: where a
        // landing comes down from and a lift-off climbs to.
        if let Some(body) = surface::surface_body(id) {
            return self.above(body);
        }
        let berth = self.berth_at(id)?;
        let out = self.way_out(id)?;
        Some(berth.position.add(out.scale(self.undock_distance())))
    }

    /// The way out of that station's door: the way the door opens, or, for
    /// a station with no door, straight away from its middle.
    fn way_out(&self, id: u32) -> Option<DVec2> {
        let station = self.station(id)?;
        if let Some((_, out)) = station.face() {
            return Some(out);
        }
        let away = self.ship.position().sub(station.centre());
        let len = away.length();
        (len > 0.0).then(|| away.scale(1.0 / len))
    }

    /// Put the ship at its berth: the one place, with [`World::start`], that
    /// it is set down rather than flown. Heading first, because the position
    /// is worked out through it.
    fn dock_at(&mut self, id: u32) {
        let Some(berth) = self.berth_at(id) else {
            if let Some(at) = self.system.absolute_position(Node::Station(id)) {
                self.ship.set_position(at);
            }
            return;
        };
        self.ship.heading = berth.heading;
        self.ship.set_position(berth.position);
        self.join_rooms(id, berth);
    }

    /// Docked: the ship's room and the station's become one deck, so the
    /// crew can walk through the airlocks. See [`crate::docking`]. The
    /// station's people are **not** in it: they keep their own room, laid
    /// out on the station's design with the station's galley, heads, bunks
    /// and benches for fixtures, and go on living there on their own
    /// timetable, under their own manager. The joined room is the ship's
    /// fixtures and the ship's crew, with the station's deck to walk on and
    /// the station's fixtures as furniture to walk round.
    fn join_rooms(&mut self, id: u32, berth: Berth) {
        let Some(station) = self.station(id) else {
            return;
        };
        let Some(joined) = crate::docking::join(
            &self.ship.design,
            self.ship.dynamics.centre_of_mass,
            station,
            &berth,
        ) else {
            return;
        };
        let (design, count, mercs, seed) = (
            station.design.clone(),
            self.people_of(station),
            self.mercenaries_of(station),
            station.map_seed,
        );
        // The residents' room: the one already open, or opened now if the
        // ship arrived faster than the room did.
        let residents = match self.residents.take() {
            Some(residents) if residents.station == id => residents,
            other => {
                // Another station's room, if that is what was open, is
                // closed the way the range closes one: its dead counted.
                self.residents = other;
                self.close_residents();
                self.open_residents(id, &design, count, mercs, seed)
            }
        };
        self.drop_loads();
        let ship_seed = self.galaxy_seed ^ self.steps;
        // The crew out of the old room, and then what leaving banked in
        // it — a sheaf in somebody's hands goes into the store as the
        // errand is given up — into the hold before the room is dropped.
        let mut old = std::mem::replace(&mut self.aboard, Aboard::new(&self.ship.design, 1, 0));
        let crew = old.room.take_crew();
        let banked = bank_medicine(&mut self.ship.design, &mut old.room);
        // On a planet, the ground beyond the town.
        let terrain = surface::surface_body(id)
            .and_then(|body| self.surface(body))
            .map(|surface| surface.terrain());
        self.aboard = Aboard::joined(
            joined,
            &self.ship.design,
            &design,
            crew,
            ship_seed,
            self.clock_minutes,
            terrain,
        );
        // Landed, the town's ground on the joined deck is under the sky.
        if surface::surface_body(id).is_some() {
            self.aboard.daylight_over_station();
        }
        let mut residents = residents;
        // Its doors are the joined room's to draw — one picture of each,
        // in one state — and the joined room is told where the residents
        // are every step (`visit`) so its doors open for them too.
        residents.aboard.room.set_doors_drawn(false);
        // And its fog is the joined room's, over both decks: this room
        // draws none, and its people only where the crew can see them.
        residents.aboard.room.set_fog(bims::sight::Fog::None);
        // And the ship is on its deck too, turned into the station's frame,
        // so its people can follow the crew aboard.
        if let Some(station) = self.station(id) {
            residents.join(
                &self.ship.design,
                self.ship.dynamics.centre_of_mass,
                station,
                &berth,
                self.clock_minutes,
            );
            // A settlement's guard takes up its post on the ground.
            residents.post_guard(station);
            // And a raider's boarders come for the ship: posted at the
            // gangway, they walk the passage onto its deck.
            if raid::raider_index(id).is_some() {
                residents.post_boarders(&self.ship.design);
            }
        }
        self.residents = Some(residents);
        self.apply_stances();
        self.restore_lamps();
        // And the laid sandbags onto both fresh rooms (feature 74).
        self.sync_deployed_cover();
        self.lay_plunder();
        if banked {
            self.on_ship_changed();
        }
    }

    /// Whose a station is, to the crew: home is friendly, a station on the
    /// hostile list is hostile, and everywhere else is neutral.
    pub fn stance(&self, station: u32) -> Stance {
        // A station the machines hold is an enemy's whatever it was
        // before (feature 83): its room is hostile, which is the switch
        // that puts its bodies at war.
        if self.is_droid_held(station) {
            Stance::Hostile
        } else if self.hostile.binary_search(&station).is_ok() {
            Stance::Hostile
        } else if station == self.home && self.star_id == self.home_star {
            Stance::Friendly
        } else {
            Stance::Neutral
        }
    }

    /// Make a station's people enemies, or not. The rooms that are open
    /// on it are told at once — and its own room, if it is open, is opened
    /// again with the crowd the new stance calls for
    /// ([`World::people_of`]): the `combat` command turns the dock hostile
    /// with its residents' room already open, and two residents are not
    /// an enemy's garrison.
    pub fn set_hostile(&mut self, station: u32, hostile: bool) {
        match (self.hostile.binary_search(&station), hostile) {
            (Err(i), true) => self.hostile.insert(i, station),
            (Ok(i), false) => {
                self.hostile.remove(i);
                // Its shelf is the desk's again, and what was on it as
                // loot is forgotten: peace is the world as it was.
                self.plunder.retain(|p| p.station != station);
            }
            _ => {}
        }
        self.reopen_residents(station);
        self.apply_stances();
        self.lay_plunder();
    }

    /// The residents' room opened again with the crowd the station now
    /// calls for, if it is open on that station and the crowd has
    /// changed: a stance turned hostile arms a garrison where two
    /// residents stood, and a station taken by the machines has no
    /// people at all (feature 83). Nothing when the count is what it
    /// was. Docked there, the fresh room is looked into the way
    /// `join_rooms` left it.
    ///
    /// The comparison is against the room's **Bims** — `crew_count`, not
    /// `Aboard::count`, which counts the machines too — since what is
    /// being reopened is the people.
    fn reopen_residents(&mut self, station: u32) {
        let reopen = self
            .residents
            .as_ref()
            .filter(|r| r.station == station)
            .and_then(|r| {
                let s = self.station(station)?;
                let count = self.people_of(s);
                let mercs = self.mercenaries_of(s);
                (count + mercs != r.aboard.room.crew_count())
                    .then(|| (s.design.clone(), s.map_seed))
            });
        let Some((design, seed)) = reopen else {
            return;
        };
        // The old crowd's dead counted before the new crowd stands,
        // and the new crowd is the fewer for them.
        self.close_residents();
        let (count, mercs) = self
            .station(station)
            .map(|s| (self.people_of(s), self.mercenaries_of(s)))
            .unwrap_or((0, 0));
        let mut residents = self.open_residents(station, &design, count, mercs, seed);
        if self.aboard.is_joined() && self.ship.state.station() == Some(station) {
            residents.aboard.room.set_doors_drawn(false);
            residents.aboard.room.set_fog(bims::sight::Fog::None);
            if let (Some(s), Some(berth)) = (self.station(station), self.berth_at(station)) {
                residents.join(
                    &self.ship.design,
                    self.ship.dynamics.centre_of_mass,
                    s,
                    &berth,
                    self.clock_minutes,
                );
            }
        }
        self.residents = Some(residents);
    }

    /// Tell every room open on a station whose it is: the residents' room
    /// its own stance, for its fog and the ring under each of its people,
    /// and the joined deck which of its tiles are the station's. Asked
    /// whenever a room opens or a stance changes.
    fn apply_stances(&mut self) {
        let docked = self.ship.state.station();
        let ashore = self.residents.as_ref().map(|r| self.stance(r.station));
        if let (Some(residents), Some(stance)) = (&mut self.residents, ashore) {
            residents.aboard.room.set_stance(stance);
            residents
                .aboard
                .room
                .set_hostile_bodies(stance == Stance::Hostile);
        }
        let foreign = match (self.aboard.station_box, docked) {
            (Some((lo, hi)), Some(station)) if self.aboard.is_joined() => Some((
                bims::math::Rect::from_corners(
                    bims::math::vec2(lo.x as f32, lo.y as f32),
                    bims::math::vec2(hi.x as f32, hi.y as f32),
                ),
                self.stance(station),
            )),
            _ => None,
        };
        // On a planet everything but the ship's own box is the town's,
        // the ground round the ship included.
        let on_plane = self.aboard.room.plane().is_some();
        match (foreign, on_plane) {
            (Some((_, stance)), true) => {
                // The hull's own box, not the build area's: the ground beside
                // the hull is the planet's.
                let t = shipdesign::TILE as f64;
                let mut span: Option<(u32, u32, u32, u32)> = None;
                for part in &self.ship.design.parts {
                    for (x, y) in part.tiles() {
                        span = Some(match span {
                            None => (x, y, x, y),
                            Some((a, b, c, d)) => (a.min(x), b.min(y), c.max(x), d.max(y)),
                        });
                    }
                }
                let (x0, y0, x1, y1) = span.unwrap_or((0, 0, 0, 0));
                let lo = self
                    .aboard
                    .offset
                    .add(worldgen::math::dvec2(x0 as f64 * t, y0 as f64 * t));
                let hi = self.aboard.offset.add(worldgen::math::dvec2(
                    (x1 + 1) as f64 * t,
                    (y1 + 1) as f64 * t,
                ));
                self.aboard.room.set_foreign_outside(
                    bims::math::Rect::from_corners(
                        bims::math::vec2(lo.x as f32, lo.y as f32),
                        bims::math::vec2(hi.x as f32, hi.y as f32),
                    ),
                    stance,
                );
            }
            (Some((rect, stance)), false) => self.aboard.room.set_foreign(Some(rect), stance),
            (None, _) => self.aboard.room.set_foreign(None, Stance::Neutral),
        }
    }

    /// Docked, the residents walk about in their own room and the joined
    /// room draws the doors: it is told where they are, in its own units,
    /// so a door opens for a resident walking through it the way it does
    /// for the crew. Once a step, after both rooms have moved. And, at a
    /// hostile station, where the fight crosses between the two rooms —
    /// both ways; see the crate note.
    /// The station's doors are in two rooms while the ship is docked — the
    /// joined deck the crew walk, and the station's own room its people
    /// walk — and a lock has to be the same door's in both: one the crew
    /// set from the panel stops the station's people (and is what they
    /// smash), one an enemy set sealing itself in stops the crew, and a
    /// smash in the station's room is the bar the crew watch on the deck.
    /// Each door is matched by its middle through the station frame, and
    /// whichever room changed the lock last is copied to the other; the
    /// smashing is copied one way, since nobody heaves at a door on the
    /// joined deck.
    fn sync_doors(&mut self) {
        let Some(residents) = &mut self.residents else {
            return;
        };
        let theirs = residents.aboard.room.door_states();
        let mine = self.aboard.room.door_states();
        for (j, state) in theirs.iter().enumerate() {
            let design =
                dvec2(state.centre.x as f64, state.centre.y as f64).sub(residents.aboard.offset);
            let Some(on_deck) = self.aboard.from_station(design) else {
                continue;
            };
            let Some(i) = self
                .aboard
                .room
                .door_index_at(bims::math::vec2(on_deck.x as f32, on_deck.y as f32))
            else {
                continue;
            };
            let Some(ours) = mine.get(i) else {
                continue;
            };
            if ours.changed {
                residents
                    .aboard
                    .room
                    .mirror_door_lock(j, ours.locked, ours.by_crew);
                self.aboard.room.door_change_seen(i);
            } else if state.changed || ours.locked != state.locked {
                self.aboard
                    .room
                    .mirror_door_lock(i, state.locked, state.by_crew);
                residents.aboard.room.door_change_seen(j);
            }
            self.aboard.room.mirror_door_smash(i, state.smash);
        }
    }

    /// Which design a lamp of `aboard` hangs in — `foreign` for one in
    /// the room's foreign box, `home` else — and its tile there.
    fn lamp_key(
        aboard: &Aboard,
        home: Option<u32>,
        foreign: Option<u32>,
        at: bims::math::Vec2,
    ) -> (Option<u32>, (u32, u32)) {
        let (is_foreign, p) = aboard.design_of(dvec2(at.x as f64, at.y as f64));
        let t = shipdesign::TILE as f64;
        let tile = (
            (p.x / t).floor().max(0.0) as u32,
            (p.y / t).floor().max(0.0) as u32,
        );
        (if is_foreign { foreign } else { home }, tile)
    }

    /// The lamp of `aboard` that hangs at `tile` of a design — the
    /// foreign one's or the room's own — with its index, if it is on
    /// this deck.
    fn lamp_index(aboard: &Aboard, foreign: bool, tile: (u32, u32)) -> Option<usize> {
        let t = shipdesign::TILE as f64;
        let middle = dvec2((tile.0 as f64 + 0.5) * t, (tile.1 as f64 + 0.5) * t);
        let p = aboard.room_of(foreign, middle)?;
        aboard
            .room
            .lamp_at(bims::math::vec2(p.x as f32, p.y as f32))
            .map(|(i, _)| i)
    }

    /// The lamps, both rooms' and the record's, kept the same: every
    /// lamp a bolt landed on this step on the crew's deck — the one
    /// deck bolts fly on — is remembered by where it hangs, and then
    /// every lamp remembered is set on whichever rooms it hangs in, which
    /// carries the hit to the residents' mirror of the same lamp and
    /// puts the damage back on a room built afresh. Every step, since a
    /// room is built afresh in more places than one.
    fn sync_lamps(&mut self) {
        let station = self.residents.as_ref().map(|r| r.station);
        for i in self.aboard.room.take_lamp_changes() {
            let Some(lamp) = self.aboard.room.lamps().get(i).copied() else {
                continue;
            };
            let (which, tile) = Self::lamp_key(&self.aboard, None, station, lamp.at);
            match self
                .lamps
                .iter_mut()
                .find(|d| d.station == which && d.tile == tile)
            {
                Some(d) => d.health = lamp.health,
                None => self.lamps.push(LampDamage {
                    station: which,
                    tile,
                    health: lamp.health,
                }),
            }
        }
        if let Some(residents) = &mut self.residents {
            // Bolts do not fly there: nothing to remember, only to drain.
            residents.aboard.room.take_lamp_changes();
        }
        self.restore_lamps();
    }

    /// Every lamp remembered damaged, set so on the rooms it hangs in —
    /// and the ship's lamps' power with them ([`World::sync_lamp_power`]).
    fn restore_lamps(&mut self) {
        let docked = self.ship.state.alongside();
        for d in &self.lamps {
            // The crew's deck: the ship's lamps always, the docked
            // station's while docked there.
            let mine = match d.station {
                None => Some(false),
                Some(id) if self.aboard.is_joined() && docked == Some(id) => Some(true),
                Some(_) => None,
            };
            if let Some(foreign) = mine
                && let Some(i) = Self::lamp_index(&self.aboard, foreign, d.tile)
            {
                self.aboard.room.set_lamp_health(i, d.health);
            }
            // The residents' room: the station's own, and the ship's while
            // it is on their deck.
            if let Some(residents) = &mut self.residents {
                let theirs = match d.station {
                    None => Some(true),
                    Some(id) if id == residents.station => Some(false),
                    Some(_) => None,
                };
                if let Some(foreign) = theirs
                    && let Some(i) = Self::lamp_index(&residents.aboard, foreign, d.tile)
                {
                    residents.aboard.room.set_lamp_health(i, d.health);
                }
            }
        }
        // And the ship's lamps' power, which a fresh room does not know
        // either.
        self.sync_lamp_power();
    }

    /// A lamp as it is to be drawn: what it has left of its health as a
    /// share, and how bright it is shown this frame — the ship's own at
    /// `station` `None`, else that station's, at its tile of the design.
    /// Read off whichever room has it — the crew's deck, or the
    /// residents' — and, for a station's lamp out of every room, off the
    /// record; whole and steady for one nobody has shot.
    pub fn lamp_look(&self, station: Option<u32>, tile: (u32, u32)) -> (f32, f32) {
        let docked = self.ship.state.alongside();
        let live = match station {
            None => Self::lamp_index(&self.aboard, false, tile)
                .and_then(|i| self.aboard.room.lamps().get(i)),
            Some(id) if self.aboard.is_joined() && docked == Some(id) => {
                Self::lamp_index(&self.aboard, true, tile)
                    .and_then(|i| self.aboard.room.lamps().get(i))
            }
            Some(id) => self
                .residents
                .as_ref()
                .filter(|r| r.station == id)
                .and_then(|r| {
                    Self::lamp_index(&r.aboard, false, tile)
                        .and_then(|i| r.aboard.room.lamps().get(i))
                }),
        };
        if let Some(lamp) = live {
            return (lamp.health / bims::sight::LAMP_HEALTH, lamp.level);
        }
        match self
            .lamps
            .iter()
            .find(|d| d.station == station && d.tile == tile)
        {
            Some(d) => {
                let share = d.health / bims::sight::LAMP_HEALTH;
                (share, if d.health > 0.0 { 1.0 } else { 0.0 })
            }
            None => (1.0, 1.0),
        }
    }

    fn visit(&mut self, events: &mut Vec<WorldEvent>) {
        if !self.aboard.is_joined() {
            // Out of reach of each other: nobody is anybody's target. The
            // residents' room goes on after the ship has gone, and a
            // target list left on it would keep its people at war with
            // nobody there.
            if let Some(residents) = &mut self.residents {
                residents.aboard.room.set_hostiles(Vec::new());
            }
            return;
        }
        // And which of them are down, so a body among them is one the
        // crew can right-click and loot; after the positions, since the
        // positions clear it.
        let (visitors, down): (Vec<DVec2>, Vec<bool>) = match &self.residents {
            Some(residents) => (0..residents.aboard.count())
                .map(|who| {
                    // **A wreck is not a body to loot** (feature 83): a
                    // machine carries nothing, so a click on one is a
                    // click on the deck and the Loot window never opens
                    // on it. Everything else that asks whether a droid
                    // is down asks the room; this list is the click's
                    // alone.
                    let machine = who >= residents.aboard.room.crew_count();
                    (
                        residents.aboard.position(who),
                        !machine && residents.aboard.room.is_down(who as usize),
                    )
                })
                .unzip(),
            None => (Vec::new(), Vec::new()),
        };
        self.aboard.visit(&visitors, &down);
        self.sync_doors();
        // And which of them may be spoken to — the mercenaries for hire —
        // so a click on one on its feet is a click on it. After the
        // positions too, for the same reason.
        if let Some(residents) = &self.residents {
            self.aboard
                .room
                .set_visitors_hailable(&residents.hailable());
        }
        // And which of them the crew can see, for their own room to draw.
        // Off the last trace, which is a frame's rather than a step's —
        // nobody crosses a bulkhead in a sixtieth of a minute.
        let seen = self.aboard.seen(&visitors);
        if let Some(residents) = &mut self.residents {
            residents.aboard.room.set_seen(&seen);
        }
        // The fight, which crosses between the two rooms both ways. A
        // hostile station's people are the crew's targets, at those same
        // positions, and what the crew's bolts landed on one since the
        // last step comes off the body in the residents' room. The crew
        // are the residents' targets in turn, in the station's own units,
        // and the residents' shots — recorded, not flown, since the body
        // they are aimed at is on the joined deck — are fired here as
        // hostile bolts. A bolt therefore flies in one room only: the
        // crew's, where both sides' bodies can be seen from.
        let hostile = self
            .residents
            .as_ref()
            .is_some_and(|r| self.stance(r.station) == Stance::Hostile);
        let hits = self.aboard.room.take_hits();
        let executed = self.aboard.room.take_executed();
        // The engineers' sentries (feature 74), for the residents to be
        // handed after the crew: each at its spot in the station's own
        // units, with its rifle. Worked out before the residents' room
        // is borrowed, since the owner's talents are the world's.
        let sentry_targets: Vec<Option<(DVec2, Weapon)>> = self
            .sentries_in_room()
            .into_iter()
            .map(|(d, at)| {
                self.aboard
                    .to_station(dvec2(at.x as f64, at.y as f64))
                    .map(|p| (p, self.sentry_weapon(d.owner_slot)))
            })
            .collect();
        // And which of the crew a taunt is running on (feature 77): the
        // radius in room units — nought for anybody not taunting — and
        // whether it pulls a charging blade too. Worked out here for the
        // same reason as the sentries: the talents are the world's.
        // This is the room the enemies aim and charge in, whoever they
        // are — a station's people, a garrison, a raider's boarders.
        let taunting: Vec<f32> = (0..self.aboard.crew_count())
            .map(|who| {
                if self.is_taunting(who) {
                    self.taunt_radius(who) * shipdesign::TILE as f32
                } else {
                    0.0
                }
            })
            .collect();
        let magnet: Vec<bool> = (0..self.aboard.crew_count())
            .map(|who| self.has_talent(who, Talent::Magnet))
            .collect();
        let Some(residents) = self.residents.as_mut().filter(|_| hostile) else {
            self.aboard.room.set_hostiles(Vec::new());
            if let Some(residents) = &mut self.residents {
                residents.aboard.room.set_hostiles(Vec::new());
            }
            return;
        };
        let shift = residents.aboard.offset;
        let room = &mut residents.aboard.room;
        let bims = room.crew_count() as usize;
        for hit in hits {
            let who = hit.who;
            if who >= room.body_count() as usize || !room.is_alive(who) {
                continue;
            }
            // A hit past the room's Bims landed on one of the machines
            // (feature 83): a droid's four parts are not a body's three,
            // so the part is read off the hit's own roll rather than off
            // `hit.part`, and there is no armour step and no blood.
            if let Some(i) = who.checked_sub(bims) {
                let part = bims::droid::DroidPart::hit_by(hit.roll);
                room.strike_droid(i, part, hit.damage);
                if let Some(last) = residents.last_hit_by.get_mut(who) {
                    *last = hit.by;
                }
                continue;
            }
            // A burst's hit splashes the deck the way a cut does (feature
            // 75); the rest land as they always did. Whose it was is kept
            // for the *rampage*.
            if hit.blast {
                room.blast(who, hit.part, hit.damage);
            } else {
                room.strike(who, hit.part, hit.damage, hit.cut);
            }
            if let Some(last) = residents.last_hit_by.get_mut(who) {
                *last = hit.by;
            }
        }
        // And the bodies the crew finished off where they lay: dead in
        // their own room, if still down — one that came round on the way
        // over is left as it is — and said.
        for (who, resident) in executed {
            if resident < room.crew_count() as usize && room.execute_body(resident) {
                events.push(WorldEvent::Executed {
                    who: who as u32,
                    resident: resident as u32,
                });
            }
        }
        // Who is down, asked of the room rather than read off the hits: a
        // shot to the head kills at the top of the body's next tick with
        // the total still well above nought, and a wound nobody dresses
        // kills without a hit landing at all. Said once each.
        // A machine destroyed is one of these too: the lists are sized
        // by the body count, so a wave landing grows them (feature 83).
        let bodies = room.body_count() as usize;
        residents.down.resize(bodies, false);
        residents.xp_down.resize(bodies, false);
        residents.xp_dead.resize(bodies, false);
        residents.last_hit_by.resize(bodies, None);
        residents.fee.resize(bodies, None);
        residents.medic.resize(bodies, false);
        residents.grave.resize(bodies, false);
        let room = &mut residents.aboard.room;
        for who in 0..residents.down.len().min(bodies) {
            let down = !room.is_alive(who);
            if down && !residents.down[who] {
                residents.down[who] = true;
                // A machine is said as a machine: it has no name, and
                // the log would otherwise call a wreck Sanne.
                let machine = who
                    .checked_sub(bims)
                    .and_then(|i| room.droid(i))
                    .map(|d| d.kind.code());
                events.push(match machine {
                    Some(kind) => WorldEvent::DroidDown {
                        station: residents.station,
                        who: who as u32,
                        kind,
                    },
                    None => WorldEvent::EnemyDown {
                        station: residents.station,
                        who: who as u32,
                    },
                });
            }
        }
        // Where the crew are, for the residents to shoot at — set before
        // their shots are read so a room that has just gone to war has
        // something to aim at from its first step. Each with what it
        // carries, since a schword within reach locks a gunner in a melee
        // (`bims::combat`), and at the peek it leans out to while it
        // peeks, with a word that it does, since a bolt reaching a body
        // in cover is dodged half the time.
        // In the residents' room's own units: the station's design units
        // plus the shift its deck took when the ship was turned onto it.
        let mut crew: Vec<Option<(bims::math::Vec2, Weapon)>> = self
            .aboard
            .crew_ashore()
            .into_iter()
            .enumerate()
            .map(|(who, p)| {
                let weapon = self
                    .aboard
                    .room
                    .weapon(who)
                    .unwrap_or(WeaponKind::LaserPistol.basic());
                p.map(|p| {
                    let at = p.add(shift);
                    (bims::math::vec2(at.x as f32, at.y as f32), weapon)
                })
            })
            .collect();
        // And the engineers' sentries after them (feature 74), each at
        // its spot with its rifle: the nearest-target rule includes them,
        // and a hit past the crew's count is a hit on one.
        let crew_count = crew.len();
        for target in sentry_targets {
            crew.push(target.map(|(p, weapon)| {
                let at = p.add(shift);
                (bims::math::vec2(at.x as f32, at.y as f32), weapon)
            }));
        }
        room.set_hostiles(crew.clone());
        room.set_hostiles_peeking(&self.aboard.crew_peeking());
        room.set_hostiles_taunting(&taunting, &magnet);
        // And the odds each dodges a bolt for its armour, the same way.
        let crew_dodge: Vec<f32> = (0..self.aboard.crew_count())
            .map(|who| self.aboard.room.dodge(who as usize))
            .collect();
        room.set_hostiles_dodge(&crew_dodge);
        let shots = room.take_shots();
        if let Some((origin, ex, ey)) = self.aboard.station_frame {
            // A point of the residents' room onto the joined deck: its
            // shift off, then the station's frame.
            let on_deck = |p: bims::math::Vec2| {
                let p = dvec2(p.x as f64, p.y as f64).sub(shift);
                let at = origin.add(ex.scale(p.x)).add(ey.scale(p.y));
                bims::math::vec2(at.x as f32, at.y as f32)
            };
            for shot in shots {
                if shot.melee {
                    // A blow, not a shot: nothing flies. It is aimed at
                    // the target the residents' room was handed — one of
                    // the positions above, so the crew member is the one
                    // stood there — and the joined room lands it on the
                    // body if the two are still within reach, since the
                    // lock was read a step ago and a body walks.
                    let who = crew
                        .iter()
                        .enumerate()
                        .filter_map(|(who, t)| t.map(|(p, _)| (who, (p - shot.at).len())))
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                        .map(|(who, _)| who);
                    if let Some(who) = who {
                        if who >= crew_count {
                            self.aboard.room.enemy_strike_sentry(
                                on_deck(shot.from),
                                who - crew_count,
                                shot.damage,
                            );
                        } else {
                            self.aboard.room.enemy_strike(
                                on_deck(shot.from),
                                who,
                                shot.damage,
                                shot.cut,
                            );
                        }
                    }
                    continue;
                }
                self.aboard.room.enemy_fire(
                    on_deck(shot.from),
                    on_deck(shot.at),
                    shot.weapon,
                    shot.moving,
                );
            }
        }
        // And the residents for the crew, the same way: alive and on their
        // feet — one out cold is nobody's target — each with its weapon, at
        // the peek while peeking, and which are peeking. **Every body of
        // that room**: its Bims and then its machines (feature 83), one
        // index space, which is what a hit past the Bims is read back
        // against above.
        let bodies = room.body_count();
        let alive: Vec<bool> = (0..bodies)
            .map(|who| room.is_alive(who as usize) && !room.is_unconscious(who as usize))
            .collect();
        let weapons: Vec<Weapon> = (0..bodies)
            .map(|who| {
                room.weapon(who as usize)
                    .unwrap_or(WeaponKind::LaserPistol.basic())
            })
            .collect();
        let peeking: Vec<bool> = (0..bodies)
            .map(|who| room.peek(who as usize).is_some())
            .collect();
        let dodge: Vec<f32> = (0..bodies).map(|who| room.dodge(who as usize)).collect();
        let exposed: Vec<DVec2> = (0..residents.aboard.count())
            .map(|who| residents.aboard.exposed(who))
            .collect();
        let targets = self
            .aboard
            .hostiles(&exposed, &alive)
            .into_iter()
            .zip(weapons)
            .map(|(p, weapon)| p.map(|p| (p, weapon)))
            .collect();
        self.aboard.room.set_hostiles(targets);
        self.aboard.room.set_hostiles_peeking(&peeking);
        self.aboard.room.set_hostiles_dodge(&dodge);
    }

    /// Who of the crew is locked in a melee — an enemy with a blade within
    /// reach, so they cannot fire and brawl instead (`bims::combat`) — said
    /// the step the lock forms, once, and again only after it has broken:
    /// a fight at arm's length is one long lock, not a hundred events.
    fn melee_locks(&mut self, events: &mut Vec<WorldEvent>) {
        for who in 0..self
            .crew_locked
            .len()
            .min(self.aboard.crew_count() as usize)
        {
            let locked = self.aboard.room.is_locked(who).is_some();
            if locked && !self.crew_locked[who] {
                events.push(WorldEvent::Locked { who: who as u32 });
            }
            self.crew_locked[who] = locked;
        }
    }

    /// What the enemy's fire did to the crew this step, said: every hit
    /// that landed on one — already on the body, since the joined room
    /// wounds its own the step a bolt lands — and whoever went down. Down
    /// is said once, the step it happens, whatever did it: a shot, blood
    /// lost to a wound nobody dressed, the room's own hunger.
    fn casualties(&mut self, events: &mut Vec<WorldEvent>) {
        for hit in self.aboard.room.take_wounds_taken() {
            if hit.who < self.aboard.crew_count() as usize {
                events.push(WorldEvent::CrewHit {
                    who: hit.who as u32,
                    part: hit.part.code(),
                });
            }
        }
        for (who, trauma) in self.aboard.room.take_traumas() {
            if who < self.aboard.crew_count() as usize {
                events.push(WorldEvent::CrewDying {
                    who: who as u32,
                    trauma: trauma.code(),
                });
            }
        }
        for (who, trauma) in self.aboard.room.take_treated() {
            if who < self.aboard.crew_count() as usize {
                events.push(WorldEvent::CrewTreated {
                    who: who as u32,
                    trauma: trauma.code(),
                });
            }
        }
        for who in 0..self.crew_down.len().min(self.aboard.crew_count() as usize) {
            let down = !self.aboard.room.is_alive(who);
            if down && !self.crew_down[who] {
                self.crew_down[who] = true;
                events.push(WorldEvent::CrewDown { who: who as u32 });
                // A dead crew member's level, experience and talents die with
                // it (feature 74), and a medic's beam and charge (feature 76).
                if let Some(progress) = self.progress.get_mut(who) {
                    *progress = Progress::default();
                }
                if let Some(medic) = self.medics.get_mut(who) {
                    *medic = Medic::default();
                }
                // And a tank's taunt with it (feature 77), and a
                // commander's rally (feature 78).
                if let Some(tank) = self.tanks.get_mut(who) {
                    *tank = Tank::default();
                }
                if let Some(commander) = self.commanders.get_mut(who) {
                    *commander = Commander::default();
                }
            }
        }
    }

    /// The hold's medicine, put on the room's shelves before it steps:
    /// the medkits to hand are the hold's `Medkit` count and the fibre on
    /// the cold store's shelf its `Fibre` count, since the hold keeps
    /// both and the room keeps neither aboard. The store's
    /// food is the room's own and is handed back unchanged. **The
    /// bandages are not here** (feature 87): a dressing is a thing in a
    /// pack, put there by [`World::restock_bandages`] and spent out of
    /// the pack by the room, so no count crosses either way.
    fn hand_the_room_the_hold_s_medicine(&mut self) {
        let design = &self.ship.design;
        let fibre = design.carrying(ResourceId::Fibre);
        let medkits = design.carrying(ResourceId::Medkit);
        // And where a kit is fetched from: the use spot of every container
        // that takes one — a locker-class cabinet, a shelf — so the walk
        // to the kit is a walk to a real cabinet. Only while there are any:
        // a stand with nothing on the shelf is nowhere to go.
        let stands: Vec<bims::math::Vec2> = if medkits > 0 {
            self.aboard
                .containers()
                .into_iter()
                .filter(|&c| self.container_takes(c, ResourceId::Medkit))
                .filter_map(|c| self.aboard.room.container_spot(c))
                .collect()
        } else {
            Vec::new()
        };
        // And what each of the crew carries in its own pack — a medic's
        // start kit, or one somebody fetched out of the hold: a helper
        // with a kit of its own opens that where it stands rather than
        // walking to a cabinet for one of these.
        let wanted = Item::Stack(ResourceId::Medkit as u32);
        let carried: Vec<u32> = (0..self.aboard.crew_count() as usize)
            .map(|who| {
                self.aboard
                    .room
                    .pack(who)
                    .iter()
                    .filter(|i| **i == Some(wanted))
                    .count() as u32
            })
            .collect();
        let room = &mut self.aboard.room;
        room.set_medkits(medkits);
        room.set_pack_kits(carried);
        room.set_kit_stands(&stands);
        let (veg, tofu, stew) = (room.store_veg(), room.store_tofu(), room.store_stew());
        room.set_stock(veg, tofu, stew, fibre);
    }

    /// What the room did with its medicine this step, moved through the
    /// hold: every bandage used comes off the count, and every sheaf of
    /// fibre the bay grew goes in, as much as the cold store has room for.
    /// Fibre the store cannot take is lost — dropped, and nothing said:
    /// the shelf is already full of food the crew would rather keep, and
    /// an event for every sheaf a full larder turned away would be noise
    /// every step of a good harvest. The count is set again next step
    /// from the hold, so the room's shelf agrees.
    fn take_the_room_s_medicine(&mut self) {
        if bank_medicine(&mut self.ship.design, &mut self.aboard.room) {
            self.on_ship_changed();
        }
    }

    /// Under way again: the ship's room is the ship's alone. The residents
    /// were in their own room throughout and go on in it, drawing their
    /// own doors again, until the ship is out of range.
    fn unjoin_rooms(&mut self) {
        if !self.aboard.is_joined() {
            return;
        }
        // A squad order marks residents of the station alongside, so it
        // goes with the deck (feature 78).
        self.clear_squad();
        // A room taken apart drops every errand, a load in somebody's arms
        // with it: whatever was on its way to a site is the hold's again.
        self.drop_loads();
        let seed = self.galaxy_seed ^ self.steps;
        // As at the join: the crew out first, then what that banked.
        let mut old = std::mem::replace(&mut self.aboard, Aboard::new(&self.ship.design, 1, 0));
        let crew = old.room.take_crew();
        let banked = bank_medicine(&mut self.ship.design, &mut old.room);
        self.aboard = old.unjoined(crew, &self.ship.design, seed, self.clock_minutes);
        // The ship alone is under its own roof: a fresh room has no
        // daylight, and this says so where a landing said the other.
        self.aboard.room.set_daylight(None);
        let station = self
            .residents
            .as_ref()
            .and_then(|r| self.station(r.station).cloned());
        let minutes = self.clock_minutes;
        if let Some(residents) = &mut self.residents {
            residents.aboard.room.set_doors_drawn(true);
            residents.aboard.room.set_fog(bims::sight::Fog::All);
            // And the ship off their deck: anybody still aboard it is put
            // at its bunk, since the ship has left without it — and the
            // settlement's guard back at its post.
            if let Some(station) = &station {
                residents.unjoin(station, minutes);
                residents.post_guard(station);
            }
        }
        self.restore_lamps();
        // A station's deck is left behind with its deployables (feature
        // 74); the ship's keep theirs, and both rooms are told the cover
        // again on the next step, since a fresh `Sight` has none.
        self.drop_station_deployables();
        self.sync_deployed_cover();
        if banked {
            self.on_ship_changed();
        }
    }

    /// The nearest station, and how far the ship is from its hull.
    fn nearest_station(&self) -> Option<(&Station, f64)> {
        let here = self.ship.position();
        self.stations
            .iter()
            .map(|s| (s, s.clearance(here)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// The station's room closed, and what it lost while it was open
    /// remembered: its own people dead and its mercenaries dead, added to
    /// [`World::losses`] for the station, so the room opens again with
    /// the survivors ([`World::people_of`], [`World::mercenaries_of`]).
    /// The one door a residents' room goes out by, bar a probe's.
    fn close_residents(&mut self) -> Option<Residents> {
        let residents = self.residents.take()?;
        let (mut own, mut hired) = (0u32, 0u32);
        // And where each of them is lying, for the room that opens next
        // (feature 85): the bodies laid out at this open among them, so
        // what the room says is the whole of what that station's deck
        // holds.
        let mut graves = Vec::new();
        // The Bims alone: a machine destroyed is no loss of the
        // station's people, and the waves are counted by
        // `World::infested` rather than by `losses` (feature 83).
        let people = residents.aboard.room.crew_count() as usize;
        for who in 0..people {
            if residents.aboard.room.is_alive(who) {
                continue;
            }
            // Not `is_alive` is dead, not out cold — one out cold wakes,
            // and is one of the survivors rather than a grave.
            let was_hired = matches!(residents.fee.get(who), Some(Some(_)));
            // A body that was already lying here when the room opened is
            // in `losses` from the day it died.
            if !residents.grave.get(who).copied().unwrap_or(false) {
                if was_hired {
                    hired += 1;
                } else {
                    own += 1;
                }
            }
            let at = residents.aboard.position(who as u32);
            graves.push(Grave {
                station: residents.station,
                x: at.x,
                y: at.y,
                gear: residents.aboard.room.gear(who),
                look: residents.aboard.room.look(who),
                hired: was_hired,
            });
        }
        // A raider's are the raid's, gone with the raider — its dead
        // included: there is no deck to come back to.
        if raid::raider_index(residents.station).is_none() {
            memory::amend_losses(&mut self.losses, residents.station, |l| {
                l.dead += own;
                l.mercenaries += hired;
            });
            memory::set_graves(&mut self.graves, residents.station, graves);
        }
        Some(residents)
    }

    /// What the station at `id` has lost to the crew.
    pub fn losses_at(&self, id: u32) -> Losses {
        memory::losses_at(&self.losses, id)
    }

    /// Every body lying on that station's deck (feature 85).
    pub fn graves_at(&self, id: u32) -> &[Grave] {
        memory::graves_at(&self.graves, id)
    }

    /// The station's room opened with what the station has: its people,
    /// its hired hands and its dead. The one door, so that no open
    /// anywhere forgets the graves.
    fn open_residents(
        &self,
        station: u32,
        design: &ShipDesign,
        count: u32,
        mercenaries: u32,
        seed: u64,
    ) -> Residents {
        Residents::open(
            station,
            design,
            count,
            mercenaries,
            seed,
            self.clock_minutes,
            self.graves_at(station),
        )
    }

    /// This system as the crew leave it, filed by its star on
    /// [`World::memories`] — everything of the world that is the system's
    /// rather than the ship's — for [`World::recall_system`] to put back.
    /// Called by a jump out; the fields themselves are left as they are
    /// for the jump to replace.
    fn remember_system(&mut self) {
        let memory = SystemMemory {
            star: self.star_id,
            hostile: self.hostile.clone(),
            reinforcements: self.reinforcements,
            station_keys: self.station_keys.clone(),
            sites: self.sites.clone(),
            plunder: self.plunder.clone(),
            lamps: self
                .lamps
                .iter()
                .filter(|d| d.station.is_some())
                .copied()
                .collect(),
            discovered: self.discovered.clone(),
            losses: self.losses.clone(),
            graves: self.graves.clone(),
            visited: self.visited.clone(),
        };
        memory::file_memory(&mut self.memories, memory);
    }

    /// The system at `star` as the crew left it, put back — `false`, and
    /// nothing touched, if they have never been. The ship's own lamps
    /// stay; the station's remembered ones join them.
    fn recall_system(&mut self, star: u32) -> bool {
        let Some(memory) = memory::memory_of(&self.memories, star).cloned() else {
            return false;
        };
        self.hostile = memory.hostile;
        self.reinforcements = memory.reinforcements;
        self.station_keys = memory.station_keys;
        self.sites = memory.sites;
        self.plunder = memory.plunder;
        self.lamps.retain(|d| d.station.is_none());
        self.lamps.extend(memory.lamps);
        self.discovered = memory.discovered;
        self.losses = memory.losses;
        self.graves = memory.graves;
        self.visited = memory.visited;
        true
    }

    /// Open the station's room when the ship comes within range of its hull,
    /// and close it when the ship leaves. Every station gets one — a
    /// derelict's has nobody in it, and is opened all the same because the
    /// room is what has the pictures of the fixtures. The way out is
    /// further than the way in, the same hysteresis as the local frame and
    /// for the same reason: a ship holding on the line must not open and
    /// close a whole room every step.
    fn settle_residents(&mut self) {
        // Docked, the station is in the ship's room and its own room is the
        // one `join_rooms` left for its pictures. Nothing to settle — and
        // not while the ship is casting off either, since the rooms are
        // still one until it does.
        if matches!(
            self.ship.state,
            ShipState::Docked { .. } | ShipState::CastingOff { .. }
        ) {
            return;
        }
        let near = self.nearest_station().map(|(s, clearance)| {
            (
                s.id,
                clearance,
                (self.people_of(s), self.mercenaries_of(s)),
                s.map_seed,
            )
        });
        if let Some(residents) = &self.residents {
            let same = near.filter(|&(id, _, _, _)| id == residents.station);
            let keep = same.is_some_and(|(_, clearance, _, _)| {
                clearance <= data::RESIDENTS_RANGE * data::LOCAL_HYSTERESIS
            });
            if keep {
                return;
            }
            self.close_residents();
        }
        if let Some((id, clearance, (count, mercs), seed)) = near
            && clearance <= data::RESIDENTS_RANGE
            && let Some(station) = self.station(id)
        {
            self.residents = Some(self.open_residents(id, &station.design, count, mercs, seed));
            // Whose it is: its fog is black for a stranger's, and its
            // people are ringed for an enemy's.
            self.apply_stances();
        }
    }

    /// What a trip would cost, without committing to it.
    ///
    /// Worked out by the page for the local player only, and never a command:
    /// two players hovering over different planets must not be an argument
    /// about where the ship is going.
    pub fn preview(&self, target: Target) -> Result<Preview, PlanError> {
        if let Some(node) = target.node()
            && !self.discovered.contains(&node)
        {
            return Err(PlanError::TargetUndiscovered);
        }

        // Under way, the honest quote includes stopping first — which is what
        // a redirect actually does, and it is often most of the bill.
        let (stopping, from, heading) = match &self.ship.state {
            ShipState::Travelling { plan, departed } => {
                let stop = flight::abort(plan, self.clock_minutes - departed);
                let end = flight::state_at(&stop, stop.duration());
                (stop.duration(), end.position, end.heading)
            }
            _ => (0.0, self.ship.position(), self.ship.heading),
        };

        let at = self
            .target_position(target)
            .ok_or(PlanError::TargetUndiscovered)?;
        let plan = flight::plan_trip(&self.ship.dynamics, from, heading, target, at)?;
        Ok(Preview {
            minutes: stopping + plan.duration(),
            stopping,
            power: self.ship.dynamics.forward_power,
            throttle: self.ship.dynamics.forward_throttle,
            docks: plan.docks,
        })
    }

    // --- the ship changing --------------------------------------------------

    /// Everything derived from the design, redone.
    ///
    /// **The one door.** Trading goes through it, a plan ending goes through
    /// it, and construction will go through it. What it promises is that the
    /// anchor and the heading do not move: the hull stays exactly where it was
    /// in the system and on the screen, and it is the *centre of mass* — the
    /// ship's position — that shifts when weight is added to one end.
    ///
    /// A trip already under way keeps the dynamics it was planned with, so
    /// this does not alter an arrival that has been promised. It is the next
    /// plan that flies differently.
    pub fn on_ship_changed(&mut self) {
        if let Ok(dynamics) = flight::dynamics(&self.ship.design, self.ship.crew_count) {
            self.ship.dynamics = dynamics;
        }
        // A battery taken off takes what was in it; one put on arrives
        // empty. Either way the charge cannot exceed what is there to hold
        // it.
        self.power_budget = shipdesign::power_budget(&self.ship.design);
        if let Some(supply) = self.probe_supply {
            self.power_budget.supply = supply;
        }
        self.powered_parts = shipdesign::powered_parts(&self.ship.design);
        let storage = self.power_budget.storage;
        if self.ship.charge > storage {
            self.ship.charge = storage;
        }
        // And the pieces of armour against the hold's count of them —
        // here because this is the one door every cargo change goes
        // through, so the invariant holds by construction rather than by
        // every caller remembering.
        self.settle_pieces();
        self.settle_guns();
        self.settle_grids();
    }

    /// Stage 6 of [`World::step`]: the reactors' output less the wired
    /// consumers' draw and what the engines are drawing this step, over one
    /// step, into the batteries and clamped to what they hold. Closed form
    /// off the step length, so a browser at 24x and a server catching up
    /// land on the same charge.
    ///
    /// The draw is charged in full whether or not the ship is browned
    /// out: what stops in a brownout is the consumers, and what they would
    /// have drawn was never there to take. The clamp at nought *is* the
    /// brownout. The engines' draw is the plan's — throttled to the spare
    /// before the trip was planned — so a burn never on its own takes the
    /// charge down; what it does is leave nothing over to charge with.
    fn run_power(&mut self) {
        let budget = self.power_budget;
        let net = (budget.supply - budget.draw - self.engine_draw_now()) * data::STEP_MINUTES;
        self.ship.charge = (self.ship.charge + net).clamp(0.0, budget.storage);
    }

    /// The other half of stage 6: what the brownout does, none of it
    /// lethal and all of it recoverable. Said once each way — `Brownout`
    /// the step the batteries go flat under an overdraw, `PowerRestored`
    /// the step the reactors cover the draw again or a battery has
    /// something in it — and put to the room: the ship's lamps dark
    /// ([`World::sync_lamp_power`]) and its bay asleep
    /// ([`World::sync_bay_power`]) while it lasts. Then the cold store:
    /// every step it is not running — browned out, or on no live network
    /// — the food in it is a step nearer spoiling, and at the hour a
    /// share of it goes ([`World::spoil`]). A cold store with power puts
    /// the clock back to nought, so a short overdraw a battery covers
    /// costs nothing and what is left when the power comes back stays.
    fn run_brownout(&mut self, events: &mut Vec<WorldEvent>) {
        let now = self.power().brownout();
        if now != self.browned_out {
            self.browned_out = now;
            events.push(if now {
                WorldEvent::Brownout
            } else {
                WorldEvent::PowerRestored
            });
            self.sync_lamp_power();
        }
        self.sync_bay_power();
        if self.powered(PartKind::ColdStore) {
            self.cold_store_out = 0;
        } else {
            self.cold_store_out += 1;
            if self.cold_store_out >= data::SPOIL_STEPS {
                self.cold_store_out = 0;
                self.spoil(events);
            }
        }
    }

    /// An hour of the cold store without power: one part in
    /// [`data::SPOIL_DIVISOR`] of the vegetables, the tofu and the stew,
    /// rounded up, off the room's shelf — what the crew cook from — and
    /// off the hold's count of the two the hold keeps, which is what the
    /// cold store's grid lays out, what the desk sells and what a room
    /// built afresh at the next dock stocks the shelf from. `FoodSpoiled`
    /// says what the shelf lost.
    fn spoil(&mut self, events: &mut Vec<WorldEvent>) {
        let lost = |n: u32| n.div_ceil(data::SPOIL_DIVISOR);
        let room = &mut self.aboard.room;
        let (veg, tofu, stew, fibre) = (
            room.store_veg(),
            room.store_tofu(),
            room.store_stew(),
            room.store_fibre(),
        );
        let (gone_veg, gone_tofu, gone_stew) = (lost(veg), lost(tofu), lost(stew));
        room.set_stock(veg - gone_veg, tofu - gone_tofu, stew - gone_stew, fibre);
        let mut hold_moved = false;
        for id in [ResourceId::Vegetable, ResourceId::Tofu] {
            let count = &mut self.ship.design.cargo[id as usize];
            let gone = lost(*count);
            if gone > 0 {
                *count -= gone;
                hold_moved = true;
            }
        }
        if hold_moved {
            self.on_ship_changed();
        }
        let units = gone_veg + gone_tofu + gone_stew;
        if units > 0 {
            events.push(WorldEvent::FoodSpoiled { units });
        }
    }

    /// The ship's lamps lit or dark by their power: a lamp on a live
    /// network has it unless the ship is browned out, one on none never
    /// does — set on the crew's deck, where the ship's lamps are the
    /// room's own, and on the residents' mirror of the ship while it is
    /// on their deck. A station's lamps are furniture — nothing reads a
    /// station's power — and stay lit. Every step, from
    /// [`World::restore_lamps`], since a room built afresh starts every
    /// lamp lit; and the step the brownout turns, so the dark lands with
    /// the event.
    fn sync_lamp_power(&mut self) {
        let dark = self.browned_out;
        let lamps: Vec<((u32, u32), bool)> = self
            .ship
            .design
            .parts
            .iter()
            .filter(|p| shipdesign::is_light(p.kind))
            .map(|p| {
                (
                    p.origin,
                    !dark && self.powered_parts.binary_search(&p.id).is_ok(),
                )
            })
            .collect();
        for (tile, on) in lamps {
            if let Some(i) = Self::lamp_index(&self.aboard, false, tile) {
                self.aboard.room.set_lamp_powered(i, on);
            }
            if let Some(residents) = &mut self.residents
                && let Some(i) = Self::lamp_index(&residents.aboard, true, tile)
            {
                residents.aboard.room.set_lamp_powered(i, on);
            }
        }
    }

    /// The ship's bay running or not: a bay aboard on a live network,
    /// and no brownout. A ship with no bay is left alone — the room
    /// stands in one of its own, and that is the room's business.
    fn sync_bay_power(&mut self) {
        let on =
            self.ship.design.count(PartKind::HydroBay) == 0 || self.powered(PartKind::HydroBay);
        self.aboard.room.set_hydro_powered(on);
    }

    /// What the engines are drawing at this moment: the effort's power,
    /// under way; nothing otherwise.
    fn engine_draw_now(&self) -> f64 {
        match &self.ship.state {
            ShipState::Travelling { plan, departed } => {
                flight::effort_at(plan, self.clock_minutes - departed).power
            }
            _ => 0.0,
        }
    }

    // --- making things -------------------------------------------------------

    /// Set what the crew are to keep made of `resource`. Clamped to what its
    /// class could hold of it with nothing else there, since a target past
    /// that is one the benches would never reach.
    pub fn set_craft_target(&mut self, resource: ResourceId, units: u32) {
        let most = self.ship.design.most_of(resource);
        self.craft_targets[resource as usize] = units.min(most);
    }

    pub fn craft_target(&self, resource: ResourceId) -> u32 {
        self.craft_targets[resource as usize]
    }

    /// Whether one of `recipe` could be made out of the hold right now:
    /// every input aboard and not spoken for by a construction site, and
    /// room in the output's class for the output once the inputs are out
    /// of it.
    fn can_make(&self, recipe: &shipdesign::Recipe) -> bool {
        let design = &self.ship.design;
        let inputs_aboard = recipe
            .inputs
            .iter()
            .all(|&(id, units)| self.free(id) >= units);
        let (output, units) = recipe.output;
        // Room for the output as the hold stands: what the inputs free is
        // not counted — the smelter's ore and its metal share the shelves,
        // but a stack of ore going does not make a place for the metal
        // until it has gone, and the order is placed before it does. So
        // a bench with a full shelf waits a step for the ore to be spent,
        // and is not lost.
        let _ = design;
        inputs_aboard && self.has_room(output, units)
    }

    /// Every recipe the benches are wanted for this step, one order per
    /// bench of its station: the hold short of the target, one makeable,
    /// the station wired and running. In recipe order, which is what the
    /// room picks from.
    pub fn craft_orders(&self) -> Vec<bims::game::Order> {
        let mut orders = Vec::new();
        for (i, recipe) in shipdesign::RECIPES.iter().enumerate() {
            let (output, units) = recipe.output;
            // What is aboard plus what is on the bench: a chain already
            // making one counts, or the target would be overshot by one
            // for every step the first one took.
            let coming = self.aboard.room.crafts_under_way(i as u32) * units;
            if self.ship.design.carrying(output) + coming >= self.craft_targets[output as usize] {
                continue;
            }
            //    A recipe the crew have not researched is not offered, however
            //    the bench came aboard — see `shipdesign::research`.
            if !self.research.recipe_allowed(i) {
                continue;
            }
            if !self.can_make(recipe) || !self.powered(recipe.station) {
                continue;
            }
            for (bench, b) in self.aboard.room.benches().iter().enumerate() {
                if b.kind == recipe.station.code() {
                    orders.push(bims::game::Order {
                        recipe: i as u32,
                        bench,
                        minutes: recipe.minutes as f32,
                        only: None,
                    });
                }
            }
        }
        // And a session of the upgrade, at the first workbench only — one
        // pair of hands a day, however many benches — while one is under
        // way and unfinished, the bench is powered and nobody is at it.
        if let Some(upgrade) = self.bench.work
            && !upgrade.complete()
            && self.powered(PartKind::Workbench)
            && self.aboard.room.crafts_under_way(UPGRADE_ORDER) == 0
            && let Some(bench) = self
                .aboard
                .room
                .benches()
                .iter()
                .position(|b| b.kind == PartKind::Workbench.code())
        {
            orders.push(bims::game::Order {
                recipe: UPGRADE_ORDER,
                bench,
                minutes: data::UPGRADE_SESSION_MINUTES as f32,
                only: None,
            });
        }
        // And the armourer's repair session (feature 74), at the first
        // workbench, for the one engineer who began it and nobody else,
        // while the bench is powered and nobody is at it.
        if let Some(who) = self.bench.repair
            && self.powered(PartKind::Workbench)
            && self.aboard.room.crafts_under_way(deploy::REPAIR_ORDER) == 0
            && let Some(bench) = self
                .aboard
                .room
                .benches()
                .iter()
                .position(|b| b.kind == PartKind::Workbench.code())
        {
            orders.push(bims::game::Order {
                recipe: deploy::REPAIR_ORDER,
                bench,
                minutes: class::ARMOUR_REPAIR_MINUTES as f32,
                only: Some(who as usize),
            });
        }
        orders
    }

    /// A Bim finished `recipe`: the inputs out of the hold and the output
    /// in, if the hold still allows it — the ore may have been sold while
    /// the Bim stood at the smelter — and an event either way. The mass
    /// moves with the cargo; `on_ship_changed` is what notices.
    fn finish_craft(&mut self, recipe: u32, events: &mut Vec<WorldEvent>) {
        if recipe == UPGRADE_ORDER {
            self.finish_upgrade_session();
            return;
        }
        if recipe == deploy::REPAIR_ORDER {
            self.finish_repair(events);
            return;
        }
        let Some(r) = shipdesign::RECIPES.get(recipe as usize) else {
            return;
        };
        if !self.can_make(r) {
            events.push(WorldEvent::CraftLost { recipe });
            return;
        }
        for &(id, units) in r.inputs {
            self.ship.design.cargo[id as usize] -= units;
        }
        self.ship.design.cargo[r.output.0 as usize] += r.output.1;
        self.on_ship_changed();
        events.push(WorldEvent::Crafted { recipe });
    }

    // --- building -------------------------------------------------------------

    /// Whether the ship is standing still: docked, or holding station.
    /// The only time a site may be laid out or worked — see
    /// [`crate::build`].
    pub fn at_rest(&self) -> bool {
        matches!(
            self.ship.state,
            ShipState::Docked { .. } | ShipState::Holding
        )
    }

    /// Whether anything is being built: a site with something carried to
    /// it, or a Bim on the way to one. What refuses a Confirm — the ship
    /// does not move while it is built on.
    pub fn under_construction(&self) -> bool {
        self.builds.iter().any(|s| s.begun()) || self.aboard.room.building_under_way()
    }

    /// Units of `resource` the sites have spoken for, delivered or in
    /// somebody's arms.
    pub fn reserved(&self, resource: ResourceId) -> u32 {
        self.builds
            .iter()
            .fold(0u32, |sum, s| sum.saturating_add(s.reserved(resource)))
    }

    /// Units of `resource` aboard that nothing has claimed: what may be
    /// sold, smelted, or carried to another site. Every hand that reaches
    /// for the hold asks this rather than the raw count.
    pub fn free(&self, resource: ResourceId) -> u32 {
        self.ship
            .design
            .carrying(resource)
            .saturating_sub(self.reserved(resource))
    }

    pub fn site(&self, id: u32) -> Option<&BuildSite> {
        self.builds.iter().find(|s| s.id == id)
    }

    /// The design with every pending site already on it, in order, each
    /// that will go. What a new site is checked against, so a wall laid
    /// out on deck that is itself laid out goes: the deck will be there by
    /// the time the wall is built, since the crew take the sites in order.
    fn design_with_sites(&self) -> ShipDesign {
        let free = shipdesign::Budget::new(Money::MAX);
        let mut design = self.ship.design.clone();
        for site in &self.builds {
            if let Ok(next) = shipdesign::apply(&design, &free, site.edit()) {
                design = next;
            }
        }
        design
    }

    /// Whether a site for `kind` at `origin` turned `rotation` may be laid
    /// out: the ship at rest, the part going where the rules say it may on
    /// the ship as it will be once the pending sites are built, and the
    /// ship as it would then be raising no error the ship does not raise
    /// already — a wall across the spot the hob is worked from is a crew
    /// that starve in front of it, and the design phase would have refused
    /// it too. `Err` is why: a refusal, or the code of the first new fault
    /// as [`crate::event::Refusal::WontFit`] with `issue` set.
    pub fn can_place_site(
        &self,
        kind: PartKind,
        origin: (u32, u32),
        rotation: Rotation,
    ) -> Result<(), SiteRefusal> {
        if !self.at_rest() {
            return Err(SiteRefusal::UnderWay);
        }
        if !self.research.part_allowed(kind) {
            return Err(SiteRefusal::NotResearched(
                shipdesign::research::node_of_part(kind).code(),
            ));
        }
        let site = BuildSite::new(0, kind, origin, rotation);
        let before = self.design_with_sites();
        let after =
            match shipdesign::apply(&before, &shipdesign::Budget::new(Money::MAX), site.edit()) {
                Ok(after) => after,
                Err(why) => return Err(SiteRefusal::WontFit(why.code())),
            };
        let crew = self.ship.crew_count;
        let errors = |design: &ShipDesign| -> Vec<u32> {
            shipdesign::validate(design, crew)
                .into_iter()
                .filter(|i| i.severity == shipdesign::Severity::Error)
                .map(|i| i.code)
                .collect()
        };
        let already = errors(&before);
        if let Some(&code) = errors(&after).iter().find(|c| !already.contains(c)) {
            return Err(SiteRefusal::Fault(code));
        }
        Ok(())
    }

    fn place_site(
        &mut self,
        slot: u32,
        kind: PartKind,
        origin: (u32, u32),
        rotation: Rotation,
        events: &mut Vec<WorldEvent>,
    ) {
        match self.can_place_site(kind, origin, rotation) {
            Ok(()) => {}
            Err(SiteRefusal::UnderWay) => {
                events.push(refused(slot, Refusal::UnderWay));
                return;
            }
            Err(SiteRefusal::NotResearched(_)) => {
                events.push(refused(slot, Refusal::NotResearched));
                return;
            }
            Err(SiteRefusal::WontFit(_) | SiteRefusal::Fault(_)) => {
                events.push(refused(slot, Refusal::WontFit));
                return;
            }
        }
        let id = self.next_site;
        self.next_site += 1;
        self.builds.push(BuildSite::new(id, kind, origin, rotation));
        events.push(WorldEvent::SitePlaced { site: id, kind });
    }

    fn cancel_site(&mut self, slot: u32, site: u32, events: &mut Vec<WorldEvent>) {
        let Some(at) = self.builds.iter().position(|s| s.id == site) else {
            events.push(refused(slot, Refusal::NoSuchSite));
            return;
        };
        let gone = self.builds.remove(at);
        events.push(WorldEvent::SiteCancelled { kind: gone.kind });
    }

    /// Every site, one order each, this step: what it still wants carried
    /// — the first material short of its recipe that the hold has any free
    /// of, a load of it — and, with everything there, that it is to be
    /// built and how long that takes. A site wanting neither is still on
    /// the list — a load may be on its way to it, and the walk has to find
    /// it — with nothing to start at it: short of something the hold has
    /// none of, or a wall on deck that is itself still a site, waiting for
    /// the deck. Nothing while the ship is not at rest. In the order the
    /// sites were laid out, which is what the room picks from.
    pub fn build_orders(&self) -> Vec<bims::game::Build> {
        if !self.at_rest() {
            return Vec::new();
        }
        let design = &self.ship.design;
        let free = shipdesign::Budget::new(Money::MAX);
        let t = shipdesign::TILE as f32;
        let offset = self.aboard.offset;
        self.builds
            .iter()
            .map(|site| {
                let recipe = site.recipe(design);
                let tiles = site
                    .tiles()
                    .into_iter()
                    .map(|(x, y)| {
                        bims::math::Rect::from_min_size(
                            bims::math::vec2(
                                x as f32 * t + offset.x as f32,
                                y as f32 * t + offset.y as f32,
                            ),
                            bims::math::vec2(t, t),
                        )
                    })
                    .collect();
                let haul = recipe.iter().find_map(|&(id, _)| {
                    let short = site.short(design, id);
                    let load = short.min(data::HAUL_LOAD).min(self.free(id));
                    (load > 0).then_some((id as u32, load))
                });
                // Only a part that would go down now is worth putting
                // together: one on a site that is itself waiting is not,
                // and nor is one `can_modify_part` says to leave.
                let buildable = site.stocked(design)
                    && self.can_modify_part(site.kind)
                    && shipdesign::apply(design, &free, site.edit()).is_ok();
                let minutes = if buildable {
                    build::build_minutes(&recipe) as f32
                } else {
                    0.0
                };
                bims::game::Build {
                    site: site.id,
                    tiles,
                    haul,
                    minutes,
                }
            })
            .collect()
    }

    /// Who may put a suit on and go out to a site beyond the hull, by
    /// slot: alive, under the dose limit — the same line a walk to mine
    /// draws — and a suit aboard to wear. The airlock itself is the room's
    /// to find.
    fn suit_ok(&self) -> Vec<bool> {
        let suit = self.ship.design.carrying(ResourceId::Suit) > 0;
        self.health
            .iter()
            .map(|h| suit && !h.dead && h.dose < data::EVA_DOSE_LIMIT)
            .collect()
    }

    /// A Bim took a load off a shelf for `site`: as much of `resource` as
    /// the site is short, the **hauler's own load** ([`World::haul_load`],
    /// twice the ordinary with *pack mule*) and what the hold has free
    /// allow is now in its arms — spoken for, not moved. Worked out again
    /// here rather than read off the order, since the order is the site's
    /// and the load is the crew member's. A site that has gone gets
    /// nothing, and the Bim carries a crate of nothing to nowhere, which
    /// the room sorts out at its next walk.
    fn finish_pick(&mut self, site: u32, resource: u32, who: usize) {
        let Some(resource) = ResourceId::ALL.get(resource as usize).copied() else {
            return;
        };
        let short = self
            .builds
            .iter()
            .find(|s| s.id == site)
            .map_or(0, |s| s.short(&self.ship.design, resource));
        let got = short
            .min(self.haul_load(who as u32))
            .min(self.free(resource));
        if let Some(site) = self.builds.iter_mut().find(|s| s.id == site) {
            site.carrying[resource as usize] += got;
        }
    }

    /// A load arrived: everything in transit to the site is delivered.
    fn finish_drop(&mut self, site: u32) {
        if let Some(site) = self.builds.iter_mut().find(|s| s.id == site) {
            for (delivered, carrying) in site.delivered.iter_mut().zip(site.carrying.iter_mut()) {
                *delivered += core::mem::take(carrying);
            }
        }
    }

    /// A load was given up short of the site: the hold's again.
    fn finish_return(&mut self, site: u32) {
        if let Some(site) = self.builds.iter_mut().find(|s| s.id == site) {
            site.carrying = [0; CARGO_SLOTS];
        }
    }

    /// Every load in somebody's arms, the hold's again: for a room being
    /// taken apart, whose crew drop everything they were carrying without
    /// the room saying so.
    fn drop_loads(&mut self) {
        for site in &mut self.builds {
            site.carrying = [0; CARGO_SLOTS];
        }
        // And a thing on its way to or from the workbench goes back into
        // the hold: the chain carrying it is gone with the room.
        if let Some(item) = self.bench.carrying.take() {
            self.hold_takes(item);
        }
    }

    /// A Bim put a site together: the part goes down and its recipe comes
    /// out of the hold in one go, if the rules still allow it — the deck
    /// under it may have been laid out and cancelled since, the metal sold
    /// — and an event either way. The site is finished with whatever
    /// happened: a part that will not go is a site to lay out again, not
    /// one to stand at for ever. The room is laid out again under the
    /// crew with the part in it.
    pub(crate) fn finish_build(&mut self, site: u32, who: usize, events: &mut Vec<WorldEvent>) {
        let Some(at) = self.builds.iter().position(|s| s.id == site) else {
            return;
        };
        let site = self.builds.remove(at);
        if !self.can_modify_part(site.kind) {
            events.push(WorldEvent::BuildLost { kind: site.kind });
            return;
        }
        // The reservation is this site's own, and it is gone with the site
        // — so the recipe is checked against what is free of *every other*
        // site's claim, which is what `free` says now that it is out of
        // the list.
        let recipe = site.recipe(&self.ship.design);
        if recipe.iter().any(|&(id, units)| self.free(id) < units) {
            events.push(WorldEvent::BuildLost { kind: site.kind });
            return;
        }
        match shipdesign::build_from_cargo(&self.ship.design, site.edit()) {
            Ok(next) => {
                self.ship.design = next;
                self.on_ship_changed();
                self.relayout_room();
                events.push(WorldEvent::Built { kind: site.kind });
                // And every engineer within the builder's vicinity learnt
                // something from it (feature 74).
                let at = self.aboard.room.bim_pos(who);
                self.award_engineers_near(at, class::XP_BUILT, events);
            }
            Err(_) => events.push(WorldEvent::BuildLost { kind: site.kind }),
        }
    }

    /// The room aboard laid out again on the ship as it now is — with the
    /// station's deck joined to it, if it is docked — keeping the crew,
    /// their errands and everything else that is state. See
    /// `Aboard::relayout`. The residents' room is the station's and is
    /// not touched.
    fn relayout_room(&mut self) {
        let joined = self.ship.state.alongside().and_then(|id| {
            let berth = self.berth_at(id)?;
            let station = self.station(id)?;
            crate::docking::join(
                &self.ship.design,
                self.ship.dynamics.centre_of_mass,
                station,
                &berth,
            )
        });
        match joined {
            Some(joined) if self.aboard.is_joined() => self.aboard.relayout(joined.design),
            _ => {
                let design = self.ship.design.clone();
                self.aboard.relayout(design);
            }
        }
        self.restore_lamps();
        // A relayout is a fresh `Sight`: the laid sandbags again (feature 74).
        self.sync_deployed_cover();
    }

    // --- a walk outside ------------------------------------------------------

    /// The belt whose frame the ship is in, if it is in one, whatever the
    /// ship is doing there.
    fn belt_here(&self) -> Option<&worldgen::Body> {
        let Frame::Local(Node::Body(id)) = self.ship.frame else {
            return None;
        };
        self.system
            .body(id)
            .filter(|b| b.kind == worldgen::BodyKind::AsteroidBelt)
    }

    /// The belt the ship is holding at, if it is: at rest in the local
    /// frame of a body of that kind.
    fn belt_alongside(&self) -> Option<&worldgen::Body> {
        if self.ship.state != ShipState::Holding {
            return None;
        }
        self.belt_here()
    }

    /// The hull's tiles, as the box they span — `(x0, y0, x1, y1)`,
    /// inclusive — which is what a mining site is laid out clear of.
    fn hull_box(&self) -> (i32, i32, i32, i32) {
        let mut span: Option<(i32, i32, i32, i32)> = None;
        for part in &self.ship.design.parts {
            for (x, y) in part.tiles() {
                let (x, y) = (x as i32, y as i32);
                span = Some(match span {
                    None => (x, y, x, y),
                    Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                });
            }
        }
        span.unwrap_or((0, 0, 0, 0))
    }

    /// Stage 4's last word: the ship has come to rest at a belt it has not
    /// held at before, so the asteroids about it are laid out now, in the
    /// frame it is holding in. Once per belt — coming back finds the site
    /// as it was left, mined tiles and all.
    fn settle_site(&mut self) {
        let Some(belt) = self.belt_alongside().map(|b| b.id) else {
            return;
        };
        if self.sites.iter().any(|s| s.belt == belt) {
            return;
        }
        let site = MiningSite::generate(self.galaxy_seed, self.star_id, belt, self.hull_box());
        let at = self.sites.partition_point(|s| s.belt < belt);
        self.sites.insert(at, site);
        self.site_version += 1;
    }

    /// The mining site the ship is holding at, if it is at one.
    pub fn site_here(&self) -> Option<&MiningSite> {
        let belt = self.belt_alongside()?.id;
        self.sites.iter().find(|s| s.belt == belt)
    }

    fn site_here_mut(&mut self) -> Option<&mut MiningSite> {
        let belt = self.belt_alongside()?.id;
        self.sites.iter_mut().find(|s| s.belt == belt)
    }

    /// Mark a rock at the site to be mined, or take the mark off again.
    /// Nothing happens away from a site or off a rock.
    fn mark_rock(&mut self, x: i32, y: i32) {
        if let Some(site) = self.site_here_mut()
            && site.toggle_mark(x, y)
        {
            self.site_version += 1;
        }
    }

    fn clear_marks(&mut self) {
        if let Some(site) = self.site_here_mut() {
            site.clear_marks();
            self.site_version += 1;
        }
    }

    /// The ship is leaving the site: the marks come off, since an order to
    /// mine a rock is an order about a place the ship is at, whoever is out
    /// there is brought back in through the door, and what that walk had
    /// mined so far is said now, since it will not come in on its own.
    fn leave_site(&mut self, events: &mut Vec<WorldEvent>) {
        // By the frame, not by the state: the ship is under way by the time
        // this is asked.
        if let Some(belt) = self.belt_here().map(|b| b.id)
            && let Some(site) = self.sites.iter_mut().find(|s| s.belt == belt)
        {
            site.clear_marks();
            self.site_version += 1;
        }
        self.aboard.room.recall_outside();
        if self.walk_tally != (0, 0, 0) {
            self.finish_walk(events);
        }
    }

    /// Whether the crew may walk outside this step, and to what: holding at
    /// a belt with rocks marked, a port to go out by, a suit in the locker,
    /// room on the shelf for what comes back, and each Bim's dose under the
    /// limit. The room is handed every rock as something to walk round and
    /// the marked ones as where to go, in the ship's own units.
    pub fn eva_offer(&self) -> Option<bims::game::Eva> {
        let site = self.site_here()?;
        shipdesign::dock::port(&self.ship.design)?;
        if self.ship.design.carrying(ResourceId::Suit) == 0 {
            return None;
        }
        let design = &self.ship.design;
        if design.stored(Storage::Shelf) >= design.capacity(Storage::Shelf) {
            return None;
        }
        let t = shipdesign::TILE as f32;
        let middle = |(x, y): (i32, i32)| {
            let (mx, my) = mining::tile_middle(x, y);
            bims::math::vec2(mx as f32, my as f32)
        };
        Some(bims::game::Eva {
            allowed: self
                .health
                .iter()
                .map(|h| !h.dead && h.dose < data::EVA_DOSE_LIMIT)
                .collect(),
            targets: site.targets().map(middle).collect(),
            rocks: site
                .tiles
                .iter()
                .map(|r| {
                    let m = middle((r.x, r.y));
                    bims::math::Rect::from_min_size(
                        bims::math::vec2(m.x - t / 2.0, m.y - t / 2.0),
                        bims::math::vec2(t, t),
                    )
                })
                .collect(),
            version: self.site_version,
            tile_minutes: data::MINE_TILE_MINUTES as f32,
        })
    }

    /// A Bim mined the rock whose middle is `at`, in the ship's units: the
    /// tile comes out of the site and what it yields goes on the shelf, as
    /// much of it as fits, counted towards what the walk brings back. A
    /// tile that is not there — mined by the other one, or the ship has
    /// left — yields nothing.
    fn finish_tile(&mut self, at: (f32, f32)) {
        let t = shipdesign::TILE as f32;
        let (x, y) = ((at.0 / t).floor() as i32, (at.1 / t).floor() as i32);
        let Some(kind) = self.site_here_mut().and_then(|s| s.mine(x, y)) else {
            return;
        };
        self.site_version += 1;
        let (resource, units) = mining::yield_of(kind);
        // What the shelves have room for: a stack topped up, or a new one
        // where it fits.
        let got = self.room_for(resource, units);
        let design = &mut self.ship.design;
        design.cargo[resource as usize] += got;
        match kind {
            mining::Rock::Stone => self.walk_tally.0 += got,
            mining::Rock::Iron => self.walk_tally.1 += got,
            mining::Rock::Galvum => self.walk_tally.2 += got,
        }
        self.on_ship_changed();
    }

    /// A walk came back: say what it brought, all at once.
    fn finish_walk(&mut self, events: &mut Vec<WorldEvent>) {
        let (rock, ore, galvum) = core::mem::take(&mut self.walk_tally);
        events.push(WorldEvent::Mined { rock, ore, galvum });
    }

    /// Stage 8: each body, one step on, exposed while outside in the suit
    /// and sheltered otherwise. Closed form in the health crate, so a
    /// server catching up on an hour lands where a browser did.
    fn run_health(&mut self, events: &mut Vec<WorldEvent>) {
        for who in 0..self.health.len() {
            let exposure = if self.aboard.room.is_outside(who) {
                health::Exposure::Exposed {
                    intensity: data::SUIT_INTENSITY,
                }
            } else {
                health::Exposure::Shielded
            };
            for event in health::update(&mut self.health[who], data::STEP_MINUTES, exposure) {
                events.push(WorldEvent::Health {
                    who: who as u32,
                    event,
                });
            }
        }
    }

    /// The ship's power, as the crew would read it off a panel.
    pub fn power(&self) -> Power {
        let budget = self.power_budget;
        Power {
            supply: budget.supply,
            draw: budget.draw,
            engines: self.engine_draw_now(),
            storage: budget.storage,
            charge: self.ship.charge,
        }
    }

    /// Whether a consumer of this kind is running: some part of that kind
    /// is on a live network, and either the ship is not browned out or the
    /// kind is one of the essentials, which run off the reactor's own
    /// output when the batteries are flat. A kind that draws nothing is
    /// running by definition; a kind that draws and is not aboard is not.
    ///
    /// Asked per **kind** rather than per part, because what asks it is a
    /// chain deciding whether the smelter works, and a chain has a kind in
    /// hand and not an id.
    pub fn powered(&self, kind: PartKind) -> bool {
        let def = kind.def();
        if !def.draws() {
            return true;
        }
        let wired = self
            .ship
            .design
            .parts
            .iter()
            .any(|p| p.kind == kind && self.powered_parts.binary_search(&p.id).is_ok());
        wired && (!self.power().brownout() || shipdesign::essential(kind))
    }

    /// Whether the construction step may touch a part of this kind.
    ///
    /// **It has to be asked before any construction or deconstruction.** One
    /// thing it refuses, and it is about a promise already made: an engine,
    /// a thruster or a reactor taken off mid-flight would change a trip that
    /// has been quoted — the reactor because it is what the engines run on.
    /// A trip keeps the dynamics it was planned with either way
    /// (`flight::dynamics`'s note); this keeps the picture honest with them.
    pub fn can_modify_part(&self, kind: PartKind) -> bool {
        let travelling = matches!(self.ship.state, ShipState::Travelling { .. });
        let def = kind.def();
        if travelling && (def.pushes() || def.turns() || def.supplies()) {
            return false;
        }
        true
    }

    /// Whether this player may give the ship an order: their crew member
    /// has to be standing at the helm. Confirm and Abort ask it; speed and
    /// trading deliberately do not.
    pub fn can_command(&self, slot: u32) -> bool {
        slot < self.speed_requests.len() as u32 && self.at_the_helm(slot)
    }

    /// The seat at the helm: the middle of the first helm's use spot, in
    /// the ship's design units. `None` for a ship with no helm, which
    /// nobody can fly from anywhere.
    pub fn helm_spot(&self) -> Option<DVec2> {
        let helm = self
            .ship
            .design
            .parts
            .iter()
            .filter(|p| p.kind == PartKind::Helm)
            .min_by_key(|p| p.id)?;
        let &(x, y) = helm.use_spots().first()?;
        let t = shipdesign::TILE as f64;
        Some(DVec2 {
            x: (x as f64 + 0.5) * t,
            y: (y as f64 + 0.5) * t,
        })
    }

    /// Whether that player's crew member is at the helm: alive, awake,
    /// aboard, and within [`data::HELM_REACH`] of the seat. Slot *i* is
    /// Bim *i*, the same pairing as the bunks. The first three are
    /// [`World::fit_to_act`]'s, the same as the desk's: a body that fell
    /// beside the seat, or a crew member dozing in it, is at the helm
    /// for the picture and for nothing else, and a Confirm from their
    /// player is `Refusal::NotAtTheHelm`. Asked when a command arrives
    /// and not again: one who walks away under way is refused the *next*
    /// order, and the trip carries on.
    pub fn at_the_helm(&self, slot: u32) -> bool {
        if !self.fit_to_act(slot) {
            return false;
        }
        self.helm_spot()
            .is_some_and(|seat| self.aboard.position(slot).distance(seat) <= data::HELM_REACH)
    }

    /// Whether that player's crew member is in a state to do anything at
    /// all for them: a slot with a Bim in it, alive, awake — neither out
    /// cold nor asleep — and aboard rather than out on the hull. What the
    /// helm and the desk ask before where the body is standing.
    fn fit_to_act(&self, slot: u32) -> bool {
        if slot >= self.aboard.crew_count() {
            return false;
        }
        let room = &self.aboard.room;
        let who = slot as usize;
        room.is_alive(who)
            && !room.is_unconscious(who)
            && !room.is_asleep(who)
            && !room.is_outside(who)
    }

    /// Whether that player's crew member is at a trading desk: alive,
    /// awake, aboard ([`World::fit_to_act`]), and within [`data::REACH`]
    /// tiles of a desk's footprint on the deck it walks — the station's,
    /// on the joined deck. What a buy or a sell wants beside the berth
    /// (`Refusal::NotAtTheDesk`): the station is traded with across its
    /// desk, and the goods still go straight into the hold. A ship has
    /// no desk of its own, so away from a berth this is never true.
    pub fn at_the_desk(&self, slot: u32) -> bool {
        if !self.fit_to_act(slot) {
            return false;
        }
        let room = &self.aboard.room;
        let who = slot as usize;
        let here = room.bim_pos(who);
        let reach = data::REACH * shipdesign::TILE as f32;
        room.desks().iter().any(|(frame, _)| {
            let near = bims::math::vec2(
                here.x.clamp(frame.min.x, frame.max.x),
                here.y.clamp(frame.min.y, frame.max.y),
            );
            (here - near).len() <= reach
        })
    }

    /// Where that player's crew member stands to trade: the first desk's
    /// stand spot on the deck, in the room's units, for the app to
    /// `send_to`. `None` with no desk on the deck — a ship on its own.
    pub fn desk_spot(&self) -> Option<bims::math::Vec2> {
        self.aboard.room.desk_spot(0)
    }

    /// Walk that player's crew member to the station's trading desk:
    /// `Command::ToDesk`, since the walk moves a crew member and every
    /// player's ship has to agree about where each of them is. False
    /// with no desk to walk to.
    pub fn walk_to_desk(&mut self, slot: u32) -> bool {
        if slot >= self.aboard.crew_count() {
            return false;
        }
        let Some(at) = self.desk_spot() else {
            return false;
        };
        self.aboard.room.send_to(slot as usize, at)
    }

    /// Stand that player's crew member at the trading desk, without the
    /// walk. For probes of trading, which have to be at it.
    pub fn man_the_desk_for_probe(&mut self, slot: u32) -> bool {
        let Some(at) = self.desk_spot() else {
            return false;
        };
        if slot >= self.aboard.crew_count() {
            return false;
        }
        self.aboard.room.post_for_probe(slot as usize, at);
        true
    }

    /// Send that player's crew member to the helm, to stand there until
    /// sent elsewhere — `Command::ToHelm`. Once a room order that crossed
    /// no seam; a command since the wire went in (feature 59), because
    /// the walk moves a crew member and the crew's positions are in the
    /// checksum. False when there is no helm or no way to it.
    pub fn order_to_helm(&mut self, slot: u32) -> bool {
        if slot >= self.aboard.crew_count() {
            return false;
        }
        let Some(seat) = self.helm_spot() else {
            return false;
        };
        let at = seat.add(self.aboard.offset);
        self.aboard
            .room
            .send_to(slot as usize, bims::math::vec2(at.x as f32, at.y as f32))
    }

    /// Lift the post [`World::order_to_helm`] set, so that player's crew
    /// member goes back about its errands. The page's Confirm walks the
    /// Bim to the seat, gives the order once it is there, and then lets it
    /// go — under way the helm is a job the room hands out itself.
    /// `CrewOrder::StandDown` through `Command::Crew`, like the walk.
    pub fn stand_down(&mut self, slot: u32) {
        if slot < self.aboard.crew_count() {
            self.aboard.room.stand_down(slot as usize);
        }
    }

    /// Stand that player's crew member at the helm this instant. For the
    /// probes: a test of a trip is not a test of the walk to the seat.
    pub fn man_the_helm_for_probe(&mut self, slot: u32) {
        let Some(seat) = self.helm_spot() else {
            return;
        };
        let at = seat.add(self.aboard.offset);
        self.aboard
            .room
            .post_for_probe(slot as usize, bims::math::vec2(at.x as f32, at.y as f32));
    }

    // --- trading ------------------------------------------------------------

    fn buy(&mut self, slot: u32, resource: ResourceId, units: u32, events: &mut Vec<WorldEvent>) {
        let ShipState::Docked { station } = self.ship.state else {
            events.push(refused(slot, Refusal::NotDocked));
            return;
        };
        let Some(desk) = self
            .station(station)
            .filter(|s| s.stock.sells(resource))
            .and_then(|s| s.market())
        else {
            events.push(refused(slot, Refusal::NotSoldHere));
            return;
        };
        // At the desk's ask — `economy::market`, never the book.
        let Ok(value) = desk.quote(resource).cost(units) else {
            events.push(refused(slot, Refusal::Unaffordable));
            return;
        };
        if value > self.money {
            events.push(refused(slot, Refusal::Unaffordable));
            return;
        }
        if !self.has_room(resource, units) {
            events.push(refused(slot, Refusal::NoRoomAboard));
            return;
        }
        // Last, as for a sale: "walk over first" only about a buy that
        // would otherwise go.
        if !self.at_the_desk(slot) {
            events.push(refused(slot, Refusal::NotAtTheDesk));
            return;
        }
        self.money -= value;
        self.ship.design.cargo[resource as usize] += units;
        self.on_ship_changed();
        events.push(WorldEvent::Traded {
            slot,
            resource,
            units: units as i64,
        });
    }

    fn sell(&mut self, slot: u32, resource: ResourceId, units: u32, events: &mut Vec<WorldEvent>) {
        let ShipState::Docked { station } = self.ship.state else {
            events.push(refused(slot, Refusal::NotDocked));
            return;
        };
        // Somebody to sell to: a derelict keeps no desk.
        let Some(desk) = self.station(station).and_then(|s| s.market()) else {
            events.push(refused(slot, Refusal::NoMarket));
            return;
        };
        // What the construction sites have claimed is not the crew's to
        // sell — see `free`.
        let aboard = self.free(resource);
        if units > aboard {
            events.push(refused(slot, Refusal::NotAboard));
            return;
        }
        // What is asked is sound; now whether anybody is at the desk to
        // ask it — last, so "walk over first" is said only about a sale
        // that would otherwise go.
        if !self.at_the_desk(slot) {
            events.push(refused(slot, Refusal::NotAtTheDesk));
            return;
        }
        // At the desk's bid, which is under its ask: what was bought here
        // and sold straight back has lost money.
        let Ok(value) = desk.quote(resource).fetches(units) else {
            events.push(refused(slot, Refusal::SumTooBig));
            return;
        };
        let Ok(money) = economy::add(self.money, value) else {
            events.push(refused(slot, Refusal::SumTooBig));
            return;
        };
        self.money = money;
        self.ship.design.cargo[resource as usize] -= units;
        self.on_ship_changed();
        events.push(WorldEvent::Traded {
            slot,
            resource,
            units: -(units as i64),
        });
    }

    // --- armour and the pack -------------------------------------------------

    /// The pieces against the hold: for each kind, as many pieces `at ==
    /// Hold` as the hold counts of its resource. A count that has grown —
    /// a purchase, a bench — gets fresh pieces, whole, ids in order; one
    /// that has shrunk — a sale — loses its most damaged piece first,
    /// which is the sell rule. Idempotent, and asked at every
    /// `on_ship_changed`, so the invariant in [`crate::armour`] holds
    /// wherever the count is read. A stow or a fetch moves the piece
    /// *before* it moves the count, so this finds nothing to do there.
    fn settle_pieces(&mut self) {
        for kind in bims::combat::ArmourKind::ALL {
            let wanted = self.ship.design.carrying(armour::resource_of(kind)) as usize;
            loop {
                let held: Vec<usize> = self
                    .pieces
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| p.kind == kind && p.at == Where::Hold)
                    .map(|(i, _)| i)
                    .collect();
                if held.len() < wanted {
                    let id = self.next_piece;
                    self.next_piece += 1;
                    self.pieces.push(Piece::new(id, kind, Tier::One));
                } else if held.len() > wanted {
                    // The most damaged; the lowest id among equals, since
                    // `min_by` keeps the first.
                    let worst = held
                        .iter()
                        .copied()
                        .min_by(|&a, &b| {
                            self.pieces[a]
                                .health
                                .partial_cmp(&self.pieces[b].health)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .expect("held is not empty");
                    self.pieces.remove(worst);
                } else {
                    break;
                }
            }
        }
    }

    /// The world's copies of the pieces in packs and on bodies, read back
    /// from the room, where those live: where each is and what it has
    /// left. Any piece the room has broken since the last look is said.
    /// Asked after the room has stepped, and after every command that
    /// moves gear, so the checksum and a reader of `pieces` see what the
    /// room sees.
    fn mirror_pieces(&mut self, events: &mut Vec<WorldEvent>) {
        let room = &self.aboard.room;
        let pieces = &mut self.pieces;
        let place = |pieces: &mut Vec<Piece>, p: bims::combat::Piece, at: Where| {
            if let Some(piece) = pieces.iter_mut().find(|q| q.id == p.id) {
                piece.health = p.health;
                piece.at = at;
            }
        };
        for who in 0..self.aboard.crew as usize {
            for (cell, item) in room.pack(who).iter().enumerate() {
                if let Some(Item::Armour(p)) = item {
                    let at = Where::Pack {
                        who: who as u32,
                        cell: cell as u8,
                    };
                    place(pieces, *p, at);
                }
            }
            for part in Part::ALL {
                if let Some(p) = room.worn(who, part) {
                    place(pieces, p, Where::Worn { who: who as u32 });
                }
            }
        }
        for (who, kind) in self.aboard.room.take_pieces_broken() {
            if who < self.aboard.crew as usize {
                events.push(WorldEvent::PieceBroke {
                    who: who as u32,
                    kind,
                });
            }
        }
    }

    // --- tiers and the workbench's upgrade -----------------------------------

    /// The weapons in the hold against its count of them: for each kind,
    /// as many `guns` as the hold counts of its resource. A count that has
    /// grown — the armoury, a test poking `cargo[]` — gets tier-one
    /// weapons; one that has shrunk loses its lowest tier first, which is
    /// the sell rule. The list is kept sorted by kind and tier, so two
    /// worlds that did the same things hash the same list. Idempotent,
    /// and asked at every `on_ship_changed` after `settle_pieces`; a stow
    /// or a fetch moves the gun *before* the count, so this finds nothing
    /// to do there.
    fn settle_guns(&mut self) {
        for kind in WeaponKind::ALL {
            let wanted = self.ship.design.carrying(armour::weapon_resource(kind)) as usize;
            loop {
                let held = self.guns.iter().filter(|g| g.kind == kind).count();
                if held < wanted {
                    self.guns.push(kind.basic());
                } else if held > wanted {
                    let worst = self
                        .guns
                        .iter()
                        .enumerate()
                        .filter(|(_, g)| g.kind == kind)
                        .min_by_key(|(_, g)| g.tier)
                        .map(|(i, _)| i)
                        .expect("held is not empty");
                    self.guns.remove(worst);
                } else {
                    break;
                }
            }
        }
        self.guns.sort_by_key(|g| (g.kind.code(), g.tier.code()));
    }

    // --- the lockers' grid ----------------------------------------------------

    /// The grids' classes, in the order `grids` keeps them: the code of
    /// each is its index. The research desk is a count of one and has
    /// none.
    pub const GRID_CLASSES: [Storage; 3] = [Storage::Shelf, Storage::ColdStore, Storage::Locker];

    /// A class's grid, if the class is one: the shelves', the cold
    /// stores' or the lockers'.
    pub fn grid(&self, class: Storage) -> Option<&Grid> {
        World::GRID_CLASSES
            .iter()
            .position(|&c| c == class)
            .map(|i| &self.grids[i])
    }

    fn grid_mut(&mut self, class: Storage) -> Option<&mut Grid> {
        World::GRID_CLASSES
            .iter()
            .position(|&c| c == class)
            .map(|i| &mut self.grids[i])
    }

    /// A class's grid in cells: every part of the class's capacity added
    /// up.
    pub fn grid_capacity(&self, class: Storage) -> u32 {
        self.ship.design.capacity(class)
    }

    /// Everything a class holds, in the order the slots are settled in:
    /// for the lockers the pieces of armour in the hold by id and the
    /// guns by kind and tier; then every other resource of the class with
    /// its count, in `ResourceId` order — each with its footprint.
    fn grid_wanted(&self, class: Storage) -> Vec<Wanted> {
        let mut wanted = Vec::new();
        if class == Storage::Locker {
            for piece in self.pieces.iter().filter(|p| p.at == Where::Hold) {
                wanted.push(Wanted::Piece(piece.id, footprint(piece.resource())));
            }
            for gun in &self.guns {
                wanted.push(Wanted::Gun(
                    gun.kind,
                    gun.tier,
                    footprint(armour::weapon_resource(gun.kind)),
                ));
            }
        }
        for &id in ResourceId::ALL.iter() {
            if storage(id) != class || armour::is_gear(id) {
                continue;
            }
            wanted.push(Wanted::Units(
                id,
                self.ship.design.carrying(id),
                footprint(id),
            ));
        }
        wanted
    }

    /// The slots of every grid against what its class holds — see
    /// [`crate::grid`] for the rule and what an overflow does. Asked at
    /// every `on_ship_changed` after the pieces and the guns, since a
    /// slot names a piece by id and a gun by tier.
    fn settle_grids(&mut self) {
        for class in World::GRID_CLASSES {
            let wanted = self.grid_wanted(class);
            let capacity = self.grid_capacity(class);
            if let Some(grid) = self.grid_mut(class) {
                grid.settle(capacity, &wanted);
            }
        }
    }

    /// Whether `units` more of a resource could come aboard: room in its
    /// class by area (`ShipDesign::has_room`) and a place on the class's
    /// grid for each — a part-full stack topped up, or a run of cells for
    /// a new one — the rule every purchase, craft, stow and upgrade asks
    /// before the count moves, so the settle finds room for what they let
    /// through.
    pub fn has_room(&self, resource: ResourceId, units: u32) -> bool {
        if !self.ship.design.has_room(resource, units) {
            return false;
        }
        let class = storage(resource);
        match self.grid(class) {
            Some(grid) => grid.can_take(
                self.grid_capacity(class),
                resource,
                units,
                footprint(resource),
            ),
            None => true,
        }
    }

    /// The most of `wanted` more units of a resource that would come
    /// aboard — what a haul or a harvest is cut to.
    pub fn room_for(&self, resource: ResourceId, wanted: u32) -> u32 {
        (0..=wanted)
            .rev()
            .find(|&n| self.has_room(resource, n))
            .unwrap_or(0)
    }

    /// Move a slot of a class's grid to `(x, y)`, turned or not — see
    /// [`Command::Arrange`]. Refused `NoRoom` when it would not lie there,
    /// there is no such slot, or the class has no grid.
    fn arrange(
        &mut self,
        slot: u32,
        class: u32,
        id: u32,
        x: u32,
        y: u32,
        turned: bool,
        events: &mut Vec<WorldEvent>,
    ) {
        let moved = Storage::from_code(class).is_some_and(|class| {
            let capacity = self.grid_capacity(class);
            self.grid_mut(class)
                .is_some_and(|grid| grid.arrange(capacity, id, x, y, turned))
        });
        if !moved {
            events.push(refused(slot, Refusal::NoRoom));
        }
    }

    /// Move a thing across a crew member's pack — see [`Command::Repack`].
    /// The room does the moving; the pieces are read back after, since a
    /// piece's cell is in the checksum.
    fn repack(
        &mut self,
        slot: u32,
        who: u32,
        cell: u32,
        to: u32,
        turned: bool,
        events: &mut Vec<WorldEvent>,
    ) {
        if self.pack_item(who, cell).is_none() {
            events.push(refused(slot, Refusal::NotAboard));
            return;
        }
        if !self
            .aboard
            .room
            .rearrange(who as usize, cell as usize, to as usize, turned)
        {
            events.push(refused(slot, Refusal::NoRoom));
            return;
        }
        self.mirror_pieces(events);
    }

    /// How many weapons of `kind` at `tier` the hold has — what a
    /// container window lists a cell for.
    pub fn guns_at(&self, kind: WeaponKind, tier: Tier) -> u32 {
        self.guns
            .iter()
            .filter(|g| g.kind == kind && g.tier == tier)
            .count() as u32
    }

    /// Whether the crew combine matching gear at the workbench whenever
    /// there is a pair — `Command::SetAutoUpgrade`.
    pub fn auto_upgrade(&self) -> bool {
        self.auto_upgrade
    }

    /// The workbench with the slots: the first workbench in bench order,
    /// by its index in the room's benches — the container a thing is put
    /// on and taken off, and the bench the day's work is done at. `None`
    /// with no workbench aboard.
    pub fn workbench(&self) -> Option<usize> {
        self.aboard
            .room
            .benches()
            .iter()
            .position(|b| b.kind == PartKind::Workbench.code())
    }

    /// The cabinet a carry to the workbench fetches from and a carry back
    /// returns to: the first bench in bench order whose part keeps the
    /// locker class — the armoury, the drug lab. The hold is one pool, so
    /// any cabinet of the class is where a gun is; a ship with no such
    /// bench has nowhere for a Bim to walk to, and the crew carry
    /// nothing of their own accord.
    pub fn store_bench(&self) -> Option<usize> {
        self.aboard.room.benches().iter().position(|b| {
            PartKind::from_code(b.kind)
                .and_then(|kind| kind.def().capacity)
                .is_some_and(|(class, _)| class == Storage::Locker)
        })
    }

    /// Whether crew member `who` stands within [`data::REACH`] of the
    /// workbench — alive, aboard, and near enough to reach onto it. What
    /// a put-on and a take-off ask first.
    pub fn in_reach_of_bench(&self, who: u32) -> bool {
        if who >= self.aboard.crew_count() {
            return false;
        }
        self.workbench().is_some_and(|bench| {
            self.aboard
                .room
                .within_reach(who as usize, Container::Bench(bench), data::REACH)
        })
    }

    /// Whether the button could be pressed now, or why not: a workbench
    /// aboard, the upgrades node researched, nothing under way, the output
    /// slot clear, and a pair in the input slots — what
    /// [`Command::Upgrade`] checks, so a window can say so first.
    pub fn can_upgrade(&self) -> Result<(), Refusal> {
        if self.workbench().is_none() {
            return Err(Refusal::NoWorkbench);
        }
        if !self.research.upgrades_allowed() {
            return Err(Refusal::NoUpgrades);
        }
        if self.bench.busy() || self.bench.slots[Workbench::OUT].is_some() {
            return Err(Refusal::BenchBusy);
        }
        self.bench.pair().map(|_| ())
    }

    /// The first pair of matching gear in the hold that could go up a
    /// tier, for the crew to carry to the bench: armour kinds first, then
    /// weapons, the lowest tier first within a kind, tier three never —
    /// and nothing at all until the upgrades node is researched, so no
    /// pair is carried to a bench that would refuse it. What the pair is;
    /// which pieces is `bench_wants`'s.
    fn upgrade_pair(&self) -> Option<(ResourceId, Tier)> {
        if !self.research.upgrades_allowed() {
            return None;
        }
        for kind in ArmourKind::ALL {
            for tier in Tier::ALL {
                if tier.next().is_none() {
                    continue;
                }
                let held = self
                    .pieces
                    .iter()
                    .filter(|p| p.kind == kind && p.tier == tier && p.at == Where::Hold)
                    .count();
                if held >= 2 {
                    return Some((armour::resource_of(kind), tier));
                }
            }
        }
        for kind in WeaponKind::ALL {
            for tier in Tier::ALL {
                if tier.next().is_some() && self.guns_at(kind, tier) >= 2 {
                    return Some((armour::weapon_resource(kind), tier));
                }
            }
        }
        None
    }

    /// What the bench wants carried to it next, out of the hold, with the
    /// tick box on: with one input slot filled, a match for it — the same
    /// kind at the same tier; with both empty, the first of the first pair
    /// in the hold. For armour the **most damaged** piece of the kind and
    /// tier: the good ones stay in circulation, and the piece that comes
    /// out is fresh whatever went in. `None` with nothing to carry — both
    /// slots full, or nothing in the hold that would pair.
    fn bench_wants(&self) -> Option<Kept> {
        if self.bench.free_in().is_none() || self.bench.busy() || !self.research.upgrades_allowed()
        {
            return None;
        }
        let other = Workbench::IN.into_iter().find_map(|i| self.bench.slots[i]);
        let (resource, tier) = match other {
            Some(item) => (armour::resource_of_item(item)?, tier_of(item)?),
            None => self.upgrade_pair()?,
        };
        if tier.next().is_none() {
            return None;
        }
        if let Some(kind) = armour::kind_of(resource) {
            self.pieces
                .iter()
                .filter(|p| p.kind == kind && p.tier == tier && p.at == Where::Hold)
                .min_by(|a, b| {
                    a.health
                        .partial_cmp(&b.health)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then(a.id.cmp(&b.id))
                })
                .map(|p| Kept::Piece(p.id))
        } else {
            let kind = armour::weapon_of(resource)?;
            (self.guns_at(kind, tier) > 0).then_some(Kept::Gun(kind, tier))
        }
    }

    /// Stage 5, before the craft orders, with the tick box on: the crew
    /// see to the bench themselves. The output waiting in its slot is
    /// carried back to the lockers; a pair in the input slots has the
    /// button pressed for it; an input slot empty has the next thing that
    /// would pair carried over from the lockers. One thing in the arms at
    /// a time, and none of it without a workbench and a cabinet aboard.
    /// What the crew are asked to carry this step, for the room.
    fn tend_bench(&mut self, events: &mut Vec<WorldEvent>) -> Vec<bims::game::Ferry> {
        let (Some(bench), Some(store)) = (self.workbench(), self.store_bench()) else {
            return Vec::new();
        };
        if self.bench.busy() {
            return Vec::new();
        }
        // A carry under way keeps its order until it lands: the room is a
        // step behind, and a chain with its order gone gives the thing up.
        if self.bench.carrying.is_some() {
            return vec![if self.bench.back {
                bims::game::Ferry {
                    from: bench,
                    to: store,
                }
            } else {
                bims::game::Ferry {
                    from: store,
                    to: bench,
                }
            }];
        }
        if !self.auto_upgrade {
            return Vec::new();
        }
        if let Some(out) = self.bench.slots[Workbench::OUT] {
            // Back to the lockers, once there is room for it there.
            let room = armour::resource_of_item(out).is_some_and(|r| self.has_room(r, 1));
            return if room {
                vec![bims::game::Ferry {
                    from: bench,
                    to: store,
                }]
            } else {
                Vec::new()
            };
        }
        if self.bench.pair().is_ok() {
            // Never refused here: the pair was just checked and nothing is
            // under way.
            let _ = self.begin_upgrade(events);
            return Vec::new();
        }
        if self.bench_wants().is_some() {
            return vec![bims::game::Ferry {
                from: store,
                to: bench,
            }];
        }
        Vec::new()
    }

    /// The day's work begun on the pair in the input slots — the button,
    /// or the tick box pressing it: [`Command::Upgrade`]. The two stay on
    /// the bench, being worked on, until `finish_upgrade` swaps them for
    /// the one.
    fn begin_upgrade(&mut self, events: &mut Vec<WorldEvent>) -> Result<(), Refusal> {
        self.can_upgrade()?;
        let (resource, tier) = self.bench.pair()?;
        let to = tier.next().ok_or(Refusal::NoPair)?;
        self.bench.work = Some(Upgrade {
            resource,
            to,
            done: 0,
        });
        events.push(WorldEvent::UpgradeBegun {
            resource: resource as u32,
            tier: to.code(),
        });
        Ok(())
    }

    /// One session of the upgrade finished at the workbench — the room
    /// said `UPGRADE_ORDER` through `take_crafted`: an hour more of the
    /// day. The item itself is `finish_upgrade`'s.
    fn finish_upgrade_session(&mut self) {
        if let Some(upgrade) = self.bench.work.as_mut()
            && upgrade.done < data::UPGRADE_SESSIONS
        {
            upgrade.done += 1;
        }
    }

    /// Every step after the crafts: an upgrade with its day done takes the
    /// pair off the bench and puts one of the next tier in the output slot
    /// — a fresh piece with a fresh id, or a gun — for a crew member to
    /// take, or the crew to carry back to the lockers with the tick box
    /// on. Nothing in the hold moves.
    fn finish_upgrade(&mut self, events: &mut Vec<WorldEvent>) {
        let Some(upgrade) = self.bench.work else {
            return;
        };
        if !upgrade.complete() {
            return;
        }
        let made = if let Some(kind) = armour::kind_of(upgrade.resource) {
            let id = self.next_piece;
            self.next_piece += 1;
            Some(Piece::new(id, kind, upgrade.to).item())
        } else {
            armour::weapon_at(upgrade.resource, upgrade.to).map(Item::Weapon)
        };
        for i in Workbench::IN {
            self.bench.slots[i] = None;
        }
        self.bench.slots[Workbench::OUT] = made;
        self.bench.work = None;
        events.push(WorldEvent::Upgraded {
            resource: upgrade.resource as u32,
            tier: upgrade.to.code(),
        });
    }

    /// A thing out of `who`'s pack onto the workbench's first free input
    /// slot. See [`Command::StowOnBench`] for what is checked.
    fn stow_on_bench(&mut self, slot: u32, who: u32, cell: u32, events: &mut Vec<WorldEvent>) {
        let Some(item) = self.pack_item(who, cell) else {
            events.push(refused(slot, Refusal::NotAboard));
            return;
        };
        if self.workbench().is_none() {
            events.push(refused(slot, Refusal::NoWorkbench));
            return;
        }
        if let Item::Armour(p) = item
            && p.broken()
        {
            events.push(refused(slot, Refusal::Broken));
            return;
        }
        let at = match self.bench.takes(item) {
            Ok(at) => at,
            Err(why) => {
                events.push(refused(slot, why));
                return;
            }
        };
        if !self.in_reach_of_bench(who) {
            events.push(refused(slot, Refusal::OutOfReach));
            return;
        }
        let Some(taken) = self.aboard.room.take(who as usize, cell as usize) else {
            events.push(refused(slot, Refusal::Broken));
            return;
        };
        // A piece leaves the world's list the way one leaving the hold
        // for a pack joins the room's: the bench keeps it whole, health
        // and all, and it is pushed back when it is taken off.
        if let Item::Armour(p) = taken {
            self.pieces.retain(|q| q.id != p.id);
        }
        self.bench.slots[at] = Some(taken);
        events.push(WorldEvent::Stowed { who });
    }

    /// A thing off one of the workbench's slots into `who`'s pack —
    /// `FetchKind::Bench`: an input back, while nothing is under way, or
    /// the output any time. Nothing in the hold moves.
    fn fetch_from_bench(&mut self, slot: u32, who: u32, at: u32, events: &mut Vec<WorldEvent>) {
        if self.workbench().is_none() {
            events.push(refused(slot, Refusal::NoWorkbench));
            return;
        }
        let Some(item) = self.bench.slots.get(at as usize).copied().flatten() else {
            events.push(refused(slot, Refusal::NotAboard));
            return;
        };
        if self.bench.busy() && at as usize != Workbench::OUT {
            events.push(refused(slot, Refusal::BenchBusy));
            return;
        }
        if !self.in_reach_of_bench(who) {
            events.push(refused(slot, Refusal::OutOfReach));
            return;
        }
        let Some(cell) = self.aboard.room.gear(who as usize).free_cell_for(item) else {
            events.push(refused(slot, Refusal::PackFull));
            return;
        };
        if !self.aboard.room.give(who as usize, Some(cell), item) {
            events.push(refused(slot, Refusal::PackFull));
            return;
        }
        self.bench.slots[at as usize] = None;
        if let Item::Armour(p) = item {
            self.pieces.push(Piece {
                id: p.id,
                kind: p.kind,
                tier: p.tier,
                health: p.health,
                at: Where::Pack {
                    who,
                    cell: cell as u8,
                },
            });
        }
    }

    /// A thing into the hold from the bench's arms — a carry given up, a
    /// carry back landed, or the room taken apart with one under way: the
    /// instance first and the count second, as everywhere. Room or no
    /// room: it was in the hold a moment ago, and the grid keeps what it
    /// cannot lay unplaced rather than losing it.
    fn hold_takes(&mut self, item: Item) {
        let Some(resource) = armour::resource_of_item(item) else {
            return;
        };
        match item {
            Item::Armour(p) => self.pieces.push(Piece {
                id: p.id,
                kind: p.kind,
                tier: p.tier,
                health: p.health,
                at: Where::Hold,
            }),
            Item::Weapon(gun) => self.guns.push(gun),
            Item::Stack(_) | Item::Key(_) => {}
        }
        self.ship.design.cargo[resource as usize] += 1;
        self.on_ship_changed();
    }

    /// A thing out of the hold, by what the bench wants, into the bench's
    /// arms: the instance, its slot on the grid, then the count.
    fn hold_gives(&mut self, kept: Kept) -> Option<Item> {
        let (item, resource) = match kept {
            Kept::Piece(id) => {
                let i = self
                    .pieces
                    .iter()
                    .position(|p| p.id == id && p.at == Where::Hold)?;
                let p = self.pieces.remove(i);
                (p.item(), p.resource())
            }
            Kept::Gun(kind, tier) => {
                let gun = kind.at(tier);
                let i = self.guns.iter().position(|g| *g == gun)?;
                self.guns.remove(i);
                (Item::Weapon(gun), armour::weapon_resource(kind))
            }
            Kept::Stack(_) => return None,
        };
        if let Some(grid) = self.grid_mut(storage(resource))
            && let Some(id) = grid.slots.iter().find(|s| s.kept == kept).map(|s| s.id)
        {
            grid.take(id);
        }
        self.ship.design.cargo[resource as usize] -= 1;
        self.on_ship_changed();
        Some(item)
    }

    /// Stage 7: a Bim reached into the first bench of a carry. From the
    /// cabinet, what the bench wants comes out of the hold into the arms
    /// — nothing, if it was sold or taken since the order, and the Bim
    /// carries nothing; from the workbench, the output comes off it.
    fn finish_ferry_pick(&mut self, ferry: bims::game::Ferry) {
        if self.bench.carrying.is_some() {
            return;
        }
        if Some(ferry.from) == self.workbench() {
            self.bench.carrying = self.bench.slots[Workbench::OUT].take();
            self.bench.back = true;
        } else if Some(ferry.to) == self.workbench()
            && let Some(kept) = self.bench_wants()
        {
            self.bench.carrying = self.hold_gives(kept);
            self.bench.back = false;
        }
    }

    /// Stage 7: a Bim put a carried thing down at the second bench. At the
    /// workbench it goes into the first free input slot — into the hold
    /// instead if the slots filled meanwhile; at the cabinet, into the
    /// hold.
    fn finish_ferry_drop(&mut self, ferry: bims::game::Ferry) {
        let Some(item) = self.bench.carrying.take() else {
            return;
        };
        if Some(ferry.to) == self.workbench()
            && let Ok(at) = self.bench.takes(item)
        {
            self.bench.slots[at] = Some(item);
        } else {
            self.hold_takes(item);
        }
    }

    /// Stage 7: a carry given up with the thing in the arms. Into the
    /// hold, whichever way it was going.
    fn finish_ferry_return(&mut self, _ferry: bims::game::Ferry) {
        if let Some(item) = self.bench.carrying.take() {
            self.hold_takes(item);
        }
    }

    /// Whether a container takes a resource: a bench whose part is a
    /// cabinet of the resource's class — the armoury and the drug lab are
    /// lockers; the smelter and the workbench hold nothing — a shelf for
    /// shelf goods and, since a storage can hold armour or weapons as
    /// well as materials, for anything worn or held; a cold store for
    /// what goes off. Which class the thing *counts* against is
    /// `economy::storage` whichever container it went through: the class
    /// rules stay the one truth about capacity.
    pub fn container_takes(&self, container: Container, resource: ResourceId) -> bool {
        let class = storage(resource);
        match container {
            Container::Bench(i) => PartKind::from_code(self.aboard.room.bench_part(i))
                .and_then(|kind| kind.def().capacity)
                .is_some_and(|(held, _)| held == class),
            // The ship's own shelves only: the station's, on the joined
            // deck, are an enemy's to plunder or a friend's to leave alone,
            // never the hold's.
            Container::Shelf(i) => {
                (class == Storage::Shelf || armour::is_gear(resource))
                    && !self.station_shelves().contains(&i)
            }
            Container::Fridge(_) => class == Storage::ColdStore,
            // The crew's own desks only: a key put on a station's desk
            // would be the station's, and the hold would count it.
            Container::Desk(i) => class == Storage::Research && Some(i) != self.station_desk(),
        }
    }

    /// Whether crew member `who` stands within [`data::REACH`] of a
    /// container that takes `resource` — alive, aboard, and near enough
    /// to reach into it. What a stow and a fetch ask first.
    pub fn in_reach(&self, who: u32, resource: ResourceId) -> bool {
        if who >= self.aboard.crew_count() {
            return false;
        }
        self.aboard.containers().into_iter().any(|c| {
            self.container_takes(c, resource)
                && self.aboard.room.within_reach(who as usize, c, data::REACH)
        })
    }

    /// What is in a pack cell, or `None` for no such crew member, no such
    /// cell, or nothing in it. The tail cell of a tall item — a key — is
    /// that item, the way the room reads it.
    fn pack_item(&self, who: u32, cell: u32) -> Option<Item> {
        if who >= self.aboard.crew_count() || cell as usize >= PACK_CELLS {
            return None;
        }
        let gear = self.aboard.room.gear(who as usize);
        gear.pack[gear.head_of(cell as usize)]
    }

    /// A stow: the thing out of the pack, the count up by one, and a piece
    /// of armour `Where::Hold` with the health it had. See
    /// [`Command::Stow`] for what is checked.
    fn stow(&mut self, slot: u32, who: u32, cell: u32, events: &mut Vec<WorldEvent>) {
        let Some(item) = self.pack_item(who, cell) else {
            events.push(refused(slot, Refusal::NotAboard));
            return;
        };
        if let Item::Armour(p) = item
            && p.broken()
        {
            events.push(refused(slot, Refusal::Broken));
            return;
        }
        let Some(resource) = armour::resource_of_item(item) else {
            events.push(refused(slot, Refusal::NotAboard));
            return;
        };
        if !self.in_reach(who, resource) {
            events.push(refused(slot, Refusal::OutOfReach));
            return;
        }
        // The whole stack goes (feature 87): a cell of five dressings is
        // five, not one, so the hold has to have room for the lot.
        let gear = self.aboard.room.gear(who as usize);
        let units = gear.units(gear.head_of(cell as usize)).max(1);
        if !self.has_room(resource, units) {
            events.push(refused(slot, Refusal::NoRoom));
            return;
        }
        // The room's own refusal is a broken piece, checked above; asked
        // all the same, since the room is the one holding it.
        let Some(taken) = self.aboard.room.take(who as usize, cell as usize) else {
            events.push(refused(slot, Refusal::Broken));
            return;
        };
        match taken {
            Item::Armour(p) => {
                if let Some(piece) = self.pieces.iter_mut().find(|q| q.id == p.id) {
                    piece.health = p.health;
                    piece.at = Where::Hold;
                }
            }
            // A weapon keeps its tier on the list, the way a piece keeps
            // its health.
            Item::Weapon(gun) => self.guns.push(gun),
            Item::Stack(_) | Item::Key(_) => {}
        }
        self.ship.design.cargo[resource as usize] += units;
        self.on_ship_changed();
        events.push(WorldEvent::Stowed { who });
    }

    /// A fetch: one piece by id, or one unit of a resource — the least
    /// damaged piece of the kind for an armour resource — out of the hold
    /// and into the first free pack cell. See [`Command::Fetch`].
    fn fetch(&mut self, slot: u32, who: u32, kind: FetchKind, events: &mut Vec<WorldEvent>) {
        // Off the workbench rather than out of the hold: its own rules.
        if let FetchKind::Bench { slot: at } = kind {
            self.fetch_from_bench(slot, who, at, events);
            return;
        }
        let in_hold = |p: &&Piece| p.at == Where::Hold;
        // A slot of a grid is whatever lies in it, asked for the way the
        // rest are: the piece by id, the gun by its tier, one off the stack
        // by its resource — and it is that slot which goes, not the first
        // of its kind.
        let (kind, from_slot) = match kind {
            FetchKind::Slot { class, id } => {
                let found = Storage::from_code(class)
                    .and_then(|class| self.grid(class))
                    .and_then(|grid| grid.slot(id))
                    .map(|s| s.kept);
                match found {
                    Some(Kept::Piece(piece)) => (FetchKind::Piece(piece), Some(id)),
                    Some(Kept::Gun(kind, tier)) => (
                        FetchKind::Tiered {
                            resource: armour::weapon_resource(kind) as u32,
                            tier: tier.code(),
                        },
                        Some(id),
                    ),
                    Some(Kept::Stack(resource)) => (FetchKind::Resource(resource as u32), Some(id)),
                    None => {
                        events.push(refused(slot, Refusal::NotAboard));
                        return;
                    }
                }
            }
            kind => (kind, None),
        };
        // What is being taken: the resource, and the instance — a piece,
        // or a gun off the list — if the resource is one that has them.
        let (resource, piece, gun) = match kind {
            FetchKind::Piece(id) => {
                let Some(p) = self.pieces.iter().find(|p| p.id == id).filter(in_hold) else {
                    events.push(refused(slot, Refusal::NotAboard));
                    return;
                };
                (p.resource(), Some(*p), None)
            }
            FetchKind::Resource(code) => {
                let Some(resource) = ResourceId::ALL.get(code as usize).copied() else {
                    events.push(refused(slot, Refusal::NotAboard));
                    return;
                };
                let best = armour::kind_of(resource).map(|kind| {
                    self.pieces
                        .iter()
                        .filter(|p| p.kind == kind)
                        .filter(in_hold)
                        .max_by(|a, b| {
                            a.health
                                .partial_cmp(&b.health)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .copied()
                });
                // A weapon by resource is the best of them: the highest
                // tier, the way a piece by resource is the least damaged.
                let gun = armour::weapon_of(resource).map(|kind| {
                    self.guns
                        .iter()
                        .filter(|g| g.kind == kind)
                        .max_by_key(|g| g.tier)
                        .copied()
                });
                match (best, gun) {
                    // An armour resource with no piece to its count, or a
                    // weapon with no gun, would be the invariant broken;
                    // refused rather than made up.
                    (Some(None), _) | (_, Some(None)) => {
                        events.push(refused(slot, Refusal::NotAboard));
                        return;
                    }
                    (piece, gun) => (resource, piece.flatten(), gun.flatten()),
                }
            }
            FetchKind::Tiered { resource, tier } => {
                let Some(resource) = ResourceId::ALL.get(resource as usize).copied() else {
                    events.push(refused(slot, Refusal::NotAboard));
                    return;
                };
                let gun = Tier::from_code(tier).and_then(|tier| armour::weapon_at(resource, tier));
                let Some(gun) = gun.filter(|g| self.guns.contains(g)) else {
                    events.push(refused(slot, Refusal::NotAboard));
                    return;
                };
                (resource, None, Some(gun))
            }
            // Resolved above into one of the three, and the bench handled
            // before any of it.
            FetchKind::Slot { .. } | FetchKind::Bench { .. } => {
                events.push(refused(slot, Refusal::NotAboard));
                return;
            }
        };
        if self.free(resource) == 0 {
            events.push(refused(slot, Refusal::NotAboard));
            return;
        }
        if !self.in_reach(who, resource) {
            events.push(refused(slot, Refusal::OutOfReach));
            return;
        }
        let item = match (piece, gun) {
            (Some(p), _) => p.item(),
            (None, Some(g)) => Item::Weapon(g),
            (None, None) => armour::item_of(resource),
        };
        // A box with room in it first (feature 87), then the first cell
        // the thing fits: a key wants two, one over the other.
        let gear = self.aboard.room.gear(who as usize);
        let cell = gear
            .stack_with_room(item)
            .or_else(|| gear.free_cell_for(item));
        let Some(cell) = cell else {
            events.push(refused(slot, Refusal::PackFull));
            return;
        };
        if !self.aboard.room.give(who as usize, Some(cell), item) {
            events.push(refused(slot, Refusal::PackFull));
            return;
        }
        // The instance first and the count second, so the settle finds
        // them agreeing.
        if let Some(p) = piece
            && let Some(piece) = self.pieces.iter_mut().find(|q| q.id == p.id)
        {
            piece.at = Where::Pack {
                who,
                cell: cell as u8,
            };
        }
        if let Some(g) = gun
            && let Some(i) = self.guns.iter().position(|q| *q == g)
        {
            self.guns.remove(i);
        }
        // And off its class's grid: a piece or a gun with its slot — the
        // one asked for, else the first that holds the thing — one unit
        // off a stack, the one asked for else the last; before the count,
        // like the rest, so the settle finds nothing to drop.
        let class = storage(resource);
        if let Some(grid) = self.grid_mut(class) {
            match (piece, gun) {
                (None, None) => {
                    grid.remove(resource, 1, from_slot);
                }
                (piece, gun) => {
                    let kept = match (piece, gun) {
                        (Some(p), _) => Kept::Piece(p.id),
                        (_, Some(g)) => Kept::Gun(g.kind, g.tier),
                        (None, None) => unreachable!(),
                    };
                    let taken = from_slot
                        .or_else(|| grid.slots.iter().find(|s| s.kept == kept).map(|s| s.id));
                    if let Some(id) = taken {
                        grid.take(id);
                    }
                }
            }
        }
        self.ship.design.cargo[resource as usize] -= 1;
        self.on_ship_changed();
    }

    /// Put on what is in a pack cell. See [`Command::Equip`]. The room
    /// does the swap; the world reads the pieces back and says so.
    fn equip(&mut self, slot: u32, who: u32, cell: u32, events: &mut Vec<WorldEvent>) {
        let kind = match self.pack_item(who, cell) {
            Some(Item::Armour(p)) => Some(p.kind),
            Some(Item::Weapon(_)) => None,
            Some(Item::Stack(_) | Item::Key(_)) | None => {
                events.push(refused(slot, Refusal::NotAboard));
                return;
            }
        };
        if !self.aboard.room.is_alive(who as usize) {
            events.push(refused(slot, Refusal::OutOfReach));
            return;
        }
        self.aboard.room.equip(who as usize, cell as usize);
        self.mirror_pieces(events);
        if let Some(kind) = kind {
            events.push(WorldEvent::Equipped { who, kind });
        }
    }

    /// Take off what is worn on a part, into the pack. See
    /// [`Command::Unequip`].
    fn unequip(&mut self, slot: u32, who: u32, part: Part, events: &mut Vec<WorldEvent>) {
        if who >= self.aboard.crew_count() || !self.aboard.room.is_alive(who as usize) {
            events.push(refused(slot, Refusal::OutOfReach));
            return;
        }
        if self.aboard.room.worn(who as usize, part).is_none() {
            events.push(refused(slot, Refusal::NotAboard));
            return;
        }
        if self.aboard.room.gear(who as usize).free_cell().is_none() {
            events.push(refused(slot, Refusal::PackFull));
            return;
        }
        self.aboard.room.unequip(who as usize, part);
        self.mirror_pieces(events);
    }

    /// Throw away what is in a pack cell. A piece of armour is gone from
    /// `pieces` with it. See [`Command::Discard`].
    fn discard(&mut self, slot: u32, who: u32, cell: u32, events: &mut Vec<WorldEvent>) {
        let Some(item) = self.pack_item(who, cell) else {
            events.push(refused(slot, Refusal::NotAboard));
            return;
        };
        if self.aboard.room.discard(who as usize, cell as usize)
            && let Item::Armour(p) = item
        {
            self.pieces.retain(|q| q.id != p.id);
        }
    }

    // --- looting a body ------------------------------------------------------

    /// The room a body is in, and its index there: the crew's own room
    /// for one of the crew, the station's people's for one of them.
    /// `None` for no such Bim, or for a resident while no station's room
    /// is open.
    fn body_room(&self, source: LootSource) -> Option<(&bims::game::Game, usize)> {
        match source {
            LootSource::Crew(who) => {
                (who < self.aboard.crew_count()).then(|| (&self.aboard.room, who as usize))
            }
            LootSource::Resident(who) => {
                let ashore = &self.residents.as_ref()?.aboard;
                (who < ashore.count()).then(|| (&ashore.room, who as usize))
            }
        }
    }

    /// Whether a body is one: dead, or out cold, in whichever room it
    /// lies (`Game::is_down`). What [`Command::Loot`] asks first, and
    /// what the Loot window watches to know when to shut. `false` for no
    /// such Bim.
    pub fn is_down(&self, source: LootSource) -> bool {
        self.body_room(source)
            .is_some_and(|(room, who)| room.is_down(who))
    }

    /// What a body shows when it is looted, cell by cell in
    /// `bims::combat::LootCell` order — the pack's nine, then the head,
    /// the body, the legs and the weapon in hand — read off whichever
    /// room the body is in. `None` for no such Bim. Down or not: the
    /// window asks, and [`Command::Loot`] is what refuses.
    pub fn loot_cells(&self, source: LootSource) -> Option<[Option<Item>; LOOT_CELLS]> {
        let (room, who) = self.body_room(source)?;
        Some(room.loot_cells(who))
    }

    /// Which way round each thing in a body's pack lies — `Gear::turned`
    /// — for the Loot window to draw it as it is.
    pub fn loot_turned(&self, source: LootSource) -> Option<[bool; PACK_CELLS]> {
        let (room, who) = self.body_room(source)?;
        Some(room.gear(who).turned)
    }

    /// How many are in each of a body's loot cells (feature 87), for the
    /// window's numbers — a box of dressings is five.
    pub fn loot_counts(&self, source: LootSource) -> Option<[u32; LOOT_CELLS]> {
        let (room, who) = self.body_room(source)?;
        Some(room.loot_counts(who))
    }

    /// Where a body lies, in the crew's room's units — the ones
    /// `Game::send_to` and the pointer speak — so the looter can be walked
    /// to it: one of the crew where it stands on the deck, or one of the
    /// station's people where it stands in its own room, put through
    /// `station_frame`. `None` for no such Bim, and for a resident while
    /// the rooms are not joined, since it is then on no deck the crew can
    /// walk.
    pub fn body_position(&self, source: LootSource) -> Option<bims::math::Vec2> {
        match source {
            LootSource::Crew(who) => {
                (who < self.aboard.crew_count()).then(|| self.aboard.room.bim_pos(who as usize))
            }
            LootSource::Resident(who) => {
                let ashore = &self.residents.as_ref()?.aboard;
                if who >= ashore.count() {
                    return None;
                }
                let at = self.aboard.from_station(ashore.position(who))?;
                Some(bims::math::vec2(at.x as f32, at.y as f32))
            }
        }
    }

    /// Which of the station's people is under a point of the crew's
    /// room, if any: what the commander's Attack key reads under the
    /// pointer (feature 78), by the same reach a click on a body has.
    /// `None` while the rooms are not joined.
    pub fn resident_at(&self, x: f32, y: f32) -> Option<u32> {
        let residents = self.residents.as_ref()?;
        let at = bims::math::vec2(x, y);
        (0..residents.aboard.count()).find(|&who| {
            self.body_position(LootSource::Resident(who))
                .is_some_and(|p| (p - at).len() <= bims::character::PICK_RADIUS)
        })
    }

    /// Whether crew member `who` stands within [`data::REACH`] tiles of a
    /// body — alive, awake, aboard, and near enough to go through its
    /// pockets. What a loot asks after the body, and what the Loot window
    /// reads to say "walk over first" before the command is sent and
    /// refused; the command checks again when it lands.
    pub fn in_reach_of_body(&self, who: u32, source: LootSource) -> bool {
        if who >= self.aboard.crew_count() {
            return false;
        }
        let room = &self.aboard.room;
        let looter = who as usize;
        if !room.is_alive(looter) || room.is_unconscious(looter) || room.is_outside(looter) {
            return false;
        }
        let Some(body) = self.body_position(source) else {
            return false;
        };
        (room.bim_pos(looter) - body).len() <= data::REACH * shipdesign::TILE as f32
    }

    /// A loot: one thing off a body into the looter's pack. See
    /// [`Command::Loot`] for what is checked. The body's room does the
    /// stripping (`Game::take_from_body`) and the crew's room the taking
    /// in (`Game::give`); a piece of armour off one of the station's
    /// people is new to the world, and joins `pieces` under a fresh id
    /// with the health it had — the room's own id for it was the
    /// station's, and would collide with the ship's. A piece off a
    /// crewmate is already on the list, and `mirror_pieces` finds it in
    /// the new pack.
    fn loot(
        &mut self,
        slot: u32,
        who: u32,
        source: LootSource,
        cell: u32,
        events: &mut Vec<WorldEvent>,
    ) {
        let Some(cell) = LootCell::from_code(cell) else {
            events.push(refused(slot, Refusal::NotAboard));
            return;
        };
        if !self.is_down(source) {
            events.push(refused(slot, Refusal::NotDown));
            return;
        }
        if !self.in_reach_of_body(who, source) {
            events.push(refused(slot, Refusal::OutOfReach));
            return;
        }
        let Some(free) = self.aboard.room.gear(who as usize).free_cell() else {
            events.push(refused(slot, Refusal::PackFull));
            return;
        };
        // How many are in the cell — a box of dressings is five (feature
        // 87) — asked before the cell is emptied.
        let units = match source {
            LootSource::Crew(body) => self.aboard.room.body_units(body as usize, cell),
            LootSource::Resident(body) => self
                .residents
                .as_ref()
                .map_or(0, |r| r.aboard.room.body_units(body as usize, cell)),
        }
        .max(1);
        let taken = match source {
            LootSource::Crew(body) => self.aboard.room.take_from_body(body as usize, cell),
            LootSource::Resident(body) => self
                .residents
                .as_mut()
                .and_then(|r| r.aboard.room.take_from_body(body as usize, cell)),
        };
        let Some(taken) = taken else {
            events.push(refused(slot, Refusal::NotAboard));
            return;
        };
        let item = match (source, taken) {
            (LootSource::Resident(_), Item::Armour(p)) => {
                let id = self.next_piece;
                self.next_piece += 1;
                let piece = Piece {
                    id,
                    kind: p.kind,
                    tier: p.tier,
                    health: p.health,
                    at: Where::Pack {
                        who,
                        cell: free as u8,
                    },
                };
                self.pieces.push(piece);
                piece.item()
            }
            _ => taken,
        };
        // The cell was free an instant ago and nothing has moved since;
        // a stack goes into a box with room before it takes one of its
        // own, and whatever will not fit is left on the body's cell —
        // `give_stack` says how many went (feature 87).
        let went = self.aboard.room.give_stack(who as usize, item, units);
        if went < units {
            let left = units - went;
            match source {
                LootSource::Crew(body) => {
                    self.aboard.room.give_stack(body as usize, item, left);
                }
                LootSource::Resident(body) => {
                    if let Some(r) = self.residents.as_mut() {
                        r.aboard.room.give_stack(body as usize, item, left);
                    }
                }
            }
        }
        self.mirror_pieces(events);
        events.push(WorldEvent::Looted {
            who,
            source_kind: source.code(),
        });
    }

    // --- an enemy's shelf ------------------------------------------------------

    /// Which of the shelves on the deck are the station's, while the
    /// rooms are joined: the ones standing in the station's box, told
    /// apart from the ship's the way its research desk is
    /// ([`World::station_desk`]). Empty on a ship of its own. These are
    /// not containers of the hold — [`World::container_takes`] says no
    /// for them — but the shelf an enemy's is plundered from.
    pub fn station_shelves(&self) -> Vec<usize> {
        let Some((lo, hi)) = self.aboard.station_box else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut i = 0;
        while let Some(frame) = self.aboard.room.container_frame(Container::Shelf(i)) {
            let m = frame.center();
            if (m.x as f64) >= lo.x
                && (m.x as f64) <= hi.x
                && (m.y as f64) >= lo.y
                && (m.y as f64) <= hi.y
            {
                out.push(i);
            }
            i += 1;
        }
        out
    }

    /// The enemy's shelf the ship is tied up beside, if it is: docked
    /// with the rooms joined at a station whose people are enemies —
    /// a raider, or a hostile station — and its shelf laid out. What the
    /// app's window draws and [`Command::Plunder`] takes from. `None` at
    /// a friend's or a stranger's, whose shelf is the desk's to sell
    /// from, and away from a berth.
    pub fn plunder_alongside(&self) -> Option<&Plunder> {
        let station = self.ship.state.station()?;
        if !self.aboard.is_joined() || self.stance(station) != Stance::Hostile {
            return None;
        }
        self.plunder.iter().find(|p| p.station == station)
    }

    /// Whether crew member `who` stands within [`data::REACH`] of one of
    /// the station's shelves on the joined deck — alive, awake and
    /// aboard. What a plunder asks after the shelf, and what the window
    /// reads to say "walk over first".
    pub fn shelf_ashore_in_reach(&self, who: u32) -> bool {
        if who >= self.aboard.crew_count() {
            return false;
        }
        let room = &self.aboard.room;
        if room.is_unconscious(who as usize) {
            return false;
        }
        self.station_shelves()
            .into_iter()
            .any(|i| room.within_reach(who as usize, Container::Shelf(i), data::REACH))
    }

    /// Where crew member `who` would stand at the nearest of the
    /// station's shelves, in the room's units, for the app to `send_to`.
    /// `None` with none on the deck.
    pub fn plunder_spot(&self, who: u32) -> Option<bims::math::Vec2> {
        if who >= self.aboard.crew_count() {
            return None;
        }
        let room = &self.aboard.room;
        let at = room.bim_pos(who as usize);
        self.station_shelves()
            .into_iter()
            .filter_map(|i| room.container_spot(Container::Shelf(i)))
            .min_by(|a, b| (*a - at).len().total_cmp(&(*b - at).len()))
    }

    /// Lay the shelf of the station alongside out as loot, if the ship is
    /// tied to an enemy's with the rooms joined and it has not been laid
    /// out before. Asked at every join and at every change of stance, so
    /// a dock turned hostile under the crew (`set_hostile`, the `combat`
    /// command) has a shelf to plunder too.
    fn lay_plunder(&mut self) {
        let Some(id) = self.ship.state.station() else {
            return;
        };
        if !self.aboard.is_joined() || self.stance(id) != Stance::Hostile {
            return;
        }
        if self.plunder.iter().any(|p| p.station == id) {
            return;
        }
        if let Some(station) = self.station(id) {
            let laid = plunder::lay_out(station);
            self.plunder.push(laid);
        }
    }

    /// A plunder: a stack off the enemy's shelf into the pack, as far as
    /// the pack takes it. See [`Command::Plunder`] for what is checked,
    /// and in what order.
    fn plunder(&mut self, slot: u32, who: u32, id: u32, events: &mut Vec<WorldEvent>) {
        let Some(station) = self
            .ship
            .state
            .station()
            .filter(|_| self.aboard.is_joined())
        else {
            events.push(refused(slot, Refusal::NotDocked));
            return;
        };
        if self.stance(station) != Stance::Hostile {
            events.push(refused(slot, Refusal::NotHostile));
            return;
        }
        let kept = self
            .plunder
            .iter()
            .find(|p| p.station == station)
            .and_then(|p| p.grid.slot(id))
            .map(|s| s.kept);
        // Nobody's shelf stocks armour or guns, so a slot is a stack or
        // nothing.
        let Some(Kept::Stack(resource)) = kept else {
            events.push(refused(slot, Refusal::NotAboard));
            return;
        };
        if !self.shelf_ashore_in_reach(who) {
            events.push(refused(slot, Refusal::OutOfReach));
            return;
        }
        let item = armour::item_of(resource);
        let room_for = |gear: &bims::combat::Gear| {
            gear.stack_with_room(item)
                .or_else(|| gear.free_cell_for(item))
        };
        if room_for(&self.aboard.room.gear(who as usize)).is_none() {
            events.push(refused(slot, Refusal::PackFull));
            return;
        }
        // One to a cell — or into a box that has room (feature 87) —
        // until the pack is full or the stack is gone; the shelf gives
        // up exactly what the pack took.
        let mut taken = 0;
        while let Some(cell) = room_for(&self.aboard.room.gear(who as usize)) {
            let on_shelf = self
                .plunder
                .iter()
                .find(|p| p.station == station)
                .and_then(|p| p.grid.slot(id))
                .map_or(0, |s| s.count);
            if on_shelf == 0 || !self.aboard.room.give(who as usize, Some(cell), item) {
                break;
            }
            if let Some(p) = self.plunder.iter_mut().find(|p| p.station == station) {
                p.grid.remove(resource, 1, Some(id));
            }
            taken += 1;
        }
        events.push(WorldEvent::Plundered { who, units: taken });
    }

    // --- mercenaries ---------------------------------------------------------

    /// What a month of that resident costs, if it is a mercenary for hire
    /// at the station whose room is open: what the `?` over its head and
    /// the menu on it say. `None` for one of the station's own, for a
    /// hired hand that has gone aboard, or for no such body.
    pub fn mercenary_fee(&self, resident: u32) -> Option<Money> {
        self.residents
            .as_ref()?
            .fee
            .get(resident as usize)
            .copied()
            .flatten()
    }

    /// Everything the Hire window wants to know before the command is
    /// sent, so it can say "walk over first" rather than be refused:
    /// the fee, whether `who` is within reach of the body, whether the
    /// money covers a month, and whether there is a bunk aboard. `None`
    /// for anybody who is not a mercenary for hire. The command checks
    /// it all again when it lands.
    pub fn hire_offer(&self, who: u32, resident: u32) -> Option<Offer> {
        // The fee the crew member doing the hiring would pay: a
        // commander's is cheaper (feature 78), so the window says the
        // discounted price while one is steered.
        let fee = self.hire_fee(who, resident)?;
        Some(Offer {
            fee,
            in_reach: self.in_reach_of_body(who, LootSource::Resident(resident)),
            affordable: self.money >= fee,
            bunk: (self.aboard.crew_count() as usize) < self.aboard.room.bed_count(),
            docked: self.residents.as_ref().map(|r| r.station) == self.ship.state.station(),
            medic: self.mercenary_is_medic(resident),
        })
    }

    /// Whether that resident of the station whose room is open is a
    /// **field medic** for hire (feature 86). False for one of the
    /// station's own, for a plain gun for hire, and for no such body.
    pub fn mercenary_is_medic(&self, resident: u32) -> bool {
        self.residents
            .as_ref()
            .and_then(|r| r.medic.get(resident as usize))
            .copied()
            .unwrap_or(false)
    }

    /// A month of every hired hand, as the world keeps it — for the crew
    /// panel to say who costs what and when it is next due.
    pub fn hired(&self) -> &[Hired] {
        &self.hired
    }

    /// Whether that crew member is a hired hand.
    pub fn is_hired(&self, who: u32) -> bool {
        self.hired.iter().any(|h| h.who == who)
    }

    /// A hire: see [`Command::Hire`] for what is asked. The station's
    /// room gives the body up (`take_crew`, the one taken out, `adopt` the
    /// rest back — every errand ashore is dropped once, the way a docking
    /// drops the crew's) and the crew's room takes it in at the same spot
    /// on the joined deck, in its own coverall still. Its armour becomes
    /// pieces of the world's under fresh ids, the way loot does. The
    /// first month is paid now and the next falls due a month on.
    /// An execution ordered: see [`Command::Execute`] for what is checked.
    /// The room does the walking and the shooting; `visit` carries the
    /// finish to the body's own room when the room says it is done.
    fn execute(&mut self, slot: u32, who: u32, resident: u32, events: &mut Vec<WorldEvent>) {
        let Some(residents) = self.residents.as_ref() else {
            events.push(refused(slot, Refusal::NotDocked));
            return;
        };
        if !self.aboard.is_joined() || self.ship.state.station() != Some(residents.station) {
            events.push(refused(slot, Refusal::NotDocked));
            return;
        }
        if self.stance(residents.station) != Stance::Hostile {
            events.push(refused(slot, Refusal::NotHostile));
            return;
        }
        let body = &residents.aboard.room;
        if resident >= residents.aboard.count()
            || !body.is_alive(resident as usize)
            || !body.is_unconscious(resident as usize)
        {
            events.push(refused(slot, Refusal::NotDown));
            return;
        }
        let room = &self.aboard.room;
        if who >= self.aboard.crew_count()
            || !room.is_alive(who as usize)
            || room.is_unconscious(who as usize)
            || room.is_outside(who as usize)
        {
            events.push(refused(slot, Refusal::NotAboard));
            return;
        }
        if room.weapon(who as usize).is_none() {
            events.push(refused(slot, Refusal::Unarmed));
            return;
        }
        if !self.aboard.room.execute(who as usize, resident as usize) {
            events.push(refused(slot, Refusal::NotDown));
        }
    }

    fn hire(&mut self, slot: u32, who: u32, resident: u32, events: &mut Vec<WorldEvent>) {
        let Some(offer) = self.hire_offer(who, resident) else {
            events.push(refused(slot, Refusal::NotForHire));
            return;
        };
        if !offer.docked || !self.aboard.is_joined() {
            events.push(refused(slot, Refusal::NotDocked));
            return;
        }
        if self.is_down(LootSource::Resident(resident)) {
            events.push(refused(slot, Refusal::NotForHire));
            return;
        }
        if !offer.in_reach {
            events.push(refused(slot, Refusal::OutOfReach));
            return;
        }
        if !offer.bunk {
            events.push(refused(slot, Refusal::NoBunk));
            return;
        }
        // The fee the *sending* slot signs for: a commander's discount
        // is his own, and it is what goes into the contract (feature
        // 78).
        let fee = self.hire_fee(slot, resident).unwrap_or(offer.fee);
        if self.money < fee {
            events.push(refused(slot, Refusal::Unaffordable));
            return;
        }
        let Some(at) = self.body_position(LootSource::Resident(resident)) else {
            events.push(refused(slot, Refusal::NotDocked));
            return;
        };
        let Some(residents) = self.residents.as_mut() else {
            events.push(refused(slot, Refusal::NotDocked));
            return;
        };
        // Out of the station's room — and off its offer for good: the
        // station remembers one fewer for hire ([`World::losses`]).
        let station = residents.station;
        let mut everybody = residents.aboard.room.take_crew();
        let mut body = everybody.remove(resident as usize);
        residents
            .aboard
            .room
            .adopt(everybody, bims::math::Vec2::ZERO);
        residents.aboard.crew = residents.aboard.room.crew_count();
        residents.down.remove(resident as usize);
        residents.xp_down.remove(resident as usize);
        residents.xp_dead.remove(resident as usize);
        residents.last_hit_by.remove(resident as usize);
        residents.fee.remove(resident as usize);
        let hire_is_medic = residents.medic.remove(resident as usize);
        residents.grave.remove(resident as usize);
        memory::amend_losses(&mut self.losses, station, |l| l.mercenaries += 1);
        // Into the crew's, where it stood on the deck, with its armour
        // renumbered as the world's — and its berth left behind, since
        // that was the station's: `adopt` gives it the bunk the offer
        // promised, the first one spare.
        let new_who = self.aboard.crew_count();
        body.bed = None;
        let mut gear = body.gear;
        for part in Part::ALL {
            if let Some(worn) = gear.worn_mut(part) {
                let id = self.next_piece;
                self.next_piece += 1;
                self.pieces.push(Piece {
                    id,
                    kind: worn.kind,
                    tier: worn.tier,
                    health: worn.health,
                    at: Where::Worn { who: new_who },
                });
                worn.id = id;
            }
        }
        body.character.stand_at(at);
        self.aboard.room.adopt(vec![body], bims::math::Vec2::ZERO);
        self.aboard.crew = self.aboard.room.crew_count();
        // `issue` puts the renumbered pieces on and redraws the body.
        self.aboard.room.issue(new_who as usize, gear);
        self.health.push(health::HealthState::new());
        self.crew_down.push(false);
        self.crew_locked.push(false);
        // Crew indices are what a beam links by, so every beam is broken
        // by a hire (feature 76); the hire's own state starts empty.
        self.clear_beams();
        self.clear_carries();
        self.medics
            .resize(self.aboard.crew as usize, Medic::default());
        self.tanks
            .resize(self.aboard.crew as usize, Tank::default());
        // And the crew's indices have moved, so the squad order — whose
        // members are crew indices and whose marks are residents' — is
        // called off (feature 78).
        self.commanders
            .resize(self.aboard.crew as usize, Commander::default());
        self.clear_squad();
        self.ship.crew_count = self.aboard.crew;
        self.money -= fee;
        self.hired.push(Hired {
            who: new_who,
            fee,
            due: self.clock_minutes + mercenary::MONTH,
            owed: false,
            medic: hire_is_medic,
        });
        // A field medic arrives with its own kit (feature 86):
        // `mercenary::MEDIC_MEDKITS` in its pack, the way a medic of the
        // class sets out with `class::MEDIC_START_MEDKITS`. They are the
        // hold's medicine from the moment the room is handed the packs,
        // so nothing else has to know where they came from.
        if hire_is_medic {
            self.give_field_medic_kit(new_who);
        }
        // *Outfitter* (feature 78): the hand arrives wearing the lowest
        // basic piece it was missing, made for it and charged for at
        // nothing.
        if self.has_talent(slot, Talent::Outfitter) {
            self.outfit_the_hire(new_who);
        }
        self.on_ship_changed();
        self.mirror_pieces(events);
        events.push(WorldEvent::Hired { who: new_who });
        // And the commander who signed it learns something by it.
        if self.is_commander(slot) {
            self.award(slot as usize, class::XP_HIRE, events);
        }
    }

    /// *Outfitter*: the lowest basic piece a fresh hire is missing —
    /// helm, then kevlar, then leg guards — made out of nothing and put
    /// on it, a piece of the world's like the tank's own start.
    fn outfit_the_hire(&mut self, who: u32) {
        let missing = [
            bims::combat::ArmourKind::BasicHelm,
            bims::combat::ArmourKind::BasicKevlar,
            bims::combat::ArmourKind::BasicLegs,
        ]
        .into_iter()
        .find(|kind| {
            self.aboard
                .room
                .gear(who as usize)
                .worn(kind.slot())
                .is_none()
        });
        let Some(kind) = missing else {
            return;
        };
        let id = self.next_piece;
        self.next_piece += 1;
        let piece = bims::combat::Piece::new(id, kind, bims::combat::Tier::One);
        self.pieces.push(Piece {
            id,
            kind,
            tier: bims::combat::Tier::One,
            health: piece.health,
            at: Where::Worn { who },
        });
        let mut gear = self.aboard.room.gear(who as usize);
        *gear.worn_mut(kind.slot()) = Some(piece);
        self.aboard.room.issue(who as usize, gear);
    }

    /// The hired hands' months, as they fall due: paid out of the money
    /// while it covers them, and a month it will not cover is owed — said
    /// once — until it can be, or until the ship is at a berth, where the
    /// unpaid hand walks off ([`World::dismiss`]).
    fn pay_wages(&mut self, events: &mut Vec<WorldEvent>) {
        let clock = self.clock_minutes;
        let docked = self.ship.state.station().is_some() && self.aboard.is_joined();
        let mut leaving = Vec::new();
        for i in 0..self.hired.len() {
            let hired = self.hired[i];
            if !mercenary::owed(&hired, clock) {
                continue;
            }
            if self.money >= hired.fee {
                self.money -= hired.fee;
                self.hired[i].due += mercenary::MONTH;
                self.hired[i].owed = false;
                events.push(WorldEvent::MercenaryPaid {
                    who: hired.who,
                    fee: hired.fee,
                });
                continue;
            }
            if !hired.owed {
                self.hired[i].owed = true;
                events.push(WorldEvent::MercenaryLeft { who: hired.who });
            }
            if docked {
                leaving.push(hired.who);
            }
        }
        // Highest first, so the earlier indices are still right.
        leaving.sort_unstable_by(|a, b| b.cmp(a));
        for who in leaving {
            self.dismiss(who, events);
        }
    }

    /// A hired hand off the crew: out of the crew's room — every errand
    /// aboard dropped once, the way a docking drops them — and, at a berth
    /// with the station's room open, into that room as a mercenary for
    /// hire again at the same fee; else gone. Its pieces go with it, and
    /// every crew member and piece after it moves down one.
    fn dismiss(&mut self, who: u32, events: &mut Vec<WorldEvent>) {
        let Some(i) = self.hired.iter().position(|h| h.who == who) else {
            return;
        };
        let hired = self.hired.remove(i);
        let index = who as usize;
        if index >= self.aboard.room.crew_count() as usize {
            return;
        }
        let here = self.aboard.room.bim_pos(index);
        let ashore = self
            .aboard
            .to_station(worldgen::math::dvec2(here.x as f64, here.y as f64));
        let mut everybody = self.aboard.room.take_crew();
        let mut body = everybody.remove(index);
        self.aboard.room.adopt(everybody, bims::math::Vec2::ZERO);
        self.aboard.crew = self.aboard.room.crew_count();
        self.ship.crew_count = self.aboard.crew;
        self.health.remove(index);
        self.crew_down.remove(index);
        self.crew_locked.remove(index);
        // And by a dismissal, which shifts every index after it (feature
        // 76); the dismissed one's own state goes with it.
        self.clear_beams();
        self.clear_carries();
        if index < self.medics.len() {
            self.medics.remove(index);
        }
        if index < self.tanks.len() {
            self.tanks.remove(index);
        }
        if index < self.commanders.len() {
            self.commanders.remove(index);
        }
        // And the squad order with them (feature 78): its members are
        // crew indices.
        self.clear_squad();
        self.pieces.retain(
            |p| !matches!(p.at, Where::Worn { who: w } | Where::Pack { who: w, .. } if w == who),
        );
        for piece in &mut self.pieces {
            match &mut piece.at {
                Where::Worn { who: w } | Where::Pack { who: w, .. } if *w > who => *w -= 1,
                _ => {}
            }
        }
        for h in &mut self.hired {
            if h.who > who {
                h.who -= 1;
            }
        }
        // Ashore, if there is an ashore to go to and a bunk in it.
        if let (Some(at), Some(residents)) = (ashore, self.residents.as_mut())
            && Some(residents.station) == self.ship.state.station()
            && (residents.aboard.room.crew_count() as usize) < residents.aboard.room.bed_count()
        {
            body.character
                .stand_at(bims::math::vec2(at.x as f32, at.y as f32));
            residents
                .aboard
                .room
                .adopt(vec![body], bims::math::Vec2::ZERO);
            residents.aboard.crew = residents.aboard.room.crew_count();
            residents.down.push(false);
            residents.xp_down.push(false);
            residents.xp_dead.push(false);
            residents.last_hit_by.push(None);
            residents.fee.push(Some(hired.fee));
            residents.medic.push(hired.medic);
            residents.grave.push(false);
            // Back on the station's offer, if it was hired off it.
            let station = residents.station;
            memory::amend_losses(&mut self.losses, station, |l| {
                l.mercenaries = l.mercenaries.saturating_sub(1)
            });
        }
        self.on_ship_changed();
        self.mirror_pieces(events);
    }

    // --- research -------------------------------------------------------------

    /// Which of the research desks on the deck is the station's, while the
    /// rooms are joined: the one standing in the station's box. `None` on
    /// a ship of its own, or at a station without one.
    pub fn station_desk(&self) -> Option<usize> {
        let (lo, hi) = self.aboard.station_box?;
        self.aboard
            .room
            .research_desks()
            .iter()
            .position(|(frame, _)| {
                let m = frame.center();
                (m.x as f64) >= lo.x
                    && (m.x as f64) <= hi.x
                    && (m.y as f64) >= lo.y
                    && (m.y as f64) <= hi.y
            })
    }

    /// Which tier of research key a station's desk still has on it, nought
    /// for none.
    pub fn station_key(&self, station: u32) -> u8 {
        self.stations
            .iter()
            .zip(&self.station_keys)
            .find(|(s, _)| s.id == station)
            .map_or(0, |(_, &key)| key)
    }

    /// Whether a station's research desk still has a key on it, of
    /// either tier.
    pub fn station_has_key(&self, station: u32) -> bool {
        self.station_key(station) > 0
    }

    /// Which tier of key the station the ship is tied to has on its desk,
    /// nought for none — or away from a berth.
    pub fn key_at_the_dock(&self) -> u8 {
        match self.ship.state {
            ShipState::Docked { station } => self.station_key(station),
            _ => 0,
        }
    }

    /// Where a crew member stands at the station's research desk, in the
    /// room's units, for the app to `send_to`. `None` with no such desk on
    /// the deck.
    pub fn key_desk_spot(&self) -> Option<bims::math::Vec2> {
        self.aboard.room.research_spot(self.station_desk()?)
    }

    /// Whether crew member `who` stands within [`data::REACH`] of the
    /// station's research desk, alive and awake — what a `TakeKey` wants,
    /// so the row can say "walk over first" before it is refused.
    pub fn key_in_reach(&self, who: u32) -> bool {
        let Some(desk) = self.station_desk() else {
            return false;
        };
        let room = &self.aboard.room;
        who < self.aboard.crew_count()
            && !room.is_unconscious(who as usize)
            && room.within_reach(who as usize, Container::Desk(desk), data::REACH)
    }

    /// Whether the ship has a research desk of its own, and whether that
    /// desk is running: the AI thinks on its power.
    pub fn research_desk_aboard(&self) -> bool {
        self.ship.design.count(PartKind::ResearchDesk) > 0
    }

    pub fn research_desk_powered(&self) -> bool {
        self.research_desk_aboard() && self.powered(PartKind::ResearchDesk)
    }

    /// How many research keys of `tier` are in the crew's own desk and
    /// not spoken for. What an `Unlock` of a node of that tier consumes
    /// one of; nought for a tier no key exists for.
    pub fn keys_in_desk(&self, tier: u8) -> u32 {
        armour::key_resource(tier).map_or(0, |r| self.free(r))
    }

    /// A take of the station's key. See [`Command::TakeKey`] for what is
    /// checked, in this order: docked with a desk on the deck, a key on
    /// it, the Bim in reach, and room in the pack.
    fn take_key(&mut self, slot: u32, who: u32, events: &mut Vec<WorldEvent>) {
        let ShipState::Docked { station } = self.ship.state else {
            events.push(refused(slot, Refusal::NotDocked));
            return;
        };
        let Some(at) = self.stations.iter().position(|s| s.id == station) else {
            events.push(refused(slot, Refusal::NotDocked));
            return;
        };
        let tier = self.station_keys[at];
        if tier == 0 || self.station_desk().is_none() {
            events.push(refused(slot, Refusal::NoKey));
            return;
        }
        if !self.key_in_reach(who) {
            events.push(refused(slot, Refusal::OutOfReach));
            return;
        }
        let key = Item::Key(tier);
        let Some(cell) = self.aboard.room.gear(who as usize).free_cell_for(key) else {
            events.push(refused(slot, Refusal::PackFull));
            return;
        };
        if !self.aboard.room.give(who as usize, Some(cell), key) {
            events.push(refused(slot, Refusal::PackFull));
            return;
        }
        self.station_keys[at] = 0;
        events.push(WorldEvent::KeyTaken { who });
    }

    /// An unlock: a key of the node's own tier out of the crew's desk,
    /// gone, and the node's lock open. See [`Command::Unlock`] for what is
    /// checked: a desk holding only the other tier's key is `NoKey`, and
    /// that key stays.
    fn unlock(&mut self, slot: u32, node: u32, events: &mut Vec<WorldEvent>) {
        if !self.research_desk_powered() {
            events.push(refused(slot, Refusal::NoResearchDesk));
            return;
        }
        let Some(node_of) = ResearchNode::from_code(node) else {
            events.push(refused(slot, Refusal::NotResearchable));
            return;
        };
        // A node with no lock wants no key, and is refused before the
        // desk is looked at; a locked one wants its own tier's.
        let key = Research::key_wanted(node_of)
            .and_then(|tier| u8::try_from(tier).ok())
            .and_then(armour::key_resource);
        let Some(key) = key else {
            events.push(refused(slot, Refusal::NotResearchable));
            return;
        };
        if self.free(key) == 0 {
            events.push(refused(slot, Refusal::NoKey));
            return;
        }
        if !self.research.unlock(node_of) {
            events.push(refused(slot, Refusal::NotResearchable));
            return;
        }
        self.ship.design.cargo[key as usize] -= 1;
        self.on_ship_changed();
        events.push(WorldEvent::Unlocked { node });
    }

    /// Queue a node for the AI. See [`Command::Research`]: a
    /// `ResearchQueued` for it and for each prerequisite that went in
    /// ahead of it; the AI takes the head in `run_research` this step.
    fn research(&mut self, slot: u32, node: u32, events: &mut Vec<WorldEvent>) {
        if !self.research_desk_aboard() {
            events.push(refused(slot, Refusal::NoResearchDesk));
            return;
        }
        let before = self.research.queue.len();
        let queued = ResearchNode::from_code(node).is_some_and(|n| self.research.enqueue(n));
        if !queued {
            events.push(refused(slot, Refusal::NotResearchable));
            return;
        }
        for node in &self.research.queue[before..] {
            events.push(WorldEvent::ResearchQueued { node: node.code() });
        }
    }

    /// Take the AI off what it is on, and off the queue what needed it.
    /// See [`Command::CancelResearch`].
    fn cancel_research(&mut self, events: &mut Vec<WorldEvent>) {
        for node in self.research.cancel() {
            events.push(WorldEvent::ResearchDropped { node: node.code() });
        }
    }

    /// Take a node off the queue, and what needed it with it. See
    /// [`Command::Dequeue`].
    fn dequeue(&mut self, slot: u32, node: u32, events: &mut Vec<WorldEvent>) {
        let dropped = ResearchNode::from_code(node)
            .map(|n| self.research.dequeue(n))
            .unwrap_or_default();
        if dropped.is_empty() {
            events.push(refused(slot, Refusal::NotQueued));
            return;
        }
        for node in dropped {
            events.push(WorldEvent::ResearchDropped { node: node.code() });
        }
    }

    /// The AI's step, while a research desk aboard is running: onto the
    /// head of the queue if it is idle, a step's minutes onto whatever it
    /// is on, and the word when a node is done — and onto the next queued
    /// node the same step, so a queue is worked through without an idle
    /// step between. Stage 6's second half — it runs on the desk's power.
    fn run_research(&mut self, events: &mut Vec<WorldEvent>) {
        if !self.research_desk_powered() {
            return;
        }
        if let Some(node) = self.research.next() {
            events.push(WorldEvent::ResearchBegun { node: node.code() });
        }
        if let Some(node) = self.research.advance(data::STEP_MINUTES) {
            events.push(WorldEvent::Researched { node: node.code() });
            if let Some(next) = self.research.next() {
                events.push(WorldEvent::ResearchBegun { node: next.code() });
            }
        }
    }

    /// Have the crew know `node` and everything it needs, without the
    /// AI's time. For probes of what research gates.
    pub fn research_for_probe(&mut self, node: ResearchNode) {
        for r in node.def().requires {
            self.research_for_probe(*r);
        }
        self.research.done[node as usize] = true;
    }

    /// Have the crew know the whole tree. For probes of the benches, which
    /// predate research and want every recipe on offer.
    pub fn know_everything_for_probe(&mut self) {
        for node in ResearchNode::ALL {
            self.research.done[node as usize] = true;
        }
    }

    // --- looking out of the window ------------------------------------------

    /// How far the crew can see: their own eyes, or the sensor arrays, and
    /// whichever of those reaches further.
    pub fn detection_range(&self) -> f64 {
        let arrays = self.ship.design.count(PartKind::SensorArray) as f64;
        let radar = (data::RADAR_RANGE_PER_SENSOR * arrays).min(data::RADAR_RANGE_MAX);
        data::VISION_RANGE.max(radar)
    }

    /// Everything that came within range of the stretch just travelled.
    ///
    /// Tested against the **segment**, not against the endpoints. At 24x a
    /// step can be a long way, and a station passed in the middle of one would
    /// otherwise be missed entirely — the ship would fly straight through
    /// somewhere and nobody would have seen it.
    fn discover_along(&mut self, from: DVec2, to: DVec2, events: &mut Vec<WorldEvent>) {
        let range = self.detection_range();
        let mut found = Vec::new();
        for node in self.system.nodes() {
            if self.discovered.contains(&node) {
                continue;
            }
            let Some(at) = self.system.absolute_position(node) else {
                continue;
            };
            if segment_distance(from, to, at) <= range {
                found.push(node);
            }
        }
        if found.is_empty() {
            return;
        }
        for node in found {
            self.discovered.push(node);
            events.push(WorldEvent::Discovered { node });
        }
        // Sorted rather than in the order they happened to be seen: the
        // checksum runs over this, and two clients that found the same two
        // things in one step must not disagree about the list.
        self.discovered.sort_by_key(node_key);
    }

    /// Which node the view is about, if it is about one at all.
    fn frame_candidate(&self) -> Option<(Node, f64, f64)> {
        let node = match &self.ship.state {
            // Tied to a raider, or pushing off one, the view is about
            // whatever it was about when the raid came: a raider is not a
            // place in the system.
            ShipState::Docked { station }
            | ShipState::CastingOff { station, .. }
            | ShipState::Undocking { station, .. }
                if raid::raider_index(*station).is_some() =>
            {
                match self.ship.frame {
                    Frame::Local(node) if self.within_exit_radius(node) => node,
                    _ => self.nearest_discovered()?,
                }
            }
            // At a settlement, the view is about the planet it is on.
            ShipState::Docked { station }
            | ShipState::CastingOff { station, .. }
            | ShipState::Undocking { station, .. }
            | ShipState::Docking { station, .. } => match surface::surface_body(*station) {
                Some(body) => Node::Body(body),
                None => Node::Station(*station),
            },
            // Holding is being *there for* whatever the ship was last
            // there for: a ship that flew to a planet and stopped beside
            // it stays in the planet's frame although the station in its
            // orbit is nearer than the planet's centre — a station orbits
            // inside the arrival radius, so it is nearer from most sides —
            // until it goes out past the exit radius. With no frame to
            // keep, the nearest.
            ShipState::Holding | ShipState::Charging { .. } => match self.ship.frame {
                Frame::Local(node) if self.within_exit_radius(node) => node,
                _ => self.nearest_discovered()?,
            },
            ShipState::Travelling { plan, departed } => {
                // The last leg of a trip aimed at somewhere, and only that.
                // Passing something on the way is not arriving at it.
                let phase = flight::state_at(plan, self.clock_minutes - departed).phase;
                if phase != Phase::Brake {
                    return None;
                }
                plan.target.node()?
            }
        };
        let at = self.system.absolute_position(node)?;
        Some((
            node,
            at.distance(self.ship.position()),
            data::local_radius(node),
        ))
    }

    /// Whether the ship is still inside a node's frame by the wider,
    /// leaving radius — `frame::settle`'s hysteresis, asked ahead of it.
    fn within_exit_radius(&self, node: Node) -> bool {
        self.system.absolute_position(node).is_some_and(|at| {
            at.distance(self.ship.position()) <= data::local_radius(node) * data::LOCAL_HYSTERESIS
        })
    }

    fn nearest_discovered(&self) -> Option<Node> {
        let here = self.ship.position();
        self.discovered
            .iter()
            .copied()
            .filter_map(|node| self.system.absolute_position(node).map(|at| (node, at)))
            .min_by(|a, b| {
                let (da, db) = (a.1.distance(here), b.1.distance(here));
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(node, _)| node)
    }

    fn settle_frame(&mut self, events: &mut Vec<WorldEvent>) {
        let now = frame::settle(self.ship.frame, self.frame_candidate());
        if now != self.ship.frame {
            self.ship.frame = now;
            events.push(WorldEvent::FrameChanged { frame: now });
        }
        self.mark_visited();
    }

    /// Where the ship is now, marked on the chart as somewhere the crew
    /// have been (feature 85). The frame is what says where it is —
    /// docked at a station, set down at a settlement, holding beside a
    /// planet or at a belt — and the one thing added here is that it must
    /// have **stopped**: a trip is in its target's frame from the moment
    /// it starts braking, and a trip aborted half a braking phase away is
    /// not somewhere anybody has been.
    fn mark_visited(&mut self) {
        if matches!(self.ship.state, ShipState::Travelling { .. }) {
            return;
        }
        let Some(node) = self.ship.frame.node() else {
            return;
        };
        if self.visited.contains(&node) {
            return;
        }
        self.visited.push(node);
        // Sorted, for the reason `discover_along` sorts the chart: the
        // checksum runs over this, and two clients must not disagree
        // about the order.
        self.visited.sort_by_key(node_key);
    }

    /// Every star the crew have been to, sorted — the galaxy's own
    /// half of "where have I been" (feature 85). A star the ship has
    /// jumped out of has a memory filed under it
    /// ([`World::remember_system`]), and the one it is at now is the
    /// star it is at: those two together are the whole of it, so there
    /// is no third list to keep in step.
    pub fn stars_visited(&self) -> Vec<u32> {
        let mut stars: Vec<u32> = self.memories.iter().map(|m| m.star).collect();
        if let Err(i) = stars.binary_search(&self.star_id) {
            stars.insert(i, self.star_id);
        }
        stars
    }

    // --- readouts -----------------------------------------------------------

    /// How many players there are: a speed request each, kept from
    /// `start_with_crew`. What a save reads back to rebuild the session.
    pub fn players(&self) -> u32 {
        self.speed_requests.len() as u32
    }

    /// What the world is actually running at: the slowest request there is.
    pub fn effective_speed(&self) -> Speed {
        speed::effective(&self.speed_requests)
    }

    /// Record what a player wants the world to run at.
    ///
    /// **The one control that does not wait for a step**, and it has to be:
    /// at a pause no steps are taken at all, so a queued speed change would
    /// never be applied and a pause could never be lifted. It is safe to be
    /// the exception because it changes nothing a step would have changed —
    /// not the clock, not the ship, not what anybody is carrying, only how
    /// fast the caller is expected to turn the crank.
    ///
    /// [`Command::SetSpeed`] goes through here too, so there is one
    /// implementation and two ways in rather than two implementations.
    pub fn request_speed(&mut self, slot: u32, speed: Speed) {
        if let Some(request) = self.speed_requests.get_mut(slot as usize) {
            *request = speed;
        }
    }

    /// Whether a command is applied the moment it is sent rather than at
    /// the top of the next step: the speed, for the reason above, and an
    /// order to the crew's room (`Command::Crew`, `ToHelm`, `ToDesk`) —
    /// a click on the deck at a pause is a click that shows, and a walk
    /// begun between two steps is what the room always did. Deterministic
    /// all the same, since every copy applies the same commands in the
    /// same order (`crates/app/src/net.rs`); what it changes is *when*
    /// between two steps, and both ends agree about that too.
    pub fn applies_at_once(command: &Command) -> bool {
        matches!(
            command,
            Command::SetSpeed { .. }
                | Command::Crew { .. }
                | Command::ToHelm { .. }
                | Command::ToDesk { .. }
        )
    }

    /// Apply one command now, between steps — for the ones
    /// [`World::applies_at_once`] says so of. The events it raised.
    pub fn apply_now(&mut self, command: Command) -> Vec<WorldEvent> {
        let mut events = Vec::new();
        self.apply(command, &mut events);
        events
    }

    /// How many steps a second of real time is worth at 1x.
    ///
    /// The page needs it to turn a frame into steps, and it is a fact about
    /// the world rather than about the browser — a 60 written down in
    /// `crates/app/src/screens/game.rs` would be a second copy of [`data::STEP_MINUTES`] waiting
    /// to disagree with the first.
    pub fn steps_per_second(&self) -> f64 {
        time::MINUTES_PER_SECOND / data::STEP_MINUTES
    }

    /// The day count, and the minutes into it — the **crew's** clock, read
    /// through the room. `clock_minutes` is elapsed time since the world
    /// opened; the room's clock opened at the waking hour and has run in
    /// step with it since (`the_crew_keep_the_world_s_clock`), and the
    /// room's is what the schedule strip, the light and the Bims' day go
    /// by, so it is the one the player is shown. Read as `clock_minutes %
    /// DAY` this was eight hours behind the schedule's "now". The host
    /// formats; no strings cross the boundary.
    pub fn day(&self) -> u32 {
        self.aboard.room.clock_day()
    }

    pub fn minutes_into_day(&self) -> f64 {
        self.aboard.minutes() % time::DAY
    }

    /// Where the trip has got to, for a caller that wants to draw it.
    pub fn trip_state(&self) -> Option<flight::State> {
        match &self.ship.state {
            ShipState::Travelling { plan, departed } => {
                Some(flight::state_at(plan, self.clock_minutes - departed))
            }
            _ => None,
        }
    }

    /// What the ship is doing to itself — the engines lit and the thrusters
    /// pushing — for a caller that wants to draw the exhaust. Nothing while
    /// docked or holding.
    pub fn effort(&self) -> flight::Effort {
        match &self.ship.state {
            ShipState::Travelling { plan, departed } => {
                flight::effort_at(plan, self.clock_minutes - departed)
            }
            _ => flight::Effort::NONE,
        }
    }

    pub fn plan(&self) -> Option<&Plan> {
        match &self.ship.state {
            ShipState::Travelling { plan, .. } => Some(plan),
            _ => None,
        }
    }

    /// How far through the trip the ship is, for a caller that wants to
    /// draw a bar: minutes flown and minutes the whole trip takes, the
    /// first never past the second. `None` when not `Travelling` — the
    /// push-off and the docking are their own states with their own
    /// lengths, and a stop is a trip too.
    pub fn trip_progress(&self) -> Option<(f64, f64)> {
        match &self.ship.state {
            ShipState::Travelling { plan, departed } => {
                let total = plan.duration();
                Some(((self.clock_minutes - departed).clamp(0.0, total), total))
            }
            _ => None,
        }
    }

    /// A number two clients can compare to find out whether they have drifted
    /// apart. See [`crate::checksum`] for what goes into it and why the floats
    /// are rounded on the way.
    pub fn checksum(&self) -> u64 {
        crate::checksum::world_checksum(self)
    }

    /// The design's identity, for a caller that wants to show it.
    pub fn design_hash(&self) -> u64 {
        design_hash(&self.ship.design)
    }

    // --- seams for probes ---------------------------------------------------

    /// Run the discovery pass over a stretch the ship did not actually fly.
    ///
    /// For probes. The alternative is a test that has to arrange a real trip
    /// past a body whose position is whatever the seed happened to put it at,
    /// which would be a test of the generator dressed up as a test of
    /// discovery. Same reason the room has `put_for_probe`.
    pub fn discover_for_probe(&mut self, from: DVec2, to: DVec2) -> Vec<WorldEvent> {
        let mut events = Vec::new();
        self.discover_along(from, to, &mut events);
        events
    }

    /// Put the ship's centre of mass somewhere, without flying it there.
    ///
    /// For probes, and for one thing in particular: a ship that is holding
    /// station does not move at all, so there is no other way to walk it
    /// across a local frame's boundary and back.
    pub fn put_for_probe(&mut self, at: DVec2) {
        self.ship.set_position(at);
    }

    /// Let go of the dock without flying anywhere: holding, with the ship's
    /// room its own again, and in no frame until the next settle — a
    /// holding ship keeps the frame it is in, and a probe that puts the
    /// ship somewhere else next wants it to start from nowhere. For probes
    /// of what happens away from a station.
    /// The room laid out again on the ship as it is — a relayout with no
    /// part built, which is what a relayout is to the sandbags laid on it
    /// (feature 74). For the tests.
    pub fn relayout_room_for_probe(&mut self) {
        self.relayout_room();
    }

    /// The ship put at a station's berth from a hold — `dock_at`, the
    /// state with it — for the tests: what a docking ends in, without
    /// the flight.
    pub fn dock_at_for_probe(&mut self, station: u32) {
        self.ship.state = ShipState::Docked { station };
        self.ship.frame = Frame::Local(Node::Station(station));
        self.dock_at(station);
        self.settle_residents();
    }

    pub fn undock_for_probe(&mut self) {
        self.ship.state = ShipState::Holding;
        self.ship.frame = Frame::Space;
        self.unjoin_rooms();
    }

    /// The ship set down on the first planet with ground in the system,
    /// without the descent: off its berth, tied up at the settlement with
    /// the rooms joined, the view about the planet. How the ground is
    /// looked at without flying there — `BIMS_LANDED=1` in the app. False
    /// in a system with nowhere to land.
    pub fn land_for_probe(&mut self) -> bool {
        let Some(body) = self.surfaces.first().map(|s| s.body) else {
            return false;
        };
        let id = surface::surface_id(body);
        self.undock_for_probe();
        self.residents = None;
        self.ship.state = ShipState::Docked { station: id };
        self.dock_at(id);
        self.settle_frame_for_probe();
        true
    }

    /// The ship holding straight over the first planet with ground, the
    /// first crew member at the helm and the landing begun and run `done`
    /// of the way — nought to one — so the descent can be looked at at any
    /// point of it: the planet growing under the ship, the black at the
    /// end. `BIMS_LANDING` in the app. False with nowhere to land.
    pub fn landing_for_probe(&mut self, done: f64) -> bool {
        let Some(body) = self.surfaces.first().map(|s| s.body) else {
            return false;
        };
        let Some(over) = self.above(body) else {
            return false;
        };
        self.undock_for_probe();
        self.residents = None;
        self.put_for_probe(over);
        // There for the planet, whatever stands nearer: a ship that flew
        // there would be in its frame from the last leg of the trip.
        self.ship.frame = Frame::Local(Node::Body(body));
        self.settle_frame_for_probe();
        self.man_the_helm_for_probe(0);
        let events = self.step(&[Command::Land { slot: 0 }]);
        if !events
            .iter()
            .any(|e| matches!(e, WorldEvent::Landing { .. }))
        {
            return false;
        }
        // And on into it, as far as asked: `done` of the way down.
        while self.landing().is_some_and(|(_, so_far)| so_far < done) {
            self.step(&[]);
        }
        true
    }

    /// Shoot the `n` lamps nearest the first crew member out, and leave
    /// the next one failing — each hit as a bolt would land it, so the
    /// world remembers them and the residents' room follows on the next
    /// step. How a lamp out and a lamp failing are looked at without a
    /// fight that happens to hit one: `BIMS_LAMPS_OUT` in the app.
    pub fn shoot_lamps_for_probe(&mut self, n: usize) {
        if self.aboard.count() == 0 {
            return;
        }
        let at = self.aboard.room.bim_pos(0);
        let mut near: Vec<(f32, usize)> = self
            .aboard
            .room
            .lamps()
            .iter()
            .enumerate()
            .map(|(i, l)| ((l.at - at).len(), i))
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (k, &(_, i)) in near.iter().take(n + 1).enumerate() {
            let damage = if k < n {
                bims::sight::LAMP_HEALTH
            } else {
                bims::sight::LAMP_HEALTH * (1.0 - bims::sight::LAMP_FAILING * 0.5)
            };
            self.aboard.room.damage_lamp_for_probe(i, damage);
        }
    }

    /// Tie up at a station without flying there: docked, the rooms joined.
    /// For probes of what a dock builds afresh.
    pub fn dock_for_probe(&mut self, id: u32) {
        self.ship.state = ShipState::Docked { station: id };
        self.dock_at(id);
    }

    /// Pretend the reactors make `supply` a minute, through every change
    /// to the ship, until told otherwise. For probes of a short network:
    /// with a basic reactor making two and a half thousand, nothing a
    /// twenty-tile ship can carry draws more than it makes, and the
    /// brownout is otherwise a long way off.
    pub fn throttle_reactors_for_probe(&mut self, supply: f64) {
        self.probe_supply = Some(supply);
        self.power_budget.supply = supply;
    }

    /// Settle the local frame, for a probe that has just moved the ship.
    pub fn settle_frame_for_probe(&mut self) -> Vec<WorldEvent> {
        let mut events = Vec::new();
        self.settle_frame(&mut events);
        events
    }

    /// Stage a fight: the station the ship is tied to made hostile, the
    /// first crew member recruited and stood just inside its port, and the
    /// first of its people stood a few tiles down the corridor from them —
    /// the state a fight is looked at in without walking the station for
    /// one. Where the resident is stood is only where it starts: from the
    /// next step it is at war, and walks to wherever its tactics say it
    /// should shoot from — and it shoots the moment it can, so it is given
    /// the **pistol** for the picture whatever the station issued it: a
    /// shotgun four tiles off kills the crew member with its first shot,
    /// which is a fight nobody gets to look at (`BIMS_ENEMY_WEAPON` in the
    /// app puts something else in its hand after this). `false`, and
    /// nothing moved, away from a berth or at a station nobody lives on.
    /// For probes and for `BIMS_FIGHT` in the app.
    pub fn stage_fight_for_probe(&mut self) -> bool {
        let Some(station) = self.ship.state.station() else {
            return false;
        };
        let Some(ashore) = self.aboard.ashore else {
            return false;
        };
        let Some(port) = self.station(station).and_then(|s| s.port()) else {
            return false;
        };
        if !self
            .residents
            .as_ref()
            .is_some_and(|r| r.aboard.count() > 0)
        {
            return false;
        }
        self.set_hostile(station, true);
        // The resident: four tiles further in than the crew member, along
        // the corridor the port opens onto, in the station's own frame.
        let reach = (data::ASHORE_TILES + 4.0) * shipdesign::TILE as f64;
        let there = bims::math::vec2(
            (port.centre.0 - port.outward.0 as f64 * reach) as f32,
            (port.centre.1 - port.outward.1 as f64 * reach) as f32,
        );
        if let Some(residents) = &mut self.residents {
            let there = residents
                .aboard
                .to_room(dvec2(there.x as f64, there.y as f64));
            residents.aboard.room.put_for_probe(0, there);
            let mut gear = residents.aboard.room.gear(0);
            // At the tier the hand had, so `outfit_for_probe`'s kit stays
            // a set.
            let tier = gear.weapon.map_or(Tier::One, |w| w.tier);
            gear.weapon = Some(WeaponKind::LaserPistol.at(tier));
            residents.aboard.room.issue(0, gear);
        }
        // The crew member: just inside the station's door, under orders.
        self.aboard
            .room
            .put_for_probe(0, bims::math::vec2(ashore.x as f32, ashore.y as f32));
        self.aboard.room.recruit_for_probe(0, true);
        true
    }

    /// Everybody's kit at one tier: every crew member's weapon at `tier`
    /// (its kind kept, the pistol for an empty hand) and a fresh helm,
    /// kevlar and leg guards at it over whatever was worn — pieces of the
    /// world's, ids off `next_piece` and `Where::Worn`, so the checksum,
    /// the health bars and a loot see them like any other; and the same
    /// for every one of the station's people in the open room, those
    /// pieces the room's own (numbered from a thousand a head past the
    /// mercenaries' kits) until a loot takes one over. The `tier2_test`
    /// and `tier3_test` commands: the fight with nothing at tier one on
    /// either side. Whatever was worn before is dropped, not stowed. For
    /// probes and for the app.
    pub fn outfit_for_probe(&mut self, tier: Tier) {
        let armed = |gear: &bims::combat::Gear| {
            Some(
                gear.weapon
                    .map_or(WeaponKind::LaserPistol, |w| w.kind)
                    .at(tier),
            )
        };
        for who in 0..self.aboard.crew_count() as usize {
            let mut gear = self.aboard.room.gear(who);
            gear.weapon = armed(&gear);
            for kind in ArmourKind::ALL {
                let id = self.next_piece;
                self.next_piece += 1;
                self.pieces.push(Piece {
                    at: Where::Worn { who: who as u32 },
                    ..Piece::new(id, kind, tier)
                });
                *gear.worn_mut(kind.slot()) = Some(bims::combat::Piece::new(id, kind, tier));
            }
            self.aboard.room.issue(who, gear);
        }
        if let Some(residents) = &mut self.residents {
            let room = &mut residents.aboard.room;
            for who in 0..room.crew_count() as usize {
                let mut gear = room.gear(who);
                gear.weapon = armed(&gear);
                let first = 1_000 * (who as u32 + 1) + 100;
                for (n, kind) in ArmourKind::ALL.into_iter().enumerate() {
                    let piece = bims::combat::Piece::new(first + n as u32, kind, tier);
                    *gear.worn_mut(kind.slot()) = Some(piece);
                }
                room.issue(who, gear);
            }
        }
    }

    /// Rebuild the station the ship is tied to as the arena —
    /// [`crate::station::arena`], the same kind and seed laid out bigger,
    /// with bunks for a whole garrison — standing where it stood, and dock
    /// there again, with a garrison of [`data::ARENA_GARRISON`] whenever it
    /// is hostile, whatever the crew's number and worth would have put up
    /// — `reinforcements` is set to the difference: the `combat`
    /// command's dock, for a fight with room to move and a crowd to
    /// fight. The rooms are laid out afresh on the new deck, so the crew
    /// start at their bunks again. `false`, and nothing moved, away from
    /// a berth. For probes and for the app.
    pub fn arena_dock_for_probe(&mut self) -> bool {
        let Some(id) = self.ship.state.station() else {
            return false;
        };
        let Some(i) = self.stations.iter().position(|s| s.id == id) else {
            return false;
        };
        let centre = self.stations[i].centre();
        let station = &mut self.stations[i];
        station.design = crate::station::arena(station.kind, station.map_seed);
        let half = station.design.build_area as f64 * shipdesign::TILE as f64 / 2.0;
        station.anchor = centre.sub(angle::rotate_design(worldgen::math::dvec2(half, half), 0.0));
        self.reinforcements = data::ARENA_GARRISON.saturating_sub(enemies_of(
            self.aboard.crew_count(),
            self.worth(),
            self.start_worth,
            self.days_gone(),
        ));
        // Docked again from the start: the berth moved with the hull, the
        // joined deck is the new one, and the residents' room — opened on
        // the old design — is opened again on this.
        self.undock_for_probe();
        self.residents = None;
        self.ship.state = ShipState::Docked { station: id };
        self.dock_at(id);
        true
    }

    /// `n` of the station alongside dead where they stand, and its room
    /// built again over them (feature 85): what a fight the crew walked
    /// away from leaves behind, without the fight. The bodies are the
    /// first `n` of its people as they are standing this instant — their
    /// spot, their kit and their face — filed as [`World::graves`] and
    /// counted as losses, so the room that opens has that many fewer on
    /// their feet and that many bodies on the deck. `false`, and nothing
    /// moved, away from a berth or where nobody lives. For probes and
    /// for `BIMS_GRAVES` in the app.
    pub fn lay_graves_for_probe(&mut self, n: u32) -> bool {
        let Some(id) = self.ship.state.station() else {
            return false;
        };
        let Some(station) = self.station(id).cloned() else {
            return false;
        };
        // The room closed first, since closing is what files the graves
        // and it would file over these.
        let Some(residents) = self.close_residents() else {
            return false;
        };
        let people = residents.aboard.room.crew_count().min(n);
        if people == 0 {
            self.residents = Some(residents);
            return false;
        }
        let laid: Vec<Grave> = (0..people)
            .map(|who| {
                let at = residents.aboard.position(who);
                Grave {
                    station: id,
                    x: at.x,
                    y: at.y,
                    gear: residents.aboard.room.gear(who as usize),
                    look: residents.aboard.room.look(who as usize),
                    hired: matches!(residents.fee.get(who as usize), Some(Some(_))),
                }
            })
            .collect();
        let (own, hired) = laid.iter().fold((0, 0), |(own, hired), g| match g.hired {
            true => (own, hired + 1),
            false => (own + 1, hired),
        });
        memory::amend_losses(&mut self.losses, id, |l| {
            l.dead += own;
            l.mercenaries += hired;
        });
        memory::set_graves(&mut self.graves, id, laid);
        let (count, mercs) = (self.people_of(&station), self.mercenaries_of(&station));
        let mut fresh = self.open_residents(id, &station.design, count, mercs, station.map_seed);
        if self.aboard.is_joined() && self.ship.state.station() == Some(id) {
            fresh.aboard.room.set_doors_drawn(false);
            fresh.aboard.room.set_fog(bims::sight::Fog::None);
            if let (Some(s), Some(berth)) = (self.station(id).cloned(), self.berth_at(id)) {
                fresh.join(
                    &self.ship.design,
                    self.ship.dynamics.centre_of_mass,
                    &s,
                    &berth,
                    self.clock_minutes,
                );
            }
        }
        self.residents = Some(fresh);
        self.apply_stances();
        true
    }

    /// A mercenary for hire at the dock whatever the roll said: the
    /// station's room opened again with [`data::TEST_MERCENARY`] of them
    /// at the least, and every friendly station from here on the same.
    /// `false`, and nothing moved, away from a berth, at an enemy's, or
    /// on a derelict. For probes and for the `test` command.
    pub fn mercenary_for_probe(&mut self) -> bool {
        let Some(id) = self.ship.state.station() else {
            return false;
        };
        let Some(station) = self.station(id).cloned() else {
            return false;
        };
        self.least_mercenaries = data::TEST_MERCENARY;
        if self.mercenaries_of(&station) == 0 {
            self.least_mercenaries = 0;
            return false;
        }
        // Opened again with the mercenary in it, looked into as
        // `join_rooms` leaves a docked station's room.
        let (count, mercs) = (self.people_of(&station), self.mercenaries_of(&station));
        let mut residents =
            self.open_residents(id, &station.design, count, mercs, station.map_seed);
        if self.aboard.is_joined() {
            residents.aboard.room.set_doors_drawn(false);
            residents.aboard.room.set_fog(bims::sight::Fog::None);
        }
        self.residents = Some(residents);
        self.apply_stances();
        true
    }

    /// Let go of the dock and hold at the first belt of this system, its
    /// site laid out — the state a walk outside is looked at in. `false`,
    /// and nothing moved, when the system has no belt. For probes and for
    /// `BIMS_AT_BELT` in the app.
    pub fn hold_at_belt_for_probe(&mut self) -> bool {
        let Some(belt) = self
            .system
            .bodies
            .iter()
            .find(|b| b.kind == worldgen::BodyKind::AsteroidBelt)
            .cloned()
        else {
            return false;
        };
        self.undock_for_probe();
        self.put_for_probe(belt.position);
        self.settle_frame_for_probe();
        self.settle_site();
        true
    }
}

/// The ship's power at this instant: units a minute in and out, and what
/// the batteries hold and have.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Power {
    pub supply: f64,
    /// The day-long draw: every wired consumer, engines aside.
    pub draw: f64,
    /// What the engines are drawing **now** — the lit set's throttled
    /// draw while the ship is under a burn, nothing while it turns, holds
    /// or is docked. Read off the plan's effort, so it agrees with the
    /// exhaust drawn behind the ship.
    pub engines: f64,
    pub storage: f64,
    pub charge: f64,
}

impl Power {
    /// How hard the reactors are working: everything drawn now over what
    /// they make, `0.0` idle to `1.0` flat out and past it when short.
    /// Nought with no reactor. What the reactor's glow is drawn from.
    pub fn load(&self) -> f64 {
        if self.supply <= 0.0 {
            return 0.0;
        }
        (self.draw + self.engines) / self.supply
    }

    /// Whether the optional consumers have stopped: more drawn than made,
    /// and nothing left in the batteries to cover the difference. A ship
    /// with no batteries and a short network is browned out for good.
    pub fn brownout(&self) -> bool {
        self.draw > self.supply && self.charge <= 0.0
    }
}

/// What a trip would cost. Never a command — see [`World::preview`].
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Preview {
    /// The whole thing, stopping included.
    pub minutes: f64,
    /// How much of that is coming to rest first. Zero from a standstill.
    pub stopping: f64,
    /// What the forward engines will draw off the reactor a minute while
    /// they burn, and how much of their full push that is — the dynamics'
    /// `forward_power` and `forward_throttle`. There is no fuel to quote;
    /// this is what a trip costs the ship, and it costs it nothing after.
    pub power: f64,
    pub throttle: f64,
    pub docks: bool,
}

// --- the droids (feature 83) ----------------------------------------------
//
// `crate::droid` is the plan — how many machines, how many waves, when —
// and `bims::droid` is the machine. This is the world's side: which
// stations are held, laying a wave out in the residents' room, and the
// clock that brings the next one.
//
// A held station's room holds **machines and no people at all**:
// `people_of` is nought for one, and `settle_droids` fills the room it
// opens with the wave that is aboard. Everything after that is the fight
// as it always was — the room is hostile, its bodies are the crew's
// targets, and `visit` carries the hits both ways — bar the one thing
// that had to be taught: a body index past the room's Bims is one of the
// machines (`Game::body_count`).

impl World {
    /// Whether the machines hold this station.
    pub fn is_droid_held(&self, id: u32) -> bool {
        self.infested.iter().any(|it| it.station == id)
    }

    /// The infestation at a station, if there is one.
    pub fn infestation(&self, id: u32) -> Option<&Infestation> {
        self.infested.iter().find(|it| it.station == id)
    }

    fn infestation_mut(&mut self, id: u32) -> Option<&mut Infestation> {
        self.infested.iter_mut().find(|it| it.station == id)
    }

    /// Put a station in the machines' hands. **The crisis step's door**,
    /// and until that step exists the probes' — `droids` and
    /// `droids_planet` call it through `Session`. Sorted by id, since the
    /// checksum runs over the list. Doing it twice changes nothing.
    pub fn infest(&mut self, id: u32) {
        if self.is_droid_held(id) {
            return;
        }
        self.infested.push(Infestation::new(id));
        self.infested.sort_by_key(|it| it.station);
        // The station's people are gone the moment the machines have it:
        // a room already open on it is opened again with nobody in it,
        // the way a stance turning hostile reopens one with a garrison.
        self.reopen_residents(id);
        self.apply_stances();
    }

    /// Whether the last machine of the last wave at this station has been
    /// destroyed. What the crisis step reads to know a station is won
    /// back; false for a station the machines never held.
    pub fn droid_station_cleared(&self, id: u32) -> bool {
        self.infestation(id).is_some_and(|it| it.cleared)
    }

    /// What tier the machines come at. One outside the probes; the
    /// `droids` probes read `BIMS_DROID_TIER` and set it.
    pub fn droid_tier(&self) -> Tier {
        self.droid_tier
    }

    /// The probes' dial: every wave from now on comes at this tier.
    pub fn set_droid_tier_for_probe(&mut self, tier: Tier) {
        self.droid_tier = tier;
    }

    /// How long after a wave is spent the next arrives, in minutes of
    /// the world's clock.
    pub fn droid_reinforce_minutes(&self) -> f64 {
        self.droid_reinforce
    }

    /// The machines' own reinforcement clock, shortened: what the
    /// `droids` probes set to a minute so a wave can be watched arriving
    /// without waiting two hours of the world's clock.
    pub fn set_droid_reinforce_minutes_for_probe(&mut self, minutes: f64) {
        self.droid_reinforce = minutes;
    }

    /// How many machines the next wave is, worked out now: the base, the
    /// crew, the calendar, the worth and the levels
    /// ([`droidplan::wave_size`]). Asked as each wave appears, never
    /// stored.
    pub fn droid_wave_size(&self) -> u32 {
        // The probes' dial says the size outright, since raising the cap
        // alone never makes a wave bigger than the formula: it is there
        // to measure what a wave of that many costs a step and a frame.
        if let Some(forced) = self.droid_wave_forced {
            return forced.max(1);
        }
        droidplan::wave_size(
            self.aboard.crew_count(),
            droidplan::day_steps(self.days_gone()),
            droidplan::worth_steps(self.worth(), self.start_worth),
            self.crew_levels(),
        )
        .min(self.droid_wave_max)
        .max(1)
    }

    /// The probes' other dial (`BIMS_DROID_WAVE`): every wave from now
    /// on is this many machines, whatever the formula and the cap say.
    /// For the measurements feature 83 asks for, and nothing else.
    pub fn set_droid_wave_for_probe(&mut self, n: u32) {
        self.droid_wave_forced = Some(n.max(1));
    }

    /// The cap as it stands — [`data::DROID_WAVE_MAX`], or what a probe
    /// has forced.
    pub fn droid_wave_max(&self) -> u32 {
        self.droid_wave_forced.unwrap_or(self.droid_wave_max)
    }

    /// How many waves a held station has all told, worked out now. Only
    /// ever asked once a station, at the crew's first dock.
    pub fn droid_wave_count(&self) -> u32 {
        // The probes' dial says it outright, the way `droid_wave_size`
        // takes its own: the `droids` commands are looked at for what a
        // wave *after* the first does, and the formula's two at day
        // nought gave one landing and then nothing.
        if let Some(forced) = self.droid_waves_forced {
            return forced.max(1);
        }
        droidplan::wave_count(
            droidplan::day_steps(self.days_gone()),
            droidplan::worth_steps(self.worth(), self.start_worth),
            self.crew_levels(),
        )
    }

    /// The probes' dial: a held station has this many waves all told,
    /// the one aboard counted. Read at the crew's **first dock** and
    /// never again, so it has to be set before the first step.
    pub fn set_droid_waves_for_probe(&mut self, n: u32) {
        self.droid_waves_forced = Some(n.max(1));
    }

    /// Which wave of machines is aboard the held station alongside and
    /// how many are still to come after it — `None` away from one, or
    /// before the crew's first dock has settled the count. What the
    /// app's warning counts off.
    pub fn droid_wave_standing(&self) -> Option<(u32, u32)> {
        let id = self.residents.as_ref()?.station;
        let it = self.infestation(id)?;
        (it.wave > 0).then_some((it.wave, it.waves_left))
    }

    /// How long until the next wave lands at the held station
    /// alongside, in minutes of the world's clock — `None` while a
    /// machine is still standing (the clock does not run then), with
    /// none left to come, or away from a held station. The countdown
    /// the app shows, the way `raid_minutes_left` is the raider's.
    pub fn droid_wave_due(&self) -> Option<f64> {
        let id = self.residents.as_ref()?.station;
        let it = self.infestation(id)?;
        it.next_wave.map(|due| (due - self.clock_minutes).max(0.0))
    }

    /// The crew's class levels less one each, added up: nought for a
    /// crew with no classes, which is what a fresh game is.
    fn crew_levels(&self) -> u32 {
        self.progress
            .iter()
            .map(|p| u32::from(p.level()).saturating_sub(1))
            .sum()
    }

    /// Where the machines' ship stands at a held station, for the
    /// painter: a point and the way it faces, in the **station's own
    /// design units**, and whether it is a lander on the ground rather
    /// than a ship at an airlock.
    ///
    /// `None` unless the wave aboard is one that **arrived** — wave one
    /// was already there and came by nothing — and some of it is still
    /// standing: the ship is drawn while its wave has droids alive and
    /// is gone with them. It is a picture and nothing else: not part of
    /// the room, not walkable, not a thing a bolt can reach.
    pub fn droid_ship(&self, station: u32) -> Option<(DVec2, DVec2, bool)> {
        let wave = self.infestation(station).map(|it| it.wave)?;
        if wave < 2 {
            return None;
        }
        let residents = self.residents.as_ref().filter(|r| r.station == station)?;
        let room = &residents.aboard.room;
        let alive = (0..room.droid_count() as usize)
            .filter_map(|i| room.droid(i))
            .any(|d| !d.destroyed && d.wave == wave);
        if !alive {
            return None;
        }
        let design = &self.station(station)?.design;
        if crate::surface::surface_body(station).is_some() {
            // A lander on the plain beyond the gate its wave walked in
            // by: north for an odd wave, south for an even one.
            let ((x, _), (_, fy)) = droidplan::gate_spot(design.build_area, wave);
            let t = shipdesign::TILE as f64;
            let out = data::DROID_LANDER_TILES * t;
            let y = if fy > 0.0 {
                // The wave walks south, so the lander is north of the
                // wall: beyond the first row.
                -out
            } else {
                design.build_area as f64 * t + out
            };
            return Some((dvec2(x, y), dvec2(0.0, -fy), true));
        }
        let port = droidplan::arrival_airlock(design)?;
        let (fx, fy) = port.face();
        Some((
            dvec2(fx, fy),
            dvec2(port.outward.0 as f64, port.outward.1 as f64),
            false,
        ))
    }

    /// Every state a machine can be drawn in, laid out on the arena's
    /// deck for **one picture**: a row a kind — Husk, Trooper, Warden —
    /// and a column a state — idle, firing or striking, arms at nothing,
    /// legs at nothing, destroyed. Nothing in the game does this; it is
    /// how feature 83's drawings are looked at (`BIMS_DROIDS=1`).
    ///
    /// The wave that was there is replaced, and the machines stand where
    /// they are put: they are all posing, so nothing walks off the mark
    /// while the frames run down to the shot.
    pub fn stage_droids_for_probe(&mut self) -> bool {
        use bims::droid::{Droid, DroidKind, DroidPart};
        let Some(id) = self.residents.as_ref().map(|r| r.station) else {
            return false;
        };
        let tier = self.droid_tier;
        let t = shipdesign::TILE as f32;
        // Laid out from the station's own door inwards, so the rack is
        // beside the ship in the frame rather than across the station:
        // the camera follows the crew member the player steers, and a
        // showcase nobody can see is a screenshot of an empty deck.
        let port = self
            .station(id)
            .and_then(|s| s.port())
            .map(|p| {
                let inside = droidplan::inside_of(&p, 4.0);
                (inside.0 as f32, inside.1 as f32)
            })
            .unwrap_or((18.0 * t, 18.0 * t));
        let across = 5.5 * t;
        let down = 6.0 * t;
        // Three rows down and five across, laid out away from the door.
        let (x0, y0) = (port.0 - across, port.1 - down);
        let Some(residents) = self.residents.as_mut() else {
            return false;
        };
        residents.aboard.room.clear_droids();
        for (row, kind) in DroidKind::ALL.into_iter().enumerate() {
            for state in 0..5u32 {
                let at = bims::math::vec2(x0 + across * state as f32, y0 + down * row as f32);
                let mut droid = Droid::new(kind, tier, 0, 1, at, 0.0, row as u64 * 16 + 5);
                // Every one of them stands where it is put: the arena is
                // hostile and the crew are docked, so left to itself the
                // whole rack would walk off to fight before the shot.
                droid.posing = true;
                match state {
                    // Idle: as it comes.
                    0 => {}
                    // Firing, or a Husk striking: held at the instant.
                    1 => droid.lit = true,
                    // The arms gone: hanging, sparking, the claws dragging.
                    2 => {
                        droid.strike(DroidPart::Arms, droid.body.max(DroidPart::Arms));
                    }
                    // The legs gone: slumped, no step.
                    3 => {
                        droid.strike(DroidPart::Legs, droid.body.max(DroidPart::Legs));
                    }
                    // Destroyed: a smaller dark wreck, the sensor out.
                    _ => droid.destroy(),
                }
                residents.aboard.room.add_droid(droid);
            }
        }
        residents.aboard.crew = residents.aboard.room.body_count();
        // And drawn whether or not the crew can see them: a station's
        // room is `Fog::None`, so a body is drawn only where the world
        // said it is in view, and a rack laid out for a picture is a
        // picture of an empty deck without this.
        residents.aboard.room.show_everybody_for_probe(true);
        true
    }

    /// How many machines are standing in the residents' room.
    pub fn droids_standing(&self) -> u32 {
        let Some(residents) = &self.residents else {
            return 0;
        };
        let room = &residents.aboard.room;
        (0..room.droid_count() as usize)
            .filter(|&i| room.droid(i).is_some_and(|d| !d.destroyed))
            .count() as u32
    }

    /// The machines a wave of `n` is, built: the kinds
    /// ([`bims::droid::wave_kinds`]) at the world's tier, each at one of
    /// `spots` in the **residents' room's** own units, facing `facing`.
    /// A Trooper's arm is dealt by its place among the Troopers, which is
    /// what `wave_kinds` orders the list for.
    fn build_wave(
        &self,
        n: u32,
        wave: u32,
        spots: &[bims::math::Vec2],
        facing: f32,
        seed: u64,
    ) -> Vec<bims::droid::Droid> {
        let tier = self.droid_tier;
        let mut troopers = 0usize;
        bims::droid::wave_kinds(n)
            .into_iter()
            .enumerate()
            .map(|(i, kind)| {
                let index = if kind == bims::droid::DroidKind::Trooper {
                    let n = troopers;
                    troopers += 1;
                    n
                } else {
                    i
                };
                let at = spots.get(i).copied().unwrap_or(bims::math::Vec2::ZERO);
                let mut droid = bims::droid::Droid::new(
                    kind,
                    tier,
                    index,
                    wave,
                    at,
                    facing,
                    seed ^ (i as u64) << 8 ^ u64::from(wave),
                );
                // **Their planning is staggered**, and not for looks: a
                // stand is scored against a lattice of every free cell
                // within the weapon's reach of every target, and sixteen
                // machines all planning on one frame is that walk
                // sixteen times in one step. Spread over the plan's own
                // period they cost a frame one apiece. Worked out from
                // the index, not rolled, so two clients stagger alike.
                droid.plan_wait = bims::game::PLAN_EVERY * (i as f32) / (n.max(1) as f32);
                droid.breach_wait = droid.plan_wait;
                droid
            })
            .collect()
    }

    /// The first wave, stood about the station's rooms: free deck tiles
    /// spread across the design.
    fn first_wave(&self, station: &Station, n: u32, wave: u32) -> Vec<bims::droid::Droid> {
        let Some(residents) = &self.residents else {
            return Vec::new();
        };
        let spots: Vec<bims::math::Vec2> = droidplan::spots_about(&station.design, n as usize)
            .into_iter()
            .map(|(x, y)| residents.aboard.to_room(dvec2(x, y)))
            .collect();
        self.build_wave(n, wave, &spots, 0.0, station.map_seed)
    }

    /// A reinforcement wave, at the airlock its ship tied up at — or, on
    /// a surface, just inside the gate its lander set down beyond, north
    /// for an odd wave and south for an even one.
    fn arriving_wave(&self, station: &Station, n: u32, wave: u32) -> Vec<bims::droid::Droid> {
        let Some(residents) = &self.residents else {
            return Vec::new();
        };
        let (at, facing) = if crate::surface::surface_body(station.id).is_some() {
            let (spot, face) = droidplan::gate_spot(station.design.build_area, wave);
            (spot, bims::math::vec2(face.0 as f32, face.1 as f32).angle())
        } else {
            let Some(port) = droidplan::arrival_airlock(&station.design) else {
                return Vec::new();
            };
            let spot = droidplan::inside_of(&port, data::ASHORE_TILES);
            (
                spot,
                bims::math::vec2(-port.outward.0 as f32, -port.outward.1 as f32).angle(),
            )
        };
        let middle = residents.aboard.to_room(dvec2(at.0, at.1));
        // Spread them round the spot so a wave does not arrive on one
        // tile: a ring a tile and a half across, and a second ring out
        // past it for a wave bigger than the first will hold.
        let t = shipdesign::TILE as f32;
        let spots: Vec<bims::math::Vec2> = (0..n as usize)
            .map(|i| {
                if i == 0 {
                    return middle;
                }
                let ring = ((i - 1) / 6 + 1) as f32;
                let step = (i - 1) % 6;
                let angle = step as f32 / 6.0 * bims::math::TAU;
                middle + bims::math::Vec2::from_angle(angle) * (ring * t * 1.5)
            })
            .collect();
        self.build_wave(n, wave, &spots, facing, station.map_seed)
    }

    /// Put the wave that is aboard into the residents' room, if the room
    /// is open on a held station and has no machines in it yet. Called
    /// wherever the residents' room is built afresh — opened, joined,
    /// unjoined — since a fresh room has no machines and the ones
    /// standing are carried across by hand.
    fn settle_droids(&mut self) {
        let Some(residents) = &self.residents else {
            return;
        };
        let id = residents.station;
        if !self.is_droid_held(id) {
            return;
        }
        // A wave laid this step but with no room to go into yet.
        if !self.droids_to_post.is_empty() {
            let waiting = std::mem::take(&mut self.droids_to_post);
            if let Some(residents) = &mut self.residents {
                residents
                    .aboard
                    .room
                    .adopt_droids(waiting, bims::math::Vec2::ZERO);
                residents.aboard.crew = residents.aboard.room.body_count();
            }
            return;
        }
        if residents.aboard.room.droid_count() > 0 {
            return;
        }
        let Some(wave) = self.infestation(id).map(|it| it.wave) else {
            return;
        };
        if wave == 0 {
            // The crew have not docked here yet, so nothing has been
            // settled and there is nothing to lay out.
            return;
        }
        let Some(station) = self.station(id).cloned() else {
            return;
        };
        let n = self.droid_wave_size();
        let droids = if wave == 1 {
            self.first_wave(&station, n, wave)
        } else {
            self.arriving_wave(&station, n, wave)
        };
        if let Some(residents) = &mut self.residents {
            residents
                .aboard
                .room
                .adopt_droids(droids, bims::math::Vec2::ZERO);
            residents.aboard.crew = residents.aboard.room.body_count();
        }
    }

    /// The machines' clock, a stage of the step: the first dock settles
    /// the wave count, the last machine of a wave starts the timer, and
    /// the timer running out brings the next wave.
    ///
    /// Nothing happens while a machine is still standing — a wave is
    /// never reinforced mid-fight — and a wave whose time came while the
    /// crew were away is laid out the moment the room opens again, since
    /// the timer is the world's clock and not the room's.
    fn droid_waves(&mut self, events: &mut Vec<WorldEvent>) {
        let Some(id) = self.residents.as_ref().map(|r| r.station) else {
            return;
        };
        if !self.is_droid_held(id) {
            return;
        }
        // The count is fixed at the crew's **first dock** and never
        // worked out again.
        if self.aboard.is_joined() && self.infestation(id).is_some_and(|it| !it.settled) {
            let waves = self.droid_wave_count();
            if let Some(it) = self.infestation_mut(id) {
                it.settle(waves);
            }
            self.settle_droids();
        }
        let standing = self.droids_standing();
        let minutes = self.clock_minutes;
        let reinforce = self.droid_reinforce;
        let mut arrive = false;
        let mut cleared = false;
        if let Some(it) = self.infestation_mut(id) {
            if it.wave == 0 {
                return;
            }
            if standing > 0 {
                // A fight is on: the clock does not run.
                it.next_wave = None;
            } else if it.more_to_come() {
                match it.next_wave {
                    None => it.next_wave = Some(minutes + reinforce),
                    Some(due) if minutes >= due => {
                        it.waves_left -= 1;
                        it.wave += 1;
                        it.next_wave = None;
                        arrive = true;
                    }
                    Some(_) => {}
                }
            } else if !it.cleared {
                it.cleared = true;
                cleared = true;
            }
        }
        if arrive {
            // The room's old wrecks go with the wave that made them:
            // a fresh wave is a fresh deck.
            if let Some(residents) = &mut self.residents {
                residents.aboard.room.clear_droids();
                // And so does what was remembered about them. The five
                // lists are one entry a **body**, and `visit` only ever
                // *grows* them — a wave landing makes the room bigger.
                // A wave cleared makes it smaller, and left as they were
                // every machine of the next wave was born already
                // flagged down: no `DroidDown` said for it when it was
                // destroyed, and no experience paid for it either.
                let bims = residents.aboard.room.crew_count() as usize;
                residents.down.truncate(bims);
                residents.xp_down.truncate(bims);
                residents.xp_dead.truncate(bims);
                residents.last_hit_by.truncate(bims);
                residents.fee.truncate(bims);
                residents.medic.truncate(bims);
                residents.grave.truncate(bims);
            }
            self.settle_droids();
            events.push(WorldEvent::DroidReinforcements { station: id });
            // As raid contact does: everybody back to 1x, once, so
            // nobody is caught at 24x by a wave landing.
            // Not a veto — anybody may raise it again.
            for request in &mut self.speed_requests {
                *request = Speed::Real;
            }
        }
        if cleared {
            events.push(WorldEvent::DroidStationCleared { station: id });
        }
    }
}

// --- classes, experience and the engineer's deployables (feature 74) -------
//
// `crate::class` is what a class and its progress are; `crate::deploy`
// what a deployable is. This is the world's side of both: the commands,
// the experience given out in the step, the deployables handed to the
// rooms and read back from them, and what the engineer's talents do to
// each of those.

impl World {
    /// That player's class.
    pub fn class_of(&self, slot: u32) -> Class {
        self.classes.get(slot as usize).copied().unwrap_or_default()
    }

    /// A crew member's progress: the class's, for a player's own; a fresh
    /// one for anybody else, who has none.
    pub fn progress_of(&self, who: u32) -> Progress {
        self.progress.get(who as usize).cloned().unwrap_or_default()
    }

    /// Whether crew member `who` is a player's engineer.
    fn is_engineer(&self, who: u32) -> bool {
        self.class_of(who) == Class::Engineer
    }

    /// Whether crew member `who` is a player's soldier (feature 75).
    fn is_soldier(&self, who: u32) -> bool {
        self.class_of(who) == Class::Soldier
    }

    /// Whether a player's crew member has picked a talent — of its own
    /// class, since a pick is a level and a side and which talent that
    /// is depends on the class.
    pub fn has_talent(&self, who: u32, talent: Talent) -> bool {
        let class = self.class_of(who);
        class == talent.class() && self.progress_of(who).has(class, talent)
    }

    /// Choose a player's class — see [`Command::SetClass`]. What a class
    /// sets out with goes into, or comes out of, its pack: an engineer's
    /// kits, a soldier's rifle, pistol and grenades. A change from one
    /// class to another takes the old kit out and puts the new one in.
    pub fn set_class(&mut self, slot: u32, class: Class) -> Result<(), Refusal> {
        if slot >= self.players() || slot as usize >= self.classes.len() {
            return Err(Refusal::NotAboard);
        }
        if self.undocked_once {
            return Err(Refusal::ClassLocked);
        }
        let was = self.classes[slot as usize];
        if was == class {
            return Ok(());
        }
        self.classes[slot as usize] = class;
        let who = slot as usize;
        match was {
            Class::None => {}
            Class::Engineer => self.take_engineer_kit(who),
            Class::Soldier => self.take_soldier_kit(who),
            Class::Medic => self.take_medic_kit(who),
            Class::Tank => self.take_tank_kit(who),
            // A commander sets out with the laser pistol every Bim is
            // issued and nothing else, so there is nothing to take off.
            Class::Commander => {}
        }
        match class {
            Class::None => {}
            Class::Engineer => self.give_engineer_kit(who),
            Class::Soldier => self.give_soldier_kit(who),
            Class::Medic => self.give_medic_kit(who),
            Class::Tank => self.give_tank_kit(who),
            // And brings nothing of his own but the orders he gives.
            Class::Commander => {}
        }
        Ok(())
    }

    /// The tank's start (feature 77): the laser pistol he has in hand
    /// already, and a fresh basic helm, kevlar and leg guards on — the
    /// world's own pieces, `next_piece` ids at `Where::Worn`, the way
    /// `outfit_for_probe` dresses a crew. Nothing of the hold's moves:
    /// the kit comes with him, like a soldier's rifle.
    fn give_tank_kit(&mut self, who: usize) {
        let mut gear = self.aboard.room.gear(who);
        for kind in ArmourKind::ALL {
            if gear.worn(kind.slot()).is_some() {
                continue;
            }
            let id = self.next_piece;
            self.next_piece += 1;
            self.pieces.push(Piece {
                at: Where::Worn { who: who as u32 },
                ..Piece::new(id, kind, Tier::One)
            });
            *gear.worn_mut(kind.slot()) = Some(bims::combat::Piece::new(id, kind, Tier::One));
        }
        self.aboard.room.issue(who, gear);
    }

    /// And off again: every whole basic piece the tank's start put on,
    /// off the body and off the world's list. A piece the crew member
    /// came by some other way — looted, fetched out of the hold — is
    /// left on, since only the tank's own were made out of nothing.
    fn take_tank_kit(&mut self, who: usize) {
        let mut gear = self.aboard.room.gear(who);
        let mut gone = Vec::new();
        for kind in ArmourKind::ALL {
            let slot = kind.slot();
            let Some(piece) = gear.worn(slot) else {
                continue;
            };
            let ours = self
                .pieces
                .iter()
                .any(|p| p.id == piece.id && p.at == Where::Worn { who: who as u32 });
            if ours && piece.tier == Tier::One {
                gone.push(piece.id);
                *gear.worn_mut(slot) = None;
            }
        }
        if gone.is_empty() {
            return;
        }
        self.pieces.retain(|p| !gone.contains(&p.id));
        self.aboard.room.issue(who, gear);
    }

    /// The medic's start (feature 76): the laser pistol it has in hand
    /// already, and [`class::MEDIC_START_MEDKITS`] medkits with
    /// [`class::MEDIC_START_BANDAGES`] bandages into the pack.
    fn give_medic_kit(&mut self, who: usize) {
        let room = &mut self.aboard.room;
        for _ in 0..class::MEDIC_START_MEDKITS {
            room.give(who, None, Item::Stack(ResourceId::Medkit as u32));
        }
        // The dressings go in boxes of five since feature 87.
        room.give_stack(who, bims::game::BANDAGE, class::MEDIC_START_BANDAGES);
    }

    /// A hired field medic's start (feature 86):
    /// [`mercenary::MEDIC_MEDKITS`] medkits in its pack and nothing
    /// else. No bandages — it is not the class, and the two kits are
    /// what the user asked it to turn up with.
    fn give_field_medic_kit(&mut self, who: u32) {
        let room = &mut self.aboard.room;
        for _ in 0..mercenary::MEDIC_MEDKITS {
            room.give(who as usize, None, Item::Stack(ResourceId::Medkit as u32));
        }
    }

    /// And out again, as many of each as are still there.
    fn take_medic_kit(&mut self, who: usize) {
        let room = &mut self.aboard.room;
        for (resource, count) in [
            (ResourceId::Medkit, class::MEDIC_START_MEDKITS),
            (ResourceId::Bandage, class::MEDIC_START_BANDAGES),
        ] {
            // By unit, not by cell: five dressings go in one box
            // (feature 87), and taking the box would take the lot.
            room.take_stack(who, Item::Stack(resource as u32), count);
        }
    }

    /// The engineer's start: its charges in the pack — [`deploy::SANDBAG_CHARGES`]
    /// sandbag kits and [`deploy::SENTRY_CHARGES`] sentry kits (feature
    /// 88), which is what the cooldowns fill it back up to.
    fn give_engineer_kit(&mut self, who: usize) {
        for (kit, count) in Self::ENGINEER_START {
            let item = Item::Stack(kit.resource() as u32);
            for _ in 0..count {
                self.aboard.room.give(who, None, item);
            }
        }
    }

    /// And out again, as many of each as are there.
    fn take_engineer_kit(&mut self, who: usize) {
        let room = &mut self.aboard.room;
        for (kit, count) in Self::ENGINEER_START {
            let item = Item::Stack(kit.resource() as u32);
            let mut left = count;
            let pack = room.pack(who);
            for (cell, thing) in pack.iter().enumerate() {
                if left > 0 && *thing == Some(item) && room.take(who, cell).is_some() {
                    left -= 1;
                }
            }
        }
    }

    /// What the engineer's class deals it, kit by kit: its charges.
    const ENGINEER_START: [(Kit, u32); 2] = [
        (Kit::Sandbag, deploy::SANDBAG_CHARGES),
        (Kit::Sentry, deploy::SENTRY_CHARGES),
    ];

    /// Kits straight into a crew member's pack, for a probe: `n` of
    /// `kit` given the way the engineer's start gives its own, and how
    /// many of them fitted. Nothing is made and nothing is paid. The
    /// class's own start is [`deploy::SANDBAG_CHARGES`] sandbag kits
    /// and [`deploy::SENTRY_CHARGES`] sentry kits, so a probe
    /// wants this only for more of either than the class deals.
    pub fn give_kits_for_probe(&mut self, who: u32, kit: Kit, n: u32) -> u32 {
        if who >= self.aboard.crew_count() {
            return 0;
        }
        let item = Item::Stack(kit.resource() as u32);
        (0..n)
            .filter(|_| self.aboard.room.give(who as usize, None, item))
            .count() as u32
    }

    /// **Exactly** `n` of every kit in every crew member's pack, for a
    /// probe (feature 88): what is there taken out and `n` put back, and
    /// each cooldown started afresh — so `n` of nought is an engineer
    /// with no charges and the full wait ahead of it, which is the one
    /// state a scripted run cannot walk itself into. `BIMS_KITS=n`.
    pub fn set_kits_for_probe(&mut self, n: u32) {
        for who in 0..self.aboard.crew_count() as usize {
            for kit in Kit::ALL {
                let item = Item::Stack(kit.resource() as u32);
                let pack = self.aboard.room.pack(who);
                for (cell, thing) in pack.iter().enumerate() {
                    if *thing == Some(item) {
                        self.aboard.room.take(who, cell);
                    }
                }
                for _ in 0..n {
                    self.aboard.room.give(who, None, item);
                }
            }
            if self.kit_timers.len() <= who {
                self.kit_timers.resize(who + 1, [None; Kit::ALL.len()]);
            }
            self.kit_timers[who] = [Some(self.clock_minutes); Kit::ALL.len()];
        }
    }

    /// The soldier's start (feature 75): a basic auto rifle in hand, the
    /// laser pistol that was there into the pack, and
    /// [`class::SOLDIER_START_GRENADES`] grenades beside it.
    fn give_soldier_kit(&mut self, who: usize) {
        let room = &mut self.aboard.room;
        let rifle = Item::Weapon(WeaponKind::AutoRifle.basic());
        if room.give(who, None, rifle) {
            let pack = room.pack(who);
            if let Some(cell) = pack.iter().position(|i| *i == Some(rifle)) {
                room.equip(who, cell);
            }
        }
        let grenade = Item::Stack(ResourceId::Grenade as u32);
        for _ in 0..class::SOLDIER_START_GRENADES {
            room.give(who, None, grenade);
        }
    }

    /// And out again: the grenades out of the pack, the pistol back in
    /// the hand and the rifle gone, as far as each is still there.
    fn take_soldier_kit(&mut self, who: usize) {
        let room = &mut self.aboard.room;
        let grenade = Item::Stack(ResourceId::Grenade as u32);
        let mut left = class::SOLDIER_START_GRENADES;
        let pack = room.pack(who);
        for (cell, item) in pack.iter().enumerate() {
            if left > 0 && *item == Some(grenade) && room.take(who, cell).is_some() {
                left -= 1;
            }
        }
        let pistol = Item::Weapon(WeaponKind::LaserPistol.basic());
        let rifle = Item::Weapon(WeaponKind::AutoRifle.basic());
        if room.weapon(who) == Some(WeaponKind::AutoRifle.basic()) {
            let pack = room.pack(who);
            if let Some(cell) = pack.iter().position(|i| *i == Some(pistol)) {
                room.equip(who, cell);
            }
        }
        let pack = room.pack(who);
        if let Some(cell) = pack.iter().position(|i| *i == Some(rifle)) {
            room.take(who, cell);
        }
    }

    /// A pick — see [`Command::PickTalent`].
    fn pick_talent(&mut self, slot: u32, level: u32, side: Side, events: &mut Vec<WorldEvent>) {
        let class = self.class_of(slot);
        if class == Class::None || slot as usize >= self.progress.len() {
            events.push(refused(slot, Refusal::NoClass));
            return;
        }
        let level = u8::try_from(level).unwrap_or(u8::MAX);
        match self.progress[slot as usize].pick(class, level, side) {
            Ok(talent) => events.push(WorldEvent::TalentPicked {
                who: slot,
                talent: talent.code(),
            }),
            Err(why) => events.push(refused(slot, why)),
        }
    }

    /// `xp` to one crew member: every level it reaches said. Public for
    /// the probes and the tests; the game gives experience through the
    /// step alone.
    pub fn award(&mut self, who: usize, xp: u32, events: &mut Vec<WorldEvent>) {
        if self.class_of(who as u32) == Class::None || who >= self.progress.len() {
            return;
        }
        let class = self.class_of(who as u32).code();
        for level in self.progress[who].gain(xp) {
            events.push(WorldEvent::LevelUp {
                who: who as u32,
                class,
                level: level as u32,
            });
        }
    }

    /// Whether crew member `who` is alive, aboard and within the vicinity
    /// of a point of the crew's room.
    pub(crate) fn in_vicinity(&self, who: usize, at: bims::math::Vec2) -> bool {
        let room = &self.aboard.room;
        who < room.crew_count() as usize
            && room.is_alive(who)
            && !room.is_outside(who)
            && (room.bim_pos(who) - at).len() <= class::VICINITY_TILES * shipdesign::TILE as f32
    }

    /// `xp` to every classed crew member within the vicinity of `at`.
    fn award_classed_near(&mut self, at: bims::math::Vec2, xp: u32, events: &mut Vec<WorldEvent>) {
        for who in 0..self.classes.len() {
            if self.class_of(who as u32) != Class::None && self.in_vicinity(who, at) {
                self.award(who, xp, events);
            }
        }
    }

    /// `xp` to every engineer within the vicinity of `at`.
    fn award_engineers_near(
        &mut self,
        at: bims::math::Vec2,
        xp: u32,
        events: &mut Vec<WorldEvent>,
    ) {
        for who in 0..self.classes.len() {
            if self.is_engineer(who as u32) && self.in_vicinity(who, at) {
                self.award(who, xp, events);
            }
        }
    }

    /// After `visit`: every enemy that went down or died this step, once
    /// each, to every classed crew member within the vicinity of where it
    /// lies on the joined deck. Only a hostile station's people, only
    /// while the rooms are joined — on an unjoined deck nobody is in
    /// anybody's vicinity. A crewmate or a hire going down is nobody's
    /// experience.
    /// Hands back every enemy that went down this step with who last hit
    /// it, for the soldiers' *rampage* (`settle_rampage`).
    fn experience(&mut self, events: &mut Vec<WorldEvent>) -> Vec<(usize, Option<usize>)> {
        let mut downed = Vec::new();
        let Some(residents) = &self.residents else {
            return downed;
        };
        if self.stance(residents.station) != Stance::Hostile || !self.aboard.is_joined() {
            return downed;
        }
        let mut gained: Vec<(bims::math::Vec2, u32)> = Vec::new();
        let count = residents.aboard.count() as usize;
        for who in 0..count.min(residents.xp_down.len()) {
            let room = &residents.aboard.room;
            let dead = !room.is_alive(who);
            let down = dead || room.is_unconscious(who);
            let Some(at) = self
                .aboard
                .from_station(residents.aboard.position(who as u32))
            else {
                continue;
            };
            let at = bims::math::vec2(at.x as f32, at.y as f32);
            if down && !residents.xp_down[who] {
                gained.push((at, class::XP_ENEMY_DOWN));
                downed.push((who, residents.last_hit_by.get(who).copied().flatten()));
            }
            if dead && !residents.xp_dead[who] {
                gained.push((at, class::XP_ENEMY_DEAD));
            }
        }
        if let Some(residents) = &mut self.residents {
            for who in 0..count.min(residents.xp_down.len()) {
                let room = &residents.aboard.room;
                let dead = !room.is_alive(who);
                residents.xp_down[who] |= dead || room.is_unconscious(who);
                residents.xp_dead[who] |= dead;
            }
        }
        for (at, xp) in gained {
            self.award_classed_near(at, xp, events);
        }
        downed
    }

    // --- the deployables ---------------------------------------------------

    /// Where a deployable stands in the crew's room, in room units, or
    /// `None` for one on a station's deck that is not in the room.
    fn deployable_room_pos(&self, d: &Deployable) -> Option<bims::math::Vec2> {
        let t = shipdesign::TILE as f64;
        let p = dvec2((d.tile.0 as f64 + 0.5) * t, (d.tile.1 as f64 + 0.5) * t);
        let at = match d.deck {
            Deck::Ship => self.aboard.room_of(false, p)?,
            Deck::Station(id) => {
                if self.ship.state.alongside() != Some(id) {
                    return None;
                }
                self.aboard.from_station(p)?
            }
        };
        Some(bims::math::vec2(at.x as f32, at.y as f32))
    }

    /// The same in the residents' room, or `None` when it has no such
    /// deck: the ship's, while the rooms are joined; the station's own,
    /// whenever the room is open.
    fn deployable_residents_pos(&self, d: &Deployable) -> Option<bims::math::Vec2> {
        let residents = self.residents.as_ref()?;
        let t = shipdesign::TILE as f64;
        let p = dvec2((d.tile.0 as f64 + 0.5) * t, (d.tile.1 as f64 + 0.5) * t);
        let at = match d.deck {
            Deck::Ship => residents.aboard.from_station(p)?,
            Deck::Station(id) => {
                if residents.station != id {
                    return None;
                }
                let at = residents.aboard.to_room(p);
                return Some(at);
            }
        };
        Some(bims::math::vec2(at.x as f32, at.y as f32))
    }

    /// The tile of low cover a point is the middle of.
    fn cover_rect(at: bims::math::Vec2) -> bims::math::Rect {
        let t = shipdesign::TILE as f32;
        bims::math::Rect::from_center_size(at, bims::math::vec2(t, t))
    }

    /// The deployable of a kind on a deck's tile, if any.
    fn deployable_at(&self, deck: Deck, tile: (u32, u32)) -> Option<&Deployable> {
        self.deployables
            .iter()
            .find(|d| d.deck == deck && d.tile == tile)
    }

    /// A deployable by id.
    pub fn deployable(&self, id: u32) -> Option<&Deployable> {
        self.deployables.iter().find(|d| d.id == id)
    }

    /// The deployable standing on the tile of the crew's room a point is
    /// in, if any, with where it stands: what a click on the deck finds.
    pub fn deployable_under(&self, p: bims::math::Vec2) -> Option<(Deployable, bims::math::Vec2)> {
        let t = shipdesign::TILE as f32;
        self.deployables.iter().find_map(|d| {
            let at = self.deployable_room_pos(d)?;
            ((p.x / t).floor() == (at.x / t).floor() && (p.y / t).floor() == (at.y / t).floor())
                .then_some((*d, at))
        })
    }

    /// Every deployable in the crew's room with where it stands, for the
    /// painter and the panels.
    pub fn deployables_in_room(&self) -> Vec<(Deployable, bims::math::Vec2)> {
        self.deployables
            .iter()
            .filter_map(|d| self.deployable_room_pos(d).map(|at| (*d, at)))
            .collect()
    }

    /// The deployables within [`data::REACH`] of crew member `who`, with
    /// where each stands.
    pub fn deployables_in_reach(&self, who: u32) -> Vec<(Deployable, bims::math::Vec2)> {
        if who >= self.aboard.crew_count() {
            return Vec::new();
        }
        let here = self.aboard.room.bim_pos(who as usize);
        let reach = data::REACH * shipdesign::TILE as f32;
        self.deployables_in_room()
            .into_iter()
            .filter(|(_, at)| (*at - here).len() <= reach)
            .collect()
    }

    /// A sentry's rifle for its owner: the auto rifle at tier one, tier
    /// two from the seventh level (*sentry mark II*), and with *sentry
    /// mark III* a **tier-three sniper rifle** (feature 88) — whose
    /// double rate and fifth more damage are [`World::sentry_skill`]'s,
    /// the weapon itself being an ordinary one.
    pub(crate) fn sentry_weapon(&self, owner: u32) -> Weapon {
        if self.has_talent(owner, Talent::SentryMarkThree) {
            return WeaponKind::SniperRifle.at(Tier::Three);
        }
        let tier = if self.progress_of(owner).level() >= class::SENTRY_MARK_TWO_LEVEL {
            Tier::Two
        } else {
            Tier::One
        };
        WeaponKind::AutoRifle.at(tier)
    }

    /// What a sentry's owner's talents do to its shooting (feature 88):
    /// *enhanced optics*' ten tiles of range, and *sentry mark III*'s
    /// fire rate and damage. [`bims::combat::Skill::NONE`] without
    /// either; the room applies it through the one `fire_as` a Bim's
    /// skill goes through.
    pub(crate) fn sentry_skill(&self, owner: u32) -> bims::combat::Skill {
        let mut skill = bims::combat::Skill::NONE;
        if self.has_talent(owner, Talent::EnhancedOptics) {
            skill.range = class::ENHANCED_OPTICS_RANGE;
        }
        if self.has_talent(owner, Talent::SentryMarkThree) {
            skill.fire_rate = class::SENTRY_MARK_THREE_FIRE_RATE;
            skill.damage = class::SENTRY_MARK_THREE_DAMAGE;
        }
        skill
    }

    /// A fresh sentry's health for its owner: *armoured sentry* on top.
    fn sentry_health(&self, owner: u32) -> f32 {
        if self.has_talent(owner, Talent::ArmouredSentry) {
            deploy::SENTRY_HEALTH * class::ARMOURED_SENTRY_HEALTH
        } else {
            deploy::SENTRY_HEALTH
        }
    }

    /// A fresh bag's health for whoever laid it: *reinforced sand* added
    /// (feature 88).
    fn sandbag_health(&self, owner: u32) -> f32 {
        if self.has_talent(owner, Talent::ReinforcedSand) {
            deploy::SANDBAG_HEALTH + class::REINFORCED_SAND_HEALTH
        } else {
            deploy::SANDBAG_HEALTH
        }
    }

    /// **Charges** of a kit an engineer has (feature 88): how many kits
    /// of that kind its pack fills back up to, one at a time on
    /// [`World::kit_cooldown`]. Nought for anybody who is not an
    /// engineer, and nought for a sentry under [`class::SENTRY_LEVEL`].
    /// For a sentry it is also the **world limit**: one more laid
    /// destroys that engineer's oldest.
    pub fn kit_charges(&self, who: u32, kit: Kit) -> u32 {
        if !self.is_engineer(who) {
            return 0;
        }
        match kit {
            Kit::Sandbag => {
                deploy::SANDBAG_CHARGES
                    + if self.has_talent(who, Talent::ExtraBags) {
                        class::EXTRA_BAGS_CHARGES
                    } else {
                        0
                    }
            }
            Kit::Sentry => {
                if self.progress_of(who).level() < class::SENTRY_LEVEL {
                    0
                } else if self.has_talent(who, Talent::SecondSentry) {
                    class::SECOND_SENTRY_CHARGES
                } else {
                    deploy::SENTRY_CHARGES
                }
            }
        }
    }

    /// Seconds of the clock one charge of a kit takes to come back.
    pub fn kit_cooldown(kit: Kit) -> f64 {
        match kit {
            Kit::Sandbag => deploy::SANDBAG_COOLDOWN,
            Kit::Sentry => deploy::SENTRY_COOLDOWN,
        }
    }

    /// Seconds of the clock until the next charge of a kit lands in that
    /// engineer's pack; nought when the pack is already at its charges —
    /// or when the cooldown has run out and the kit is waiting on room.
    pub fn kit_cooldown_left(&self, who: u32, kit: Kit) -> f64 {
        let Some(began) = self
            .kit_timers
            .get(who as usize)
            .and_then(|t| t[kit.code() as usize])
        else {
            return 0.0;
        };
        // Seconds of the room's clock, the grenade cooldown's arithmetic:
        // a game minute is a real second at 1×.
        let since = (self.clock_minutes - began) / time::MINUTES_PER_SECOND;
        (World::kit_cooldown(kit) - since).max(0.0)
    }

    /// How many sentries an engineer may have standing: its sentry
    /// charges (feature 88), so the charge limit *is* the world limit.
    pub fn sentry_limit(&self, owner: u32) -> u32 {
        self.kit_charges(owner, Kit::Sentry)
    }

    /// How many sentries an engineer has standing, anywhere.
    pub fn sentries_of(&self, owner: u32) -> u32 {
        self.deployables
            .iter()
            .filter(|d| d.kind == DeployKind::Sentry && d.owner_slot == owner)
            .count() as u32
    }

    /// How many of a kit a crew member carries in its pack — one a
    /// stack, the way [`World::grenades_of`] counts grenades. What the
    /// engineer's two boxes at the foot of the screen count (feature 80).
    pub fn kits_of(&self, who: u32, kit: Kit) -> u32 {
        if who >= self.aboard.crew_count() {
            return 0;
        }
        let wanted = Item::Stack(kit.resource() as u32);
        self.aboard
            .room
            .pack(who as usize)
            .iter()
            .filter(|i| **i == Some(wanted))
            .count() as u32
    }

    /// How many sentries an engineer could lay now: the kits in its pack
    /// (feature 88). It is no longer held down to the room the limit
    /// leaves, since one over the limit destroys the oldest rather than
    /// being refused.
    pub fn sentries_left(&self, who: u32) -> u32 {
        self.kits_of(who, Kit::Sentry)
    }

    /// Every engineer's packs filled back up to its [`World::kit_charges`]
    /// on the kinds' cooldowns (feature 88), a step of the world's clock
    /// at a time: a charge's cooldown runs whenever the pack is short,
    /// and when it runs out one kit goes in. In combat as out of it —
    /// this is an ability's cooldown and not the dressings' restock — and
    /// nothing is conjured out of the hold: a kit is the ability.
    fn restock_kits(&mut self) {
        let crew = self.aboard.crew_count() as usize;
        if self.kit_timers.len() < crew {
            self.kit_timers.resize(crew, [None; Kit::ALL.len()]);
        }
        let now = self.clock_minutes;
        for who in 0..crew {
            for kit in Kit::ALL {
                let k = kit.code() as usize;
                let charges = self.kit_charges(who as u32, kit);
                let held = self.kits_of(who as u32, kit);
                if held >= charges || !self.aboard.room.is_alive(who) {
                    self.kit_timers[who][k] = None;
                    continue;
                }
                let Some(began) = self.kit_timers[who][k] else {
                    self.kit_timers[who][k] = Some(now);
                    continue;
                };
                let since = (now - began) / time::MINUTES_PER_SECOND;
                if since < World::kit_cooldown(kit) {
                    continue;
                }
                // The charge is up. A pack with nowhere to put it keeps
                // the timer where it is and the kit lands the step room
                // is made, the way the grids keep an overflow unplaced.
                let item = Item::Stack(kit.resource() as u32);
                if self.aboard.room.give(who, None, item) {
                    self.kit_timers[who][k] = (held + 1 < charges).then_some(now);
                }
            }
        }
    }

    /// What a deploy asks, before the errand: the slot's Bim fit to act,
    /// an engineer with the kit in its pack, a sentry from the third
    /// level, and the tile free to take it. What the app greys a press out
    /// with, and [`Command::Deploy`]'s own check.
    pub fn can_deploy(&self, slot: u32, kit: Kit, tile: (i32, i32)) -> Result<(), Refusal> {
        if !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        if !self.is_engineer(slot) {
            return Err(Refusal::NotAnEngineer);
        }
        let who = slot as usize;
        let room = &self.aboard.room;
        let wanted = Item::Stack(kit.resource() as u32);
        if !room.pack(who).iter().any(|i| *i == Some(wanted)) {
            return Err(Refusal::NoKit);
        }
        // A sentry over the limit is not refused (feature 88): the laying
        // destroys the engineer's oldest, so the charges are the limit.
        if kit == Kit::Sentry && self.progress_of(slot).level() < class::SENTRY_LEVEL {
            return Err(Refusal::NoSentryYet);
        }
        let t = shipdesign::TILE as f32;
        let at = bims::math::vec2((tile.0 as f32 + 0.5) * t, (tile.1 as f32 + 0.5) * t);
        if !room.deploy_tile_ok(who, at) || self.deployable_under(at).is_some() {
            return Err(Refusal::CantDeployThere);
        }
        Ok(())
    }

    /// The deploy begun: the walk and the work are the room's
    /// (`Game::deploy`); the kit leaves the pack when the work is done
    /// (`finish_deploy`). See [`Command::Deploy`].
    fn deploy(&mut self, slot: u32, kit: Kit, tile: (i32, i32)) -> Result<(), Refusal> {
        self.can_deploy(slot, kit, tile)?;
        let minutes = self.deploy_minutes(slot, kit);
        let t = shipdesign::TILE as f32;
        let at = bims::math::vec2((tile.0 as f32 + 0.5) * t, (tile.1 as f32 + 0.5) * t);
        if !self
            .aboard
            .room
            .deploy(slot as usize, at, kit == Kit::Sentry, minutes as f32)
        {
            return Err(Refusal::CantDeployThere);
        }
        Ok(())
    }

    /// How long laying a kit takes an engineer, in game minutes: the
    /// kind's, halved by *sandbagger* or *quick build*.
    pub fn deploy_minutes(&self, slot: u32, kit: Kit) -> f64 {
        match kit {
            Kit::Sandbag => {
                let factor = if self.has_talent(slot, Talent::Sandbagger) {
                    class::SANDBAGGER_TIME
                } else {
                    1.0
                };
                deploy::DEPLOY_SANDBAG_MINUTES * factor
            }
            Kit::Sentry => {
                let factor = if self.has_talent(slot, Talent::QuickBuild) {
                    class::QUICK_BUILD_TIME
                } else {
                    1.0
                };
                deploy::DEPLOY_SENTRY_MINUTES * factor
            }
        }
    }

    /// Which deck a point of the crew's room is on, and which tile of
    /// that deck's design.
    fn deck_of(&self, at: bims::math::Vec2) -> (Deck, (u32, u32)) {
        let (foreign, p) = self.aboard.design_of(dvec2(at.x as f64, at.y as f64));
        let t = shipdesign::TILE as f64;
        let tile = (
            (p.x / t).floor().max(0.0) as u32,
            (p.y / t).floor().max(0.0) as u32,
        );
        let deck = match (foreign, self.ship.state.alongside()) {
            (true, Some(id)) => Deck::Station(id),
            _ => Deck::Ship,
        };
        (deck, tile)
    }

    /// The work done: the room said `who` laid a kit at `at`. The kit
    /// comes out of the pack now and the deployable goes down — if the
    /// kit is still there, the engineer still one and the tile still
    /// free; else nothing, and the kit stays where it is. A re-used kit
    /// (`reused_kits`) is spent first and gives no experience; a fresh one
    /// is [`class::XP_BUILT`] to every engineer in the layer's vicinity.
    /// *Bulk bags* lays a second tile of sandbags beside the first out of
    /// the one kit, on the first free neighbour. A sentry laid with as
    /// many of that engineer's standing as it has charges **destroys its
    /// oldest** (feature 88): the charges are the world limit.
    fn finish_deploy(
        &mut self,
        who: usize,
        at: bims::math::Vec2,
        sentry: bool,
        events: &mut Vec<WorldEvent>,
    ) {
        let slot = who as u32;
        let kit = if sentry { Kit::Sentry } else { Kit::Sandbag };
        if !self.is_engineer(slot) {
            return;
        }
        let wanted = Item::Stack(kit.resource() as u32);
        let Some(cell) = self
            .aboard
            .room
            .pack(who)
            .iter()
            .position(|i| *i == Some(wanted))
        else {
            return;
        };
        let (deck, tile) = self.deck_of(at);
        if self.deployable_at(deck, tile).is_some() {
            return;
        }
        if self.aboard.room.take(who, cell).is_none() {
            return;
        }
        // One more than the charges allow: the oldest of this engineer's
        // goes, so a sentry can be moved about the deck freely and never
        // outnumbers its charges (feature 88).
        if sentry {
            let limit = self.sentry_limit(slot);
            while self.sentries_of(slot) >= limit.max(1) {
                let Some(oldest) = self
                    .deployables
                    .iter()
                    .filter(|d| d.kind == DeployKind::Sentry && d.owner_slot == slot)
                    .map(|d| d.id)
                    .min()
                else {
                    break;
                };
                self.deployables.retain(|d| d.id != oldest);
                events.push(WorldEvent::DeployableLost {
                    kind: DeployKind::Sentry.code(),
                });
            }
        }
        self.lay(kit.lays(), slot, deck, tile);
        if kit == Kit::Sandbag && self.has_talent(slot, Talent::BulkBags) {
            let t = shipdesign::TILE as f32;
            let beside = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)]
                .into_iter()
                .map(|(dx, dy)| at + bims::math::vec2(dx * t, dy * t))
                .find(|&p| {
                    self.aboard.room.deploy_tile_ok(who, p) && self.deployable_under(p).is_none()
                });
            if let Some(p) = beside {
                let (deck, tile) = self.deck_of(p);
                self.lay(DeployKind::Sandbags, slot, deck, tile);
            }
        }
        events.push(WorldEvent::Deployed {
            who: slot,
            kind: kit.lays().code(),
        });
        if self.reused_kits.get(who).copied().unwrap_or(0) > 0 {
            self.reused_kits[who] -= 1;
        } else {
            self.award_engineers_near(at, class::XP_BUILT, events);
        }
        self.sync_deployed_cover();
    }

    /// One deployable down, fresh, for `owner`.
    fn lay(&mut self, kind: DeployKind, owner: u32, deck: Deck, tile: (u32, u32)) {
        let id = self.next_deployable;
        self.next_deployable += 1;
        let health = match kind {
            DeployKind::Sandbags => self.sandbag_health(owner),
            DeployKind::Sentry => self.sentry_health(owner),
        };
        self.deployables.push(Deployable {
            id,
            kind,
            owner_slot: owner,
            deck,
            tile,
            health,
        });
        self.deployables.sort_by_key(|d| d.id);
    }

    /// Which deployable a player's engineer stands within reach of, by
    /// id, fit to act — what a pack-up wants first.
    fn deployable_in_reach(&self, slot: u32, id: u32) -> Result<Deployable, Refusal> {
        if !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        if !self.is_engineer(slot) {
            return Err(Refusal::NotAnEngineer);
        }
        let d = *self.deployable(id).ok_or(Refusal::NoSuchDeployable)?;
        if !self
            .deployables_in_reach(slot)
            .iter()
            .any(|(near, _)| near.id == id)
        {
            return Err(Refusal::OutOfReach);
        }
        Ok(d)
    }

    /// A deployable back into the pack as a kit — see [`Command::PackUp`].
    fn pack_up(&mut self, slot: u32, id: u32, events: &mut Vec<WorldEvent>) {
        let d = match self.deployable_in_reach(slot, id) {
            Ok(d) => d,
            Err(why) => {
                events.push(refused(slot, why));
                return;
            }
        };
        let kit = Item::Stack(d.kind.kit().resource() as u32);
        if !self.aboard.room.give(slot as usize, None, kit) {
            events.push(refused(slot, Refusal::PackFull));
            return;
        }
        self.deployables.retain(|x| x.id != id);
        if let Some(n) = self.reused_kits.get_mut(slot as usize) {
            *n += 1;
        }
        events.push(WorldEvent::PackedUp {
            who: slot,
            kind: d.kind.code(),
        });
        self.sync_deployed_cover();
    }

    /// Before the rooms step: what the engineers' talents do to the crew's
    /// working steps and to a deploy under fire, the sandbags laid as
    /// cover on both rooms, and the sentries on the crew's deck.
    fn hand_the_room_the_engineers(&mut self) {
        let crew = self.aboard.crew_count();
        // The craft factor is one for everybody since feature 88 took
        // *quick hands* off the tree — every talent of the engineer's is
        // combat's now — and the pair is kept because the room's own
        // `set_work_factors` is a craft's and a build's.
        let factors: Vec<(f32, f32)> = (0..crew)
            .map(|who| {
                (
                    1.0,
                    if self.has_talent(who, Talent::SiteForeman) {
                        class::SITE_FOREMAN_EFFORT
                    } else {
                        1.0
                    },
                )
            })
            .collect();
        let steady: Vec<bool> = (0..crew)
            .map(|who| self.has_talent(who, Talent::SteadyHands))
            .collect();
        self.aboard.room.set_work_factors(factors);
        self.aboard.room.set_steady_hands(steady);
        self.sync_deployed_cover();
        self.hand_the_room_the_sentries();
    }

    /// What each crew member's class wears (feature 81), to the room.
    /// Drawing only: a class is a player slot's, so the crew past the
    /// players — a hire, a mercenary — are in nothing, and a station's
    /// residents are never told at all. Said every step because a class
    /// is chosen in the yard, a hire shifts nobody's slot and a save
    /// carries the classes but not the picture; `set_outfit` writes only
    /// when the answer changed.
    fn hand_the_room_the_outfits(&mut self) {
        let crew = self.aboard.crew_count();
        let outfits: Vec<bims::character::Outfit> = (0..crew)
            .map(|who| self.class_of(who as u32).outfit())
            .collect();
        for (who, outfit) in outfits.into_iter().enumerate() {
            self.aboard.room.set_outfit(who, outfit);
        }
    }

    /// The sentries on the crew's deck, as they stand now, to the room.
    fn hand_the_room_the_sentries(&mut self) {
        let sentries: Vec<Sentry> = self
            .deployables
            .iter()
            .filter(|d| d.kind == DeployKind::Sentry)
            .filter_map(|d| {
                let at = self.deployable_room_pos(d)?;
                Some(Sentry {
                    id: d.id,
                    at,
                    weapon: self.sentry_weapon(d.owner_slot),
                    skill: self.sentry_skill(d.owner_slot),
                    dug_in: self.has_talent(d.owner_slot, Talent::DugIn),
                    trigger: bims::combat::Trigger::default(),
                })
            })
            .collect();
        self.aboard.room.set_sentries(sentries);
    }

    /// The laid sandbags as cover on both rooms, as they stand now. Cheap
    /// when nothing changed (`Sight::set_laid_cover` compares), so it is
    /// asked every step and after every change to the list.
    fn sync_deployed_cover(&mut self) {
        let bags: Vec<&Deployable> = self
            .deployables
            .iter()
            .filter(|d| d.kind == DeployKind::Sandbags)
            .collect();
        let crew: Vec<bims::math::Rect> = bags
            .iter()
            .filter_map(|d| self.deployable_room_pos(d).map(World::cover_rect))
            .collect();
        let theirs: Vec<bims::math::Rect> = bags
            .iter()
            .filter_map(|d| self.deployable_residents_pos(d).map(World::cover_rect))
            .collect();
        self.aboard.room.set_laid_cover(&crew);
        if let Some(residents) = &mut self.residents {
            residents.aboard.room.set_laid_cover(&theirs);
        }
    }

    /// The sentries in the crew's room, index for index with how they
    /// were handed over, with where each stands: what the residents'
    /// room is handed after the crew as targets.
    fn sentries_in_room(&self) -> Vec<(Deployable, bims::math::Vec2)> {
        self.aboard
            .room
            .sentries()
            .iter()
            .filter_map(|s| self.deployable(s.id).map(|d| (*d, s.at)))
            .collect()
    }

    /// After `visit`: what the fight did to the deployables — the hits the
    /// sentries took, the bolts the sandbags stopped — and what is gone
    /// for it, said. Nothing comes back: a destroyed sentry's charge
    /// returns on its cooldown like any other (feature 88).
    pub(crate) fn settle_deployables(&mut self, events: &mut Vec<WorldEvent>) {
        for (id, damage) in self.aboard.room.take_sentry_hits() {
            if let Some(d) = self.deployables.iter_mut().find(|d| d.id == id) {
                d.health -= damage;
            }
        }
        let hits = self.aboard.room.take_cover_hits();
        if !hits.is_empty() {
            let t = shipdesign::TILE as f32;
            for ((x, y), damage) in hits {
                let p = bims::math::vec2((x as f32 + 0.5) * t, (y as f32 + 0.5) * t);
                if let Some((d, _)) = self
                    .deployable_under(p)
                    .filter(|(d, _)| d.kind == DeployKind::Sandbags)
                    && let Some(d) = self.deployables.iter_mut().find(|x| x.id == d.id)
                {
                    d.health -= damage;
                }
            }
        }
        let gone: Vec<Deployable> = self
            .deployables
            .iter()
            .filter(|d| d.health <= 0.0)
            .copied()
            .collect();
        if gone.is_empty() {
            return;
        }
        self.deployables.retain(|d| d.health > 0.0);
        self.hand_the_room_the_sentries();
        for d in gone {
            events.push(WorldEvent::DeployableLost {
                kind: d.kind.code(),
            });
        }
        self.sync_deployed_cover();
    }

    /// The deployables on a station's deck are lost when the rooms
    /// unjoin: called from `unjoin_rooms`.
    fn drop_station_deployables(&mut self) {
        self.deployables.retain(|d| d.deck == Deck::Ship);
    }

    // --- the armourer's repair ---------------------------------------------

    /// Whether a player's engineer could begin a repair now, or why not:
    /// what greys the bench window's button. The command's own check.
    pub fn can_repair(&self, slot: u32) -> Result<(), Refusal> {
        if !self.has_talent(slot, Talent::Armourer) {
            return Err(Refusal::NoTalent);
        }
        if !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        if self.workbench().is_none() {
            return Err(Refusal::NoWorkbench);
        }
        if self.bench.busy() || self.bench.slots[Workbench::OUT].is_some() {
            return Err(Refusal::BenchBusy);
        }
        let damaged = match (self.bench.slots[0], self.bench.slots[1]) {
            (Some(Item::Armour(p)), None) => p.health < p.stats().health,
            _ => false,
        };
        if !damaged {
            return Err(Refusal::NoPair);
        }
        if self.free(ResourceId::Metal) < deploy::ARMOUR_REPAIR_METAL {
            return Err(Refusal::NotAboard);
        }
        Ok(())
    }

    /// A repair begun at the workbench — see [`Command::Repair`].
    fn begin_repair(&mut self, slot: u32, events: &mut Vec<WorldEvent>) -> Result<(), Refusal> {
        self.can_repair(slot)?;
        self.ship.design.cargo[ResourceId::Metal as usize] -= deploy::ARMOUR_REPAIR_METAL;
        self.on_ship_changed();
        self.bench.repair = Some(slot);
        let _ = events;
        Ok(())
    }

    // --- the soldier: the brace, the skills and the grenades (feature 75) --

    /// What a crew member shoots with over its weapon, for the room's one
    /// shooter (`bims::combat::Skill`): a soldier's talents and its brace,
    /// every factor `crate::class`'s constant; `Skill::NONE` for anybody
    /// else. Worked out fresh every step, since the brace and the
    /// *rampage* stacks move.
    pub fn skill_of(&self, who: u32) -> bims::combat::Skill {
        let mut skill = if self.is_medic(who) {
            self.medic_skill(who)
        } else if self.is_tank(who) {
            self.tank_skill(who)
        } else {
            self.soldier_skill(who)
        };
        // How fast a worn piece drains is everybody's business, not only
        // a tank's: *rallying wall* gives it to the crew round him
        // (feature 77).
        skill.armour_drain = self.armour_drain(who);
        // And an engineer's *higher quality armour* is his own pieces'
        // (feature 88): a point on their protection, and five per cent
        // more health said as the drain's reciprocal, which is the tank's
        // own mechanism — a piece's stored health never changes meaning
        // as it moves between bodies.
        if self.has_talent(who, Talent::BetterArmour) {
            skill.armour_protection_add = class::BETTER_ARMOUR_PROTECTION;
            skill.armour_drain /= class::BETTER_ARMOUR_HEALTH;
        }
        // And so is a commander's aura, his rally and his squad order
        // (feature 78): they lift whatever the crew member's own class
        // gave it, a player's own steered Bim included.
        self.lift_by_aura(who, &mut skill);
        skill
    }

    /// The soldier's half of [`World::skill_of`]; `Skill::NONE` for
    /// anybody else.
    fn soldier_skill(&self, who: u32) -> bims::combat::Skill {
        if !self.is_soldier(who) {
            return bims::combat::Skill::NONE;
        }
        let progress = self.progress_of(who);
        let has = |talent| progress.has(Class::Soldier, talent);
        let braced = self.aboard.room.is_braced(who as usize);
        let mut skill = bims::combat::Skill::NONE;
        if braced {
            skill.accuracy *= class::BRACE_ACCURACY;
        }
        if has(Talent::Marksman) {
            skill.accuracy *= class::MARKSMAN_ACCURACY;
        }
        if has(Talent::PointBlank) {
            skill.point_blank = class::POINT_BLANK_DAMAGE;
        }
        if has(Talent::Runner) {
            skill.pace = class::RUNNER_PACE;
        }
        if has(Talent::SteadyAim) {
            skill.walking = class::steady_aim_walking();
        }
        if has(Talent::IronNerve) {
            skill.nerve = true;
        }
        if has(Talent::CoverMaster) {
            skill.cover_dodge = (skill.cover_dodge * class::COVER_MASTER_DODGE).min(1.0);
        }
        if progress.level() >= class::DRILL_LEVEL {
            skill.fire_rate *= class::DRILL_FIRE_RATE;
        }
        if has(Talent::Bruiser) {
            skill.melee = class::BRUISER_MELEE;
        }
        if has(Talent::DugInBraced) && braced {
            skill.dodge = class::DUG_IN_DODGE;
        }
        if has(Talent::Deadeye) {
            skill.deadeye = true;
        }
        if has(Talent::Rampage) {
            let stacks = self
                .aboard
                .room
                .rampage(who as usize)
                .min(class::RAMPAGE_STACKS);
            skill.fire_rate *= class::RAMPAGE_FIRE_RATE.powi(stacks as i32);
        }
        skill
    }

    /// Whether a player's soldier may brace, or why not: a soldier, and
    /// fit to act. What the app greys the key with and [`Command::Brace`]
    /// asks.
    pub fn can_brace(&self, slot: u32) -> Result<(), Refusal> {
        if !class::can(self.class_of(slot), class::Ability::Brace) {
            return Err(Refusal::NotASoldier);
        }
        if !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        Ok(())
    }

    /// Brace, or stand easy — see [`Command::Brace`]. The room holds the
    /// flag (`Game::set_braced`), and ends it on its own when the soldier
    /// is ordered anywhere or goes down.
    fn brace(&mut self, slot: u32, on: bool) -> Result<(), Refusal> {
        self.can_brace(slot)?;
        self.aboard.room.set_braced(slot as usize, on);
        Ok(())
    }

    /// Whether a crew member is braced.
    pub fn is_braced(&self, who: u32) -> bool {
        self.aboard.room.is_braced(who as usize)
    }

    /// How far a soldier throws, in tiles: the range, half again with
    /// *long throw*.
    pub fn grenade_range(&self, who: u32) -> f32 {
        if self.has_talent(who, Talent::LongThrow) {
            class::GRENADE_RANGE * class::LONG_THROW_RANGE
        } else {
            class::GRENADE_RANGE
        }
    }

    /// Seconds from the throw to the burst: the fuse, halved with *short
    /// fuse*.
    pub fn grenade_fuse(&self, who: u32) -> f32 {
        if self.has_talent(who, Talent::ShortFuse) {
            class::GRENADE_FUSE * class::SHORT_FUSE_TIME
        } else {
            class::GRENADE_FUSE
        }
    }

    /// How far a burst reaches, in tiles: the radius, half again with
    /// *frag*.
    pub fn grenade_radius(&self, who: u32) -> f32 {
        if self.has_talent(who, Talent::Frag) {
            class::GRENADE_RADIUS * class::FRAG_RADIUS
        } else {
            class::GRENADE_RADIUS
        }
    }

    /// Seconds of the clock between one throw and the next: the
    /// cooldown, halved with *quick draw*.
    pub fn grenade_cooldown(&self, who: u32) -> f64 {
        if self.has_talent(who, Talent::QuickDraw) {
            class::GRENADE_COOLDOWN * class::QUICK_DRAW_COOLDOWN
        } else {
            class::GRENADE_COOLDOWN
        }
    }

    /// Seconds of the clock until a crew member may throw again; nought
    /// when it may.
    pub fn grenade_cooldown_left(&self, who: u32) -> f64 {
        let Some(last) = self.last_throw.get(who as usize).copied().flatten() else {
            return 0.0;
        };
        // Seconds of the room's clock: a game minute is a real second at 1×
        // (`time::MINUTES_PER_SECOND`), the way the room steps.
        let since = (self.clock_minutes - last) / time::MINUTES_PER_SECOND;
        (self.grenade_cooldown(who) - since).max(0.0)
    }

    /// Grenades in a crew member's pack.
    pub fn grenades_of(&self, who: u32) -> u32 {
        if who >= self.aboard.crew_count() {
            return 0;
        }
        let wanted = Item::Stack(ResourceId::Grenade as u32);
        self.aboard
            .room
            .pack(who as usize)
            .iter()
            .filter(|i| **i == Some(wanted))
            .count() as u32
    }

    /// What a throw asks, in the order the refusals are said: the slot's
    /// Bim fit to act (`OutOfReach`), a soldier (`NotASoldier`), at the
    /// grenade level (`NoGrenadesYet`), a grenade in the pack
    /// (`NoGrenade`), past its cooldown (`CoolingDown`), and the tile —
    /// a room tile, like a deploy's — deck of the room (`CantThrowThere`)
    /// within its range (`OutOfThrowRange`) with nothing opaque between
    /// (`NoLineToTile`): walls and shut doors stop a throw, sandbags do
    /// not. What the app greys a press with, and [`Command::Throw`]'s
    /// own check.
    pub fn can_throw(&self, slot: u32, tile: (i32, i32)) -> Result<(), Refusal> {
        if !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        if !class::can(self.class_of(slot), class::Ability::Throw) {
            return Err(Refusal::NotASoldier);
        }
        if self.progress_of(slot).level() < class::GRENADE_LEVEL {
            return Err(Refusal::NoGrenadesYet);
        }
        if self.grenades_of(slot) == 0 {
            return Err(Refusal::NoGrenade);
        }
        if self.grenade_cooldown_left(slot) > 0.0 {
            return Err(Refusal::CoolingDown);
        }
        let t = shipdesign::TILE as f32;
        let at = bims::math::vec2((tile.0 as f32 + 0.5) * t, (tile.1 as f32 + 0.5) * t);
        if !self.aboard.room.is_deck_tile(at) {
            return Err(Refusal::CantThrowThere);
        }
        let from = self.aboard.room.bim_pos(slot as usize);
        if (at - from).len() > self.grenade_range(slot) * t {
            return Err(Refusal::OutOfThrowRange);
        }
        if !self.aboard.room.line_clear(from, at) {
            return Err(Refusal::NoLineToTile);
        }
        Ok(())
    }

    /// The throw — see [`Command::Throw`]: the grenade out of the pack
    /// now, the clock noted, and the room throws it with the fuse, the
    /// radius and the damage the soldier's talents give it.
    fn throw(&mut self, slot: u32, tile: (i32, i32)) -> Result<(), Refusal> {
        self.can_throw(slot, tile)?;
        let who = slot as usize;
        let wanted = Item::Stack(ResourceId::Grenade as u32);
        let cell = self
            .aboard
            .room
            .pack(who)
            .iter()
            .position(|i| *i == Some(wanted))
            .ok_or(Refusal::NoGrenade)?;
        if self.aboard.room.take(who, cell).is_none() {
            return Err(Refusal::NoGrenade);
        }
        if self.last_throw.len() <= who {
            self.last_throw.resize(who + 1, None);
        }
        self.last_throw[who] = Some(self.clock_minutes);
        let t = shipdesign::TILE as f32;
        let at = bims::math::vec2((tile.0 as f32 + 0.5) * t, (tile.1 as f32 + 0.5) * t);
        let fuse = self.grenade_fuse(slot);
        let radius = self.grenade_radius(slot) * t;
        self.aboard
            .room
            .throw_grenade(who, at, fuse, radius, class::GRENADE_DAMAGE);
        Ok(())
    }

    // --- the medic: the beam and the surge (feature 76) --------------------
    //
    // `crate::medic` is the state; this is the rules. The beam is a list
    // of crew indices on the medic, checked every step before the rooms
    // step and handed to the room as what it does to each body
    // (`bims::health::Beamed`); the surge is the room's own timer on
    // each body, set here off the medic's charge.

    /// Whether crew member `who` is a player's medic.
    fn is_medic(&self, who: u32) -> bool {
        self.class_of(who) == Class::Medic
    }

    /// A crew member's medic state — an empty one for anybody the world
    /// keeps none for.
    pub fn medic_of(&self, who: u32) -> Medic {
        self.medics.get(who as usize).cloned().unwrap_or_default()
    }

    /// The crew members a medic's beam holds, by index.
    pub fn patients_of(&self, who: u32) -> Vec<u32> {
        self.medic_of(who).patients
    }

    /// Whether a medic's beam holds anybody.
    pub fn is_beaming(&self, who: u32) -> bool {
        self.medic_of(who).is_linked()
    }

    /// The medic's state, made if the crew grew past the list.
    fn medic_mut(&mut self, who: usize) -> &mut Medic {
        if self.medics.len() <= who {
            self.medics.resize(who + 1, Medic::default());
        }
        &mut self.medics[who]
    }

    /// What a medic shoots with (feature 76): its fire held while the
    /// beam is linked, or at [`class::GUNNER_MEDIC_FIRE_RATE`] with
    /// *gunner medic*; `Skill::NONE` unlinked.
    fn medic_skill(&self, who: u32) -> bims::combat::Skill {
        let mut skill = bims::combat::Skill::NONE;
        if self.is_beaming(who) {
            if self.has_talent(who, Talent::GunnerMedic) {
                skill.fire_rate *= class::GUNNER_MEDIC_FIRE_RATE;
            } else {
                skill.holds_fire = true;
            }
        }
        skill
    }

    /// How far a medic's beam reaches, in tiles: the range, half again
    /// with *long beam*.
    pub fn beam_range(&self, who: u32) -> f32 {
        if self.has_talent(who, Talent::LongBeam) {
            class::HEAL_BEAM_RANGE * class::LONG_BEAM_RANGE
        } else {
            class::HEAL_BEAM_RANGE
        }
    }

    /// Blood a beamed patient gains an hour: the rate, half again with
    /// *strong beam*.
    pub fn beam_blood(&self, who: u32) -> f32 {
        if self.has_talent(who, Talent::StrongBeam) {
            class::HEAL_BEAM_BLOOD * class::STRONG_BEAM_BLOOD
        } else {
            class::HEAL_BEAM_BLOOD
        }
    }

    /// How many patients a medic's beam holds at once: one, or
    /// [`class::DOUBLE_LINK_PATIENTS`] with *double link*.
    pub fn beam_patients(&self, who: u32) -> usize {
        if self.has_talent(who, Talent::DoubleLink) {
            class::DOUBLE_LINK_PATIENTS
        } else {
            1
        }
    }

    /// Minutes of qualifying beaming a medic's surge wants to be full:
    /// [`class::SURGE_CHARGE_MINUTES`], less with *quick charge*.
    pub fn surge_charge_wanted(&self, who: u32) -> f64 {
        if self.has_talent(who, Talent::QuickCharge) {
            class::SURGE_CHARGE_MINUTES / class::QUICK_CHARGE_RATE
        } else {
            class::SURGE_CHARGE_MINUTES
        }
    }

    /// How full a medic's surge is, nought to one.
    pub fn surge_charge(&self, who: u32) -> f32 {
        (self.medic_of(who).charge / self.surge_charge_wanted(who)).clamp(0.0, 1.0) as f32
    }

    /// Minutes of the clock a medic's surge runs: [`class::SURGE_MINUTES`],
    /// half again with *long surge*.
    pub fn surge_minutes(&self, who: u32) -> f64 {
        if self.has_talent(who, Talent::LongSurge) {
            class::SURGE_MINUTES * class::LONG_SURGE_TIME
        } else {
            class::SURGE_MINUTES
        }
    }

    /// Seconds of the room's clock a crew member's surge has left; nought
    /// with none running.
    pub fn surge_left(&self, who: u32) -> f64 {
        self.aboard.room.surge_left(who as usize) as f64
    }

    /// Whether a surge runs on a crew member.
    pub fn is_surging(&self, who: u32) -> bool {
        self.aboard.room.is_surging(who as usize)
    }

    /// Whether a crewmate is where a medic's beam reaches it: alive, in
    /// the room, within the medic's range and in its sight. What a link
    /// asks, and what keeps one.
    fn beam_reaches(&self, medic: u32, patient: u32) -> Result<(), Refusal> {
        let room = &self.aboard.room;
        let (m, p) = (medic as usize, patient as usize);
        if patient >= self.aboard.crew_count() || patient == medic || !room.is_alive(p) {
            return Err(Refusal::NotACrewmate);
        }
        if room.is_outside(p) || room.is_outside(m) {
            return Err(Refusal::OutOfBeamRange);
        }
        let at = room.bim_pos(p);
        let t = shipdesign::TILE as f32;
        if (at - room.bim_pos(m)).len() > self.beam_range(medic) * t {
            return Err(Refusal::OutOfBeamRange);
        }
        if !room.sees(m, at) {
            return Err(Refusal::NoSightOfPatient);
        }
        Ok(())
    }

    /// Whether a player's medic may link its beam to `patient`, or why
    /// not, in the order the refusals are said: a medic (`NotAMedic`),
    /// fit to act (`OutOfReach`), a living crewmate — any crew member
    /// but itself (`NotACrewmate`) — in the room and within range
    /// (`OutOfBeamRange`), in its sight (`NoSightOfPatient`). What the
    /// app greys the key with and [`Command::Beam`] asks.
    pub fn can_beam(&self, slot: u32, patient: u32) -> Result<(), Refusal> {
        if !class::can(self.class_of(slot), class::Ability::Beam) {
            return Err(Refusal::NotAMedic);
        }
        if !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        self.beam_reaches(slot, patient)
    }

    /// Link, or unlink — see [`Command::Beam`]. A patient already held is
    /// held still; one past the beam's count takes the oldest's place.
    fn beam(&mut self, slot: u32, patient: Option<u32>) -> Result<(), Refusal> {
        let Some(patient) = patient else {
            if !class::can(self.class_of(slot), class::Ability::Beam) {
                return Err(Refusal::NotAMedic);
            }
            self.medic_mut(slot as usize).unlink();
            self.aboard.room.set_beaming(slot as usize, false);
            return Ok(());
        };
        self.can_beam(slot, patient)?;
        let most = self.beam_patients(slot);
        let medic = self.medic_mut(slot as usize);
        if !medic.holds(patient) {
            medic.patients.push(patient);
            while medic.patients.len() > most {
                medic.patients.remove(0);
            }
        }
        self.aboard.room.set_beaming(slot as usize, true);
        Ok(())
    }

    /// Whether a player's medic may trigger its surge, or why not, in
    /// order: a medic (`NotAMedic`), fit to act (`OutOfReach`), at
    /// [`class::SURGE_LEVEL`] (`NoSurgeYet`), linked (`NotLinked`), and
    /// charged (`NotCharged`).
    pub fn can_surge(&self, slot: u32) -> Result<(), Refusal> {
        if !class::can(self.class_of(slot), class::Ability::Surge) {
            return Err(Refusal::NotAMedic);
        }
        if !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        if self.progress_of(slot).level() < class::SURGE_LEVEL {
            return Err(Refusal::NoSurgeYet);
        }
        if !self.is_beaming(slot) {
            return Err(Refusal::NotLinked);
        }
        if self.surge_charge(slot) < 1.0 {
            return Err(Refusal::NotCharged);
        }
        Ok(())
    }

    /// The surge — see [`Command::Surge`]: the charge emptied, and the
    /// room's timer set on the medic and every patient (and, with *mass
    /// surge*, every crew member within [`class::MASS_SURGE_TILES`] of a
    /// patient) for the medic's minutes; a patient's closes its wounds as
    /// it ends with *closing surge*.
    fn surge(&mut self, slot: u32) -> Result<(), Refusal> {
        self.can_surge(slot)?;
        let seconds = (self.surge_minutes(slot) / time::MINUTES_PER_SECOND) as f32;
        let closing = self.has_talent(slot, Talent::ClosingSurge);
        let patients = self.patients_of(slot);
        let mut covered: Vec<u32> = Vec::new();
        if self.has_talent(slot, Talent::MassSurge) {
            let t = shipdesign::TILE as f32;
            let room = &self.aboard.room;
            for &p in &patients {
                let at = room.bim_pos(p as usize);
                for other in 0..self.aboard.crew_count() {
                    if other != slot
                        && !patients.contains(&other)
                        && !covered.contains(&other)
                        && room.is_alive(other as usize)
                        && !room.is_outside(other as usize)
                        && (room.bim_pos(other as usize) - at).len() <= class::MASS_SURGE_TILES * t
                    {
                        covered.push(other);
                    }
                }
            }
        }
        self.medic_mut(slot as usize).charge = 0.0;
        let room = &mut self.aboard.room;
        room.set_surge(slot as usize, seconds, false);
        for p in patients {
            room.set_surge(p as usize, seconds, closing);
        }
        for other in covered {
            room.set_surge(other as usize, seconds, false);
        }
        Ok(())
    }

    /// Crew member `who` made a **field medic** with nothing else about
    /// the world touched, for a probe and for `BIMS_FIELD_MEDIC=n` in
    /// the app (feature 86): the contract written as a hire writes one,
    /// the kits put in its pack, and no money taken — the fight the
    /// probe wants to look at is what it does, not what it cost. False
    /// with no such crew member, or with one that is hired already.
    pub fn field_medic_for_probe(&mut self, who: u32) -> bool {
        if who >= self.aboard.crew_count() || self.hired.iter().any(|h| h.who == who) {
            return false;
        }
        self.hired.push(Hired {
            who,
            fee: 0,
            due: self.clock_minutes + mercenary::MONTH,
            owed: false,
            medic: true,
        });
        self.give_field_medic_kit(who);
        true
    }

    /// Crew member `carrier` with crew member `patient` in its arms, the
    /// patient taken out cold first so there is something to carry and
    /// the two stood beside each other — `BIMS_CARRY=1`, for looking at
    /// a body being carried off the deck without staging a fight and
    /// waiting for somebody to go down. False if it would not go.
    pub fn carry_for_probe(&mut self, carrier: u32, patient: u32) -> bool {
        if carrier >= self.aboard.crew_count() || patient >= self.aboard.crew_count() {
            return false;
        }
        // Beside the carrier, and bled past the line so it is out cold
        // and worth fetching.
        let at = self.aboard.room.bim_pos(carrier as usize)
            + bims::math::vec2(shipdesign::TILE as f32 * 0.8, 0.0);
        self.aboard.room.put_for_probe(patient as usize, at);
        self.aboard
            .room
            .wound(patient as usize, bims::health::Part::Legs, 1000.0);
        self.aboard.room.set_blood_for_probe(patient as usize, 0.2);
        self.step(&[]);
        self.aboard.room.take_up(carrier as usize, patient as usize)
    }

    /// Slot 0 a medic beaming crew member 1, for a probe and for
    /// `BIMS_BEAM` in the app: crew member 1 stood a tile from it with a
    /// wound open — so the patient wants holding and the charge fills —
    /// and the link made. With `surge` the charge is filled and the
    /// surge triggered besides, which wants the medic at
    /// [`class::SURGE_LEVEL`] (the caller's `BIMS_LEVEL`, or this puts
    /// it there). `false`, and nothing moved, with fewer than two aboard
    /// or with slot 0 no medic.
    pub fn beam_for_probe(&mut self, surge: bool) -> bool {
        if self.aboard.crew_count() < 2 || !self.is_medic(0) {
            return false;
        }
        let at = self.aboard.room.bim_pos(0) + bims::math::vec2(shipdesign::TILE as f32, 0.0);
        self.aboard.room.put_for_probe(1, at);
        self.aboard.room.wound(1, bims::health::Part::Legs, 2.0);
        self.step(&[]);
        if self.beam(0, Some(1)).is_err() {
            return false;
        }
        if surge {
            if self.progress_of(0).level() < class::SURGE_LEVEL {
                let want = class::LEVEL_XP[class::SURGE_LEVEL as usize - 1];
                let mut events = Vec::new();
                self.award(0, want, &mut events);
            }
            let charge = self.surge_charge_wanted(0);
            self.medic_mut(0).charge = charge;
            if self.surge(0).is_err() {
                return false;
            }
        }
        true
    }

    /// Every beam broken: what a change of crew indices does, since an
    /// index is all a link is.
    fn clear_beams(&mut self) {
        for (who, medic) in self.medics.iter_mut().enumerate() {
            medic.unlink();
            self.aboard.room.set_beaming(who, false);
        }
    }

    /// Before the rooms step: every beam checked — broken where the room
    /// ended it (an order to an errand, the medic down), the medic unfit
    /// to act, or a patient dead, gone from the room, out of range or
    /// out of sight — the surge charged for a patient that qualifies,
    /// and the room told what each body is held by
    /// (`bims::health::Beamed`) and what each Bim's doctoring runs at
    /// (`bims::health::Doctoring`).
    fn hand_the_room_the_medics(&mut self, events: &mut Vec<WorldEvent>) {
        let crew = self.aboard.crew_count() as usize;
        if self.medics.len() < crew {
            self.medics.resize(crew, Medic::default());
        }
        let mut held: Vec<Option<bims::health::Beamed>> = vec![None; crew];
        for m in 0..crew {
            if !self.medics[m].is_linked() {
                continue;
            }
            let who = m as u32;
            let was = self.medics[m].patients.clone();
            let keep: Vec<u32> = if !self.aboard.room.is_beaming(m) || !self.fit_to_act(who) {
                Vec::new()
            } else {
                was.iter()
                    .copied()
                    .filter(|&p| self.beam_reaches(who, p).is_ok())
                    .collect()
            };
            if keep.is_empty() {
                self.medics[m].unlink();
                self.aboard.room.set_beaming(m, false);
                events.push(WorldEvent::Beamed { who, patient: None });
                continue;
            }
            self.medics[m].patients = keep.clone();
            // The charge fills while a patient wants holding.
            let room = &self.aboard.room;
            let qualifies = keep.iter().any(|&p| {
                room.blood(p as usize) < bims::health::MAX_BLOOD || room.bleeding(p as usize) > 0
            });
            if qualifies {
                let wanted = self.surge_charge_wanted(who);
                let medic = &mut self.medics[m];
                medic.charge = (medic.charge + data::STEP_MINUTES).min(wanted);
            }
            let beamed = bims::health::Beamed {
                blood_an_hour: self.beam_blood(who),
                mend: if self.progress_of(who).level() >= class::MENDER_LEVEL {
                    class::MENDER_RECOVER
                } else {
                    1.0
                },
                bleed: 0.0,
            };
            for &p in &keep {
                // Two beams on one body: the stronger holds it.
                let slot = &mut held[p as usize];
                if slot.is_none_or(|h| h.blood_an_hour < beamed.blood_an_hour) {
                    *slot = Some(beamed);
                }
            }
            if self.has_talent(who, Talent::SelfCare) && held[m].is_none() {
                held[m] = Some(bims::health::Beamed::HELD);
            }
        }
        // *Hold fast* (feature 77): a taunting tank's wounds and traumas
        // do not bleed while it runs. Nothing else a beam does — the
        // same entry *self-care* gives a medic.
        for who in 0..crew {
            if held[who].is_none()
                && self.has_talent(who as u32, Talent::HoldFast)
                && self.is_taunting(who as u32)
            {
                held[who] = Some(bims::health::Beamed::HELD);
            }
        }
        // *Steady ranks* (feature 78): a Bim in a commander's aura with
        // that talent bleeds slower — slower, not not at all, so it is
        // the same entry with the bleeding only damped, and anything
        // that stops the bleeding outright keeps its place.
        for who in 0..crew {
            if held[who].is_some() {
                continue;
            }
            let Some(aura) = self.aura_reaching(who as u32) else {
                continue;
            };
            if aura.bleed < 1.0 {
                held[who] = Some(bims::health::Beamed {
                    blood_an_hour: 0.0,
                    mend: 1.0,
                    bleed: aura.bleed,
                });
            }
        }
        let doctoring: Vec<bims::health::Doctoring> = (0..crew as u32)
            .map(|who| {
                let mut d = bims::health::Doctoring::NONE;
                if !self.is_medic(who) {
                    return d;
                }
                if self.has_talent(who, Talent::FieldDressing) {
                    d.bandage = 1.0 / class::FIELD_DRESSING_TIME;
                }
                if self.has_talent(who, Talent::Surgeon) {
                    d.treat = 1.0 / class::SURGEON_TIME;
                }
                if self.has_talent(who, Talent::FieldSurgeon)
                    && !self.medic_of(who).field_surgery_used
                {
                    d.bare = Some(1.0 / class::FIELD_SURGEON_TIME);
                }
                d.clean_hands = self.has_talent(who, Talent::CleanHands);
                if self.has_talent(who, Talent::SteadyHandsMedic) {
                    d.treated_to = bims::health::TREATED_TO * class::STEADY_HANDS_TREATED;
                }
                d
            })
            .collect();
        self.aboard.room.set_held(held);
        self.aboard.room.set_doctoring(doctoring);
    }

    // --- the field medics (feature 86) -------------------------------------

    /// Whether that crew member is a hired **field medic**: a mercenary
    /// taken on for the job of fetching the fallen out of the fire and
    /// treating them, with none of the medic class's talents. It is the
    /// contract that says so ([`mercenary::Hired::medic`]), not the
    /// body, so a hand let go and hired again by somebody else is a
    /// field medic to them too.
    pub fn is_field_medic(&self, who: u32) -> bool {
        self.hired.iter().any(|h| h.who == who && h.medic)
    }

    /// Every crew member's trade said to the room, every step: the room
    /// reads it in [`bims::game::Game::bot_stand`] and in the stand it
    /// picks, and keeps none of it in a save.
    fn hand_the_room_the_field_medics(&mut self) {
        for who in 0..self.aboard.crew_count() {
            let medic = self.is_field_medic(who);
            self.aboard.room.set_field_medic(who as usize, medic);
        }
    }

    /// A hired field medic fills its pack back up out of the hold, one
    /// kit a step (feature 86): **out of combat** — the room's own
    /// `calm`, twenty seconds with nothing fired and the enemy out of
    /// sight and out of reach — or, whatever the fight is doing,
    /// **once it has spent its last kit** and is standing somewhere
    /// clear of it, which is where the carry takes it anyway.
    ///
    /// It is bookkeeping and not an errand: the medic is aboard with a
    /// cabinet to hand, and the room is already handed the hold's
    /// medicine every step without anybody walking for it. What it will
    /// not do is conjure one — the hold has to have a medkit free — so
    /// a crew out of medkits has a medic out of medkits.
    fn restock_field_medics(&mut self) {
        if self.hired.iter().all(|h| !h.medic) {
            return;
        }
        let wanted = Item::Stack(ResourceId::Medkit as u32);
        let calm = self.aboard.room.calm();
        for i in 0..self.hired.len() {
            let hired = self.hired[i];
            if !hired.medic || hired.who >= self.aboard.crew_count() {
                continue;
            }
            let who = hired.who as usize;
            if !self.aboard.room.is_alive(who)
                || self.aboard.room.is_unconscious(who)
                || self.aboard.room.is_outside(who)
            {
                continue;
            }
            let carried = self
                .aboard
                .room
                .pack(who)
                .iter()
                .filter(|item| **item == Some(wanted))
                .count() as u32;
            if carried >= mercenary::MEDIC_MEDKITS {
                continue;
            }
            if !calm && !(carried == 0 && self.aboard.room.is_out_of_harm(who)) {
                continue;
            }
            if self.free(ResourceId::Medkit) == 0 {
                continue;
            }
            let Some(cell) = self.aboard.room.gear(who).free_cell_for(wanted) else {
                continue;
            };
            if !self.aboard.room.give(who, Some(cell), wanted) {
                continue;
            }
            // Out of the hold, the count and the grid together, the way
            // a fetch takes one.
            self.ship.design.cargo[ResourceId::Medkit as usize] =
                self.ship.design.cargo[ResourceId::Medkit as usize].saturating_sub(1);
            let class = storage(ResourceId::Medkit);
            if let Some(grid) = self.grid_mut(class) {
                grid.remove(ResourceId::Medkit, 1, None);
            }
            self.on_ship_changed();
        }
    }

    // --- the dressings a crew member carries (feature 87) ------------------

    /// **Every** crew member fills its pack back up to the Management
    /// tab's number out of the hold, one dressing a step (feature 87) —
    /// the player's own Bim as well as the bots, since a bandage is spent
    /// out of the pack of whoever winds it and the player's hands need
    /// one as much as anybody's. **Out of combat only**, the room's own
    /// `calm`: twenty seconds with nothing fired, the enemy out of sight
    /// and out of reach. Nobody walks for it — it is bookkeeping, like
    /// the hold's medicine handed over every step — and it conjures
    /// nothing: a hold with no bandages in it restocks nobody, which is
    /// what the drug lab is for.
    fn restock_bandages(&mut self) {
        let wanted = self.aboard.room.target(bims::manager::Stock::Bandages);
        if wanted == 0 || !self.aboard.room.calm() {
            return;
        }
        for who in 0..self.aboard.crew_count() as usize {
            if !self.aboard.room.is_alive(who)
                || self.aboard.room.is_unconscious(who)
                || self.aboard.room.is_outside(who)
                || self.aboard.room.bandages_of(who) >= wanted
                || self.free(ResourceId::Bandage) == 0
            {
                continue;
            }
            if self.aboard.room.give_stack(who, bims::game::BANDAGE, 1) == 0 {
                continue;
            }
            // Out of the hold, the count and the grid together, the way
            // a fetch takes one.
            self.ship.design.cargo[ResourceId::Bandage as usize] =
                self.ship.design.cargo[ResourceId::Bandage as usize].saturating_sub(1);
            let class = storage(ResourceId::Bandage);
            if let Some(grid) = self.grid_mut(class) {
                grid.remove(ResourceId::Bandage, 1, None);
            }
            self.on_ship_changed();
        }
    }

    /// Whether a crew member may carry at all (feature 86): a medic of
    /// the class, or a hired field medic. A commander's hands are as
    /// good as a medic's, but the carry is the medic's trade and the
    /// user asked for it as one.
    pub fn can_lift(&self, who: u32) -> bool {
        self.class_of(who) == Class::Medic || self.is_field_medic(who)
    }

    /// Whom that crew member has in its arms, if anybody — the room's
    /// own answer, for the app's picture and for the tests.
    pub fn carrying_of(&self, who: u32) -> Option<u32> {
        self.aboard.room.carrying(who as usize).map(|p| p as u32)
    }

    /// Every crewmate `who` could pick up from where it stands (feature
    /// 86), lowest index first: what the carry's box counts, and what
    /// the deck rings while the pointer rests on it. Empty for anybody
    /// that is not a medic of some kind.
    pub fn carryable_near(&self, who: u32) -> Vec<u32> {
        if !self.can_lift(who) {
            return Vec::new();
        }
        (0..self.aboard.crew_count())
            .filter(|&p| self.can_carry(who, p).is_ok())
            .collect()
    }

    /// Whether `who` could take `patient` up this instant — see
    /// [`Command::Carry`]. The refusals in order: not a medic of any
    /// kind ([`Refusal::NotCarrying`]), unfit to act or out of reach
    /// ([`Refusal::OutOfReach`]), not a crewmate or itself
    /// ([`Refusal::NotACrewmate`]), in somebody's arms already
    /// ([`Refusal::AlreadyCarried`]), and a body that wants no carrying
    /// ([`Refusal::NotHurt`]). What the app greys the key with, and what
    /// the command asks again when it lands.
    pub fn can_carry(&self, who: u32, patient: u32) -> Result<(), Refusal> {
        if !self.can_lift(who) {
            return Err(Refusal::NotCarrying);
        }
        if !self.fit_to_act(who) {
            return Err(Refusal::OutOfReach);
        }
        if patient == who || patient >= self.aboard.crew_count() {
            return Err(Refusal::NotACrewmate);
        }
        let room = &self.aboard.room;
        if room.is_carried(patient as usize) {
            return Err(Refusal::AlreadyCarried);
        }
        if !room.needs_rescue(patient as usize) {
            return Err(Refusal::NotHurt);
        }
        if !room.can_take_up(who as usize, patient as usize) {
            return Err(Refusal::OutOfReach);
        }
        Ok(())
    }

    /// The carry — see [`Command::Carry`]: the body taken up, or
    /// whatever is in the arms set down. Hands back whom it picked up,
    /// `None` for a set down; a set down with empty arms is
    /// [`Refusal::NotCarrying`], so the key says something either way.
    fn carry(&mut self, who: u32, patient: Option<u32>) -> Result<Option<u32>, Refusal> {
        let Some(patient) = patient else {
            if !self.can_lift(who) {
                return Err(Refusal::NotCarrying);
            }
            return match self.aboard.room.set_down(who as usize) {
                Some(_) => Ok(None),
                None => Err(Refusal::NotCarrying),
            };
        };
        // Pressed on the one already in its arms: that is a set down.
        if self.carrying_of(who) == Some(patient) {
            self.aboard.room.set_down(who as usize);
            return Ok(None);
        }
        self.can_carry(who, patient)?;
        if !self.aboard.room.take_up(who as usize, patient as usize) {
            return Err(Refusal::OutOfReach);
        }
        Ok(Some(patient))
    }

    /// Every carry let go: the crew's indices are about to move — a
    /// hire, a dismissal — and an index is the whole of what an arm
    /// holds, exactly as a beam's link is.
    fn clear_carries(&mut self) {
        for who in 0..self.aboard.crew_count() as usize {
            self.aboard.room.set_down(who);
        }
    }

    /// After the rooms step: every dressing and treatment the room
    /// finished — [`class::XP_HEALED`] to a medic that did one on a
    /// crewmate, and a field surgery marked used — and the field surgery
    /// given back when the fight ends (the rooms unjoined, or no enemy
    /// standing), like the soldiers' *rampage*.
    fn settle_medics(&mut self, events: &mut Vec<WorldEvent>) {
        for healed in self.aboard.room.take_healings() {
            if healed.with == bims::game::Healing::Bare {
                self.medic_mut(healed.helper).field_surgery_used = true;
            }
            if healed.helper != healed.patient && self.is_medic(healed.helper as u32) {
                self.award(healed.helper, class::XP_HEALED, events);
            }
        }
        if !self.enemy_standing() {
            for medic in &mut self.medics {
                medic.field_surgery_used = false;
            }
        }
    }

    // --- the tank: the wall, the taunt and the hits (feature 77) -----------
    //
    // `crate::tank` is the state — when he last taunted, and nothing
    // else. Bulwark is a flag on the Bim, the hits a count on it, and
    // every talent is read afresh each step into the room's one
    // `bims::combat::Skill`.

    /// Whether crew member `who` is a player's tank.
    fn is_tank(&self, who: u32) -> bool {
        self.class_of(who) == Class::Tank
    }

    /// A crew member's tank state — an empty one for anybody the world
    /// keeps none for.
    pub fn tank_of(&self, who: u32) -> Tank {
        self.tanks.get(who as usize).cloned().unwrap_or_default()
    }

    /// The tank's state, made if the crew grew past the list.
    fn tank_mut(&mut self, who: usize) -> &mut Tank {
        if self.tanks.len() <= who {
            self.tanks.resize(who + 1, Tank::default());
        }
        &mut self.tanks[who]
    }

    /// What a tank fights with: the armour passive is `skill_of`'s, and
    /// this is the rest of the talents — the wall's pace and dodge, the
    /// plating, the iron frame, the unmoving legs and the breacher's
    /// shoulder.
    fn tank_skill(&self, who: u32) -> bims::combat::Skill {
        let mut skill = bims::combat::Skill::NONE;
        let progress = self.progress_of(who);
        let has = |talent| progress.has(Class::Tank, talent);
        if self.is_bulwark(who) {
            skill.walk = self.bulwark_pace(who);
            if has(Talent::Guarded) {
                skill.dodge = class::GUARDED_DODGE;
            }
        }
        if has(Talent::Plated) {
            skill.armour_protection = class::PLATED_PROTECTION;
        }
        if has(Talent::Breacher) {
            skill.smash_rate = 1.0 / class::BREACHER_TIME;
        }
        if has(Talent::Unmovable) {
            skill.nerve = true;
            skill.steady_pace = true;
        }
        if progress.level() >= class::IRON_FRAME_LEVEL {
            skill.iron_frame = true;
        }
        skill
    }

    /// What a crew member's worn armour drains at, of the damage that
    /// gets past its protection: [`class::TANK_DRAIN`] for a tank —
    /// *fortress* again on top — [`class::RALLYING_WALL_DRAIN`] for a
    /// crewmate within [`class::RALLYING_WALL_TILES`] of a taunting tank
    /// with *rallying wall*, and one for everybody else.
    pub fn armour_drain(&self, who: u32) -> f32 {
        if self.is_tank(who) {
            let mut drain = class::TANK_DRAIN;
            if self.has_talent(who, Talent::Fortress) {
                drain *= class::FORTRESS_DRAIN;
            }
            return drain;
        }
        let crew = self.aboard.crew_count();
        let room = &self.aboard.room;
        if who >= crew || !room.is_alive(who as usize) || room.is_outside(who as usize) {
            return 1.0;
        }
        let at = room.bim_pos(who as usize);
        let t = shipdesign::TILE as f32;
        let sheltered = (0..crew).any(|tank| {
            tank != who
                && self.is_taunting(tank)
                && self.has_talent(tank, Talent::RallyingWall)
                && !room.is_outside(tank as usize)
                && (room.bim_pos(tank as usize) - at).len() <= class::RALLYING_WALL_TILES * t
        });
        if sheltered {
            class::RALLYING_WALL_DRAIN
        } else {
            1.0
        }
    }

    /// How far a tank's bulwark reaches, in tiles: the reach, twice that
    /// with *wide wall*.
    pub fn bulwark_reach(&self, who: u32) -> f32 {
        if self.has_talent(who, Talent::WideWall) {
            class::BULWARK_REACH * class::WIDE_WALL_REACH
        } else {
            class::BULWARK_REACH
        }
    }

    /// What a tank's pace is multiplied by while the wall is up: half,
    /// half again as much again with *fast wall*.
    pub fn bulwark_pace(&self, who: u32) -> f32 {
        if self.has_talent(who, Talent::FastWall) {
            class::BULWARK_PACE * class::FAST_WALL_PACE
        } else {
            class::BULWARK_PACE
        }
    }

    /// How big a load a crew member's haul trip carries, in units:
    /// [`data::HAUL_LOAD`], [`class::PACK_MULE_LOADS`] of it with *pack
    /// mule*. Read as the load is taken, so it is the hauler's own.
    pub fn haul_load(&self, who: u32) -> u32 {
        if self.has_talent(who, Talent::PackMule) {
            data::HAUL_LOAD * class::PACK_MULE_LOADS
        } else {
            data::HAUL_LOAD
        }
    }

    /// Whether a crew member stands as a wall.
    pub fn is_bulwark(&self, who: u32) -> bool {
        self.aboard.room.is_bulwark(who as usize)
    }

    /// Whether a player's tank may stand as a wall, or why not: a tank
    /// (`NotATank`), and fit to act (`OutOfReach`). What the app greys
    /// the key with and [`Command::Bulwark`] asks.
    pub fn can_bulwark(&self, slot: u32) -> Result<(), Refusal> {
        if !class::can(self.class_of(slot), class::Ability::Bulwark) {
            return Err(Refusal::NotATank);
        }
        if !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        Ok(())
    }

    /// Stand as a wall, or stand down — see [`Command::Bulwark`]. The
    /// room holds the flag (`Game::set_bulwark`) and drops it when the
    /// tank goes down.
    fn bulwark(&mut self, slot: u32, on: bool) -> Result<(), Refusal> {
        self.can_bulwark(slot)?;
        self.aboard.room.set_bulwark(slot as usize, on);
        Ok(())
    }

    /// Minutes of the clock a tank's taunt runs: [`class::TAUNT_MINUTES`],
    /// half again with *long taunt*.
    pub fn taunt_minutes(&self, who: u32) -> f64 {
        if self.has_talent(who, Talent::LongTaunt) {
            class::TAUNT_MINUTES * class::LONG_TAUNT_TIME
        } else {
            class::TAUNT_MINUTES
        }
    }

    /// How far a tank's taunt reaches, in tiles: the radius, half again
    /// with *loud taunt*.
    pub fn taunt_radius(&self, who: u32) -> f32 {
        if self.has_talent(who, Talent::LoudTaunt) {
            class::TAUNT_RADIUS * class::LOUD_TAUNT_RADIUS
        } else {
            class::TAUNT_RADIUS
        }
    }

    /// Minutes of the clock a tank's taunt has left; nought with none
    /// running.
    pub fn taunt_left(&self, who: u32) -> f64 {
        let Some(last) = self.tank_of(who).last_taunt else {
            return 0.0;
        };
        (self.taunt_minutes(who) - (self.clock_minutes - last)).max(0.0)
    }

    /// Whether a taunt is running on a crew member.
    pub fn is_taunting(&self, who: u32) -> bool {
        self.taunt_left(who) > 0.0
    }

    /// Seconds of the clock until a tank may taunt again; nought when he
    /// may. Read the way the grenade's cooldown is.
    pub fn taunt_cooldown_left(&self, who: u32) -> f64 {
        let Some(last) = self.tank_of(who).last_taunt else {
            return 0.0;
        };
        let since = (self.clock_minutes - last) / time::MINUTES_PER_SECOND;
        (class::TAUNT_COOLDOWN - since).max(0.0)
    }

    /// Whether a player's tank may taunt, or why not, in order: a tank
    /// (`NotATank`), fit to act (`OutOfReach`), at
    /// [`class::TAUNT_LEVEL`] (`NoTauntYet`), and out of the cooldown
    /// (`CoolingDown`).
    pub fn can_taunt(&self, slot: u32) -> Result<(), Refusal> {
        if !class::can(self.class_of(slot), class::Ability::Taunt) {
            return Err(Refusal::NotATank);
        }
        if !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        if self.progress_of(slot).level() < class::TAUNT_LEVEL {
            return Err(Refusal::NoTauntYet);
        }
        if self.taunt_cooldown_left(slot) > 0.0 {
            return Err(Refusal::CoolingDown);
        }
        Ok(())
    }

    /// The taunt — see [`Command::Taunt`]: the clock noted, which is the
    /// whole of it. What it *does* is read off that every step, in
    /// `hand_the_room_the_tanks`.
    fn taunt(&mut self, slot: u32) -> Result<(), Refusal> {
        self.can_taunt(slot)?;
        let now = self.clock_minutes;
        self.tank_mut(slot as usize).last_taunt = Some(now);
        Ok(())
    }

    /// Before the rooms step: the walls standing among the crew to the
    /// crew's room, so a bolt aimed through one is dodged like a bolt in
    /// cover (and, with *interpose*, lands on the wall). A tank not fit
    /// to act shelters nobody, so a wall goes down with the tank the
    /// same step he does.
    fn hand_the_room_the_tanks(&mut self) {
        let crew = self.aboard.crew_count();
        if self.tanks.len() < crew as usize {
            self.tanks.resize(crew as usize, Tank::default());
        }
        let walls: Vec<bims::combat::Bulwark> = (0..crew)
            .filter(|&who| self.is_tank(who) && self.is_bulwark(who) && self.fit_to_act(who))
            .map(|who| bims::combat::Bulwark {
                who: who as usize,
                reach: self.bulwark_reach(who),
                interpose: self.has_talent(who, Talent::Interpose),
            })
            .collect();
        self.aboard.room.set_bulwarks(walls);
    }

    /// After the rooms step: every whole point of experience the enemy's
    /// fire has made for a tank — [`class::TANK_HITS_PER_XP`] hits each,
    /// the remainder left on the Bim to count on from.
    fn settle_tanks(&mut self, events: &mut Vec<WorldEvent>) {
        for who in 0..self.aboard.crew_count() {
            if !self.is_tank(who) {
                continue;
            }
            let hits = self.aboard.room.hits_taken(who as usize);
            let points = hits / class::TANK_HITS_PER_XP;
            if points == 0 {
                continue;
            }
            self.aboard
                .room
                .set_hits_taken(who as usize, hits % class::TANK_HITS_PER_XP);
            self.award(who as usize, points, events);
        }
    }

    // --- the commander: the aura, the squad and the rally (feature 78) -----
    //
    // `crate::commander` is the state — when each commander last
    // rallied, and the one squad order — and this is the rules. The
    // aura is not kept at all: it is worked out every step from where
    // the commanders stand and goes to the room through `skill_of`.
    //
    // **Whom each half reaches.** The aura and the rally lift every
    // friendly Bim in range, a player's own steered Bim included; a
    // squad order commands only the squad, which is every crew member
    // no player is steering.

    /// Whether crew member `who` is a player's commander.
    fn is_commander(&self, who: u32) -> bool {
        self.class_of(who) == Class::Commander
    }

    /// A crew member's commander state — an empty one for anybody the
    /// world keeps none for.
    pub fn commander_of(&self, who: u32) -> Commander {
        self.commanders
            .get(who as usize)
            .cloned()
            .unwrap_or_default()
    }

    /// The commander's state, made if the crew grew past the list.
    fn commander_mut(&mut self, who: usize) -> &mut Commander {
        if self.commanders.len() <= who {
            self.commanders.resize(who + 1, Commander::default());
        }
        &mut self.commanders[who]
    }

    /// Whether a crew member is somebody a player steers: a slot's own
    /// Bim, which no squad order ever touches.
    fn is_steered(&self, who: u32) -> bool {
        who < self.players()
    }

    /// Whether a crew member is on the crew's deck to be reached at all:
    /// alive and not outside in a suit.
    fn on_the_deck(&self, who: u32) -> bool {
        let room = &self.aboard.room;
        who < self.aboard.crew_count()
            && room.is_alive(who as usize)
            && !room.is_outside(who as usize)
    }

    /// How far a commander's aura reaches, in tiles: the radius, half
    /// again with *wide presence*.
    pub fn aura_radius(&self, who: u32) -> f32 {
        if self.has_talent(who, Talent::WidePresence) {
            class::AURA_TILES * class::WIDE_PRESENCE_RADIUS
        } else {
            class::AURA_TILES
        }
    }

    /// What one commander's aura does to a Bim standing in it: every
    /// bonus deepened together by *strong presence* and, while he
    /// stands still, by *anchor*.
    fn aura_cast_by(&self, who: u32) -> Aura {
        let progress = self.progress_of(who);
        let has = |talent| progress.has(Class::Commander, talent);
        let mut factor = 1.0;
        if has(Talent::StrongPresence) {
            factor *= class::STRONG_PRESENCE;
        }
        if has(Talent::Anchor) && self.aboard.room.is_standing_still(who as usize) {
            factor *= class::ANCHOR_BONUS;
        }
        Aura {
            work: class::aura_bonus(class::AURA_WORK, factor),
            aim: class::aura_bonus(class::AURA_AIM, factor),
            nerve: class::aura_bonus(class::AURA_NERVE, factor),
            bleed: if has(Talent::SteadyRanks) {
                class::aura_bonus(class::STEADY_RANKS_BLEED, factor)
            } else {
                1.0
            },
            pace: if has(Talent::DoubleTime) {
                class::aura_bonus(class::DOUBLE_TIME_PACE, factor)
            } else {
                1.0
            },
        }
    }

    /// Whether a commander's aura reaches a crew member: a commander
    /// conscious and on the deck, the Bim on the deck too, and the two
    /// within the aura's radius. Never himself and never an enemy —
    /// the crew's room is the only room asked.
    pub fn in_aura_of(&self, commander: u32, who: u32) -> bool {
        if commander == who || !self.is_commander(commander) || !self.fit_to_act(commander) {
            return false;
        }
        if !self.on_the_deck(who) || !self.on_the_deck(commander) {
            return false;
        }
        let room = &self.aboard.room;
        let gap = room.bim_pos(who as usize) - room.bim_pos(commander as usize);
        gap.len() <= self.aura_radius(commander) * shipdesign::TILE as f32
    }

    /// The strongest aura reaching a crew member, or `None`. **Two
    /// commanders' auras never stack**: the one whose bonuses are
    /// deepest holds it, and the others do nothing.
    pub fn aura_reaching(&self, who: u32) -> Option<Aura> {
        (0..self.aboard.crew_count())
            .filter(|&c| self.in_aura_of(c, who))
            .map(|c| self.aura_cast_by(c))
            .max_by(|a, b| a.work.total_cmp(&b.work))
    }

    /// Minutes of the clock a commander's rally runs:
    /// [`class::RALLY_MINUTES`], half again with *long rally*.
    pub fn rally_minutes(&self, who: u32) -> f64 {
        if self.has_talent(who, Talent::LongRally) {
            class::RALLY_MINUTES * class::LONG_RALLY_TIME
        } else {
            class::RALLY_MINUTES
        }
    }

    /// Seconds of the clock between one rally and the next: halved with
    /// *quick rally*.
    pub fn rally_cooldown(&self, who: u32) -> f64 {
        if self.has_talent(who, Talent::QuickRally) {
            class::RALLY_COOLDOWN * class::QUICK_RALLY_COOLDOWN
        } else {
            class::RALLY_COOLDOWN
        }
    }

    /// Minutes of the clock a commander's rally has left; nought with
    /// none running.
    pub fn rally_left(&self, who: u32) -> f64 {
        let Some(last) = self.commander_of(who).last_rally else {
            return 0.0;
        };
        (self.rally_minutes(who) - (self.clock_minutes - last)).max(0.0)
    }

    /// Whether a rally is running on a commander.
    pub fn is_rallying(&self, who: u32) -> bool {
        self.rally_left(who) > 0.0
    }

    /// Seconds of the clock until he may rally again; nought when he
    /// may. Read the way the taunt's cooldown is.
    pub fn rally_cooldown_left(&self, who: u32) -> f64 {
        let Some(last) = self.commander_of(who).last_rally else {
            return 0.0;
        };
        let since = (self.clock_minutes - last) / time::MINUTES_PER_SECOND;
        (self.rally_cooldown(who) - since).max(0.0)
    }

    /// Whether a rally covers a crew member: one running on a commander
    /// whose aura reaches it — the whole room with *warcry* — or on the
    /// crew member itself, since a commander rallies himself too.
    pub fn rally_reaching(&self, who: u32) -> Option<u32> {
        (0..self.aboard.crew_count()).find(|&c| {
            self.is_rallying(c)
                && self.on_the_deck(who)
                && (c == who
                    || self.in_aura_of(c, who)
                    || (self.has_talent(c, Talent::Warcry) && self.on_the_deck(c)))
        })
    }

    /// Whether a player's commander may rally, or why not, in order: a
    /// commander (`NotACommander`), fit to act (`OutOfReach`), at
    /// [`class::RALLY_LEVEL`] (`NoRallyYet`), and out of the cooldown
    /// (`CoolingDown`).
    pub fn can_rally(&self, slot: u32) -> Result<(), Refusal> {
        if !class::can(self.class_of(slot), class::Ability::Rally) {
            return Err(Refusal::NotACommander);
        }
        if !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        if self.progress_of(slot).level() < class::RALLY_LEVEL {
            return Err(Refusal::NoRallyYet);
        }
        if self.rally_cooldown_left(slot) > 0.0 {
            return Err(Refusal::CoolingDown);
        }
        Ok(())
    }

    /// The rally — see [`Command::Rally`]: the clock noted, which is the
    /// whole of it. What it does is read off that every step, in
    /// `skill_of`.
    fn rally(&mut self, slot: u32) -> Result<(), Refusal> {
        self.can_rally(slot)?;
        let now = self.clock_minutes;
        self.commander_mut(slot as usize).last_rally = Some(now);
        Ok(())
    }

    // --- the two standing orders every player has (feature 84) ------------

    /// Whether a player may give their bots a standing order: fit to act
    /// — alive, awake, aboard — and nothing else. It is not a class's
    /// ability and wants no level, no cooldown and nobody in range:
    /// **attack** and **retreat** are the two commands every player has
    /// from the first step, and `crate::orders` says why.
    pub fn can_order(&self, slot: u32) -> Result<(), Refusal> {
        if !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        Ok(())
    }

    /// The standing order — see [`Command::Orders`]. The same order
    /// given again is the bots back to following, which is what a
    /// second press of the key does; the code that comes back is the
    /// order they are on now, so nought is that release.
    fn give_orders(&mut self, slot: u32, order: Standing) -> Result<u32, Refusal> {
        self.can_order(slot)?;
        let t = shipdesign::TILE as f32;
        if let Standing::Attack { tile } = order
            && !self.aboard.room.is_banner_tile(bims::math::vec2(
                (tile.0 as f32 + 0.5) * t,
                (tile.1 as f32 + 0.5) * t,
            ))
        {
            return Err(Refusal::NoGroundThere);
        }
        let players = self.players() as usize;
        if self.standing.len() < players {
            self.standing.resize(players, Standing::Follow);
        }
        let Some(mine) = self.standing.get_mut(slot as usize) else {
            return Err(Refusal::OutOfReach);
        };
        *mine = if mine.same_as(order) {
            Standing::Follow
        } else {
            order
        };
        Ok(mine.code())
    }

    /// What player `slot`'s bots are under, for the app and the tests.
    pub fn standing_of(&self, slot: u32) -> Standing {
        self.standing
            .get(slot as usize)
            .copied()
            .unwrap_or(Standing::Follow)
    }

    /// Before the rooms step: a standing order dropped where the player
    /// who gave it is no longer fit to act — down, asleep or outside,
    /// the same gate the order was taken under — and then handed to the
    /// room, one `bims::game::Standing` a player slot, an attack's tile
    /// turned into the room's own units.
    fn hand_the_room_the_standing(&mut self) {
        let players = self.players() as usize;
        if self.standing.len() != players {
            self.standing.resize(players, Standing::Follow);
        }
        for slot in 0..players {
            if !self.fit_to_act(slot as u32) {
                self.standing[slot] = Standing::Follow;
            }
        }
        let t = shipdesign::TILE as f32;
        let said: Vec<bims::game::Standing> = self
            .standing
            .iter()
            .map(|order| match *order {
                Standing::Follow => bims::game::Standing::Follow,
                Standing::Retreat => bims::game::Standing::Retreat,
                Standing::Attack { tile } => bims::game::Standing::Attack {
                    at: bims::math::vec2((tile.0 as f32 + 0.5) * t, (tile.1 as f32 + 0.5) * t),
                },
            })
            .collect();
        self.aboard.room.set_standing(said);
        // And where the **ship's** own gangway is, which is where a
        // retreat goes. The room cannot work that out for itself on a
        // joined deck: `Room::gangway` there is the joined design's
        // first free airlock, and the ship's own is mated to the
        // station, so the room's answer is the station's far door —
        // which is what used to walk a retreat out through the building
        // instead of home. `Aboard::gangway` is the right spot, in room
        // units, and `None` for a ship flying on its own, where the
        // room's own answer is right.
        let home = self
            .aboard
            .gangway
            .map(|at| bims::math::vec2(at.x as f32, at.y as f32));
        self.aboard.room.set_home(home);
    }

    /// Where a fall back gathers, in the crew's room's units (feature
    /// 84): the deck just inside the ship's own airlock. What the app
    /// draws the defend sign on.
    pub fn fall_back_point(&self) -> (f32, f32) {
        let at = self.aboard.room.fall_back_point();
        (at.x, at.y)
    }

    /// What a commander's aura and rally do to a crew member's
    /// shooting, working and nerve, over whatever its own class gave
    /// it. Everything here reaches a player's own steered Bim as
    /// readily as a bot.
    fn lift_by_aura(&self, who: u32, skill: &mut bims::combat::Skill) {
        if let Some(aura) = self.aura_reaching(who) {
            skill.accuracy *= aura.aim;
            skill.effort *= aura.work;
            skill.walk *= aura.pace;
            skill.nerve_hold = class::NERVE_HOLD * aura.nerve;
        }
        if let Some(commander) = self.rally_reaching(who) {
            skill.accuracy *= class::RALLY_AIM;
            skill.nerve = true;
            if self.has_talent(commander, Talent::Grit) {
                skill.unhurt = true;
            }
        }
        // *Focus fire*: the squad's odds against the enemy its order
        // marked, and against nobody else.
        if let Some(order) = &self.squad
            && order.has(who)
            && matches!(order.kind, SquadKind::Attack { .. })
            && self.has_talent(order.by_slot, Talent::FocusFire)
        {
            skill.marked_accuracy = class::FOCUS_FIRE_ACCURACY;
        }
        // A squad member standing its ground never runs.
        if let Some(order) = &self.squad
            && order.has(who)
            && matches!(order.kind, SquadKind::StandGround)
        {
            skill.nerve = true;
        }
    }

    /// How far a commander's squad orders reach, in tiles: the range,
    /// and the whole room from [`class::LONG_REACH_LEVEL`] (*long
    /// reach*).
    pub fn squad_range(&self, who: u32) -> f32 {
        if self.progress_of(who).level() >= class::LONG_REACH_LEVEL {
            f32::MAX
        } else {
            class::SQUAD_RANGE
        }
    }

    /// Whether a player's commander may send the squad, or why not: a
    /// commander (`NotACommander`) and fit to act (`OutOfReach`).
    pub fn can_squad(&self, slot: u32) -> Result<(), Refusal> {
        if !class::can(self.class_of(slot), class::Ability::SquadOrder) {
            return Err(Refusal::NotACommander);
        }
        if !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        Ok(())
    }

    /// Who a commander's order would reach: every crew member no player
    /// is steering, alive and on the deck, within his reach — lowest
    /// index first, which is the order an attack splits the squad by.
    pub fn squad_members(&self, slot: u32) -> Vec<u32> {
        if !self.on_the_deck(slot) {
            return Vec::new();
        }
        let room = &self.aboard.room;
        let at = room.bim_pos(slot as usize);
        let reach = self.squad_range(slot);
        let t = shipdesign::TILE as f32;
        (0..self.aboard.crew_count())
            .filter(|&who| {
                !self.is_steered(who)
                    && self.on_the_deck(who)
                    && (reach == f32::MAX || (room.bim_pos(who as usize) - at).len() <= reach * t)
            })
            .collect()
    }

    /// Whether that resident of the station alongside is an enemy still
    /// standing: the rooms joined, the station hostile, and the body
    /// alive and on its feet.
    fn enemy_standing_at(&self, enemy: u32) -> bool {
        let Some(residents) = &self.residents else {
            return false;
        };
        if self.stance(residents.station) != Stance::Hostile || !self.aboard.is_joined() {
            return false;
        }
        let room = &residents.aboard.room;
        enemy < residents.aboard.count()
            && room.is_alive(enemy as usize)
            && !room.is_unconscious(enemy as usize)
    }

    /// Whether that resident is dead — what ends a *relentless* mark.
    fn enemy_dead_at(&self, enemy: u32) -> bool {
        let Some(residents) = &self.residents else {
            return true;
        };
        enemy >= residents.aboard.count() || !residents.aboard.room.is_alive(enemy as usize)
    }

    /// The order a player's ask comes out as: an attack's enemy checked
    /// and, with *pincer*, added beside the one already marked; a fall
    /// back's tile the one named or, for a tile that is not deck of the
    /// room, the commander's own.
    fn squad_kind_of(&self, slot: u32, ask: SquadAsk) -> Result<SquadKind, Refusal> {
        Ok(match ask {
            SquadAsk::Attack { enemy } => {
                if !self.enemy_standing_at(enemy) {
                    return Err(Refusal::NoEnemyThere);
                }
                let mut enemies = vec![enemy];
                if self.has_talent(slot, Talent::Pincer)
                    && let Some(order) = &self.squad
                    && order.by_slot == slot
                    && let SquadKind::Attack { enemies: on } = &order.kind
                    && !on.contains(&enemy)
                {
                    let mut both = on.clone();
                    both.push(enemy);
                    while both.len() > class::PINCER_MARKS {
                        both.remove(0);
                    }
                    enemies = both;
                }
                SquadKind::Attack { enemies }
            }
            SquadAsk::FallBack { tile } => {
                let t = shipdesign::TILE as f32;
                let deck = tile.filter(|&(x, y)| {
                    self.aboard
                        .room
                        .is_deck_tile(bims::math::vec2((x as f32 + 0.5) * t, (y as f32 + 0.5) * t))
                });
                SquadKind::FallBack {
                    tile: deck.unwrap_or_else(|| {
                        let at = self.aboard.room.bim_pos(slot as usize);
                        ((at.x / t).floor() as i32, (at.y / t).floor() as i32)
                    }),
                }
            }
            SquadAsk::StandGround => SquadKind::StandGround,
        })
    }

    /// A squad order — see [`Command::Squad`]. `Ok(None)` is the same
    /// order given again, which releases the squad.
    fn squad_order(&mut self, slot: u32, ask: SquadAsk) -> Result<Option<SquadKind>, Refusal> {
        self.can_squad(slot)?;
        let kind = self.squad_kind_of(slot, ask)?;
        if let Some(order) = &self.squad
            && order.by_slot == slot
            && order.kind.same_as(&kind)
        {
            self.squad = None;
            return Ok(None);
        }
        let members = self.squad_members(slot);
        if members.is_empty() {
            return Err(Refusal::NoSquadInRange);
        }
        self.squad = Some(SquadOrder {
            by_slot: slot,
            kind: kind.clone(),
            members,
        });
        Ok(Some(kind))
    }

    /// The squad order called off, whatever it was: what a hire, a
    /// dismissal and an unjoin do, since the members are crew indices
    /// and the marks are residents'.
    fn clear_squad(&mut self) {
        self.squad = None;
    }

    /// One crew member out of the squad order until the next one: what
    /// its own player's click order does to it.
    fn take_out_of_squad(&mut self, who: u32) {
        if let Some(order) = &mut self.squad {
            order.members.retain(|&m| m != who);
            if order.members.is_empty() {
                self.squad = None;
            }
        }
    }

    /// Everybody a player's own order moved out of the squad: the crew
    /// member an errand names, or whoever is selected for a move or a
    /// line.
    fn take_the_ordered_out_of_squad(&mut self, slot: u32, order: bims::order::CrewOrder) {
        if self.squad.is_none() {
            return;
        }
        if let Some(who) = order.errand_for() {
            self.take_out_of_squad(who);
            return;
        }
        if matches!(
            order,
            bims::order::CrewOrder::Move { .. } | bims::order::CrewOrder::Line { .. }
        ) {
            for who in self.aboard.room.selected_all(slot) {
                self.take_out_of_squad(who as u32);
            }
        }
    }

    /// Before the rooms step: the squad order pruned — a member a
    /// player has begun steering, one dead or outside, and the whole
    /// order when the commander is no longer fit to act or the mark is
    /// gone — and then handed to the room, one [`bims::game::Squad`] a
    /// crew member.
    fn hand_the_room_the_squad(&mut self) {
        self.settle_squad();
        let crew = self.aboard.crew_count() as usize;
        let mut squad = vec![bims::game::Squad::None; crew];
        if let Some(order) = self.squad.clone() {
            let t = shipdesign::TILE as f32;
            for &who in &order.members {
                if who as usize >= crew {
                    continue;
                }
                squad[who as usize] = match &order.kind {
                    SquadKind::Attack { .. } => match order.mark_for(who) {
                        Some(enemy) => bims::game::Squad::Attack {
                            enemy: enemy as usize,
                            seen: self.enemy_standing_at(enemy),
                        },
                        None => bims::game::Squad::None,
                    },
                    SquadKind::FallBack { tile } => bims::game::Squad::FallBack {
                        at: bims::math::vec2((tile.0 as f32 + 0.5) * t, (tile.1 as f32 + 0.5) * t),
                    },
                    SquadKind::StandGround => bims::game::Squad::StandGround,
                };
            }
        }
        self.aboard.room.set_squad(squad);
    }

    /// The pruning half of the step: an order ends when the commander
    /// goes down or dies, when an attack's marks are all gone — down or
    /// dead, and dead alone with *relentless* — or when nobody is left
    /// under it.
    fn settle_squad(&mut self) {
        if self.commanders.len() < self.aboard.crew_count() as usize {
            self.commanders
                .resize(self.aboard.crew_count() as usize, Commander::default());
        }
        let Some(order) = &self.squad else {
            return;
        };
        let by = order.by_slot;
        if !self.fit_to_act(by) || !self.is_commander(by) {
            self.squad = None;
            return;
        }
        if let SquadKind::Attack { enemies } = &order.kind {
            let relentless = self.has_talent(by, Talent::Relentless);
            let alive: Vec<u32> = enemies
                .iter()
                .copied()
                .filter(|&e| {
                    if relentless {
                        !self.enemy_dead_at(e)
                    } else {
                        self.enemy_standing_at(e)
                    }
                })
                .collect();
            if alive.is_empty() {
                self.squad = None;
                return;
            }
            if alive.len() != enemies.len()
                && let Some(order) = &mut self.squad
            {
                order.kind = SquadKind::Attack { enemies: alive };
            }
        }
        let keep: Vec<u32> = self
            .squad
            .as_ref()
            .map(|o| o.members.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|&who| !self.is_steered(who) && self.on_the_deck(who))
            .collect();
        match self.squad.as_mut() {
            Some(order) if !keep.is_empty() => order.members = keep,
            _ => self.squad = None,
        }
    }

    /// What a mercenary's month costs when this slot does the hiring:
    /// the fee less [`class::HIRE_DISCOUNT_PERCENT`] for a commander fit
    /// to act — two fifths with *haggler* — rounded down to whole euros,
    /// and the plain fee for everybody else. Read once, as the contract
    /// is signed: it stays that hand's fee whatever happens to him.
    pub fn hire_fee(&self, slot: u32, resident: u32) -> Option<Money> {
        let fee = self.mercenary_fee(resident)?;
        if !class::can(self.class_of(slot), class::Ability::SquadOrder) || !self.fit_to_act(slot) {
            return Some(fee);
        }
        let off = if self.has_talent(slot, Talent::Haggler) {
            class::HAGGLER_DISCOUNT_PERCENT
        } else {
            class::HIRE_DISCOUNT_PERCENT
        };
        Some(fee - fee * Money::from(off) / 100)
    }

    /// Before the rooms step: every crew member's skill to the room.
    fn hand_the_room_the_soldiers(&mut self) {
        let crew = self.aboard.crew_count();
        let skills: Vec<bims::combat::Skill> = (0..crew).map(|who| self.skill_of(who)).collect();
        self.aboard.room.set_skills(skills);
    }

    /// Whether an enemy is standing in the crew's room: the rooms joined
    /// and hostile, and one of the station's people alive and on its
    /// feet. A *rampage* lasts while one is.
    fn enemy_standing(&self) -> bool {
        let Some(residents) = &self.residents else {
            return false;
        };
        if self.stance(residents.station) != Stance::Hostile || !self.aboard.is_joined() {
            return false;
        }
        let room = &residents.aboard.room;
        (0..room.crew_count() as usize).any(|who| room.is_alive(who) && !room.is_unconscious(who))
    }

    /// After `visit` and the experience: what the fight did to the
    /// soldiers' *rampage* — a stack for each enemy one of them downed,
    /// off the residents' `last_hit_by`, and every stack gone when the
    /// fight ends, which is the rooms unjoined or no enemy standing.
    fn settle_rampage(&mut self, downed: &[(usize, Option<usize>)]) {
        if !self.enemy_standing() {
            for who in 0..self.aboard.crew_count() as usize {
                if self.aboard.room.rampage(who) > 0 {
                    self.aboard.room.set_rampage(who, 0);
                }
            }
            return;
        }
        for &(_, by) in downed {
            let Some(by) = by else {
                continue;
            };
            if self.has_talent(by as u32, Talent::Rampage) {
                let stacks = (self.aboard.room.rampage(by) + 1).min(class::RAMPAGE_STACKS);
                self.aboard.room.set_rampage(by, stacks);
            }
        }
    }

    /// After `visit`: the laid sandbags a burst reached are gone, whatever
    /// they had left (`DeployableLost`).
    fn settle_bursts(&mut self, events: &mut Vec<WorldEvent>) {
        let blown = self.aboard.room.take_bags_blown();
        if blown.is_empty() {
            return;
        }
        let t = shipdesign::TILE as f32;
        let mut gone = Vec::new();
        for (x, y) in blown {
            let p = bims::math::vec2((x as f32 + 0.5) * t, (y as f32 + 0.5) * t);
            if let Some((d, _)) = self
                .deployable_under(p)
                .filter(|(d, _)| d.kind == DeployKind::Sandbags)
                && !gone.contains(&d.id)
            {
                gone.push(d.id);
            }
        }
        if gone.is_empty() {
            return;
        }
        self.deployables.retain(|d| !gone.contains(&d.id));
        for _ in &gone {
            events.push(WorldEvent::DeployableLost {
                kind: DeployKind::Sandbags.code(),
            });
        }
        self.sync_deployed_cover();
    }

    /// The session done: the piece into the output slot with the metal's
    /// worth back on it, capped at its tier's full health.
    fn finish_repair(&mut self, events: &mut Vec<WorldEvent>) {
        if self.bench.repair.take().is_none() {
            return;
        }
        let Some(Item::Armour(mut piece)) = self.bench.slots[0] else {
            return;
        };
        piece.health = (piece.health
            + class::ARMOUR_REPAIR_PER_METAL * deploy::ARMOUR_REPAIR_METAL as f32)
            .min(piece.stats().health);
        self.bench.slots[0] = None;
        self.bench.slots[Workbench::OUT] = Some(Item::Armour(piece));
        events.push(WorldEvent::Repaired {
            kind: piece.kind.code(),
        });
    }
}

fn refused(slot: u32, why: Refusal) -> WorldEvent {
    WorldEvent::Refused { slot, why }
}

/// The refusal a walk's code is, if it is one: the room's `ORDER_LOCKED`
/// and `ORDER_NOWHERE` — see `bims::order`. `None` for a walk that went,
/// or an order that was not a walk.
fn walk_refusal(code: u32) -> Option<Refusal> {
    match code {
        bims::game::ORDER_LOCKED => Some(Refusal::DoorLocked),
        bims::game::ORDER_NOWHERE => Some(Refusal::NoWayThere),
        _ => None,
    }
}

/// What a room banked since last asked, folded into the hold: the
/// medkits used off the count, the fibre harvested onto the shelf as far
/// as it has room. `true` when anything moved, so the caller knows to run
/// `on_ship_changed`. Asked of the crew's room every step
/// (`take_the_room_s_medicine`), and of a room about to be thrown away —
/// a docking or a casting off replaces the whole room — *after* the crew
/// have been taken out of it, since giving up an errand is what puts the
/// sheaf in somebody's hands into the store.
fn bank_medicine(design: &mut ShipDesign, room: &mut bims::game::Game) -> bool {
    let kits = room.take_medkits_used();
    let grown = room.take_harvested_fibre();
    // A helper that opened a kit out of its own pack: the medkit leaves
    // the pack and goes on the hold's count, since from the moment it is
    // in a hand it is counted the way a kit off a shelf is — spent by
    // `medkits_used` above when the treatment finishes, and put back on a
    // shelf by `Task::let_go` when the chain is given up.
    let wanted = Item::Stack(ResourceId::Medkit as u32);
    let mut out_of_packs = 0;
    for who in room.take_pack_kits_used() {
        if who >= room.crew_count() as usize {
            continue;
        }
        let cell = room.pack(who).iter().position(|i| *i == Some(wanted));
        if let Some(cell) = cell
            && room.take(who, cell).is_some()
        {
            out_of_packs += 1;
        }
    }
    if kits == 0 && grown == 0 && out_of_packs == 0 {
        return false;
    }
    design.cargo[ResourceId::Medkit as usize] += out_of_packs;
    let medkits = &mut design.cargo[ResourceId::Medkit as usize];
    *medkits = medkits.saturating_sub(kits);
    // By area: a stack of fibre is one cell, so a cell spare is a stack
    // that lies.
    let space = design.room_for(ResourceId::Fibre);
    design.cargo[ResourceId::Fibre as usize] += grown.min(space);
    true
}

/// A total order over nodes, for keeping [`World::discovered`] sorted.
/// Bodies before stations, ids ascending — the same order
/// `StarSystem::nodes` hands them out in.
pub fn node_key(node: &Node) -> (u32, u32) {
    match node {
        Node::Body(id) => (0, *id),
        Node::Station(id) => (1, *id),
    }
}

/// [`segment_distance`], for the probe that checks the geometry directly
/// rather than through a trip whose shape the seed decides.
pub fn segment_distance_for_probe(from: DVec2, to: DVec2, point: DVec2) -> f64 {
    segment_distance(from, to, point)
}

/// How near a point comes to a line segment. The one piece of geometry
/// discovery needs, and it is a segment rather than a line because the ship
/// travelled a stretch and not a whole axis.
fn segment_distance(from: DVec2, to: DVec2, point: DVec2) -> f64 {
    let along = to.sub(from);
    let len2 = along.length_squared();
    if len2 <= 0.0 {
        return from.distance(point);
    }
    let t = (point.sub(from).x * along.x + point.sub(from).y * along.y) / len2;
    let t = t.clamp(0.0, 1.0);
    from.add(along.scale(t)).distance(point)
}

/// Any dock somebody lives on and nobody shoots from, anywhere in the
/// galaxy: the `pick`-th of them, in star then station order, wrapping
/// round. For `nix run .#test`, which wants a *different* place each time
/// and a place a crew can live — so a random roll from the page turns into
/// a random system, and the same roll into the same one. Never a hostile
/// station: a crew that opened at an enemy's would open under fire, and
/// the lobby's own pick (`lobby::Lobby::random_start`) skips them too.
///
/// Every system is generated to answer it, which is what the lobby's World
/// tab does too; a galaxy is a few hundred stars and it takes a moment.
pub fn spawn_anywhere(galaxy: &Galaxy, pick: u64) -> Option<(u32, u32)> {
    let docks: Vec<(u32, u32)> = galaxy
        .every_system()
        .iter()
        .flat_map(|system| {
            system
                .stations
                .iter()
                .filter(|s| crate::station::residents_of(s.kind) > 0 && !s.hostile)
                .map(move |s| (system.star_id, s.id))
        })
        .collect();
    if docks.is_empty() {
        return None;
    }
    Some(docks[(pick % docks.len() as u64) as usize])
}

/// [`spawn_anywhere`], in a system with ground to set down on: the
/// `pick`-th lived-in, unhostile dock of a system whose **first** landable
/// body's people are not enemies either — the planet
/// [`World::land_for_probe`] puts the ship on. For `nix run .#test_planet`,
/// the `test` command landed: a crew that opened on an enemy's ground
/// would open under the guard's fire, so a surface rolled hostile
/// (`Surface::all_of`) rules its system out the way a hostile station is
/// skipped above.
pub fn spawn_with_ground(galaxy: &Galaxy, pick: u64) -> Option<(u32, u32)> {
    let docks: Vec<(u32, u32)> = galaxy
        .every_system()
        .iter()
        .filter(|system| {
            Surface::all_of(system, galaxy.seed)
                .first()
                .is_some_and(|surface| !surface.hostile)
        })
        .flat_map(|system| {
            system
                .stations
                .iter()
                .filter(|s| crate::station::residents_of(s.kind) > 0 && !s.hostile)
                .map(move |s| (system.star_id, s.id))
        })
        .collect();
    if docks.is_empty() {
        return None;
    }
    Some(docks[(pick % docks.len() as u64) as usize])
}

/// Where the **simulation** starts: the lowest star id with an orbital
/// nobody shoots from and a belt to mine, and the lowest such orbital in
/// that system.
///
/// An orbital, because it is the ordinary case — somewhere people live,
/// with the bunks and the shelf of a town — and not a derelict, since a
/// crew that opens docked at a wreck with nobody aboard opens at a place
/// to salvage rather than a place to start from. Nobody there is an enemy
/// (`StationBlueprint::hostile`), because a crew that opens at an enemy's
/// opens under fire. And the system has an asteroid belt, because a belt
/// is the mining site (`crate::mining`) and the playtest is where mining
/// is looked at: `BIMS_AT_BELT` and every fixture test that walks outside
/// hold at the spawn system's first belt, and a spawn system without one
/// would leave them nothing to hold at.
///
/// The game proper starts where the lobby's World tab said, and that pair
/// comes into [`World::start`] from outside. This is for `nix run
/// .#simulation`, which has no lobby in front of it and wants the same dock
/// every time — and for the fixtures, for the same reason.
pub fn spawn(galaxy: &Galaxy) -> Option<(u32, u32)> {
    for star in &galaxy.stars {
        let Some(system) = galaxy.system(star.id) else {
            continue;
        };
        if !system
            .bodies
            .iter()
            .any(|b| b.kind == worldgen::BodyKind::AsteroidBelt)
        {
            continue;
        }
        if let Some(station) = system
            .stations
            .iter()
            .filter(|s| s.kind == worldgen::StationKind::Orbital && !s.hostile)
            .map(|s| s.id)
            .min()
        {
            return Some((star.id, station));
        }
    }
    None
}
