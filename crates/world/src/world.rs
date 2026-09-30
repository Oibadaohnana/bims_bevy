//! The world, and the one loop that advances it.
//!
//! # One clock
//!
//! There is a single [`World::step`] and it moves a mission on by exactly
//! [`data::STEP_MINUTES`] of the mission clock. What the crew have seen, the
//! machines, the crew themselves, power and construction all happen inside
//! it, in a fixed order, at the same instant. Nothing in this crate may grow
//! a clock of its own or a loop of its own; a second one is two simulations
//! that will disagree. The world clock moves only when the crew travel
//! (`mission.rs`, `crate::run`): a trip is resolved, not flown.
//!
//! The order inside a step is written out in [`World::step`]. The crew are
//! in it — spawned at their bunks when the world opens, stepped in the
//! fifth stage — and construction is the seventh.
//!
//! # Where the ship is
//!
//! Two numbers and a rule. [`Ship::anchor`] is where design tile (0, 0) sits
//! in the system, [`Ship::heading`] is which way the ship is pointing, and the
//! ship's *position* — the thing a berth sets and the camera is centred on —
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
//! while docked, a recipe at a bench, and crew joining or leaving.
//! Construction and deconstruction move materials from the hold into the hull
//! and back — `shipdesign::materials` is that contract — and change the
//! centre of mass and the inertia without changing the total.

use economy::market::{self, Quote};
use economy::{Money, trade_price};
use flight::{Dynamics, angle};
use physics::ResourceId;
use shipdesign::parts::{PartKind, Rotation};
use shipdesign::{CARGO_SLOTS, ShipDesign, design_hash};
use worldgen::math::{DVec2, dvec2};
use worldgen::{Galaxy, GalaxyType, Node, StarSystem};

use bims::combat::{ArmourKind, Item, Sentry, Tier, Weapon, WeaponKind};
use bims::game::Container;
use bims::health::Part;
use bims::order::CrewOrder;
use bims::sight::Stance;

use crate::armour::{self, LootSource};
use crate::build::{self, BuildSite, SiteRefusal};
use crate::class::{self, Charge, Class, Progress};
use crate::commander::{Aura, Commander};
use crate::crew::{Aboard, Residents};
use crate::data;
use crate::defense::{self, Defense};
use crate::deploy::{Deck, DeployKind, Deployable};
use crate::droid::{self as droidplan, Infestation};
use crate::event::{Refusal, WorldEvent};
use crate::frame::{self, Frame};
use crate::heart;
use crate::holdings::{self, GearSlot, GearSource, Holdings};
use crate::jammer;
use crate::medic::Medic;
use crate::memory::{self, Grave, Losses, SystemMemory};
use crate::mercenary::{self, Hired, Offer};
use crate::orders::Standing;
use crate::run::{self, Run, SiteKind};
use crate::speed::{self, Speed};
use crate::station::{Berth, Station};
use crate::surface::{self, Surface};
use crate::tank::Tank;

// The run's half of the world (feature 103): travel, a mission's start
// and end, dying and buying back. A child of this module, so it reaches
// the fields the rest of the `impl World` blocks here do.
#[path = "mission.rs"]
mod mission;

// The relics' half of the world (feature 106): what a relic does, the
// choosing, the caches and the win. A child for the same reason.
#[path = "relics.rs"]
mod relics;

// What task 118's relics do in a fight: the hits on a machine, the auras,
// the timed effects, the pay. A child for the same reason.
#[path = "relic_hooks.rs"]
mod relic_hooks;
#[cfg(test)]
pub(crate) use relic_hooks::MachineKill;

// The Machine Heart's half of the world (feature 108): the fortress, its
// fight, the win and the map's preview. A child for the same reason.
#[path = "fortress.rs"]
mod fortress;
pub use fortress::{HeartStatus, RunSummary};

// The Manufacturers' half of the world (feature 109): which sites of this
// system are theirs, their people laid on the deck, and the fight's few
// differences from the machines'. A child for the same reason.
#[path = "garrison.rs"]
mod garrison;

// The trader's half of the world (task 114): which sites of the galaxy
// are traders, the visit on the map, the shelf, the relic's vote and the
// combining. A child for the same reason.
#[path = "trading.rs"]
mod trading;

// The machines' outposts (task 136): the sites of a system theirs from
// the first day, every other one. A child for the same reason.
#[path = "outposts.rs"]
pub mod outposts;

// What a system offers (task 135): one station and one town, and one
// fight a system. A child for the same reason.
#[path = "offered.rs"]
pub mod offered;

/// What a player can ask the world to do.
///
/// Every one of them carries the slot that sent it, because every one of them
/// can be sent by anybody: there is one ship and the crew run it together.
/// Nothing flies it: a trip is chosen on the world map between missions
/// ([`Command::Propose`], [`Command::Accept`]) and resolved in one go.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Command {
    SetSpeed {
        slot: u32,
        speed: Speed,
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
    /// Put a thing on crew member `who`'s loadout (task 113): out of the
    /// armory by its id, or off another Bim's slot — the slot it goes on
    /// is the one it is made for, a weapon the hand and a piece its part.
    /// What was there goes into the armory. Between missions, or in one
    /// while the Bim is inside the ship (`Refusal::GearLocked` out on the
    /// deck), and only onto — and off — the
    /// player's own Bim or a bot's (`Refusal::NotYours`): another
    /// player's Bim is given a thing by [`Command::Offer`].
    Equip {
        slot: u32,
        who: u32,
        from: GearSource,
    },
    /// Take what crew member `who` has on `part` off into the armory. The
    /// same rules as [`Command::Equip`].
    Unequip {
        slot: u32,
        who: u32,
        part: GearSlot,
    },
    /// Offer what that player's own Bim has on `part` to player `to`'s
    /// Bim: it moves when `to` accepts, the thing `to` had there going
    /// into the armory, and is withdrawn when either side changes that
    /// slot, the offerer takes it back or a mission starts. Between
    /// missions only.
    Offer {
        slot: u32,
        part: GearSlot,
        to: u32,
    },
    /// Answer the offer player `from` made of its `part`: the recipient
    /// accepts or declines it; the offerer saying no takes it back.
    AnswerOffer {
        slot: u32,
        from: u32,
        part: GearSlot,
        yes: bool,
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
    /// An order to the crew's room — a click on the deck, a walk, a row
    /// of a fixture's menu, a box on the Management tab — see
    /// [`bims::order::CrewOrder`]. A command because the crew's positions
    /// are in the checksum: every player's ship has to agree about who
    /// walked where, so nothing reaches into the room except through
    /// here. They go to `Game::order`. A walk with no way there is
    /// `Refused` with `NoWayThere`; every other order
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
    /// Choose that player's class (feature 74, `crate::class`): what its
    /// own crew member is. Allowed until the ship first leaves its
    /// berth, refused `ClassLocked` after. A class sets out with the
    /// charges its ranks give it (`World::charges_held`), and a class
    /// put back to none has none.
    SetClass {
        slot: u32,
        class: Class,
    },
    /// Buy a rank of that player's own crew member's ranked kit (task
    /// 124): `ability_slot` 0 to 3 for Q, C, E and R, one skill point
    /// spent. Refused `NoClass` for a classless crew member, `NoRankedKit`
    /// for a slot past R, `NoSkillPoint`, `TopRank` at
    /// [`class::MAX_RANK`] and `RankLocked` below the level the rank wants
    /// ([`class::rank_level`]). Heard between missions as well as in one.
    RankUp {
        slot: u32,
        ability_slot: u32,
    },
    /// Send that player's own engineer to lay sandbags (E) or a Healing
    /// Sentry (C) — `kind`, never the sentry, which is `Command::Sentry`
    /// — on the tile `(x, y)` of the crew's room: a room tile, which the
    /// world puts on the ship's deck or the station's by where it lies.
    /// Wants the Bim fit to act (`OutOfReach`), an engineer
    /// (`NotAnEngineer`), the ability at rank one (`NotLearnt`), a charge
    /// (`NoKit`) and a tile of reachable deck floor that is not a door,
    /// holds no blocking part and no deployable (`CantDeployThere`). The
    /// Bim walks beside it and works there — see `crate::deploy`.
    Deploy {
        slot: u32,
        kind: DeployKind,
        x: i32,
        y: i32,
    },
    /// That player's own engineer lays its **sentry** (task 127, its
    /// ultimate, R) on the room tile `tile`: refused `NotAnEngineer`,
    /// `OutOfReach` (not fit to act, downed among it), `NotLearnt` at rank
    /// nought, `CoolingDown` within its rank's [`class::SENTRY_COOLDOWN`]
    /// of the last one laid, and `CantDeployThere` for a tile that will
    /// not take it. A hit does not interrupt the laying.
    Sentry {
        slot: u32,
        tile: (i32, i32),
    },
    /// That player's own engineer throws an **EMP** (task 127, Q) at the
    /// room tile `(x, y)`: the grenade's range and fuse, refused as a
    /// throw is (`OutOfReach`, `NotAnEngineer`, `NotLearnt`, `NoKit` with
    /// no charge, `CantThrowThere`, `OutOfThrowRange`, `NoLineToTile`).
    /// It harms nothing; it stuns every enemy machine in its radius.
    Emp {
        slot: u32,
        x: i32,
        y: i32,
    },
    /// Take one of that player's own engineer's sandbags or Healing
    /// Sentries (`id`) back up: the engineer fit to act and within
    /// [`data::REACH`] of it, the charge back, capped at its charges. The
    /// sentry is never taken up (`NoSuchDeployable`).
    PackUp {
        slot: u32,
        id: u32,
    },
    /// Brace that player's own soldier, or stand it easy (feature 75,
    /// `crate::class`): braced, it holds where it stands — no errands,
    /// no running, its misses cut by [`class::BRACE_MISS_CUT`] of its
    /// rank and the damage it takes by [`class::BRACE_DAMAGE_TAKEN`] —
    /// until this with `on` false, an order that moves it, or going down.
    /// Refused `NotASoldier` for anybody else, `OutOfReach` for one not
    /// fit to act and `NotLearnt` at rank nought (task 124).
    Brace {
        slot: u32,
        on: bool,
    },
    /// Throw a grenade from that player's own soldier's pack at the tile
    /// `(x, y)` of the crew's room — a room tile like a deploy's. Wants
    /// the soldier fit to act, a rank of Frag Grenade (task 124), a
    /// grenade charge in the pack — which is the whole of the cooldown
    /// since feature 90: the charges go one after the other and each comes
    /// back [`class::GRENADE_COOLDOWN`] of its rank after it is thrown — and a
    /// tile of deck within its range with nothing opaque between
    /// (`World::can_throw`). The grenade leaves the pack at once and
    /// bursts its fuse later.
    Throw {
        slot: u32,
        x: i32,
        y: i32,
    },
    /// That player's own Bim throws at the tile `(x, y)` of the crew's
    /// room — a grenade, or with `emp` an engineer's EMP — **walking
    /// first where it must**: within reach and with a line to the tile it
    /// is [`Command::Throw`] (or [`Command::Emp`]) at once; out of reach
    /// or behind a wall it walks to the nearest spot it can throw from
    /// (`World::throw_stand`) and throws the step it can
    /// ([`World::throws`]). Called off by any other order that moves it,
    /// by it going down, and by a spot it walked to that will not do. The
    /// other refusals are the throw's own, said at once.
    ThrowAt {
        slot: u32,
        emp: bool,
        x: i32,
        y: i32,
    },
    /// That player's own soldier goes on a **Rampage** (task 124, its
    /// ultimate, R): for [`class::RAMPAGE_SECONDS`] of its rank on the
    /// mission clock it fires faster, takes less and aims on the move as
    /// if standing. Refused `NotASoldier`, `OutOfReach` (not fit to act,
    /// downed among it), `NotLearnt` at rank nought, `AlreadyActive`
    /// while one runs and `CoolingDown` within [`class::RAMPAGE_COOLDOWN`]
    /// of the last.
    Rampage {
        slot: u32,
    },
    /// Link that player's own medic's heal beam to crew member `patient`
    /// — a player's Bim or a mercenary, or itself; never an enemy —
    /// or unlink with `None` (feature 76, `crate::class`,
    /// `crate::medic`). Wants the medic fit to act and the patient
    /// alive, within [`class::HEAL_BEAM_RANGE`] tiles and in its sight
    /// (`World::can_beam`) — or the medic itself (task 120). Linked, the
    /// patient gains [`class::HEAL_BEAM_HP`] hit points an hour of the
    /// clock times the rank's [`class::HEAL_BEAM_RATE`] (task 130); the
    /// medic walks but does not fire until the third rank, and then at
    /// half the rate. At the fourth a second patient is held beside the
    /// first, and a third takes the first's place.
    Beam {
        slot: u32,
        patient: Option<u32>,
    },
    /// Set off that player's own medic's **Nanite Burst** (task 130, Q):
    /// every friendly Bim on its feet within the rank's
    /// [`class::NANITE_BURST_RADIUS`] of him and in his sight, himself
    /// included, healed at once by the rank's
    /// [`class::NANITE_BURST_HEAL`]. Wants the medic fit to act and not
    /// downed, a rank, and the cooldown run out (`World::can_nanite_burst`).
    NaniteBurst {
        slot: u32,
    },
    /// **Cloak** crew member `target` — that player's own medic's ultimate
    /// (task 130, R): the crewmate under the pointer within
    /// [`class::CLOAK_RANGE`] tiles and in his sight, downed or not, or the
    /// medic himself, which the app sends with nobody under the pointer.
    /// Wants the medic fit to act and not downed, a rank, and the cooldown
    /// run out (`World::can_cloak`).
    Cloak {
        slot: u32,
        target: u32,
    },
    /// Stand that player's own tank as a wall, or stand it down (feature
    /// 77; E of his ranked kit since task 139, `crate::class`,
    /// `crate::tank`): with it on he walks at the rank's
    /// [`class::BULWARK_PACE`] and a crewmate within its
    /// [`class::BULWARK_REACH`] of him that he stands between and the
    /// shooter is in cover against the shot. Refused `NotATank` for
    /// anybody else, `OutOfReach` for one not fit to act and `NotLearnt`
    /// at rank nought.
    Bulwark {
        slot: u32,
        on: bool,
    },
    /// That player's own tank **taunts** (Q, task 139): for the rank's
    /// [`class::TAUNT_SECONDS`] of the mission clock every enemy within
    /// its [`class::TAUNT_RADIUS`] that can see him shoots at him and
    /// nobody else. Refused `NotATank`, `OutOfReach` (not fit to act,
    /// downed among it), `NotLearnt` at rank nought and `CoolingDown`
    /// within the rank's [`class::TAUNT_COOLDOWN`] of the last
    /// (`World::can_taunt`).
    Taunt {
        slot: u32,
    },
    /// That player's own tank goes **Juggernaut** (R, his ultimate, task
    /// 139): for the rank's [`class::JUGGERNAUT_SECONDS`] every enemy
    /// that can see him, at any distance, shoots at him and nobody else,
    /// and he takes the rank's [`class::JUGGERNAUT_DAMAGE_TAKEN`]. Refused
    /// `NotATank`, `OutOfReach` (not fit to act, downed among it),
    /// `NotLearnt`, `AlreadyActive` while one runs and `CoolingDown`
    /// within [`class::JUGGERNAUT_COOLDOWN`] of the last
    /// (`World::can_juggernaut`).
    Juggernaut {
        slot: u32,
    },
    /// That player's own commander **rallies** (task 129, his E): for
    /// [`class::RALLY_SECONDS`] of his rank on the mission clock every
    /// friendly Bim within [`class::RALLY_TILES`] of him as he calls it —
    /// himself and a player's own included — takes
    /// [`class::RALLY_DAMAGE_TAKEN`] of what hits it and walks at
    /// [`class::RALLY_PACE`]. Refused `NotACommander`, `OutOfReach` (not
    /// fit to act, downed among it), `NotLearnt` at rank nought and
    /// `CoolingDown` within [`class::RALLY_COOLDOWN`] of the last
    /// (`World::can_rally`).
    Rally {
        slot: u32,
    },
    /// That player's own commander calls a **Battle Cry** (task 129, his
    /// Q): for [`class::BATTLE_CRY_SECONDS`] of his rank on the mission
    /// clock every friendly Bim within [`class::BATTLE_CRY_TILES`] of him
    /// as he calls it — himself included — fires at
    /// [`class::BATTLE_CRY_FIRE_RATE`]. Refused as a rally is
    /// (`World::can_battle_cry`).
    BattleCry {
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
    /// ([`World::can_carry`]); the body has to be a crewmate that is
    /// downed, within
    /// [`bims::game::CARRY_REACH`] tiles, and in nobody else's arms. Carried,
    /// a body walks nowhere of its own and the carrier holds its fire
    /// and walks at [`bims::game::CARRY_PACE`] — the point of it being
    /// to get somebody out of the fire and revive them where it is
    /// quiet. A set down with empty arms does nothing and says so.
    Carry {
        slot: u32,
        who: Option<u32>,
    },
    /// Put a destination to the crew, between missions (feature 103,
    /// [`crate::run`]): a station or a settlement in this system, or in
    /// one a hyperlane hop away. It replaces whatever was on the table,
    /// every acceptance with it, and counts as the proposer's own yes.
    Propose {
        slot: u32,
        star: u32,
        station: u32,
    },
    /// Say yes to the destination on the table, or take a yes back. The
    /// last yes of every connected player is the trip: the world clock put
    /// on by its length and the crew arriving there, in that instant.
    Accept {
        slot: u32,
        yes: bool,
    },
    /// *Back to ship*: this player is done here. The first press sends
    /// every bot home; the departure check runs once every standing
    /// player has pressed it and is aboard — or, once the fight is won,
    /// once every player alive has pressed it, wherever they are (task
    /// 133). Pressed again, it asks a turned-down departure again.
    Return {
        slot: u32,
    },
    /// This player's answer to the departure check: leave the ones
    /// outside the ship behind, or not.
    LeaveBehind {
        slot: u32,
        yes: bool,
    },
    /// The host saying that player has left the game: a vote no longer
    /// waits for them, and the departure check does not either. `slot`
    /// is the player gone, not the one saying so.
    PlayerGone {
        slot: u32,
    },
    /// *Ready* for the mission held for the ready check, or taken back
    /// ([`crate::run::Run::briefing`]). The last yes of every connected
    /// player starts it.
    Ready {
        slot: u32,
        yes: bool,
    },
    /// Put a relic on offer to the crew for player `to`'s Bim (feature
    /// 106, [`crate::relic`]) — `relic` a [`crate::Relic`] code, or
    /// `u32::MAX` for taking none. It replaces whatever was on the table,
    /// every acceptance with it, and counts as the proposer's own yes. A
    /// relic not on offer, or a Bim no player steers, is refused.
    ProposeRelic {
        slot: u32,
        relic: u32,
        to: u32,
    },
    /// Say yes to the relic on the table, or take a yes back. The last
    /// yes of every connected player gives it.
    AcceptRelic {
        slot: u32,
        yes: bool,
    },
    /// Crew member `who` opens the relic cache on the research desk of
    /// the site the crew are at: within reach of it, and no relic choice
    /// being made already. One relic of the site's tier is put to the
    /// crew, pending until the site is cleared.
    OpenCache {
        slot: u32,
        who: u32,
    },
    /// Buy what is in slot `index` of the shelf of the trader the crew
    /// are at (task 114, [`crate::trader`]), out of the pool: onto crew
    /// member `to`'s loadout — the player's own Bim or a bot, what was
    /// on the slot going into the armory — or into the armory with
    /// `None`. Any player, no vote. Refused when the pool cannot pay or
    /// the thing is gone: the first command to want it has it.
    BuyShelf {
        slot: u32,
        index: u32,
        to: Option<u32>,
    },
    /// Combine two weapons or two pieces of armour of one kind and one
    /// tier into one of the next, at a trader (task 114) — what the
    /// workbench did. Each out of the armory or off the player's own Bim
    /// or a bot, never another player's; at once, for
    /// [`data::COMBINE_FEE`]. A tier three combines into nothing.
    Combine {
        slot: u32,
        a: GearSource,
        b: GearSource,
    },
    /// Roll the shelf of the trader the crew are at again, its weapons and
    /// its armour — never its relic: a relic's *Restock Codes* (task 118),
    /// once a visit, any player while one of them holds it.
    Restock {
        slot: u32,
    },
}

/// What the ship is doing. Nothing is flown (feature 104): the ship is
/// tied up at a site for a mission, or holding off one between missions
/// while the crew choose where next — a trip is resolved, and the crew
/// arrive docked (`mission.rs`).
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ShipState {
    /// Alongside a station, where money works. The centre of mass sits on the
    /// station's own position.
    Docked { station: u32 },
    /// Stopped, somewhere. Not docked: a station with no airlock is somewhere
    /// to hold beside rather than somewhere to go aboard.
    Holding,
}

impl ShipState {
    /// The number that crosses the wasm boundary.
    pub fn code(&self) -> u32 {
        match self {
            ShipState::Docked { .. } => 0,
            ShipState::Holding => 1,
        }
    }

    /// The station the ship is tied up at with the rooms joined. What the
    /// painter draws the airlocks mated for.
    pub fn alongside(&self) -> Option<u32> {
        match self {
            ShipState::Docked { station } => Some(*station),
            ShipState::Holding => None,
        }
    }

    /// The station the ship is at: the one it is tied up at. What the
    /// local frame and the residents' room are about.
    pub fn station(&self) -> Option<u32> {
        self.alongside()
    }
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
    pub frame: Frame,
    /// What is in the batteries, in power units — never more than what
    /// the wired batteries hold. Full when the world opens, and moved by
    /// [`World::run_power`] alone; see [`Power`].
    pub charge: f64,
}

impl Ship {
    /// Where the ship is: its centre of mass, in system coordinates.
    pub fn position(&self) -> DVec2 {
        self.anchor.add(angle::rotate_design(
            self.dynamics.centre_of_mass,
            self.heading,
        ))
    }

    /// Put the centre of mass there, by moving the anchor under it.
    fn set_position(&mut self, at: DVec2) {
        self.anchor = at.sub(angle::rotate_design(
            self.dynamics.centre_of_mass,
            self.heading,
        ));
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
    /// The crew's takings not yet shared out: what a fight pays goes in
    /// here, and at the end of every mission it is shared evenly into
    /// the players' [`World::wallets`] (`share_out`), what does not
    /// divide left for the next. The design phase's pool, which it was,
    /// is shared out the same way when the world opens. Only a build
    /// site still pays out of it.
    pub money: Money,
    /// Each player's own money, slot for slot: what they buy with at
    /// their own trader, combine and hire with, and what a buyback draws
    /// on — one player's spending never drains another's.
    #[cfg_attr(feature = "serde", serde(default))]
    pub wallets: Vec<Money>,
    /// The crew's hits on the machines landed through the relics this step
    /// (`land_on_machines`), for `visit` to say as `WorldEvent::Hit`s: on
    /// whom, how much, and whether critical. A picture's; never saved or
    /// hashed.
    #[cfg_attr(feature = "serde", serde(skip))]
    shown_hits: Vec<(u32, f32, bool)>,
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
    /// The probes' override of what tier the machines come at
    /// (`BIMS_DROID_TIER`). `None` — the game's own — leaves it to how far
    /// the system is from the origin (feature 93,
    /// [`World::droid_tier`], [`data::DROID_TIER_THREE_HOPS`]). What
    /// `world_checksum` eats is the answer rather than this, since it is
    /// the size of the fight.
    droid_tier: Option<Tier>,
    /// How long after a wave is spent the next arrives, in steps of the
    /// mission clock: [`data::DROID_REINFORCE_STEPS`], bar the probes,
    /// which shorten it to a minute so a wave can be watched arriving. In
    /// `world_checksum` for the same reason.
    droid_reinforce: u64,
    /// A wave forced to a size by a probe (`BIMS_DROID_WAVE`), whatever
    /// the formula says: the measurements' dial, `None` in the game. In
    /// `world_checksum` with the rest.
    droid_wave_forced: Option<u32>,
    /// The wave formula's dials ([`droidplan::WaveScaling`]), the
    /// constants unless the app's `scaling.ron` says otherwise — tuning
    /// while the game runs. **Neither saved nor hashed**: the app hands
    /// them over again every frame they differ, a load and a restart
    /// included, and what they decide (`Infestation::waves_left`, the
    /// machines laid) is what is kept.
    #[cfg_attr(feature = "serde", serde(skip))]
    wave_scaling: droidplan::WaveScaling,
    /// The run's difficulty as the game setup picked it
    /// ([`droidplan::Difficulty`]): the base machines a wave, a player's
    /// and a time step's, laid over `wave_scaling`; `None` is the tuning
    /// file's own. **Saved** — a load plays at the difficulty the run was
    /// begun at — **but not hashed**, like `wave_scaling`: every machine
    /// of a lobby is dealt the same at Start, and what it decides (the
    /// machines laid) is hashed.
    #[cfg_attr(feature = "serde", serde(default))]
    difficulty: Option<droidplan::Difficulty>,
    /// What a fight pays and what things cost ([`crate::rewards::Rewards`]),
    /// the constants unless the app's `rewards.ron` says otherwise.
    /// Neither saved nor hashed, like `wave_scaling`: the money and the
    /// experience they decide are.
    #[cfg_attr(feature = "serde", serde(skip))]
    rewards: crate::rewards::Rewards,
    /// How many waves a held station has all told, forced by a probe
    /// (`BIMS_DROID_WAVES`, and three on the `droids` commands) whatever
    /// the formula says; `None` in the game. **Neither saved nor
    /// hashed**, unlike the three above: it is read once, at the crew's
    /// first dock, and what it decides is `Infestation::waves_left`,
    /// which is both.
    #[cfg_attr(feature = "serde", serde(skip))]
    droid_waves_forced: Option<u32>,
    /// Every wave forced to be exactly these machines, in this order, by
    /// a probe — the `guardian` command's one Guardian and two Troopers
    /// (feature 100) — whatever the tier and the mix say; `None` in the
    /// game. Saved, since a restart must bring the same waves again, and
    /// not hashed: what it decides is the machines in the residents'
    /// room, which the checksum leaves out like everything of that room.
    #[cfg_attr(feature = "serde", serde(default))]
    droid_kinds_forced: Option<Vec<bims::droid::DroidKind>>,
    /// The probes' word that a defence's waves are **the machines'**
    /// whatever the day (task 131): before day ten they are the
    /// Manufacturers' in the game, and the tests of the machines'
    /// fight at a defence run at day nought. Saved and not hashed, as
    /// `droid_kinds_forced` is.
    #[cfg_attr(feature = "serde", serde(default))]
    defense_by_machines_forced: bool,
    /// The probes' word that the run's first mission is **not** eased
    /// (`WaveScaling::first_mission_ease`): the app's `end` command,
    /// whose first mission is the Machine Heart. Saved and not hashed,
    /// as `droid_kinds_forced` is: what it decides is the machines laid.
    #[cfg_attr(feature = "serde", serde(default))]
    first_mission_uneased: bool,
    /// Where the machines began (feature 92): the one star the crisis
    /// spreads out from, rolled once at [`World::start`]
    /// ([`droidplan::origin`]) at least [`data::DROID_ORIGIN_MIN_HOPS`]
    /// from the crew's own. Saved and in `world_checksum`: two clients
    /// that disagreed about it would disagree about which half of the
    /// galaxy is falling. Read by [`World::droid_origin`].
    droid_origin: u32,
    /// How many lane hops every star is from [`World::droid_origin`],
    /// indexed by star id — the whole of the spread rule, since a star's
    /// day is `crisis_first_day + DROID_SPREAD_DAYS * hops`.
    /// **Derived, never saved**: worked out from the galaxy at the start
    /// and again at every load ([`World::settle_crisis`]), which is why a
    /// save carries the origin and not this.
    #[cfg_attr(feature = "serde", serde(skip))]
    droid_hops: Vec<u16>,
    /// How many lane hops every star is from the crew's own, [`World::home_star`],
    /// indexed by star id: what tier two is ramped on (feature 106,
    /// [`World::site_tier`]). Derived and never saved, as `droid_hops` is.
    #[cfg_attr(feature = "serde", serde(skip))]
    home_hops: Vec<u16>,
    /// The sites made the Manufacturers' near the crew's own star on top
    /// of the galaxy's roll (feature 109, [`crate::manufacturer::near_sites`]),
    /// `(star, station)` pairs. Derived off the galaxy and
    /// [`World::home_star`] at the start and at every load, as `home_hops`
    /// is, and never saved.
    #[cfg_attr(feature = "serde", serde(skip))]
    manufacturer_near: Vec<(u32, u32)>,
    /// The sites made traders near the crew's own star on top of the
    /// galaxy's roll (task 114, [`crate::trader::near_sites`]): derived
    /// at the start and at every load behind `manufacturer_near`, which
    /// it reads, and never saved.
    #[cfg_attr(feature = "serde", serde(skip))]
    trader_near: Vec<(u32, u32)>,
    /// The day the origin turns: **nought** — the crisis is there from
    /// the start (feature 102) — bar the `crisis` probe
    /// (`BIMS_CRISIS_DAY`), which moves it. In `world_checksum` with the
    /// droids' other dials, since it is when the whole galaxy falls.
    crisis_first_day: u32,
    /// The towns the machines are attacking and the crew are defending
    /// (feature 94), by station id, sorted — one [`Defense`] a town the
    /// crew have ever landed at while it was threatened, kept for good
    /// so a fight paused by a take-off is resumed where it stood. Saved
    /// and in `world_checksum`. **This system's alone**, like
    /// [`World::infested`]: a station id is a system's own, so the list
    /// goes onto [`SystemMemory`] at a jump and comes back with the
    /// system (`remember_system`, `recall_system`).
    defenses: Vec<Defense>,
    /// The towns the crew **held**: the last machine of the last wave
    /// destroyed. Sorted station ids, saved and in `world_checksum` —
    /// a held town stays friendly, trades and hires even after its
    /// system has fallen, so the crisis reads this before it flips one.
    /// This system's alone, the same way as `defenses`: carried across a
    /// jump it held the next system's town of the same id.
    held_towns: Vec<u32>,
    /// How long after the crew land at a threatened town the first wave
    /// comes, in steps of the mission clock:
    /// [`data::DEFENSE_DELAY_STEPS`], bar the `defense` probe. Saved and
    /// in `world_checksum` beside the machines' own dials, since when a
    /// wave lands is the fight.
    defense_delay: u64,
    /// The tests' dial (task 111, [`World::set_quiet_sites_for_probe`]):
    /// every site that is neither a trader nor held by an enemy is a
    /// peaceful stop rather than a defence, for the tests whose subject is
    /// not the fight. Off in every run. Saved, so a save a test reads back
    /// is the world it wrote, and not hashed: it is a test's condition,
    /// not a state of the game.
    #[cfg_attr(feature = "serde", serde(default))]
    quiet_sites: bool,
    /// The tests' other dial (task 135, [`World::set_whole_systems_for_probe`]):
    /// every system keeps every station and town the generator made, as
    /// before a system offered one of each, with no one-fight rule. Off in
    /// every run. Saved, so a load settles the same system, and not hashed.
    #[cfg_attr(feature = "serde", serde(default))]
    whole_systems: bool,
    /// A site made an elite by a probe ([`World::set_elite_for_probe`]),
    /// beside the galaxy's roll. Saved, so a restart brings it again, and
    /// not hashed, like the forced wave kinds.
    #[cfg_attr(feature = "serde", serde(default))]
    elite_forced: Option<run::Site>,
    /// The ship's power over its live networks, worked out from the parts
    /// once per change to them — `on_ship_changed` — rather than once a
    /// step: it is a union-find over every tile of the grid, and the
    /// parts change when something is built and not otherwise.
    power_budget: shipdesign::PowerBudget,
    /// What the crew have seen, shared between all of them and never
    /// forgotten. Sorted, so a checksum over it means something.
    pub discovered: Vec<Node>,
    /// One per player, in slot order: 1× or paused (task 119), and a pause
    /// by any of them pauses the world.
    pub speed_requests: Vec<Speed>,
    /// The parts laid out to be built and not built yet, in the order they
    /// were laid out — which is the order the crew take them in. See
    /// [`crate::build`]. In `world_checksum` whole.
    pub builds: Vec<BuildSite>,
    /// The next site's id. Only ever climbs, like a part's.
    pub next_site: u32,
    /// The station the crew set out from: the one place they are at home,
    /// and friendly while the machines do not hold it.
    pub home: u32,
    /// The star `home` is at. A jump takes the ship to a system with ids of
    /// its own, and home is only home while the ship is at its star.
    pub home_star: u32,
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
    /// The ship's holdings bar the money (task 113, [`crate::holdings`]):
    /// the armory — every weapon and piece of armour nobody wears — the
    /// research keys picked up, and the offers between players standing.
    /// In `world_checksum` whole.
    pub holdings: Holdings,
    /// What the ship and its hold were worth when the world opened —
    /// [`World::worth`] at step nought — fixed for the whole game. Every
    /// half of it the crew's worth has grown by since is more hands for
    /// hire (`crate::mercenary::how_many`); the machines never read it
    /// (feature 105, `crate::droid::wave_size`). Not in `world_checksum`: it
    /// is a function of the design the world started on, which two
    /// clients share.
    pub start_worth: Money,
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
    /// What a probe told the reactors to make instead of what they make
    /// (`throttle_reactors_for_probe`), kept across `on_ship_changed`.
    /// Never set by the game.
    probe_supply: Option<f64>,
    /// Whether the run is over: no crew member standing — dead or out
    /// cold, every one — said once as [`WorldEvent::CrewLost`] and kept,
    /// since the app ends the run on it. In `world_checksum`.
    pub lost: bool,
    /// What every station of this system has lost to the crew — its own
    /// people dead, its mercenaries hired away or dead — by the station's
    /// id, sorted. Added to whenever a station's room is closed
    /// ([`World::close_residents`]) and taken off the crowd the room is
    /// opened with again ([`World::people_of`], [`World::mercenaries_of`]),
    /// so a station's dead stay dead. See [`crate::memory`]. In
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
    /// the keys, the stations' lamps, the chart, the losses, the dead and
    /// the machines' hold — filed by star at the jump out
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
    /// How many of each [`class::Charge`] each crew member holds, by
    /// index and by the charge's code (task 127): a **counter**, never a
    /// thing in a pack. Raised one at a time by `World::restock_charges`,
    /// lowered by a deployable laid or an EMP or a grenade thrown, raised
    /// by a pack-up. In `world_checksum`.
    #[cfg_attr(feature = "serde", serde(default))]
    pub charges_held: Vec<[u32; Charge::CODES]>,
    /// When the cooldown on each crew member's next of each
    /// [`class::Charge`] began, in mission minutes, or `None` for one not
    /// running (features 88 and 90) — one entry a charge, by its code.
    /// `World::restock_charges` keeps it, and it is in `world_checksum`:
    /// a charge waiting is a different fight from one held.
    pub charge_timers: Vec<[Option<f64>; Charge::CODES]>,
    /// Each crew member's engineer state, by index (task 127,
    /// `crate::engineer`): when its sentry was last laid. Empty for
    /// anybody but a player's engineer. In `world_checksum`.
    #[cfg_attr(feature = "serde", serde(default))]
    pub engineers: Vec<crate::engineer::Engineer>,
    /// Each crew member's medic state, by index (feature 76,
    /// `crate::medic`): who its beam holds, and when it last set off a
    /// Nanite Burst and cloaked somebody (task 130). Empty for anybody but
    /// a player's medic. In `world_checksum` whole.
    pub medics: Vec<Medic>,
    /// Each crew member's **cloak**, by index (task 130,
    /// `crate::medic::Cloak`): when it ends and how fast it walks while
    /// it lasts. Default for anybody not cloaked. In `world_checksum`
    /// where any is.
    #[cfg_attr(feature = "serde", serde(default))]
    pub cloaks: Vec<crate::medic::Cloak>,
    /// Each crew member's tank state, by index (feature 77,
    /// `crate::tank`): when he last taunted, which is the whole of it.
    /// Empty for anybody but a player's tank. In `world_checksum`.
    pub tanks: Vec<Tank>,
    /// Each crew member's commander state, by index (feature 78,
    /// `crate::commander`): when his last Battle Cry and Rally began and
    /// whom each reached (task 129). Empty for anybody but a player's
    /// commander. In `world_checksum`.
    pub commanders: Vec<Commander>,
    /// The Bims the commanders brought to this mission (task 129, their
    /// Reinforcements), by crew index, lowest first: laid at the
    /// mission's start, gone from the deck when one dies and off the crew
    /// at the mission's end. Empty between missions. Saved, and in
    /// `world_checksum` where there are any.
    #[cfg_attr(feature = "serde", serde(default))]
    pub reinforcements: Vec<crate::commander::Reinforcement>,
    /// Each crew member's soldier state, by index (task 124,
    /// `crate::soldier`): when its last Rampage began and when the one
    /// running ends. Empty for anybody but a player's soldier. Saved and
    /// in `world_checksum`.
    #[cfg_attr(feature = "serde", serde(default))]
    pub soldiers: Vec<crate::soldier::Soldier>,
    /// The throws a crew member is walking out to make ([`Command::ThrowAt`]):
    /// one at most a crew member, sorted by who. Saved, and in
    /// `world_checksum` where there is any.
    #[cfg_attr(feature = "serde", serde(default))]
    pub throws: Vec<PendingThrow>,
    /// The stream Weak Spot's critical hits are rolled off (task 124):
    /// seeded from the galaxy's seed, used for nothing else, lent to the
    /// crew's room for its step and taken back after, so no other roll of
    /// a fight moves because a soldier has it. Saved and in
    /// `world_checksum`.
    pub crit_rng: bims::rng::Rng,
    /// What each player's bots are under, by player slot (feature 84,
    /// `crate::orders`): [`Standing::Follow`] for a slot that has said
    /// nothing, which is every slot until somebody presses a key. As
    /// long as there are players, and in `world_checksum`.
    pub standing: Vec<Standing>,
    /// Whether the crew build onto their ship and buy what the ship lives
    /// on (feature 102): a construction site placed, and the goods on a
    /// station's shelf — food, suits, medicine — bought. **Off in every
    /// run**: the ship is the default one and stays it, bar the class
    /// deployables, and a desk sells the crew gear and nothing else. On
    /// only for the tests of building and of the shelf. Saved and in
    /// `world_checksum`.
    shipyard_enabled: bool,
    /// The run (feature 103, [`crate::run`]): in a mission or between
    /// them, the mission clock, the bounty waiting on the site being
    /// cleared, the destination on the table and who has accepted it, who
    /// has pressed *Back to ship*, the departure check, and the dead
    /// players waiting to be bought back. Saved and in `world_checksum`
    /// whole.
    pub run: Run,
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

/// What [`World::beam_for_probe`] leaves the patient's bar at, as a
/// share of whole: short enough that the beam has plenty to put back.
const BEAM_PROBE_HEALTH: f32 = 0.6;

/// What the galaxy's seed is salted with for Weak Spot's crit stream
/// (task 124, `World::crit_rng`): a stream of its own, so nothing else
/// draws off it.
const CRIT_SALT: u64 = 0x_C817_5EED_0124;

/// What the galaxy's seed is salted with for a reinforcement's face (task
/// 129): off the seed and the mission, never the room's stream, so laying
/// them moves no roll a fight makes.
const REINFORCEMENT_SALT: u64 = 0x_5E1F_0CE5_0129;

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
        // And the planets' settlements, rolled here rather than by the
        // generator (`crate::surface`).
        let surfaces = Surface::all_of(&system, seed);

        // Where the machines began, and how far every star is from it
        // (feature 92). Rolled here, off the galaxy still in hand: the
        // hop table is derived from the two and is worked out again at
        // every load rather than saved.
        let droid_origin = droidplan::origin(&galaxy, star_id);
        let droid_hops = galaxy.hops_from(droid_origin);
        let home_hops = galaxy.hops_from(star_id);
        // And which sites near home are the Manufacturers' on top of the
        // roll (feature 109): derived the same way.
        let manufacturer_near = crate::manufacturer::near_sites(&galaxy, star_id, true);

        let dynamics = flight::dynamics(&design, crew).map_err(StartError::NotAShip)?;
        let aboard = Aboard::new(&design, crew, seed);
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
            frame: Frame::Local(Node::Station(station_id)),
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
            wallets: Vec::new(),
            shown_hits: Vec::new(),
            ship,
            aboard,
            stations,
            surfaces,
            residents: None,
            infested: Vec::new(),
            droids_to_post: Vec::new(),
            droid_tier: None,
            droid_reinforce: data::DROID_REINFORCE_STEPS,
            droid_wave_forced: None,
            wave_scaling: droidplan::WaveScaling::DEFAULT,
            difficulty: None,
            rewards: crate::rewards::Rewards::DEFAULT,
            droid_waves_forced: None,
            droid_kinds_forced: None,
            defense_by_machines_forced: false,
            first_mission_uneased: false,
            droid_origin,
            droid_hops,
            home_hops,
            manufacturer_near,
            // Worked out below, once the world stands: it asks which sites
            // are the Manufacturers'.
            trader_near: Vec::new(),
            // The crisis is there from day nought (feature 102): the
            // origin is the machines' the moment the run opens, and every
            // star due by then with it.
            crisis_first_day: 0,
            defenses: Vec::new(),
            held_towns: Vec::new(),
            defense_delay: data::DEFENSE_DELAY_STEPS,
            quiet_sites: false,
            whole_systems: false,
            elite_forced: None,
            power_budget: shipdesign::power_budget(&design_for_charge),
            discovered: Vec::new(),
            // Everybody starts at real time. Anything else would have the
            // world already moving before the first player had looked at it.
            speed_requests: vec![Speed::Real; players as usize],
            builds: Vec::new(),
            next_site: 1,
            home: station_id,
            home_star: star_id,
            hired: Vec::new(),
            least_mercenaries: 0,
            crew_down: vec![false; crew as usize],
            crew_locked: vec![false; crew as usize],
            holdings: Holdings::new(),
            // Taken below, once the world stands: `worth` reads the
            // crew's gear and the pool as well as the ship.
            start_worth: 0,
            lamps: Vec::new(),
            powered_parts: shipdesign::powered_parts(&design_for_charge),
            browned_out: false,
            probe_supply: None,
            lost: false,
            losses: Vec::new(),
            graves: Vec::new(),
            visited: Vec::new(),
            memories: Vec::new(),
            classes: vec![Class::None; players as usize],
            progress: vec![Progress::default(); crew as usize],
            undocked_once: false,
            deployables: Vec::new(),
            next_deployable: 1,
            charges_held: vec![[0; Charge::CODES]; crew as usize],
            charge_timers: vec![[None; Charge::CODES]; crew as usize],
            engineers: vec![crate::engineer::Engineer::default(); crew as usize],
            medics: vec![Medic::default(); crew as usize],
            cloaks: vec![crate::medic::Cloak::default(); crew as usize],
            tanks: vec![Tank::default(); crew as usize],
            commanders: vec![Commander::default(); crew as usize],
            reinforcements: Vec::new(),
            soldiers: vec![crate::soldier::Soldier::default(); crew as usize],
            throws: Vec::new(),
            crit_rng: bims::rng::Rng::new(seed ^ CRIT_SALT),
            standing: vec![Standing::Follow; players as usize],
            // Feature 102: a run has no shipyard. The tests that are
            // about building switch it on (`set_shipyard_enabled`).
            shipyard_enabled: false,
            // Feature 103: the world opens in its first mission, at the
            // dock, with the world clock standing still until the crew
            // travel.
            run: Run::new(players),
        };

        // Whatever gear the design was accepted carrying is so many whole
        // things in the armory from the first step, and its research keys
        // the holdings' count: nothing is stored in the hold (task 113).
        world.stock_the_armory();
        // Which sites near home are traders on top of the roll (task 114):
        // derived, behind the Manufacturers' it reads.
        world.trader_near = world.trader_near_sites(&galaxy);
        // And the machines' jammer, if this system is already theirs —
        // which only a probe that wound the clock forward can arrange,
        // but the rule is the rule (feature 93). Before the chart below,
        // so the station is on it.
        world.settle_jammer();
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
        // Every player's share of the pool in their own hands.
        world.share_out();
        // What the crew set out with, for the enemies to be scaled
        // against — the **same** sum `worth` gives from then on, the
        // starting pool included, so unspent money is never counted as
        // growth (feature 95).
        world.start_worth = world.worth();
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

    /// Advance a mission by exactly one [`data::STEP_MINUTES`] of the
    /// mission clock. The world clock does not move here: only travel moves
    /// it (`mission.rs`).
    ///
    /// **The order below is the contract.** Later steps add their systems at
    /// the numbered places and nowhere else; what they must not do is
    /// introduce a second clock or a second loop, because then there are two
    /// answers to "what time is it aboard".
    ///
    /// Commands go first so that a command stamped for this step lands before
    /// anything moves — an order and the walk it causes are the same
    /// instant, not one step apart.
    pub fn step(&mut self, commands: &[Command]) -> Vec<WorldEvent> {
        let mut events = Vec::new();

        // 0. Between missions (feature 103) nothing moves: the map is up
        //    and the crew are choosing where next. The commands are heard
        //    — a vote, a speed — and the step is counted, since it is
        //    what a command is stamped with; nothing else happens.
        //    The reward screen (feature 106) is the same: a relic being
        //    chosen, nothing moving. And so is a mission held for the
        //    ready check: the room as it was met, the mission clock at
        //    nought, until every player has pressed *Ready*.
        if self.run.phase != run::Phase::Mission || self.run.briefing {
            for &command in commands {
                self.apply(command, &mut events);
            }
            self.steps += 1;
            return events;
        }
        //    And a fight won (task 133): from the step after the site is
        //    cleared the deck is frozen — the room is not stepped, so
        //    nobody moves and nobody downed bleeds out, and the mission
        //    clock stops. What is heard is what the ready check hears
        //    and *Back to ship*, and the departure check still runs: it
        //    takes everybody alive home from where they lie.
        if self.fight_over() {
            for &command in commands {
                self.apply(command, &mut events);
            }
            self.steps += 1;
            self.settle_run(&mut events);
            return events;
        }
        //    And the first step of a mission photographs the site before
        //    anything has moved: what leaving it uncleared puts back.
        self.open_the_mission();

        // 1. What the players asked for, in the order it arrived.
        for &command in commands {
            self.apply(command, &mut events);
        }

        // 2. The clocks. Everything below reads them; nothing below sets
        //    them. The **mission** clock runs with every step; the
        //    **world** clock runs only with travel (feature 103), and the
        //    hired hands' months are paid there (`pay_wages_due`).
        self.run.mission_steps += 1;
        self.steps += 1;

        // 3. The ship does not move: nothing is flown and nothing comes to
        //    it (feature 104). Where it is, is what is in range.
        let was = self.ship.position();
        let now = was;
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
        //    And the crisis (feature 92): a system whose day has come goes
        //    into the machines' hands the first step no room of the crew's
        //    is open in it. Before `settle_droids`, so a system that flips
        //    this step has its wave laid this step.
        self.spread_crisis(&mut events);
        self.settle_droids();
        self.droid_waves(&mut events);
        //    And, at a **threatened town the crew have landed at**
        //    (feature 94), the same clock again for a fight that is the
        //    town's rather than the machines': the first wave an hour
        //    after the landing, the next after each is destroyed, and the
        //    whole of it held where it stands while the ship is away.
        self.defense_waves(&mut events);

        // 5. Crew: the room's own update, aboard, on this clock. Bims live
        //    in ship-design coordinates and the ship's position, rotation and
        //    acceleration do not reach them. See `crates/world/src/crew.rs`.
        //    The station's residents, when the ship is near one, are the
        //    same room again on the same clock.
        //
        //    How many of the crew are players' own: a room is built
        //    afresh at every dock and undock and starts at one, so it is
        //    told every step, before it moves anybody (`bims::order`).
        self.aboard.room.set_players(self.players());
        //    And the crew's room is the one whose people revive one
        //    another of their own accord (task 120).
        self.aboard.room.set_revivers(true);
        //    And the construction sites, what each still wants, and who may
        //    go out to one beyond the hull. What the room did about them is
        //    read in stage 7.
        let builds = self.build_orders();
        let suit_ok = self.suit_ok();
        self.aboard.room.set_build_orders(builds, suit_ok);
        //    And the engineers' work (feature 74): the sandbags laid as
        //    cover on both rooms, and the sentries on the crew's deck to be
        //    fired there. What the fight did to them is read back after
        //    `visit`. And every class's charges: a spent sandbag, Healing
        //    Sentry, EMP or grenade comes back onto its counter on its own
        //    cooldown (features 88 and 90, task 127), before the boxes at
        //    the foot of the screen are read.
        self.restock_charges();
        self.hand_the_room_the_engineers();
        //    And what each class wears (feature 81): drawing only, said
        //    every step because a class is chosen, a crew member joins
        //    and a save is read without anything else telling the room.
        self.hand_the_room_the_outfits();
        //    And the medics' cloaks (task 130): one run out forgotten and
        //    the room told who is cloaked, before the beams, which a
        //    cloaked medic lets go.
        self.hand_the_room_the_cloaks();
        //    And the medics' (feature 76): every beam checked and the
        //    patients healed, before the soldiers' skills, since a
        //    medic beaming holds its fire through them.
        self.hand_the_room_the_medics(&mut events);
        //    And the engineers' Healing Sentries (task 127), beside the beam.
        self.heal_by_sentries();
        //    And which crew members are hired **field medics** (feature
        //    86), whose business under arms is the fallen: said every
        //    step, since it is the contract that
        //    knows and a save reads the contract back.
        self.hand_the_room_the_field_medics();
        //    And the tanks' (feature 77): the walls standing among the
        //    crew, before the skills, which read the bulwark off the room.
        self.hand_the_room_the_tanks();
        //    The commanders' list kept as long as the crew.
        self.size_the_commanders();
        //    And every player's own two standing orders (feature 84).
        self.hand_the_room_the_standing();
        self.hand_the_room_the_soldiers();
        //    And a pistol for every empty hand under arms.
        self.arm_the_empty_handed(&mut events);
        //    How many hits each player's Bim had taken before the rooms
        //    stepped, for a relic that fires on one (feature 106).
        let hits_before = self.hits_before_the_step();
        //    And who of the crew was down, for a relic that fires on a
        //    crewmate going down (task 118).
        let downs_before = self.downs_before_the_step();
        // Weak Spot's stream (task 124) lent to the crew's room for its
        // step — the one room the crew's bolts land in — and taken back.
        self.aboard.room.lend_crit_rng(self.crit_rng.clone());
        self.aboard.step();
        if let Some(rng) = self.aboard.room.take_crit_rng() {
            self.crit_rng = rng;
        }
        // An EMP that burst in the crew's room stuns the machines it
        // reached before their own room steps (task 127).
        self.settle_stuns();
        // And a throw a crew member walked out to make, the step it can.
        self.settle_throws(&mut events);
        // A townsperson with a crew member's hands on it stands its
        // countdown, as a crewmate being revived does.
        let tended = self.tended_residents();
        if let Some(residents) = &mut self.residents {
            residents.aboard.room.set_tended(&tended);
            residents.aboard.step();
        }
        self.visit(&mut events);
        self.settle_deployables(&mut events);
        self.settle_bursts(&mut events);
        self.sync_lamps();
        self.casualties(&mut events);
        //    And the relics (feature 106): a player's Bim that went down or
        //    was hit this step, its triggers.
        self.settle_relics(&hits_before, &mut events);
        self.settle_relic_downs(&downs_before, &mut events);
        self.relics_mend();
        self.experience(&mut events);
        self.settle_medics(&mut events);
        self.melee_locks(&mut events);
        //    And the Machine Heart (feature 108): its phase read off the room,
        //    what its fabricators build, and the core down being the run
        //    won — before the loss, so a core that falls the step the last
        //    player does is a win.
        self.heart_step(&mut events);
        //    And the run over with nobody standing.
        self.check_lost(&mut events);
        //    And what the fight did to the armour: a piece broken is said
        //    once, and stays worn (task 113).
        self.say_pieces_broken(&mut events);

        // 6. Power: what the reactors made this step against what the
        //    wired consumers drew, into or out of the batteries. What is
        //    running in a brownout is `World::powered`, read by whatever
        //    draws — the benches — and what the
        //    brownout does to the rest is `run_brownout`: the lamps dark.
        self.run_power();
        self.run_brownout(&mut events);

        // 7. Construction: what the crew did at the sites this step — a
        //    part put together. Nothing is carried to a site since
        //    feature 95: a part is **bought**, and its price leaves the
        //    pool the moment it goes down. See `crate::build` and
        //    `shipdesign::materials`.
        for (site, _who) in self.aboard.room.take_built() {
            self.finish_build(site, &mut events);
        }
        //    And the kits laid, each a deployable put down.
        for (who, at, kind) in self.aboard.room.take_deployed() {
            self.finish_deploy(who, at, kind, &mut events);
        }

        // 8. The run (feature 103): the bounty paid the step the site is
        //    cleared, and the departure check — which, answered, is the
        //    ship leaving and the map coming up. Last, so it sees the
        //    step whole.
        self.settle_run(&mut events);

        events
    }

    /// Everything a command can do. Split out from [`World::step`] so the
    /// order of the step reads as an order rather than as a wall of matches.
    fn apply(&mut self, command: Command, events: &mut Vec<WorldEvent>) {
        let slot = match command {
            Command::SetSpeed { slot, .. }
            | Command::PlaceSite { slot, .. }
            | Command::CancelSite { slot, .. }
            | Command::Equip { slot, .. }
            | Command::Unequip { slot, .. }
            | Command::Offer { slot, .. }
            | Command::AnswerOffer { slot, .. }
            | Command::Hire { slot, .. }
            | Command::Crew { slot, .. }
            | Command::CrewLater { slot, .. }
            | Command::SetClass { slot, .. }
            | Command::RankUp { slot, .. }
            | Command::Rampage { slot }
            | Command::Deploy { slot, .. }
            | Command::Sentry { slot, .. }
            | Command::Emp { slot, .. }
            | Command::PackUp { slot, .. }
            | Command::Brace { slot, .. }
            | Command::Throw { slot, .. }
            | Command::ThrowAt { slot, .. }
            | Command::Beam { slot, .. }
            | Command::NaniteBurst { slot }
            | Command::Cloak { slot, .. }
            | Command::Bulwark { slot, .. }
            | Command::Taunt { slot }
            | Command::Juggernaut { slot }
            | Command::Rally { slot }
            | Command::BattleCry { slot }
            | Command::Carry { slot, .. }
            | Command::Orders { slot, .. }
            | Command::Propose { slot, .. }
            | Command::Accept { slot, .. }
            | Command::Return { slot }
            | Command::LeaveBehind { slot, .. }
            | Command::PlayerGone { slot }
            | Command::Ready { slot, .. }
            | Command::ProposeRelic { slot, .. }
            | Command::AcceptRelic { slot, .. }
            | Command::OpenCache { slot, .. }
            | Command::BuyShelf { slot, .. }
            | Command::Combine { slot, .. }
            | Command::Restock { slot } => slot,
        };

        // Between missions nothing happens but the choosing: the map
        // is up and nothing steps. An order to the crew's room is heard —
        // a selection, a pick in a panel — and moves nobody until the next
        // mission is under way.
        if self.run.phase != run::Phase::Mission
            && !matches!(
                command,
                Command::SetSpeed { .. }
                    | Command::Propose { .. }
                    | Command::Accept { .. }
                    | Command::ProposeRelic { .. }
                    | Command::AcceptRelic { .. }
                    | Command::PlayerGone { .. }
                    | Command::Ready { .. }
                    | Command::Crew { .. }
                    | Command::CrewLater { .. }
                    | Command::Equip { .. }
                    | Command::Unequip { .. }
                    | Command::Offer { .. }
                    | Command::AnswerOffer { .. }
                    | Command::BuyShelf { .. }
                    | Command::Combine { .. }
                    | Command::Restock { .. }
                    | Command::RankUp { .. }
            )
        {
            events.push(refused(slot, Refusal::BetweenMissions));
            return;
        }
        // A mission held for the ready check is the map's again: the
        // loadouts, a selection, a rank and *Ready* are heard, and nothing
        // that would move anybody or start anything.
        if self.run.briefing
            && !matches!(
                command,
                Command::SetSpeed { .. }
                    | Command::Ready { .. }
                    | Command::PlayerGone { .. }
                    | Command::Crew { .. }
                    | Command::CrewLater { .. }
                    | Command::Equip { .. }
                    | Command::Unequip { .. }
                    | Command::Offer { .. }
                    | Command::AnswerOffer { .. }
                    | Command::RankUp { .. }
            )
        {
            events.push(refused(slot, Refusal::AwaitingReady));
            return;
        }
        // A fight won is a frozen deck (task 133): the loadouts, a rank,
        // *Back to ship* and its question are heard, and nothing that
        // would move anybody or start anything.
        if self.fight_over()
            && !matches!(
                command,
                Command::SetSpeed { .. }
                    | Command::Return { .. }
                    | Command::LeaveBehind { .. }
                    | Command::PlayerGone { .. }
                    | Command::Ready { .. }
                    | Command::Equip { .. }
                    | Command::Unequip { .. }
                    | Command::Offer { .. }
                    | Command::AnswerOffer { .. }
                    | Command::RankUp { .. }
            )
        {
            events.push(refused(slot, Refusal::FightOver));
            return;
        }
        let before = events.len();
        // **A cloaked Bim uses no class ability** (task 130): every key a
        // class has is refused while its cloak lasts — a brace or a wall
        // put *down*, a beam let go, a pack-up and a carry are not
        // abilities used, and go through.
        let class_ability = matches!(
            command,
            Command::Deploy { .. }
                | Command::Sentry { .. }
                | Command::Emp { .. }
                | Command::Throw { .. }
                | Command::ThrowAt { .. }
                | Command::Rampage { .. }
                | Command::Brace { on: true, .. }
                | Command::Beam {
                    patient: Some(_),
                    ..
                }
                | Command::NaniteBurst { .. }
                | Command::Cloak { .. }
                | Command::Bulwark { on: true, .. }
                | Command::Taunt { .. }
                | Command::Juggernaut { .. }
                | Command::Rally { .. }
                | Command::BattleCry { .. }
        );
        if class_ability && self.is_cloaked(slot) {
            events.push(refused(slot, Refusal::Cloaked));
            return;
        }
        match command {
            // Speed is not an order to the ship: a player who is nowhere
            // near anything may still say they want to watch this bit
            // slowly.
            Command::SetSpeed { speed, .. } => self.request_speed(slot, speed),
            Command::Propose { star, station, .. } => {
                self.propose(slot, run::Site { star, station }, events)
            }
            Command::Accept { yes, .. } => self.accept_proposal(slot, yes, events),
            Command::Return { .. } => self.press_return(slot, events),
            Command::LeaveBehind { yes, .. } => self.answer_departure(slot, yes, events),
            Command::PlayerGone { .. } => self.player_gone(slot, events),
            Command::Ready { yes, .. } => self.press_ready(slot, yes, events),
            Command::ProposeRelic { relic, to, .. } => {
                self.propose_relic(slot, crate::relic::Relic::from_code(relic), to, events)
            }
            Command::AcceptRelic { yes, .. } => self.accept_relic(slot, yes, events),
            Command::OpenCache { who, .. } => self.open_cache(slot, who, events),
            Command::BuyShelf { index, to, .. } => self.buy_shelf(slot, index, to, events),
            Command::Combine { a, b, .. } => self.combine(slot, a, b, events),
            Command::Restock { .. } => self.restock(slot, events),
            Command::PlaceSite {
                kind,
                origin,
                rotation,
                ..
            } => self.place_site(slot, kind, origin, rotation, events),
            Command::CancelSite { site, .. } => self.cancel_site(slot, site, events),
            Command::Equip { who, from, .. } => self.equip(slot, who, from, events),
            Command::Unequip { who, part, .. } => self.unequip(slot, who, part, events),
            Command::Offer { part, to, .. } => self.offer(slot, part, to, events),
            Command::AnswerOffer {
                from, part, yes, ..
            } => self.answer_offer(slot, from, part, yes, events),
            Command::Hire { who, resident, .. } => self.hire(slot, who, resident, events),
            Command::Crew { order, .. } => {
                // An order that moves the Bim calls off a throw it was
                // walking out to make.
                if calls_off_a_throw(order) {
                    self.throws.retain(|p| p.who != slot);
                }
                // A room built since the last step starts at one player.
                self.aboard.room.set_players(self.players());
                let code = self.aboard.room.order(slot, order);
                // A walk with no way there is the one order that is said:
                // the room's `ORDER_NOWHERE`.
                if let Some(why) = walk_refusal(code) {
                    events.push(refused(slot, why));
                }
            }
            Command::CrewLater { order, .. } => {
                if calls_off_a_throw(order) {
                    self.throws.retain(|p| p.who != slot);
                }
                self.aboard.room.set_players(self.players());
                let code = self.aboard.room.order_later(slot, order);
                if let Some(why) = walk_refusal(code) {
                    events.push(refused(slot, why));
                }
            }
            Command::SetClass { class, .. } => {
                if let Err(why) = self.set_class(slot, class) {
                    events.push(refused(slot, why));
                }
            }
            Command::RankUp { ability_slot, .. } => self.rank_up(slot, ability_slot, events),
            Command::Rampage { .. } => match self.rampage(slot) {
                Ok(()) => events.push(WorldEvent::Rampaged { who: slot }),
                Err(why) => events.push(refused(slot, why)),
            },
            Command::Deploy { kind, x, y, .. } => {
                if let Err(why) = self.deploy(slot, kind, (x, y)) {
                    events.push(refused(slot, why));
                }
            }
            Command::Sentry { tile, .. } => {
                if let Err(why) = self.lay_sentry(slot, tile) {
                    events.push(refused(slot, why));
                }
            }
            Command::Emp { x, y, .. } => match self.throw_emp(slot, (x, y)) {
                Ok(()) => events.push(WorldEvent::EmpThrown { who: slot }),
                Err(why) => events.push(refused(slot, why)),
            },
            Command::PackUp { id, .. } => self.pack_up(slot, id, events),
            Command::Brace { on, .. } => match self.brace(slot, on) {
                Ok(()) => events.push(WorldEvent::Braced { who: slot, on }),
                Err(why) => events.push(refused(slot, why)),
            },
            Command::Throw { x, y, .. } => match self.throw(slot, (x, y)) {
                Ok(()) => events.push(WorldEvent::Thrown { who: slot }),
                Err(why) => events.push(refused(slot, why)),
            },
            Command::ThrowAt { emp, x, y, .. } => {
                if let Err(why) = self.throw_at(slot, emp, (x, y), events) {
                    events.push(refused(slot, why));
                }
            }
            Command::Beam { patient, .. } => match self.beam(slot, patient) {
                Ok(()) => events.push(WorldEvent::Beamed { who: slot, patient }),
                Err(why) => events.push(refused(slot, why)),
            },
            Command::NaniteBurst { .. } => match self.nanite_burst(slot) {
                Ok(healed) => events.push(WorldEvent::NaniteBurst { who: slot, healed }),
                Err(why) => events.push(refused(slot, why)),
            },
            Command::Cloak { target, .. } => match self.cloak(slot, target) {
                Ok(()) => events.push(WorldEvent::Cloaked { who: slot, target }),
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
            Command::Juggernaut { .. } => match self.juggernaut(slot) {
                Ok(()) => events.push(WorldEvent::Juggernaut { who: slot }),
                Err(why) => events.push(refused(slot, why)),
            },
            Command::Rally { .. } => match self.rally(slot) {
                Ok(()) => events.push(WorldEvent::Rallied { who: slot }),
                Err(why) => events.push(refused(slot, why)),
            },
            Command::BattleCry { .. } => match self.battle_cry(slot) {
                Ok(()) => events.push(WorldEvent::BattleCried { who: slot }),
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
        // A class key gone through is an ability used (feature 106): the
        // relics that wait on one are told.
        let ability = matches!(
            command,
            Command::Deploy { .. }
                | Command::Sentry { .. }
                | Command::Emp { .. }
                | Command::Throw { .. }
                | Command::Rampage { .. }
                | Command::NaniteBurst { .. }
                | Command::Cloak { .. }
                | Command::Taunt { .. }
                | Command::Juggernaut { .. }
                | Command::Rally { .. }
                | Command::BattleCry { .. }
        );
        let turned_down = events[before..]
            .iter()
            .any(|e| matches!(e, WorldEvent::Refused { .. }));
        if ability && !turned_down {
            self.relic_trigger(slot, crate::relic::Trigger::AbilityUse, events);
        }
    }

    // --- the jump --------------------------------------------------------------

    /// The galaxy this world is a system of, generated afresh: the stars
    /// and, per star, the system — what the galaxy chart in the map shows.
    /// Cheap for the stars; a system is generated when asked for.
    pub fn galaxy(&self) -> Galaxy {
        Galaxy::new(self.galaxy_seed, self.galaxy_type)
    }

    /// Put the ship in another system: what a trip across a hyperlane
    /// does on the way (`World::travel`, feature 103). **The one place a
    /// system is replaced**, so this is the list of what belongs to one:
    /// the chart, the stations and their people, the keys on the
    /// stations' desks, the local frame. The ship, the crew,
    /// the hold, the sites on the deck, the research, the money and the
    /// hired hands come along. It lands holding, in empty space
    /// (`jump::landing_point`), pointing the way it was, with only what its
    /// own sensors reach on the chart. False, and nothing moved, for a
    /// star the galaxy has not got.
    pub(crate) fn jump(&mut self, star: u32, events: &mut Vec<WorldEvent>) -> bool {
        let Some(system) = self.galaxy().system(star) else {
            return false;
        };
        // The system left behind, as it was left — its station's room
        // closed first, so its dead are counted — filed under its star.
        self.close_residents();
        self.remember_system();
        self.star_id = star;
        self.system = system;
        self.stations = Station::all_of(&self.system);
        self.surfaces = Surface::all_of(&self.system, self.galaxy_seed);
        // The system arrived at: as the crew left it, if they have been
        // here, else as the generator rolled it.
        if !self.recall_system(star) {
            self.lamps.retain(|d| d.station.is_none());
            self.discovered.clear();
            self.losses.clear();
            self.graves.clear();
            self.visited.clear();
            // And the machines' hold on the *last* system's stations,
            // which names ids this one has of its own: the crisis lays
            // its own the first step after the arrival.
            self.infested.clear();
            // And the towns the crew defended and held there, for the
            // same reason: a town id is its system's own.
            self.defenses.clear();
            self.held_towns.clear();
        }
        // The machines' jammer goes in **before** the landing point is
        // picked (feature 93), so the two are never the same spot and the
        // ship does not arrive inside the station it came to destroy.
        self.settle_jammer();
        let at = crate::jump::landing_point(&self.system);
        self.ship.state = ShipState::Holding;
        self.ship.set_position(at);
        self.ship.frame = Frame::Space;
        events.push(WorldEvent::Jumped { star });
        true
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
    }

    /// The settlement whose station id that is, if it is one's.
    fn surface_of(&self, id: u32) -> Option<&Surface> {
        surface::surface_body(id).and_then(|body| self.surface(body))
    }

    /// The settlement on `body`, if the body has ground to stand one on.
    pub fn surface(&self, body: u32) -> Option<&Surface> {
        self.surfaces.iter().find(|s| s.body == body)
    }

    /// The planet the ship is on the ground of — tied up at its
    /// settlement — if it is on one.
    pub fn landed(&self) -> Option<u32> {
        self.ship.state.alongside().and_then(surface::surface_body)
    }

    /// The end: nobody of the crew standing — alive and awake — and the
    /// run is over, said once and kept.
    fn check_lost(&mut self, events: &mut Vec<WorldEvent>) {
        // Since the run (feature 103) the run is lost when every player's
        // Bim is dead at once — not merely down, and whatever the bots are
        // doing: see `mission.rs`.
        self.check_run_lost(events);
    }

    /// The crew's **whole net worth** now, in whole euros (feature 95):
    ///
    /// - every part of the ship at its price;
    /// - what the ship carries that is not gear at the **book value**
    ///   (`economy::trade_price`, the same everywhere; a valuation, not
    ///   what any desk would pay);
    /// - every gun and every piece of armour the crew own — in the armory
    ///   or on a Bim's loadout (task 113) — at the same book and its
    ///   **tier** (`economy::TIER_PRICE`), and the research keys;
    /// - and the **money in hand**.
    ///
    /// Nothing a crew own changes what they are worth by moving from one
    /// pocket to another: a purchase, a sale and a piece put on are all
    /// worth the spread and nothing else. A charge — a kit, a grenade — is
    /// not property: it came back by itself and will again, so a crew that
    /// has spent its grenades is no poorer.
    ///
    /// Saturating throughout: a world worth more than a `u64` is a bug
    /// upstream, and a wrap would hand an enemy a crew worth nothing.
    pub fn worth(&self) -> Money {
        let design = &self.ship.design;
        let mut sum: Money = design
            .parts
            .iter()
            .fold(0, |sum, p| sum.saturating_add(p.kind.def().price));
        // What the ship carries, bar the gear, which is never a count
        // (task 113).
        for &id in ResourceId::ALL.iter() {
            if economy::tiered(id) {
                continue;
            }
            let units = design.carrying(id) as Money;
            sum = sum.saturating_add(trade_price(id).saturating_mul(units));
        }
        // The armory, then every loadout.
        let value = |item: Item| match item {
            Item::Weapon(w) => gear_value(armour::weapon_resource(w.kind), w.tier.code()),
            Item::Armour(p) => gear_value(armour::resource_of(p.kind), p.tier.code()),
            Item::Stack(_) => 0,
        };
        for stored in &self.holdings.armory {
            sum = sum.saturating_add(value(stored.item));
        }
        let room = &self.aboard.room;
        for who in 0..room.crew_count() as usize {
            // A reinforcement's rifle is the Republic's, not the crew's
            // (task 129).
            if self.is_reinforcement(who as u32) {
                continue;
            }
            let gear = room.gear(who);
            for slot in GearSlot::ALL {
                if let Some(item) = slot.read(&gear) {
                    sum = sum.saturating_add(value(item));
                }
            }
        }
        // And the keys, at the tier-one key's book.
        sum = sum.saturating_add(
            trade_price(ResourceId::ResearchKey).saturating_mul(self.holdings.keys as Money),
        );
        self.wallets
            .iter()
            .fold(sum.saturating_add(self.money), |sum, &w| {
                sum.saturating_add(w)
            })
    }

    /// How many whole days the game has run: `clock_minutes` — elapsed
    /// time since the world opened, not the crew's calendar
    /// ([`World::day`]) — over a day, floored, and read in whole minutes
    /// first, so a server catching up counts the same day. The crisis
    /// spreads by it.
    pub fn days_gone(&self) -> u32 {
        let minutes = self.clock_minutes.floor() as u64;
        (minutes / (time::DAY as u64)) as u32
    }

    /// How many whole hours the game has run, the same way as
    /// [`World::days_gone`]: the world clock, floored, read in whole
    /// minutes. The machines grow by one every [`data::ENEMIES_HOURS`] of
    /// it (`crate::droid::time_steps`, feature 105).
    pub fn hours_gone(&self) -> u32 {
        let minutes = self.clock_minutes.floor() as u64;
        (minutes / (time::HOUR as u64)).min(u32::MAX as u64) as u32
    }

    /// How many people a station's room is opened with: the people who
    /// live there — every human is friendly (feature 104), so there is no
    /// garrison to arm — **less its dead** ([`World::losses`]): the people
    /// who died there stay dead, so a station opens with its survivors
    /// and one emptied opens empty. Nobody on a derelict, and nobody at a
    /// station the machines hold. Asked when the room opens, so a station
    /// keeps the crowd it was reached with until the ship has gone and
    /// come back.
    pub fn people_of(&self, station: &Station) -> u32 {
        // A station the machines hold has no people at all (feature 83):
        // whoever lived there is gone, and what the room is opened with
        // is a wave of droids (`World::settle_droids`).
        if self.is_droid_held(station.id) {
            return 0;
        }
        station
            .residents()
            .saturating_sub(self.losses_at(station.id).dead)
    }

    /// How many mercenaries for hire live at a station, on top of
    /// [`World::people_of`]: none where the machines hold it or on a
    /// derelict, else
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
        if station.residents() == 0 {
            return 0;
        }
        // And one more inside the front (feature 94), which is what a
        // system with the machines a few hops off looks like from the
        // hiring hall: people on the move, and armed.
        let near_front = self.front_at(station.id).is_some();
        mercenary::how_many(self.worth(), self.start_worth, station.map_seed, near_front)
            .max(self.least_mercenaries)
            .saturating_sub(self.losses_at(station.id).mercenaries)
    }

    /// Where this ship docks at that station: airlock to airlock, outside
    /// its hull. `None` for a station that is not there or has no door.
    pub fn berth_at(&self, id: u32) -> Option<Berth> {
        self.station(id)?
            .berth(&self.ship.design, self.ship.dynamics.centre_of_mass)
    }

    /// Put the ship at its berth: the one place, with [`World::start`], that
    /// it is set down — nothing is flown. Heading first, because the
    /// position is worked out through it.
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
        let ship_seed = self.galaxy_seed ^ self.steps;
        // The crew out of the old room, and then what leaving banked in
        // it — a sheaf in somebody's hands goes into the store as the
        // errand is given up — into the hold before the room is dropped.
        let mut old = std::mem::replace(&mut self.aboard, Aboard::new(&self.ship.design, 1, 0));
        let crew = old.room.take_crew();
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
        }
        self.residents = Some(residents);
        self.apply_stances();
        self.restore_lamps();
        // And the laid sandbags onto both fresh rooms (feature 74).
        self.sync_deployed_cover();
    }

    /// Whose a station is, to the crew: a station the machines hold is
    /// hostile, home is friendly, and everywhere else is neutral. **No
    /// human is ever the crew's enemy** (feature 104): whatever the
    /// generator rolled a station's people (`Station::hostile`), they are
    /// a friend's at home and a stranger's anywhere else.
    pub fn stance(&self, station: u32) -> Stance {
        // A station the machines hold is an enemy's whatever it was
        // before (feature 83): its room is hostile, which is the switch
        // that puts its bodies at war.
        if self.is_droid_held(station) {
            Stance::Hostile
        } else if station == self.home && self.star_id == self.home_star {
            Stance::Friendly
        } else {
            Stance::Neutral
        }
    }

    // --- what a run has switched off (feature 102) ------------------------

    /// Whether the crew build onto their ship and buy what it lives on:
    /// see the field.
    pub fn shipyard_enabled(&self) -> bool {
        self.shipyard_enabled
    }

    /// Switch the shipyard on or off — the tests of building and of the
    /// shelf switch it on.
    pub fn set_shipyard_enabled(&mut self, on: bool) {
        self.shipyard_enabled = on;
    }

    /// The residents' room opened again with the crowd the station now
    /// calls for, if it is open on that station and the crowd has
    /// changed: a station taken by the machines has no people at all
    /// (feature 83). Nothing when the count is what it was. Docked there, the fresh room is looked into the way
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
                // And the defenders (task 111), who are Bims of the room
                // as much as the people are.
                let defenders = self.defenders_of(station);
                (count + mercs + defenders != r.aboard.room.crew_count())
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
        // The machines destroyed this step, for the Republic's bounty
        // (feature 103): said when the room is done with below.
        // Each with who hit it last, for a relic that pays or pays back
        // for its own kills (feature 106).
        let mut machine_kills: Vec<relic_hooks::MachineKill> = Vec::new();
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
                    // Nor is a Manufacturer (feature 109): nothing of theirs
                    // is ever taken.
                    let machine = who >= residents.aboard.room.crew_count()
                        || residents.aboard.room.is_manufacturer(who as usize);
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
        // And which of them the crew may pick up with the medkit: a
        // station's own person downed — a townsperson fallen defending
        // it — at a station that is not at war with the crew.
        let revivable = self.revivable_residents();
        self.aboard.room.set_visitors_revivable(&revivable);
        // And which of them the crew can see, for their own room to draw.
        // Off the last trace, which is a frame's rather than a step's —
        // nobody crosses a bulkhead in a sixtieth of a minute.
        let seen = self.aboard.seen(&visitors);
        if let Some(residents) = &mut self.residents {
            residents.aboard.room.set_seen(&seen);
        }
        // The fight, which crosses between the two rooms both ways. A
        // hostile station's bodies — the machines, every human being
        // friendly (feature 104) — are the crew's targets, at those same
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
        // **And the one fight that is not one room against another**
        // (feature 94): a town the crew are defending, where the
        // machines stand in the residents' room with the town's own
        // people. The station stays friendly throughout — its people are
        // never the crew's targets and never shoot at them — so what is
        // exchanged across the seam is the machines alone, and what
        // happens between the machines and the townsfolk happens inside
        // that one room and never crosses.
        let defending = self.defense_here().is_some()
            && self
                .residents
                .as_ref()
                .is_some_and(|r| Some(r.station) == self.ship.state.station());
        let hits = self.aboard.room.take_hits();
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
        // And what each of the crew forces on the enemy (features 77 and
        // 139): a tank's Taunt or his Juggernaut. Worked out here for the
        // same reason as the sentries: the ranks are the world's. This is
        // the room the enemies aim and charge in, whoever they are — and
        // in a town the crew defend, the machines' own list starts with
        // the crew, so it is said to that list too.
        let taunts = self.taunts_for_the_enemy();
        // And who of the crew no enemy aims at — a relic's *Signal
        // Scrambler* (task 118), a medic's cloak (task 130) — for the
        // same reason.
        let unseen = self.hidden_from_enemies();
        let withheld = unseen.iter().any(|&h| h);
        // The crew's hits on the machines, landed through the relics
        // (task 118) — with none held, the same strike the loop below made
        // — and the hits on the residents' Bims handed back to it.
        let hits = if hostile || defending {
            self.land_on_machines(hits)
        } else {
            hits
        };
        // And Weak Spot's crits on what is left, the hits on the
        // residents' Bims (task 124): after every factor, before the armour.
        let hits: Vec<bims::combat::Hit> = hits
            .into_iter()
            .map(|mut hit| {
                hit.damage += self.crit_extra(&hit);
                hit
            })
            .collect();
        let Some(residents) = self.residents.as_mut().filter(|_| hostile || defending) else {
            self.aboard.room.set_hostiles(Vec::new());
            if let Some(residents) = &mut self.residents {
                residents.aboard.room.set_hostiles(Vec::new());
                residents.aboard.room.clear_machine_hostiles();
                residents.aboard.room.set_sheltering(&[]);
                residents.aboard.room.set_targets_withheld(false);
            }
            return;
        };
        let shift = residents.aboard.offset;
        let room = &mut residents.aboard.room;
        let bims = room.crew_count() as usize;
        // What the relics' hits on the machines did, said for the numbers
        // over them (`land_on_machines`), and every hit below the same way.
        for (who, damage, crit) in std::mem::take(&mut self.shown_hits) {
            events.push(shown_hit(true, who, damage, crit));
        }
        for hit in hits {
            let who = hit.who;
            if who >= room.body_count() as usize || !room.is_alive(who) {
                continue;
            }
            events.push(shown_hit(true, who as u32, hit.damage, hit.crit));
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
        // Who is down, asked of the room rather than read off the hits: a
        // downed body dies at the top of its tick when its countdown runs
        // out, without a hit landing at all. Said once each.
        // A machine destroyed is one of these too: the lists are sized
        // by the body count, so a wave landing grows them (feature 83).
        let bodies = room.body_count() as usize;
        residents.down.resize(bodies, false);
        residents.xp_down.resize(bodies, false);
        residents.last_hit_by.resize(bodies, None);
        residents.fee.resize(bodies, None);
        residents.medic.resize(bodies, false);
        residents.grave.resize(bodies, false);
        residents.defender.resize(bodies, false);
        let room = &mut residents.aboard.room;
        for who in 0..residents.down.len().min(bodies) {
            let down = !room.is_alive(who);
            if down && !residents.down[who] {
                residents.down[who] = true;
                // The Republic pays for a machine destroyed as it pays
                // for an enemy taken down, by its tier (feature 103):
                // every enemy is a machine now, and a fight is how a crew
                // earns. Pending until the site is cleared.
                if let Some(d) = who.checked_sub(bims).and_then(|i| room.droid(i)) {
                    let by = residents.last_hit_by.get(who).copied().flatten();
                    // How it went, for a relic that pays for a crippled
                    // machine or fires on one taken from behind (task 118).
                    let body = &d.body;
                    let crippled = !body.is_solid()
                        && (body.gone(bims::droid::DroidPart::Arms)
                            || body.gone(bims::droid::DroidPart::Legs));
                    let flanked = {
                        let relics = &mut self.run.relics;
                        let was = relics.flanked.contains(&(who as u32));
                        relics.flanked.retain(|&b| b != who as u32);
                        relics.limb_aimed.retain(|&(_, b)| b != who as u32);
                        relics.spotted.retain(|&(_, b, _)| b != who as u32);
                        was
                    };
                    machine_kills.push(relic_hooks::MachineKill {
                        by,
                        bounty: bounty_share(
                            self.rewards.bounty_for(d.tier.code()),
                            droid_bounty_percent(d.kind),
                        ),
                        crippled,
                        flanked,
                    });
                }
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
        // A crew member under *Signal Scrambler*'s cloak (task 118) or a
        // medic's (task 130) is on nobody's list: no enemy aims at it while
        // it lasts, and one that was drops it this step. And an enemy left
        // with nobody it may pick holds where it stands.
        for (who, &hidden) in unseen.iter().enumerate() {
            if hidden && let Some(c) = crew.get_mut(who) {
                *c = None;
            }
        }
        room.set_targets_withheld(withheld);
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
        if defending {
            // **The town's own fight** (feature 94). Its people shoot
            // the machines *in their own room* and nothing else: the
            // crew are friends and are not on their list at all, which
            // is what "the townsfolk's hits reach droids" means — a
            // bolt of theirs flies here and lands here.
            //
            // **By body index** since task 131: a wave before day ten is
            // the Manufacturers' people as well as their Troopers, and
            // their people are Bims of this room. So the list is the
            // room's whole index space — every Bim `None` but a
            // Manufacturer on its feet, then the machines — and a hit
            // comes back as a body index (below).
            let machines = room.droid_count() as usize;
            let intruder_up = |who: usize| {
                room.is_manufacturer(who) && room.is_alive(who) && !room.is_downed(who)
            };
            let at_enemies: Vec<Option<(bims::math::Vec2, Weapon)>> = (0..bims)
                .map(|who| {
                    intruder_up(who).then(|| {
                        (
                            room.exposed_at(who),
                            room.weapon(who).unwrap_or(WeaponKind::LaserPistol.basic()),
                        )
                    })
                })
                .chain((0..machines).map(|i| {
                    room.droid(i)
                        .filter(|d| !d.destroyed)
                        .map(|d| (d.pos, d.weapon))
                }))
                .collect();
            let enemy_peek: Vec<bool> = (0..bims)
                .map(|who| intruder_up(who) && room.peek(who).is_some())
                .chain((0..machines).map(|i| room.droid(i).is_some_and(|d| d.peek.is_some())))
                .collect();
            let enemy_dodge: Vec<f32> = (0..bims)
                .map(|who| {
                    if intruder_up(who) {
                        room.dodge(who)
                    } else {
                        0.0
                    }
                })
                .chain((0..machines).map(|_| 0.0))
                .collect();
            // And a Guardian's shield faces the town's people as it faces
            // the crew (feature 100): their bolts fly here, so it is
            // decided here, in the room's own frame.
            let enemy_shields: Vec<Option<bims::math::Vec2>> = (0..bims)
                .map(|_| None)
                .chain((0..machines).map(|i| room.droid(i).and_then(|d| d.shield())))
                .collect();
            room.set_hostiles(at_enemies);
            room.set_hostiles_peeking(&enemy_peek);
            room.set_hostiles_dodge(&enemy_dodge);
            room.set_hostiles_shields(&enemy_shields);
            // And **the machines' own list**: the crew across the seam
            // first — shot at with a recorded `Shot` the world flies on
            // the joined deck — then the town's people, shot at with a
            // bolt that flies in this room and lands on the body.
            let mut theirs = crew.clone();
            let cross = theirs.len();
            for who in 0..bims {
                // Never one of their own people (task 131), who came with
                // the wave.
                let theirs_too = room.is_manufacturer(who);
                theirs.push(
                    (!theirs_too && room.is_alive(who) && !room.is_downed(who)).then(|| {
                        (
                            room.exposed_at(who),
                            room.weapon(who).unwrap_or(WeaponKind::LaserPistol.basic()),
                        )
                    }),
                );
            }
            room.set_machine_hostiles(theirs, cross);
            room.set_machine_hostiles_taunting(&taunts);
            // And who takes arms: **a town's guard, any mercenaries and
            // the defenders** (task 111). Everybody else walks into the
            // nearest house — the nearest bunk, at a station — and stays
            // there until the attack is over.
            let town = surface::surface_body(residents.station).is_some();
            let sheltering: Vec<bool> = (0..bims)
                .map(|who| {
                    !(town && who == surface::GUARD as usize)
                        && !residents.is_mercenary(who)
                        && !residents.is_defender(who)
                        // A Manufacturer come to attack (task 131) runs
                        // into no house.
                        && !residents.aboard.room.is_manufacturer(who)
                })
                .collect();
            let room = &mut residents.aboard.room;
            room.set_sheltering(&sheltering);
        } else {
            // A reinforcement of the Manufacturers' standing here was told
            // where the crew are, as the machines' is (`Droid::seeking`),
            // and comes looking for them rather than waiting at its
            // airlock: their garrison is wave one, and a wave of theirs
            // lands only once the one before it is down.
            room.set_told(residents.manufacturers_laid > 1);
            room.set_hostiles(crew.clone());
            room.set_hostiles_peeking(&self.aboard.crew_peeking());
            room.set_hostiles_taunting(&taunts);
            // And the odds each dodges a bolt for its armour, the same way.
            let crew_dodge: Vec<f32> = (0..self.aboard.crew_count())
                .map(|who| self.aboard.room.dodge(who as usize))
                .collect();
            room.set_hostiles_dodge(&crew_dodge);
            room.clear_machine_hostiles();
            room.set_sheltering(&[]);
        }
        let room = &mut residents.aboard.room;
        // **What the town's people did to the machines stays in the
        // room** (feature 94): their bolts and their blows are ordinary
        // friendly `Hit`s on this room's own target list, which is the
        // machines by droid index, so they are delivered to the machine
        // they were aimed at without crossing anywhere.
        // Since task 131 the list is by body index, so a hit is a machine's
        // past the Bims and a Manufacturer's among them.
        let own = room.take_hits();
        let bims = room.crew_count() as usize;
        for hit in own {
            match hit.who.checked_sub(bims) {
                Some(i) => {
                    room.strike_droid(i, bims::droid::DroidPart::hit_by(hit.roll), hit.damage);
                    events.push(shown_hit(true, hit.who as u32, hit.damage, hit.crit));
                }
                None if room.is_manufacturer(hit.who) => {
                    events.push(shown_hit(true, hit.who as u32, hit.damage, hit.crit));
                    if hit.blast {
                        room.blast(hit.who, hit.part, hit.damage);
                    } else {
                        room.strike(hit.who, hit.part, hit.damage, hit.cut);
                    }
                }
                None => {}
            }
        }
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
                // A Guardian's Sweeper (feature 100): the beam laid over
                // the same arc on the joined deck, its two aim points put
                // through the frame like any shot's, so a turned or
                // mirrored station carries the sweep the right way round.
                if let Some(end) = shot.sweep {
                    self.aboard.room.enemy_sweep(
                        on_deck(shot.from),
                        on_deck(shot.at),
                        on_deck(end),
                        shot.weapon,
                        shot.damage,
                        shot.pace,
                    );
                    continue;
                }
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
        // feet — one downed is nobody's target — each with its weapon, at
        // the peek while peeking, and which are peeking. **Every body of
        // that room**: its Bims and then its machines (feature 83), one
        // index space, which is what a hit past the Bims is read back
        // against above.
        let bodies = room.body_count();
        let town_bims = room.crew_count();
        let alive: Vec<bool> = (0..bodies)
            .map(|who| {
                // **The crew's targets are the machines only** while a
                // town is being defended (feature 94): its people are
                // friends, so they are handed over as nobody's target
                // and the crew never aim at one. The index space is
                // still the whole room's, which is what a hit read back
                // past the Bims relies on. The Manufacturers come with a
                // wave before day ten (task 131) are the crew's enemies
                // among them.
                if defending && who < town_bims && !room.is_manufacturer(who as usize) {
                    return false;
                }
                room.is_alive(who as usize) && !room.is_downed(who as usize)
            })
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
        // And which way each Guardian's shield faces (feature 100), in the
        // residents' room's frame; turned onto the joined deck below.
        let shields_there: Vec<Option<bims::math::Vec2>> = (0..bodies)
            .map(|who| room.shield_of(who as usize))
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
        // And which way each Guardian's shield faces (feature 100), turned
        // from the residents' room onto the joined deck: the frame's two
        // unit axes, a direction taking no origin and no shift.
        let shields: Vec<Option<bims::math::Vec2>> = shields_there
            .into_iter()
            .map(|v| {
                let v = v?;
                let (_, ex, ey) = self.aboard.station_frame?;
                let d = ex.scale(v.x as f64).add(ey.scale(v.y as f64));
                Some(bims::math::vec2(d.x as f32, d.y as f32))
            })
            .collect();
        self.aboard.room.set_hostiles(targets);
        self.aboard.room.set_hostiles_peeking(&peeking);
        self.aboard.room.set_hostiles_dodge(&dodge);
        self.aboard.room.set_hostiles_shields(&shields);
        // And what the Republic owes for the machines destroyed this step.
        // A machine's last hit by a player's Bim with a relic that
        // reads kills (feature 106): *Salvage Beacon*'s share on the
        // bounty, *Kill Relay*'s seconds off the cooldowns.
        // Every machine destroyed, for the run's summary (feature 108).
        self.run.machines_destroyed = self
            .run
            .machines_destroyed
            .saturating_add(machine_kills.len() as u32);
        let machine_bounty = self.machine_kills_noted(&machine_kills, events);
        self.earn_bounty(machine_bounty, events);
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
    /// wounds its own the step a bolt lands — whoever a hit downed, and
    /// whoever died. Death is said once, the step it happens, whatever did
    /// it: a downed body's countdown run out, most often.
    fn casualties(&mut self, events: &mut Vec<WorldEvent>) {
        for hit in self.aboard.room.take_wounds_taken() {
            if hit.who < self.aboard.crew_count() as usize {
                events.push(WorldEvent::CrewHit {
                    who: hit.who as u32,
                    part: hit.part.code(),
                });
                events.push(shown_hit(false, hit.who as u32, hit.damage, hit.crit));
            }
        }
        for who in self.aboard.room.take_downs() {
            if who < self.aboard.crew_count() as usize {
                events.push(WorldEvent::CrewDowned { who: who as u32 });
            }
        }
        for who in 0..self.crew_down.len().min(self.aboard.crew_count() as usize) {
            let down = !self.aboard.room.is_alive(who);
            if down && !self.crew_down[who] {
                self.crew_down[who] = true;
                events.push(WorldEvent::CrewDown { who: who as u32 });
                // Since the run (feature 103) a dead player's level,
                // experience and talents are **kept** for its buyback; a
                // bot is gone for good and costs the pool (`fall`). A
                // medic's beam and timers go with the body (feature 76),
                // and so does a cloak on anybody (task 130).
                self.fall(who as u32, events);
                if let Some(medic) = self.medics.get_mut(who) {
                    *medic = Medic::default();
                }
                if let Some(cloak) = self.cloaks.get_mut(who) {
                    *cloak = crate::medic::Cloak::default();
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
        // A reinforcement dead is gone from the deck at once (task 129).
        self.settle_reinforcements();
    }

    /// Off the berth: the ship's room is the ship's alone. The residents
    /// were in their own room throughout and go on in it, drawing their
    /// own doors again, until the room is closed.
    fn unjoin_rooms(&mut self) {
        if !self.aboard.is_joined() {
            return;
        }
        let seed = self.galaxy_seed ^ self.steps;
        // As at the join: the crew out first, then what that banked.
        let mut old = std::mem::replace(&mut self.aboard, Aboard::new(&self.ship.design, 1, 0));
        let crew = old.room.take_crew();
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
            // at its bunk, since the ship has left without it.
            if let Some(station) = &station {
                residents.unjoin(station, minutes);
            }
        }
        self.restore_lamps();
        // A station's deck is left behind with its deployables (feature
        // 74); the ship's keep theirs, and both rooms are told the cover
        // again on the next step, since a fresh `Sight` has none.
        self.drop_station_deployables();
        self.sync_deployed_cover();
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
            // Nor is a Manufacturer (feature 109): they are not the
            // station's people, and their dead are nobody's grave.
            if residents.aboard.room.is_alive(who) || residents.aboard.room.is_manufacturer(who) {
                continue;
            }
            // Not `is_alive` is dead, not downed — one downed wakes,
            // and is one of the survivors rather than a grave.
            let was_hired = matches!(residents.fee.get(who), Some(Some(_)));
            // A body that was already lying here when the room opened is
            // in `losses` from the day it died — and a defender is nobody's
            // loss at all (task 111): it came for the fight, and the
            // station's own people are as many as they were. Its body
            // stays where it fell, in the station's coverall.
            if !residents.grave.get(who).copied().unwrap_or(false) && !residents.is_defender(who) {
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
        memory::amend_losses(&mut self.losses, residents.station, |l| {
            l.dead += own;
            l.mercenaries += hired;
        });
        memory::set_graves(&mut self.graves, residents.station, graves);
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
    /// its hired hands, its defenders while the machines are coming for
    /// it (task 111, [`World::defenders_of`]) and its dead. The one door,
    /// so that no open anywhere forgets the graves or the defenders.
    fn open_residents(
        &self,
        station: u32,
        design: &ShipDesign,
        count: u32,
        mercenaries: u32,
        seed: u64,
    ) -> Residents {
        let mut residents = Residents::open(
            station,
            design,
            count,
            mercenaries,
            self.defenders_of(station),
            seed,
            self.clock_minutes,
            self.graves_at(station),
        );
        // And their peacetime rounds (feature 102), dealt the moment the
        // site's people are: walked only with the needs off, but dealt
        // either way so a switch thrown later finds them. Not where the
        // stance is hostile — only a station the machines hold is, and
        // it has no people to deal them to.
        if self.stance(station) != Stance::Hostile
            && let Some(site) = self.station(station)
        {
            residents.deal_roles(site);
        }
        residents
    }

    /// This system as the crew leave it, filed by its star on
    /// [`World::memories`] — everything of the world that is the system's
    /// rather than the ship's — for [`World::recall_system`] to put back.
    /// Called by a jump out; the fields themselves are left as they are
    /// for the jump to replace.
    fn remember_system(&mut self) {
        let memory = SystemMemory {
            star: self.star_id,
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
            infested: self.infested.clone(),
            defenses: self.defenses.clone(),
            held_towns: self.held_towns.clone(),
        };
        memory::file_memory(&mut self.memories, memory);
    }

    /// The system at `star` as the crew left it, put back — `false`, and
    /// nothing touched, if they have never been. The ship's own lamps
    /// stay; the station's remembered ones join them.
    fn recall_system(&mut self, star: u32) -> bool {
        let Some(mut memory) = memory::memory_of(&self.memories, star).cloned() else {
            return false;
        };
        // A system the crisis has taken since the crew were last in it
        // (feature 92) gives back no people and no losses:
        // whoever the crew met there is gone, and the flip that happens
        // the moment they arrive is what stands in the place of it. What
        // is kept is the chart, the rocks, the keys — and the
        // infestations, since a station cleared stays cleared.
        if self.infested(star) {
            memory.overrun();
        }
        self.lamps.retain(|d| d.station.is_none());
        self.lamps.extend(memory.lamps);
        self.discovered = memory.discovered;
        self.losses = memory.losses;
        self.graves = memory.graves;
        self.visited = memory.visited;
        self.infested = memory.infested;
        self.defenses = memory.defenses;
        self.held_towns = memory.held_towns;
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
        // one `join_rooms` left for its pictures. Nothing to settle.
        if matches!(self.ship.state, ShipState::Docked { .. }) {
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

    // --- the ship changing --------------------------------------------------

    /// Everything derived from the design, redone.
    ///
    /// **The one door.** Trading goes through it, a recipe goes through it,
    /// and construction goes through it. What it promises is that the
    /// anchor and the heading do not move: the hull stays exactly where it
    /// was in the system and on the screen, and it is the *centre of mass*
    /// — the ship's position — that shifts when weight is added to one end.
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
    }

    /// The hold emptied, for a run that sets out with nothing aboard
    /// (feature 110): every count of the design's cargo at nought, the
    /// armory and the keys with it — what the ship holds is the pool and
    /// nothing else. The crew keep their loadouts and their charges.
    /// Taken at the start, so `start_worth` is taken again after it: the
    /// crew set out worth the ship, their loadouts and the pool.
    pub fn set_out_empty(&mut self) {
        self.ship.design.cargo = [0; CARGO_SLOTS];
        self.holdings.armory.clear();
        self.holdings.keys = 0;
        self.on_ship_changed();
        self.start_worth = self.worth();
    }

    /// Stage 6 of [`World::step`]: the reactors' output less the wired
    /// consumers' draw, over one step, into the batteries and clamped to
    /// what they hold. Closed form off the step length, so a browser at 24x
    /// and a server catching up land on the same charge.
    ///
    /// The draw is charged in full whether or not the ship is browned
    /// out: what stops in a brownout is the consumers, and what they would
    /// have drawn was never there to take. The clamp at nought *is* the
    /// brownout. The engines draw nothing: nothing is flown.
    fn run_power(&mut self) {
        let budget = self.power_budget;
        let net = (budget.supply - budget.draw) * data::STEP_MINUTES;
        self.ship.charge = (self.ship.charge + net).clamp(0.0, budget.storage);
    }

    /// The other half of stage 6: what the brownout does, none of it
    /// lethal and all of it recoverable. Said once each way — `Brownout`
    /// the step the batteries go flat under an overdraw, `PowerRestored`
    /// the step the reactors cover the draw again or a battery has
    /// something in it — and put to the room: the ship's lamps dark
    /// ([`World::sync_lamp_power`]) while it lasts.
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

    // --- building -------------------------------------------------------------

    /// Whether anything is being built: a Bim on the way to a site, or at
    /// one.
    pub fn under_construction(&self) -> bool {
        self.aboard.room.building_under_way()
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
    /// out: the shipyard open, the part going where
    /// the rules say it may on the ship as it will be once the pending
    /// sites are built, and the ship as it would then be raising no error
    /// the ship does not raise already — a wall across the spot the hob is
    /// worked from is a crew that starve in front of it, and the design
    /// phase would have refused it too. `Err` is why: a refusal, or the
    /// code of the first new fault as [`crate::event::Refusal::WontFit`]
    /// with `issue` set.
    pub fn can_place_site(
        &self,
        kind: PartKind,
        origin: (u32, u32),
        rotation: Rotation,
    ) -> Result<(), SiteRefusal> {
        // Nothing is built onto the ship in a run (feature 102): it is the
        // default ship and stays it. The class deployables are not parts
        // and do not come through here.
        if !self.shipyard_enabled {
            return Err(SiteRefusal::NoShipyard);
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
            Err(SiteRefusal::NoShipyard) => {
                events.push(refused(slot, Refusal::NoShipyard));
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

    /// What the money, less every site already begun, still covers: what
    /// `affordable_site` measures a site's price against.
    ///
    /// "Begun" is a site a Bim is on its way to or standing at
    /// (`Game::building_at`): the crew work the sites in order, and a
    /// price is only spoken for once somebody is walking to it. A site
    /// nobody has walked to costs nothing to lay out and nothing to give
    /// up.
    pub fn free_money(&self) -> Money {
        let design = &self.ship.design;
        let spoken_for: Money = self
            .builds
            .iter()
            .filter(|s| self.aboard.room.building_at(s.id))
            .fold(0, |sum, s| sum.saturating_add(s.price(design)));
        self.crew_money().saturating_sub(spoken_for)
    }

    /// What the crew hold between them: the takings not yet shared out
    /// and every player's wallet. What a build site — the crew's, not
    /// any one player's — is paid out of ([`World::spend_shared`]).
    pub fn crew_money(&self) -> Money {
        self.wallets
            .iter()
            .fold(self.money, |sum, &w| sum.saturating_add(w))
    }

    /// `price` out of what the crew hold between them: the takings first,
    /// then the wallets in slot order. The caller has asked
    /// [`World::crew_money`] first.
    pub(crate) fn spend_shared(&mut self, price: Money) {
        let mut left = price;
        let from_takings = left.min(self.money);
        self.money -= from_takings;
        left -= from_takings;
        for wallet in &mut self.wallets {
            let take = left.min(*wallet);
            *wallet -= take;
            left -= take;
        }
    }

    /// Whether a site may be begun now: its price is inside what the pool
    /// has left after the sites already begun. What keeps two Bims from
    /// walking to two parts the crew can only afford one of.
    pub fn affordable_site(&self, site: &BuildSite) -> bool {
        self.aboard.room.building_at(site.id) || site.price(&self.ship.design) <= self.free_money()
    }

    /// Every site, one order each, this step: where it is and how long
    /// putting it together takes. A site the crew cannot afford, or one
    /// on a site that is itself still a site, is on the list with nothing
    /// to start at it — the walk has to find it — and `minutes` nought is
    /// what says so. In the order the sites were laid out, which is what
    /// the room picks from.
    pub fn build_orders(&self) -> Vec<bims::game::Build> {
        let design = &self.ship.design;
        let free = shipdesign::Budget::new(Money::MAX);
        let t = shipdesign::TILE as f32;
        let offset = self.aboard.offset;
        self.builds
            .iter()
            .map(|site| {
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
                // Only a part that would go down now, and that the crew
                // can pay for, is worth walking to: one on a site that is
                // itself waiting is not.
                let buildable = self.affordable_site(site)
                    && shipdesign::apply(design, &free, site.edit()).is_ok();
                let minutes = if buildable {
                    build::build_minutes(site.price(design)) as f32
                } else {
                    0.0
                };
                bims::game::Build {
                    site: site.id,
                    tiles,
                    minutes,
                }
            })
            .collect()
    }

    /// Who may put a suit on and go out to a site beyond the hull, by
    /// crew member: everybody, while there is a suit aboard to wear. The
    /// airlock itself is the room's to find.
    fn suit_ok(&self) -> Vec<bool> {
        let suit = self.ship.design.carrying(ResourceId::Suit) > 0;
        vec![suit; self.aboard.crew_count() as usize]
    }

    /// A Bim put a site together: the part goes down and its recipe comes
    /// out of the hold in one go, if the rules still allow it — the deck
    /// under it may have been laid out and cancelled since, the metal sold
    /// — and an event either way. The site is finished with whatever
    /// happened: a part that will not go is a site to lay out again, not
    /// one to stand at for ever. The room is laid out again under the
    /// crew with the part in it.
    pub(crate) fn finish_build(&mut self, site: u32, events: &mut Vec<WorldEvent>) {
        let Some(at) = self.builds.iter().position(|s| s.id == site) else {
            return;
        };
        let site = self.builds.remove(at);
        // The price is this site's own, and the site is out of the list by
        // now — so it is checked against the whole pool less what the
        // *other* sites under way have spoken for, which is what
        // `free_money` says.
        let price = site.price(&self.ship.design);
        if price > self.free_money() {
            events.push(refused(0, Refusal::NotEnoughMoney));
            events.push(WorldEvent::BuildLost { kind: site.kind });
            return;
        }
        let free = shipdesign::Budget::new(Money::MAX);
        match shipdesign::apply(&self.ship.design, &free, site.edit()) {
            Ok(next) => {
                self.ship.design = next;
                // Paid for at the moment the part goes down, wherever the
                // ship is: money is the one thing that may be spent away
                // from a station (`shipdesign::materials`).
                self.spend_shared(price);
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
        // A relayout is a fresh `Sight`: the laid sandbags again (feature 74).
        self.sync_deployed_cover();
    }

    // --- holding at a belt ---------------------------------------------------

    /// Let go of the dock and hold at the first belt of this system.
    /// `false`, and nothing moved, when the system has no belt. There was
    /// a mining site laid out here until feature 95; a belt is a body
    /// with nothing to do at it now, and this is only a way to a node the
    /// ship has been at.
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
        true
    }

    /// The ship's power, as the crew would read it off a panel.
    pub fn power(&self) -> Power {
        let budget = self.power_budget;
        Power {
            supply: budget.supply,
            draw: budget.draw,
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
    /// chain deciding whether a bench works, and a chain has a kind in
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

    /// Whether that player's crew member is in a state to do anything at
    /// all for them: a slot with a Bim in it, alive, awake — neither out
    /// cold nor asleep — and aboard rather than out on the hull. What the
    /// desk and every class's key ask before where the body is standing.
    fn fit_to_act(&self, slot: u32) -> bool {
        if slot >= self.aboard.crew_count() {
            return false;
        }
        let room = &self.aboard.room;
        let who = slot as usize;
        room.is_alive(who) && !room.is_downed(who) && !room.is_outside(who)
    }

    // --- trading ------------------------------------------------------------

    /// How near the front this station's desk is, in hops — `None` for
    /// one too far out for it to matter, and for every station before the
    /// machines hold anything (feature 94).
    ///
    /// The system's own front ([`World::front`]), bar one case: a **town
    /// the crew defended and held** counts as one hop out whatever the
    /// chart says, since its system has fallen round it and it is the
    /// last friendly desk inside the infection.
    pub fn front_at(&self, station: u32) -> Option<u16> {
        let hops = if self.town_held(station) {
            1
        } else {
            self.front(self.star_id)?
        };
        (hops <= data::FRONT_HOPS).then_some(hops)
    }

    /// What this station's desk adds to its own lean on `resource`, per
    /// cent: [`data::FRONT_BIAS`] a hop inside [`data::FRONT_HOPS`], on
    /// war goods alone (`economy::market::war_goods`) and nought on
    /// everything else. Fifteen on the edge of the infection, five three
    /// hops out.
    pub fn front_bias(&self, station: u32, resource: ResourceId) -> i32 {
        if !market::war_goods(resource) {
            return 0;
        }
        let Some(hops) = self.front_at(station) else {
            return 0;
        };
        data::FRONT_BIAS * i32::from(data::FRONT_HOPS + 1 - hops)
    }

    /// **The one place a price is quoted.** What the desk at `station`
    /// asks for one unit of `resource` and what it bids for one — the
    /// station's kind and its own rolled lean through `economy::market`,
    /// with the front premium added on top. `None` where there is no desk
    /// at all: a derelict, a station the machines hold.
    ///
    /// Every quote in the game goes through here — `buy`, `sell`, and
    /// `ship::Session::quote` for the panels — so that nothing can show
    /// one price and charge another.
    pub fn quote(&self, station: u32, resource: ResourceId) -> Option<Quote> {
        if self.is_droid_held(station) {
            return None;
        }
        let desk = self.station(station)?.market()?;
        let bias = desk.bias.of(resource) + self.front_bias(station, resource);
        Some(market::quote(desk.kind, bias, resource))
    }

    /// The same for a piece of gear at a **tier** (feature 95): the
    /// tier-one quote with both sides multiplied by `economy::TIER_PRICE`,
    /// so a desk asks four times as much for a tier-two rifle as for a
    /// tier-one and bids four times as much for one off the crew's shelf.
    /// A resource that comes at no tier is quoted as it always was,
    /// whatever tier is asked for.
    pub fn quote_at(&self, station: u32, resource: ResourceId, tier: u32) -> Option<Quote> {
        let quote = self.quote(station, resource)?;
        Some(if economy::tiered(resource) {
            quote.at_tier(tier)
        } else {
            quote
        })
    }

    // --- the holdings and the loadouts (task 113) ----------------------------

    /// The design's gear cargo as things in the armory, and its research
    /// keys as the holdings' count, every one of those counts at nought
    /// after: what a world opens with. Nothing is stored in the hold, so a
    /// ship accepted carrying three helms sets out with three helms in
    /// the armory, whole, at tier one.
    fn stock_the_armory(&mut self) {
        for &id in ResourceId::ALL.iter() {
            let units = self.ship.design.carrying(id);
            if units == 0 {
                continue;
            }
            if let Some(kind) = armour::weapon_of(id) {
                for _ in 0..units {
                    self.holdings.put(Item::Weapon(kind.basic()));
                }
            } else if let Some(kind) = armour::kind_of(id) {
                // At its own lowest tier: a design carrying a Reflective
                // plate carries one at tier three (task 116).
                for _ in 0..units {
                    let piece = self.holdings.new_piece(kind, kind.min_tier());
                    self.holdings.put(Item::Armour(piece));
                }
            } else if matches!(id, ResourceId::ResearchKey | ResourceId::ResearchKeyTwo) {
                self.holdings.keys += units;
            } else {
                continue;
            }
            self.ship.design.cargo[id as usize] = 0;
        }
        self.on_ship_changed();
    }

    /// How many of a resource the crew have to sell: for a gun or a piece
    /// of armour, how many of that kind lie in the armory (what a sale
    /// takes from — never off a Bim); for anything else, the ship's count.
    pub fn held(&self, resource: ResourceId) -> u32 {
        if armour::weapon_of(resource).is_some() || armour::kind_of(resource).is_some() {
            return self
                .holdings
                .armory
                .iter()
                .filter(|s| armour::resource_of_item(s.item) == Some(resource))
                .count() as u32;
        }
        self.ship.design.carrying(resource)
    }

    /// A research key picked up (task 113): counted the moment it is.
    /// The one door a key comes in by, for whatever lays one on a site.
    pub fn pick_up_key(&mut self, events: &mut Vec<WorldEvent>) {
        self.holdings.keys = self.holdings.keys.saturating_add(1);
        events.push(WorldEvent::KeyFound {
            keys: self.holdings.keys,
        });
    }

    /// Whether player `slot` may change crew member `who`'s loadout: its
    /// own Bim, or a bot's — anybody past the players — and nobody
    /// else's. A player's Bim is given a thing only by an offer it
    /// accepts.
    pub fn may_change(&self, slot: u32, who: u32) -> bool {
        slot < self.players()
            && who < self.aboard.crew_count()
            && (who == slot || who >= self.players())
    }

    /// Why a loadout or armory command by `slot` on `who` would be
    /// refused now, if it would: in a mission with `who` anywhere but
    /// inside the ship, alive — the armory is the ship's, so a crew
    /// arriving at a site kits itself out from it before stepping off,
    /// and a Bim out on the deck keeps what it has until it comes back
    /// aboard — or not the player's to change.
    fn gear_refusal(&self, slot: u32, who: u32) -> Option<Refusal> {
        if who >= self.aboard.crew_count() {
            return Some(Refusal::NotAboard);
        }
        if self.in_mission() && !(self.inside_ship(who) && self.aboard.room.is_alive(who as usize))
        {
            return Some(Refusal::GearLocked);
        }
        if !self.may_change(slot, who) {
            return Some(Refusal::NotYours);
        }
        None
    }

    /// Whether player `slot` may change crew member `who`'s loadout
    /// **now**: [`World::may_change`], and in a mission only while `who`
    /// is inside the ship. What the Armory panel greys by.
    pub fn may_change_now(&self, slot: u32, who: u32) -> bool {
        self.gear_refusal(slot, who).is_none()
    }

    /// What crew member `who` has on `part`.
    pub fn worn_on(&self, who: u32, part: GearSlot) -> Option<Item> {
        (who < self.aboard.crew_count())
            .then(|| part.read(&self.aboard.room.gear(who as usize)))
            .flatten()
    }

    /// Put `item` on `who`'s `part` — or empty it — through the room, and
    /// withdraw every offer that slot was part of; what was there. The
    /// caller has checked the thing goes on the slot.
    fn set_slot(
        &mut self,
        who: u32,
        part: GearSlot,
        item: Option<Item>,
        events: &mut Vec<WorldEvent>,
    ) -> Option<Item> {
        let mut gear = self.aboard.room.gear(who as usize);
        let was = part.write(&mut gear, item).ok().flatten();
        self.aboard.room.issue(who as usize, gear);
        for offer in self.holdings.withdraw_touching(who, part) {
            events.push(WorldEvent::OfferWithdrawn {
                from: offer.from,
                part: offer.slot.code(),
                to: offer.to,
            });
        }
        was
    }

    /// [`Command::Equip`]: a thing out of the armory, or off another
    /// Bim's slot, onto `who`'s — the slot the thing is made for — and
    /// what was there into the armory.
    fn equip(&mut self, slot: u32, who: u32, from: GearSource, events: &mut Vec<WorldEvent>) {
        if let Some(why) = self.gear_refusal(slot, who) {
            events.push(refused(slot, why));
            return;
        }
        let item = match from {
            GearSource::Armory { id } => self.holdings.get(id).map(|s| s.item),
            GearSource::Worn {
                who: other,
                slot: part,
            } => {
                if let Some(why) = self.gear_refusal(slot, other) {
                    events.push(refused(slot, why));
                    return;
                }
                self.worn_on(other, part)
            }
        };
        let Some(item) = item else {
            events.push(refused(slot, Refusal::NoSuchGear));
            return;
        };
        let Some(part) = GearSlot::of_item(item) else {
            events.push(refused(slot, Refusal::NoSuchGear));
            return;
        };
        if matches!(from, GearSource::Worn { who: other, .. } if other == who) {
            // Onto the slot it is already on: nothing to do.
            return;
        }
        match from {
            GearSource::Armory { id } => {
                self.holdings.take(id);
            }
            GearSource::Worn { who: other, slot } => {
                self.set_slot(other, slot, None, events);
            }
        }
        if let Some(old) = self.set_slot(who, part, Some(item), events) {
            self.holdings.put(old);
        }
        events.push(WorldEvent::GearChanged {
            who,
            part: part.code(),
        });
    }

    /// [`Command::Unequip`]: what `who` has on `part` off into the armory.
    fn unequip(&mut self, slot: u32, who: u32, part: GearSlot, events: &mut Vec<WorldEvent>) {
        if let Some(why) = self.gear_refusal(slot, who) {
            events.push(refused(slot, why));
            return;
        }
        if self.worn_on(who, part).is_none() {
            events.push(refused(slot, Refusal::NoSuchGear));
            return;
        }
        if let Some(old) = self.set_slot(who, part, None, events) {
            self.holdings.put(old);
        }
        events.push(WorldEvent::GearChanged {
            who,
            part: part.code(),
        });
    }

    /// [`Command::Offer`]: what player `slot`'s own Bim has on `part`,
    /// offered to player `to`'s. One offer a slot: a second of the same
    /// slot replaces the first.
    fn offer(&mut self, slot: u32, part: GearSlot, to: u32, events: &mut Vec<WorldEvent>) {
        if self.in_mission() {
            events.push(refused(slot, Refusal::GearLocked));
            return;
        }
        if slot >= self.players() || to >= self.players() || to == slot {
            events.push(refused(slot, Refusal::NotAPlayer));
            return;
        }
        if self.worn_on(slot, part).is_none() {
            events.push(refused(slot, Refusal::NoSuchGear));
            return;
        }
        self.holdings
            .offers
            .retain(|o| !(o.from == slot && o.slot == part));
        self.holdings.offers.push(holdings::Offer {
            from: slot,
            slot: part,
            to,
        });
        events.push(WorldEvent::GearOffered {
            from: slot,
            part: part.code(),
            to,
        });
    }

    /// [`Command::AnswerOffer`]: the recipient taking or declining player
    /// `from`'s offer of its `part`, or the offerer taking it back.
    fn answer_offer(
        &mut self,
        slot: u32,
        from: u32,
        part: GearSlot,
        yes: bool,
        events: &mut Vec<WorldEvent>,
    ) {
        if self.in_mission() {
            events.push(refused(slot, Refusal::GearLocked));
            return;
        }
        let Some(at) = self
            .holdings
            .offers
            .iter()
            .position(|o| o.from == from && o.slot == part && (o.to == slot || o.from == slot))
        else {
            events.push(refused(slot, Refusal::NoOffer));
            return;
        };
        let offer = self.holdings.offers[at];
        // The offerer can only take it back; only the recipient takes it.
        if !yes || slot != offer.to {
            self.holdings.offers.remove(at);
            events.push(WorldEvent::OfferWithdrawn {
                from: offer.from,
                part: part.code(),
                to: offer.to,
            });
            return;
        }
        let Some(item) = self.worn_on(offer.from, part) else {
            self.holdings.offers.remove(at);
            events.push(refused(slot, Refusal::NoSuchGear));
            return;
        };
        self.holdings.offers.remove(at);
        self.set_slot(offer.from, part, None, events);
        if let Some(old) = self.set_slot(offer.to, part, Some(item), events) {
            self.holdings.put(old);
        }
        events.push(WorldEvent::OfferTaken {
            from: offer.from,
            part: part.code(),
            to: offer.to,
        });
    }

    /// Every piece there is made whole, worn or in the armory: what a
    /// mission's start does (task 113). Armour is never destroyed — a
    /// piece at nothing stays worn and does nothing until then.
    fn mend_all_armour(&mut self) {
        for stored in &mut self.holdings.armory {
            stored.item = holdings::mend(stored.item);
        }
        for who in 0..self.aboard.room.crew_count() as usize {
            let mut gear = self.aboard.room.gear(who);
            holdings::mend_gear(&mut gear);
            self.aboard.room.issue(who, gear);
        }
    }

    /// Crew member `who`'s weapon and armour into the armory, its slots
    /// left empty: what a dead or left-behind bot's loadout does at the
    /// end of a mission (task 113). Its charges go with it.
    fn store_loadout(&mut self, who: u32) {
        if who >= self.aboard.crew_count() {
            return;
        }
        let gear = self.aboard.room.gear(who as usize);
        for part in GearSlot::ALL {
            if let Some(item) = part.read(&gear) {
                self.holdings.put(holdings::mend(item));
            }
        }
        self.aboard
            .room
            .issue(who as usize, bims::combat::Gear::default());
    }

    /// The fight's broken pieces, said once each: a piece at nothing
    /// stays worn and does nothing for the rest of the mission (task
    /// 113), and nothing else happens to it.
    fn say_pieces_broken(&mut self, events: &mut Vec<WorldEvent>) {
        for (who, kind) in self.aboard.room.take_pieces_broken() {
            events.push(WorldEvent::PieceBroke {
                who: who as u32,
                kind,
            });
        }
    }

    // --- the bodies on the deck ----------------------------------------------

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

    /// Whether a body is one: dead, or downed, in whichever room it
    /// lies (`Game::is_down`). `false` for no such Bim.
    pub fn is_down(&self, source: LootSource) -> bool {
        self.body_room(source)
            .is_some_and(|(room, who)| room.is_down(who))
    }

    /// Where a body lies, in the crew's room's units — the ones
    /// `Game::send_to` and the pointer speak — so a crew member can be
    /// walked to it: one of the crew where it stands on the deck, or one of the
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
    /// body — alive, awake, aboard, and near enough to speak to it: what a
    /// hire asks of the hand being hired.
    pub fn in_reach_of_body(&self, who: u32, source: LootSource) -> bool {
        if who >= self.aboard.crew_count() {
            return false;
        }
        let room = &self.aboard.room;
        let looter = who as usize;
        if !room.is_alive(looter) || room.is_downed(looter) || room.is_outside(looter) {
            return false;
        }
        let Some(body) = self.body_position(source) else {
            return false;
        };
        (room.bim_pos(looter) - body).len() <= data::REACH * shipdesign::TILE as f32
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
    /// the fee, whether `who` is within reach of the body, and whether the
    /// money covers a month. A bunk is furniture, and puts no cap on the
    /// crew. `None` for anybody who is not a mercenary for hire. The
    /// command checks it all again when it lands.
    pub fn hire_offer(&self, who: u32, resident: u32) -> Option<Offer> {
        // The fee the crew member doing the hiring would pay: a
        // commander's is cheaper (feature 78), so the window says the
        // discounted price while one is steered.
        let fee = self.hire_fee(who, resident)?;
        Some(Offer {
            fee,
            in_reach: self.in_reach_of_body(who, LootSource::Resident(resident)),
            affordable: self.wallet(who) >= fee,
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

    /// Move one of the station's people out of its room and into the
    /// crew's, at the spot it stands on, with its worn armour renumbered
    /// as pieces of the world's under fresh ids. What a **hire** and a
    /// townsperson **joining** after a defence (feature 94) both do; the
    /// money, the contract and the kit are the caller's, and so is
    /// `on_ship_changed`/`mirror_pieces` afterwards.
    ///
    /// `for_hire` says which of the station's losses to count it against
    /// — a mercenary gone from its offer, or one of its own people gone
    /// from the town — so the room opens again with the right crowd
    /// either way.
    ///
    /// Answers the crew index it took and whether it was a field medic,
    /// or `None` with no station's room open.
    fn take_resident_aboard(&mut self, resident: u32, for_hire: bool) -> Option<(u32, bool)> {
        let at = self.body_position(LootSource::Resident(resident))?;
        let residents = self.residents.as_mut()?;
        if resident >= residents.aboard.room.crew_count() {
            return None;
        }
        // Out of the station's room — and off its crowd for good: the
        // station remembers one fewer ([`World::losses`]).
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
        residents.last_hit_by.remove(resident as usize);
        residents.fee.remove(resident as usize);
        let was_medic = residents.medic.remove(resident as usize);
        residents.grave.remove(resident as usize);
        if (resident as usize) < residents.defender.len() {
            residents.defender.remove(resident as usize);
        }
        memory::amend_losses(&mut self.losses, station, |l| {
            if for_hire {
                l.mercenaries += 1;
            } else {
                l.dead += 1;
            }
        });
        // Into the crew's, where it stood on the deck, with the gear it
        // brings as its loadout (task 113), its armour renumbered off the
        // holdings — and its berth left behind, since that was the
        // station's: `adopt` gives it the first bunk spare.
        let new_who = self.aboard.crew_count();
        body.bed = None;
        let mut gear = body.gear;
        for part in Part::ALL {
            if let Some(worn) = gear.worn_mut(part) {
                worn.id = self.holdings.take_id();
            }
        }
        body.character.stand_at(at);
        self.aboard.room.adopt(vec![body], bims::math::Vec2::ZERO);
        self.aboard.crew = self.aboard.room.crew_count();
        // Crew now, and the crew keep no station's round (feature 102).
        self.aboard.room.clear_routine(new_who as usize);
        // `issue` puts the renumbered pieces on and redraws the body.
        self.aboard.room.issue(new_who as usize, gear);
        self.crew_down.push(false);
        self.crew_locked.push(false);
        // Crew indices are what a beam links by, so every beam is broken
        // by one arriving (feature 76); the new hand's state starts empty.
        self.clear_beams();
        self.clear_carries();
        self.medics
            .resize(self.aboard.crew as usize, Medic::default());
        self.cloaks
            .resize(self.aboard.crew as usize, crate::medic::Cloak::default());
        self.tanks
            .resize(self.aboard.crew as usize, Tank::default());
        self.commanders
            .resize(self.aboard.crew as usize, Commander::default());
        self.ship.crew_count = self.aboard.crew;
        Some((new_who, was_medic))
    }

    /// A hire: see [`Command::Hire`] for what is asked. The station's
    /// room gives the body up (`take_crew`, the one taken out, `adopt` the
    /// rest back — every errand ashore is dropped once, the way a docking
    /// drops the crew's) and the crew's room takes it in at the same spot
    /// on the joined deck, in its own coverall still. Its armour becomes
    /// pieces of the world's under fresh ids. The first month is paid now
    /// and the next falls due a month on.
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
        // The fee the *sending* slot signs for: a commander's discount
        // is his own, and it is what goes into the contract (feature
        // 78).
        let fee = self.hire_fee(slot, resident).unwrap_or(offer.fee);
        if self.wallet(slot) < fee {
            events.push(refused(slot, Refusal::Unaffordable));
            return;
        }
        let Some((new_who, hire_is_medic)) = self.take_resident_aboard(resident, true) else {
            events.push(refused(slot, Refusal::NotDocked));
            return;
        };
        self.pay_from(slot, fee);
        self.hired.push(Hired {
            who: new_who,
            by: slot,
            fee,
            due: self.clock_minutes + mercenary::MONTH,
            owed: false,
            medic: hire_is_medic,
        });
        self.on_ship_changed();
        events.push(WorldEvent::Hired { who: new_who });
    }

    /// The hired hands' months, as they fall due: paid out of the money
    /// while it covers them, and a month it will not cover is owed — said
    /// once — until it can be. Asked when a trip puts the world clock on
    /// (`pay_wages_due`), the one time it moves, with the ship holding
    /// between missions: an unpaid hand sails on owed. (It walked off at a
    /// berth when the clock ran with the step at one; that went with the
    /// free clock, feature 104.)
    fn pay_wages(&mut self, events: &mut Vec<WorldEvent>) {
        let clock = self.clock_minutes;
        for i in 0..self.hired.len() {
            let hired = self.hired[i];
            if !mercenary::owed(&hired, clock) {
                continue;
            }
            if self.pay_from(hired.by, hired.fee) {
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
        }
    }

    // --- the station's research desk ------------------------------------------

    /// Which of the research desks on the deck is the station's, while the
    /// rooms are joined: the one standing in the station's box. `None` on
    /// a ship of its own, or at a station without one. Research is gone
    /// from the game (feature 106); the desk stays a solid on every
    /// layout, and at a site the machines hold it is where a **relic
    /// cache** lies ([`World::cache_here`]).
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

    /// Where a crew member stands at the station's research desk, in the
    /// room's units, for the app to `send_to`. `None` with no such desk on
    /// the deck.
    pub fn research_desk_spot(&self) -> Option<bims::math::Vec2> {
        self.aboard.room.research_spot(self.station_desk()?)
    }

    /// Whether crew member `who` stands within [`data::REACH`] of the
    /// station's research desk, alive and awake.
    pub fn research_desk_in_reach(&self, who: u32) -> bool {
        let Some(desk) = self.station_desk() else {
            return false;
        };
        let room = &self.aboard.room;
        who < self.aboard.crew_count()
            && !room.is_downed(who as usize)
            && room.within_reach(who as usize, Container::Desk(desk), data::REACH)
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
            // At a settlement, the view is about the planet it is on.
            ShipState::Docked { station } => match surface::surface_body(*station) {
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
            ShipState::Holding => match self.ship.frame {
                Frame::Local(node) if self.within_exit_radius(node) => node,
                _ => self.nearest_discovered()?,
            },
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
    /// planet or at a belt.
    fn mark_visited(&mut self) {
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

    /// What the world is actually running at: paused if anybody asked for a
    /// pause, 1× otherwise.
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
    /// order to the crew's room (`Command::Crew`) —
    /// a click on the deck at a pause is a click that shows, and a walk
    /// begun between two steps is what the room always did. Deterministic
    /// all the same, since every copy applies the same commands in the
    /// same order (`crates/app/src/net.rs`); what it changes is *when*
    /// between two steps, and both ends agree about that too.
    pub fn applies_at_once(command: &Command) -> bool {
        // And the run's own (feature 103): a vote on the map, where no
        // step is taken at all, a press of *Back to ship*, an answer to
        // the departure check and a player gone. The trip a last yes
        // sets off, and the leaving the last answer allows, happen at
        // the same place in every copy's order, like everything here.
        matches!(
            command,
            Command::SetSpeed { .. }
                | Command::Crew { .. }
                | Command::Propose { .. }
                | Command::Accept { .. }
                | Command::Return { .. }
                | Command::LeaveBehind { .. }
                | Command::PlayerGone { .. }
                // And *Ready*: a held mission takes no step to start.
                | Command::Ready { .. }
                | Command::ProposeRelic { .. }
                | Command::AcceptRelic { .. }
                // And the loadouts' (task 113): changed on the map and the
                // reward screen, where nothing steps, and refused in a
                // mission whenever they land.
                | Command::Equip { .. }
                | Command::Unequip { .. }
                | Command::Offer { .. }
                | Command::AnswerOffer { .. }
                // And the trader's (task 114): bought and combined on the
                // map, where nothing steps.
                | Command::BuyShelf { .. }
                | Command::Combine { .. }
                | Command::Restock { .. }
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
    /// room's is what the light and the Bims' day go by, so it is the one
    /// the player is shown. Read as `clock_minutes % DAY` it would be eight
    /// hours behind. The host formats; no strings cross the boundary.
    pub fn day(&self) -> u32 {
        self.aboard.room.clock_day()
    }

    pub fn minutes_into_day(&self) -> f64 {
        self.aboard.minutes() % time::DAY
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
    /// For probes. The alternative is a test that has to put the ship
    /// beside a body whose position is whatever the seed happened to put it
    /// at, which would be a test of the generator dressed up as a test of
    /// discovery. Same reason the room has `put_for_probe`.
    pub fn discover_for_probe(&mut self, from: DVec2, to: DVec2) -> Vec<WorldEvent> {
        let mut events = Vec::new();
        self.discover_along(from, to, &mut events);
        events
    }

    /// Put the ship's centre of mass somewhere.
    ///
    /// For probes, and for one thing in particular: nothing moves a ship
    /// under its own power, so there is no other way to walk it across a
    /// local frame's boundary and back.
    pub fn put_for_probe(&mut self, at: DVec2) {
        self.ship.set_position(at);
    }

    /// The room laid out again on the ship as it is — a relayout with no
    /// part built, which is what a relayout is to the sandbags laid on it
    /// (feature 74). For the tests.
    pub fn relayout_room_for_probe(&mut self) {
        self.relayout_room();
    }

    /// The ship put at a station's berth from a hold — `dock_at`, the
    /// state with it — for the tests: what an arrival ends in, without
    /// the trip.
    pub fn dock_at_for_probe(&mut self, station: u32) {
        self.ship.state = ShipState::Docked { station };
        self.ship.frame = Frame::Local(Node::Station(station));
        self.dock_at(station);
        self.settle_residents();
    }

    /// Let go of the dock without going anywhere: holding, with the ship's
    /// room its own again, and in no frame until the next settle — a
    /// holding ship keeps the frame it is in, and a probe that puts the
    /// ship somewhere else next wants it to start from nowhere. For probes
    /// of what happens away from a station.
    pub fn undock_for_probe(&mut self) {
        self.ship.state = ShipState::Holding;
        self.ship.frame = Frame::Space;
        self.unjoin_rooms();
    }

    /// The ship set down on the first planet with ground in the system:
    /// off its berth, tied up at the settlement with the rooms joined, the
    /// view about the planet. How the ground is looked at without a trip
    /// there — `test_planet`, `droids_planet` and `defense` in the app.
    /// False in a system with nowhere to land.
    /// The station alongside built again as the station `seed` generates
    /// (feature 112), its kind and its place kept: the seed put on it and
    /// its plan rolled again off that, then docked from the start as
    /// `kind` over its own when one is given. Docked from the start as
    /// [`World::arena_dock_for_probe`] docks — so a layout seen on a
    /// screenshot is the layout `BIMS_STATION_SEED` asks for, and the
    /// `nav_map_of_a_station` probe prints it with the same seed and kind.
    /// `false`, and nothing moved, away from a station's berth.
    pub fn regenerate_dock_for_probe(
        &mut self,
        seed: u64,
        kind: Option<worldgen::StationKind>,
    ) -> bool {
        let Some(id) = self.ship.state.station() else {
            return false;
        };
        let Some(i) = self.stations.iter().position(|s| s.id == id) else {
            return false;
        };
        let station = &mut self.stations[i];
        station.map_seed = seed;
        if let Some(kind) = kind {
            station.kind = kind;
        }
        station.replan(crate::station::Plan::rolled(seed));
        self.undock_for_probe();
        self.residents = None;
        self.ship.state = ShipState::Docked { station: id };
        self.dock_at(id);
        true
    }

    /// The town [`World::land_for_probe`] sets down at drawn from `seed`
    /// instead of its own (feature 112) — its biome and its population
    /// kept, so `BIMS_TOWN_SEED`, `BIMS_TOWN_BIOME` and
    /// `BIMS_TOWN_POPULATION` on the `nav_map_of_a_station` probe print it.
    /// Before the landing; the town is built when it is first asked for.
    /// Its biome and population, or `None` with no ground in the system.
    pub fn reseed_ground_for_probe(&mut self, seed: u64) -> Option<(crate::surface::Biome, u32)> {
        let surface = self.surfaces.first_mut()?;
        surface.reseed(seed);
        Some((surface.biome, surface.population))
    }

    pub fn land_for_probe(&mut self) -> bool {
        let Some(body) = self.surfaces.first().map(|s| s.body) else {
            return false;
        };
        let id = surface::surface_id(body);
        // A town the machines hold as an outpost (task 136) is given back:
        // what lands here is a friendly town.
        self.give_back_outposts(Some(id));
        self.undock_for_probe();
        self.residents = None;
        self.ship.state = ShipState::Docked { station: id };
        self.dock_at(id);
        self.settle_frame_for_probe();
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

    /// Tie up at a station without a trip there: docked, the rooms joined.
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

    /// Stage a fight with the machines: the station the ship is tied to
    /// put in their hands ([`World::infest`]) and its wave count settled
    /// as a first dock would, and in place of its first wave **one
    /// machine** of `kind`, stood a few tiles down the corridor from the
    /// station's port facing it, with the first crew member recruited and
    /// stood just inside the station's door — the state a fight is looked
    /// at and tested in without walking the station for one. What
    /// `stage_fight_for_probe` did with a station's people before every
    /// human was friendly (feature 104).
    ///
    /// The machine carries `weapon`, or, with none, is held where it is
    /// put (`Droid::posing`): it neither walks nor fires and is a target
    /// and nothing else — the disarmed enemy a test of the crew's own
    /// fire wants. A held station's room has no Bims, so it is body
    /// nought of its room: `LootSource::Resident(0)` to the world. Its
    /// wave is the first, so its going down starts the clock to the
    /// next. `false`, and nothing moved, away from a berth or at a
    /// station with no door.
    pub fn stage_droid_fight_for_probe(
        &mut self,
        kind: bims::droid::DroidKind,
        weapon: Option<Weapon>,
    ) -> bool {
        let Some(station) = self.ship.state.station() else {
            return false;
        };
        let Some(ashore) = self.aboard.ashore else {
            return false;
        };
        let Some((port, seed)) = self
            .station(station)
            .and_then(|s| Some((s.port()?, s.map_seed)))
        else {
            return false;
        };
        self.infest(station);
        let waves = self.droid_wave_count();
        if let Some(it) = self.infestation_mut(station) {
            it.settle(waves);
        }
        // Four tiles further in than the crew member, along the corridor
        // the port opens onto, in the station's own frame — where a
        // resident was stood for the same picture.
        let reach = (data::ASHORE_TILES + 4.0) * shipdesign::TILE as f64;
        let there = dvec2(
            port.centre.0 - port.outward.0 as f64 * reach,
            port.centre.1 - port.outward.1 as f64 * reach,
        );
        let facing = bims::math::vec2(port.outward.0 as f32, port.outward.1 as f32).angle();
        let tier = self.droid_tier();
        let Some(residents) = self.residents.as_mut().filter(|r| r.station == station) else {
            return false;
        };
        let at = residents.aboard.to_room(there);
        let mut droid = bims::droid::Droid::new(kind, tier, 0, 1, at, facing, seed);
        match weapon {
            Some(weapon) => droid.weapon = weapon,
            None => droid.posing = true,
        }
        residents.aboard.room.clear_droids();
        residents
            .aboard
            .room
            .adopt_droids(vec![droid], bims::math::Vec2::ZERO);
        residents.aboard.crew = residents.aboard.room.body_count();
        // The crew member: just inside the station's door, under orders.
        self.aboard
            .room
            .put_for_probe(0, bims::math::vec2(ashore.x as f32, ashore.y as f32));
        self.aboard.room.recruit_for_probe(0, true);
        true
    }

    /// Everybody's kit at one tier: every crew member's weapon at `tier`
    /// (its kind kept, the pistol for an empty hand — and a kind never
    /// made that low at its own lowest tier instead, task 115: a rail
    /// lance at tier two is no weapon) and a fresh helm,
    /// kevlar and leg guards at it over whatever was worn — ids off the
    /// holdings, so the checksum and the health bars see them like any
    /// other. The crew's
    /// half of the `tier2_test` and `tier3_test` commands, whose machines
    /// come at the tier of themselves (`World::set_droid_tier_for_probe`):
    /// the fight with nothing at tier one on either side. Whatever was
    /// worn before is dropped, not stowed. For probes and for the app.
    pub fn outfit_for_probe(&mut self, tier: Tier) {
        let armed = |gear: &bims::combat::Gear| {
            let kind = gear.weapon.map_or(WeaponKind::LaserPistol, |w| w.kind);
            Some(kind.at(tier.max(kind.min_tier())))
        };
        for who in 0..self.aboard.crew_count() as usize {
            let mut gear = self.aboard.room.gear(who);
            gear.weapon = armed(&gear);
            for kind in ArmourKind::BASIC {
                *gear.worn_mut(kind.slot()) = Some(self.holdings.new_piece(kind, tier));
            }
            self.aboard.room.issue(who, gear);
        }
    }

    /// Every weapon and every piece of armour there is, one of each kind
    /// at every tier it is made at (`WeaponKind::ALL`, the carried kinds,
    /// and `ArmourKind::ALL`, each by `made_at`), put into the armory: what the
    /// combat-ship runs open with so any kit can be tried on. For probes
    /// and for the app.
    pub fn stock_every_thing_for_probe(&mut self) {
        for tier in Tier::ALL {
            for kind in WeaponKind::ALL {
                if kind.made_at(tier) {
                    self.holdings.put(Item::Weapon(kind.at(tier)));
                }
            }
            for kind in ArmourKind::ALL {
                if kind.made_at(tier) {
                    let piece = self.holdings.new_piece(kind, tier);
                    self.holdings.put(Item::Armour(piece));
                }
            }
        }
    }

    /// Rebuild the station the ship is tied to as the arena —
    /// [`crate::station::arena`], the same kind and seed laid out bigger —
    /// standing where it stood, and dock there again: the `droids`
    /// command's dock before the machines have it, for a fight with room
    /// to move. The rooms are laid out afresh on the new deck, so the crew
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
        // Docked again from the start: the berth moved with the hull, the
        // joined deck is the new one, and the residents' room — opened on
        // the old design — is opened again on this.
        self.undock_for_probe();
        self.residents = None;
        self.ship.state = ShipState::Docked { station: id };
        self.dock_at(id);
        true
    }

    /// The first of the station alongside's people — a town's guard —
    /// stood a tile from crew member 0 and downed there, its countdown
    /// running: a townsperson to pick up with the medkit, without a fight.
    /// `false`, and nothing moved, with no room joined or nobody in it.
    /// For `BIMS_DOWN_RESIDENT` in the app.
    pub fn down_resident_for_probe(&mut self) -> bool {
        // A landing's room is peopled by the steps after it, and a
        // defence's first step stands the crew ashore.
        for step in 0..10 {
            if step >= 2
                && self
                    .residents
                    .as_ref()
                    .is_some_and(|r| r.aboard.room.crew_count() > 0)
            {
                break;
            }
            self.step(&[]);
        }
        let near = self.aboard.room.bim_pos(0) + bims::math::vec2(shipdesign::TILE as f32, 0.0);
        let Some(station) = self.aboard.to_station(dvec2(near.x as f64, near.y as f64)) else {
            return false;
        };
        let Some(residents) = self.residents.as_mut() else {
            return false;
        };
        if residents.aboard.room.crew_count() == 0 {
            return false;
        }
        let there = residents.aboard.to_room(station);
        residents.aboard.room.put_for_probe(0, there);
        residents.aboard.room.knock_out_for_probe(0);
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
}

/// The ship's power at this instant: units a minute in and out, and what
/// the batteries hold and have.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Power {
    pub supply: f64,
    /// The day-long draw: every wired consumer. The engines draw nothing,
    /// since nothing is flown.
    pub draw: f64,
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
        self.draw / self.supply
    }

    /// Whether the optional consumers have stopped: more drawn than made,
    /// and nothing left in the batteries to cover the difference. A ship
    /// with no batteries and a short network is browned out for good.
    pub fn brownout(&self) -> bool {
        self.draw > self.supply && self.charge <= 0.0
    }
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
        // **A site of the Manufacturers' is never the machines'** (feature
        // 109): the spread passes it by, whoever holds it, cleared or not.
        if self.is_manufacturer_station(id) {
            return;
        }
        // **A town the crew held is never taken** (feature 94): the
        // machines came for it once and were destroyed, and after that
        // the system falling round it changes nothing — it goes on
        // friendly, trading and hiring, inside the infection.
        if self.town_held(id) {
            return;
        }
        // **Nor is a trader** (tasks 114 and 111): the crisis passes one
        // by, and a trader is never somewhere to attack.
        if self.is_trader_here(id) {
            return;
        }
        // A town with a defence still running has **lost** it: the day
        // came while the crew were away with waves left, or its last
        // person is dead. Either way the fight is over and the machines
        // have the place.
        // A defence already over stays as it ended (task 111): a station
        // the crew held is infested later as any other, and its fight was
        // still won.
        if let Some(d) = self.defense_mut(id).filter(|d| !d.over()) {
            d.lost = true;
        }
        // And whether it hides a relic cache (feature 106): rolled here,
        // once, off the galaxy's seed.
        let mut it = Infestation::new(id);
        it.cache = self.is_elite_here(id)
            && crate::relic::cache_rolled(self.galaxy_seed, self.star_id, id);
        self.infested.push(it);
        self.infested.sort_by_key(|it| it.station);
        // The station's people are gone the moment the machines have it:
        // a room already open on it is opened again with nobody in it.
        self.reopen_residents(id);
        self.apply_stances();
    }

    /// Whether the last machine of the last wave at this station has been
    /// destroyed. What the crisis reads to know a station is won back;
    /// false for a station the machines never held.
    pub fn droid_station_cleared(&self, id: u32) -> bool {
        self.infestation(id).is_some_and(|it| it.cleared)
    }

    // --- the crisis (feature 92) ------------------------------------------
    //
    // The machines appear at one star on day ten and spread one hyperlane
    // hop every five days. The *rule* is a line of arithmetic — a star is
    // infested from `crisis_first_day + DROID_SPREAD_DAYS * hops` on, and
    // that is the whole of it: no per-tick state, no rolls, nothing to
    // accumulate, and two clients that agree about the day and the lane
    // graph agree about every star in the galaxy without exchanging a
    // word. What *is* state is the **flip**: the moment the crew's own
    // system turns, its stations go into the machines' hands
    // (`World::infest`, the droid step's own door) and stay there.

    /// Where the machines began. Rolled once at [`World::start`] and never
    /// again — saved, and in the checksum.
    pub fn droid_origin(&self) -> u32 {
        self.droid_origin
    }

    /// The hop table from the origin, worked out again off the galaxy.
    /// **Called at every load**, since the table is derived and a save
    /// carries only the origin; and by the probes that move the origin.
    pub fn settle_crisis(&mut self) {
        let galaxy = self.galaxy();
        self.droid_hops = galaxy.hops_from(self.droid_origin);
        self.home_hops = galaxy.hops_from(self.home_star);
        self.manufacturer_near =
            crate::manufacturer::near_sites(&galaxy, self.home_star, !self.whole_systems);
        self.trader_near = self.trader_near_sites(&galaxy);
        // The hop table is what says whether this system is theirs, so
        // the jammer is settled behind it — and this is the call every
        // load goes through (`ship::Game::resume`), which is what keeps a
        // derived jammer out of a save (feature 93).
        self.settle_jammer();
    }

    /// The day this star turns, counting from the day the world opened:
    /// the crisis's first day — nought in a run (feature 102) — plus
    /// [`data::DROID_SPREAD_DAYS`] a hop from the origin, and
    /// [`u32::MAX`] — never — for a star the lanes do not reach. What the
    /// chart says when a star is picked.
    pub fn infested_on(&self, star: u32) -> u32 {
        let hops = self
            .droid_hops
            .get(star as usize)
            .copied()
            .unwrap_or(u16::MAX);
        droidplan::turns_on(self.crisis_first_day, hops)
    }

    /// Whether the machines have this star's system: its day has come.
    ///
    /// Note that a *station* being droid-held ([`World::is_droid_held`])
    /// is the other half — the flip is what turns one into the other, and
    /// a station the crew have cleared stays cleared however long the
    /// system has been infested.
    pub fn infested(&self, star: u32) -> bool {
        let day = self.infested_on(star);
        day != u32::MAX && self.days_gone() >= day
    }

    /// Every star the machines have by now, in id order. The chart's, and
    /// it is every one of them charted or not: the crisis is not a secret.
    pub fn infested_stars(&self) -> Vec<u32> {
        (0..self.droid_hops.len() as u32)
            .filter(|&star| self.infested(star))
            .collect()
    }

    // --- the front (feature 94) -------------------------------------------
    //
    // The infection is a disc on the lane graph: every star within
    // `radius` hops of the origin has fallen, and the rest have not. The
    // **front** is how far outside that disc a star still is — one hop
    // out is the edge, and the edge is where a gun is dear and a
    // mercenary is easy to find. Both numbers are arithmetic off the day
    // and the hop table, like the spread itself: nothing is saved, and
    // two clients that agree about the day agree about the front.

    /// How far the infection has spread from the origin by today, in
    /// hops: the largest `n` with `crisis_first_day + DROID_SPREAD_DAYS *
    /// n` on or before the day gone. `None` before the first day, when
    /// the machines hold nothing at all.
    pub fn crisis_radius(&self) -> Option<u16> {
        let days = self.days_gone();
        if days < self.crisis_first_day {
            return None;
        }
        let spread = data::DROID_SPREAD_DAYS.max(1);
        Some(((days - self.crisis_first_day) / spread).min(u16::MAX as u32) as u16)
    }

    /// How far this star is outside the infection, in hops — one for a
    /// star on the edge of it, and up. `None` before the machines hold
    /// anything, for a star they already hold, and for one the lanes do
    /// not reach, which is never theirs and so never has a front.
    ///
    /// Derived, never saved: the day and the hop table are all of it.
    pub fn front(&self, star: u32) -> Option<u16> {
        let radius = self.crisis_radius()?;
        let hops = self.hops_from_origin(star);
        if hops == u16::MAX || hops <= radius {
            return None;
        }
        Some(hops - radius)
    }

    /// The day the origin turns, as this world counts it. The probes'
    /// dial reads and writes it; the game never moves it.
    pub fn crisis_first_day(&self) -> u32 {
        self.crisis_first_day
    }

    /// The `crisis` probe's dial (`BIMS_CRISIS_DAY`): the origin turns on
    /// this day instead of day nought, and every other star five days a
    /// hop after it.
    pub fn set_crisis_first_day_for_probe(&mut self, day: u32) {
        self.crisis_first_day = day;
    }

    /// The `crisis` probe's third dial: the clock wound on to the start of
    /// this day, so a flip that is ten days out is a minute away instead of
    /// a morning. It moves **everything** the clock decides — the wages,
    /// the crisis, how big a wave is — which is why it is a probe's and
    /// not a command's.
    ///
    /// The **crew's calendar goes with it** (`Game::wind_clock`, the way a
    /// station's room is wound to the world's day when it opens): the
    /// strip along the top reads `World::day`, which is the room's, and a
    /// probe that wound one clock and not the other would open on day ten
    /// of the crisis and Day 1 of the crew's own diary.
    pub fn set_day_for_probe(&mut self, day: u32) {
        let was = self.clock_minutes;
        self.clock_minutes = f64::from(day) * time::DAY;
        let on = self.clock_minutes - was;
        if on > 0.0 {
            self.aboard.room.wind_clock(on as f32);
            if let Some(residents) = &mut self.residents {
                residents.aboard.room.wind_clock(on as f32);
            }
        }
    }

    /// The `crisis` probe's other dial: the machines began at this star
    /// instead of the one the roll picked, and the hop table is worked
    /// out again from it. [`World::start_star_hops_for_probe`] is how a
    /// star a given number of hops off is found.
    pub fn set_droid_origin_for_probe(&mut self, star: u32) {
        self.droid_origin = star;
        self.settle_crisis();
    }

    /// How many lane hops every star is from the crew's own, for a probe
    /// that wants to put the origin a stated distance away.
    pub fn start_star_hops_for_probe(&self) -> Vec<u16> {
        self.galaxy().hops_from(self.star_id)
    }

    /// The `jammer` probe's dial: every station of this system into the
    /// machines' hands at once, the derived jammer laid first, without
    /// waiting for the crew to be off the berth the way
    /// [`World::spread_crisis`] does. The one thing it is short of is the
    /// docked guard, which is the whole point: the probe opens with the
    /// crew tied up at a held station.
    pub fn infest_here_for_probe(&mut self) {
        self.settle_jammer();
        let mut ids: Vec<u32> = self.stations.iter().map(|s| s.id).collect();
        ids.extend(self.surfaces.iter().map(|s| s.id));
        for id in ids {
            // Passed by as the crisis passes it (task 114), bar the dock
            // the probe opens tied up at.
            if self.is_trader_here(id) && self.ship.state.alongside() != Some(id) {
                continue;
            }
            self.infest(id);
        }
    }

    /// The crisis, applied to the system the ship is in: every station of
    /// it, orbital or town, goes into the machines' hands.
    ///
    /// Two things hold it off. **A station the crew have cleared stays
    /// cleared** — it keeps the [`Infestation`] it was cleared with, so
    /// `infest` refuses it and the crisis never re-arms it. And **the flip
    /// waits for the crew to leave**: a system whose day comes while the
    /// rooms are **joined** — docked — is the system they arrived in until
    /// the ship is off the berth. Anything else would empty a deck of the people standing on it
    /// while the crew were walking about among them, and take the deck they
    /// were standing on with it.
    ///
    /// A station's own room open *alongside* is not that: the crew are
    /// aboard their own ship, and `infest` opens the room again with the
    /// machines in it (`reopen_residents`, the same machinery a stance
    /// turning hostile uses) — which is the sight the crisis is worth
    /// watching for.
    fn spread_crisis(&mut self, events: &mut Vec<WorldEvent>) {
        if !self.infested(self.star_id) {
            return;
        }
        // A system with no orbital station of its own gets the machines'
        // own (feature 93) — **before** the wait for the crew to leave the
        // berth, since `World::jammer_station` names it the moment the
        // system is theirs and a name with no station behind it is a
        // helm that cannot say whose jammer is shut. It stands out in
        // space; laying it does nothing to the deck the crew are on.
        self.settle_jammer();
        if matches!(self.ship.state, ShipState::Docked { .. }) {
            return;
        }
        let mut ids: Vec<u32> = self.stations.iter().map(|s| s.id).collect();
        ids.extend(self.surfaces.iter().map(|s| s.id));
        let mut taken = false;
        for id in ids {
            if self.is_droid_held(id) {
                continue;
            }
            // A trader is never the machines' (task 114): it is closed while
            // its system is theirs, and open again once it is liberated.
            if self.is_trader_here(id) {
                continue;
            }
            self.infest(id);
            taken = true;
        }
        if !taken {
            return;
        }
        // What the crew did to the people who lived here is no longer
        // what they will meet: the people are gone. The memory filed
        // under this star is dropped the same way when it is recalled
        // (`World::recall_system`), for a system that turned while the
        // crew were somewhere else.
        self.losses.clear();
        self.graves.clear();
        events.push(WorldEvent::Infested { star: self.star_id });
    }

    // --- the jammer (feature 93) ------------------------------------------
    //
    // A jump goes one hop and only down a lane, and an infested system
    // holds the lanes *inward* shut while its jammer stands. Which station
    // the jammer is on is decided here and nowhere else; what it is built
    // out of when the system has no station of its own is `crate::jammer`.

    /// How many lane hops a star is from the machines' origin, and
    /// [`u16::MAX`] for one the lanes do not reach. The table the crisis
    /// is read off ([`World::infested_on`]) and the number the jam
    /// compares: inward is fewer.
    pub fn hops_from_origin(&self, star: u32) -> u16 {
        self.droid_hops
            .get(star as usize)
            .copied()
            .unwrap_or(u16::MAX)
    }

    /// Which station in this system holds the machines' jammer, or `None`
    /// when the system is not infested.
    ///
    /// The **orbital station with the lowest id**, a derived jammer
    /// excluded from the running — and, when the system has no orbital
    /// station at all, the derived one ([`crate::jammer`]), which
    /// [`World::settle_jammer`] has already put in the system by the time
    /// anybody asks. A town on a planet's surface is never it: a system
    /// may have no orbit worth the name, and the jammer has to be
    /// somewhere in every infested one.
    pub fn jammer_station(&self) -> Option<u32> {
        if !self.infested(self.star_id) {
            return None;
        }
        // Never a site of the Manufacturers' (feature 109), who have no
        // jammer, nor the system's trader, which is never a site to clear.
        self.jammer_site_among(self.star_id, &self.system.stations)
            .or_else(|| Some(jammer::jammer_id(self.star_id)))
    }

    /// Whether the lanes inward are shut: the system is infested and its
    /// jammer station has not been cleared. What
    /// [`Refusal::Jammed`] is said off.
    pub fn jammed(&self) -> bool {
        self.jammer_station()
            .is_some_and(|id| !self.droid_station_cleared(id))
    }

    /// Whether a jump from `from` to `to` would be turned back by a
    /// jammer: `from` infested, and `to` nearer the machines' origin than
    /// `from` is. For the system the ship is in the live answer is used —
    /// a jammer the crew have brought down is down — and for any other
    /// star the jammer is taken to be standing, which is what the chart
    /// draws along a route.
    pub fn jammed_step(&self, from: u32, to: u32) -> bool {
        let standing = if from == self.star_id {
            self.jammed()
        } else {
            self.infested(from)
        };
        standing && self.hops_from_origin(to) < self.hops_from_origin(from)
    }

    /// The shortest way from the star the ship is at to another, both ends
    /// in it — `worldgen::Galaxy::route`, which is breadth-first with ties
    /// to the lower star id. What the chart draws and counts hops off, and
    /// what the Jump button charges the first step of.
    pub fn route_to(&self, star: u32) -> Option<Vec<u32>> {
        self.galaxy().route(self.star_id, star)
    }

    /// Whether a lane joins the star the ship is at to this one: what a
    /// jump wants, and what the chart lights up round the ship.
    pub fn laned_to(&self, star: u32) -> bool {
        self.galaxy().lanes(self.star_id).contains(&star)
    }

    /// The stars a jump could reach from here, in id order: the lanes out
    /// of the ship's own star. For the chart.
    pub fn reachable_stars(&self) -> Vec<u32> {
        self.galaxy().lanes(self.star_id).to_vec()
    }

    /// The derived jammer station put into this system, or taken out of
    /// it. **The one place either happens**, and it is called wherever a
    /// system is settled: at the start, at a jump, at every load
    /// ([`World::settle_crisis`]) and the step a system falls to the
    /// crisis.
    ///
    /// A derived jammer is never saved: whatever a save carried is thrown
    /// away here and rolled again off the star's own stream, so the
    /// station is the seed's rather than an old build's. What is saved is
    /// what happened to it — its `Infestation`, filed by station id like
    /// any other station's.
    ///
    /// It is **charted the moment it is laid**, the way the crisis itself
    /// is not a secret: a jammer the crew cannot find is a system they
    /// cannot leave.
    pub fn settle_jammer(&mut self) {
        // First the system cut to what it offers (task 135): one station
        // and one town, its trader beside them — the same doors.
        self.settle_offered();
        // The Machine Heart's fortress (feature 108) is taken out first and
        // laid again last: it is never the jammer, it is not a station of
        // the system's own for the rule below, and a derived jammer is
        // rolled off the system without it.
        self.system.stations.retain(|s| !heart::is_heart(s.id));
        self.stations.retain(|s| !heart::is_heart(s.id));
        self.settle_derived_jammer();
        self.settle_heart();
        // And the Manufacturers' sites of this system held by them (feature
        // 109): the same doors — the start, a jump, a spread, every load.
        self.settle_manufacturers();
        // And the system's elite, the machines' from the first day, before
        // the outposts are dealt round it.
        self.settle_elite();
        // And the machines' outposts (task 136), behind the Manufacturers'
        // and the trader, which are never one.
        self.settle_outposts();
    }

    /// [`World::settle_jammer`]'s own half: the derived jammer, the
    /// fortress out of the system while it is decided.
    fn settle_derived_jammer(&mut self) {
        let derived = jammer::jammer_id(self.star_id);
        // A system with no station the jammer could be on: none at all, or
        // only the Manufacturers', who have no jammer (feature 109), and its
        // trader, which is never one.
        let own: Vec<worldgen::StationBlueprint> = self
            .system
            .stations
            .iter()
            .filter(|s| !heart::is_heart(s.id))
            .cloned()
            .collect();
        let wanted =
            self.infested(self.star_id) && self.jammer_site_among(self.star_id, &own).is_none();
        let had = self.stations.iter().any(|s| s.id == derived);
        if wanted && had {
            return;
        }
        // Out it comes — from the system, the station list and the keys
        // beside it, which are indexed alongside the stations. A derived
        // jammer is always the last of them, its id being past everything
        // the generator numbers, so the keys are simply cut back to the
        // stations: a memory or a save that carried one leaves an extra
        // entry behind otherwise.
        self.system.stations.retain(|s| !jammer::is_derived(s.id));
        self.stations.retain(|s| !jammer::is_derived(s.id));
        self.discovered
            .retain(|n| !matches!(n, Node::Station(id) if jammer::is_derived(*id)));
        if !wanted {
            return;
        }
        let blueprint = jammer::blueprint(&self.system, self.galaxy_seed, self.star_id);
        let at = blueprint.position;
        self.system.stations.push(blueprint.clone());
        // Its id is bigger than anything the generator numbers, so the end
        // of the list is id order and the keys stay in step.
        self.stations.push(Station::build(&blueprint, at));
        self.discovered.push(Node::Station(derived));
        self.discovered.sort_by_key(node_key);
        self.discovered.dedup();
    }

    /// What tier the machines come at (feature 93): **tier three within
    /// [`data::DROID_TIER_THREE_HOPS`] hops of the origin** and tier one
    /// anywhere else, until there is a general rule for what tier an enemy
    /// carries. `BIMS_DROID_TIER` overrides it in the probes, which is the
    /// only thing that does.
    pub fn droid_tier(&self) -> Tier {
        // The Machine Heart's fortress is tier three whatever else is said
        // (feature 108): the fight the run is won by is the hardest there is.
        if self
            .residents
            .as_ref()
            .is_some_and(|r| heart::is_heart(r.station))
        {
            return Tier::Three;
        }
        if let Some(tier) = self.droid_tier {
            return tier;
        }
        let site = self.ship.state.alongside().or(self.run.site);
        self.site_tier(self.star_id, site, self.clock_minutes)
    }

    /// What tier the machines at a site come at, at the world clock
    /// `clock_minutes`: **tier three within [`data::DROID_TIER_THREE_HOPS`]
    /// hops of the origin**; past [`data::ENEMY_TIER2_HOURS`], **tier two**
    /// on the distance ramp from the crew's own star
    /// ([`crate::relic::tier_two_rolled`], feature 106) — tier two used to
    /// wait on the crew researching it; and tier one everywhere else. The
    /// wave on arrival ([`World::droid_tier`]) and the map's quote read
    /// the same answer. `station` `None` is anywhere in the system, which
    /// the ramp calls tier two only past its sure distance.
    pub fn site_tier(&self, star: u32, station: Option<u32>, clock_minutes: f64) -> Tier {
        if self.hops_from_origin(star) <= data::DROID_TIER_THREE_HOPS {
            return Tier::Three;
        }
        if clock_minutes < f64::from(data::ENEMY_TIER2_HOURS) * time::HOUR {
            return Tier::One;
        }
        let hops = self
            .home_hops
            .get(star as usize)
            .copied()
            .unwrap_or(u16::MAX);
        let two = match station {
            Some(id) => crate::relic::tier_two_rolled(self.galaxy_seed, star, id, hops),
            None => hops >= data::ENEMY_TIER2_SURE_HOPS,
        };
        if two { Tier::Two } else { Tier::One }
    }

    /// The tiers a star's sites come at, at the world clock
    /// `clock_minutes` — the least and the most, off [`World::site_tier`]'s
    /// own rule without generating the system: one tier where the rule is
    /// sure of it, and one to two on the distance ramp, where it is rolled
    /// a site. What the galaxy chart writes beside every star. The probes'
    /// dial, where set, is every site's.
    pub fn system_tiers(&self, star: u32, clock_minutes: f64) -> (Tier, Tier) {
        if let Some(tier) = self.droid_tier {
            return (tier, tier);
        }
        let sure = self.site_tier(star, None, clock_minutes);
        if sure != Tier::One || clock_minutes < f64::from(data::ENEMY_TIER2_HOURS) * time::HOUR {
            return (sure, sure);
        }
        let hops = self
            .home_hops
            .get(star as usize)
            .copied()
            .unwrap_or(u16::MAX);
        if hops == 0 {
            (Tier::One, Tier::One)
        } else {
            (Tier::One, Tier::Two)
        }
    }

    /// The probes' dial: every wave from now on comes at this tier, or
    /// `None` to put the distance rule back.
    pub fn set_droid_tier_for_probe(&mut self, tier: Option<Tier>) {
        self.droid_tier = tier;
    }

    /// How long after a wave is spent the next arrives, in minutes of
    /// the mission clock (feature 103).
    pub fn droid_reinforce_minutes(&self) -> f64 {
        self.droid_reinforce as f64 * data::STEP_MINUTES
    }

    /// The same in steps of the mission clock, which is what it is kept in.
    pub fn droid_reinforce_steps(&self) -> u64 {
        self.droid_reinforce
    }

    /// The machines' own reinforcement clock, shortened: what the
    /// `droids` probes set to a minute so a wave can be watched arriving
    /// without waiting two of the mission clock. In minutes of it, a
    /// minute being sixty steps; nought is the very next step.
    pub fn set_droid_reinforce_minutes_for_probe(&mut self, minutes: f64) {
        self.droid_reinforce = steps_of(minutes);
    }

    /// How many machines the next wave is, worked out now: the base, the
    /// players and the world clock ([`droidplan::wave_size`]) — never
    /// the bots, the worth or the levels (feature 105). Asked as each wave
    /// appears, never stored.
    ///
    /// **At a site the crew are defending** (task 111) the defenders the
    /// site fielded — standing or fallen — count as that many more
    /// players: a site that brings its own guns meets a wave the size
    /// those guns would have met aboard. Nothing else about the formula
    /// moves.
    ///
    /// **In the run's first mission** every wave is
    /// `first_mission_ease` fewer ([`droidplan::WaveScaling`];
    /// [`data::FIRST_MISSION_WAVE_EASE`] untuned), never under one — unless a
    /// probe forced it.
    pub fn droid_wave_size(&self) -> u32 {
        let defenders = if self.defense_here().is_some() {
            self.defenders_fielded()
        } else {
            0
        };
        let size = self.wave_size_with(self.hours_gone(), defenders);
        let forced = self.droid_kinds_forced.is_some() || self.droid_wave_forced.is_some();
        if self.run.missions <= 1 && !forced && !self.first_mission_uneased {
            size.saturating_sub(self.scaling().first_mission_ease)
                .max(1)
        } else {
            size
        }
    }

    /// [`World::droid_wave_size`] with the world clock at `hours` gone:
    /// what a wave would be on arrival, for the map's preview of the
    /// Machine Heart (feature 108) as well as for the wave appearing now.
    pub fn wave_size_at(&self, hours: u32) -> u32 {
        self.wave_size_with(hours, 0)
    }

    /// The wave at `hours` gone with `more` on top of the players.
    fn wave_size_with(&self, hours: u32, more: u32) -> u32 {
        // A wave forced to its machines is as many as it names.
        if let Some(kinds) = &self.droid_kinds_forced {
            return (kinds.len() as u32).max(1);
        }
        // The probes' dial says the size outright: the measurements of
        // what a wave of that many costs, and the `droids` commands'
        // sixteen.
        if let Some(forced) = self.droid_wave_forced {
            return forced.max(1);
        }
        self.scaling().size(self.players() + more, hours).max(1)
    }

    /// The probes' other dial (`BIMS_DROID_WAVE`): every wave from now
    /// on is this many machines, whatever the formula says.
    /// For the measurements feature 83 asks for, and nothing else.
    pub fn set_droid_wave_for_probe(&mut self, n: u32) {
        self.droid_wave_forced = Some(n.max(1));
    }

    /// The probes' third dial (feature 100): every wave from now on is
    /// exactly `kinds`, in that order, at the world's tier — the
    /// `guardian` command's one Guardian and two Troopers.
    pub fn set_droid_kinds_for_probe(&mut self, kinds: Vec<bims::droid::DroidKind>) {
        self.droid_kinds_forced = Some(kinds);
    }

    /// Every defence's waves the machines' from now on, whatever the day
    /// (task 131): for the tests of the machines' fight at a defence,
    /// which run at day nought, where the game sends the Manufacturers.
    pub fn set_defense_by_machines_for_probe(&mut self) {
        self.defense_by_machines_forced = true;
    }

    /// The run's first mission as strong as any other: no
    /// `first_mission_ease` off its waves (the app's `end` command).
    pub fn set_first_mission_uneased_for_probe(&mut self) {
        self.first_mission_uneased = true;
    }

    /// Tune the wave formula (`scaling.ron`, read by the app while the
    /// game runs): from the next wave laid and the next count settled
    /// on, the dials are these. Not saved and not hashed.
    pub fn set_wave_scaling(&mut self, scaling: droidplan::WaveScaling) {
        self.wave_scaling = scaling;
    }

    /// The wave formula's dials as the tuning file handed them, without
    /// the run's [`World::difficulty`] over them.
    pub fn wave_scaling(&self) -> droidplan::WaveScaling {
        self.wave_scaling
    }

    /// Set the run's difficulty (the game setup's pick, dealt at Start):
    /// from the next wave laid and the next count settled on, its three
    /// dials stand over the tuning file's. `None` is the file's own.
    pub fn set_difficulty(&mut self, difficulty: Option<droidplan::Difficulty>) {
        self.difficulty = difficulty;
    }

    /// The run's difficulty, if the setup picked one.
    pub fn difficulty(&self) -> Option<droidplan::Difficulty> {
        self.difficulty
    }

    /// The wave formula as it is worked: the tuning file's dials with the
    /// run's difficulty over them.
    pub fn scaling(&self) -> droidplan::WaveScaling {
        match self.difficulty {
            Some(d) => d.over(self.wave_scaling),
            None => self.wave_scaling,
        }
    }

    /// Tune what a fight pays and what things cost (`rewards.ron`, read
    /// by the app while the game runs): from the next enemy down and the
    /// next price asked. Not saved and not hashed.
    pub fn set_rewards(&mut self, rewards: crate::rewards::Rewards) {
        self.rewards = rewards;
    }

    /// The reward and price dials as they stand.
    pub fn rewards(&self) -> crate::rewards::Rewards {
        self.rewards
    }

    /// The wave size a probe has forced (`BIMS_DROID_WAVE`), if any.
    pub fn droid_wave_forced(&self) -> Option<u32> {
        self.droid_wave_forced
    }

    /// How many waves a held station has all told, worked out now: the
    /// scaling's count at the tier the machines come at
    /// ([`World::droid_tier`]). Only ever asked once a station, at the
    /// crew's first dock.
    pub fn droid_wave_count(&self) -> u32 {
        self.wave_count_for(self.droid_tier())
    }

    /// [`World::droid_wave_count`] for machines at `tier`.
    pub fn wave_count_for(&self, tier: Tier) -> u32 {
        // The probes' dial says it outright, the way `droid_wave_size`
        // takes its own: the `droids` commands are looked at for what a
        // wave *after* the first does, and a tier-one site's one wave
        // is one landing and then nothing.
        if let Some(forced) = self.droid_waves_forced {
            return forced.max(1);
        }
        self.scaling().count(tier).max(1)
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
    /// alongside, in minutes of the mission clock (feature 103) — `None`
    /// while a machine is still standing (the clock does not run then),
    /// with none left to come, or away from a held station. The countdown
    /// the app shows.
    pub fn droid_wave_due(&self) -> Option<f64> {
        let id = self.residents.as_ref()?.station;
        let it = self.infestation(id)?;
        let now = self.run.mission_steps;
        it.next_wave
            .map(|due| due.saturating_sub(now) as f64 * data::STEP_MINUTES)
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
        // A held station's wave, or a defended town's (feature 94),
        // whose every wave **arrives** — there is no wave one already on
        // the ground — so its lander is drawn from the first.
        let (wave, from) = match self.infestation(station) {
            Some(it) => (it.wave, 2),
            None => (self.defense(station).map(|d| d.wave)?, 1),
        };
        if wave < from {
            return None;
        }
        let residents = self.residents.as_ref().filter(|r| r.station == station)?;
        let room = &residents.aboard.room;
        let alive = (0..room.droid_count() as usize)
            .filter_map(|i| room.droid(i))
            .any(|d| !d.destroyed && d.wave == wave);
        if !alive && !(self.is_manufacturer_held(station) && self.manufacturers_standing() > 0) {
            return None;
        }
        let site = self.station(station)?;
        let design = &site.design;
        if crate::surface::surface_body(station).is_some() {
            // A lander on the plain beyond the gate its wave walked in
            // by: the town's gates in turn (feature 112).
            let gate = crate::surface::gate_for_wave(&site.gates, wave)?;
            let out = data::DROID_LANDER_TILES * shipdesign::TILE as f64;
            let (at, outward) = gate.beyond(out);
            return Some((at, outward, true));
        }
        let port = droidplan::arrival_airlock_at(design, station, wave)?;
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
        let tier = self.droid_tier();
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
        // The Machine Heart's own are not a wave (feature 108): a conduit
        // standing must not hold the next wave off for ever.
        (0..room.droid_count() as usize)
            .filter(|&i| {
                room.droid(i)
                    .is_some_and(|d| !d.destroyed && !d.kind.is_structure())
            })
            .count() as u32
            // And the Manufacturers on their feet (feature 109): one downed
            // is out of the wave, and holds nothing up.
            + self.manufacturers_standing()
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
        let tier = self.droid_tier();
        let mut troopers = 0usize;
        let kinds = self
            .droid_kinds_forced
            .clone()
            .unwrap_or_else(|| bims::droid::wave_kinds(n, tier));
        // An elite's Guardian comes in its wave, whatever the tier.
        let elite = self
            .residents
            .as_ref()
            .is_some_and(|r| self.is_elite_here(r.station));
        let kinds = if elite {
            crate::elite::with_guardian(kinds, wave)
        } else {
            kinds
        };
        kinds
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
    /// a surface, just inside the gate its lander set down beyond — the
    /// town's gates in turn (`surface::gate_for_wave`).
    fn arriving_wave(&self, station: &Station, n: u32, wave: u32) -> Vec<bims::droid::Droid> {
        let Some((spots, facing)) = self.arrival_spots(station, n, wave) else {
            return Vec::new();
        };
        self.build_wave(n, wave, &spots, facing, station.map_seed)
    }

    /// Where a wave of `n` that **arrives** is stood, in the residents'
    /// room's own units, and which way it faces: round the spot just
    /// inside the airlock its ship tied up at, or inside the gate its
    /// lander set down beyond. The machines' reinforcements and the
    /// Manufacturers' (feature 109) land alike.
    pub(super) fn arrival_spots(
        &self,
        station: &Station,
        n: u32,
        wave: u32,
    ) -> Option<(Vec<bims::math::Vec2>, f32)> {
        let residents = self.residents.as_ref()?;
        let (at, facing) = if crate::surface::surface_body(station.id).is_some() {
            let gate = crate::surface::gate_for_wave(&station.gates, wave)?;
            let (spot, face) = (gate.spot(), gate.inward());
            (
                (spot.x, spot.y),
                bims::math::vec2(face.0 as f32, face.1 as f32).angle(),
            )
        } else {
            let Some(port) = droidplan::arrival_airlock_at(&station.design, station.id, wave)
            else {
                return None;
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
        // And onto deck they can walk off from: a ring round a spot in
        // a corridor is half in its walls, and a machine snapped out of
        // one could come out on the far side of it, shut in.
        let spots = residents.aboard.room.spread_wave(&spots);
        Some((spots, facing))
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
        // A site of the Manufacturers' lays its own (feature 109).
        if self.is_manufacturer_held(id) {
            self.lay_manufacturers();
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
        // The Machine Heart's own go on the deck first (feature 108), so
        // they keep the front of the list through every wave after.
        let mut droids = self.heart_machines_to_lay(&station);
        droids.extend(if wave == 1 {
            self.first_wave(&station, n, wave)
        } else {
            // A reinforcement was told where the crew are, and comes
            // looking for them rather than waiting at its airlock.
            let mut arriving = self.arriving_wave(&station, n, wave);
            for d in &mut arriving {
                d.seeking = true;
            }
            arriving
        });
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
            let waves = self.wave_count_here(id);
            if let Some(it) = self.infestation_mut(id) {
                it.settle(waves);
            }
            // And, at the Machine Heart's fortress, its own fight
            // settled beside the waves (feature 108).
            self.settle_heart_fight(id);
            self.settle_droids();
        }
        let standing = self.droids_standing();
        // The mission clock (feature 103): a wave is timed from the
        // arrival, whatever day it is.
        let now = self.run.mission_steps;
        let reinforce = self.reinforce_steps_here(id);
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
                    None => it.next_wave = Some(now + reinforce),
                    Some(due) if now >= due => {
                        it.waves_left -= 1;
                        it.wave += 1;
                        it.next_wave = None;
                        arrive = true;
                    }
                    Some(_) => {}
                }
            } else if !it.cleared && it.heart.is_none() {
                // The Machine Heart's fortress is cleared by its core and
                // nothing else (feature 108, `World::heart_step`): the
                // last wave spent there leaves the core to be fought.
                it.cleared = true;
                cleared = true;
            }
        }
        if arrive {
            // The room's old wrecks go with the wave that made them:
            // a fresh wave is a fresh deck — all but the Machine Heart's
            // own, which stay where they stand, shot down or not, at the
            // front of the list (feature 108).
            if let Some(residents) = &mut self.residents {
                let kept = residents.aboard.room.clear_wave_droids();
                // And so does what was remembered about them. The five
                // lists are one entry a **body**, and `visit` only ever
                // *grows* them — a wave landing makes the room bigger.
                // A wave cleared makes it smaller, and left as they were
                // every machine of the next wave was born already
                // flagged down: no `DroidDown` said for it when it was
                // destroyed, and no experience paid for it either.
                let bims = residents.aboard.room.crew_count() as usize + kept;
                residents.down.truncate(bims);
                residents.xp_down.truncate(bims);
                residents.last_hit_by.truncate(bims);
                residents.fee.truncate(bims);
                residents.medic.truncate(bims);
                residents.grave.truncate(bims);
                residents.defender.truncate(bims);
            }
            self.settle_droids();
            events.push(WorldEvent::DroidReinforcements { station: id });
        }
        if cleared {
            events.push(WorldEvent::DroidStationCleared { station: id });
        }
    }

    // --- defending a site (features 94 and 111) -------------------------------
    //
    // Every site that is neither a trader nor held by an enemy is
    // **threatened** from the first day (task 111; a town one hop outside
    // the infection, until then), and the first time the crew arrive at one
    // the machines come for it twenty seconds later. What makes it
    // different from every other fight in the game is that it happens
    // **inside one room**: the site's people, its defenders and the
    // machines are all in the residents' room, and the crew are in theirs.
    // So three target lists rather than two — the crew's (the machines
    // alone), the site's (the machines alone) and the machines' own (the
    // crew *and* the site's people) — and the hits between the two sides
    // in the residents' room never cross the seam at all.

    /// What a site of this system is to the crew (task 111): **exactly
    /// one** of attack, defence and trader. A trader
    /// ([`World::is_trader_here`]) is a trader; a site an enemy holds —
    /// the machines, the Manufacturers, or the Machine Heart in its
    /// fortress, or one of the machines' outposts (task 136, every other
    /// site of a system) — is an attack, cleared or not, and so is every
    /// site of a system the machines have; every other site, derelicts
    /// included, is a defence. Derived, never saved.
    pub fn site_kind(&self, station: u32) -> SiteKind {
        if self.is_trader_here(station) {
            return SiteKind::Trader;
        }
        // **A system the machines have is only attacks** (task 136): a
        // town held there, or a site the flip has not reached yet while the
        // crew are docked, is not somewhere to defend.
        if self.infested(self.star_id)
            || self.is_droid_held(station)
            || self.is_manufacturer_station(station)
            || heart::is_heart(station)
            || jammer::is_derived(station)
        {
            return SiteKind::Attack;
        }
        SiteKind::Defend
    }

    /// Whether the machines are **coming for** this site (task 111): a
    /// defence site ([`World::site_kind`]) whose fight has not been fought
    /// to an end — no [`Defense`] there won or lost — and not a town the
    /// crew held. **From the first day of a run**, wherever the crisis is:
    /// until task 111 only a friendly town one hop outside the infection
    /// was ever threatened. A station or a derelict defended and held is
    /// cleared and threatened no longer — until the crisis takes it, when
    /// it is an attack. Off for every site under the tests' dial
    /// ([`World::set_quiet_sites_for_probe`]).
    pub fn site_threatened(&self, station: u32) -> bool {
        if self.quiet_sites || self.site_kind(station) != SiteKind::Defend {
            return false;
        }
        if self.town_held(station) {
            return false;
        }
        self.defense(station).is_none_or(|d| !d.over())
    }

    /// The tests' dial (task 111): every site that is neither a trader nor
    /// held by an enemy a peaceful stop — no defence, no defenders — for
    /// a test whose subject is not the fight. The shared fixtures set it;
    /// a test of the defence takes it off again with `false`. Never set
    /// for `SURVIVORS`, the ship's `PINNED` and `PICTURES`, or the
    /// reference run: the game is what they pin.
    pub fn set_quiet_sites_for_probe(&mut self, quiet: bool) {
        // And no outposts (task 136): the ones laid here and not yet
        // fought over are nobody's again; the dial off lays them back.
        if quiet && !self.quiet_sites {
            self.drop_outposts();
        }
        self.quiet_sites = quiet;
        if !quiet {
            self.settle_outposts();
        }
        // A room already open was opened with the defenders the old
        // setting called for: opened again with the new one's.
        if let Some(id) = self.residents.as_ref().map(|r| r.station) {
            self.reopen_residents(id);
            self.apply_stances();
        }
    }

    /// How many armed **defenders** a site's room is opened with (task
    /// 111): none unless the machines are coming for it or its defence is
    /// still running, and otherwise [`defense::defenders`] of the day.
    pub fn defenders_of(&self, station: u32) -> u32 {
        let running = self.defense(station).is_some_and(|d| !d.over());
        if !running && !self.site_threatened(station) {
            return 0;
        }
        defense::defenders(self.days_gone())
    }

    /// How many defenders the room open on the site the crew are
    /// defending holds, standing or not: what the wave is sized against
    /// beside the players ([`World::droid_wave_size`]).
    pub fn defenders_fielded(&self) -> u32 {
        self.residents
            .as_ref()
            .map_or(0, |r| r.defender.iter().filter(|&&d| d).count() as u32)
    }

    /// Whether the crew have held this town: the last machine of the last
    /// wave destroyed. A held town stays friendly for good — the crisis
    /// never flips one — and goes on trading and hiring inside the
    /// infection.
    pub fn town_held(&self, station: u32) -> bool {
        self.held_towns.binary_search(&station).is_ok()
    }

    /// Every town the crew have held, in station order.
    pub fn held_towns(&self) -> &[u32] {
        &self.held_towns
    }

    /// The attack on that town, if there has ever been one.
    pub fn defense(&self, station: u32) -> Option<&Defense> {
        self.defenses.iter().find(|d| d.station == station)
    }

    fn defense_mut(&mut self, station: u32) -> Option<&mut Defense> {
        self.defenses.iter_mut().find(|d| d.station == station)
    }

    /// Every attack the world is keeping, in station order — the
    /// checksum's, and the save's.
    pub fn defenses(&self) -> &[Defense] {
        &self.defenses
    }

    /// The attack on the site the ship is **standing in**, still running:
    /// `None` away from one, at one that was never attacked, and at one
    /// whose fight is over either way. What the step and `visit` read to
    /// know the site's own fight is on — a station's or a derelict's as
    /// well as a town's since task 111.
    pub fn defense_here(&self) -> Option<&Defense> {
        let station = self.ship.state.station()?;
        if !self.aboard.is_joined() {
            return None;
        }
        self.defense(station).filter(|d| !d.over())
    }

    /// How long until the next wave lands on the town the crew are
    /// defending, in minutes of the world's clock — `None` while a
    /// machine is still standing, with none left to come, and away from
    /// an attack. What the app counts down along the top.
    pub fn defense_wave_due(&self) -> Option<f64> {
        self.defense_here()?
            .next_in
            .map(|left| left as f64 * data::STEP_MINUTES)
    }

    /// Which wave of machines is on the town's ground and how many are
    /// still to come after it — `None` away from an attack, and before
    /// the first wave has landed.
    pub fn defense_wave_standing(&self) -> Option<(u32, u32)> {
        let d = self.defense_here()?;
        (d.wave > 0).then_some((d.wave, d.waves_left))
    }

    /// How long after the crew land at a threatened town the first wave
    /// comes, in minutes of the mission clock (feature 103).
    pub fn defense_delay_minutes(&self) -> f64 {
        self.defense_delay as f64 * data::STEP_MINUTES
    }

    /// The same in steps of the mission clock, which is what it is kept in.
    pub fn defense_delay_steps(&self) -> u64 {
        self.defense_delay
    }

    /// The `defense` probe's dial: the wait before the first wave, in
    /// minutes of the mission clock, a minute being sixty steps.
    pub fn set_defense_delay_for_probe(&mut self, minutes: f64) {
        self.defense_delay = steps_of(minutes);
    }

    /// The attack's stage of the step, right after the machines' own.
    ///
    /// Everything about it waits for the crew: the first landing starts
    /// it, the count is fixed then, the clock runs only while the ship is
    /// on the pad, and taking off leaves it exactly where it stood.
    fn defense_waves(&mut self, events: &mut Vec<WorldEvent>) {
        let Some(id) = self
            .ship
            .state
            .station()
            .filter(|_| self.aboard.is_joined())
        else {
            return;
        };
        // The first arrival at a threatened site is what starts it, and
        // it starts once: a site attacked and left is attacked still.
        if self.defense(id).is_none() {
            if !self.site_threatened(id) {
                return;
            }
            let mut fresh = Defense::new(id);
            fresh.next_in = Some(self.defense_delay);
            let at = self.defenses.partition_point(|d| d.station < id);
            self.defenses.insert(at, fresh);
            // And the crew are put ashore, the moment the fight is on
            // (task 111): the prep time is for standing where they mean
            // to hold, not for walking off the ship.
            self.stand_the_crew_ashore(id);
        }
        if self.defense(id).is_some_and(|d| d.over()) {
            return;
        }
        // The count is worked out once, at that same first landing.
        if self.defense(id).is_some_and(|d| !d.settled) {
            let waves = self.droid_wave_count();
            if let Some(d) = self.defense_mut(id) {
                d.settle(waves);
            }
        }
        // **Every one of the town's people dead is the town lost**, wave
        // or no wave: there is nobody left to defend.
        if self.town_is_dead(id) {
            if let Some(d) = self.defense_mut(id) {
                d.lost = true;
            }
            // The town falls as any station does: the machines have it,
            // and `infest` reopens its room with them in it.
            self.infest(id);
            return;
        }
        // **A wave the crew left behind is put back where it stood.** A
        // town's room is built afresh at every landing, so the machines
        // on the ground have to be laid again — as many of them as were
        // still up, and not the wave at full strength, or a fight could
        // be won or lost by taking off and landing again.
        // **Before day ten the wave is the Manufacturers'** (task 131):
        // their people and the day's share of Troopers. Their people are
        // Bims, and a room can hold the site's dead besides, so a room
        // holds their wave when it says so (`manufacturers_laid`) rather
        // than when it has machines.
        let by_them = self.defense_by_manufacturers();
        let wave_now = self.defense(id).map_or(0, |d| d.wave);
        let fresh_room = self.residents.as_ref().is_some_and(|r| {
            r.station == id
                && if by_them {
                    r.manufacturers_laid != wave_now
                } else {
                    r.aboard.room.droid_count() == 0
                }
        });
        let left_standing = self.defense(id).map_or(0, |d| d.standing);
        if fresh_room && left_standing > 0 {
            if by_them {
                self.lay_defense_manufacturers(id, left_standing);
            } else {
                self.settle_defense_droids(id, left_standing);
            }
        }
        let standing = self.droids_standing();
        if let Some(d) = self.defense_mut(id) {
            d.standing = standing;
        }
        // Steps of the mission clock (feature 103), counted down one a
        // step while the crew are here.
        let reinforce = self.droid_reinforce;
        let mut arrive = false;
        let mut won = false;
        // **Not won until every wreck is counted** (task 111): `visit`
        // counts a machine down — its bounty, its experience — only while
        // the defence is running, and a win declared the step the last one
        // went down left that one uncounted and its bounty unpaid.
        // A Manufacturer is counted at its first down (task 131), the way
        // `experience` pays for one, and not only once it has bled out.
        let counted = self.residents.as_ref().is_none_or(|r| {
            let room = &r.aboard.room;
            let bims = room.crew_count() as usize;
            (0..room.droid_count() as usize).all(|i| r.down.get(bims + i).copied().unwrap_or(false))
                && (0..bims)
                    .filter(|&who| room.is_manufacturer(who))
                    .all(|who| r.xp_down.get(who).copied().unwrap_or(false))
        });
        if let Some(d) = self.defense_mut(id) {
            if standing > 0 {
                // A fight is on: the clock does not run.
                d.next_in = None;
            } else if d.wave == 0 || d.more_to_come() {
                match d.next_in {
                    None => d.next_in = Some(reinforce),
                    Some(left) if left <= 1 => {
                        if d.wave > 0 {
                            d.waves_left -= 1;
                        }
                        d.wave += 1;
                        d.next_in = None;
                        arrive = true;
                    }
                    Some(left) => d.next_in = Some(left - 1),
                }
            } else if !d.won && counted {
                d.won = true;
                won = true;
            }
        }
        if arrive {
            self.clear_wrecks();
            let n = self.droid_wave_size();
            if by_them {
                self.lay_defense_manufacturers(id, n);
            } else {
                self.settle_defense_droids(id, n);
            }
            if let Some(d) = self.defense_mut(id) {
                d.standing = n;
            }
            events.push(WorldEvent::DroidReinforcements { station: id });
        }
        if won {
            events.push(WorldEvent::TownHeld { station: id });
            // **A town held is held for good** (feature 94): friendly
            // whatever the crisis does, and some of its people join. A
            // station or a derelict held (task 111) is cleared and no
            // more — nobody joins, and the crisis may take it later, when
            // it is somewhere to attack.
            if surface::surface_body(id).is_some() {
                let at = self.held_towns.partition_point(|&s| s < id);
                self.held_towns.insert(at, id);
                self.townsfolk_join(id, events);
            }
        }
    }

    /// Every living crew member on their feet put **inside the site**, the
    /// step its defence starts (task 111): round the spot a little further
    /// in than the ashore point from the site's own airlock — the rings a
    /// wave arrives in (`arrival_spots`) — each snapped to a free cell of
    /// the joined deck. Nothing moves where the site has no airlock or
    /// the rooms are not joined.
    fn stand_the_crew_ashore(&mut self, id: u32) {
        let Some(port) = self.station(id).and_then(|s| s.port()) else {
            return;
        };
        let inside = droidplan::inside_of(&port, data::ASHORE_TILES + 1.0);
        let Some(middle) = self.aboard.from_station(dvec2(inside.0, inside.1)) else {
            return;
        };
        let t = shipdesign::TILE as f64;
        let room = &mut self.aboard.room;
        let mut placed = 0usize;
        for who in 0..room.crew_count() as usize {
            if !room.is_alive(who) || room.is_down(who) {
                continue;
            }
            let at = if placed == 0 {
                middle
            } else {
                let ring = ((placed - 1) / 6 + 1) as f64;
                let angle = ((placed - 1) % 6) as f64 / 6.0 * core::f64::consts::TAU;
                middle.add(dvec2(angle.cos(), angle.sin()).scale(ring * t * 1.5))
            };
            room.stand_at(who, bims::math::vec2(at.x as f32, at.y as f32));
            placed += 1;
        }
    }

    /// Whether every one of the site's **own** people is dead — the
    /// mercenaries are nobody's townsfolk and the defenders nobody's at
    /// all (task 111), so neither is counted. A site with none of its own
    /// — a derelict, defended by its defenders alone — is never dead, and
    /// a site whose room is not open answers false, since nothing is
    /// known about it.
    fn town_is_dead(&self, station: u32) -> bool {
        let Some(residents) = self.residents.as_ref().filter(|r| r.station == station) else {
            return false;
        };
        let bims = residents.aboard.room.crew_count() as usize;
        let mut own = (0..bims)
            .filter(|&who| residents.is_own(who))
            .filter(|&who| !residents.aboard.room.is_manufacturer(who))
            .peekable();
        if own.peek().is_none() {
            return false;
        }
        own.all(|who| !residents.aboard.room.is_alive(who))
    }

    /// The old wave's wrecks off the deck, and what the room remembered
    /// about them cut back to its Bims — the arrival half of
    /// [`World::droid_waves`], shared with the town's.
    fn clear_wrecks(&mut self) {
        let Some(residents) = &mut self.residents else {
            return;
        };
        residents.aboard.room.clear_droids();
        let bims = residents.aboard.room.crew_count() as usize;
        residents.down.truncate(bims);
        residents.xp_down.truncate(bims);
        residents.last_hit_by.truncate(bims);
        residents.fee.truncate(bims);
        residents.medic.truncate(bims);
        residents.grave.truncate(bims);
        residents.defender.truncate(bims);
    }

    /// A wave onto the town's ground: the droid step's own arrival, at
    /// the gate its lander set down beyond, the town's gates in turn
    /// (`surface::gate_for_wave`).
    fn settle_defense_droids(&mut self, id: u32, n: u32) {
        let Some(wave) = self.defense(id).map(|d| d.wave) else {
            return;
        };
        if wave == 0 || n == 0 {
            return;
        }
        let Some(station) = self.station(id).cloned() else {
            return;
        };
        let droids = self.arriving_wave(&station, n, wave);
        if let Some(residents) = &mut self.residents {
            residents
                .aboard
                .room
                .adopt_droids(droids, bims::math::Vec2::ZERO);
            residents.aboard.crew = residents.aboard.room.body_count();
        }
    }

    /// The town is held: the survivors who go with the crew.
    ///
    /// The larger of one and a fifth of the town's own people still
    /// alive, never more than the survivors other than the guard, taken
    /// **lowest index first and never the guard** — and each moved out
    /// of the residents' room into the crew's the way a hire is, with no
    /// contract, no wages and no bunk asked for: a classless crew bot
    /// like any other, which sleeps on the deck under the ordinary rules
    /// if there is no bunk spare. They keep what they carry and the hit
    /// points they have; nothing is issued.
    fn townsfolk_join(&mut self, station: u32, events: &mut Vec<WorldEvent>) {
        let Some(residents) = self.residents.as_ref().filter(|r| r.station == station) else {
            return;
        };
        let bims = residents.aboard.room.crew_count() as usize;
        let living: Vec<u32> = (0..bims)
            .filter(|&who| residents.is_own(who))
            // Nor one of the Manufacturers who attacked it (task 131), downed
            // and not yet bled out.
            .filter(|&who| !residents.aboard.room.is_manufacturer(who))
            .filter(|&who| residents.aboard.room.is_alive(who))
            .map(|who| who as u32)
            .collect();
        let guard_alive = living.first() == Some(&(surface::GUARD as u32));
        let want = defense::joiners(living.len() as u32, guard_alive);
        // Lowest index first, never the guard.
        let mut going: Vec<u32> = living
            .into_iter()
            .filter(|&who| who != surface::GUARD as u32)
            .take(want as usize)
            .collect();
        if going.is_empty() {
            return;
        }
        // Taken highest index first, so that removing one does not move
        // the index of the next.
        going.sort_unstable();
        let count = going.len() as u32;
        for who in going.into_iter().rev() {
            self.take_resident_aboard(who, false);
        }
        self.on_ship_changed();
        events.push(WorldEvent::TownsfolkJoined { count });
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
        self.change_class_kit(slot as usize, was, class);
        Ok(())
    }

    /// What a crew member carries as a class, changed from `was`'s to
    /// `class`'s: the old class's kit taken out and the new one's put in.
    fn change_class_kit(&mut self, who: usize, was: Class, class: Class) {
        match was {
            Class::None => {}
            Class::Engineer => self.take_engineer_kit(who),
            Class::Soldier => self.take_soldier_kit(who),
            // A medic carries nothing of its own since task 120.
            Class::Medic => {}
            Class::Tank => self.take_tank_kit(who),
            // A commander sets out with the laser pistol every Bim is
            // issued and nothing else, so there is nothing to take off.
            Class::Commander => {}
        }
        match class {
            Class::None => {}
            Class::Engineer => self.give_engineer_kit(who),
            Class::Soldier => self.give_soldier_kit(who),
            Class::Medic => {}
            Class::Tank => self.give_tank_kit(who),
            // And brings nothing of his own but the orders he gives.
            Class::Commander => {}
        }
    }

    /// The `end` command's crew (the app's): every crew member past the
    /// players a bot **of a class** — the classes of `Class::ALL` dealt
    /// down them in turn, so ten bots are two of each — at the top level
    /// with every rank bought, and the players' own Bims at the top level
    /// with every point still to spend; then everybody's kit at `tier`
    /// (`outfit_for_probe`), a bot that is no soldier with a gun of its
    /// own (`WeaponKind::ALL` dealt the same way) first, and a
    /// commander's Reinforcements brought for the ranks just bought.
    /// A class is a player slot's everywhere else, so `classes` is made
    /// as long as the crew here and nowhere else; `class_of` reads it by
    /// crew index, which is what makes the bots' classes count. The same
    /// on every machine of a lobby, which all call it with the same
    /// world. For the app.
    pub fn classed_crew_for_probe(&mut self, tier: Tier) {
        let players = self.players() as usize;
        let crew = self.aboard.crew_count() as usize;
        if self.classes.len() < crew {
            self.classes.resize(crew, Class::None);
        }
        let dealt: Vec<Class> = Class::ALL
            .into_iter()
            .filter(|c| *c != Class::None)
            .collect();
        for (n, who) in (players..crew).enumerate() {
            let class = dealt[n % dealt.len()];
            if class != Class::Soldier {
                let mut gear = self.aboard.room.gear(who);
                gear.weapon = Some(WeaponKind::ALL[n % WeaponKind::ALL.len()].basic());
                self.aboard.room.issue(who, gear);
            }
            let was = self.classes[who];
            self.classes[who] = class;
            self.change_class_kit(who, was, class);
        }
        let top = class::LEVEL_XP[class::LEVELS as usize - 1];
        let mut events = Vec::new();
        for who in 0..crew {
            self.award(who, top, &mut events);
        }
        for who in players..crew {
            self.set_ranks_for_probe(who as u32, [class::MAX_RANK; class::SLOTS]);
        }
        self.outfit_for_probe(tier);
        self.reinforce_for_probe();
    }

    /// The tank's start (feature 77): the laser pistol he has in hand
    /// already, and a fresh basic helm, kevlar and leg guards on where he
    /// wears nothing — his loadout from then on, ids off the holdings the
    /// way `outfit_for_probe` dresses a crew. Nothing of the armory's
    /// moves: the kit comes with him, like a soldier's rifle.
    fn give_tank_kit(&mut self, who: usize) {
        let mut gear = self.aboard.room.gear(who);
        for kind in ArmourKind::BASIC {
            if gear.worn(kind.slot()).is_some() {
                continue;
            }
            *gear.worn_mut(kind.slot()) = Some(self.holdings.new_piece(kind, Tier::One));
        }
        self.aboard.room.issue(who, gear);
    }

    /// And off again: every tier-one piece he wears, which is the tank's
    /// start — a class is chosen before the ship first leaves its berth,
    /// and a loadout changes only between missions (task 113), so
    /// nothing else can have put one on him by then.
    fn take_tank_kit(&mut self, who: usize) {
        let mut gear = self.aboard.room.gear(who);
        let mut changed = false;
        for kind in ArmourKind::BASIC {
            let slot = kind.slot();
            if gear.worn(slot).is_some_and(|p| p.tier == Tier::One) {
                *gear.worn_mut(slot) = None;
                changed = true;
            }
        }
        if changed {
            self.aboard.room.issue(who, gear);
        }
    }

    /// The engineer's start (task 127): the charges its ranks give it —
    /// none at rank nought — which is what the cooldowns fill back up to.
    /// Counters on the world, never things in a pack.
    fn give_engineer_kit(&mut self, who: usize) {
        for charge in [Charge::Sandbag, Charge::HealingSentry, Charge::Emp] {
            let n = self.charges(who as u32, charge);
            self.set_charges_held(who as u32, charge, n);
        }
    }

    /// And out again: every counter at nought, and the ultimate's
    /// cooldown forgotten.
    fn take_engineer_kit(&mut self, who: usize) {
        for charge in [Charge::Sandbag, Charge::HealingSentry, Charge::Emp] {
            self.set_charges_held(who as u32, charge, 0);
        }
        if let Some(e) = self.engineers.get_mut(who) {
            *e = crate::engineer::Engineer::default();
        }
    }

    /// **Exactly** `n` of one charge on every crew member, for a probe
    /// (features 88 and 90): the count set and that cooldown started
    /// afresh — so `n` of nought is a class with no charges and the full
    /// wait ahead of it, which is the one state a scripted run cannot
    /// walk itself into. `BIMS_KITS=n` and `BIMS_GRENADES=n`.
    pub fn set_charges_for_probe(&mut self, charge: Charge, n: u32) {
        let c = charge.code() as usize;
        for who in 0..self.aboard.crew_count() as usize {
            self.set_charges_held(who as u32, charge, n);
            if self.charge_timers.len() <= who {
                self.charge_timers.resize(who + 1, [None; Charge::CODES]);
            }
            self.charge_timers[who][c] = Some(self.mission_minutes());
        }
    }

    /// The engineer's three, all at `n`: `BIMS_KITS=n`.
    pub fn set_kits_for_probe(&mut self, n: u32) {
        for charge in [Charge::Sandbag, Charge::HealingSentry, Charge::Emp] {
            self.set_charges_for_probe(charge, n);
        }
    }

    /// The soldier's grenades at `n`, the same way: `BIMS_GRENADES=n`.
    pub fn set_grenades_for_probe(&mut self, n: u32) {
        self.set_charges_for_probe(Charge::Grenade, n);
    }

    /// A crew member's counter of a charge set outright (task 127): the
    /// list grown to reach it.
    pub(crate) fn set_charges_held(&mut self, who: u32, charge: Charge, n: u32) {
        let who = who as usize;
        if self.charges_held.len() <= who {
            self.charges_held.resize(who + 1, [0; Charge::CODES]);
        }
        self.charges_held[who][charge.code() as usize] = n;
    }

    /// The soldier's start (feature 75): a basic auto rifle in hand, the
    /// weapon that was there into the armory, and the grenades its rank
    /// gives ([`class::GRENADE_CHARGES`]) — which is what the cooldown
    /// fills back up to (feature 90).
    fn give_soldier_kit(&mut self, who: usize) {
        let mut gear = self.aboard.room.gear(who);
        let was = gear.weapon.replace(WeaponKind::AutoRifle.basic());
        self.aboard.room.issue(who, gear);
        if let Some(was) = was {
            self.holdings.put(Item::Weapon(was));
        }
        // As many as its Frag Grenade's rank fills to — none at rank
        // nought (task 124); a rank bought later puts them in hand.
        let n = self.charges(who as u32, Charge::Grenade);
        self.set_charges_held(who as u32, Charge::Grenade, n);
    }

    /// And out again: the grenades gone, and the rifle the class
    /// brought given up for a pistol out of the armory — a fresh one if
    /// the armory has none, the one that went in being what came out.
    fn take_soldier_kit(&mut self, who: usize) {
        self.set_charges_held(who as u32, Charge::Grenade, 0);
        let mut gear = self.aboard.room.gear(who);
        if gear.weapon == Some(WeaponKind::AutoRifle.basic()) {
            gear.weapon = Some(self.draw_pistol());
            self.aboard.room.issue(who, gear);
        }
    }

    /// A basic laser pistol out of the armory — a fresh one if the
    /// armory has none. What an empty hand is given.
    fn draw_pistol(&mut self) -> bims::combat::Weapon {
        let pistol = WeaponKind::LaserPistol.basic();
        if let Some(id) = self
            .holdings
            .armory
            .iter()
            .rev()
            .find(|s| s.item == Item::Weapon(pistol))
            .map(|s| s.id)
        {
            self.holdings.take(id);
        }
        pistol
    }

    /// Nobody goes into a fight empty-handed: while the crew are under
    /// arms in a mission, every living crew member with nothing in its
    /// weapon slot — the armory took it, or another Bim did — is given a
    /// basic pistol ([`World::draw_pistol`]). A bot with no weapon never
    /// reaches its stand, and stood aboard while the others fought.
    fn arm_the_empty_handed(&mut self, events: &mut Vec<WorldEvent>) {
        if !self.in_mission() || !self.aboard.room.is_mustered() {
            return;
        }
        for who in 0..self.aboard.crew_count() as usize {
            let mut gear = self.aboard.room.gear(who);
            if gear.weapon.is_some() || !self.aboard.room.is_alive(who) {
                continue;
            }
            gear.weapon = Some(self.draw_pistol());
            self.aboard.room.issue(who, gear);
            events.push(WorldEvent::GearChanged {
                who: who as u32,
                part: GearSlot::Weapon.code(),
            });
        }
    }

    /// A crew member's level (task 124; one table for every class since
    /// task 139): what every gate a level opens asks.
    pub fn level_of(&self, who: u32) -> u8 {
        self.progress_of(who).level()
    }

    /// The rank a crew member has bought of its ranked kit's ability
    /// slot (task 124, [`class::SLOT_Q`] …): nought for none, and for a
    /// class with no ranked kit.
    pub fn rank_of(&self, who: u32, ability_slot: u8) -> u8 {
        if !class::ranked(self.class_of(who)) {
            return 0;
        }
        self.progress_of(who).rank(ability_slot)
    }

    /// A crew member's skill points not spent (task 124): nought for a
    /// class with no ranked kit.
    pub fn points_of(&self, who: u32) -> u8 {
        self.progress_of(who).points(self.class_of(who))
    }

    /// Whether a rank of that slot may be bought for that player's own
    /// crew member now, and the rank it would be, or why not
    /// ([`Progress::can_rank_up`]). What the app draws the "+" by.
    pub fn can_rank_up(&self, slot: u32, ability_slot: u8) -> Result<u8, Refusal> {
        self.progress_of(slot)
            .can_rank_up(self.class_of(slot), ability_slot)
    }

    /// A rank bought — see [`Command::RankUp`]. A rank that lifts a
    /// charge's count — a grenade, an EMP, sandbags, a Healing Sentry —
    /// puts the new ones in hand at once: what the rank is bought for is
    /// to use it.
    fn rank_up(&mut self, slot: u32, ability_slot: u32, events: &mut Vec<WorldEvent>) {
        let class = self.class_of(slot);
        let ability_slot = u8::try_from(ability_slot).unwrap_or(u8::MAX);
        if slot >= self.players() || slot as usize >= self.progress.len() {
            events.push(refused(slot, Refusal::NoRankedKit));
            return;
        }
        let was = self.charges_by_rank(slot);
        match self.progress[slot as usize].rank_up(class, ability_slot) {
            Ok(rank) => {
                self.grant_what_ranks_added(slot, was);
                events.push(WorldEvent::RankedUp {
                    who: slot,
                    class: class.code(),
                    ability_slot: ability_slot as u32,
                    rank: rank as u32,
                });
            }
            Err(why) => events.push(refused(slot, why)),
        }
    }

    /// A ranked kit's four ranks set outright, for a probe (task 124,
    /// `BIMS_RANKS=q,c,e,r`): each capped at [`class::MAX_RANK`] and by the
    /// gates of the level the crew member is at, whatever the points say,
    /// and the charges the ranks give put in hand. Nothing
    /// for a class with no ranked kit.
    pub fn set_ranks_for_probe(&mut self, who: u32, ranks: [u8; class::SLOTS]) {
        let class = self.class_of(who);
        if !class::ranked(class) || who as usize >= self.progress.len() {
            return;
        }
        let level = self.level_of(who);
        let was = self.charges_by_rank(who);
        for (slot, &want) in ranks.iter().enumerate() {
            let most = (1..=class::MAX_RANK)
                .take_while(|&r| {
                    class::rank_level(class, slot as u8, r).is_some_and(|l| l <= level)
                })
                .last()
                .unwrap_or(0);
            self.progress[who as usize].ranks[slot] = want.min(most);
        }
        self.grant_what_ranks_added(who, was);
    }

    /// How many of each charge a crew member's ranks give it now, by code.
    fn charges_by_rank(&self, who: u32) -> [u32; Charge::CODES] {
        let mut out = [0; Charge::CODES];
        for charge in Charge::ALL {
            out[charge.code() as usize] = self.charges(who, charge);
        }
        out
    }

    /// Every charge a rank lifted since `was` (`charges_by_rank`) put in
    /// the crew member's hand at once.
    fn grant_what_ranks_added(&mut self, who: u32, was: [u32; Charge::CODES]) {
        for charge in Charge::ALL {
            let now = self.charges(who, charge);
            let before = was[charge.code() as usize];
            if now > before {
                self.grant_charges(who, charge, now - before);
            }
        }
    }

    /// `n` of a charge put in a crew member's hand at once: the counter
    /// raised, never past its charges.
    fn grant_charges(&mut self, who: u32, charge: Charge, n: u32) {
        let held = self.charges_of(who, charge);
        let most = self.charges(who, charge);
        self.set_charges_held(who, charge, (held + n).min(most.max(held)));
    }

    /// `xp` to one crew member: every level it reaches said. Public for
    /// the probes and the tests; the game gives experience through the
    /// step alone.
    pub fn award(&mut self, who: usize, xp: u32, events: &mut Vec<WorldEvent>) {
        if self.class_of(who as u32) == Class::None || who >= self.progress.len() {
            return;
        }
        let class = self.class_of(who as u32);
        for level in self.progress[who].gain(xp) {
            events.push(WorldEvent::LevelUp {
                who: who as u32,
                class: class.code(),
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

    /// After `visit`: every enemy that went down or died this step, once
    /// each, to every classed crew member within the vicinity of where it
    /// lies on the joined deck. Only the crew's enemies
    /// ([`World::first_enemy_body`]) — a hostile room's bodies, which are
    /// the machines since every human is friendly (feature 104), and the
    /// machines in a town the crew are defending, whose own people going
    /// down are nobody's experience — and only while the rooms are
    /// joined: on an unjoined deck nobody is in anybody's vicinity. A
    /// crewmate or a hire going down is nobody's experience either.
    /// Hands back every enemy that went down this step with who last hit
    /// it.
    fn experience(&mut self, events: &mut Vec<WorldEvent>) -> Vec<(usize, Option<usize>)> {
        let mut downed = Vec::new();
        let Some(first) = self.first_enemy_body() else {
            return downed;
        };
        let Some(residents) = &self.residents else {
            return downed;
        };
        let mut gained: Vec<(bims::math::Vec2, u32)> = Vec::new();
        let mut bounty: Money = 0;
        // Every enemy counted this step and its worth, for the numbers the
        // app floats over it.
        let mut rewarded: Vec<(u32, Money)> = Vec::new();
        let station = residents.station;
        let count = residents.aboard.count() as usize;
        for who in 0..count.min(residents.xp_down.len()) {
            let room = &residents.aboard.room;
            // Past `first`, and a Manufacturer come with a wave to a site
            // the crew defend (task 131), which is a Bim of the site's room.
            if who < first && !room.is_manufacturer(who) {
                continue;
            }
            let dead = !room.is_alive(who);
            let down = dead || room.is_downed(who);
            let Some(at) = self
                .aboard
                .from_station(residents.aboard.position(who as u32))
            else {
                continue;
            };
            let at = bims::math::vec2(at.x as f32, at.y as f32);
            // **The only experience there is, and every class's alike**
            // (task 119): an enemy going down — downed, or dead without
            // being down first, which is every machine — is
            // [`class::XP_ENEMY_DOWN`], once. A downed Manufacturer
            // dying after — bled out or finished — is worth nothing more.
            if down && !residents.xp_down[who] {
                gained.push((at, self.rewards.xp_per_down));
                downed.push((who, residents.last_hit_by.get(who).copied().flatten()));
                // The Republic's bounty (feature 95), once per enemy at
                // the first down or death, whoever did it. A machine is
                // worth nothing: the Republic pays for people.
                let crew = residents.aboard.room.crew_count() as usize;
                let worth = if who < crew {
                    manufacturer_bounty(&self.rewards, room, who)
                } else {
                    room.droid(who - crew).map_or(0, |d| {
                        bounty_share(
                            self.rewards.bounty_for(d.tier.code()),
                            droid_bounty_percent(d.kind),
                        )
                    })
                };
                if who < crew {
                    bounty = bounty.saturating_add(worth);
                }
                rewarded.push((who as u32, self.bounty_here(worth)));
            }
        }
        if let Some(residents) = &mut self.residents {
            for who in 0..count.min(residents.xp_down.len()) {
                let room = &residents.aboard.room;
                residents.xp_down[who] |= !room.is_alive(who) || room.is_downed(who);
            }
        }
        for (at, xp) in gained {
            self.award_classed_near(at, xp, events);
        }
        for (who, money) in rewarded {
            events.push(WorldEvent::EnemyRewarded {
                station,
                who,
                xp: self.rewards.xp_per_down,
                money,
            });
        }
        // And what the Republic owes for them, said once however many
        // went down this step — paid at once where there is nothing left
        // to clear, and pending until the site is cleared where there is
        // (feature 103).
        self.earn_bounty(bounty, events);
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

    /// The engineer's sentry's minigun for its owner (task 127): at the
    /// tier its R rank gives it, [`class::SENTRY_TIER`] — tier two at the
    /// first rank.
    pub(crate) fn sentry_weapon(&self, owner: u32) -> Weapon {
        let rank = self.rank_of(owner, class::SLOT_R).max(1);
        let tier = class::by_rank(class::SENTRY_TIER, rank).unwrap_or(Tier::Two);
        WeaponKind::Minigun.at(tier)
    }

    /// What a deployable that stands in the enemies' sights carries, for
    /// their nearest-target rule (a blade locks a gunner): the sentry's
    /// minigun, and the pistol for a Healing Sentry, which has no barrel
    /// and is no blade.
    fn deployable_weapon(&self, d: &Deployable) -> Weapon {
        match d.kind {
            DeployKind::Sentry => self.sentry_weapon(d.owner_slot),
            DeployKind::HealingSentry | DeployKind::Sandbags => WeaponKind::LaserPistol.basic(),
        }
    }

    /// What the sentry's rank does to its shooting (task 127): the fire
    /// rate multiplied by [`class::SENTRY_FIRE_RATE`] and the tiles
    /// [`class::SENTRY_RANGE`] adds to its reach; the room applies both
    /// through the one `fire_as` a Bim's skill goes through.
    pub(crate) fn sentry_skill(&self, owner: u32) -> bims::combat::Skill {
        let mut skill = bims::combat::Skill::NONE;
        let rank = self.rank_of(owner, class::SLOT_R).max(1);
        skill.fire_rate = class::by_rank(class::SENTRY_FIRE_RATE, rank).unwrap_or(1.0);
        skill.range = class::by_rank(class::SENTRY_RANGE, rank).unwrap_or(0.0);
        skill
    }

    /// A fresh deployable's health for its owner: its kind's table at the
    /// rank the owner has of its slot (task 127) — the first rank's for
    /// one laid at none, which only a probe does.
    pub fn laid_health(&self, kind: DeployKind, owner: u32) -> f32 {
        let (table, slot) = match kind {
            DeployKind::Sandbags => (class::SANDBAG_HEALTH, class::SLOT_E),
            DeployKind::HealingSentry => (class::HEALING_SENTRY_HEALTH, class::SLOT_C),
            DeployKind::Sentry => (class::SENTRY_HEALTH, class::SLOT_R),
        };
        class::by_rank(table, self.rank_of(owner, slot).max(1)).unwrap_or(table[0])
    }

    /// **Charges** a crew member has of one thing its class spends
    /// (features 88 and 90, task 127): how many of it its counter fills
    /// back up to, one at a time on [`World::charge_cooldown`] — the
    /// table of the ability's rank ([`Charge::slot`]). Nought for anybody
    /// of another class, and nought at rank nought: a charge nothing can
    /// spend does not come back.
    ///
    /// For a Healing Sentry it is also the **standing limit**: one more
    /// laid destroys that engineer's oldest.
    pub fn charges(&self, who: u32, charge: Charge) -> u32 {
        if self.class_of(who) != charge.class() {
            return 0;
        }
        let table = match charge {
            Charge::Sandbag => class::SANDBAG_CHARGES,
            Charge::HealingSentry => class::HEALING_SENTRY_CHARGES,
            Charge::Emp => class::EMP_CHARGES,
            // Frag Grenade's rank (task 124): none before the first.
            Charge::Grenade => class::GRENADE_CHARGES,
        };
        class::by_rank(table, self.rank_of(who, charge.slot())).unwrap_or(0)
    }

    /// Seconds of the clock one spent charge takes to come back — its
    /// rank's (tasks 124 and 127) — and a relic's factor on it.
    pub fn charge_cooldown(&self, who: u32, charge: Charge) -> f64 {
        let table = match charge {
            Charge::Sandbag => class::SANDBAG_COOLDOWN,
            Charge::HealingSentry => class::HEALING_SENTRY_COOLDOWN,
            Charge::Emp => class::EMP_COOLDOWN,
            Charge::Grenade => class::GRENADE_COOLDOWN,
        };
        let rank = self.rank_of(who, charge.slot()).max(1);
        let own = class::by_rank(table, rank).unwrap_or(0.0);
        // A relic's *Coolant Loop* (feature 106).
        own * self.relic_factor(who, crate::relic::Stat::Cooldowns)
    }

    /// Seconds of the clock until the next charge comes back to that crew
    /// member; nought when it already holds its charges.
    pub fn charge_cooldown_left(&self, who: u32, charge: Charge) -> f64 {
        let Some(began) = self
            .charge_timers
            .get(who as usize)
            .and_then(|t| t[charge.code() as usize])
        else {
            return 0.0;
        };
        // Seconds of the room's clock: a game minute is a real second at
        // 1× (`time::MINUTES_PER_SECOND`), the way the room steps.
        let since = (self.mission_minutes() - began) / time::MINUTES_PER_SECOND;
        (self.charge_cooldown(who, charge) - since).max(0.0)
    }

    /// How many of a charge a crew member holds now: its counter (task
    /// 127). What the boxes at the foot of the screen count (feature 80).
    pub fn charges_of(&self, who: u32, charge: Charge) -> u32 {
        self.charges_held
            .get(who as usize)
            .map_or(0, |held| held[charge.code() as usize])
    }

    /// How many of a kind an engineer has standing, anywhere.
    pub fn laid_of(&self, owner: u32, kind: DeployKind) -> u32 {
        self.deployables
            .iter()
            .filter(|d| d.kind == kind && d.owner_slot == owner)
            .count() as u32
    }

    /// Every class's counters raised back up to their [`World::charges`]
    /// on the kinds' cooldowns (features 88 and 90, task 127), a step of
    /// the mission clock at a time: a charge's cooldown runs whenever the
    /// counter is short, and when it runs out it goes up by one. In combat
    /// as out of it, and nothing is conjured out of the hold: the charge
    /// **is** the ability, and no class makes or buys one.
    fn restock_charges(&mut self) {
        let crew = self.aboard.crew_count() as usize;
        if self.charge_timers.len() < crew {
            self.charge_timers.resize(crew, [None; Charge::CODES]);
        }
        if self.charges_held.len() < crew {
            self.charges_held.resize(crew, [0; Charge::CODES]);
        }
        let now = self.mission_minutes();
        for who in 0..crew {
            for charge in Charge::ALL {
                let c = charge.code() as usize;
                let charges = self.charges(who as u32, charge);
                let held = self.charges_of(who as u32, charge);
                if held >= charges || !self.aboard.room.is_alive(who) {
                    self.charge_timers[who][c] = None;
                    continue;
                }
                let Some(began) = self.charge_timers[who][c] else {
                    self.charge_timers[who][c] = Some(now);
                    continue;
                };
                let since = (now - began) / time::MINUTES_PER_SECOND;
                if since < self.charge_cooldown(who as u32, charge) {
                    continue;
                }
                // The charge is up: the counter one higher.
                self.charges_held[who][c] = held + 1;
                self.charge_timers[who][c] = (held + 1 < charges).then_some(now);
            }
        }
    }

    /// Whether the tile `tile` of the crew's room takes a deployable laid
    /// by `slot`: reachable deck floor, not a door or an airlock, nothing
    /// blocking on it and nothing laid there.
    fn deploy_tile(&self, slot: u32, tile: (i32, i32)) -> Result<bims::math::Vec2, Refusal> {
        let t = shipdesign::TILE as f32;
        let at = bims::math::vec2((tile.0 as f32 + 0.5) * t, (tile.1 as f32 + 0.5) * t);
        if !self.aboard.room.deploy_tile_ok(slot as usize, at)
            || self.deployable_under(at).is_some()
        {
            return Err(Refusal::CantDeployThere);
        }
        Ok(at)
    }

    /// What a deploy of sandbags or a Healing Sentry asks, before the
    /// errand: the slot's Bim fit to act (`OutOfReach`), an engineer
    /// (`NotAnEngineer`), the ability at rank one (`NotLearnt`), a charge
    /// (`NoKit`), and the tile free to take it (`CantDeployThere`). The
    /// sentry is asked [`World::can_lay_sentry`]. What the app greys a
    /// press out with, and [`Command::Deploy`]'s own check.
    pub fn can_deploy(&self, slot: u32, kind: DeployKind, tile: (i32, i32)) -> Result<(), Refusal> {
        let Some(charge) = kind.charge() else {
            return self.can_lay_sentry(slot, tile);
        };
        if !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        if !self.is_engineer(slot) {
            return Err(Refusal::NotAnEngineer);
        }
        if self.rank_of(slot, charge.slot()) == 0 {
            return Err(Refusal::NotLearnt);
        }
        if self.charges_of(slot, charge) == 0 {
            return Err(Refusal::NoKit);
        }
        self.deploy_tile(slot, tile).map(|_| ())
    }

    /// The deploy begun: the walk and the work are the room's
    /// (`Game::deploy`); the charge is spent when the work is done
    /// (`finish_deploy`). See [`Command::Deploy`].
    fn deploy(&mut self, slot: u32, kind: DeployKind, tile: (i32, i32)) -> Result<(), Refusal> {
        self.can_deploy(slot, kind, tile)?;
        self.start_laying(slot, kind, tile)
    }

    /// The errand handed to the room, the checks made.
    fn start_laying(
        &mut self,
        slot: u32,
        kind: DeployKind,
        tile: (i32, i32),
    ) -> Result<(), Refusal> {
        let at = self.deploy_tile(slot, tile)?;
        let minutes = self.deploy_minutes(slot, kind);
        let steady = kind == DeployKind::Sentry;
        if !self
            .aboard
            .room
            .deploy(slot as usize, at, kind.code(), steady, minutes as f32)
        {
            return Err(Refusal::CantDeployThere);
        }
        Ok(())
    }

    /// How long laying one takes an engineer, in game minutes of working
    /// steps: its kind's at the rank (task 127).
    pub fn deploy_minutes(&self, slot: u32, kind: DeployKind) -> f64 {
        match kind {
            DeployKind::Sandbags => {
                let rank = self.rank_of(slot, class::SLOT_E).max(1);
                class::by_rank(class::SANDBAG_MINUTES, rank).unwrap_or(0.0)
            }
            DeployKind::HealingSentry => {
                let rank = self.rank_of(slot, class::SLOT_C).max(1);
                class::by_rank(class::HEALING_SENTRY_MINUTES, rank).unwrap_or(0.0)
            }
            DeployKind::Sentry => class::SENTRY_MINUTES,
        }
    }

    // --- the engineer's ultimate: the sentry (task 127) ----------------------

    /// Seconds of the mission clock from one sentry laid to the next: the
    /// R rank's [`class::SENTRY_COOLDOWN`], and a relic's factor on it.
    pub fn sentry_cooldown(&self, who: u32) -> f64 {
        let rank = self.rank_of(who, class::SLOT_R).max(1);
        class::by_rank(class::SENTRY_COOLDOWN, rank).unwrap_or(0.0)
            * self.relic_factor(who, crate::relic::Stat::Cooldowns)
    }

    /// Seconds of the mission clock until that engineer may lay its
    /// sentry again; nought when it is ready — at every mission's start.
    pub fn sentry_cooldown_left(&self, who: u32) -> f64 {
        let Some(laid) = self.engineers.get(who as usize).and_then(|e| e.sentry_laid) else {
            return 0.0;
        };
        let since = (self.mission_minutes() - laid) / time::MINUTES_PER_SECOND;
        (self.sentry_cooldown(who) - since).max(0.0)
    }

    /// Whether that engineer's sentry stands. It stands until it is
    /// destroyed, another is laid, the mission ends or the rooms unjoin —
    /// there is no timer.
    pub fn sentry_standing(&self, who: u32) -> bool {
        self.deployables
            .iter()
            .any(|d| d.kind == DeployKind::Sentry && d.owner_slot == who)
    }

    /// What laying the sentry asks: the slot's Bim fit to act
    /// (`OutOfReach`), an engineer (`NotAnEngineer`), the ultimate at rank
    /// one (`NotLearnt`), its cooldown run out (`CoolingDown`), and the
    /// tile free to take it (`CantDeployThere`). See [`Command::Sentry`].
    pub fn can_lay_sentry(&self, slot: u32, tile: (i32, i32)) -> Result<(), Refusal> {
        if !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        if !self.is_engineer(slot) {
            return Err(Refusal::NotAnEngineer);
        }
        if self.rank_of(slot, class::SLOT_R) == 0 {
            return Err(Refusal::NotLearnt);
        }
        if self.sentry_cooldown_left(slot) > 0.0 {
            return Err(Refusal::CoolingDown);
        }
        self.deploy_tile(slot, tile).map(|_| ())
    }

    /// The sentry's laying begun — see [`Command::Sentry`].
    fn lay_sentry(&mut self, slot: u32, tile: (i32, i32)) -> Result<(), Refusal> {
        self.can_lay_sentry(slot, tile)?;
        self.start_laying(slot, DeployKind::Sentry, tile)
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

    /// The work done: the room said `who` laid a deployable of kind `code`
    /// at `at`. It goes down now — if the Bim is still an engineer, the
    /// tile still free and, for sandbags and a Healing Sentry, a charge
    /// still held, which is spent; for the sentry, if it is still ready.
    /// Nothing laid is anybody's experience (task 119).
    ///
    /// From [`class::SANDBAG_DOUBLE_RANK`] one charge of sandbags lays a
    /// second tile on the first free neighbour, north, east, south, west.
    /// A Healing Sentry laid with as many of that engineer's standing as
    /// it has charges **destroys its oldest**; the sentry, one standing at
    /// a time, destroys the one before and starts its cooldown.
    fn finish_deploy(
        &mut self,
        who: usize,
        at: bims::math::Vec2,
        code: u32,
        events: &mut Vec<WorldEvent>,
    ) {
        let slot = who as u32;
        let Some(kind) = DeployKind::from_code(code) else {
            return;
        };
        if !self.is_engineer(slot) {
            return;
        }
        let (deck, tile) = self.deck_of(at);
        if self.deployable_at(deck, tile).is_some() {
            return;
        }
        match kind.charge() {
            Some(charge) => {
                let held = self.charges_of(slot, charge);
                if held == 0 {
                    return;
                }
                self.set_charges_held(slot, charge, held - 1);
            }
            None => {
                if self.sentry_cooldown_left(slot) > 0.0 {
                    return;
                }
                let now = self.mission_minutes();
                if self.engineers.len() <= who {
                    self.engineers
                        .resize(who + 1, crate::engineer::Engineer::default());
                }
                self.engineers[who].sentry_laid = Some(now);
            }
        }
        // The standing limit: one sentry, and a Healing Sentry's charges.
        let limit = match kind {
            DeployKind::Sentry => 1,
            DeployKind::HealingSentry => self.charges(slot, Charge::HealingSentry).max(1),
            DeployKind::Sandbags => u32::MAX,
        };
        while self.laid_of(slot, kind) >= limit {
            let Some(oldest) = self
                .deployables
                .iter()
                .filter(|d| d.kind == kind && d.owner_slot == slot)
                .map(|d| d.id)
                .min()
            else {
                break;
            };
            self.deployables.retain(|d| d.id != oldest);
            events.push(WorldEvent::DeployableLost { kind: kind.code() });
        }
        self.lay(kind, slot, deck, tile);
        if kind == DeployKind::Sandbags
            && self.rank_of(slot, class::SLOT_E) >= class::SANDBAG_DOUBLE_RANK
        {
            let t = shipdesign::TILE as f32;
            let beside = [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)]
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
            kind: kind.code(),
        });
        self.sync_deployed_cover();
        self.hand_the_room_the_sentries();
    }

    /// One deployable down, fresh, for `owner`.
    fn lay(&mut self, kind: DeployKind, owner: u32, deck: Deck, tile: (u32, u32)) {
        let id = self.next_deployable;
        self.next_deployable += 1;
        let health = self.laid_health(kind, owner);
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

    /// Sandbags or a Healing Sentry taken back up — see
    /// [`Command::PackUp`]: the charge back, capped at the engineer's
    /// charges. The sentry is never taken up.
    fn pack_up(&mut self, slot: u32, id: u32, events: &mut Vec<WorldEvent>) {
        let d = match self.deployable_in_reach(slot, id) {
            Ok(d) => d,
            Err(why) => {
                events.push(refused(slot, why));
                return;
            }
        };
        let Some(charge) = d.kind.charge() else {
            events.push(refused(slot, Refusal::NoSuchDeployable));
            return;
        };
        self.grant_charges(slot, charge, 1);
        self.deployables.retain(|x| x.id != id);
        events.push(WorldEvent::PackedUp {
            who: slot,
            kind: d.kind.code(),
        });
        self.sync_deployed_cover();
        self.hand_the_room_the_sentries();
    }

    /// Before the rooms step: the sandbags laid as cover on both rooms,
    /// and the sentries on the crew's deck.
    fn hand_the_room_the_engineers(&mut self) {
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

    /// The sentries on the crew's deck, as they stand now, to the room:
    /// the engineer's sentry, which fires, and every Healing Sentry,
    /// which does not — both a body the enemies aim at.
    fn hand_the_room_the_sentries(&mut self) {
        let sentries: Vec<Sentry> = self
            .deployables
            .iter()
            .filter(|d| d.kind.is_sentry())
            .filter_map(|d| {
                let at = self.deployable_room_pos(d)?;
                Some(Sentry {
                    id: d.id,
                    at,
                    weapon: self.deployable_weapon(d),
                    skill: self.sentry_skill(d.owner_slot),
                    heals: d.kind == DeployKind::HealingSentry,
                    trigger: bims::combat::Trigger::default(),
                    facing: 0.0,
                    flash: 0.0,
                })
            })
            .collect();
        self.aboard.room.set_sentries(sentries);
    }

    /// Every Healing Sentry's heal this step (task 127): which crew body
    /// each reaches — within its rank's radius, in its sight from its tile
    /// (walls and shut doors stop it), on its feet and below its full bar
    /// — and at how many hit points an hour, a share of the medic's beam.
    /// `(sentry id, crew index, rate)`, in id order; what the room draws
    /// its lines by, and what [`World::heal_by_sentries`] heals by.
    pub fn healing_links(&self) -> Vec<(u32, u32, f32)> {
        let room = &self.aboard.room;
        let crew = self.aboard.crew_count();
        let t = shipdesign::TILE as f32;
        let mut links = Vec::new();
        for d in self
            .deployables
            .iter()
            .filter(|d| d.kind == DeployKind::HealingSentry)
        {
            let Some(at) = self.deployable_room_pos(d) else {
                continue;
            };
            let rank = self.rank_of(d.owner_slot, class::SLOT_C).max(1);
            let radius = class::by_rank(class::HEALING_SENTRY_RADIUS, rank).unwrap_or(0.0) * t;
            let rate = class::by_rank(class::HEALING_SENTRY_RATE, rank).unwrap_or(0.0)
                * class::HEAL_BEAM_HP;
            for who in 0..crew {
                let w = who as usize;
                if !room.is_alive(w)
                    || room.is_downed(w)
                    || room.is_outside(w)
                    || room.health(w) >= bims::health::MAX_HEALTH
                {
                    continue;
                }
                let p = room.bim_pos(w);
                if (p - at).len() > radius || !room.line_clear(at, p) {
                    continue;
                }
                links.push((d.id, who, rate));
            }
        }
        links
    }

    /// The Healing Sentries' heal, a step's worth (task 127): every crew
    /// body reached healed at the **highest** rate reaching it — several
    /// never stack — through `Game::heal`, never past its full bar.
    fn heal_by_sentries(&mut self) {
        if !self
            .deployables
            .iter()
            .any(|d| d.kind == DeployKind::HealingSentry)
        {
            return;
        }
        let crew = self.aboard.crew_count() as usize;
        let mut best = vec![0.0_f32; crew];
        for (_, who, rate) in self.healing_links() {
            let slot = &mut best[who as usize];
            *slot = slot.max(rate);
        }
        // An hour of the clock is sixty minutes, a step `STEP_MINUTES` of
        // them — the beam's arithmetic.
        let share = (data::STEP_MINUTES / 60.0) as f32;
        for (who, rate) in best.into_iter().enumerate() {
            if rate > 0.0 {
                // Times the Healing Aura where the body stands (task 130).
                self.heal_crew(who as u32, rate * share);
            }
        }
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
    /// for it, said. Nothing comes back: a Healing Sentry's charge returns
    /// on its cooldown like any other, and the sentry's cooldown runs on.
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
    /// unjoin, and so is the sentry wherever it stands: called from
    /// `unjoin_rooms`.
    fn drop_station_deployables(&mut self) {
        self.deployables
            .retain(|d| d.deck == Deck::Ship && d.kind != DeployKind::Sentry);
    }

    // --- the engineer's EMP (task 127) ---------------------------------------

    /// How far an EMP's burst reaches, in tiles: the Q rank's
    /// [`class::EMP_RADIUS`], nought before the first.
    pub fn emp_radius(&self, who: u32) -> f32 {
        class::by_rank(class::EMP_RADIUS, self.rank_of(who, class::SLOT_Q)).unwrap_or(0.0)
    }

    /// Seconds an EMP stuns for: the Q rank's [`class::EMP_STUN`].
    pub fn emp_stun(&self, who: u32) -> f32 {
        class::by_rank(class::EMP_STUN, self.rank_of(who, class::SLOT_Q)).unwrap_or(0.0)
            * self.ability_length_factor(who) as f32
    }

    /// What an EMP throw asks, in the order the refusals are said: the
    /// slot's Bim fit to act (`OutOfReach`), an engineer
    /// (`NotAnEngineer`), a rank of EMP (`NotLearnt`), a charge (`NoKit`),
    /// and the tile — deck of the room (`CantThrowThere`) within the
    /// grenade's range (`OutOfThrowRange`) with nothing opaque between
    /// (`NoLineToTile`). See [`Command::Emp`].
    pub fn can_throw_emp(&self, slot: u32, tile: (i32, i32)) -> Result<(), Refusal> {
        if !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        if !class::can(self.class_of(slot), class::Ability::Emp) {
            return Err(Refusal::NotAnEngineer);
        }
        if self.rank_of(slot, class::SLOT_Q) == 0 {
            return Err(Refusal::NotLearnt);
        }
        if self.charges_of(slot, Charge::Emp) == 0 {
            return Err(Refusal::NoKit);
        }
        let t = shipdesign::TILE as f32;
        let at = bims::math::vec2((tile.0 as f32 + 0.5) * t, (tile.1 as f32 + 0.5) * t);
        if !self.aboard.room.is_deck_tile(at) {
            return Err(Refusal::CantThrowThere);
        }
        let from = self.aboard.room.bim_pos(slot as usize);
        if (at - from).len() > class::GRENADE_RANGE * t {
            return Err(Refusal::OutOfThrowRange);
        }
        if !self.aboard.room.line_clear(from, at) {
            return Err(Refusal::NoLineToTile);
        }
        Ok(())
    }

    /// The EMP thrown — see [`Command::Emp`]: the charge spent now, and
    /// the room throws it with the grenade's fuse and the rank's radius,
    /// stun and, at the top rank, the exposure.
    fn throw_emp(&mut self, slot: u32, tile: (i32, i32)) -> Result<(), Refusal> {
        self.can_throw_emp(slot, tile)?;
        let held = self.charges_of(slot, Charge::Emp);
        self.set_charges_held(slot, Charge::Emp, held - 1);
        let t = shipdesign::TILE as f32;
        let at = bims::math::vec2((tile.0 as f32 + 0.5) * t, (tile.1 as f32 + 0.5) * t);
        let expose = self.rank_of(slot, class::SLOT_Q) >= class::EMP_EXPOSE_RANK;
        let (radius, stun) = (self.emp_radius(slot) * t, self.emp_stun(slot));
        self.aboard
            .room
            .throw_emp(slot as usize, at, class::GRENADE_FUSE, radius, stun, expose);
        Ok(())
    }

    /// The EMPs that burst in the crew's room this step, carried to the
    /// machines they reached (task 127): a target past the residents'
    /// Bims is a machine of their room, stunned there — every kind that
    /// walks, never the Machine Heart's (`Droid::stun` refuses one), and
    /// never an enemy Bim or the crew's own sentries, which are no target.
    fn settle_stuns(&mut self) {
        let stuns = self.aboard.room.take_stuns();
        if stuns.is_empty() {
            return;
        }
        let Some(residents) = self.residents.as_mut() else {
            return;
        };
        let room = &mut residents.aboard.room;
        let bims = room.crew_count() as usize;
        for (target, seconds, expose) in stuns {
            if let Some(i) = target.checked_sub(bims) {
                room.stun_droid(i, seconds, expose);
            }
        }
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
        // How fast a worn piece drains: a tank's half, a quarter from
        // Plated's fourth rank, and one for everybody else.
        skill.armour_drain = self.armour_drain(who);
        // And so are a commander's aura, his Battle Cry and his Rally
        // (task 129): they lift whatever the crew member's own class gave
        // it, a player's own steered Bim included.
        self.lift_by_commanders(who, &mut skill);
        // And a player's relics (feature 106), last: a share on top of
        // whatever the class and the aura made of it.
        self.lift_by_relics(who, &mut skill);
        // And task 118's, which read the crew round it as well.
        self.lift_by_relic_hooks(who, &mut skill);
        // And a medic's cloak on it (task 130): no fire, and a quicker
        // walk, whoever cast it.
        self.lift_by_cloak(who, &mut skill);
        // And how long it takes to revive a downed crewmate (task 120).
        skill.revive = self.revive_seconds(who);
        // And whether it is a medic, whom the other bots leave a downed
        // crewmate to (task 125).
        skill.medic = self.is_medic(who) || self.is_field_medic(who);
        skill
    }

    /// How long crew member `who` takes to revive a downed crewmate, in
    /// seconds (task 120): `bims::health::REVIVE_SECONDS` (ten) for
    /// anybody, [`class::MEDIC_REVIVE_SECONDS`] (four) for a medic — of the
    /// class, or a hired field medic — and a *Trauma Kit* on it
    /// [`data::TRAUMA_KIT_REVIVE_SECONDS`] quicker again, never under
    /// [`data::REVIVE_FLOOR_SECONDS`].
    pub fn revive_seconds(&self, who: u32) -> f32 {
        let medic = self.is_medic(who) || self.is_field_medic(who);
        let quicker = crate::relic::rule_of(self.relics_of(who), |r| match r {
            crate::relic::Rule::QuickRevive { seconds } => Some(seconds),
            _ => None,
        })
        .unwrap_or(0.0);
        class::revive_time(medic, quicker)
    }

    /// The soldier's half of [`World::skill_of`] (task 124), off its four
    /// ranks and nothing else; `Skill::NONE` for anybody else. **Weak
    /// Spot** is the crit chance on every weapon hit; **Brace**, while
    /// braced, cuts the misses by its share (and far aim equals near at
    /// the fourth rank) and the damage taken; **Rampage**, while it runs,
    /// the fire rate up, the damage taken down and full aim on the move.
    fn soldier_skill(&self, who: u32) -> bims::combat::Skill {
        let mut skill = bims::combat::Skill::NONE;
        if !self.is_soldier(who) {
            return skill;
        }
        if let Some(chance) =
            class::by_rank(class::WEAK_SPOT_CHANCE, self.rank_of(who, class::SLOT_C))
        {
            skill.crit_chance = chance;
        }
        let brace = self.rank_of(who, class::SLOT_E);
        if brace > 0 && self.aboard.room.is_braced(who as usize) {
            skill.miss_cut = class::by_rank(class::BRACE_MISS_CUT, brace).unwrap_or(0.0);
            skill.damage_taken *= class::by_rank(class::BRACE_DAMAGE_TAKEN, brace).unwrap_or(1.0);
            skill.deadeye = brace >= class::BRACE_DEADEYE_RANK;
        }
        if self.is_rampaging(who) {
            let rank = self.rank_of(who, class::SLOT_R);
            skill.fire_rate *= class::by_rank(class::RAMPAGE_FIRE_RATE, rank).unwrap_or(1.0);
            skill.damage_taken *= class::by_rank(class::RAMPAGE_DAMAGE_TAKEN, rank).unwrap_or(1.0);
            skill.walking = 1.0;
        }
        skill
    }

    /// Whether a player's soldier may brace, or why not: a soldier
    /// (`NotASoldier`), fit to act (`OutOfReach`), and a rank of Brace
    /// (`NotLearnt`, task 124). What the app greys the key with and
    /// [`Command::Brace`] asks.
    pub fn can_brace(&self, slot: u32) -> Result<(), Refusal> {
        if !class::can(self.class_of(slot), class::Ability::Brace) {
            return Err(Refusal::NotASoldier);
        }
        if !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        if self.rank_of(slot, class::SLOT_E) == 0 {
            return Err(Refusal::NotLearnt);
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

    /// How far a soldier throws, in tiles: [`class::GRENADE_RANGE`] at
    /// every rank.
    pub fn grenade_range(&self, _who: u32) -> f32 {
        class::GRENADE_RANGE
    }

    /// Seconds from the throw to the burst: [`class::GRENADE_FUSE`] at
    /// every rank.
    pub fn grenade_fuse(&self, _who: u32) -> f32 {
        class::GRENADE_FUSE
    }

    /// How far a burst reaches, in tiles: Frag Grenade's rank's
    /// ([`class::GRENADE_RADIUS`]), nought before the first.
    pub fn grenade_radius(&self, who: u32) -> f32 {
        class::by_rank(class::GRENADE_RADIUS, self.rank_of(who, class::SLOT_Q)).unwrap_or(0.0)
    }

    /// What a burst does at its centre: Frag Grenade's rank's
    /// ([`class::GRENADE_DAMAGE`]), nought before the first.
    pub fn grenade_damage(&self, who: u32) -> f32 {
        class::by_rank(class::GRENADE_DAMAGE, self.rank_of(who, class::SLOT_Q)).unwrap_or(0.0)
    }

    /// Seconds of the clock one spent grenade charge takes to come back
    /// into the pack (feature 90): its rank's cooldown.
    pub fn grenade_cooldown(&self, who: u32) -> f64 {
        self.charge_cooldown(who, Charge::Grenade)
    }

    /// Seconds of the clock until the soldier's next grenade lands in
    /// its pack; nought when it is already at its charges.
    pub fn grenade_cooldown_left(&self, who: u32) -> f64 {
        self.charge_cooldown_left(who, Charge::Grenade)
    }

    /// Grenades in a crew member's pack: its charges in hand.
    pub fn grenades_of(&self, who: u32) -> u32 {
        self.charges_of(who, Charge::Grenade)
    }

    /// What a throw asks, in the order the refusals are said: the slot's
    /// Bim fit to act (`OutOfReach`), a soldier (`NotASoldier`), a rank
    /// of Frag Grenade (`NoGrenadesYet`, task 124), a grenade in the pack
    /// (`NoGrenade` — the charge **is** the cooldown since feature 90, so
    /// the charges may go one after the other), and the tile —
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
        if self.rank_of(slot, class::SLOT_Q) == 0 {
            return Err(Refusal::NoGrenadesYet);
        }
        if self.grenades_of(slot) == 0 {
            return Err(Refusal::NoGrenade);
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

    /// The throw — see [`Command::Throw`]: the grenade's charge spent
    /// now, and the room throws it with the fuse, the radius and the
    /// damage the soldier's rank gives it. Nothing is noted down: the
    /// charge is gone, so `restock_charges` starts its cooldown the next
    /// step, the way a laid deployable's starts (feature 90).
    fn throw(&mut self, slot: u32, tile: (i32, i32)) -> Result<(), Refusal> {
        self.can_throw(slot, tile)?;
        let who = slot as usize;
        let held = self.charges_of(slot, Charge::Grenade);
        if held == 0 {
            return Err(Refusal::NoGrenade);
        }
        self.set_charges_held(slot, Charge::Grenade, held - 1);
        let t = shipdesign::TILE as f32;
        let at = bims::math::vec2((tile.0 as f32 + 0.5) * t, (tile.1 as f32 + 0.5) * t);
        let fuse = self.grenade_fuse(slot);
        let radius = self.grenade_radius(slot) * t;
        let damage = self.grenade_damage(slot);
        self.aboard
            .room
            .throw_grenade(who, at, fuse, radius, damage);
        Ok(())
    }

    // --- a throw walked out to (the ability range indicators) -------------

    /// How far a throw reaches, in tiles: the grenade's, or the EMP's,
    /// which is the grenade's own.
    pub fn throw_range(&self, who: u32, emp: bool) -> f32 {
        if emp {
            class::GRENADE_RANGE
        } else {
            self.grenade_range(who)
        }
    }

    /// Whether `slot` could throw at `tile` from where it stands now:
    /// [`World::can_throw`] or [`World::can_throw_emp`].
    pub fn can_throw_now(&self, slot: u32, emp: bool, tile: (i32, i32)) -> Result<(), Refusal> {
        if emp {
            self.can_throw_emp(slot, tile)
        } else {
            self.can_throw(slot, tile)
        }
    }

    /// The throw made now, with its event and the relics told of an
    /// ability used.
    fn throw_now(
        &mut self,
        slot: u32,
        emp: bool,
        tile: (i32, i32),
        events: &mut Vec<WorldEvent>,
    ) -> Result<(), Refusal> {
        if emp {
            self.throw_emp(slot, tile)?;
            events.push(WorldEvent::EmpThrown { who: slot });
        } else {
            self.throw(slot, tile)?;
            events.push(WorldEvent::Thrown { who: slot });
        }
        self.relic_trigger(slot, crate::relic::Trigger::AbilityUse, events);
        Ok(())
    }

    /// [`Command::ThrowAt`]: thrown now where it can be; out of reach or
    /// behind a wall, the Bim walked to [`World::throw_stand`] and the
    /// throw kept on [`World::throws`] until it can be made. Whatever it
    /// was walking out to throw before is called off either way.
    fn throw_at(
        &mut self,
        slot: u32,
        emp: bool,
        tile: (i32, i32),
        events: &mut Vec<WorldEvent>,
    ) -> Result<(), Refusal> {
        self.throws.retain(|p| p.who != slot);
        match self.can_throw_now(slot, emp, tile) {
            Ok(()) => self.throw_now(slot, emp, tile, events),
            Err(Refusal::OutOfThrowRange | Refusal::NoLineToTile) => {
                let Some(stand) = self.throw_stand(slot, emp, tile) else {
                    return Err(Refusal::NoLineToTile);
                };
                if !self.aboard.room.walk_to(slot as usize, tile_centre(stand)) {
                    return Err(Refusal::NoLineToTile);
                }
                let at = self.throws.partition_point(|p| p.who < slot);
                self.throws.insert(
                    at,
                    PendingThrow {
                        who: slot,
                        emp,
                        tile,
                        stand,
                    },
                );
                Ok(())
            }
            Err(why) => Err(why),
        }
    }

    /// Where `who` could throw at `tile` from: the deck tile nearest the
    /// Bim whose middle is within reach of the tile — a body's width short
    /// of it, since a walk ends on the free cell nearest the spot and not
    /// on its middle — with a clear line to it, and that the Bim can walk
    /// to. The candidates are every such tile, taken nearest first, a tie
    /// by row then column, so every machine picks the same. `None` for a
    /// tile nobody could throw at from anywhere near.
    pub fn throw_stand(&self, who: u32, emp: bool, tile: (i32, i32)) -> Option<(i32, i32)> {
        /// How many of the nearest candidates are asked whether the Bim
        /// can walk there: a route search each.
        const TRIED: usize = 24;
        let room = &self.aboard.room;
        let t = shipdesign::TILE as f32;
        let reach = (self.throw_range(who, emp) - THROW_STAND_MARGIN) * t;
        let target = tile_centre(tile);
        if !room.is_deck_tile(target) {
            return None;
        }
        let from = room.bim_pos(who as usize);
        let span = (reach / t).ceil() as i32;
        let mut found: Vec<(f32, i32, i32)> = Vec::new();
        for dy in -span..=span {
            for dx in -span..=span {
                let stand = (tile.0 + dx, tile.1 + dy);
                let p = tile_centre(stand);
                if (p - target).len() > reach
                    || !room.is_deck_tile(p)
                    || !room.line_clear(p, target)
                {
                    continue;
                }
                found.push(((p - from).len(), stand.1, stand.0));
            }
        }
        found.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        found
            .into_iter()
            .take(TRIED)
            .map(|(_, y, x)| (x, y))
            .find(|&stand| room.reaches(who as usize, tile_centre(stand)))
    }

    /// Every throw walked out to, once the rooms have stepped: made the
    /// step it can be; called off for a Bim no longer fit to act, and
    /// for one that got where it was going and still cannot — said, like
    /// a refusal — or that cannot for another reason (its last charge
    /// spent, its rank gone).
    fn settle_throws(&mut self, events: &mut Vec<WorldEvent>) {
        if self.throws.is_empty() {
            return;
        }
        for pending in std::mem::take(&mut self.throws) {
            let who = pending.who;
            if !self.fit_to_act(who) {
                continue;
            }
            match self.can_throw_now(who, pending.emp, pending.tile) {
                Ok(()) => {
                    if let Err(why) = self.throw_now(who, pending.emp, pending.tile, events) {
                        events.push(refused(who, why));
                    }
                }
                Err(why @ (Refusal::OutOfThrowRange | Refusal::NoLineToTile)) => {
                    if self.aboard.room.has_arrived(who as usize) {
                        events.push(refused(who, why));
                    } else {
                        self.throws.push(pending);
                    }
                }
                Err(why) => events.push(refused(who, why)),
            }
        }
    }

    // --- the soldier's Rampage (task 124) ----------------------------------

    /// Weak Spot's stream as the world opened with it (task 124): what
    /// the checksum compares against, so a stream never drawn on is
    /// hashed nowhere.
    pub(crate) fn fresh_crit_rng(&self) -> bims::rng::Rng {
        bims::rng::Rng::new(self.galaxy_seed ^ CRIT_SALT)
    }

    /// A crew member's soldier state — an empty one for anybody the world
    /// keeps none for.
    pub fn soldier_of(&self, who: u32) -> crate::soldier::Soldier {
        self.soldiers.get(who as usize).cloned().unwrap_or_default()
    }

    /// The soldier's state, made if the crew grew past the list.
    fn soldier_mut(&mut self, who: usize) -> &mut crate::soldier::Soldier {
        if self.soldiers.len() <= who {
            self.soldiers
                .resize(who + 1, crate::soldier::Soldier::default());
        }
        &mut self.soldiers[who]
    }

    /// Whether a crew member's Rampage is running.
    pub fn is_rampaging(&self, who: u32) -> bool {
        self.is_soldier(who) && self.soldier_of(who).rampaging(self.mission_minutes())
    }

    /// Seconds of the mission clock the Rampage running has left; nought
    /// with none.
    pub fn rampage_left(&self, who: u32) -> f64 {
        if !self.is_rampaging(who) {
            return 0.0;
        }
        (self.soldier_of(who).until - self.mission_minutes()) / time::MINUTES_PER_SECOND
    }

    /// Seconds of the mission clock a Rampage runs at the soldier's rank,
    /// before any extension ([`class::RAMPAGE_SECONDS`]).
    pub fn rampage_seconds(&self, who: u32) -> f64 {
        class::by_rank(class::RAMPAGE_SECONDS, self.rank_of(who, class::SLOT_R)).unwrap_or(0.0)
            * self.ability_length_factor(who)
    }

    /// Seconds of the mission clock from one Rampage to the next:
    /// [`class::RAMPAGE_COOLDOWN`] of its rank, shorter with a relic's
    /// *Coolant Loop* as every class cooldown is.
    pub fn rampage_cooldown(&self, who: u32) -> f64 {
        let rank = self.rank_of(who, class::SLOT_R).max(1);
        class::by_rank(class::RAMPAGE_COOLDOWN, rank).unwrap_or(0.0)
            * self.relic_factor(who, crate::relic::Stat::Cooldowns)
    }

    /// Seconds of the mission clock until a soldier may go on a Rampage
    /// again; nought when it may.
    pub fn rampage_cooldown_left(&self, who: u32) -> f64 {
        let Some(began) = self.soldier_of(who).began else {
            return 0.0;
        };
        let since = (self.mission_minutes() - began) / time::MINUTES_PER_SECOND;
        (self.rampage_cooldown(who) - since).max(0.0)
    }

    /// Whether a player's soldier may go on a Rampage, or why not, in
    /// order: a soldier (`NotASoldier`), fit to act — downed among it —
    /// (`OutOfReach`), a rank of Rampage (`NotLearnt`), none running
    /// (`AlreadyActive`) and out of the cooldown (`CoolingDown`). It may
    /// go on one braced: the two stack.
    pub fn can_rampage(&self, slot: u32) -> Result<(), Refusal> {
        if !class::can(self.class_of(slot), class::Ability::Rampage) {
            return Err(Refusal::NotASoldier);
        }
        if !self.fit_to_act(slot) || self.aboard.room.is_down(slot as usize) {
            return Err(Refusal::OutOfReach);
        }
        if self.rank_of(slot, class::SLOT_R) == 0 {
            return Err(Refusal::NotLearnt);
        }
        if self.is_rampaging(slot) {
            return Err(Refusal::AlreadyActive);
        }
        if self.rampage_cooldown_left(slot) > 0.0 {
            return Err(Refusal::CoolingDown);
        }
        Ok(())
    }

    /// The Rampage — see [`Command::Rampage`]: the mission clock noted,
    /// and when it ends. What it *does* is read off that every step, in
    /// `soldier_skill`.
    fn rampage(&mut self, slot: u32) -> Result<(), Refusal> {
        self.can_rampage(slot)?;
        let now = self.mission_minutes();
        let until = now + self.rampage_seconds(slot) * time::MINUTES_PER_SECOND;
        let soldier = self.soldier_mut(slot as usize);
        soldier.began = Some(now);
        soldier.until = until;
        soldier.extended = 0.0;
        Ok(())
    }

    /// A machine downed by crew member `who` (credited the way *Kill
    /// Relay* credits a kill): at Rampage's fourth rank, the one running
    /// is lengthened by [`class::RAMPAGE_EXTEND_SECONDS`], up to
    /// [`class::RAMPAGE_EXTEND_MAX`] a use.
    pub(crate) fn rampage_kill(&mut self, who: u32) {
        if !self.is_rampaging(who) || self.rank_of(who, class::SLOT_R) < class::RAMPAGE_EXTEND_RANK
        {
            return;
        }
        let soldier = self.soldier_mut(who as usize);
        let more = class::RAMPAGE_EXTEND_SECONDS.min(class::RAMPAGE_EXTEND_MAX - soldier.extended);
        if more > 0.0 {
            soldier.extended += more;
            soldier.until += more * time::MINUTES_PER_SECOND;
        }
    }

    /// What Weak Spot adds to one of the crew's hits read back from the
    /// room (task 124): a hit the room marked critical — rolled there off
    /// [`World::crit_rng`], lent for the step — has the weapon's flat
    /// damage times the shooter's crit damage less one added, **after**
    /// every relic and ability factor has had its say and before the
    /// armour: what this is, for one hit, and nought for a hit not
    /// critical. Asked where a hit lands on a machine, after the relics'
    /// machine hooks (`land_on_machines`), and in `visit` for a hit on a
    /// Bim, which no relic multiplies.
    pub(crate) fn crit_extra(&self, hit: &bims::combat::Hit) -> f32 {
        let Some(by) = hit.by.filter(|_| hit.crit) else {
            return 0.0;
        };
        // Slot C is Weak Spot on a soldier alone: a commander's is his
        // aura (task 129).
        if !self.is_soldier(by as u32) {
            return 0.0;
        }
        let rank = self.rank_of(by as u32, class::SLOT_C);
        class::by_rank(class::WEAK_SPOT_DAMAGE, rank)
            .map_or(0.0, |crit| crate::soldier::crit_bonus(hit.flat, crit))
    }

    // --- the medic: a ranked kit (task 130) -------------------------------
    //
    // `crate::medic` is the state; this is the rules. The beam is a list
    // of crew indices on the medic, checked every step before the rooms
    // step and healed off here (`Game::heal`); the Nanite Burst and the
    // Cloak are a mission minute each on the medic, read the soldier's
    // Rampage's way; a cloak is a mission minute on the crew member it
    // covers; the Healing Aura is worked out afresh whenever a heal is
    // given (`heal_factor`). There is no surge.

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

    /// What a medic shoots with (task 130): its fire held while the beam
    /// is linked, or from the beam's third rank ([`class::HEAL_BEAM_FIRE_RANK`])
    /// at [`class::HEAL_BEAM_FIRE_RATE`]; `Skill::NONE` unlinked. And, whatever
    /// he is doing, a crewmate he revives gets up at
    /// [`class::MEDIC_REVIVED_TO`] of its bar.
    fn medic_skill(&self, who: u32) -> bims::combat::Skill {
        let mut skill = bims::combat::Skill::NONE;
        skill.revived_to = class::MEDIC_REVIVED_TO;
        if self.is_beaming(who) {
            if self.rank_of(who, class::SLOT_E) >= class::HEAL_BEAM_FIRE_RANK {
                skill.fire_rate *= class::HEAL_BEAM_FIRE_RATE;
            } else {
                skill.holds_fire = true;
            }
        }
        skill
    }

    /// The beam's rank, read as its first where none is bought: what the
    /// readings below take, so a box can say what the first rank would do.
    fn beam_rank(&self, who: u32) -> u8 {
        self.rank_of(who, class::SLOT_E).max(1)
    }

    /// How far a medic's beam reaches, in tiles, at his rank
    /// ([`class::HEAL_BEAM_RANGES`]).
    pub fn beam_range(&self, who: u32) -> f32 {
        class::by_rank(class::HEAL_BEAM_RANGES, self.beam_rank(who))
            .unwrap_or(class::HEAL_BEAM_RANGE)
    }

    /// Hit points a beamed patient gains an hour of the clock before the
    /// Healing Aura: [`class::HEAL_BEAM_HP`] times his rank's
    /// [`class::HEAL_BEAM_RATE`].
    pub fn beam_rate(&self, who: u32) -> f32 {
        class::HEAL_BEAM_HP
            * class::by_rank(class::HEAL_BEAM_RATE, self.beam_rank(who)).unwrap_or(1.0)
    }

    /// How many patients a medic's beam holds at once, at his rank
    /// ([`class::HEAL_BEAM_PATIENTS`]), each at the full rate.
    pub fn beam_patients(&self, who: u32) -> usize {
        class::by_rank(class::HEAL_BEAM_PATIENTS, self.beam_rank(who)).unwrap_or(1)
    }

    /// Whether a crew member is where a medic's beam reaches it: alive,
    /// in the room, within the medic's range and in its sight — or the
    /// medic itself (task 120: a medic may beam its own bar). What a link
    /// asks, and what keeps one.
    fn beam_reaches(&self, medic: u32, patient: u32) -> Result<(), Refusal> {
        let room = &self.aboard.room;
        let (m, p) = (medic as usize, patient as usize);
        if patient >= self.aboard.crew_count() || !room.is_alive(p) {
            return Err(Refusal::NotACrewmate);
        }
        if room.is_outside(p) || room.is_outside(m) {
            return Err(Refusal::OutOfBeamRange);
        }
        if patient == medic {
            return Ok(());
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
    /// fit to act (`OutOfReach`), a rank of the beam (`NotLearnt`), a
    /// living crew member — itself too (`NotACrewmate`) — in the room and
    /// within range (`OutOfBeamRange`), in its sight (`NoSightOfPatient`).
    /// What the app greys the key with and [`Command::Beam`] asks.
    pub fn can_beam(&self, slot: u32, patient: u32) -> Result<(), Refusal> {
        if !class::can(self.class_of(slot), class::Ability::Beam) {
            return Err(Refusal::NotAMedic);
        }
        if !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        if self.rank_of(slot, class::SLOT_E) == 0 {
            return Err(Refusal::NotLearnt);
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

    // The Nanite Burst (Q).

    /// Hit points a medic's Nanite Burst puts back into each Bim it
    /// reaches, at his rank, before the Healing Aura
    /// ([`class::NANITE_BURST_HEAL`]); nought before the first.
    pub fn nanite_burst_heal(&self, who: u32) -> f32 {
        class::by_rank(class::NANITE_BURST_HEAL, self.rank_of(who, class::SLOT_Q)).unwrap_or(0.0)
    }

    /// How far a medic's Nanite Burst reaches, in tiles, at his rank
    /// ([`class::NANITE_BURST_RADIUS`]); nought before the first.
    pub fn nanite_burst_radius(&self, who: u32) -> f32 {
        class::by_rank(class::NANITE_BURST_RADIUS, self.rank_of(who, class::SLOT_Q)).unwrap_or(0.0)
    }

    /// Seconds of the mission clock between one burst and the next:
    /// [`class::NANITE_BURST_COOLDOWN`] of his rank (the first's before
    /// one), times the cooldown relics.
    pub fn nanite_burst_cooldown(&self, who: u32) -> f64 {
        let rank = self.rank_of(who, class::SLOT_Q).max(1);
        class::by_rank(class::NANITE_BURST_COOLDOWN, rank).unwrap_or(0.0)
            * self.relic_factor(who, crate::relic::Stat::Cooldowns)
    }

    /// Seconds of the mission clock until he may burst again; nought when
    /// he may.
    pub fn nanite_burst_cooldown_left(&self, who: u32) -> f64 {
        let Some(began) = self.medic_of(who).last_burst else {
            return 0.0;
        };
        let since = (self.mission_minutes() - began) / time::MINUTES_PER_SECOND;
        (self.nanite_burst_cooldown(who) - since).max(0.0)
    }

    /// Seconds of the mission clock since his last burst, for the ring the
    /// app draws spreading out to its radius; `None` with none this
    /// mission.
    pub fn nanite_burst_since(&self, who: u32) -> Option<f64> {
        let began = self.medic_of(who).last_burst?;
        Some((self.mission_minutes() - began) / time::MINUTES_PER_SECOND)
    }

    /// Whom a medic's Nanite Burst set off now would heal: every crew
    /// member alive, on its feet and on the deck within his radius and in
    /// his sight — walls block — himself included, lowest index first.
    pub fn nanite_burst_reaching(&self, slot: u32) -> Vec<u32> {
        if !self.on_the_deck(slot) {
            return Vec::new();
        }
        let room = &self.aboard.room;
        let at = room.bim_pos(slot as usize);
        let reach = self.nanite_burst_radius(slot) * shipdesign::TILE as f32;
        (0..self.aboard.crew_count())
            .filter(|&who| {
                let p = room.bim_pos(who as usize);
                self.on_the_deck(who)
                    && !room.is_downed(who as usize)
                    && (p - at).len() <= reach
                    && (who == slot || room.sees(slot as usize, p))
            })
            .collect()
    }

    /// Whether a player's medic may set off a Nanite Burst, or why not, in
    /// the Rally's order: a medic (`NotAMedic`), fit to act — downed among
    /// it — (`OutOfReach`), a rank (`NotLearnt`) and out of the cooldown
    /// (`CoolingDown`).
    pub fn can_nanite_burst(&self, slot: u32) -> Result<(), Refusal> {
        if !class::can(self.class_of(slot), class::Ability::NaniteBurst) {
            return Err(Refusal::NotAMedic);
        }
        if !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        if self.rank_of(slot, class::SLOT_Q) == 0 {
            return Err(Refusal::NotLearnt);
        }
        if self.nanite_burst_cooldown_left(slot) > 0.0 {
            return Err(Refusal::CoolingDown);
        }
        Ok(())
    }

    /// The Nanite Burst — see [`Command::NaniteBurst`]: the mission clock
    /// noted and every Bim it reaches healed at once, each by the burst
    /// times its own Healing Aura. How many it reached.
    fn nanite_burst(&mut self, slot: u32) -> Result<u32, Refusal> {
        self.can_nanite_burst(slot)?;
        let heal = self.nanite_burst_heal(slot);
        let reached: Vec<(u32, f32)> = self
            .nanite_burst_reaching(slot)
            .into_iter()
            .map(|who| (who, heal * self.heal_factor(who)))
            .collect();
        for &(who, points) in &reached {
            self.aboard.room.heal(who as usize, points);
        }
        let now = self.mission_minutes();
        self.medic_mut(slot as usize).last_burst = Some(now);
        Ok(reached.len() as u32)
    }

    // The Healing Aura (C).

    /// How far a medic's Healing Aura reaches, in tiles, at his rank
    /// ([`class::HEALING_AURA_RADIUS`]); nought before the first and for
    /// anybody not a medic.
    pub fn healing_aura_radius(&self, who: u32) -> f32 {
        if !self.is_medic(who) {
            return 0.0;
        }
        class::by_rank(class::HEALING_AURA_RADIUS, self.rank_of(who, class::SLOT_C)).unwrap_or(0.0)
    }

    /// The medic whose Healing Aura covers a crew member where it stands,
    /// if any: a medic on his feet and on the deck — itself included —
    /// within his radius. Two reaching one Bim: the higher factor holds
    /// it, and they never stack.
    pub fn healing_aura_reaching(&self, who: u32) -> Option<u32> {
        if !self.on_the_deck(who) {
            return None;
        }
        let room = &self.aboard.room;
        let at = room.bim_pos(who as usize);
        let t = shipdesign::TILE as f32;
        (0..self.aboard.crew_count())
            .filter(|&m| {
                let radius = self.healing_aura_radius(m);
                radius > 0.0
                    && self.on_the_deck(m)
                    && !room.is_downed(m as usize)
                    && (room.bim_pos(m as usize) - at).len() <= radius * t
            })
            .max_by(|&a, &b| {
                let of = |m: u32| {
                    class::by_rank(class::HEALING_AURA_FACTOR, self.rank_of(m, class::SLOT_C))
                        .unwrap_or(1.0)
                };
                of(a).total_cmp(&of(b))
            })
    }

    /// What every heal crew member `who` takes is multiplied by (task
    /// 130): the strongest Healing Aura reaching **where it stands**,
    /// wherever the heal comes from — the beam, the burst, a Healing
    /// Sentry, a relic — and one where none does. Every call the world
    /// makes to `Game::heal` goes through it; a revive is not a heal and
    /// never asks.
    pub fn heal_factor(&self, who: u32) -> f32 {
        self.healing_aura_reaching(who)
            .and_then(|m| {
                class::by_rank(class::HEALING_AURA_FACTOR, self.rank_of(m, class::SLOT_C))
            })
            .unwrap_or(1.0)
    }

    /// `points` of health put into crew member `who`, times its Healing
    /// Aura (`heal_factor`): what every heal of the world's goes through.
    /// How much went in.
    pub(crate) fn heal_crew(&mut self, who: u32, points: f32) -> f32 {
        let points = points * self.heal_factor(who);
        self.aboard.room.heal(who as usize, points)
    }

    // The Cloak (R).

    /// Seconds of the mission clock a cloak a medic casts lasts, at his
    /// rank ([`class::CLOAK_SECONDS`]); nought before the first.
    pub fn cloak_seconds(&self, medic: u32) -> f64 {
        class::by_rank(class::CLOAK_SECONDS, self.rank_of(medic, class::SLOT_R)).unwrap_or(0.0)
            * self.ability_length_factor(medic)
    }

    /// Seconds of the mission clock between one cloak and the next:
    /// [`class::CLOAK_COOLDOWN`] of his rank (the first's before one),
    /// times the cooldown relics.
    pub fn cloak_cooldown(&self, medic: u32) -> f64 {
        let rank = self.rank_of(medic, class::SLOT_R).max(1);
        class::by_rank(class::CLOAK_COOLDOWN, rank).unwrap_or(0.0)
            * self.relic_factor(medic, crate::relic::Stat::Cooldowns)
    }

    /// Seconds of the mission clock until he may cloak again; nought when
    /// he may.
    pub fn cloak_cooldown_left(&self, medic: u32) -> f64 {
        let Some(began) = self.medic_of(medic).last_cloak else {
            return 0.0;
        };
        let since = (self.mission_minutes() - began) / time::MINUTES_PER_SECOND;
        (self.cloak_cooldown(medic) - since).max(0.0)
    }

    /// A crew member's cloak as kept — the default, no cloak, for anybody
    /// the world keeps none for.
    pub fn cloak_of(&self, who: u32) -> crate::medic::Cloak {
        self.cloaks.get(who as usize).copied().unwrap_or_default()
    }

    /// Seconds of the mission clock a crew member's cloak has left;
    /// nought with none on it.
    pub fn cloak_left(&self, who: u32) -> f64 {
        let Some(until) = self.cloak_of(who).until else {
            return 0.0;
        };
        ((until - self.mission_minutes()) / time::MINUTES_PER_SECOND).max(0.0)
    }

    /// Whether a crew member is under a cloak now: no enemy picks it, it
    /// fires nothing and uses no ability.
    pub fn is_cloaked(&self, who: u32) -> bool {
        self.cloak_left(who) > 0.0
    }

    /// Whether a player's medic may cloak crew member `target`, or why
    /// not, in order: a medic (`NotAMedic`), fit to act — downed among it
    /// — (`OutOfReach`), a rank (`NotLearnt`), out of the cooldown
    /// (`CoolingDown`), a living crew member, downed or not, himself
    /// included (`NotACrewmate`), then — for another — within
    /// [`class::CLOAK_RANGE`] (`OutOfCloakRange`) and in his sight
    /// (`NoSightOfTarget`).
    pub fn can_cloak(&self, slot: u32, target: u32) -> Result<(), Refusal> {
        if !class::can(self.class_of(slot), class::Ability::Cloak) {
            return Err(Refusal::NotAMedic);
        }
        if !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        if self.rank_of(slot, class::SLOT_R) == 0 {
            return Err(Refusal::NotLearnt);
        }
        if self.cloak_cooldown_left(slot) > 0.0 {
            return Err(Refusal::CoolingDown);
        }
        let room = &self.aboard.room;
        if target >= self.aboard.crew_count() || !room.is_alive(target as usize) {
            return Err(Refusal::NotACrewmate);
        }
        if target == slot {
            return Ok(());
        }
        if room.is_outside(target as usize) {
            return Err(Refusal::OutOfCloakRange);
        }
        let at = room.bim_pos(target as usize);
        let t = shipdesign::TILE as f32;
        if (at - room.bim_pos(slot as usize)).len() > class::CLOAK_RANGE * t {
            return Err(Refusal::OutOfCloakRange);
        }
        if !room.sees(slot as usize, at) {
            return Err(Refusal::NoSightOfTarget);
        }
        Ok(())
    }

    /// The Cloak — see [`Command::Cloak`]: the mission clock noted on the
    /// medic, and on the target a cloak to the later of its own end and
    /// this one's — never the two added — at the faster of the two paces.
    /// A medic cloaking himself lets his beam's patients go.
    fn cloak(&mut self, slot: u32, target: u32) -> Result<(), Refusal> {
        self.can_cloak(slot, target)?;
        let now = self.mission_minutes();
        let until = now + self.cloak_seconds(slot) * time::MINUTES_PER_SECOND;
        let pace =
            class::by_rank(class::CLOAK_PACE, self.rank_of(slot, class::SLOT_R)).unwrap_or(1.0);
        let was = self.cloak_of(target);
        let running = self.is_cloaked(target);
        let t = target as usize;
        if self.cloaks.len() <= t {
            self.cloaks.resize(t + 1, crate::medic::Cloak::default());
        }
        let end = match was.until {
            Some(end) if running => end.max(until),
            _ => until,
        };
        self.cloaks[t] = crate::medic::Cloak {
            until: Some(end),
            pace: if running { was.pace.max(pace) } else { pace },
            seconds: (end - now) / time::MINUTES_PER_SECOND,
        };
        self.medic_mut(slot as usize).last_cloak = Some(now);
        if target == slot {
            self.medic_mut(slot as usize).unlink();
            self.aboard.room.set_beaming(slot as usize, false);
        }
        self.aboard.room.set_cloaked(t, true);
        Ok(())
    }

    /// What a cloak does to a crew member's fighting (task 130): it holds
    /// its fire and walks at the cloak's pace, at all times.
    fn lift_by_cloak(&self, who: u32, skill: &mut bims::combat::Skill) {
        if self.is_cloaked(who) {
            skill.holds_fire = true;
            skill.walk *= self.cloak_of(who).pace;
        }
    }

    /// Before the rooms step: every cloak run out forgotten, and the room
    /// told who is cloaked, for its picture.
    fn hand_the_room_the_cloaks(&mut self) {
        let crew = self.aboard.crew_count() as usize;
        if self.cloaks.len() < crew {
            self.cloaks.resize(crew, crate::medic::Cloak::default());
        }
        for who in 0..crew {
            let on = self.is_cloaked(who as u32);
            if !on && self.cloaks[who].until.is_some() {
                self.cloaks[who] = crate::medic::Cloak::default();
            }
            self.aboard.room.set_cloaked(who, on);
        }
    }

    /// Which of the crew no enemy may pick now: a medic's cloak (task
    /// 130) or a relic's *Signal Scrambler* (task 118). Empty while
    /// neither is on anybody, so a fight without them hands over what it
    /// always did.
    pub(crate) fn hidden_from_enemies(&self) -> Vec<bool> {
        let unseen = self.unseen_by_machines();
        let crew = self.aboard.crew_count();
        if unseen.is_empty() && !(0..crew).any(|who| self.is_cloaked(who)) {
            return Vec::new();
        }
        (0..crew)
            .map(|who| unseen.get(who as usize).copied().unwrap_or(false) || self.is_cloaked(who))
            .collect()
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
            by: 0,
            fee: 0,
            due: self.clock_minutes + mercenary::MONTH,
            owed: false,
            medic: true,
        });
        true
    }

    /// Crew member `carrier` with crew member `patient` in its arms, the
    /// patient downed first so there is something to carry and the two
    /// stood beside each other — `BIMS_CARRY=1`, for looking at a body
    /// being carried off the deck without staging a fight and waiting for
    /// somebody to go down. False if it would not go.
    pub fn carry_for_probe(&mut self, carrier: u32, patient: u32) -> bool {
        if carrier >= self.aboard.crew_count() || patient >= self.aboard.crew_count() {
            return false;
        }
        // Beside the carrier, and downed so it is worth fetching.
        let at = self.aboard.room.bim_pos(carrier as usize)
            + bims::math::vec2(shipdesign::TILE as f32 * 0.8, 0.0);
        self.aboard.room.put_for_probe(patient as usize, at);
        self.aboard.room.knock_out_for_probe(patient as usize);
        self.step(&[]);
        self.aboard.room.take_up(carrier as usize, patient as usize)
    }

    /// Slot 0 a medic beaming crew member 1, for a probe and for
    /// `BIMS_BEAM` in the app: crew member 1 stood a tile from it at
    /// [`BEAM_PROBE_HEALTH`] of its bar — so the patient wants healing and
    /// the beam has something to do, and the green numbers over it
    /// (feature 91) count — and the link made, the beam's first rank
    /// bought if none is (`BIMS_RANKS` can say more). `false`, and nothing
    /// moved, with fewer than two aboard or with slot 0 no medic.
    pub fn beam_for_probe(&mut self) -> bool {
        if self.aboard.crew_count() < 2 || !self.is_medic(0) {
            return false;
        }
        if self.rank_of(0, class::SLOT_E) == 0 {
            let mut ranks = self.progress.first().map(|p| p.ranks).unwrap_or_default();
            ranks[class::SLOT_E as usize] = 1;
            self.set_ranks_for_probe(0, ranks);
        }
        // Both posted where they stand: a patient short of nothing but hit
        // points is on its feet and would walk off about its round, and
        // the beam break at its range, before anybody had looked at it.
        let here = self.aboard.room.bim_pos(0);
        self.aboard.room.post_for_probe(0, here);
        let at = here + bims::math::vec2(shipdesign::TILE as f32, 0.0);
        self.aboard.room.post_for_probe(1, at);
        self.aboard
            .room
            .set_health_for_probe(1, bims::health::MAX_HEALTH * BEAM_PROBE_HEALTH);
        self.step(&[]);
        self.beam(0, Some(1)).is_ok()
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
    /// to act or cloaked, or a patient dead, gone from the room, out of
    /// range or out of sight — and every beamed body given its hit points
    /// for the step, times the Healing Aura where it stands (task 130).
    /// Two beams on one body: the stronger. A downed body takes nothing:
    /// only a revive gets it up.
    fn hand_the_room_the_medics(&mut self, events: &mut Vec<WorldEvent>) {
        let crew = self.aboard.crew_count() as usize;
        if self.medics.len() < crew {
            self.medics.resize(crew, Medic::default());
        }
        // Hit points an hour of the clock, by body.
        let mut held: Vec<f32> = vec![0.0; crew];
        for m in 0..crew {
            if !self.medics[m].is_linked() {
                continue;
            }
            let who = m as u32;
            let was = self.medics[m].patients.clone();
            let keep: Vec<u32> =
                if !self.aboard.room.is_beaming(m) || !self.fit_to_act(who) || self.is_cloaked(who)
                {
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
            let rate = self.beam_rate(who);
            for &p in &keep {
                let slot = &mut held[p as usize];
                *slot = slot.max(rate);
            }
        }
        // An hour of the clock is sixty minutes, a step `STEP_MINUTES` of
        // them.
        let share = (data::STEP_MINUTES / 60.0) as f32;
        for (who, rate) in held.into_iter().enumerate() {
            if rate > 0.0 {
                self.heal_crew(who as u32, rate * share);
            }
        }
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
    /// hire, a bot dropped off the crew — and an index is the whole of
    /// what an arm holds, exactly as a beam's link is.
    fn clear_carries(&mut self) {
        for who in 0..self.aboard.crew_count() as usize {
            self.aboard.room.set_down(who);
        }
    }

    /// After the rooms step: every revive the room finished (task 120) —
    /// said, and the relics a revive sets off; no experience.
    fn settle_medics(&mut self, events: &mut Vec<WorldEvent>) {
        for revived in self.aboard.room.take_revives() {
            events.push(WorldEvent::CrewRevived {
                who: revived.patient as u32,
                by: revived.helper as u32,
            });
            if self.any_relics() {
                self.relics_on_a_revive(revived.helper, revived.patient, events);
            }
        }
        // And a townsperson the crew's hands brought round on the joined
        // deck: up in its own room at the helper's share of the bar, and
        // back in the fight. No relic fires for it — they are the crew's.
        let guests = self.aboard.room.take_guest_revives();
        if let Some(residents) = &mut self.residents {
            for revived in guests {
                if residents
                    .aboard
                    .room
                    .bring_round(revived.visitor, revived.share)
                {
                    events.push(WorldEvent::ResidentRevived {
                        station: residents.station,
                        who: revived.visitor as u32,
                        by: revived.helper as u32,
                    });
                }
            }
        }
    }

    /// Which of the station's people the crew may pick up with the medkit,
    /// index for index with the visitors (`Aboard::visit`): a Bim of the
    /// residents' room — never a machine, never a Manufacturer — downed and
    /// still alive, on the deck and in nobody's arms, at a station not at
    /// war with the crew. What `Game::set_visitors_revivable` is told.
    fn revivable_residents(&self) -> Vec<bool> {
        let Some(residents) = &self.residents else {
            return Vec::new();
        };
        if self.stance(residents.station) == Stance::Hostile {
            return Vec::new();
        }
        let room = &residents.aboard.room;
        let bims = room.crew_count() as usize;
        (0..residents.aboard.count() as usize)
            .map(|who| {
                who < bims
                    && !room.is_manufacturer(who)
                    && room.is_alive(who)
                    && room.is_downed(who)
                    && !room.is_outside(who)
                    && !room.is_carried(who)
            })
            .collect()
    }

    /// Which of the station's people have a crew member's hands on them,
    /// index for index with its room, for the residents' room to stand
    /// their countdowns (`Game::set_tended`): what the crew's room says of
    /// every revive of a visitor under way.
    fn tended_residents(&self) -> Vec<bool> {
        let Some(residents) = &self.residents else {
            return Vec::new();
        };
        (0..residents.aboard.count() as usize)
            .map(|who| {
                self.aboard
                    .room
                    .revive_share(bims::game::GUEST + who)
                    .is_some()
            })
            .collect()
    }

    // --- the tank: a ranked kit and two base traits (task 139) -------------
    //
    // `crate::tank` is the state — when his Taunt and his Juggernaut last
    // began and the window each covers — and this is the rules. Bulwark
    // is a flag on the Bim, and Plated and the armour's drain are read
    // afresh each step into the room's one `bims::combat::Skill`.

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

    /// The tank's half of [`World::skill_of`], off his ranks and nothing
    /// else; `Skill::NONE` for anybody else. **Plated** is the damage he
    /// takes, before the armour; **Bulwark**, while it is up, the rank's
    /// pace and from the third rank the dodge; **Juggernaut**, while it
    /// runs, the damage he takes again — every factor multiplied together,
    /// and with a commander's Rally after (`lift_by_commanders`). The
    /// armour's drain is `armour_drain`'s.
    fn tank_skill(&self, who: u32) -> bims::combat::Skill {
        let mut skill = bims::combat::Skill::NONE;
        if !self.is_tank(who) {
            return skill;
        }
        if let Some(taken) =
            class::by_rank(class::PLATED_DAMAGE_TAKEN, self.rank_of(who, class::SLOT_C))
        {
            skill.damage_taken *= taken;
        }
        let wall = self.rank_of(who, class::SLOT_E);
        if wall > 0 && self.is_bulwark(who) {
            skill.walk = self.bulwark_pace(who);
            if wall >= class::GUARDED_RANK {
                skill.dodge = class::GUARDED_DODGE;
            }
        }
        if self.is_juggernaut(who) {
            let rank = self.rank_of(who, class::SLOT_R);
            skill.damage_taken *=
                class::by_rank(class::JUGGERNAUT_DAMAGE_TAKEN, rank).unwrap_or(1.0);
        }
        skill
    }

    /// What a crew member's worn armour drains at, of the damage that
    /// gets past its protection: [`class::TANK_DRAIN`] for a tank — again
    /// times [`class::FORTRESS_DRAIN`] from Plated's
    /// [`class::FORTRESS_RANK`], a quarter in all — and one for everybody
    /// else.
    pub fn armour_drain(&self, who: u32) -> f32 {
        if !self.is_tank(who) {
            return 1.0;
        }
        if self.rank_of(who, class::SLOT_C) >= class::FORTRESS_RANK {
            class::TANK_DRAIN * class::FORTRESS_DRAIN
        } else {
            class::TANK_DRAIN
        }
    }

    /// How far a tank's bulwark reaches, in tiles: its rank's
    /// [`class::BULWARK_REACH`], nought before the first.
    pub fn bulwark_reach(&self, who: u32) -> f32 {
        class::by_rank(class::BULWARK_REACH, self.rank_of(who, class::SLOT_E)).unwrap_or(0.0)
    }

    /// What a tank's pace is multiplied by while the wall is up: its
    /// rank's [`class::BULWARK_PACE`], one before the first.
    pub fn bulwark_pace(&self, who: u32) -> f32 {
        class::by_rank(class::BULWARK_PACE, self.rank_of(who, class::SLOT_E)).unwrap_or(1.0)
    }

    /// Whether a crew member stands as a wall.
    pub fn is_bulwark(&self, who: u32) -> bool {
        self.aboard.room.is_bulwark(who as usize)
    }

    /// Whether a player's tank may stand as a wall, or why not: a tank
    /// (`NotATank`), fit to act (`OutOfReach`), and a rank of Bulwark
    /// (`NotLearnt`, task 139). What the app greys the key with and
    /// [`Command::Bulwark`] asks.
    pub fn can_bulwark(&self, slot: u32) -> Result<(), Refusal> {
        if !class::can(self.class_of(slot), class::Ability::Bulwark) {
            return Err(Refusal::NotATank);
        }
        if !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        if self.rank_of(slot, class::SLOT_E) == 0 {
            return Err(Refusal::NotLearnt);
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

    /// Seconds of the mission clock a tank's taunt runs: its rank's
    /// [`class::TAUNT_SECONDS`], nought before the first.
    pub fn taunt_seconds(&self, who: u32) -> f64 {
        class::by_rank(class::TAUNT_SECONDS, self.rank_of(who, class::SLOT_Q)).unwrap_or(0.0)
            * self.ability_length_factor(who)
    }

    /// How far a tank's taunt reaches, in tiles: its rank's
    /// [`class::TAUNT_RADIUS`], nought before the first.
    pub fn taunt_radius(&self, who: u32) -> f32 {
        class::by_rank(class::TAUNT_RADIUS, self.rank_of(who, class::SLOT_Q)).unwrap_or(0.0)
    }

    /// Seconds of the mission clock a tank's taunt has left; nought with
    /// none running.
    pub fn taunt_left(&self, who: u32) -> f64 {
        if !self.is_taunting(who) {
            return 0.0;
        }
        (self.tank_of(who).taunt.until - self.mission_minutes()) / time::MINUTES_PER_SECOND
    }

    /// Whether a taunt is running on a crew member.
    pub fn is_taunting(&self, who: u32) -> bool {
        self.is_tank(who) && self.tank_of(who).taunt.running(self.mission_minutes())
    }

    /// Seconds of the mission clock until a tank may taunt again; nought
    /// when he may.
    pub fn taunt_cooldown_left(&self, who: u32) -> f64 {
        let Some(last) = self.tank_of(who).last_taunt else {
            return 0.0;
        };
        let since = (self.mission_minutes() - last) / time::MINUTES_PER_SECOND;
        (self.taunt_cooldown(who) - since).max(0.0)
    }

    /// Seconds of the mission clock between one taunt and the next: its
    /// rank's [`class::TAUNT_COOLDOWN`] (the first's before one), shorter
    /// with a relic's *Coolant Loop* as every class cooldown is.
    pub fn taunt_cooldown(&self, who: u32) -> f64 {
        let rank = self.rank_of(who, class::SLOT_Q).max(1);
        class::by_rank(class::TAUNT_COOLDOWN, rank).unwrap_or(0.0)
            * self.relic_factor(who, crate::relic::Stat::Cooldowns)
    }

    /// Whether a player's tank may taunt, or why not, in order: a tank
    /// (`NotATank`), fit to act — downed among it — (`OutOfReach`), a
    /// rank of Taunt (`NotLearnt`), and out of the cooldown
    /// (`CoolingDown`).
    pub fn can_taunt(&self, slot: u32) -> Result<(), Refusal> {
        if !class::can(self.class_of(slot), class::Ability::Taunt) {
            return Err(Refusal::NotATank);
        }
        if !self.fit_to_act(slot) || self.aboard.room.is_down(slot as usize) {
            return Err(Refusal::OutOfReach);
        }
        if self.rank_of(slot, class::SLOT_Q) == 0 {
            return Err(Refusal::NotLearnt);
        }
        if self.taunt_cooldown_left(slot) > 0.0 {
            return Err(Refusal::CoolingDown);
        }
        Ok(())
    }

    /// The taunt — see [`Command::Taunt`]: the mission clock noted, and
    /// when it ends. What it *does* is read off that every step, in
    /// `taunts_for_the_enemy`.
    fn taunt(&mut self, slot: u32) -> Result<(), Refusal> {
        self.can_taunt(slot)?;
        let now = self.mission_minutes();
        let until = now + self.taunt_seconds(slot) * time::MINUTES_PER_SECOND;
        let tank = self.tank_mut(slot as usize);
        tank.last_taunt = Some(now);
        tank.taunt = crate::tank::Window { began: now, until };
        Ok(())
    }

    /// Seconds of the mission clock a Juggernaut runs: its rank's
    /// [`class::JUGGERNAUT_SECONDS`], nought before the first.
    pub fn juggernaut_seconds(&self, who: u32) -> f64 {
        class::by_rank(class::JUGGERNAUT_SECONDS, self.rank_of(who, class::SLOT_R)).unwrap_or(0.0)
            * self.ability_length_factor(who)
    }

    /// Whether a crew member's Juggernaut is running.
    pub fn is_juggernaut(&self, who: u32) -> bool {
        self.is_tank(who) && self.tank_of(who).juggernaut.running(self.mission_minutes())
    }

    /// Seconds of the mission clock the Juggernaut running has left;
    /// nought with none.
    pub fn juggernaut_left(&self, who: u32) -> f64 {
        if !self.is_juggernaut(who) {
            return 0.0;
        }
        (self.tank_of(who).juggernaut.until - self.mission_minutes()) / time::MINUTES_PER_SECOND
    }

    /// Seconds of the mission clock from one Juggernaut to the next: its
    /// rank's [`class::JUGGERNAUT_COOLDOWN`], shorter with a relic's
    /// *Coolant Loop*.
    pub fn juggernaut_cooldown(&self, who: u32) -> f64 {
        let rank = self.rank_of(who, class::SLOT_R).max(1);
        class::by_rank(class::JUGGERNAUT_COOLDOWN, rank).unwrap_or(0.0)
            * self.relic_factor(who, crate::relic::Stat::Cooldowns)
    }

    /// Seconds of the mission clock until a tank may go Juggernaut again;
    /// nought when he may.
    pub fn juggernaut_cooldown_left(&self, who: u32) -> f64 {
        let Some(last) = self.tank_of(who).last_juggernaut else {
            return 0.0;
        };
        let since = (self.mission_minutes() - last) / time::MINUTES_PER_SECOND;
        (self.juggernaut_cooldown(who) - since).max(0.0)
    }

    /// Whether a player's tank may go Juggernaut, or why not, in order: a
    /// tank (`NotATank`), fit to act — downed among it — (`OutOfReach`), a
    /// rank of Juggernaut (`NotLearnt`), none running (`AlreadyActive`)
    /// and out of the cooldown (`CoolingDown`). A taunt running beside it
    /// is no bar.
    pub fn can_juggernaut(&self, slot: u32) -> Result<(), Refusal> {
        if !class::can(self.class_of(slot), class::Ability::Juggernaut) {
            return Err(Refusal::NotATank);
        }
        if !self.fit_to_act(slot) || self.aboard.room.is_down(slot as usize) {
            return Err(Refusal::OutOfReach);
        }
        if self.rank_of(slot, class::SLOT_R) == 0 {
            return Err(Refusal::NotLearnt);
        }
        if self.is_juggernaut(slot) {
            return Err(Refusal::AlreadyActive);
        }
        if self.juggernaut_cooldown_left(slot) > 0.0 {
            return Err(Refusal::CoolingDown);
        }
        Ok(())
    }

    /// The Juggernaut — see [`Command::Juggernaut`]: the mission clock
    /// noted, and when it ends. What it does is read off that every step,
    /// in `tank_skill` and `taunts_for_the_enemy`.
    fn juggernaut(&mut self, slot: u32) -> Result<(), Refusal> {
        self.can_juggernaut(slot)?;
        let now = self.mission_minutes();
        let until = now + self.juggernaut_seconds(slot) * time::MINUTES_PER_SECOND;
        let tank = self.tank_mut(slot as usize);
        tank.last_juggernaut = Some(now);
        tank.juggernaut = crate::tank::Window { began: now, until };
        Ok(())
    }

    /// What each of the crew forces on the enemy, index for index with
    /// the crew (task 139): a tank's Taunt — its rank's radius in room
    /// units, the blades pulled from [`class::TAUNT_MAGNET_RANK`] — or his
    /// Juggernaut, at any distance; the most recent of two tanks' the
    /// higher `order`. `Taunt::NONE` for everybody else. A tank the enemy
    /// may not pick — cloaked — is no target on their list at all, so his
    /// forces nothing while it lasts.
    fn taunts_for_the_enemy(&self) -> Vec<bims::combat::Taunt> {
        let now = self.mission_minutes();
        let crew = self.aboard.crew_count();
        let since: Vec<Option<f64>> = (0..crew)
            .map(|who| {
                self.is_tank(who)
                    .then(|| self.tank_of(who).forcing_since(now))
                    .flatten()
            })
            .collect();
        (0..crew)
            .map(|who| {
                let Some(began) = since[who as usize] else {
                    return bims::combat::Taunt::NONE;
                };
                let radius = if self.is_juggernaut(who) {
                    f32::INFINITY
                } else {
                    self.taunt_radius(who) * shipdesign::TILE as f32
                };
                let magnet = self.is_taunting(who)
                    && self.rank_of(who, class::SLOT_Q) >= class::TAUNT_MAGNET_RANK;
                // The more recent, the higher: one more than every
                // forcing that began before this one.
                let order = 1 + since.iter().flatten().filter(|&&b| b < began).count() as u32;
                bims::combat::Taunt {
                    radius,
                    magnet,
                    order,
                }
            })
            .collect()
    }

    /// Before the rooms step: the walls standing among the crew to the
    /// crew's room, so a bolt aimed through one is dodged like a bolt in
    /// cover (and, from Bulwark's [`class::INTERPOSE_RANK`], lands on the
    /// wall). A tank not fit to act shelters nobody, so a wall goes down
    /// with the tank the same step he does.
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
                interpose: self.rank_of(who, class::SLOT_E) >= class::INTERPOSE_RANK,
            })
            .collect();
        self.aboard.room.set_bulwarks(walls);
    }

    // --- the commander: a ranked kit and a base trait (task 129) ----------
    //
    // `crate::commander` is the state — when each commander last cried
    // and rallied and whom each reached, and the reinforcements of the
    // mission — and this is the rules. The aura is not kept at all: it
    // is worked out every step from where the commanders stand and goes
    // to the room through `skill_of`.
    //
    // **Whom each reaches.** The aura, the Battle Cry and the Rally lift
    // every friendly Bim in range, a player's own steered Bim and the
    // commander himself included. His squad orders (attack, fall back,
    // stand ground) were removed; the cheaper hire is his one base trait.

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

    /// Whether a crew member is on the crew's deck to be reached at all:
    /// alive and not outside in a suit.
    fn on_the_deck(&self, who: u32) -> bool {
        let room = &self.aboard.room;
        who < self.aboard.crew_count()
            && room.is_alive(who as usize)
            && !room.is_outside(who as usize)
    }

    /// Every crew member on the deck within `tiles` of the commander at
    /// `slot`, himself included, lowest index first: whom a Battle Cry or
    /// a Rally called now would reach — the list it keeps.
    pub fn crew_within(&self, slot: u32, tiles: f32) -> Vec<u32> {
        if !self.on_the_deck(slot) {
            return Vec::new();
        }
        let room = &self.aboard.room;
        let at = room.bim_pos(slot as usize);
        let reach = tiles * shipdesign::TILE as f32;
        (0..self.aboard.crew_count())
            .filter(|&who| {
                self.on_the_deck(who) && (room.bim_pos(who as usize) - at).len() <= reach
            })
            .collect()
    }

    /// How far a commander's **Command Aura** reaches, in tiles: its
    /// rank's radius ([`class::AURA_TILES`]), nought before the first.
    pub fn aura_radius(&self, who: u32) -> f32 {
        if !self.is_commander(who) {
            return 0.0;
        }
        class::by_rank(class::AURA_TILES, self.rank_of(who, class::SLOT_C)).unwrap_or(0.0)
    }

    /// What one commander's aura does to a Bim standing in it: its rank's
    /// damage factor ([`class::AURA_DAMAGE`]).
    fn aura_cast_by(&self, who: u32) -> Aura {
        Aura {
            damage: class::by_rank(class::AURA_DAMAGE, self.rank_of(who, class::SLOT_C))
                .unwrap_or(1.0),
        }
    }

    /// Whether a commander's aura reaches a crew member: a commander with
    /// a rank of it, on his feet — fit to act, not downed — the Bim on
    /// the deck, and the two within the aura's radius. **Himself
    /// included** since task 129, and never an enemy — the crew's room is
    /// the only room asked.
    pub fn in_aura_of(&self, commander: u32, who: u32) -> bool {
        if !self.is_commander(commander) || !self.fit_to_act(commander) {
            return false;
        }
        let radius = self.aura_radius(commander);
        if radius <= 0.0 || !self.on_the_deck(who) {
            return false;
        }
        let room = &self.aboard.room;
        let gap = room.bim_pos(who as usize) - room.bim_pos(commander as usize);
        gap.len() <= radius * shipdesign::TILE as f32
    }

    /// The strongest aura reaching a crew member, or `None`. **Two
    /// commanders' auras never stack**: the higher factor holds it, and
    /// the others do nothing.
    pub fn aura_reaching(&self, who: u32) -> Option<Aura> {
        (0..self.aboard.crew_count())
            .filter(|&c| self.in_aura_of(c, who))
            .map(|c| self.aura_cast_by(c))
            .max_by(|a, b| a.damage.total_cmp(&b.damage))
    }

    // The Rally (E) and the Battle Cry (Q): a timestamp and a list each,
    // read the soldier's Rampage's way — seconds of the mission clock.

    /// Seconds of the mission clock a commander's Rally runs at his rank
    /// ([`class::RALLY_SECONDS`]).
    pub fn rally_seconds(&self, who: u32) -> f64 {
        class::by_rank(class::RALLY_SECONDS, self.rank_of(who, class::SLOT_E)).unwrap_or(0.0)
            * self.ability_length_factor(who)
    }

    /// Seconds of the mission clock between one Rally and the next:
    /// [`class::RALLY_COOLDOWN`] of his rank, shorter with a relic's
    /// *Coolant Loop* as every class cooldown is.
    pub fn rally_cooldown(&self, who: u32) -> f64 {
        let rank = self.rank_of(who, class::SLOT_E).max(1);
        class::by_rank(class::RALLY_COOLDOWN, rank).unwrap_or(0.0)
            * self.relic_factor(who, crate::relic::Stat::Cooldowns)
    }

    /// Seconds of the mission clock a commander's Rally has left; nought
    /// with none running.
    pub fn rally_left(&self, who: u32) -> f64 {
        let Some(began) = self.commander_of(who).last_rally else {
            return 0.0;
        };
        let since = (self.mission_minutes() - began) / time::MINUTES_PER_SECOND;
        (self.rally_seconds(who) - since).max(0.0)
    }

    /// Whether a Rally is running on a commander.
    pub fn is_rallying(&self, who: u32) -> bool {
        self.is_commander(who) && self.rally_left(who) > 0.0
    }

    /// Seconds of the mission clock until he may rally again; nought
    /// when he may.
    pub fn rally_cooldown_left(&self, who: u32) -> f64 {
        let Some(began) = self.commander_of(who).last_rally else {
            return 0.0;
        };
        let since = (self.mission_minutes() - began) / time::MINUTES_PER_SECOND;
        (self.rally_cooldown(who) - since).max(0.0)
    }

    /// The commander whose running Rally covers a crew member, if any:
    /// one it reached when he called it, walked off or not. Two covering
    /// one Bim: the lower damage taken holds it.
    pub fn rally_reaching(&self, who: u32) -> Option<u32> {
        (0..self.aboard.crew_count())
            .filter(|&c| self.is_rallying(c) && self.commander_of(c).rallied.contains(&who))
            .min_by(|&a, &b| {
                let of = |c: u32| {
                    class::by_rank(class::RALLY_DAMAGE_TAKEN, self.rank_of(c, class::SLOT_E))
                        .unwrap_or(1.0)
                };
                of(a).total_cmp(&of(b))
            })
    }

    /// Whether a player's commander may rally, or why not, in order: a
    /// commander (`NotACommander`), fit to act — downed among it —
    /// (`OutOfReach`), a rank of Rally (`NotLearnt`) and out of the
    /// cooldown (`CoolingDown`).
    pub fn can_rally(&self, slot: u32) -> Result<(), Refusal> {
        if !class::can(self.class_of(slot), class::Ability::Rally) {
            return Err(Refusal::NotACommander);
        }
        if !self.fit_to_act(slot) || self.aboard.room.is_down(slot as usize) {
            return Err(Refusal::OutOfReach);
        }
        if self.rank_of(slot, class::SLOT_E) == 0 {
            return Err(Refusal::NotLearnt);
        }
        if self.rally_cooldown_left(slot) > 0.0 {
            return Err(Refusal::CoolingDown);
        }
        Ok(())
    }

    /// The Rally — see [`Command::Rally`]: the mission clock noted and
    /// **whom it reaches, fixed now**. What it does is read off those
    /// every step, in `skill_of`.
    fn rally(&mut self, slot: u32) -> Result<(), Refusal> {
        self.can_rally(slot)?;
        let now = self.mission_minutes();
        let reached = self.crew_within(slot, class::RALLY_TILES);
        let commander = self.commander_mut(slot as usize);
        commander.last_rally = Some(now);
        commander.rallied = reached;
        Ok(())
    }

    /// Seconds of the mission clock a commander's Battle Cry runs at his
    /// rank ([`class::BATTLE_CRY_SECONDS`]).
    pub fn battle_cry_seconds(&self, who: u32) -> f64 {
        class::by_rank(class::BATTLE_CRY_SECONDS, self.rank_of(who, class::SLOT_Q)).unwrap_or(0.0)
            * self.ability_length_factor(who)
    }

    /// Seconds of the mission clock between one Battle Cry and the next:
    /// [`class::BATTLE_CRY_COOLDOWN`] of his rank, times the cooldown
    /// relics.
    pub fn battle_cry_cooldown(&self, who: u32) -> f64 {
        let rank = self.rank_of(who, class::SLOT_Q).max(1);
        class::by_rank(class::BATTLE_CRY_COOLDOWN, rank).unwrap_or(0.0)
            * self.relic_factor(who, crate::relic::Stat::Cooldowns)
    }

    /// Seconds of the mission clock a commander's Battle Cry has left;
    /// nought with none running.
    pub fn battle_cry_left(&self, who: u32) -> f64 {
        let Some(began) = self.commander_of(who).last_battle_cry else {
            return 0.0;
        };
        let since = (self.mission_minutes() - began) / time::MINUTES_PER_SECOND;
        (self.battle_cry_seconds(who) - since).max(0.0)
    }

    /// Whether a Battle Cry is running on a commander.
    pub fn is_crying(&self, who: u32) -> bool {
        self.is_commander(who) && self.battle_cry_left(who) > 0.0
    }

    /// Seconds of the mission clock until he may cry again; nought when
    /// he may.
    pub fn battle_cry_cooldown_left(&self, who: u32) -> f64 {
        let Some(began) = self.commander_of(who).last_battle_cry else {
            return 0.0;
        };
        let since = (self.mission_minutes() - began) / time::MINUTES_PER_SECOND;
        (self.battle_cry_cooldown(who) - since).max(0.0)
    }

    /// The commander whose running Battle Cry covers a crew member, if
    /// any: one it reached when he called it. Two covering one Bim: the
    /// higher fire rate holds it.
    pub fn battle_cry_reaching(&self, who: u32) -> Option<u32> {
        (0..self.aboard.crew_count())
            .filter(|&c| self.is_crying(c) && self.commander_of(c).cried.contains(&who))
            .max_by(|&a, &b| {
                let of = |c: u32| {
                    class::by_rank(class::BATTLE_CRY_FIRE_RATE, self.rank_of(c, class::SLOT_Q))
                        .unwrap_or(1.0)
                };
                of(a).total_cmp(&of(b))
            })
    }

    /// Whether a player's commander may call a Battle Cry, or why not, in
    /// the Rally's order: `NotACommander`, `OutOfReach`, `NotLearnt`,
    /// `CoolingDown`.
    pub fn can_battle_cry(&self, slot: u32) -> Result<(), Refusal> {
        if !class::can(self.class_of(slot), class::Ability::BattleCry) {
            return Err(Refusal::NotACommander);
        }
        if !self.fit_to_act(slot) || self.aboard.room.is_down(slot as usize) {
            return Err(Refusal::OutOfReach);
        }
        if self.rank_of(slot, class::SLOT_Q) == 0 {
            return Err(Refusal::NotLearnt);
        }
        if self.battle_cry_cooldown_left(slot) > 0.0 {
            return Err(Refusal::CoolingDown);
        }
        Ok(())
    }

    /// The Battle Cry — see [`Command::BattleCry`]: the mission clock
    /// noted and whom it reaches fixed now.
    fn battle_cry(&mut self, slot: u32) -> Result<(), Refusal> {
        self.can_battle_cry(slot)?;
        let now = self.mission_minutes();
        let reached = self.crew_within(slot, class::BATTLE_CRY_TILES);
        let commander = self.commander_mut(slot as usize);
        commander.last_battle_cry = Some(now);
        commander.cried = reached;
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
        let players = self.players() as usize;
        if self.standing.len() < players {
            self.standing.resize(players, Standing::Follow);
        }
        let Some(&mine) = self.standing.get(slot as usize) else {
            return Err(Refusal::OutOfReach);
        };
        // **A release is never refused for its ground.** The ground is
        // asked about a banner being *put down*: an attack wants a tile
        // of the deck under it. Asked of the same order given again —
        // which is the only press there is that takes a banner up — it
        // refused the one thing the crew needed, since a banner on a
        // station's deck is on no tile of the crew's room once the ship
        // has cast off, and they stood under arms at it for ever.
        if mine.same_as(order) {
            self.standing[slot as usize] = Standing::Follow;
            return Ok(Standing::Follow.code());
        }
        if !self.ground_for(order) {
            return Err(Refusal::NoGroundThere);
        }
        self.standing[slot as usize] = order;
        Ok(order.code())
    }

    /// Whether an order has ground under it: an attack's tile is a tile
    /// of the crew's room (the deck, a joined station's deck, or
    /// anywhere at all out on a plain — [`bims::game::Game::is_banner_tile`]),
    /// and every other order wants nothing.
    fn ground_for(&self, order: Standing) -> bool {
        let Standing::Attack { tile } = order else {
            return true;
        };
        let t = shipdesign::TILE as f32;
        self.aboard.room.is_banner_tile(bims::math::vec2(
            (tile.0 as f32 + 0.5) * t,
            (tile.1 as f32 + 0.5) * t,
        ))
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
    /// the same gate the order was taken under — **or where an attack's
    /// tile is no longer a tile of the crew's room**, which is every
    /// dock, undock, landing and lift-off, since a banner is a tile of
    /// the deck the crew walk and that deck is built afresh at each of
    /// them. Without it the crew stood under arms at a tile that was
    /// nowhere — the station's deck, a ship's length astern — taking no
    /// errand and never arriving. Then the lot is handed to the room,
    /// one `bims::game::Standing` a player slot, an attack's tile turned
    /// into the room's own units.
    fn hand_the_room_the_standing(&mut self) {
        let players = self.players() as usize;
        if self.standing.len() != players {
            self.standing.resize(players, Standing::Follow);
        }
        for slot in 0..players {
            if !self.fit_to_act(slot as u32) || !self.ground_for(self.standing[slot]) {
                self.standing[slot] = Standing::Follow;
            }
        }
        let t = shipdesign::TILE as f32;
        // *Back to ship* pressed (feature 103): every bot goes home,
        // whoever it follows and whatever it was told before.
        let recalled = self.run.recalled;
        let said: Vec<bims::game::Standing> = self
            .standing
            .iter()
            .map(|order| if recalled { &Standing::Retreat } else { order })
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
        // units, and `None` for a ship on its own, where the room's own
        // answer is right.
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

    /// What the commanders do to a crew member's fighting, over whatever
    /// its own class gave it (task 129): the strongest **Command Aura**
    /// reaching it on its damage, bolt and blow alike; a **Battle Cry**
    /// that reached it on its fire rate; a **Rally** that reached it on the
    /// damage it takes and on its pace. Everything here reaches a player's
    /// own steered Bim as readily as a bot, and each multiplies into the
    /// skill with every other factor. A sentry has a skill of its own
    /// (`sentry_skill`) and is lifted by none of it.
    fn lift_by_commanders(&self, who: u32, skill: &mut bims::combat::Skill) {
        if let Some(aura) = self.aura_reaching(who) {
            skill.damage *= aura.damage;
            skill.melee *= aura.damage;
        }
        if let Some(c) = self.battle_cry_reaching(who) {
            let rank = self.rank_of(c, class::SLOT_Q);
            skill.fire_rate *= class::by_rank(class::BATTLE_CRY_FIRE_RATE, rank).unwrap_or(1.0);
        }
        if let Some(c) = self.rally_reaching(who) {
            let rank = self.rank_of(c, class::SLOT_E);
            skill.damage_taken *= class::by_rank(class::RALLY_DAMAGE_TAKEN, rank).unwrap_or(1.0);
            skill.walk *= class::by_rank(class::RALLY_PACE, rank).unwrap_or(1.0);
        }
    }

    /// Before the rooms step: a commander's state for every crew member,
    /// the list grown with the crew (the checksum reads its length).
    fn size_the_commanders(&mut self) {
        if self.commanders.len() < self.aboard.crew_count() as usize {
            self.commanders
                .resize(self.aboard.crew_count() as usize, Commander::default());
        }
    }

    /// What a mercenary's month costs when this slot does the hiring:
    /// the fee less [`class::HIRE_DISCOUNT_PERCENT`] for a commander fit
    /// to act — a base trait, at every level — rounded down to whole
    /// euros, and the plain fee for everybody else. Read once, as the
    /// contract is signed: it stays that hand's fee whatever happens to
    /// him.
    pub fn hire_fee(&self, slot: u32, resident: u32) -> Option<Money> {
        let fee = self.mercenary_fee(resident)?;
        if !self.is_commander(slot) || !self.fit_to_act(slot) {
            return Some(fee);
        }
        Some(fee - fee * Money::from(class::HIRE_DISCOUNT_PERCENT) / 100)
    }

    // --- the commander's Reinforcements (task 129) --------------------------
    //
    // A reinforcement is a crew member marked with the commander who
    // brought it, for one mission: laid on free deck beside him at its
    // start (`bring_reinforcements`), gone from the deck the moment it
    // dies (`settle_reinforcements`, the room's `vanish` — its index kept,
    // so nobody else's shifts in the middle of a fight) and off the crew
    // at its end, alive or not (`send_reinforcements_home`). It is a bot
    // to everything that asks — the aura, a revive — and to
    // nothing that pays or counts: no experience (it has no class), no
    // loot, no wages, no penalty, no worth, and no run is kept going by it.

    /// The player slot of the commander a crew member is a reinforcement
    /// of; `None` for everybody else.
    pub fn reinforcement_of(&self, who: u32) -> Option<u32> {
        self.reinforcements
            .iter()
            .find(|r| r.who == who)
            .map(|r| r.by)
    }

    /// Whether a crew member is a reinforcement.
    pub fn is_reinforcement(&self, who: u32) -> bool {
        self.reinforcement_of(who).is_some()
    }

    /// The reinforcements a commander brought that are still alive, by
    /// crew index.
    pub fn reinforcements_of(&self, commander: u32) -> Vec<u32> {
        self.reinforcements
            .iter()
            .filter(|r| r.by == commander && self.aboard.room.is_alive(r.who as usize))
            .map(|r| r.who)
            .collect()
    }

    /// How many Bims a commander brings to a mission's start at his rank
    /// ([`class::REINFORCEMENTS`]); nought before the first, and for
    /// anybody but a commander.
    pub fn reinforcements_due(&self, commander: u32) -> u32 {
        if !self.is_commander(commander) {
            return 0;
        }
        class::by_rank(
            class::REINFORCEMENTS,
            self.rank_of(commander, class::SLOT_R),
        )
        .unwrap_or(0)
    }

    /// A mission's start: every commander with a rank of Reinforcements —
    /// each player's in slot order — brings his, onto the free deck
    /// nearest him within [`class::REINFORCEMENT_REACH_TILES`], as many as
    /// his rank gives and as the tiles found allow. Each is a classless
    /// Bim in the crew's coverall with the rank's auto rifle and nothing
    /// to wear, its face rolled off the galaxy's seed and the mission
    /// rather than the room's stream. Every classed crew member is asked,
    /// which is every player in a run and the `end` command's classed bots
    /// after them (`classed_crew_for_probe`).
    fn bring_reinforcements(&mut self, events: &mut Vec<WorldEvent>) {
        let t = shipdesign::TILE as f32;
        let mut brought = false;
        for slot in 0..self.classes.len() as u32 {
            if slot >= self.aboard.crew_count() || !self.aboard.room.is_alive(slot as usize) {
                continue;
            }
            let due = self.reinforcements_due(slot);
            if due == 0 {
                continue;
            }
            let rank = self.rank_of(slot, class::SLOT_R);
            let tier = class::by_rank(class::REINFORCEMENT_TIER, rank).unwrap_or(Tier::One);
            let at = self.aboard.room.bim_pos(slot as usize);
            let spots = self
                .aboard
                .room
                .free_tiles_near(at, class::REINFORCEMENT_REACH_TILES * t);
            let mut count = 0u32;
            for spot in spots.into_iter().take(due as usize) {
                let seed = self.galaxy_seed
                    ^ REINFORCEMENT_SALT
                    ^ (u64::from(self.run.missions) << 24)
                    ^ (u64::from(slot) << 8)
                    ^ u64::from(count);
                let gear = bims::combat::Gear {
                    weapon: Some(bims::combat::WeaponKind::AutoRifle.at(tier)),
                    ..bims::combat::Gear::default()
                };
                let who = self.aboard.room.enlist_reinforcement(spot, gear, seed) as u32;
                self.crew_down.push(false);
                self.crew_locked.push(false);
                self.reinforcements
                    .push(crate::commander::Reinforcement { who, by: slot });
                count += 1;
            }
            if count > 0 {
                brought = true;
                events.push(WorldEvent::Reinforced { who: slot, count });
            }
        }
        if !brought {
            return;
        }
        let crew = self.aboard.room.crew_count();
        self.aboard.crew = crew;
        self.ship.crew_count = crew;
        self.medics.resize(crew as usize, Medic::default());
        self.tanks.resize(crew as usize, Tank::default());
        self.commanders.resize(crew as usize, Commander::default());
        self.on_ship_changed();
    }

    /// A probe's way to the Reinforcements of a mission already under way
    /// (`BIMS_RANKS` sets the ranks after the probe's mission began):
    /// every commander brings his as a mission's start would, unless he
    /// has some already.
    pub fn reinforce_for_probe(&mut self) -> Vec<WorldEvent> {
        let mut events = Vec::new();
        if self.reinforcements.is_empty() {
            self.bring_reinforcements(&mut events);
        }
        events
    }

    /// After the deaths are said: a reinforcement that died is **gone
    /// from the deck at once** — not drawn, picked or counted — and does
    /// not come back that mission. Its crew index stays until the
    /// mission's end, so nobody else's moves in the middle of a fight.
    fn settle_reinforcements(&mut self) {
        for r in &self.reinforcements {
            let who = r.who as usize;
            if !self.aboard.room.is_alive(who) && !self.aboard.room.is_gone(who) {
                self.aboard.room.vanish(who);
            }
        }
    }

    /// A mission's end: every reinforcement off the crew, alive or not,
    /// its rifle with it — highest index first so the indices below stay
    /// right.
    fn send_reinforcements_home(&mut self) {
        let mut gone: Vec<u32> = self.reinforcements.iter().map(|r| r.who).collect();
        gone.sort_unstable();
        for who in gone.into_iter().rev() {
            self.drop_crew_member(who);
        }
        self.reinforcements.clear();
    }

    /// Before the rooms step: every crew member's skill to the room.
    fn hand_the_room_the_soldiers(&mut self) {
        let crew = self.aboard.crew_count();
        let skills: Vec<bims::combat::Skill> = (0..crew).map(|who| self.skill_of(who)).collect();
        self.aboard.room.set_skills(skills);
        self.hand_the_room_the_shield_fronts();
    }

    /// Which of the residents' bodies are the crew's enemies, as the
    /// first of them — every body from there to the end of the list —
    /// or `None` where no fight is on: the rooms unjoined, or a station
    /// at peace. At a hostile station it is all of them; in a town the
    /// crew are defending it is the machines alone, past the town's own
    /// Bims, who are no enemy of theirs. What the experience asks.
    fn first_enemy_body(&self) -> Option<usize> {
        let residents = self.residents.as_ref()?;
        if !self.aboard.is_joined() {
            return None;
        }
        if self.stance(residents.station) == Stance::Hostile {
            Some(0)
        } else if self.defense_here().is_some()
            && Some(residents.station) == self.ship.state.station()
        {
            Some(residents.aboard.room.crew_count() as usize)
        } else {
            None
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
}

fn refused(slot: u32, why: Refusal) -> WorldEvent {
    WorldEvent::Refused { slot, why }
}

/// A throw a crew member is walking out to make ([`Command::ThrowAt`]):
/// whose, a grenade or an EMP, the room tile it is for and the tile it
/// walks to to throw from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PendingThrow {
    pub who: u32,
    pub emp: bool,
    pub tile: (i32, i32),
    pub stand: (i32, i32),
}

/// How far inside a throw's reach the spot walked to has to be, in
/// tiles: a walk ends on the free cell nearest the spot, not its middle.
const THROW_STAND_MARGIN: f32 = 0.75;

/// The middle of a room tile, in room units.
fn tile_centre(tile: (i32, i32)) -> bims::math::Vec2 {
    let t = shipdesign::TILE as f32;
    bims::math::vec2((tile.0 as f32 + 0.5) * t, (tile.1 as f32 + 0.5) * t)
}

/// Whether an order to the room calls off a throw its Bim was walking
/// out to make: everything that moves it or sets it to something, and
/// not a selection, a recruit, the hands or a box on the Management tab.
fn calls_off_a_throw(order: bims::order::CrewOrder) -> bool {
    use bims::order::CrewOrder;
    !matches!(
        order,
        CrewOrder::Select { .. }
            | CrewOrder::SelectOwn
            | CrewOrder::Recruit
            | CrewOrder::WorkPriority { .. }
            | CrewOrder::Autonomous { .. }
            | CrewOrder::Hand { .. }
    )
}

/// The refusal a walk's code is, if it is one: the room's `ORDER_NOWHERE`
/// — see `bims::order`. `None` for a walk that went, or an order that was
/// not a walk.
fn walk_refusal(code: u32) -> Option<Refusal> {
    (code == bims::game::ORDER_NOWHERE).then_some(Refusal::NoWayThere)
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
/// opens under fire. And the system has an **asteroid belt** — which was
/// the mining site until the money rework (feature 95) took the mining
/// away, and is kept because the simulation's dock is what every pinned
/// hash and checksum in the workspace stands on: dropping the condition
/// would move the spawn to another station in another system and re-pin
/// the lot for nothing.
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

/// Minutes of the mission clock as whole steps of it (feature 103),
/// rounded to the nearest and never below nought: how a probe's dial in
/// minutes becomes the step count the world keeps a timer in.
fn steps_of(minutes: f64) -> u64 {
    (minutes / data::STEP_MINUTES).round().max(0.0) as u64
}

/// What the Republic pays for an enemy of that gear tier — [`data::REPUBLIC_BOUNTY`]
/// indexed safely, since a tier arrives from the room as a number
/// (feature 95). Nought for no tier at all.
pub fn bounty_for(tier: u32) -> Money {
    data::REPUBLIC_BOUNTY
        .get(tier as usize)
        .copied()
        .unwrap_or(0)
}

/// A hit as the picture is told it (`WorldEvent::Hit`): whole points, a
/// scratch under one said as one.
fn shown_hit(resident: bool, who: u32, damage: f32, crit: bool) -> WorldEvent {
    WorldEvent::Hit {
        resident,
        who,
        damage: damage.round().max(1.0) as u32,
        crit,
    }
}

/// How much of its tier's bounty an enemy is worth, in per cent: the weaker
/// of a tier [`data::BOUNTY_SPREAD_PERCENT`] less, the stronger as much
/// more, so a fight pays about what it did. A machine by its kind: a Husk,
/// claws and nothing else, the less; a Trooper the tier's own; a Warden
/// and a Guardian the more — a wave being half Troopers, a third Husks and
/// a sixth Wardens, it comes to about the tier's own.
pub fn droid_bounty_percent(kind: bims::droid::DroidKind) -> u32 {
    use bims::droid::DroidKind;
    match kind {
        DroidKind::Husk => 100 - data::BOUNTY_SPREAD_PERCENT,
        DroidKind::Trooper => 100,
        DroidKind::Warden | DroidKind::Guardian => 100 + data::BOUNTY_SPREAD_PERCENT,
        // The Machine Heart's own pay as a tier's.
        DroidKind::Core | DroidKind::Conduit | DroidKind::Fabricator => 100,
    }
}

/// One of the Manufacturers' worth in per cent, by what it carries: the
/// spread under the tier's own for a pistol and nothing worn, the spread
/// back for a better gun and again for any armour — so a better gun in
/// armour is the spread over.
pub fn manufacturer_bounty_percent(gear: &bims::combat::Gear) -> u32 {
    let step = data::BOUNTY_SPREAD_PERCENT;
    let gun = gear
        .weapon
        .is_some_and(|w| w.kind != bims::combat::WeaponKind::LaserPistol);
    let armour = gear.head.is_some() || gear.body.is_some() || gear.legs.is_some();
    100 - step + step * u32::from(gun) + step * u32::from(armour)
}

/// A bounty at `percent` of itself, rounded down to whole euros.
pub fn bounty_share(amount: Money, percent: u32) -> Money {
    amount.saturating_mul(Money::from(percent)) / 100
}

/// What the Republic pays for one of the Manufacturers' people down: its
/// gear tier's bounty at its gear's share.
fn manufacturer_bounty(
    rewards: &crate::rewards::Rewards,
    room: &bims::game::Game,
    who: usize,
) -> Money {
    bounty_share(
        rewards.bounty_for(gear_tier(room, who)),
        manufacturer_bounty_percent(&room.gear(who)),
    )
}

/// What tier of gear a body in `room` carries: the best of what is in its
/// hand and on its back, tier one for a body with nothing at all. What the
/// Republic's bounty is paid by.
fn gear_tier(room: &bims::game::Game, who: usize) -> u32 {
    let gear = room.gear(who);
    let worn = [gear.head, gear.body, gear.legs]
        .into_iter()
        .flatten()
        .map(|p| p.tier.code());
    gear.weapon
        .map(|w| w.tier.code())
        .into_iter()
        .chain(worn)
        .max()
        .unwrap_or(1)
}

/// What one unit of a resource is worth at a tier: its book value times
/// `economy::TIER_PRICE` (feature 95). The one place a tier touches a
/// valuation, the way `Quote::at_tier` is the one place it touches a
/// price.
fn gear_value(resource: ResourceId, tier: u32) -> Money {
    trade_price(resource).saturating_mul(economy::tier_price(tier))
}
