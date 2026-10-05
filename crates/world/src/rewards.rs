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

use crate::data;
use economy::Money;

/// The dials. Integers only.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(default)
)]
pub struct Rewards {
    /// What a site is worth in experience on the run's first day, to every
    /// player: each enemy down there pays its wave's share of it, once an
    /// enemy, to every classed crew member in range ([`Rewards::site_xp_on`],
    /// `World::xp_per_down`). It was fifteen an enemy, whatever the wave,
    /// until October 2026 (`xp_per_down`).
    pub site_xp: u32,
    /// How much more a site is worth every day after the first, in per
    /// cent, compounded.
    pub site_xp_growth_percent: u32,
    /// What the Republic pays for a site on the run's first day, in euros:
    /// each enemy down there pays its wave's share of it, once
    /// ([`Rewards::site_money_on`], `World::money_per_down`), as the
    /// experience is shared. It was a bounty an enemy by its tier until
    /// October 2026 (`bounty`).
    pub site_money: Money,
    /// How much more a site pays every day after the first, in per cent,
    /// compounded.
    pub site_money_growth_percent: u32,
    /// How much of that a defence pays, in per cent of it: a hundred is
    /// the same as an attack, nought is nothing.
    pub defense_bounty_percent: u32,
    /// How much of an enemy's money is paid when one of the crew's bots
    /// took it down, in per cent of it.
    pub bot_bounty_percent: u32,
    /// How much is paid when a player's own Bim took it down (a
    /// commander's reinforcements and his medic as his own), in per cent.
    pub player_bounty_percent: u32,
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
        site_xp: data::SITE_XP,
        site_xp_growth_percent: data::SITE_XP_GROWTH_PERCENT,
        site_money: data::SITE_MONEY,
        site_money_growth_percent: data::SITE_MONEY_GROWTH_PERCENT,
        defense_bounty_percent: data::DEFENSE_BOUNTY_PERCENT,
        bot_bounty_percent: data::BOT_BOUNTY_PERCENT,
        player_bounty_percent: data::PLAYER_BOUNTY_PERCENT,
        bounty_waits_for_clear: true,
        buyback: data::BUYBACK_COST,
        shelf_price_percent: 100,
    };

    /// What a site is worth on run day `day` (one the first): `site_xp`
    /// grown by `site_xp_growth_percent` a day, compounded in thousandths
    /// and rounded down — whole numbers, so two machines agree.
    pub fn site_xp_on(&self, day: u32) -> u32 {
        grown(u64::from(self.site_xp), self.site_xp_growth_percent, day).min(u64::from(u32::MAX))
            as u32
    }

    /// What a site pays on run day `day`, the same way: `site_money` grown
    /// by `site_money_growth_percent` a day.
    pub fn site_money_on(&self, day: u32) -> Money {
        grown(self.site_money, self.site_money_growth_percent, day)
    }

    /// `amount` at a defence: its per cent, rounded down.
    pub fn at_defense(&self, amount: Money) -> Money {
        amount.saturating_mul(self.defense_bounty_percent as Money) / 100
    }

    /// `amount` for an enemy one of the crew's bots took down: its per
    /// cent, rounded down.
    pub fn by_bot(&self, amount: Money) -> Money {
        amount.saturating_mul(self.bot_bounty_percent as Money) / 100
    }

    /// `amount` for an enemy a player took down: its per cent, rounded
    /// down.
    pub fn by_player(&self, amount: Money) -> Money {
        amount.saturating_mul(self.player_bounty_percent as Money) / 100
    }

    /// A shelf price scaled by its per cent, rounded down.
    pub fn shelf_price(&self, ask: Money) -> Money {
        ask.saturating_mul(self.shelf_price_percent as Money) / 100
    }
}

/// `base` grown by `percent` a day to run day `day` (one the first),
/// compounded in thousandths and rounded down — whole numbers, so two
/// machines agree.
fn grown(base: u64, percent: u32, day: u32) -> u64 {
    let mut milli = base.saturating_mul(1_000);
    let grow = 100 + u64::from(percent);
    for _ in 1..day.min(400) {
        milli = milli.saturating_mul(grow) / 100;
    }
    milli / 1_000
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
        assert_eq!(d.site_money_on(1), data::SITE_MONEY);
        assert_eq!(d.site_money_on(10), 3_492);
        assert_eq!(d.site_money_on(30), 80_687);
        assert_eq!(d.site_xp_on(31), 1_797);
        assert_eq!(d.shelf_price(1_234), 1_234);
        let tuned = Rewards {
            site_money: 10,
            site_money_growth_percent: 0,
            defense_bounty_percent: 50,
            bot_bounty_percent: 25,
            player_bounty_percent: 120,
            shelf_price_percent: 150,
            ..d
        };
        assert_eq!(tuned.site_money_on(20), 10);
        assert_eq!(tuned.at_defense(25), 12);
        assert_eq!(d.by_bot(1_500), 1_500);
        assert_eq!(d.by_player(1_500), 1_650);
        assert_eq!(tuned.by_player(1_000), 1_200);
        assert_eq!(tuned.by_bot(1_000), 250);
        assert_eq!(tuned.shelf_price(1_000), 1_500);
    }
}
