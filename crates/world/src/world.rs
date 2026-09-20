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

use economy::{Money, Storage, footprint, storage, trade_value};
use flight::{Dynamics, Phase, Plan, PlanError, Target, angle};
use physics::ResourceId;
use shipdesign::parts::{PartKind, Rotation};
use shipdesign::research::{Node as ResearchNode, Research};
use shipdesign::{CARGO_SLOTS, ShipDesign, design_hash};
use worldgen::math::{DVec2, dvec2};
use worldgen::{Galaxy, GalaxyType, Node, StarSystem};

use bims::combat::{ArmourKind, Item, LOOT_CELLS, LootCell, PACK_CELLS, Tier, Weapon, WeaponKind};
use bims::game::Container;
use bims::health::Part;
use bims::sight::Stance;

use crate::armour::{self, FetchKind, LootSource, Piece, Where};
use crate::build::{self, BuildSite, SiteRefusal};
use crate::crew::{Aboard, Residents};
use crate::data;
use crate::event::{Refusal, WorldEvent};
use crate::frame::{self, Frame};
use crate::grid::{Grid, Kept, Wanted};
use crate::mercenary::{self, Hired, Offer};
use crate::mining::{self, MiningSite};
use crate::speed::{self, Speed};
use crate::station::{Berth, Station, enemies_of};
use crate::surface::{self, Surface};

/// What a player can ask the world to do.
///
/// Every one of them carries the slot that sent it, because every one of them
/// can be sent by anybody: there is one ship and the crew fly it together.
/// Which of them a player is *allowed* to send is [`World::can_command`], and
/// today the answer is always yes.
#[derive(Clone, Copy, PartialEq, Debug)]
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
    /// Take the research key off the research desk of the station the
    /// ship is tied to, into crew member `who`'s pack: the ship docked,
    /// a key there (`World::station_keys`), `who` alive, awake, aboard
    /// and within [`data::REACH`] of that desk, and two cells free in the
    /// pack, one over the other — a key is that big. The desk is bare
    /// after; the key is stowed in the crew's own desk from the pack
    /// like anything else, and consumed there by an `Unlock`.
    TakeKey {
        slot: u32,
        who: u32,
    },
    /// Consume the research key in the crew's research desk to open the
    /// lock on node `node` (`shipdesign::research::Node`'s code) — one
    /// key, one node. Wants a key in the desk and a powered research desk
    /// aboard; a node open already, or with no lock, is refused, so no
    /// key is spent for nothing.
    Unlock {
        slot: u32,
        node: u32,
    },
    /// Put the ship's AI onto node `node` of the research tree
    /// (`shipdesign::research::Node`'s code). Wants a research desk
    /// aboard and the node to be one that can be begun; what the AI was on
    /// before is dropped. A command because every player's ship has to
    /// agree about what its crew know.
    Research {
        slot: u32,
        node: u32,
    },
    /// Take the AI off whatever it is on.
    CancelResearch {
        slot: u32,
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
}

/// What the ship is doing.
#[derive(Clone, PartialEq, Debug)]
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
    /// arena ([`World::arena_dock_for_probe`], [`data::ARENA_REINFORCEMENTS`]).
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
    /// Whether each station's research desk still has its key on it, by
    /// index into `stations`: what the blueprint rolled (`Station::key`),
    /// the spawn's always, until a crew member takes it. In
    /// `world_checksum` whole.
    pub station_keys: Vec<bool>,
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
    /// What is on the workbench being upgraded, and how far along. The
    /// two that went in are out of the hold; the one that comes out is
    /// delivered by `deliver_upgrade`. In `world_checksum` whole.
    pub upgrade: Option<Upgrade>,
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
}

/// A lamp a fight has damaged, remembered by where it hangs: which
/// station's design it is in — `None` for the ship's own — and its tile
/// there, with what it has left of `bims::sight::LAMP_HEALTH`; nought is
/// out, for good. See [`World::lamps`].
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct LampDamage {
    pub station: Option<u32>,
    pub tile: (u32, u32),
    pub health: f32,
}

/// An upgrade under way at the workbench: two items of a kind and a tier,
/// gone from the hold the step it began, becoming one of the next tier
/// over [`data::UPGRADE_SESSIONS`] hours of work. Progress is whole
/// sessions and the world's, so a Bim that leaves the bench between them
/// loses nothing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Upgrade {
    /// What is being made, as the resource it counts as in the hold.
    pub resource: ResourceId,
    /// The tier it comes out at.
    pub to: Tier,
    /// Sessions done, of [`data::UPGRADE_SESSIONS`].
    pub done: u32,
}

