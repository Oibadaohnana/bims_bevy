//! What a station's desk actually charges, and pays.
//!
//! [`crate::trade_price`] is the **book value** of a unit — what a thing is
//! *worth*, the one number a hold is valued at — and nothing is ever bought
//! or sold at it. A transaction goes through [`quote`]: the book, leaned
//! on by what kind of station the desk is in ([`kind_bias`]) and by the
//! station's own roll ([`Bias`], rolled in `worldgen` and carried on the
//! station), then split either side of that into what the desk **asks**
//! for one and what it **bids** for one. The ask is always above the bid,
//! so a thing bought and sold straight back at one desk always loses
//! money; the whole of "a trading game" so far is that two desks lean
//! different ways.
//!
//! Everything in here is integer arithmetic on whole euros, the rounding
//! written out where it happens, no float anywhere — a native server has
//! to quote the same euro the client does, and the bias is in the galaxy
//! checksum for the same reason.

use physics::ResourceId;

use crate::{EconomyError, Money, trade_price};

/// How many resources there are: a bias is one number for each of them.
const RESOURCES: usize = ResourceId::ALL.len();

/// The desk's spread, in basis points of the mid price — the *whole*
/// spread, ask to bid; each side is half of it. A thousand is ten per
/// cent between them, five either side of the mid. Placeholder, like the
/// biases; a value, not a table, so it can be tuned in one place.
pub const SPREAD_BP: Money = 1_000;

/// The furthest a station's own roll leans a price, per cent, either
/// way: a local bias is in `-MAX_BIAS..=MAX_BIAS`.
pub const MAX_BIAS: i8 = 15;

/// The most anything outside this crate ever adds to a station's own
/// lean, per cent — the **front premium** of feature 94, which
/// `world::data`'s `FRONT_BIAS * FRONT_HOPS` comes to. It is written
/// down here because [`quote`] has to hold its promise (`bid < ask`, and
/// nothing quoted at nothing) over the **sum**, not over the roll alone,
/// and the test below is what pins that; `world`'s own test pins that
/// its two numbers do not exceed it.
pub const MAX_FRONT_BIAS: i32 = 15;

/// Whether a resource is **war goods**: a weapon, a piece of armour, a
/// medkit or a bandage. What a desk near the front charges over the odds
/// for (feature 94) — the one place that list is written down, as a
/// `match` with a row a resource so a resource added to `physics` is a
/// compile error here rather than a thing quietly priced as groceries.
///
/// The class charges — a sandbag kit, a sentry kit, a grenade — are
/// **not** on it: since features 88 and 90 those are abilities that come
/// back on a cooldown rather than things a desk stocks, and nobody's
/// shelf sells one.
pub fn war_goods(resource: ResourceId) -> bool {
    match resource {
        // The five weapons a hand holds.
        ResourceId::Handgun
        | ResourceId::Shotgun
        | ResourceId::AutoRifle
        | ResourceId::SniperRifle
        | ResourceId::Schword => true,
        // What is worn into a fight. The pressure suit is not armour: it
        // is for going outside, and a war does not make it dearer.
        ResourceId::Helm | ResourceId::Kevlar | ResourceId::LegGuard => true,
        // And what patches up what those two did.
        ResourceId::Medkit | ResourceId::Bandage => true,
        ResourceId::Vegetable
        | ResourceId::Tofu
        | ResourceId::Suit
        | ResourceId::ResearchKey
        | ResourceId::ResearchKeyTwo
        | ResourceId::SandbagKit
        | ResourceId::SentryKit
        | ResourceId::Grenade => false,
    }
}

/// What kind of desk quotes: the station kinds that keep one, and the
/// settlement on a planet's surface, which is a station of its own kind
/// to a market whatever plan it is built on. A derelict has no market —
/// nobody aboard to keep a desk — so there is no variant for it: a place
/// with no market has no `Market`, rather than a kind that quotes
/// nothing.
///
/// Its own enum rather than `worldgen::StationKind`, since the generator
/// has no settlement kind and this crate knows no generator; the world
/// maps one to the other (`world::station::market_kind`). The
/// discriminants are written out for the same reason every other code in
/// this workspace's are.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MarketKind {
    Orbital = 0,
    Refinery = 1,
    MiningOutpost = 2,
    Relay = 3,
    Settlement = 4,
}

impl MarketKind {
    /// Every kind, in discriminant order — the column order of
    /// [`kind_bias`]'s rows.
    pub const ALL: [MarketKind; 5] = [
        MarketKind::Orbital,
        MarketKind::Refinery,
        MarketKind::MiningOutpost,
        MarketKind::Relay,
        MarketKind::Settlement,
    ];

    pub fn code(self) -> u32 {
        self as u32
    }
}

