//! Mercenaries: hired hands living at friendly stations, paid by the month.
//!
//! A station whose people are not enemies may have a mercenary or two
//! living among them — extra bodies in the residents' room, in the
//! olive coverall (`bims::character::Uniform::Mercenary`) so they are told
//! from the residents at a glance, armed better than a resident
//! (`Gear::hired_for`) and for hire. How many is [`how_many`]: a function
//! of the crew's worth against what they set out with, the way an enemy
//! garrison is (`crate::station::enemies_of`), plus a roll off the
//! station's seed — so at the start worth one turns up at some stations
//! and none at the rest, and a richer crew finds more. The world derives
//! all of it from the seed and the worth when the room opens; nothing new
//! is hashed for a mercenary standing at a station.
//!
//! # The fee is the kit
//!
//! A mercenary's price is a month of it, and it is what they carry: the
//! weapon's [`WEAPON_FEE`] and a piece of armour's [`ARMOUR_FEE`] for
//! every piece worn, added up ([`fee_of`]), then moved up or down by up
//! to [`VARIANCE_PERCENT`] off the seed ([`priced`]) — whole euros in and
//! out, so a server quotes the same. A pistol and nothing else is about
//! two thousand; a sniper rifle in full armour about twenty.
//!
//! # Hired, a mercenary is crew that costs money
//!
//! `Command::Hire` (`World::hire`) moves the body out of the residents'
//! room into the crew's — a bunk aboard, the first month paid down — and
//! records a [`Hired`]: which crew member, the fee, and when the next
//! month falls due. Every step [`World::pay_wages`] pays what has fallen
//! due out of the crew's money; a month the money will not cover has
//! the mercenary leave at the dock — back into the station's room,
//! for hire again — or wait for one. Slots and mercenaries do not mix:
//! a mercenary is appended after the players, takes no orders of its
//! own and asks for no speed. `Hired` is in `world_checksum`.

use bims::combat::{ArmourKind, Gear, WeaponKind};
use economy::Money;
use worldgen::rng::Rng;

use crate::data;

/// How long a month of pay is, in the world's minutes.
pub const MONTH: f64 = 30.0 * time::DAY;

/// The most mercenaries a station ever has for hire at once.
pub const MERCENARIES_MAX: u32 = 4;
/// The odds a station has one for hire on top of what the crew's worth
/// says — what makes one "turn up occasionally" at the start.
pub const MERCENARY_CHANCE: f64 = 0.4;
/// How far a fee is moved off the kit's price, either way, at most.
pub const VARIANCE_PERCENT: u64 = 15;

/// The odds a mercenary for hire is a **field medic** (feature 86):
/// somebody whose trade is fetching the fallen out of the fire and
/// patching them up where it is quiet. About a third of them, so a
/// station with one for hire usually has a gun and a station with three
/// usually has a medic among them.
pub const MEDIC_CHANCE: f64 = 0.35;
/// What a field medic asks on top of the kit, a month, in euros. The
/// trade rather than the gear: a field medic carries a pistol like
/// anybody else, and what is being paid for is that it walks into the
/// fire for somebody who cannot walk out.
pub const MEDIC_FEE: Money = 4_000;
/// How many medkits a hired field medic brings with it. Two, as the
/// user asked: enough to get two crew members out of a dying state
/// before it has to go back to the hold for more.
pub const MEDIC_MEDKITS: u32 = 2;

/// A month of a mercenary carrying that weapon, in euros. Tier one is the
/// pistol, tier two the shotgun, the auto rifle and the schword, tier
/// three the sniper rifle.
pub const WEAPON_FEE: [(WeaponKind, Money); 5] = [
    (WeaponKind::LaserPistol, 2_000),
    (WeaponKind::Schword, 3_500),
    (WeaponKind::Shotgun, 6_000),
    (WeaponKind::AutoRifle, 8_000),
    (WeaponKind::SniperRifle, 15_000),
];

/// What a piece of armour worn adds to the month, in euros. One tier of
/// armour exists; a heavier tier is a row here when it does.
pub const ARMOUR_FEE: [(ArmourKind, Money); 3] = [
    (ArmourKind::BasicHelm, 1_000),
    (ArmourKind::BasicKevlar, 3_000),
    (ArmourKind::BasicLegs, 1_000),
];

