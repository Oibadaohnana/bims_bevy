//! The numbers the world is generated from.
//!
//! All placeholders, and all load-bearing. Every one of them either decides
//! what a seed produces or decides whether a layout is legal, so the list at
//! the bottom of this comment is not a formality: change any of these and the
//! same seed gives a different galaxy, which means saves and lobbies
//! disagreeing about where anything is.
//!
//! **A change to any of the following is a [`crate::GENERATOR_VERSION`] bump:**
//! [`TRAVEL_BAND`], [`REFERENCE_SHIP`] and anything it is built from —
//! `physics`'s `PLAYER_MASS` and the resource masses it carries — the day
//! length in the `time` crate, the travel-time formula itself, and the
//! desolation mapping below.
//!
//! The station shares, the salvage, the hazards, the shelves and the price
//! biases are *not* on that list on purpose: they change what is in a
//! system without changing whether a layout passes validation, so they can
//! be tuned while the world stays the shape it was — the checksum moves,
//! the version does not.

use economy::market::{Bias, MAX_BIAS};
use physics::{EngineSpec, Facing, Mass, ResourceId};

/// How far apart things in a system are allowed to be, **in days for the
/// reference ship**. Not in distance: a distance means nothing to a player
/// and everything to a generator, and days are what the constraint is
/// actually about — a system you can cross in an afternoon has no geography,
/// and one that takes a month to cross has too much.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TravelBand {
    pub min_days: f64,
    pub max_days: f64,
}

pub const TRAVEL_BAND: TravelBand = TravelBand {
    min_days: 1.0,
    max_days: 14.0,
};

/// The ship every layout is measured against.
///
/// **Fixed.** Not the player's ship, not derived from the build area or from
/// how many people are in the lobby — the world has to come out the same for
/// a seed whatever the lobby settings say, and a reference ship that moved
/// with them would quietly make a four-player galaxy a different galaxy.
///
/// The values are chosen so the derived forward acceleration is exactly
/// `1.0`: two engines of a thousand against two thousand of mass. That is
/// arbitrary and it is also convenient — it makes a one-day hop 518400 world
/// units, which is a number that can be held in the head while reading a
/// layout dump.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReferenceShip {
    pub engine_count: u32,
    pub engine_thrust: f64,
    pub facing: Facing,
    pub hull_mass: f64,
    pub crew_count: u32,
    /// Carried as a manifest rather than as a lump of mass so it goes through
    /// the same `physics::ship_mass` every other ship does. A second way of
    /// weighing a ship is a second thing to keep in step.
    pub cargo: [(ResourceId, u32); 1],
    /// The reference ship turns round to brake, so its deceleration is its
    /// acceleration. **If the flight step ever forbids rotation this becomes
    /// false, every hop in every system gets longer, and the generator
    /// version has to move with it.**
    pub can_flip: bool,
}

pub const REFERENCE_SHIP: ReferenceShip = ReferenceShip {
    engine_count: 2,
    engine_thrust: 1000.0,
    facing: Facing::Forward,
    hull_mass: 1000.0,
    crew_count: 2,
    cargo: [(ResourceId::LegGuard, 100)],
    can_flip: true,
};

impl ReferenceShip {
    /// What it weighs, through the ordinary mass function.
    ///
    /// Panics if the constants above do not describe a ship that can exist.
    /// That is deliberate: this is a compile-time-ish fact about a constant,
    /// not a runtime condition, and a generator that quietly carried on with
    /// a nonsense reference would produce a nonsense galaxy in silence.
    pub fn mass(&self) -> Mass {
        physics::ship_mass(self.hull_mass, &self.cargo, self.crew_count)
            .expect("the reference ship's constants must describe a real ship")
    }

    pub fn engines(&self) -> Vec<EngineSpec> {
        (0..self.engine_count)
            .map(|_| EngineSpec {
                thrust: self.engine_thrust,
                facing: self.facing,
            })
            .collect()
    }

    /// Acceleration along the axis it thrusts on. **Derived, never stored** —
    /// storing it would be a second copy of the masses to keep in step, and
    /// the whole reason the physics contract is a crate is to not have those.
    pub fn acceleration(&self) -> f64 {
        physics::axis_acceleration(&self.engines(), self.mass(), self.facing)
    }

