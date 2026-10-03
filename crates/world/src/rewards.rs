//! What a fight pays and what things cost, as dials the app tunes while
//! the game runs — `rewards.ron`, beside the wave formula's
//! `scaling.ron` ([`crate::droid::WaveScaling`]).
//!
//! The default is the constants ([`crate::data`], [`crate::class`]), so a
//! world never told plays as they say. Like the wave scaling the dials
//! are **neither saved nor hashed**: the app hands them over again every
//! frame the world's differ, a load and a restart included, and what they
//! decide (the money, the experience) is what is kept and hashed. In a
//! two-player run each game reads its own file, and the two have to
//! agree.

use crate::{class, data};
use economy::Money;

/// The dials. Integers only.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(default)
)]
pub struct Rewards {
    /// Experience to every classed crew member in range for an enemy
    /// going down (downed, or destroyed outright), once an enemy.
    pub xp_per_down: u32,
    /// Money for an enemy going down, by the tier of its gear (a machine:
    /// its own tier) — tier one, two, three. Once an enemy, whoever did
    /// it.
    pub bounty: [Money; 3],
    /// How much of that a defence pays, in per cent of it: a hundred is
    /// the same as an attack, nought is nothing.
    pub defense_bounty_percent: u32,
    /// Whether the money waits for the site to be cleared (and is lost
    /// if the crew leave first); `false` pays it the step the enemy goes
    /// down.
    pub bounty_waits_for_clear: bool,
    /// What bringing a dead player's Bim back costs the pool.
    pub buyback: Money,
    /// A trader's shelf prices, in per cent of the trader's ask.
    pub shelf_price_percent: u32,
}

impl Rewards {
    /// The constants: the game as it plays untuned.
    pub const DEFAULT: Rewards = Rewards {
        xp_per_down: class::XP_ENEMY_DOWN,
        bounty: [
            data::REPUBLIC_BOUNTY[1],
            data::REPUBLIC_BOUNTY[2],
            data::REPUBLIC_BOUNTY[3],
        ],
        defense_bounty_percent: data::DEFENSE_BOUNTY_PERCENT,
        bounty_waits_for_clear: true,
        buyback: data::BUYBACK_COST,
        shelf_price_percent: 100,
    };

    /// The money for an enemy of gear tier `tier` (one to three), indexed
    /// safely since a tier arrives from the room as a number: nought for
    /// no tier at all or one past three.
    pub fn bounty_for(&self, tier: u32) -> Money {
        match tier {
            1..=3 => self.bounty[tier as usize - 1],
            _ => 0,
        }
    }

    /// `amount` at a defence: its per cent, rounded down.
    pub fn at_defense(&self, amount: Money) -> Money {
        amount.saturating_mul(self.defense_bounty_percent as Money) / 100
    }

    /// A shelf price scaled by its per cent, rounded down.
    pub fn shelf_price(&self, ask: Money) -> Money {
        ask.saturating_mul(self.shelf_price_percent as Money) / 100
    }
}

impl Default for Rewards {
    fn default() -> Rewards {
        Rewards::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_the_constants_and_a_dial_moves_it() {
        let d = Rewards::DEFAULT;
        for tier in 0..6 {
            assert_eq!(
                d.bounty_for(tier),
                data::REPUBLIC_BOUNTY
                    .get(tier as usize)
                    .copied()
                    .unwrap_or(0)
            );
        }
        assert_eq!(d.shelf_price(1_234), 1_234);
        let tuned = Rewards {
            bounty: [10, 20, 30],
            defense_bounty_percent: 50,
            shelf_price_percent: 150,
            ..d
        };
        assert_eq!(tuned.bounty_for(2), 20);
        assert_eq!(tuned.at_defense(25), 12);
        assert_eq!(tuned.shelf_price(1_000), 1_500);
    }
}
