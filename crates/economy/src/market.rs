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
/// values, to be tuned**: a mining outpost sells ore and galvum cheap and
/// pays well for food, metal and components; a refinery sells metal cheap
/// and pays well for ore; an orbital sells food and fibre cheap; a relay
/// is dear on everything and pays well for food and bandages; a
/// settlement sells food cheap and pays well for metal.
pub fn kind_bias(kind: MarketKind, resource: ResourceId) -> i32 {
    // Columns: Orbital, Refinery, MiningOutpost, Relay, Settlement.
    const O: usize = MarketKind::ALL.len();
    let row: [i32; O] = match resource {
        ResourceId::Ore => [0, 20, -20, 15, 0],
        ResourceId::Metal => [0, -20, 15, 15, 15],
        ResourceId::Components => [0, 0, 15, 15, 0],
        ResourceId::Vegetable => [-15, 0, 15, 25, -15],
        ResourceId::Tofu => [-15, 0, 15, 25, -15],
        ResourceId::Galvum => [0, 0, -20, 15, 0],
        ResourceId::Emitter => [0, 0, 0, 15, 0],
        ResourceId::Suit => [0, 0, 0, 15, 0],
        ResourceId::Handgun => [0, 0, 0, 15, 0],
        ResourceId::Vest => [0, 0, 0, 15, 0],
        ResourceId::Medkit => [0, 0, 0, 15, 0],
        ResourceId::Rock => [0, 0, 0, 15, 0],
        ResourceId::Fibre => [-15, 0, 0, 15, 0],
        ResourceId::Bandage => [0, 0, 0, 25, 0],
        ResourceId::Helm => [0, 0, 0, 15, 0],
        ResourceId::Kevlar => [0, 0, 0, 15, 0],
        ResourceId::LegGuard => [0, 0, 0, 15, 0],
        ResourceId::Shotgun => [0, 0, 0, 15, 0],
        ResourceId::AutoRifle => [0, 0, 0, 15, 0],
        ResourceId::SniperRifle => [0, 0, 0, 15, 0],
        ResourceId::Schword => [0, 0, 0, 15, 0],
        ResourceId::ResearchKey => [0, 0, 0, 15, 0],
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
/// The half is at least a euro, so that a thing worth two — rock — still
/// has a spread: without it the desk would buy and sell rock at one
/// price, and `bid < ask` is the one thing every quote promises. Nothing
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
                for bias in (-MAX_BIAS as i32 - 5)..=(MAX_BIAS as i32 + 5) {
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

    /// The sum, worked by hand for metal at the book: mid 60, half 3.
    /// Then leaned: a refinery sells metal at twenty under, an outpost
    /// buys it at fifteen over, and a local roll adds to either.
    #[test]
    fn the_sum_comes_out_as_written() {
        let plain = quote(MarketKind::Orbital, 0, ResourceId::Metal);
        assert_eq!(plain, Quote { ask: 63, bid: 57 });
        // 60 * 80 / 100 = 48; half = 48 * 1000 / 20000 = 2.
        assert_eq!(
            quote(MarketKind::Refinery, 0, ResourceId::Metal),
            Quote { ask: 50, bid: 46 }
        );
        // 60 * 115 / 100 = 69; half = 3.
        assert_eq!(
            quote(MarketKind::MiningOutpost, 0, ResourceId::Metal),
            Quote { ask: 72, bid: 66 }
        );
        // And with the station's own lean on top: 60 * 130 / 100 = 78,
        // half 3.
        assert_eq!(
            quote(MarketKind::MiningOutpost, 15, ResourceId::Metal),
            Quote { ask: 81, bid: 75 }
        );
        // Rounding is down at each division: 8 * 85 / 100 = 6 (6.8), and
        // the half is the floor of one euro.
        assert_eq!(
            quote(MarketKind::Orbital, 0, ResourceId::Vegetable),
            Quote { ask: 7, bid: 5 }
        );
        // Rock is worth two and the half is still a euro of it.
        assert_eq!(
            quote(MarketKind::Orbital, 0, ResourceId::Rock),
            Quote { ask: 3, bid: 1 }
        );
    }

    /// The starting leans, as they were asked for.
    #[test]
    fn each_kind_leans_the_way_it_was_asked_to() {
        use MarketKind::*;
        assert!(kind_bias(MiningOutpost, ResourceId::Ore) < 0);
        assert!(kind_bias(MiningOutpost, ResourceId::Galvum) < 0);
        assert!(kind_bias(MiningOutpost, ResourceId::Vegetable) > 0);
        assert!(kind_bias(MiningOutpost, ResourceId::Metal) > 0);
        assert!(kind_bias(MiningOutpost, ResourceId::Components) > 0);
        assert!(kind_bias(Refinery, ResourceId::Metal) < 0);
        assert!(kind_bias(Refinery, ResourceId::Ore) > 0);
        assert!(kind_bias(Orbital, ResourceId::Vegetable) < 0);
        assert!(kind_bias(Orbital, ResourceId::Tofu) < 0);
        assert!(kind_bias(Orbital, ResourceId::Fibre) < 0);
        assert!(kind_bias(Settlement, ResourceId::Vegetable) < 0);
        assert!(kind_bias(Settlement, ResourceId::Metal) > 0);
        for &resource in ResourceId::ALL.iter() {
            assert!(
                kind_bias(Relay, resource) > 0,
                "{resource:?} not dear at a relay"
            );
        }
        assert!(kind_bias(Relay, ResourceId::Vegetable) > kind_bias(Relay, ResourceId::Ore));
        assert!(kind_bias(Relay, ResourceId::Bandage) > kind_bias(Relay, ResourceId::Ore));
    }

    /// A market quotes through its own bias, and the plain one at none.
    #[test]
    fn a_market_quotes_through_its_own_lean() {
        let mut bias = Bias::NONE;
        bias.0[ResourceId::Ore as usize] = -15;
        assert!(bias.in_range());
        let desk = Market::new(MarketKind::Orbital, bias);
        assert_eq!(
            desk.quote(ResourceId::Ore),
            quote(MarketKind::Orbital, -15, ResourceId::Ore)
        );
        assert_eq!(
            desk.quote(ResourceId::Metal),
            quote(MarketKind::Orbital, 0, ResourceId::Metal)
        );
        assert_eq!(
            Market::PLAIN.quote(ResourceId::Ore),
            quote(MarketKind::Orbital, 0, ResourceId::Ore)
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