    /// What it slows down with. The same, while it can turn round; nothing at
    /// all if it cannot and has no engine facing backwards — and a ship that
    /// cannot stop gets no travel time, which is exactly what should happen.
    pub fn deceleration(&self) -> f64 {
        if self.can_flip {
            self.acceleration()
        } else {
            physics::axis_acceleration(&self.engines(), self.mass(), Facing::Backward)
        }
    }
}

/// How long the reference ship takes to cross `distance`, in days. The one
/// question the layout checks ask.
pub fn reference_days(distance: f64) -> Option<f64> {
    let ship = REFERENCE_SHIP;
    physics::travel_days(distance, ship.acceleration(), ship.deceleration())
}

/// How far apart two things have to be for the reference ship to take `days`
/// over the trip. The one answer the layout *builder* needs.
pub fn reference_distance(days: f64) -> Option<f64> {
    let ship = REFERENCE_SHIP;
    physics::travel_distance(days, ship.acceleration(), ship.deceleration())
}

// --- how run-down a system is ------------------------------------------

/// Desolation is drawn from this shape. Raising a uniform draw to a power
/// above one leans it towards zero, so most systems are middling and the
/// genuinely abandoned ones are rare — which is what makes finding one worth
/// anything.
const DESOLATION_SKEW: f64 = 1.6;

/// Draw a system's desolation. In `[0, 1]`, **stored and never displayed**:
/// it is a generator input, and a number on the screen saying how bad a place
/// is would do the exploring for the player.
pub fn desolation(u: f64) -> f64 {
    u.clamp(0.0, 1.0).powf(DESOLATION_SKEW)
}

/// What desolation means for how spread out a system is: nothing at all at
/// zero, and the full span at one.
///
/// This is the *target* hop, not the hop. The actual distance between two
/// neighbours is drawn between the minimum and this, so a desolate system is
/// on average emptier rather than uniformly stretched.
pub fn target_hop_days(desolation: f64) -> f64 {
    let TravelBand { min_days, max_days } = TRAVEL_BAND;
    let d = desolation.clamp(0.0, 1.0);
    (min_days + (max_days - min_days) * d).min(max_days)
}

// --- stations -----------------------------------------------------------

/// What fraction of systems have a station at all. A target rather than a
/// quota — each system rolls against it on its own. It was a quarter, and
/// the galaxy read as empty: a map where most stars are somewhere you cannot
/// start and a system with one dock is a system with nowhere to go.
pub const STATION_SHARE: f64 = 0.6;

/// Once a system has a station, the chance of a second, and then of a
/// third, and so on up to nine. Each only where there is a body of the
/// right kind free for it — [`parent_suits`] and "at most one station per
/// parent body" still hold — so a two-planet system stays a two-station
/// system whatever these say. Two of a kind in one system is allowed: a
/// system with two rocky planets can have an orbital round each, and with
/// some of them now hostile there is a reason to have the second. Eight
/// rolls, up from five, went in with the wider body count (`MIN_BODIES`
/// and `MAX_BODIES` in `system`) and the version bump that took: a system
/// with a station has four or five now rather than three, which is what
/// gives the two sides ([`HOSTILE_SHARE`]) a corner each to sit in.
pub const MORE_STATIONS: [f64; 8] = [0.9, 0.85, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3];

/// What share of the stations somebody lives on are somebody else's: docked
/// there, the crew are the enemy. Rolled off a station's own branch, so a
/// station does not change sides when its neighbour gains a hazard — but
/// the roll decides only **how many** of a system's stations are the
/// enemy's; *which* of them is `system::take_sides`, which puts them
/// together at one end of the system and the rest at the other. A
/// derelict is never hostile — there is nobody aboard to be.
pub const HOSTILE_SHARE: f64 = 0.3;

/// Relays want somewhere nobody goes. A system at or above this is a
/// candidate; below it, a relay is not sited there.
pub const RELAY_DESOLATION: f64 = 0.6;

/// What a station is.
///
/// The discriminants cross the wasm boundary as numbers, so they are written
/// out and must not be reordered — the host's name table is indexed by them,
/// exactly as `SPOT_NAMES` and `JOB_NAMES` are.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StationKind {
    /// In orbit of somewhere people live. The ordinary case.
    Orbital = 0,
    /// Hung off a gas giant, cracking its atmosphere.
    Refinery = 1,
    /// Dug into a rocky planet or an ice world. It was bolted to a belt
    /// until the belts became the crew's own mining sites — see
    /// [`parent_suits`].
    MiningOutpost = 2,
    /// Nobody aboard. What is left is worth taking.
    Derelict = 3,
    /// Deep space, on its own, listening.
    Relay = 4,
}