/// A hired mercenary, as the world keeps it: which crew member it is,
/// what a month costs, and the clock minute the next month falls due.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Hired {
    pub who: u32,
    pub fee: Money,
    pub due: f64,
    /// A month fell due that the money did not cover, said once; the
    /// hand walks off at the next berth unless it is paid first.
    pub owed: bool,
    /// Whether this one is a **field medic** (feature 86): hired for
    /// the job of fetching the fallen out of the fire and treating them,
    /// with none of the medic class's talents. It is kept on the
    /// contract rather than on the body because it is what was hired,
    /// and it is what the world hands the room every step
    /// (`bims::game::Game::set_field_medic`). In `world_checksum` with
    /// the rest of the row.
    pub medic: bool,
}

/// What a hire would come to, for a window to show before the command
/// is sent — `World::hire_offer`.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Offer {
    /// A month, in euros.
    pub fee: Money,
    /// Whether the crew member doing the hiring stands within reach.
    pub in_reach: bool,
    /// Whether the money in hand covers the first month.
    pub affordable: bool,
    /// Whether there is a bunk aboard for one more.
    pub bunk: bool,
    /// Whether the ship is tied up at the station the body lives on.
    pub docked: bool,
    /// Whether the body is a field medic (feature 86), so the window can
    /// say what it is being asked to pay the premium for.
    pub medic: bool,
}

/// How many mercenaries a station with `seed` has for hire against a
/// crew worth `worth` now that set out worth `start_worth`: one for
/// every half of the start worth the worth has grown by, plus one on a
/// roll of [`MERCENARY_CHANCE`] off the seed, never more than
/// [`MERCENARIES_MAX`]. Whole euros in, whole number out.
///
/// `near_front` is feature 94's: a station inside `data::FRONT_HOPS` of
/// the infection — or a town the crew defended — has **one more** hand
/// for hire, people being on the move and armed where the machines are
/// coming. Still never more than [`MERCENARIES_MAX`], so at the cap the
/// front adds nothing.
pub fn how_many(worth: Money, start_worth: Money, seed: u64, near_front: bool) -> u32 {
    let step = start_worth / 2;
    let grown = if step == 0 || worth <= start_worth {
        0
    } else {
        ((worth - start_worth) / step).min(MERCENARIES_MAX as Money) as u32
    };
    let rolled = Rng::new(seed ^ 0x_4D45_5243).chance(MERCENARY_CHANCE) as u32;
    (grown + rolled + u32::from(near_front)).min(MERCENARIES_MAX)
}

/// A month of that kit, before the variance: the weapon's fee and every
/// worn piece's. A body with nothing in its hand is priced as a pistol,
/// since every mercenary carries at least that.
pub fn fee_of(gear: &Gear) -> Money {
    let weapon = gear.weapon.map_or(WeaponKind::LaserPistol, |w| w.kind);
    let mut fee = WEAPON_FEE
        .iter()
        .find(|(kind, _)| *kind == weapon)
        .map_or(0, |(_, fee)| *fee);
    for piece in [gear.head, gear.body, gear.legs].into_iter().flatten() {
        fee += ARMOUR_FEE
            .iter()
            .find(|(kind, _)| *kind == piece.kind)
            .map_or(0, |(_, fee)| *fee);
    }
    fee
}

/// The fee asked: [`fee_of`] moved by up to [`VARIANCE_PERCENT`] either
/// way off `seed`, in whole percent, so the same mercenary asks the same
/// every time and no two ask quite alike. Whole euros.
pub fn priced(seed: u64, gear: &Gear) -> Money {
    priced_as(seed, gear, false)
}

/// [`priced`] with the trade said: a **field medic** asks [`MEDIC_FEE`]
/// on top of the kit, and the variance is rolled over the sum, so the
/// same medic asks the same every time. The roll is drawn from the same
/// stream in the same order either way, so which mercenary is a medic
/// moves nobody else's price.
pub fn priced_as(seed: u64, gear: &Gear, medic: bool) -> Money {
    let mut rng = Rng::new(seed ^ 0x_5052_4943_45);
    let span = 2 * VARIANCE_PERCENT + 1;
    let percent = 100 - VARIANCE_PERCENT + rng.below(span as u32) as u64;
    let kit = fee_of(gear) + if medic { MEDIC_FEE } else { 0 };
    kit * percent / 100
}

/// Whether the mercenary rolled off `seed` is a field medic (feature
/// 86): one roll of [`MEDIC_CHANCE`] off a stream of its own, so it is
/// a function of the seed like the gear and the price and the world
/// derives it when the room opens rather than keeping it.
pub fn is_medic(seed: u64) -> bool {
    Rng::new(seed ^ 0x_4D45_4449_43).chance(MEDIC_CHANCE)
}

