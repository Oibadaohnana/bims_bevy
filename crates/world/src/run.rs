//! The run (feature 103): the roguelike's loop, **world map → travel →
//! mission → back to ship → world map**, and the handful of things the
//! world keeps to walk it.
//!
//! # Two clocks, and only travel moves one of them
//!
//! `World::clock_minutes` is the **world clock**: the day, the crisis,
//! the front, the wages. In a run it moves **only when the crew travel**
//! — a trip is resolved rather than flown, and committing to one puts
//! the clock on by its length in one go, and everything that runs on
//! days is simply read at the new day (the crisis has no state, so a
//! skipped span spreads it exactly as the same days step by step would
//! have). It does not move during a mission and it does not move on the
//! map.
//!
//! [`Run::mission_steps`] is the **mission clock**: steps since the crew
//! arrived, nought on arrival. Everything inside a mission is timed by
//! it — the waves and their reinforcements, a town's first wave, the
//! class cooldowns — so a fight is the same fight whatever day it is
//! fought on. The room's own clocks (a routine's stops, a surge, a
//! grenade's fuse) are its steps already.
//!
//! There is no third: the old game's clock, which ran with every step and
//! flew the ship, went with the flying (feature 104).
//!
//! # What a mission is
//!
//! A mission begins on arrival at any site, peaceful or not — bar a
//! **trader** (task 114, [`Phase::Trade`]), which is visited entirely on
//! the map: no room, no mission, neither clock. At its start every crew member's
//! health is made whole, every charge is set to its start amount and
//! every cooldown is fresh, and every piece of armour is whole again
//! (task 113). The site's state
//! is photographed the first step the mission runs ([`SiteSnapshot`]),
//! so that leaving it **uncleared** puts it back exactly as it was met.
//!
//! **Cleared** is no machine left and none still to come: a held
//! station's last wave destroyed, a defended town held, and every other
//! site from the moment the crew arrive — there is nothing there to
//! clear. The Republic's bounty for a machine destroyed is **pending**
//! until the site is cleared, and paid into the pool then, once; left
//! uncleared, it is thrown away. Experience is always kept.
//!
//! # Ending a mission
//!
//! Every player has a *Back to ship* button ([`crate::Command::Return`]).
//! The first press sends every bot home. When every player's Bim that is
//! standing — not downed, not dead, not out, and still at the keyboard —
//! has pressed it **and** is inside the ship, the departure check runs:
//! everybody still alive outside the ship would be left behind, and if
//! anybody would, every connected player is asked ([`Departure`]). All
//! yes, and the ship leaves; one no, and it does not, the presses
//! standing. Left behind is dead.
//!
//! **Once the fight is won** (`World::fight_over`, task 133) the deck is
//! frozen — the room is not stepped, so nobody moves and nobody downed
//! bleeds out — and nobody walks home: the ship leaves when every player
//! alive, downed or not, has pressed *Back to ship*, and takes every crew
//! member alive with it from where it lies.
//!
//! # Dying
//!
//! A player's Bim that dies is **out** ([`Fallen`]) for the rest of the
//! mission and respawns when it ends, with its whole loadout,
//! class, level and talents, the pool paying
//! [`crate::data::BUYBACK_COST`] — or what it holds, down to nought, since
//! a respawn never waits for money (task 113). Gear is never lost. A
//! bot's is gone for good and costs nothing. The run is
//! over when every player's Bim is dead at once.

use economy::Money;

use crate::defense::Defense;
use crate::droid::Infestation;
use crate::memory::{Grave, Losses};
use crate::relic::Relics;
use crate::world::LampDamage;

/// What a site is to the crew (task 111): every site the map lists is
/// exactly one of the three. `World::site_kind` decides it for this
/// system and `TravelQuote::kind` says it on the map.
///
/// - **Attack**: an enemy holds it — the machines, the Manufacturers, or
///   the Machine Heart in its fortress — and the crew go in to clear it.
///   A site cleared is still one: it is where an attack was.
/// - **Defend**: every other site, derelicts included. The machines are
///   coming for it from the first day of a run, and the crew stand them
///   off; held, it is cleared.
/// - **Trader**: a trader site (task 114), visited on the map. Never
///   attacked, never taken by the crisis, never the jammer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SiteKind {
    Attack,
    Defend,
    Trader,
}