impl StationKind {
    pub const ALL: [StationKind; 5] = [
        StationKind::Orbital,
        StationKind::Refinery,
        StationKind::MiningOutpost,
        StationKind::Derelict,
        StationKind::Relay,
    ];

    /// Whether a station of this **kind** has this to sell.
    ///
    /// The ceiling on a shelf, and — for everything but the gear — the
    /// only place the rule is written down. A derelict sells nothing:
    /// there is nobody aboard to sell it.
    ///
    /// **The guns and the armour are not here.** Since the money rework
    /// (feature 95) whether a place trades in weapons, or in armour, is
    /// rolled per **station** rather than per kind — `Stock::roll`, off
    /// [`WEAPON_TRADE_CHANCE`] and [`ARMOUR_TRADE_CHANCE`] — so that two
    /// orbitals in one system are two different shops. This answers
    /// `true` for all eight, since the ceiling has no opinion about them
    /// and `Stock` is what decides; a caller wanting to know whether a
    /// thing is actually on sale asks the station's `Stock::sells`.
    ///
    /// Every station **buys** anything; this is only about what is on the
    /// shelf.
    pub fn sells(self, resource: physics::ResourceId) -> bool {
        use physics::ResourceId;
        match (self, resource) {
            (StationKind::Derelict, _) => false,
            // A research key is found on a station's research desk, never
            // on its shelf; a class's charges (features 88 and 90) come
            // back on a cooldown and nobody stocks one. Any station buys
            // any of them.
            (
                _,
                ResourceId::ResearchKey
                | ResourceId::ResearchKeyTwo
                | ResourceId::SandbagKit
                | ResourceId::SentryKit
                | ResourceId::Grenade,
            ) => false,
            // A dressing is on the shelf where there are people to hurt
            // themselves: the orbitals and the refineries. A medkit is
            // made at a drug lab and is on every other shelf.
            (StationKind::Orbital | StationKind::Refinery, ResourceId::Bandage) => true,
            (_, ResourceId::Bandage) => false,
            // A pressure suit hangs where people work outside.
            (StationKind::Refinery | StationKind::MiningOutpost, ResourceId::Suit) => true,
            (_, ResourceId::Suit) => false,
            _ => true,
        }
    }
}

/// What a body is.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BodyKind {
    RockyPlanet = 0,
    GasGiant = 1,
    IceWorld = 2,
    /// Measured as a single point, however wide it looks on a map. A belt
    /// that was a ring would make "how far apart are these two things" a
    /// different question for every pair, and the layout checks would have to
    /// answer it a different way for every pair too.
    AsteroidBelt = 3,
}

impl BodyKind {
    pub const ALL: [BodyKind; 4] = [
        BodyKind::RockyPlanet,
        BodyKind::GasGiant,
        BodyKind::IceWorld,
        BodyKind::AsteroidBelt,
    ];

    /// How often each turns up when a system is being filled in. Weights, not
    /// probabilities — they are normalised where they are used.
    pub fn weight(self) -> f64 {
        match self {
            BodyKind::RockyPlanet => 4.0,
            BodyKind::GasGiant => 2.5,
            BodyKind::IceWorld => 2.0,
            BodyKind::AsteroidBelt => 2.5,
        }
    }
}

/// Which bodies a kind of station can be built on.
///
/// This is the whole of the matching rule and the only place it is written
/// down. A refinery hangs off a gas giant because that is what it refines; an
/// outpost is dug into a rocky planet or an ice world, because that is where
/// there is a crust to dig; a relay is out in deep space on its own; and a
/// derelict can be anywhere a station could stand, because whatever it was
/// for stopped mattering a long time ago.
///
/// **Nothing sits at a belt.** A belt is the crew's mining site — the ship
/// holds at it and the asteroids are laid out about the hull — and a
/// station in orbit of one stood in the way of that: a trip to a body ends
/// `flight`'s `ARRIVAL_RADIUS_BODY` short of it, further out than a station
/// orbits its parent, so a station on the near side was the nearest thing
/// to the ship when it came to rest, the view settled on *it* rather than
/// on the belt, and no site was laid out. The outposts were bolted to the
/// belts until the belts became sites, and moved then.
pub fn parent_suits(kind: StationKind, parent: Option<BodyKind>) -> bool {
    match (kind, parent) {
        (_, Some(BodyKind::AsteroidBelt)) => false,
        (StationKind::Refinery, Some(BodyKind::GasGiant)) => true,
        (StationKind::MiningOutpost, Some(BodyKind::RockyPlanet | BodyKind::IceWorld)) => true,
        (StationKind::Orbital, Some(BodyKind::RockyPlanet | BodyKind::IceWorld)) => true,
        (StationKind::Relay, None) => true,
        (StationKind::Derelict, _) => true,
        _ => false,
    }
}

