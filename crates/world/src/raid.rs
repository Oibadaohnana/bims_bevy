//! Raiders: a hostile ship that docks to you.
//!
//! Everything a hostile dock does — its people enemies, the two rooms
//! joined through the mated airlocks, the fight crossing between them,
//! a body looted where it fell — happens to the crew at a station they
//! chose to visit. A raid is the same thing arriving: a **raider** is a
//! [`Station`] the world builds when the raid is due, with an id of its
//! own ([`raider_id`], well clear of the generator's and the surfaces'),
//! [`Plan::Raider`] for a hull and `Stance::Hostile` from the moment it
//! exists, and on arrival the ship's state becomes `Docked` at it and
//! `join_rooms` runs as at any berth. Downstream of that nothing knows
//! the difference, which is the point.
//!
//! # When
//!
//! A schedule on the world's clock, in whole minutes, off a stream of its
//! own ([`Purpose::Raids`], from the galaxy seed alone — a raid follows
//! the ship, not a system): each raid falls due a gap after the last
//! ([`Raids::gap`]), and comes at the first step the ship is **holding**
//! on or after that minute — docked, under way, casting off, pushing off,
//! coming alongside and charging a jump are never raided, and one that
//! falls due under way waits for the next hold. None before the crew have
//! left their start station for the first time. Two worlds on one seed
//! that hold at the same minutes are raided at the same minutes with the
//! same boarders; the whole of it is in `world_checksum`.
//!
//! # Contact, and the way in
//!
//! At contact the raider is on the radar at the edge of the ship's range
//! ([`World::detection_range`]) on a rolled bearing, and closes on the
//! ship in a straight line at [`data::RAIDER_SPEED`], so the warning
//! grows with the sensors aboard. The world says so
//! (`WorldEvent::RaidContact`) and puts **every player's speed request
//! back to 1×**, once — a reset, not a veto: anybody may raise it again.
//! If the ship is not holding when the raider arrives — it left, or it
//! jumped — the raid is cancelled. Arrived, the raider is set down so that
//! its berth for this ship is where the ship is, and the ship is docked
//! to it; the boarders are posted at the ship's gangway, the deck just
//! inside its airlock, and come through the passage after it
//! (`Residents::post_boarders`), forcing the airlock if it is locked
//! against them (`bims::game::Game::breach`).
//!
//! # How many, and the end
//!
//! [`boarders_of`]: one, one a month the game has run and one a crew
//! member, doubled with the crew's worth the way a station's garrison is,
//! capped at [`data::BOARDERS_MAX`].
//! Every boarder down and the raider is a derelict tied to the ship
//! (`WorldEvent::RaidRepelled`): the crew loot the bodies as at any hostile
//! dock, and its shelf with them (`crate::plunder`), and casting off
//! removes it — the raider is gone from the world the moment the ship
//! pushes off, and never seen again. No crew member
//! standing, and the run is over (`WorldEvent::CrewLost`), whatever put
//! them down.

use shipdesign::parts::TILE;
use worldgen::math::{DVec2, dvec2};
use worldgen::rng::{Purpose, Rng, mix, seed_for};
use worldgen::{StationKind, Stock};

use crate::data;
use crate::station::{Plan, Station, base_by_day, layout_raider, scaled};
use crate::surface::SURFACE_BASE;

/// The bit that marks a station id as a raider's. Clear of the
/// generator's ids and of [`SURFACE_BASE`].
pub const RAIDER_BASE: u32 = 0x2000_0000;

/// The station id of the `n`th raider.
pub fn raider_id(n: u32) -> u32 {
    RAIDER_BASE | n
}

/// Which raid a station id names, if it names one.
pub fn raider_index(id: u32) -> Option<u32> {
    (id & RAIDER_BASE != 0 && id & SURFACE_BASE == 0).then_some(id & !RAIDER_BASE)
}

/// The kind a raider is built as: an orbital's, like a settlement, for
/// what its shelf holds. Its plan is [`Plan::Raider`] whatever this says.
pub const RAIDER_KIND: StationKind = StationKind::Orbital;

/// How many boarders a raider carries against a crew of `crew` whose ship
/// is worth `worth` now and was worth `start_worth` at the start, `days`
/// whole days ago: one, one a month gone (`crate::station::base_by_day`)
/// and one a crewmate, doubled every half of the start's worth the crew
/// have grown by, and never more than [`data::BOARDERS_MAX`] — the
/// raider has a bunk for each and a ship's deck is not an arena. The
/// same arithmetic as a station's garrison (`crate::station::enemies_of`)
/// to a lower cap.
pub fn boarders_of(
    crew: u32,
    worth: economy::Money,
    start_worth: economy::Money,
    days: u32,
) -> u32 {
    scaled(
        base_by_day(days).saturating_add(crew),
        worth,
        start_worth,
        data::BOARDERS_MAX,
    )
}