impl SiteKind {
    /// Every kind, in code order.
    pub const ALL: [SiteKind; 3] = [SiteKind::Attack, SiteKind::Defend, SiteKind::Trader];

    /// The number that crosses the seam and indexes the name tables.
    pub fn code(self) -> u32 {
        match self {
            SiteKind::Attack => 0,
            SiteKind::Defend => 1,
            SiteKind::Trader => 2,
        }
    }
}

/// What a fight at a station asks of the crew beyond its waves (October
/// 2026, `World::mission_at`): the plain fight, or one of the missions
/// the map shapes. Derived off the galaxy's seed, the star and the site,
/// never saved.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Mission {
    /// Every wave down: an attack cleared, a defence held.
    #[default]
    Plain,
    /// **Seal the breaches** (a defence): the machines come in by every
    /// way in left open, on a clock, until the crew have welded every one
    /// shut and destroyed what is aboard.
    Breaches,
    /// **Sabotage** (an attack): plant a charge deep in the site, hold it
    /// while the machines make to disarm it — disarmed, the run is lost —
    /// then get out by another airlock before it blows.
    Sabotage,
    /// **Evacuation** (a defence): the site's people and its refugees
    /// follow the flag a player carries — or stay by it where it is
    /// dropped — to the crew's ship; held once every one alive is
    /// aboard, the reward the share of them saved.
    Evacuation,
    /// **A nest hunt** (an attack): nests grown into the
    /// site's walls build a machine each on a clock until every one is
    /// destroyed; cleared with the nests and the deck.
    Nests,
}

impl Mission {
    /// Every one, in code order.
    pub const ALL: [Mission; 5] = [
        Mission::Plain,
        Mission::Breaches,
        Mission::Sabotage,
        Mission::Evacuation,
        Mission::Nests,
    ];

    /// The number that indexes the name tables.
    pub fn code(self) -> u32 {
        match self {
            Mission::Plain => 0,
            Mission::Breaches => 1,
            Mission::Sabotage => 2,
            Mission::Evacuation => 3,
            Mission::Nests => 4,
        }
    }
}

/// A mission's **bonus wave** (October 2026): chosen by the crew in the
/// ready check, before the fight starts, it is one more wave at the end
/// of the site's own — half as big again, worth half the site's
/// experience again and its machines' bounty. Kept on [`Run::bonus`];
/// saved, and hashed only where one was chosen.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BonusWave {
    /// Not chosen.
    #[default]
    None,
    /// Chosen: one wave more is to come after the site's own.
    Chosen,
    /// Landed: the bonus wave is on the deck, the site's last.
    Landed,
    /// Fought: the site cleared with it.
    Done,
}

impl BonusWave {
    /// The number that goes into the checksum.
    pub fn code(self) -> u32 {
        match self {
            BonusWave::None => 0,
            BonusWave::Chosen => 1,
            BonusWave::Landed => 2,
            BonusWave::Done => 3,
        }
    }

    /// Whether one was chosen this mission.
    pub fn chosen(self) -> bool {
        self != BonusWave::None
    }
}

/// Where the run stands.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Phase {
    /// At a site, the mission clock running. What a world opens in.
    #[default]
    Mission,
    /// Between missions: the world map up for everybody, nothing moving,
    /// a destination being chosen.
    Map,
    /// Between missions, before the map (feature 106): the site just left
    /// was an elite's, cleared, and the crew are choosing together which
    /// of the relics it offers to take, if any
    /// ([`crate::relic::RelicChoice`]). Nothing moves, as on the map; the
    /// map comes up when the choice is made.
    Reward,
    /// At a trader (task 114): arrived, and the whole visit on the map.
    /// No room is loaded and no mission runs; nothing moves and neither
    /// clock runs, as on the map. The trader's shelf and the combining are
    /// open ([`crate::trader`]), and the vote on where next
    /// works as it does on the map: carried, the crew leave and travel.
    Trade,
}