/// What is left to be pulled out of the wreck.
///
/// Only derelicts have any, by default — and "by default" is this function:
/// somewhere to hang an exception without going hunting through the
/// generator for the place the number is decided.
pub fn salvage_sites(kind: StationKind, u: f64) -> u32 {
    match kind {
        StationKind::Derelict => 3 + (u * 6.0) as u32,
        _ => 0,
    }
}

/// What can be wrong with a station.
///
/// **Stored and never displayed.** The map generator places these; the
/// preview does not mention them, because a player who can read the hazards
/// off the star map before setting out is not exploring.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HazardKind {
    Radiation = 0,
    /// A hull that is open to space.
    Breach = 1,
    Fire = 2,
    /// Something growing, or something spilt.
    Contamination = 3,
}

impl HazardKind {
    pub const ALL: [HazardKind; 4] = [
        HazardKind::Radiation,
        HazardKind::Breach,
        HazardKind::Fire,
        HazardKind::Contamination,
    ];
}

/// How many kinds of thing tend to be wrong with each kind of station, and
/// how many sites of each. A working station has the odd problem; a derelict
/// is nothing but.
pub fn hazard_pressure(kind: StationKind) -> f64 {
    match kind {
        StationKind::Orbital => 0.15,
        StationKind::Refinery => 0.45,
        StationKind::MiningOutpost => 0.35,
        StationKind::Derelict => 1.0,
        StationKind::Relay => 0.25,
    }
}

// A station used to have stores, and a blueprint used to carry them. Both
// went when the crew started bringing **money** rather than starting with
// material: what a ship costs is now a price in euros out of one shared pool
// — see `crates/economy` and `crates/shipdesign` — and a station holding
// crates of metal was a starting condition dressed up as a fact about the
// world. Trading is a separate thing that does not exist yet, and when it
// does, what a station will sell is a price list rather than a stockpile.

#[cfg(test)]
mod tests {
    use super::*;

    /// The defaults are chosen to make this exactly one. If it ever is not,
    /// every distance in every system has quietly changed scale.
    #[test]
    fn the_reference_ship_accelerates_at_one() {
        let a = REFERENCE_SHIP.acceleration();
        assert!((a - 1.0).abs() < 1e-12, "acceleration was {a}");
        assert_eq!(REFERENCE_SHIP.deceleration(), a);
    }