/// One station's own lean on every price: a small whole number per cent
/// a resource, in [`ResourceId::ALL`] order, each in
/// `-MAX_BIAS..=MAX_BIAS`. Rolled by the generator for every resource
/// whether the station stocks it or not — so a resource added later does
/// not reshuffle the rest — and in the galaxy checksum beside the shelf.
/// [`Bias::NONE`] is a desk with no lean of its own, which the start
/// station's is made to be whatever it rolled.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bias(pub [i8; RESOURCES]);

impl Bias {
    /// No lean on anything.
    pub const NONE: Bias = Bias([0; RESOURCES]);

    /// The lean on one resource, per cent.
    pub fn of(self, resource: ResourceId) -> i32 {
        self.0[resource as usize] as i32
    }

    /// Whether every entry is inside the range the generator rolls.
    pub fn in_range(self) -> bool {
        self.0.iter().all(|&b| (-MAX_BIAS..=MAX_BIAS).contains(&b))
    }
}

/// What a desk quotes for one unit: what it asks to sell one for, and
/// what it bids to buy one at. `bid < ask` always.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Quote {
    pub ask: Money,
    pub bid: Money,
}

impl Quote {
    /// What `units` cost at the ask, or a refusal if the sum does not fit.
    pub fn cost(self, units: u32) -> Result<Money, EconomyError> {
        value(self.ask, units)
    }

    /// What `units` fetch at the bid, or a refusal if the sum does not fit.
    pub fn fetches(self, units: u32) -> Result<Money, EconomyError> {
        value(self.bid, units)
    }

    /// The same quote for a piece of gear at `tier`: **both sides**
    /// multiplied by `crate::tier_price`, so a desk asks four times as
    /// much for a tier-two rifle as for a tier-one and bids four times as
    /// much for one off the crew's shelf. The one place a tier touches a
    /// price; `crate::tiered` says which resources it may be asked of.
    pub fn at_tier(self, tier: u32) -> Quote {
        let factor = crate::tier_price(tier);
        Quote {
            ask: self.ask.saturating_mul(factor),
            bid: self.bid.saturating_mul(factor),
        }
    }
}

/// `price * units`, or a refusal. Checked like every other sum here.
pub fn value(price: Money, units: u32) -> Result<Money, EconomyError> {
    price
        .checked_mul(units as Money)
        .ok_or(EconomyError::Overflow)
}

/// One desk: its kind and its own lean. What a station carries to be
/// quoted from, and what a design phase's budget buys against.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Market {
    pub kind: MarketKind,
    pub bias: Bias,
}

impl Market {
    pub const fn new(kind: MarketKind, bias: Bias) -> Market {
        Market { kind, bias }
    }

    /// An orbital's desk with no lean of its own: the ordinary station.
    /// What a budget with no station named is priced at — a fixture's, a
    /// test's, the world's free construction budget's, which buys no
    /// goods — so that nothing is ever bought at the book by default.
    pub const PLAIN: Market = Market::new(MarketKind::Orbital, Bias::NONE);

    /// What this desk quotes for one unit of `resource`.
    pub fn quote(self, resource: ResourceId) -> Quote {
        quote(self.kind, self.bias.of(resource), resource)
    }
}

/// How a kind of station leans on a resource, per cent of the book: below
/// nought it sells the thing cheap, above it pays well for it — one
/// number does both, since the spread is what tells the two apart.
///
/// A `match` on the resource with a row a resource, so a resource added
/// to `physics` is a compile error here rather than a thing every desk
/// quotes at the book; the columns are [`MarketKind::ALL`] in order,
/// and the row's length is what makes a new kind one too. **Starting
/// values, to be tuned**: an orbital and a settlement sell food cheap and
/// a mining outpost, out on its own, pays well for it; a relay is dear on
/// everything, and dearest of all on the food and the dressings it has to
/// have shipped in; a settlement, where the fighting is, pays well for
/// guns and armour.
pub fn kind_bias(kind: MarketKind, resource: ResourceId) -> i32 {
    // Columns: Orbital, Refinery, MiningOutpost, Relay, Settlement.
    const O: usize = MarketKind::ALL.len();
    let row: [i32; O] = match resource {
        ResourceId::Vegetable => [-15, 0, 15, 25, -15],
        ResourceId::Tofu => [-15, 0, 15, 25, -15],
        // A suit is a refinery's and an outpost's stock in trade: both
        // are places people work outside.
        ResourceId::Suit => [0, -15, -15, 15, 0],
        // The guns and the armour. A settlement buys them well — it is
        // the one place with a militia to arm.
        ResourceId::Handgun
        | ResourceId::Shotgun
        | ResourceId::AutoRifle
        | ResourceId::SniperRifle
        | ResourceId::Schword => [0, 0, 0, 15, 10],
        ResourceId::Helm | ResourceId::Kevlar | ResourceId::LegGuard => [0, 0, 0, 15, 10],
        ResourceId::Medkit => [0, 0, 0, 15, 0],
        ResourceId::Bandage => [0, 0, 0, 25, 0],
        ResourceId::ResearchKey => [0, 0, 0, 15, 0],
        ResourceId::ResearchKeyTwo => [0, 0, 0, 15, 0],
        ResourceId::SandbagKit => [0, 0, 0, 15, 0],
        ResourceId::SentryKit => [0, 0, 0, 15, 0],
        ResourceId::Grenade => [0, 0, 0, 15, 0],
    };
    row[kind as usize]
}