impl Phase {
    /// The number that crosses the seam and goes into the checksum.
    pub fn code(self) -> u32 {
        match self {
            Phase::Mission => 0,
            Phase::Map => 1,
            Phase::Reward => 2,
            Phase::Trade => 3,
        }
    }
}

/// A place the crew can travel to: a station or a planet's settlement,
/// by its star and its station id in that star's system — a settlement
/// by `crate::surface::surface_id` of its body, a derived jammer by
/// `crate::jammer::jammer_id` of its star.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Site {
    pub star: u32,
    pub station: u32,
}

/// A destination put to the crew: which, who put it, and who has said
/// yes. Every connected player has to; the one who put it is counted as
/// having done so, and any change — another destination put — starts the
/// count again.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Proposal {
    pub site: Site,
    /// The player slot that put it.
    pub by: u32,
    /// One a player slot: whether that player has accepted it.
    pub accepted: Vec<bool>,
}

impl Proposal {
    /// A destination put by `by`, who accepts it by putting it.
    pub fn new(site: Site, by: u32, players: u32) -> Proposal {
        let mut accepted = vec![false; players as usize];
        if let Some(a) = accepted.get_mut(by as usize) {
            *a = true;
        }
        Proposal { site, by, accepted }
    }

    /// Whether every player still at the keyboard has said yes.
    /// `connected` is one a slot; a slot past it counts as connected.
    pub fn carried(&self, connected: &[bool]) -> bool {
        self.accepted
            .iter()
            .enumerate()
            .all(|(slot, &yes)| yes || !connected.get(slot).copied().unwrap_or(true))
    }
}

/// The departure check, while it is asking or after it was turned down.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Departure {
    /// Everybody who would be left behind — crew indices, players' and
    /// bots' — and each player's answer so far: `None` not yet said.
    Asking {
        behind: Vec<u32>,
        answers: Vec<Option<bool>>,
    },
    /// Somebody said no to leaving **these** behind. The check does not
    /// ask again about the same list — the presses stand, and it would
    /// ask for ever — but a different list is a different question, and
    /// a player pressing *Back to ship* again asks it again.
    Declined { behind: Vec<u32> },
}

impl Departure {
    /// A fresh question about `behind`, nobody's answer in yet.
    pub fn ask(behind: Vec<u32>, players: u32) -> Departure {
        Departure::Asking {
            behind,
            answers: vec![None; players as usize],
        }
    }

    /// Everybody it is asking about, or declined about.
    pub fn behind(&self) -> &[u32] {
        match self {
            Departure::Asking { behind, .. } | Departure::Declined { behind } => behind,
        }
    }
}

/// A player's Bim that is dead and waiting for the mission's end, and when
/// it fell, so the longest dead go first.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Fallen {
    pub slot: u32,
    /// The run's death count when it fell: only ever climbs, so a lower
    /// one fell first.
    pub order: u64,
}

/// A site as it stood the first step of a mission: everything about it
/// the world keeps, so that leaving it uncleared puts it back exactly.
/// What the crew carried away is theirs — a joiner — and is not
/// here: putting the site back is not taking back what they took.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SiteSnapshot {
    pub station: u32,
    pub infestation: Option<Infestation>,
    pub defense: Option<Defense>,
    pub losses: Option<Losses>,
    pub graves: Vec<Grave>,
    pub lamps: Vec<LampDamage>,
}