    #[test]
    fn a_day_is_the_distance_it_looks() {
        let d = reference_distance(1.0).unwrap();
        assert!((d - 518_400.0).abs() < 1e-6, "a one-day hop was {d}");
        assert!((reference_days(d).unwrap() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn the_travel_band_makes_sense_and_desolation_maps_onto_the_whole_of_it() {
        // --- the_travel_band_makes_sense ---
        {
            assert!(TRAVEL_BAND.min_days > 0.0);
            assert!(TRAVEL_BAND.max_days > TRAVEL_BAND.min_days);
        }

        // --- desolation_maps_onto_the_whole_band ---
        {
            assert!((target_hop_days(0.0) - TRAVEL_BAND.min_days).abs() < 1e-12);
            assert!((target_hop_days(1.0) - TRAVEL_BAND.max_days).abs() < 1e-12);
            // And never out of it, whatever it is handed.
            for &d in &[-1.0, 0.0, 0.3, 1.0, 2.0] {
                let t = target_hop_days(d);
                assert!(
                    t >= TRAVEL_BAND.min_days && t <= TRAVEL_BAND.max_days,
                    "{t}"
                );
            }
        }

        // --- desolation_stays_in_its_range_and_leans_low ---
        {
            let mut rng = crate::rng::Rng::new(1);
            let mut high = 0;
            for _ in 0..10_000 {
                let d = desolation(rng.unit());
                assert!((0.0..=1.0).contains(&d));
                if d > 0.5 {
                    high += 1;
                }
            }
            // A uniform draw would put half of them above a half.
            assert!(high < 4_000, "{high} systems in ten thousand were desolate");
        }
    }

    /// What is on the shelf where: nothing at a derelict, no key and no
    /// class charge anywhere, a dressing at the orbitals and the
    /// refineries, a pressure suit where people work outside, and
    /// everything else everywhere somebody lives. The **gear** is the
    /// station's own roll and not the kind's, so the ceiling lets all
    /// eight pieces through.
    #[test]
    fn what_each_kind_of_station_sells() {
        use physics::ResourceId;
        for kind in StationKind::ALL {
            for resource in ResourceId::ALL {
                let want = match (kind, resource) {
                    (StationKind::Derelict, _) => false,
                    (
                        _,
                        ResourceId::ResearchKey
                        | ResourceId::ResearchKeyTwo
                        | ResourceId::SandbagKit
                        | ResourceId::SentryKit
                        | ResourceId::Grenade,
                    ) => false,
                    (StationKind::Orbital | StationKind::Refinery, ResourceId::Bandage) => true,
                    (_, ResourceId::Bandage) => false,
                    (StationKind::Refinery | StationKind::MiningOutpost, ResourceId::Suit) => true,
                    (_, ResourceId::Suit) => false,
                    _ => true,
                };
                assert_eq!(kind.sells(resource), want, "{kind:?} {resource:?}");
            }
        }
        assert!(StationKind::Orbital.sells(ResourceId::Medkit));
        assert!(StationKind::Orbital.sells(ResourceId::Vegetable));
        assert!(!StationKind::Orbital.sells(ResourceId::ResearchKey));
        assert!(!StationKind::Relay.sells(ResourceId::ResearchKeyTwo));
        assert!(!StationKind::Orbital.sells(ResourceId::SandbagKit));
        assert!(!StationKind::Refinery.sells(ResourceId::SentryKit));
        assert!(!StationKind::Orbital.sells(ResourceId::Grenade));
        // Dressings: the orbitals and the refineries, and not a staple.
        assert!(StationKind::Orbital.sells(ResourceId::Bandage));
        assert!(StationKind::Refinery.sells(ResourceId::Bandage));
        assert!(!StationKind::MiningOutpost.sells(ResourceId::Bandage));
        assert!(!StationKind::Relay.sells(ResourceId::Bandage));
        assert!(!STAPLES.contains(&ResourceId::Bandage));
        // Suits: where there is work outside, and nowhere else.
        assert!(StationKind::Refinery.sells(ResourceId::Suit));
        assert!(StationKind::MiningOutpost.sells(ResourceId::Suit));
        assert!(!StationKind::Orbital.sells(ResourceId::Suit));
        // A derelict sells nothing at all, gear included.
        for resource in ResourceId::ALL {
            assert!(!StationKind::Derelict.sells(resource), "{resource:?}");
        }
    }

    /// The two gear trades are a **function of the seed alone**, rolled
    /// off their own stream, and each puts its whole list on the shelf or
    /// none of it (feature 95). A derelict rolls neither.
    #[test]
    fn the_gear_trades_are_a_roll_of_their_own() {
        use crate::rng::Rng;
        let roll =
            |kind, seed: u64| Stock::roll(kind, &mut Rng::new(seed), &mut Rng::new(seed ^ 0x_9E37));
        // The same seed gives the same shelf, every time.
        for seed in 0..50u64 {
            let once = roll(StationKind::Orbital, seed);
            assert_eq!(once, roll(StationKind::Orbital, seed), "seed {seed}");
            // All five weapons or none; all three pieces or none.
            let weapons = WEAPONS.iter().filter(|&&r| once.sells(r)).count();
            assert!(weapons == 0 || weapons == WEAPONS.len(), "seed {seed}");
            let armour = ARMOUR.iter().filter(|&&r| once.sells(r)).count();
            assert!(armour == 0 || armour == ARMOUR.len(), "seed {seed}");
            assert_eq!(once.weapon_trade(), weapons > 0, "seed {seed}");
            assert_eq!(once.armour_trade(), armour > 0, "seed {seed}");
            // A derelict has no market and rolls nothing.
            assert_eq!(roll(StationKind::Derelict, seed), Stock::NONE);
        }
        // Over enough seeds, about two in five each way, and the two are
        // independent — so neither is always on and neither always off.
        let mut both = 0;
        let mut neither = 0;
        let (mut guns, mut plate) = (0, 0);
        for seed in 0..400u64 {
            let s = roll(StationKind::Orbital, seed);
            guns += s.weapon_trade() as u32;
            plate += s.armour_trade() as u32;
            both += (s.weapon_trade() && s.armour_trade()) as u32;
            neither += (!s.weapon_trade() && !s.armour_trade()) as u32;
        }
        assert!((100..=220).contains(&guns), "{guns} of 400 sold guns");
        assert!((100..=220).contains(&plate), "{plate} of 400 sold armour");
        assert!(both > 20, "{both} sold both");
        assert!(neither > 20, "{neither} sold neither");
        // And a staple is on every shelf that has a market at all.
        for seed in 0..50u64 {
            for &staple in STAPLES.iter() {
                let s = roll(StationKind::Orbital, seed);
                assert!(s.sells(staple), "seed {seed}: {staple:?}");
            }
        }
    }

    /// The matching rule, both ways round: what each kind accepts, and what
    /// it refuses. The refusals are the half that would go unnoticed.
    #[test]
    fn stations_only_sit_where_they_belong() {
        use BodyKind::*;
        use StationKind::*;
        assert!(parent_suits(Refinery, Some(GasGiant)));
        assert!(!parent_suits(Refinery, Some(RockyPlanet)));
        assert!(!parent_suits(Refinery, None));
        assert!(parent_suits(MiningOutpost, Some(RockyPlanet)));
        assert!(parent_suits(MiningOutpost, Some(IceWorld)));
        assert!(!parent_suits(MiningOutpost, Some(GasGiant)));
        assert!(!parent_suits(MiningOutpost, None));
        assert!(parent_suits(Orbital, Some(RockyPlanet)));
        assert!(parent_suits(Orbital, Some(IceWorld)));
        assert!(!parent_suits(Orbital, Some(GasGiant)));
        assert!(!parent_suits(Orbital, None));
        assert!(parent_suits(Relay, None));
        assert!(!parent_suits(Relay, Some(RockyPlanet)));
        for &b in &BodyKind::ALL {
            assert_eq!(parent_suits(Derelict, Some(b)), b != AsteroidBelt);
        }
        assert!(parent_suits(Derelict, None));
        // A belt is a mining site, and nothing at all stands at one.
        for &k in &StationKind::ALL {
            assert!(!parent_suits(k, Some(AsteroidBelt)), "{k:?} at a belt");
        }
    }

    #[test]
    fn only_derelicts_have_salvage_by_default() {
        for &k in &StationKind::ALL {
            let n = salvage_sites(k, 0.9);
            if k == StationKind::Derelict {
                assert!(n > 0);
            } else {
                assert_eq!(n, 0, "{k:?} had salvage");
            }
        }
    }
}

/// What one station has on the shelf: a bit a resource, in
/// [`ResourceId::ALL`] order.
///
/// [`StationKind::sells`] is the ceiling — nothing a kind never stocks is
/// ever on a shelf — and this is what one station keeps under it. The
/// staples ([`STAPLES`]: ore, metal and both foods) are on every shelf,
/// because a station where the crew can buy nothing to build with and
/// nothing to eat is a trap rather than a place; each of the rest is rolled off the
/// station's own stream, so two stations of a kind stock different things
/// and there is a reason to fly to the other one. A theme — what a
/// station is *for* — would replace the roll, not the ceiling.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stock(pub u32);

/// On every shelf the kind allows: what the crew eat, and the medicine
/// they cannot do without.
pub const STAPLES: [ResourceId; 3] = [ResourceId::Vegetable, ResourceId::Tofu, ResourceId::Medkit];

/// The five weapons a **weapon trade** puts on its shelf, and the three
/// pieces a **armour trade** does (feature 95). Whether a station has
/// either is one flag each, rolled off its own stream — so a place may
/// sell guns, armour, both or neither, and a crew who want a sniper rifle
/// have somewhere to fly to rather than a bench to stand at.
pub const WEAPONS: [ResourceId; 5] = [
    ResourceId::Handgun,
    ResourceId::Shotgun,
    ResourceId::AutoRifle,
    ResourceId::SniperRifle,
    ResourceId::Schword,
];
pub const ARMOUR: [ResourceId; 3] = [ResourceId::Helm, ResourceId::Kevlar, ResourceId::LegGuard];

/// How likely a station is to stock any one good that is not a staple.
pub const STOCKED_CHANCE: f64 = 0.6;

/// How likely a place with a market is to trade in weapons, and how
/// likely it is to trade in armour. **Independent**: the two are rolled
/// separately, so two in five places sell guns and two in five sell
/// armour and about one in six sells both. Placeholders, like every other
/// number here.
pub const WEAPON_TRADE_CHANCE: f64 = 0.4;
pub const ARMOUR_TRADE_CHANCE: f64 = 0.4;

impl Stock {
    /// Nothing on the shelf: a derelict's, or a page with no market.
    pub const NONE: Stock = Stock(0);

    /// Roll one station's shelf. `roll` is the station's own stream and
    /// `gear` its **gear-trade** stream (`Purpose::GearTrade`, feature
    /// 95), kept apart so that reworking what is on a shelf never moves
    /// which places sell guns. A draw is made from `roll` for every
    /// resource the kind sells whether it is a staple or not, so that
    /// adding a staple does not reshuffle the rest.
    ///
    /// A **derelict has no market** and rolls nothing: neither stream is
    /// drawn from, since there is nobody aboard to keep a desk.
    pub fn roll(
        kind: StationKind,
        roll: &mut crate::rng::Rng,
        gear: &mut crate::rng::Rng,
    ) -> Stock {
        if kind == StationKind::Derelict {
            return Stock::NONE;
        }
        let mut bits = 0u32;
        for &resource in ResourceId::ALL.iter() {
            if !kind.sells(resource) || WEAPONS.contains(&resource) || ARMOUR.contains(&resource) {
                continue;
            }
            let drawn = roll.chance(STOCKED_CHANCE);
            if drawn || STAPLES.contains(&resource) {
                bits |= 1 << resource as u32;
            }
        }
        // The two trades, in this order and always both drawn, so that
        // turning one chance does not move the other.
        let weapons = gear.chance(WEAPON_TRADE_CHANCE);
        let armour = gear.chance(ARMOUR_TRADE_CHANCE);
        if weapons {
            for &resource in WEAPONS.iter() {
                bits |= 1 << resource as u32;
            }
        }
        if armour {
            for &resource in ARMOUR.iter() {
                bits |= 1 << resource as u32;
            }
        }
        Stock(bits)
    }

    /// Everything the kind sells, the gear included, without a roll: a
    /// fixed shelf for a test that wants the kind's rule and nothing else.
    pub fn everything(kind: StationKind) -> Stock {
        let mut bits = 0u32;
        for &resource in ResourceId::ALL.iter() {
            if kind.sells(resource) {
                bits |= 1 << resource as u32;
            }
        }
        Stock(bits)
    }

    pub fn sells(self, resource: ResourceId) -> bool {
        self.0 & (1 << resource as u32) != 0
    }

    /// Whether this place trades in **weapons**: the flag, read back off
    /// the shelf. What the system preview, the start selection and the
    /// station tooltip say (feature 95).
    pub fn weapon_trade(self) -> bool {
        self.sells(ResourceId::Handgun)
    }

    /// Whether this place trades in **armour**, the same way.
    pub fn armour_trade(self) -> bool {
        self.sells(ResourceId::Helm)
    }
}

/// Roll one station's lean on every price: a whole number per cent in
/// `-MAX_BIAS..=MAX_BIAS` a resource, off `roll`, which is the station's
/// own branch of its stream. Drawn for **every** resource whether the
/// station stocks it or not, in [`ResourceId::ALL`] order, so a resource
/// added later is drawn after the rest and moves none of them; and by
/// [`crate::rng::Rng::below`] alone — no `unit`, nothing off the
/// desolation — so it is the same integer on every target. The world's
/// settlements roll theirs the same way (`world::surface`). It is in the
/// galaxy checksum beside the shelf.
pub fn price_bias(roll: &mut crate::rng::Rng) -> Bias {
    let span = 2 * MAX_BIAS as u32 + 1;
    let mut bias = Bias::NONE;
    for &resource in ResourceId::ALL.iter() {
        bias.0[resource as usize] = roll.below(span) as i8 - MAX_BIAS;
    }
    bias
}