impl Upgrade {
    /// Whether the day's work is done and the item waits only for room in
    /// the lockers.
    pub fn complete(&self) -> bool {
        self.done >= data::UPGRADE_SESSIONS
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
    /// `combat` command opens on five crew this way, one a player.
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
        hostile.sort_unstable();

        // What the crew set out with, for the enemies to be scaled against.
        let start_worth = shipdesign::Budget::spent(&design);
        let dynamics = flight::dynamics(&design, crew).map_err(StartError::NotAShip)?;
        let aboard = Aboard::new(&design, crew, seed);
        let station_keys: Vec<bool> = stations
            .iter()
            .map(|s| s.key || s.id == station_id)
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
            residents: None,
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
            upgrade: None,
            grids: [Grid::default(), Grid::default(), Grid::default()],
            lamps: Vec::new(),
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
        let now = self.ship.position();

        // 4. What that brought into range.
        self.discover_along(was, now, &mut events);
        self.settle_frame(&mut events);
        self.settle_residents();
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
        //    And what the benches are wanted for, worked out fresh from the
        //    hold, the targets and the power; then, after the step, what
        //    they finished, moved through the hold.
        //    First, with the tick box on, a pair of matching gear goes onto
        //    the workbench — out of the hold now — so the orders below can
        //    have a Bim work at it.
        self.begin_upgrade(&mut events);
        let orders = self.craft_orders();
        self.aboard.room.set_craft_orders(orders);
        //    The sickbay's shelf is the hold's too: the bandages to hand
        //    are the hold's count, and the fibre on the cold store's shelf
        //    is the hold's count — the room keeps neither of its own
        //    aboard. What the room used and what it grew are read back
        //    after the step, below.
        self.hand_the_room_the_hold_s_medicine();
        let eva = self.eva_offer();
        self.aboard.room.set_eva(eva);
        //    And the construction sites, what each still wants, and who may
        //    go out to one beyond the hull. What the room did about them is
        //    read in stage 7.
        let builds = self.build_orders();
        let suit_ok = self.suit_ok();
        self.aboard.room.set_build_orders(builds, suit_ok);
        self.aboard.step();
        if let Some(residents) = &mut self.residents {
            residents.aboard.step();
        }
        self.visit(&mut events);
        self.sync_lamps();
        self.casualties(&mut events);
        self.melee_locks(&mut events);
        //    And what the fight did to the armour: the pieces in packs and
        //    on bodies are the room's, and the world's copies are read
        //    back after the step so the checksum sees them as they are.
        self.mirror_pieces(&mut events);
        self.take_the_room_s_medicine();
        for recipe in self.aboard.room.take_crafted() {
            self.finish_craft(recipe, &mut events);
        }
        self.deliver_upgrade(&mut events);
        for at in self.aboard.room.take_mined() {
            self.finish_tile(at);
        }
        for _ in 0..self.aboard.room.take_walks() {
            self.finish_walk(&mut events);
        }

        // 6. Power: what the reactors made this step against what the
        //    wired consumers drew, into or out of the batteries. What is
        //    running in a brownout is `World::powered`, read by whatever
        //    draws — nothing aboard reads it yet.
        self.run_power();
        //    And the AI's research, which runs on the research desk's power.
        self.run_research(&mut events);

        // 7. Construction: what the crew did at the sites this step — a load
        //    taken off a shelf, a load put down, a load given up, a part put
        //    together — moved through the hold. Building goes through
        //    `shipdesign::materials`, out of what is aboard, and asks
        //    `World::can_modify_part` first. See `crate::build`.
        for (site, resource, units) in self.aboard.room.take_picked() {
            self.finish_pick(site, resource, units);
        }
        for site in self.aboard.room.take_dropped() {
            self.finish_drop(site);
        }
        for site in self.aboard.room.take_returned() {
            self.finish_return(site);
        }
        for site in self.aboard.room.take_built() {
            self.finish_build(site, &mut events);
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
            | Command::Unlock { slot, .. }
            | Command::Research { slot, .. }
            | Command::CancelResearch { slot }
            | Command::SetAutoUpgrade { slot, .. }
            | Command::Arrange { slot, .. }
            | Command::Repack { slot, .. } => slot,
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
            Command::Unlock { node, .. } => self.unlock(slot, node, events),
            Command::Research { node, .. } => self.research(slot, node, events),
            Command::CancelResearch { .. } => self.research.cancel(),
            Command::SetAutoUpgrade { on, .. } => self.auto_upgrade = on,
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
        self.star_id = star;
        self.system = system;
        self.stations = Station::all_of(&self.system);
        let mut hostile: Vec<u32> = self
            .stations
            .iter()
            .filter(|s| s.hostile)
            .map(|s| s.id)
            .collect();
        hostile.sort_unstable();
        self.hostile = hostile;
        self.station_keys = self.stations.iter().map(|s| s.key).collect();
        self.residents = None;
        self.reinforcements = 0;
        self.sites.clear();
        self.site_version += 1;
        self.discovered.clear();
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
                let along = self.way_out(station).unwrap_or(DVec2 { x: 0.0, y: 1.0 });
                self.unjoin_rooms();
                self.ship.state = ShipState::Undocking {
                    station,
                    from,
                    along,
                    began: self.clock_minutes,
                };
                let slot = self.ship.pending.map(|(slot, _)| slot).unwrap_or(0);
                events.push(WorldEvent::Undocking { slot });
            }
            ShipState::Undocking {
                from, along, began, ..
            } => {
                let t = self.clock_minutes - began;
                let reach = self.undock_distance() * eased(t, data::UNDOCK_MINUTES);
                self.ship.set_position(from.add(along.scale(reach)));
                if t < data::UNDOCK_MINUTES {
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
        let half = data::DOCK_MINUTES / 2.0;
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
        if t < data::DOCK_MINUTES {
            return;
        }
        self.ship.state = ShipState::Docked { station };
        self.dock_at(station);
        events.push(WorldEvent::Arrived {
            station: Some(station),
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

    pub fn station(&self, id: u32) -> Option<&Station> {
        self.stations.iter().find(|s| s.id == id)
    }

    /// What the ship and everything in its hold are worth now, in whole
    /// euros: every part's price and every unit of cargo at its trade
    /// value — `shipdesign::Budget::spent`, the same sum the design phase
    /// charged for it. The crew's money in hand is not in it: net worth
    /// here is the ship and its contents, which is what an enemy sizes up.
    pub fn worth(&self) -> Money {
        shipdesign::Budget::spent(&self.ship.design)
    }

    /// How many people a station's room is opened with: the people who
    /// live there, or, at an enemy's, the enemies — [`enemies_of`] the
    /// crew's number and worth against [`World::start_worth`]. Nobody on a
    /// derelict either way. Asked when the room opens, so a station keeps
    /// the crowd it was reached with until the ship has gone and come back.
    pub fn people_of(&self, station: &Station) -> u32 {
        if station.residents() == 0 {
            return 0;
        }
        match self.stance(station.id) {
            Stance::Hostile => enemies_of(self.aboard.crew_count(), self.worth(), self.start_worth)
                .saturating_add(self.reinforcements)
                .min(data::ENEMIES_MAX),
            Stance::Friendly | Stance::Neutral => station.residents(),
        }
    }

    /// How many mercenaries for hire live at a station, on top of
    /// [`World::people_of`]: none at an enemy's or on a derelict, else
    /// [`mercenary::how_many`] the crew's worth against
    /// [`World::start_worth`] off the station's seed — at least
    /// `least_mercenaries` for the `test` command. Asked when the room
    /// opens, like the people, so a station keeps its offer until the
    /// ship has gone and come back.
    pub fn mercenaries_of(&self, station: &Station) -> u32 {
        if station.residents() == 0 || self.stance(station.id) == Stance::Hostile {
            return 0;
        }
        mercenary::how_many(self.worth(), self.start_worth, station.map_seed)
            .max(self.least_mercenaries)
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
            _ => Residents::open(id, &design, count, mercs, seed, self.clock_minutes),
        };
        self.drop_loads();
        let ship_seed = self.galaxy_seed ^ self.steps;
        // The crew out of the old room, and then what leaving banked in
        // it — a sheaf in somebody's hands goes into the store as the
        // errand is given up — into the hold before the room is dropped.
        let mut old = std::mem::replace(&mut self.aboard, Aboard::new(&self.ship.design, 1, 0));
        let crew = old.room.take_crew();
        let banked = bank_medicine(&mut self.ship.design, &mut old.room);
        self.aboard = Aboard::joined(
            joined,
            &self.ship.design,
            &design,
            crew,
            ship_seed,
            self.clock_minutes,
        );
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
        }
        self.residents = Some(residents);
        self.apply_stances();
        self.restore_lamps();
        if banked {
            self.on_ship_changed();
        }
    }

    /// Whose a station is, to the crew: home is friendly, a station on the
    /// hostile list is hostile, and everywhere else is neutral.
    pub fn stance(&self, station: u32) -> Stance {
        if self.hostile.binary_search(&station).is_ok() {
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
            }
            _ => {}
        }
        let reopen = self
            .residents
            .as_ref()
            .filter(|r| r.station == station)
            .and_then(|r| {
                let s = self.station(station)?;
                let count = self.people_of(s);
                let mercs = self.mercenaries_of(s);
                (count + mercs != r.aboard.count())
                    .then(|| (s.design.clone(), count, mercs, s.map_seed))
            });
        if let Some((design, count, mercs, seed)) = reopen {
            let mut residents =
                Residents::open(station, &design, count, mercs, seed, self.clock_minutes);
            // Docked there, the room is looked into the way `join_rooms`
            // left it: its doors drawn by the joined deck and its fog the
            // joined deck's, so the new crowd is seen where the old was.
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
        self.apply_stances();
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
        match foreign {
            Some((rect, stance)) => self.aboard.room.set_foreign(Some(rect), stance),
            None => self.aboard.room.set_foreign(None, Stance::Neutral),
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

    /// Every lamp remembered damaged, set so on the rooms it hangs in.
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
                    (
                        residents.aboard.position(who),
                        residents.aboard.room.is_down(who as usize),
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
        let Some(residents) = self.residents.as_mut().filter(|_| hostile) else {
            self.aboard.room.set_hostiles(Vec::new());
            if let Some(residents) = &mut self.residents {
                residents.aboard.room.set_hostiles(Vec::new());
            }
            return;
        };
        let shift = residents.aboard.offset;
        let room = &mut residents.aboard.room;
        for hit in hits {
            let who = hit.who;
            if who >= room.crew_count() as usize || !room.is_alive(who) {
                continue;
            }
            room.strike(who, hit.part, hit.damage, hit.cut);
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
        for who in 0..residents.down.len().min(room.crew_count() as usize) {
            let down = !room.is_alive(who);
            if down && !residents.down[who] {
                residents.down[who] = true;
                events.push(WorldEvent::EnemyDown {
                    station: residents.station,
                    who: who as u32,
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
        let crew: Vec<Option<(bims::math::Vec2, Weapon)>> = self
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
        room.set_hostiles(crew.clone());
        room.set_hostiles_peeking(&self.aboard.crew_peeking());
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
                        self.aboard.room.enemy_strike(
                            on_deck(shot.from),
                            who,
                            shot.damage,
                            shot.cut,
                        );
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
        // the peek while peeking, and which are peeking.
        let alive: Vec<bool> = (0..room.crew_count())
            .map(|who| room.is_alive(who as usize) && !room.is_unconscious(who as usize))
            .collect();
        let weapons: Vec<Weapon> = (0..room.crew_count())
            .map(|who| {
                room.weapon(who as usize)
                    .unwrap_or(WeaponKind::LaserPistol.basic())
            })
            .collect();
        let peeking: Vec<bool> = (0..room.crew_count())
            .map(|who| room.peek(who as usize).is_some())
            .collect();
        let dodge: Vec<f32> = (0..room.crew_count())
            .map(|who| room.dodge(who as usize))
            .collect();
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
            }
        }
    }

    /// The hold's medicine, put on the room's shelves before it steps:
    /// the bandages and the medkits to hand are the hold's `Bandage` and
    /// `Medkit` counts, and the fibre on the cold store's shelf is the
    /// hold's `Fibre` count, since the hold keeps all three and the room
    /// keeps none aboard. The store's
    /// food is the room's own and is handed back unchanged.
    fn hand_the_room_the_hold_s_medicine(&mut self) {
        let design = &self.ship.design;
        let (bandages, fibre) = (
            design.carrying(ResourceId::Bandage),
            design.carrying(ResourceId::Fibre),
        );
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
        let room = &mut self.aboard.room;
        room.set_bandages(bandages);
        room.set_medkits(medkits);
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
        // A room taken apart drops every errand, a load in somebody's arms
        // with it: whatever was on its way to a site is the hold's again.
        self.drop_loads();
        let seed = self.galaxy_seed ^ self.steps;
        // As at the join: the crew out first, then what that banked.
        let mut old = std::mem::replace(&mut self.aboard, Aboard::new(&self.ship.design, 1, 0));
        let crew = old.room.take_crew();
        let banked = bank_medicine(&mut self.ship.design, &mut old.room);
        self.aboard = old.unjoined(crew, &self.ship.design, seed, self.clock_minutes);
        if let Some(residents) = &mut self.residents {
            residents.aboard.room.set_doors_drawn(true);
            residents.aboard.room.set_fog(bims::sight::Fog::All);
            // And the ship off their deck: anybody still aboard it is put
            // at its bunk, since the ship has left without it.
            let station = self
                .stations
                .iter()
                .find(|s| s.id == residents.station)
                .cloned();
            if let Some(station) = station {
                residents.unjoin(&station, self.clock_minutes);
            }
        }
        self.restore_lamps();
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
            self.residents = None;
        }
        if let Some((id, clearance, (count, mercs), seed)) = near
            && clearance <= data::RESIDENTS_RANGE
            && let Some(station) = self.station(id)
        {
            self.residents = Some(Residents::open(
                id,
                &station.design,
                count,
                mercs,
                seed,
                self.clock_minutes,
            ));
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
                    });
                }
            }
        }
        // And a session of the upgrade, at the first workbench only — one
        // pair of hands a day, however many benches — while one is under
        // way and unfinished, the bench is powered and nobody is at it.
        if let Some(upgrade) = self.upgrade
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

    /// A Bim took a load off a shelf for `site`: as much of `units` of
    /// `resource` as the hold has free is now in its arms — spoken for, not
    /// moved. A site that has gone gets nothing, and the Bim carries a
    /// crate of nothing to nowhere, which the room sorts out at its next
    /// walk.
    fn finish_pick(&mut self, site: u32, resource: u32, units: u32) {
        let Some(resource) = ResourceId::ALL.get(resource as usize).copied() else {
            return;
        };
        let got = units.min(self.free(resource));
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
    }

    /// A Bim put a site together: the part goes down and its recipe comes
    /// out of the hold in one go, if the rules still allow it — the deck
    /// under it may have been laid out and cancelled since, the metal sold
    /// — and an event either way. The site is finished with whatever
    /// happened: a part that will not go is a site to lay out again, not
    /// one to stand at for ever. The room is laid out again under the
    /// crew with the part in it.
    fn finish_build(&mut self, site: u32, events: &mut Vec<WorldEvent>) {
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
            .any(|p| p.kind == kind && shipdesign::is_powered(&self.ship.design, p.id));
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

    /// Whether that player's crew member is at the helm: within
    /// [`data::HELM_REACH`] of the seat. Slot *i* is Bim *i*, the same
    /// pairing as the bunks.
    pub fn at_the_helm(&self, slot: u32) -> bool {
        if slot >= self.aboard.crew_count() {
            return false;
        }
        self.helm_spot()
            .is_some_and(|seat| self.aboard.position(slot).distance(seat) <= data::HELM_REACH)
    }

    /// Whether that player's crew member is at a trading desk: alive,
    /// awake, aboard, and within [`data::REACH`] tiles of a desk's
    /// footprint on the deck it walks — the station's, on the joined
    /// deck. What a buy or a sell wants beside the berth
    /// (`Refusal::NotAtTheDesk`): the station is traded with across its
    /// desk, and the goods still go straight into the hold. A ship has
    /// no desk of its own, so away from a berth this is never true.
    pub fn at_the_desk(&self, slot: u32) -> bool {
        if slot >= self.aboard.crew_count() {
            return false;
        }
        let room = &self.aboard.room;
        let who = slot as usize;
        if !room.is_alive(who) || room.is_unconscious(who) || room.is_outside(who) {
            return false;
        }
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
    /// sent elsewhere. A room order like any other, not a command: it
    /// crosses no seam and lands on nobody else's screen. False when there
    /// is no helm or no way to it.
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
    /// go — under way the helm is a job the room hands out itself. A room
    /// order like the walk: it crosses no seam.
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
        if !self
            .station(station)
            .is_some_and(|s| s.stock.sells(resource))
        {
            events.push(refused(slot, Refusal::NotSoldHere));
            return;
        }
        let Ok(value) = trade_value(resource, units) else {
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
        if !matches!(self.ship.state, ShipState::Docked { .. }) {
            events.push(refused(slot, Refusal::NotDocked));
            return;
        }
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
        let Ok(value) = trade_value(resource, units) else {
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

    /// Whether a workbench is aboard at all: the bench an upgrade is
    /// worked at, powered or not.
    fn workbench_aboard(&self) -> bool {
        self.aboard
            .room
            .benches()
            .iter()
            .any(|b| b.kind == PartKind::Workbench.code())
    }

    /// The first pair of matching gear in the hold that could go up a
    /// tier: armour kinds first, then weapons, the lowest tier first
    /// within a kind, tier three never. For armour the two **most
    /// damaged** pieces of the kind and tier — the good ones stay in
    /// circulation, and the piece that comes out is fresh whatever went
    /// in. What the pair is and, for armour, which two pieces.
    fn upgrade_pair(&self) -> Option<(ResourceId, Tier, Vec<u32>)> {
        for kind in ArmourKind::ALL {
            for tier in Tier::ALL {
                if tier.next().is_none() {
                    continue;
                }
                let mut held: Vec<&Piece> = self
                    .pieces
                    .iter()
                    .filter(|p| p.kind == kind && p.tier == tier && p.at == Where::Hold)
                    .collect();
                if held.len() < 2 {
                    continue;
                }
                held.sort_by(|a, b| {
                    a.health
                        .partial_cmp(&b.health)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then(a.id.cmp(&b.id))
                });
                return Some((
                    armour::resource_of(kind),
                    tier,
                    held.iter().take(2).map(|p| p.id).collect(),
                ));
            }
        }
        for kind in WeaponKind::ALL {
            for tier in Tier::ALL {
                if tier.next().is_some() && self.guns_at(kind, tier) >= 2 {
                    return Some((armour::weapon_resource(kind), tier, Vec::new()));
                }
            }
        }
        None
    }

    /// Stage 5, before the craft orders: with the tick box on, nothing on
    /// the workbench and a workbench aboard, the first pair goes onto it —
    /// out of the hold now, the way the user asked, so what is being made
    /// is never also what is being worn — and the upgrade is begun. A pair
    /// with no workbench aboard waits, untouched.
    fn begin_upgrade(&mut self, events: &mut Vec<WorldEvent>) {
        if !self.auto_upgrade || self.upgrade.is_some() || !self.workbench_aboard() {
            return;
        }
        let Some((resource, tier, pieces)) = self.upgrade_pair() else {
            return;
        };
        let Some(to) = tier.next() else {
            return;
        };
        // The instances before the count, so the settle finds them
        // agreeing.
        if pieces.is_empty() {
            for _ in 0..2 {
                if let Some(i) = self
                    .guns
                    .iter()
                    .position(|g| armour::weapon_resource(g.kind) == resource && g.tier == tier)
                {
                    self.guns.remove(i);
                }
            }
        } else {
            self.pieces.retain(|p| !pieces.contains(&p.id));
        }
        self.ship.design.cargo[resource as usize] -= 2;
        self.on_ship_changed();
        self.upgrade = Some(Upgrade {
            resource,
            to,
            done: 0,
        });
        events.push(WorldEvent::UpgradeBegun {
            resource: resource as u32,
            tier: to.code(),
        });
    }

    /// One session of the upgrade finished at the workbench — the room
    /// said `UPGRADE_ORDER` through `take_crafted`: an hour more of the
    /// day. The item itself is delivered by `deliver_upgrade`.
    fn finish_upgrade_session(&mut self) {
        if let Some(upgrade) = self.upgrade.as_mut()
            && upgrade.done < data::UPGRADE_SESSIONS
        {
            upgrade.done += 1;
        }
    }

    /// Every step after the crafts: an upgrade with its day done goes
    /// into the hold as one item of the next tier — a fresh piece, or a
    /// gun on the list — if the lockers have room for it. No room, and it
    /// waits, complete, and is tried again next step: nothing is lost.
    fn deliver_upgrade(&mut self, events: &mut Vec<WorldEvent>) {
        let Some(upgrade) = self.upgrade else {
            return;
        };
        if upgrade.done < data::UPGRADE_SESSIONS {
            return;
        }
        if !self.has_room(upgrade.resource, 1) {
            return;
        }
        // The instance before the count, as everywhere.
        if let Some(kind) = armour::kind_of(upgrade.resource) {
            let id = self.next_piece;
            self.next_piece += 1;
            self.pieces.push(Piece::new(id, kind, upgrade.to));
        } else if let Some(gun) = armour::weapon_at(upgrade.resource, upgrade.to) {
            self.guns.push(gun);
        } else {
            self.upgrade = None;
            return;
        }
        self.ship.design.cargo[upgrade.resource as usize] += 1;
        self.on_ship_changed();
        self.upgrade = None;
        events.push(WorldEvent::Upgraded {
            resource: upgrade.resource as u32,
            tier: upgrade.to.code(),
        });
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
            Container::Shelf(_) => class == Storage::Shelf || armour::is_gear(resource),
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
        if !self.has_room(resource, 1) {
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
        self.ship.design.cargo[resource as usize] += 1;
        self.on_ship_changed();
        events.push(WorldEvent::Stowed { who });
    }

    /// A fetch: one piece by id, or one unit of a resource — the least
    /// damaged piece of the kind for an armour resource — out of the hold
    /// and into the first free pack cell. See [`Command::Fetch`].
    fn fetch(&mut self, slot: u32, who: u32, kind: FetchKind, events: &mut Vec<WorldEvent>) {
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
            // Resolved above into one of the three.
            FetchKind::Slot { .. } => {
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
        // The first cell the thing fits: a key wants two, one over the
        // other.
        let Some(cell) = self.aboard.room.gear(who as usize).free_cell_for(item) else {
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
        // The cell was free an instant ago and nothing has moved since.
        let given = self.aboard.room.give(who as usize, Some(free), item);
        debug_assert!(given, "the free cell took it");
        self.mirror_pieces(events);
        events.push(WorldEvent::Looted {
            who,
            source_kind: source.code(),
        });
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
        let fee = self.mercenary_fee(resident)?;
        Some(Offer {
            fee,
            in_reach: self.in_reach_of_body(who, LootSource::Resident(resident)),
            affordable: self.money >= fee,
            bunk: (self.aboard.crew_count() as usize) < self.aboard.room.bed_count(),
            docked: self.residents.as_ref().map(|r| r.station) == self.ship.state.station(),
        })
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
        if !offer.affordable {
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
        // Out of the station's room.
        let mut everybody = residents.aboard.room.take_crew();
        let mut body = everybody.remove(resident as usize);
        residents
            .aboard
            .room
            .adopt(everybody, bims::math::Vec2::ZERO);
        residents.aboard.crew = residents.aboard.room.crew_count();
        residents.down.remove(resident as usize);
        residents.fee.remove(resident as usize);
        // Into the crew's, where it stood on the deck, with its armour
        // renumbered as the world's.
        let new_who = self.aboard.crew_count();
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
        self.ship.crew_count = self.aboard.crew;
        self.money -= offer.fee;
        self.hired.push(Hired {
            who: new_who,
            fee: offer.fee,
            due: self.clock_minutes + mercenary::MONTH,
            owed: false,
        });
        self.on_ship_changed();
        self.mirror_pieces(events);
        events.push(WorldEvent::Hired { who: new_who });
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
            residents.fee.push(Some(hired.fee));
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

    /// Whether a station's research desk still has its key on it.
    pub fn station_has_key(&self, station: u32) -> bool {
        self.stations
            .iter()
            .zip(&self.station_keys)
            .any(|(s, &key)| s.id == station && key)
    }

    /// Whether the station the ship is tied to has a key on its desk.
    pub fn key_at_the_dock(&self) -> bool {
        match self.ship.state {
            ShipState::Docked { station } => self.station_has_key(station),
            _ => false,
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

    /// How many research keys are in the crew's own desk and not spoken
    /// for. What an `Unlock` consumes one of.
    pub fn keys_in_desk(&self) -> u32 {
        self.free(ResourceId::ResearchKey)
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
        if !self.station_keys[at] || self.station_desk().is_none() {
            events.push(refused(slot, Refusal::NoKey));
            return;
        }
        if !self.key_in_reach(who) {
            events.push(refused(slot, Refusal::OutOfReach));
            return;
        }
        let key = Item::Key(1);
        let Some(cell) = self.aboard.room.gear(who as usize).free_cell_for(key) else {
            events.push(refused(slot, Refusal::PackFull));
            return;
        };
        if !self.aboard.room.give(who as usize, Some(cell), key) {
            events.push(refused(slot, Refusal::PackFull));
            return;
        }
        self.station_keys[at] = false;
        events.push(WorldEvent::KeyTaken { who });
    }

    /// An unlock: a key out of the crew's desk, gone, and the node's lock
    /// open. See [`Command::Unlock`] for what is checked.
    fn unlock(&mut self, slot: u32, node: u32, events: &mut Vec<WorldEvent>) {
        if !self.research_desk_powered() {
            events.push(refused(slot, Refusal::NoResearchDesk));
            return;
        }
        if self.keys_in_desk() == 0 {
            events.push(refused(slot, Refusal::NoKey));
            return;
        }
        let opened = ResearchNode::from_code(node).is_some_and(|n| self.research.unlock(n));
        if !opened {
            events.push(refused(slot, Refusal::NotResearchable));
            return;
        }
        self.ship.design.cargo[ResourceId::ResearchKey as usize] -= 1;
        self.on_ship_changed();
        events.push(WorldEvent::Unlocked { node });
    }

    /// Put the AI onto a node. See [`Command::Research`].
    fn research(&mut self, slot: u32, node: u32, events: &mut Vec<WorldEvent>) {
        if !self.research_desk_aboard() {
            events.push(refused(slot, Refusal::NoResearchDesk));
            return;
        }
        let begun = ResearchNode::from_code(node).is_some_and(|n| self.research.begin(n));
        if !begun {
            events.push(refused(slot, Refusal::NotResearchable));
            return;
        }
        events.push(WorldEvent::ResearchBegun { node });
    }

    /// The AI's step: a step's minutes onto whatever it is on, while a
    /// research desk aboard is running, and the word when a node is done.
    /// Stage 6's second half — it runs on the desk's power.
    fn run_research(&mut self, events: &mut Vec<WorldEvent>) {
        if self.research.current.is_none() || !self.research_desk_powered() {
            return;
        }
        if let Some(node) = self.research.advance(data::STEP_MINUTES) {
            events.push(WorldEvent::Researched { node: node.code() });
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
            ShipState::Docked { station }
            | ShipState::CastingOff { station, .. }
            | ShipState::Undocking { station, .. }
            | ShipState::Docking { station, .. } => Node::Station(*station),
            ShipState::Holding | ShipState::Charging { .. } => self.nearest_discovered()?,
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
    }

    // --- readouts -----------------------------------------------------------

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
    /// room its own again. For probes of what happens away from a station.
    pub fn undock_for_probe(&mut self) {
        self.ship.state = ShipState::Holding;
        self.unjoin_rooms();
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

    /// Pretend the reactors make `supply` a minute until the ship next
    /// changes. For probes of a short network: with a basic reactor making
    /// two and a half thousand, nothing a twenty-tile ship can carry draws
    /// more than it makes, and the brownout is otherwise unreachable.
    pub fn throttle_reactors_for_probe(&mut self, supply: f64) {
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
            gear.weapon = Some(bims::combat::WeaponKind::LaserPistol.basic());
            residents.aboard.room.issue(0, gear);
        }
        // The crew member: just inside the station's door, under orders.
        self.aboard
            .room
            .put_for_probe(0, bims::math::vec2(ashore.x as f32, ashore.y as f32));
        self.aboard.room.recruit_for_probe(0, true);
        true
    }

    /// Rebuild the station the ship is tied to as the arena —
    /// [`crate::station::arena`], the same kind and seed laid out bigger,
    /// with bunks for a whole garrison — standing where it stood, and dock
    /// there again, with [`data::ARENA_REINFORCEMENTS`] more enemies than
    /// the garrison would be whenever it is hostile: the `combat`
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
        self.reinforcements = data::ARENA_REINFORCEMENTS;
        // Docked again from the start: the berth moved with the hull, the
        // joined deck is the new one, and the residents' room — opened on
        // the old design — is opened again on this.
        self.undock_for_probe();
        self.residents = None;
        self.ship.state = ShipState::Docked { station: id };
        self.dock_at(id);
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
        let mut residents = Residents::open(
            id,
            &station.design,
            count,
            mercs,
            station.map_seed,
            self.clock_minutes,
        );
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

fn refused(slot: u32, why: Refusal) -> WorldEvent {
    WorldEvent::Refused { slot, why }
}

/// What a room banked since last asked, folded into the hold: the
/// bandages and medkits used off their counts, the fibre harvested onto the shelf as far
/// as it has room. `true` when anything moved, so the caller knows to run
/// `on_ship_changed`. Asked of the crew's room every step
/// (`take_the_room_s_medicine`), and of a room about to be thrown away —
/// a docking or a casting off replaces the whole room — *after* the crew
/// have been taken out of it, since giving up an errand is what puts the
/// sheaf in somebody's hands into the store.
fn bank_medicine(design: &mut ShipDesign, room: &mut bims::game::Game) -> bool {
    let used = room.take_bandages_used();
    let kits = room.take_medkits_used();
    let grown = room.take_harvested_fibre();
    if used == 0 && kits == 0 && grown == 0 {
        return false;
    }
    let bandages = &mut design.cargo[ResourceId::Bandage as usize];
    *bandages = bandages.saturating_sub(used);
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