/// Everything the run keeps. Saved and in `world_checksum` whole.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Run {
    pub phase: Phase,
    /// The mission clock: steps since the crew arrived.
    pub mission_steps: u64,
    /// How many missions have begun, the first — the one a world opens
    /// in — counted.
    pub missions: u32,
    /// The station the crew are at: the mission's site, or the one they
    /// have just left and are choosing from. What a trip in this system
    /// starts from.
    pub site: Option<u32>,
    /// The site as the mission met it, taken the first step it ran:
    /// `None` until then and at a site with nothing to put back.
    pub snapshot: Option<SiteSnapshot>,
    /// Whether this mission's first step has run and the photograph
    /// above been taken (or found to be nothing).
    pub snapped: bool,
    /// The Republic's bounty earned at this site and not yet paid: it is
    /// paid the step the site is cleared, and thrown away if the crew
    /// leave before.
    pub pending_bounty: Money,
    /// The destination on the table, between missions.
    pub proposal: Option<Proposal>,
    /// One a player slot: pressed *Back to ship* this mission.
    pub returning: Vec<bool>,
    /// Whether the bots have been sent home: the first press does it.
    pub recalled: bool,
    pub departure: Option<Departure>,
    /// One a player slot: still at the keyboard. The host says when one
    /// goes ([`crate::Command::PlayerGone`]), and a vote does not wait for
    /// a player who is not there.
    pub connected: Vec<bool>,
    /// Dead players waiting for the mission's end, the longest dead first.
    pub fallen: Vec<Fallen>,
    /// Deaths so far: what [`Fallen::order`] is stamped from.
    pub deaths: u64,
    /// The relics (feature 106, [`crate::relic`]): what the crew hold and
    /// the choice being made.
    pub relics: Relics,
    /// Whether this mission's site had machines to clear when the mission
    /// met it: what makes its clear worth a relic.
    pub fought: bool,
    /// Whether this mission's site has been cleared since the mission met
    /// it uncleared — said once, the step it happens.
    pub cleared_here: bool,
    /// Whether the run is **won** ([`crate::World::run_won`]): said once
    /// and kept, as the loss is.
    pub won: bool,
    /// The probes' dial (`BIMS_WIN=1`): the run is won the next time a
    /// site is cleared with machines in it — the victory screen without the
    /// Machine Heart's fight (feature 108) in front of it.
    pub win_on_clear: bool,
    /// The run in numbers, for the victory screen (feature 108): every
    /// machine destroyed, every site cleared of machines, and every
    /// system **liberated** — its jammer cleared, so its lanes inward are
    /// open for good.
    #[cfg_attr(feature = "serde", serde(default))]
    pub machines_destroyed: u32,
    #[cfg_attr(feature = "serde", serde(default))]
    pub sites_cleared: u32,
    #[cfg_attr(feature = "serde", serde(default))]
    pub systems_liberated: u32,
    /// Every trader the crew have been to this run (task 114,
    /// [`crate::trader::Trader`]): what is left on its shelf, rolled again every
    /// visit. Sorted by site.
    #[cfg_attr(feature = "serde", serde(default))]
    pub traders: Vec<crate::trader::Trader>,
    /// The best tier of each kind each player slot has bought off a shelf
    /// this run, a kind once, sorted by resource (October 2026): its
    /// shelves sell that kind one tier past it from then on
    /// (`World::shelf_tier`). Hashed with the traders, where any is set.
    #[cfg_attr(feature = "serde", serde(default))]
    pub shelf_bought: Vec<Vec<crate::trader::ShelfItem>>,
    /// The items' clocks (October 2026, [`crate::items::ItemClocks`]).
    #[cfg_attr(feature = "serde", serde(default))]
    pub items: crate::items::ItemClocks,
    /// The ready check's switch: on, a mission with a fight in it — an
    /// Attack site not yet cleared, a Defend site threatened — opens
    /// held, nothing moving, until every connected player has pressed
    /// *Ready* ([`crate::Command::Ready`]). Off in `World::start`, so the
    /// tests' and the staged commands' worlds step at once; the `game`
    /// run switches it on (`World::set_ready_check`).
    #[cfg_attr(feature = "serde", serde(default))]
    pub ready_check: bool,
    /// Whether this mission is held for the ready check: the room stands
    /// as it was met and the mission clock waits at nought.
    #[cfg_attr(feature = "serde", serde(default))]
    pub briefing: bool,
    /// One a player slot: pressed *Ready* for this mission.
    #[cfg_attr(feature = "serde", serde(default))]
    pub ready: Vec<bool>,
    /// The fight chosen in each system (task 135): the Attack or Defend
    /// site a mission was begun at, one a star, sorted. The other fight of
    /// that system is refused for the rest of the run
    /// ([`crate::Refusal::OtherSiteChosen`]).
    #[cfg_attr(feature = "serde", serde(default))]
    pub chosen: Vec<Site>,
    /// The floor's switch (October 2026, [`crate::floor`]): on, the map is
    /// the floor — a trip goes only up it, to a place the place the crew
    /// are at leads to, and a row is a day. Off in `World::start`, so the
    /// tests' worlds travel the galaxy's lanes as they always did; every
    /// session of the app switches it on (`World::set_floor`).
    #[cfg_attr(feature = "serde", serde(default))]
    pub floor: bool,
    /// The size of this mission's first wave, once it has landed (October
    /// 2026): every wave after it is as many, whoever of the crew has fallen
    /// since — a bot or a defender dead still counts. `None` before the
    /// first, and again at every mission's start.
    #[cfg_attr(feature = "serde", serde(default))]
    pub wave_size: Option<u32>,
    /// The experience an enemy of the wave on the deck pays every classed
    /// crew member in range (October 2026, `World::price_the_wave`): the
    /// site's budget for the day over its waves and the wave's bodies —
    /// before the relics, the items and the catch-up put theirs on.
    /// `None` before the first wave of a mission is laid, and again at
    /// every mission's start.
    #[cfg_attr(feature = "serde", serde(default))]
    pub xp_each: Option<u32>,
    /// And the money each pays, the same way off the site's money budget
    /// (October 2026), before who took it down, its kind and the site's
    /// shares.
    #[cfg_attr(feature = "serde", serde(default))]
    pub money_each: Option<Money>,
    /// *Clean Sweep*'s book (October 2026): the experience each crew
    /// member earned at this site since its last clear, by crew index...
    #[cfg_attr(feature = "serde", serde(default))]
    pub clean_xp: Vec<u32>,
    /// ...and whether a player's Bim went down since.
    #[cfg_attr(feature = "serde", serde(default))]
    pub clean_spoiled: bool,
    /// This mission's bonus wave (October 2026, [`BonusWave`]): chosen in
    /// the ready check, none at every mission's start.
    #[cfg_attr(feature = "serde", serde(default))]
    pub bonus: BonusWave,
    /// The ways in this mission's site has welded shut (October 2026,
    /// `World::weld`): each an airlock's index among the design's
    /// airlocks or a gate's among the town's. A wave skips one welded and
    /// comes in by the next in turn; with every one welded it burns
    /// through the one its turn names. None at every mission's start.
    #[cfg_attr(feature = "serde", serde(default))]
    pub welded: Vec<u32>,
    /// The welding under way (October 2026): each way in a hand has been
    /// put to and the steps of work it has had (`World::settle_welds`).
    /// Cleared with `welded`.
    #[cfg_attr(feature = "serde", serde(default))]
    pub weld_work: Vec<(u32, u32)>,
    /// The welds a wave burnt through this step, for the step to say
    /// (`World::say_burns`); empty between steps.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub burnt: Vec<u32>,
}