/// What a desk of `kind` with a local lean of `bias` per cent quotes for
/// one unit of `resource`. The sum, in whole euros, rounding down at
/// each division:
///
/// ```text
/// mid  = book * (100 + kind_bias + bias) / 100
/// half = max(1, mid * SPREAD_BP / 20_000)
/// ask  = mid + half
/// bid  = max(1, mid - half)
/// ```
///
/// The half is at least a euro, so that a thing worth a handful — a
/// crate of vegetables — still has a spread: without it the desk would
/// buy and sell food at one price, and `bid < ask` is the one thing every
/// quote promises. Nothing
/// here can overflow: the book tops out in the thousands and the leans
/// in the tens, so the products are small however the biases go.
pub fn quote(kind: MarketKind, bias: i32, resource: ResourceId) -> Quote {
    let book = trade_price(resource);
    // Per cent of the book. The kind leans at most a few tens either way
    // and the local roll at most MAX_BIAS, so this is well above nought;
    // held there anyway, so a wild bias cannot quote a negative price.
    let percent = (100 + kind_bias(kind, resource) + bias).max(1) as Money;
    let mid = book * percent / 100;
    let half = (mid * SPREAD_BP / 20_000).max(1);
    Quote {
        ask: mid + half,
        bid: mid.saturating_sub(half).max(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one thing every quote promises: a buy and a sell at one desk
    /// always loses money — for every kind, every resource and every
    /// lean the generator can roll, and a little beyond it.
    #[test]
    fn the_bid_is_under_the_ask_everywhere() {
        for kind in MarketKind::ALL {
            for &resource in ResourceId::ALL.iter() {
                // Out to the **largest possible sum**: the station's own
                // roll plus the front premium the world adds on top of
                // it (feature 94), and a little beyond either way.
                for bias in (-MAX_BIAS as i32 - 5)..=(MAX_BIAS as i32 + MAX_FRONT_BIAS + 5) {
                    let q = quote(kind, bias, resource);
                    assert!(
                        q.bid < q.ask,
                        "{kind:?} {resource:?} at {bias}: bid {} is not under ask {}",
                        q.bid,
                        q.ask
                    );
                    assert!(q.bid >= 1, "{kind:?} {resource:?} at {bias}: pays nothing");
                }
            }
        }
    }

    /// The sum, worked by hand for a handgun at the book: mid 1500, half
    /// 75. Then leaned: a settlement pays ten over for a weapon, and a
    /// local roll adds to either.
    #[test]
    fn the_sum_comes_out_as_written() {
        let plain = quote(MarketKind::Orbital, 0, ResourceId::Handgun);
        assert_eq!(
            plain,
            Quote {
                ask: 1_575,
                bid: 1_425
            }
        );
        // 1500 * 110 / 100 = 1650; half = 1650 * 1000 / 20000 = 82.
        assert_eq!(
            quote(MarketKind::Settlement, 0, ResourceId::Handgun),
            Quote {
                ask: 1_732,
                bid: 1_568
            }
        );
        // And with the station's own lean on top: 1500 * 125 / 100 = 1875,
        // half 93.
        assert_eq!(
            quote(MarketKind::Settlement, 15, ResourceId::Handgun),
            Quote {
                ask: 1_968,
                bid: 1_782
            }
        );
        // Rounding is down at each division: 8 * 85 / 100 = 6 (6.8), and
        // the half is the floor of one euro.
        assert_eq!(
            quote(MarketKind::Orbital, 0, ResourceId::Vegetable),
            Quote { ask: 7, bid: 5 }
        );
        // A tier multiplies both sides and nothing else: four times at
        // tier two, sixteen at tier three (feature 95).
        assert_eq!(
            plain.at_tier(2),
            Quote {
                ask: 4 * 1_575,
                bid: 4 * 1_425
            }
        );
        assert_eq!(
            plain.at_tier(3),
            Quote {
                ask: 16 * 1_575,
                bid: 16 * 1_425
            }
        );
        assert_eq!(plain.at_tier(1), plain);
    }

    /// The ask stays over the bid at **every tier**, which is what makes a
    /// thing bought and sold straight back lose money whatever tier it is
    /// at (feature 95).
    #[test]
    fn the_ask_is_over_the_bid_at_every_tier() {
        for kind in MarketKind::ALL {
            for &resource in ResourceId::ALL.iter().filter(|&&r| crate::tiered(r)) {
                for bias in (-MAX_BIAS as i32)..=(MAX_BIAS as i32 + MAX_FRONT_BIAS) {
                    let base = quote(kind, bias, resource);
                    let mut last = 0;
                    for tier in 1..=3 {
                        let q = base.at_tier(tier);
                        assert!(q.bid < q.ask, "{kind:?} {resource:?} tier {tier}");
                        assert!(
                            q.bid > last,
                            "{kind:?} {resource:?} tier {tier} is no dearer"
                        );
                        last = q.bid;
                    }
                }
            }
        }
    }

    /// The starting leans, as they were asked for.
    #[test]
    fn each_kind_leans_the_way_it_was_asked_to() {
        use MarketKind::*;
        assert!(kind_bias(Orbital, ResourceId::Vegetable) < 0);
        assert!(kind_bias(Orbital, ResourceId::Tofu) < 0);
        assert!(kind_bias(Settlement, ResourceId::Vegetable) < 0);
        assert!(kind_bias(MiningOutpost, ResourceId::Vegetable) > 0);
        assert!(kind_bias(MiningOutpost, ResourceId::Suit) < 0);
        assert!(kind_bias(Refinery, ResourceId::Suit) < 0);
        // A settlement has a militia, and pays for what arms it.
        assert!(kind_bias(Settlement, ResourceId::AutoRifle) > 0);
        assert!(kind_bias(Settlement, ResourceId::Kevlar) > 0);
        for &resource in ResourceId::ALL.iter() {
            assert!(
                kind_bias(Relay, resource) > 0,
                "{resource:?} not dear at a relay"
            );
        }
        assert!(kind_bias(Relay, ResourceId::Vegetable) > kind_bias(Relay, ResourceId::Handgun));
        assert!(kind_bias(Relay, ResourceId::Bandage) > kind_bias(Relay, ResourceId::Handgun));
    }

    /// What the front premium is charged on: a weapon, a piece of
    /// armour, a medkit or a bandage, and nothing else (feature 94).
    #[test]
    fn war_goods_are_the_guns_the_armour_and_the_medicine() {
        for resource in [
            ResourceId::Handgun,
            ResourceId::Shotgun,
            ResourceId::AutoRifle,
            ResourceId::SniperRifle,
            ResourceId::Schword,
            ResourceId::Helm,
            ResourceId::Kevlar,
            ResourceId::LegGuard,
            ResourceId::Medkit,
            ResourceId::Bandage,
        ] {
            assert!(war_goods(resource), "{resource:?} is war goods");
        }
        for resource in [
            ResourceId::Vegetable,
            ResourceId::Tofu,
            ResourceId::Suit,
            ResourceId::ResearchKey,
            ResourceId::ResearchKeyTwo,
            ResourceId::SandbagKit,
            ResourceId::SentryKit,
            ResourceId::Grenade,
        ] {
            assert!(!war_goods(resource), "{resource:?} is not war goods");
        }
        // Ten of them, and the table covers every resource there is.
        let all = ResourceId::ALL.iter().filter(|&&r| war_goods(r)).count();
        assert_eq!(all, 10, "the war goods");
    }

    /// A market quotes through its own bias, and the plain one at none.
    #[test]
    fn a_market_quotes_through_its_own_lean() {
        let mut bias = Bias::NONE;
        bias.0[ResourceId::Handgun as usize] = -15;
        assert!(bias.in_range());
        let desk = Market::new(MarketKind::Orbital, bias);
        assert_eq!(
            desk.quote(ResourceId::Handgun),
            quote(MarketKind::Orbital, -15, ResourceId::Handgun)
        );
        assert_eq!(
            desk.quote(ResourceId::Tofu),
            quote(MarketKind::Orbital, 0, ResourceId::Tofu)
        );
        assert_eq!(
            Market::PLAIN.quote(ResourceId::Handgun),
            quote(MarketKind::Orbital, 0, ResourceId::Handgun)
        );
        bias.0[0] = MAX_BIAS + 1;
        assert!(!bias.in_range());
        // And what a quantity comes to is checked, never wrapped.
        let q = Quote { ask: 3, bid: 1 };
        assert_eq!(q.cost(4), Ok(12));
        assert_eq!(q.fetches(4), Ok(4));
        assert_eq!(value(Money::MAX, 2), Err(EconomyError::Overflow));
    }
}
