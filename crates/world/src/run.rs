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
//! [`Run::free_clock`] is the old game's switch, **off in every run**
//! and on only for the tests of what the free-running clock did — flight,
//! raids, the day passing by the step — in the pattern feature 102 set:
//! a world of the old game turns it on right after it is built.
//!
//! # What a mission is
//!
//! A mission begins on arrival at any site, peaceful or not: a trader
//! visit is a mission without a fight. At its start every crew member's
//! health is made whole, every class charge and cooldown is fresh, and
//! the dead players the pool can pay for are bought back
//! ([`crate::data::BUYBACK_COST`], longest dead first). The site's state
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
//! # Dying
//!
//! A player's Bim that dies is **out** ([`Fallen`]) until bought back,
//! its gear lost with the body and its class, level and talents kept. A
//! bot's is gone for good and costs the pool
//! [`crate::data::BOT_DEATH_PENALTY`], never below nought. The run is
//! over when every player's Bim is dead at once.

use economy::Money;

use crate::defense::Defense;
use crate::droid::Infestation;
use crate::memory::{Grave, Losses};
use crate::plunder::Plunder;
use crate::world::LampDamage;

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
}

impl Phase {
    /// The number that crosses the seam and goes into the checksum.
    pub fn code(self) -> u32 {
        match self {
            Phase::Mission => 0,
            Phase::Map => 1,
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

/// A player's Bim that is dead and waiting to be bought back, and when
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
/// What the crew carried away is theirs — a key, a hire — and is not
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
    pub plunder: Option<Plunder>,
}

/// Everything the run keeps. Saved and in `world_checksum` whole.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Run {
    pub phase: Phase,
    /// The old game's switch: the world clock running with every step,
    /// as it did before feature 103. Off in every run.
    pub free_clock: bool,
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
    /// Dead players waiting to be bought back, the longest dead first.
    pub fallen: Vec<Fallen>,
    /// Deaths so far: what [`Fallen::order`] is stamped from.
    pub deaths: u64,
}

impl Run {
    /// A run opening in its first mission, for `players`.
    pub fn new(players: u32) -> Run {
        Run {
            phase: Phase::Mission,
            free_clock: false,
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
        }
    }

    /// Whether that player is still at the keyboard.
    pub fn is_connected(&self, slot: u32) -> bool {
        self.connected.get(slot as usize).copied().unwrap_or(true)
    }

    /// Whether that player's Bim is dead and waiting to be bought back.
    pub fn is_out(&self, slot: u32) -> bool {
        self.fallen.iter().any(|f| f.slot == slot)
    }

    /// Whether that player has pressed *Back to ship* this mission.
    pub fn is_returning(&self, slot: u32) -> bool {
        self.returning.get(slot as usize).copied().unwrap_or(false)
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
    /// How long it takes, in days: [`crate::data::JUMP_CHARGE_MINUTES`]
    /// for a jump, and `physics::travel_days` of the leg in the system —
    /// from the site the crew are at, or from where a jump lands them.
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
    /// Whether the site is a town the machines will be coming for.
    pub threatened: bool,
    /// Whether the site is somewhere the crew have already cleared or
    /// held.
    pub cleared: bool,
}