impl Run {
    /// A run opening in its first mission, for `players`.
    pub fn new(players: u32) -> Run {
        Run {
            phase: Phase::Mission,
            mission_steps: 0,
            missions: 1,
            site: None,
            snapshot: None,
            snapped: false,
            pending_bounty: 0,
            proposal: None,
            returning: vec![false; players as usize],
            recalled: false,
            departure: None,
            connected: vec![true; players as usize],
            fallen: Vec::new(),
            deaths: 0,
            relics: Relics::default(),
            fought: false,
            cleared_here: false,
            won: false,
            win_on_clear: false,
            machines_destroyed: 0,
            sites_cleared: 0,
            systems_liberated: 0,
            traders: Vec::new(),
            shelf_bought: Vec::new(),
            items: crate::items::ItemClocks::default(),
            ready_check: false,
            briefing: false,
            ready: vec![false; players as usize],
            chosen: Vec::new(),
            floor: false,
            wave_size: None,
            xp_each: None,
            money_each: None,
            clean_xp: Vec::new(),
            clean_spoiled: false,
            bonus: BonusWave::None,
            welded: Vec::new(),
            weld_work: Vec::new(),
            burnt: Vec::new(),
        }
    }

    /// Whether that player is still at the keyboard.
    pub fn is_connected(&self, slot: u32) -> bool {
        self.connected.get(slot as usize).copied().unwrap_or(true)
    }