/// Where a raid is.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Raid {
    /// None under way.
    Quiet,
    /// A raider on the radar, closing: raid `n` with `boarders` aboard,
    /// from `from` at `began` on the clock to `at` — where the ship was
    /// holding at contact — at `arrives`.
    Closing {
        n: u32,
        boarders: u32,
        from: DVec2,
        at: DVec2,
        began: f64,
        arrives: f64,
    },
    /// The raider tied to the ship, with `boarders` aboard it — a
    /// derelict once every one of them is down (`repelled`).
    Docked {
        station: Station,
        boarders: u32,
        repelled: bool,
    },
}

/// The raids: the schedule, and the one under way.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Raids {
    /// The next raid's number: how many have come.
    pub next: u32,
    /// The whole minute of the world's clock the next raid falls due at.
    pub due: u64,
    /// Whether the crew have left their start station yet: no raid
    /// before they have.
    pub left_home: bool,
    /// The raid under way, if any.
    pub state: Raid,
}

impl Raids {
    /// The schedule from the start: the first raid due a gap after the
    /// clock's nought.
    pub fn new(galaxy_seed: u64) -> Raids {
        Raids {
            next: 0,
            due: Raids::gap(galaxy_seed, 0),
            left_home: false,
            state: Raid::Quiet,
        }
    }

    /// The stream raid `n` is rolled off: the galaxy's raid stream, mixed
    /// with the number, so a change to one raid's rolls moves no other's.
    fn stream(galaxy_seed: u64, n: u32) -> Rng {
        let base = seed_for(galaxy_seed, 0, worldgen::GENERATOR_VERSION, Purpose::Raids);
        Rng::new(base ^ mix(n as u64))
    }

    /// How long after the last raid — or the start — raid `n` falls due,
    /// in whole minutes: [`data::RAID_GAP_MIN`] and a roll below
    /// [`data::RAID_GAP_SPREAD`] more, off branch "GAP".
    pub fn gap(galaxy_seed: u64, n: u32) -> u64 {
        let roll = Raids::stream(galaxy_seed, n)
            .branch(0x_4741_5000_0000_0000)
            .below(data::RAID_GAP_SPREAD as u32) as u64;
        data::RAID_GAP_MIN + roll
    }

    /// The bearing raid `n` comes in on, in whole degrees off branch
    /// "BEAR", as a unit vector.
    pub fn bearing(galaxy_seed: u64, n: u32) -> DVec2 {
        let degrees = Raids::stream(galaxy_seed, n)
            .branch(0x_4245_4152_0000_0000)
            .below(360);
        let angle = (degrees as f64).to_radians();
        dvec2(angle.cos(), angle.sin())
    }

    /// Raid `n`'s hull seed, off branch "HULL", and its shelf, off "STOC".
    pub fn hull(galaxy_seed: u64, n: u32) -> (u64, Stock) {
        let stream = Raids::stream(galaxy_seed, n);
        let map_seed = stream.branch(0x_4855_4c4c_0000_0000).next_u64();
        let stock = Stock::roll(RAIDER_KIND, &mut stream.branch(0x_5354_4f43_0000_0000));
        (map_seed, stock)
    }

    /// The raider the raid under way is tied to the ship as, if it is.
    pub fn station(&self) -> Option<&Station> {
        match &self.state {
            Raid::Docked { station, .. } => Some(station),
            _ => None,
        }
    }

    /// Where the raider is while it closes: on the line from `from` to
    /// `at`, as far along as the clock says. `None` unless one is closing.
    pub fn contact(&self, clock_minutes: f64) -> Option<DVec2> {
        let Raid::Closing {
            from,
            at,
            began,
            arrives,
            ..
        } = &self.state
        else {
            return None;
        };
        let span = (arrives - began).max(1e-9);
        let f = ((clock_minutes - began) / span).clamp(0.0, 1.0);
        Some(from.add(at.sub(*from).scale(f)))
    }

    /// Build raid `n`'s raider so that its berth for `ship` — its centre
    /// of mass at `centre_of_mass` — is exactly `berth`: the ship, holding
    /// there, is docked to it without moving. `boarders` live aboard.
    pub fn build_raider(
        galaxy_seed: u64,
        n: u32,
        boarders: u32,
        ship: &shipdesign::ShipDesign,
        centre_of_mass: DVec2,
        berth: DVec2,
    ) -> Station {
        let (map_seed, stock) = Raids::hull(galaxy_seed, n);
        let design = layout_raider(map_seed);
        let mut station = Station {
            id: raider_id(n),
            kind: RAIDER_KIND,
            plan: Plan::Raider,
            anchor: DVec2::ZERO,
            design,
            population: boarders,
            map_seed,
            stock,
            bias: economy::market::Bias::NONE,
            hostile: true,
            key: false,
        };
        // The berth is the anchor and a constant, so the anchor that puts
        // the berth on the ship is the ship less the berth at nought.
        if let Some(at_zero) = station.berth(ship, centre_of_mass) {
            station.anchor = berth.sub(at_zero.position);
        } else {
            let half = station.design.build_area as f64 * TILE as f64 / 2.0;
            station.anchor = berth.sub(dvec2(half, half));
        }
        station
    }
}