/// The seed a station's mercenary number `n` is rolled off: the station's
/// map seed and its place among them, kept apart from the residents'
/// gear seeds (`map_seed ^ who`) by a salt.
pub fn seed_for(map_seed: u64, n: u32) -> u64 {
    map_seed ^ 0x_4849_5245_0000 ^ (n as u64)
}

/// How much of a hire is left unpaid, if any: nothing while the due
/// minute is ahead of the clock.
pub fn owed(hired: &Hired, clock_minutes: f64) -> bool {
    clock_minutes >= hired.due
}

/// What the `test` command wants: whether a station with nobody rolled
/// gets one anyway, for looking at. `data::TEST_MERCENARY` says.
pub fn at_least_for_probe(count: u32) -> u32 {
    count.max(data::TEST_MERCENARY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bims::combat::{Piece, Tier};

    /// The two prices the user named, and the variance kept within its
    /// bounds every way it can fall.
    #[test]
    fn a_pistol_is_two_thousand_and_a_sniper_in_armour_twenty() {
        let pistol = Gear {
            weapon: Some(WeaponKind::LaserPistol.basic()),
            ..Gear::default()
        };
        assert_eq!(fee_of(&pistol), 2_000);
        let heavy = Gear {
            weapon: Some(WeaponKind::SniperRifle.basic()),
            head: Some(Piece::new(1, ArmourKind::BasicHelm, Tier::One)),
            body: Some(Piece::new(2, ArmourKind::BasicKevlar, Tier::One)),
            legs: Some(Piece::new(3, ArmourKind::BasicLegs, Tier::One)),
            ..Gear::default()
        };
        assert_eq!(fee_of(&heavy), 20_000);
        let (mut low, mut high) = (Money::MAX, 0);
        for seed in 0..400u64 {
            let fee = priced(seed, &heavy);
            assert!((17_000..=23_000).contains(&fee), "{fee}");
            low = low.min(fee);
            high = high.max(fee);
        }
        assert!(
            low < 18_000 && high > 22_000,
            "the variance is used: {low}..{high}"
        );
        assert_eq!(priced(7, &heavy), priced(7, &heavy), "the same every time");
    }

    /// A field medic asks the kit and the trade's premium on top, and
    /// about a third of the hands for hire are one.
    #[test]
    fn a_field_medic_asks_for_the_trade_on_top_of_the_kit() {
        let pistol = Gear {
            weapon: Some(WeaponKind::LaserPistol.basic()),
            ..Gear::default()
        };
        for seed in 0..50u64 {
            let plain = priced_as(seed, &pistol, false);
            let medic = priced_as(seed, &pistol, true);
            assert!(medic > plain, "{medic} over {plain}");
            // The same percentage over the bigger sum, so the ratio is
            // the sums' whatever the roll.
            assert_eq!(medic * 2_000 / 6_000, plain, "seed {seed}");
        }
        let medics = (0..200u64).filter(|&s| is_medic(s)).count();
        assert!((50..=110).contains(&medics), "{medics} of two hundred");
        assert_eq!(is_medic(3), is_medic(3), "the same every time");
    }

    /// At the start worth a station has one or none; a crew twice as
    /// rich finds more; and nobody gets more than the most.
    #[test]
    fn how_many_grows_with_the_worth_and_is_sometimes_one_at_the_start() {
        let start = 100_000;
        let mut ones = 0;
        for seed in 0..100u64 {
            let n = how_many(start, start, seed, false);
            assert!(n <= 1, "{n} at the start worth");
            ones += n;
        }
        assert!((20..=60).contains(&ones), "{ones} of a hundred stations");
        assert!(how_many(2 * start, start, 3, false) >= 2);
        assert_eq!(how_many(100 * start, start, 3, false), MERCENARIES_MAX);
        assert!(how_many(start, 0, 3, false) <= 1, "nothing to grow by");
    }

    /// Near the front there is one more hand for hire, and never more
    /// than the most (feature 94).
    #[test]
    fn the_front_finds_one_more_hand_and_never_over_the_cap() {
        let start = 100_000;
        for seed in 0..100u64 {
            let away = how_many(start, start, seed, false);
            let front = how_many(start, start, seed, true);
            assert_eq!(front, (away + 1).min(MERCENARIES_MAX), "seed {seed}");
            assert!(front <= MERCENARIES_MAX);
        }
        // At the cap the front adds nothing at all.
        assert_eq!(how_many(100 * start, start, 3, true), MERCENARIES_MAX);
    }
}