    /// Whether that player's Bim is dead and waiting for the mission's end.
    pub fn is_out(&self, slot: u32) -> bool {
        self.fallen.iter().any(|f| f.slot == slot)
    }

    /// Whether that player has pressed *Back to ship* this mission.
    pub fn is_returning(&self, slot: u32) -> bool {
        self.returning.get(slot as usize).copied().unwrap_or(false)
    }

    /// Whether that player has pressed *Ready* for this mission.
    pub fn is_ready(&self, slot: u32) -> bool {
        self.ready.get(slot as usize).copied().unwrap_or(false)
    }
}

/// What a trip to a site would be, worked out before anybody commits to
/// it: how long, when the crew would get there, and what they would find.
/// [`crate::World::travel_quote`].
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TravelQuote {
    pub site: Site,
    /// Whether it is a jump to another system.
    pub jump: bool,
    /// How many hyperlanes it crosses (the second map rework): nought within the
    /// system, one or two for a jump ([`crate::data::MAX_TRIP_HOPS`]).
    pub hops: u32,
    /// How long it takes, in days: a day for a jump
    /// ([`crate::data::JUMP_MINUTES`]) and nought within a system (the map
    /// rework).
    pub days: f64,
    /// The same in whole minutes, rounded up: what the world clock is put
    /// on by.
    pub minutes: u64,
    /// The day the crew arrive on, by `World::days_gone` — the day the
    /// crisis is read at.
    pub arrival_day: u32,
    /// The same day as the crew's calendar has it — what the clock panel
    /// will show on arrival (`bims::clock::day_at`), which is what the map
    /// says.
    pub arrival_date: u32,
    /// Whether the machines hold the site's system by then.
    pub infested: bool,
    /// What tier the machines there come at.
    pub tier: bims::combat::Tier,
    /// Whether the site is that system's jammer.
    pub jammer: bool,
    /// Whether a defence starts on arrival (task 111): a defence site
    /// whose fight is not over and which is not a town held — any such
    /// site, from the first day; a town on the front, until then.
    pub threatened: bool,
    /// Whether the site is somewhere the crew have already cleared or
    /// held: an attack cleared, or a defence won.
    pub cleared: bool,
    /// Whether the site is the Manufacturers' (feature 109): their people
    /// on the deck, and [`TravelQuote::tier`] what they will carry.
    pub manufacturers: bool,
    /// Whether the site is its system's **elite** (`crate::elite`): the
    /// machines', at least two waves, a Guardian in the second, and relics
    /// on the clear.
    pub elite: bool,
    /// Whether the site is a **trader** (task 114): the visit is on the
    /// map, no mission. A trader closed now or on arrival is no quote at
    /// all but a refusal (`Refusal::TraderClosed`,
    /// `Refusal::ClosedOnArrival`).
    pub trader: bool,
    /// What the site is on arrival (task 111): an attack, a defence or a
    /// trader, and exactly one of them. What the map leads each row with.
    pub kind: SiteKind,
    /// At the Machine Heart's fortress (feature 108), what the crew would
    /// meet on arrival: the conduits, the core, and the waves at the
    /// arrival day. `None` at every other site.
    pub heart: Option<crate::heart::HeartPreview>,
}

/// A star's system as the map draws it (the map rework,
/// [`crate::World::system_look`]): what it offers, and every site of it.
#[derive(Clone, Debug)]
pub struct SystemLook {
    pub system: worldgen::StarSystem,
    pub sites: Vec<SiteLook>,
}

/// One site of a [`SystemLook`]: where it lies in its system and what it
/// would be on arrival — a quote with every question about whether the
/// crew may go there left unasked.
#[derive(Clone, Copy, Debug)]
pub struct SiteLook {
    pub site: Site,
    pub at: worldgen::math::DVec2,
    pub quote: TravelQuote,
}

/// A star's one mission — or its trader — as the galaxy chart marks it
/// (the galaxy-only map, [`crate::World::star_missions`]): the site, what
/// it is today, and whether its fight is over.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StarMission {
    pub site: Site,
    pub kind: SiteKind,
    pub cleared: bool,
    /// A defence that is an Area defend (October 2026).
    pub area: bool,
    /// What the fight asks beyond its waves (October 2026).
    pub mission: Mission,
}
