//! Money, and the only sums anybody is allowed to do with it.
//!
//! Nobody starts with stores any more. Each Bim brings **money**, all of it
//! goes into **one shared pool**, and everything in the design phase is
//! bought out of that pool: the parts of the ship, and the goods the crew
//! load into it. This crate is that pool's arithmetic and nothing else — it
//! renders nothing, it knows no parts, and its only dependency is `physics`,
//! for [`ResourceId`].
//!
//! # What a thing is worth, and what a desk charges for it
//!
//! Two numbers, and they are kept apart on purpose. [`trade_price`] is the
//! **book value** of a unit — what a resource is *worth*, the same
//! everywhere, and the one number a hold is valued at (`Budget::spent`,
//! `World::worth`). Nothing is bought or sold at it. What a station's desk
//! charges is the [`market`]: the book leaned on by the kind of station
//! and by the station's own roll, then split into an ask and a bid, so
//! that a crate of vegetables is cheap at the settlement that grows it
//! and dear at the relay that has to have it shipped in. *Whether* a
//! station sells a thing is a different question again and is the
//! station's — `worldgen::StationKind::sells` for the food and the
//! station's own `Stock` for the gear — since this crate knows no
//! stations.
//!
//! Supply is unlimited. What bounds a purchase is **the ship**: goods are
//! stowed, and [`storage`] says in what — food in a cold store,
//! everything else in a locker. A ship with nowhere to put a thing cannot
//! buy it.
//!
//! Three things it is built around:
//!
//! - **A [`Money`] is a whole number of euros.** Never a float. Two players
//!   have to end up with the same pool down to the last euro, and `0.1 + 0.2`
//!   is a promise nobody made. The euro sign and the digit grouping live in
//!   the host — no strings cross the wasm boundary, and none are made here.
//! - **Every sum is checked.** Overflow is an [`EconomyError`], never a wrap
//!   and never a saturation. A wrap would hand somebody a fortune and a
//!   saturation would quietly make two different lobbies agree; both are
//!   worse than a refusal that says so.
//! - **It compiles for native and for wasm32 and gives the same answers.**
//!   There is no `usize`, no float and no hashing in here for exactly that
//!   reason. The wasm half of the check is `ship_self_check` in
//!   `crates/ship`; the native half is the tests below.

pub mod market;

use physics::ResourceId;

/// Whole euros. There are no cents: a part costs what it costs, and a
/// fractional euro is a rounding rule two machines could disagree about.
pub type Money = u64;

/// What a lone player gets on top of their own money.
///
/// A ship is a ship whether one person or four are paying for it — the hull,
/// the galley and the heads cost the same — so somebody playing alone would
/// otherwise be building a quarter of a ship. The bonus is a **placeholder**
/// like every other number in the game so far, and it is deliberately a lump
/// rather than a multiplier: it is meant to cover the fixed part of a ship,
/// and the fixed part does not scale.
pub const SOLO_BONUS: Money = 20_000;

/// Why a sum could not be done.
///
/// The discriminants are written out because they will one day cross the wasm
/// boundary the way [`crate::Money`] does; `0` is left free for "nothing went
/// wrong", which is the shape every other code in this workspace has.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EconomyError {
    /// A game with nobody in it. There is no pool to build one.
    NoPlayers = 1,
    /// The sum does not fit in a [`Money`]. A refusal rather than a wrap.
    Overflow = 2,
}

impl EconomyError {
    pub fn code(self) -> u32 {
        self as u32
    }
}

/// What the crew have between them at the start of the design phase.
///
/// Everybody's money goes into **one pool**: there is one ship and it is
/// built together, so there is nothing for a per-player purse to mean. A lone
/// player gets [`SOLO_BONUS`] on top, because the ship they have to build is
/// the same size as everybody else's.
///
/// Checked throughout. `player_count == 0` is an error rather than an empty
/// pool — a game with no players is a question nobody meant to ask, and
/// answering "nothing" would let it start.
pub fn starting_pool(money_per_bim: Money, player_count: u32) -> Result<Money, EconomyError> {
    if player_count == 0 {
        return Err(EconomyError::NoPlayers);
    }
    let brought = money_per_bim
        .checked_mul(player_count as Money)
        .ok_or(EconomyError::Overflow)?;
    if player_count == 1 {
        brought
            .checked_add(SOLO_BONUS)
            .ok_or(EconomyError::Overflow)
    } else {
        Ok(brought)
    }
}

/// `a + b`, or a refusal. The one way two amounts of money are added.
pub fn add(a: Money, b: Money) -> Result<Money, EconomyError> {
    a.checked_add(b).ok_or(EconomyError::Overflow)
}

// --- what a station sells, and where it goes ------------------------------

/// Where a resource is stowed aboard.
///
/// Not a part: several parts can be a locker between them, and the question
/// a purchase asks is "is there room in the **class**", not "is there room
/// in that cupboard". `PartDef::capacity` in `shipdesign` is what says which
/// part provides which class and how much of it.
///
/// There was a `Shelf` at `0` until the money rework (feature 95) took the
/// materials away: with nothing left that keeps on racking, a shelf is a
/// locker's worth of room and `PartKind::Shelf` provides the locker class.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Storage {
    /// Food, which goes off.
    ColdStore = 0,
    /// Things worn or carried: suits, guns, armour, medicine. A suit
    /// locker, an armoury and a shelf all provide it.
    Locker = 1,
    /// A research desk's own slot: where a research key sits until it is
    /// consumed. One a desk, and nothing else goes in it.
    Research = 2,
}

impl Storage {
    /// Every class, in discriminant order. The host builds one capacity
    /// readout per entry.
    pub const ALL: [Storage; 3] = [Storage::ColdStore, Storage::Locker, Storage::Research];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<Storage> {
        Storage::ALL.get(code as usize).copied()
    }
}

/// What a piece of gear at each tier costs, as a multiple of its book
/// value at tier one: indexed by the tier's own number, so
/// `TIER_PRICE[1]` is one, `TIER_PRICE[2]` four and `TIER_PRICE[3]`
/// sixteen. Index `0` is no tier and is never asked for.
///
/// **Each step is more than double the last**, and that is the whole
/// point of the table: two of a kind combined at the workbench make one
/// of the next tier, so a player who can buy two tier-ones for less than
/// one tier-two has a reason to walk to the bench. `tier_price_doubles`
/// below is what holds it. Placeholders like every other number here.
pub const TIER_PRICE: [Money; 4] = [0, 1, 4, 16];

/// What one tier multiplies a price by — [`TIER_PRICE`] indexed safely,
/// since a tier arrives from the room as a number. One for anything that
/// is not a tier.
pub fn tier_price(tier: u32) -> Money {
    TIER_PRICE.get(tier as usize).copied().unwrap_or(1).max(1)
}

/// Whether a resource comes at a **tier**: the five weapons and the three
/// pieces of armour, the things `bims::combat` keeps a `Tier` on. What
/// [`tier_price`] applies to, and nothing else — a medkit is a medkit.
///
/// A `match` rather than a list, so a resource added to `physics` is a
/// compile error here.
pub fn tiered(resource: ResourceId) -> bool {
    match resource {
        ResourceId::Handgun
        | ResourceId::Shotgun
        | ResourceId::AutoRifle
        | ResourceId::SniperRifle
        | ResourceId::Schword
        | ResourceId::Helm
        | ResourceId::Kevlar
        | ResourceId::LegGuard => true,
        ResourceId::Vegetable
        | ResourceId::Tofu
        | ResourceId::Suit
        | ResourceId::Medkit
        | ResourceId::Bandage
        | ResourceId::ResearchKey
        | ResourceId::ResearchKeyTwo
        | ResourceId::SandbagKit
        | ResourceId::SentryKit
        | ResourceId::Grenade => false,
    }
}

/// What one unit is **worth**, in whole euros: the book value at **tier
/// one**. **Valuation only** — `Budget::spent` and `World::worth` are
/// what it is for, and nothing is bought or sold at it: every transaction
/// goes through [`market::quote`], which leans on this and splits it, and
/// a piece of gear above tier one is worth this times
/// [`TIER_PRICE`]. The same everywhere, which is what makes it a
/// valuation.
///
/// **Every number here is written in by hand.** It was worked out from a
/// recipe until the money rework (feature 95) took the recipes away;
/// there is nothing left to derive a price from, so what a thing is worth
/// is a decision rather than a sum. The gear is the part that was
/// decided rather than inherited: a pistol at fifteen hundred and a
/// sniper rifle at five thousand, and **armour at a hundred euros a point
/// of the piece's health** — fifteen for a helm, twenty for the kevlar,
/// ten for the leg guards, which is where its three numbers come from.
///
/// A `match` rather than a table so that a new [`ResourceId`] is a compile
/// error here rather than a resource that is quietly worth nothing.
pub fn trade_price(resource: ResourceId) -> Money {
    match resource {
        // The food, cheap: what a crew eat.
        ResourceId::Vegetable => 8,
        ResourceId::Tofu => 12,
        // A pressure suit. What a walk outside the hull wants.
        ResourceId::Suit => 2_500,
        // The five weapons, at tier one.
        ResourceId::Handgun => 1_500,
        ResourceId::Shotgun => 3_000,
        ResourceId::AutoRifle => 4_000,
        ResourceId::SniperRifle => 5_000,
        ResourceId::Schword => 5_000,
        // The three pieces of armour, at a hundred a point of health:
        // fifteen, twenty and ten (`bims::balance`).
        ResourceId::LegGuard => 1_000,
        ResourceId::Helm => 1_500,
        ResourceId::Kevlar => 2_000,
        // Two vegetables and a quarter of an hour at the drug lab — the
        // one thing the crew still make — and a dressing off a shelf.
        ResourceId::Medkit => 32,
        ResourceId::Bandage => 12,
        // Found, never made and never sold; a station pays for one as a
        // curiosity, which is a great deal less than what it opens.
        ResourceId::ResearchKey => 5_000,
        // The tier-two key, off an enemy's desk: twice the tier-one's.
        ResourceId::ResearchKeyTwo => 10_000,
        // A class's charges (features 88 and 90): they come back on a
        // cooldown rather than being bought, and nobody stocks one, but a
        // station will buy one off a pack and a price is what it pays.
        ResourceId::SandbagKit => 60,
        ResourceId::SentryKit => 880,
        ResourceId::Grenade => 80,
    }
}

/// What `units` of it are worth at the book, or a refusal. Checked like
/// every other sum here: a hold of four billion vegetables is a refusal,
/// not a wrap into a hold worth nothing. Valuation, like [`trade_price`];
/// what an order *costs* is [`market::Quote::cost`].
pub fn trade_value(resource: ResourceId, units: u32) -> Result<Money, EconomyError> {
    market::value(trade_price(resource), units)
}

/// Where a resource is stowed. Same reason for the `match` as above.
pub fn storage(resource: ResourceId) -> Storage {
    match resource {
        ResourceId::Vegetable | ResourceId::Tofu => Storage::ColdStore,
        ResourceId::Suit
        | ResourceId::Handgun
        | ResourceId::Medkit
        | ResourceId::Bandage
        | ResourceId::Helm
        | ResourceId::Kevlar
        | ResourceId::LegGuard
        | ResourceId::Shotgun
        | ResourceId::AutoRifle
        | ResourceId::SniperRifle
        | ResourceId::Schword
        | ResourceId::SandbagKit
        | ResourceId::SentryKit
        | ResourceId::Grenade => Storage::Locker,
        ResourceId::ResearchKey | ResourceId::ResearchKeyTwo => Storage::Research,
    }
}

/// How much of a locker a thing takes up: so many rows by so many
/// columns of the lockers' grid, the way an inventory in a survival game
/// lays gear out — a rifle lies along a row, a helm is a wide square. Either
/// way round: a thing can be turned in its locker, and its footprint
/// turned with it (`turned()`). Nothing here is a float or a `usize`, for
/// the reason the module note gives; the number that goes in a hash is
/// [`Footprint::cells`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Footprint {
    pub rows: u8,
    pub cols: u8,
}

impl Footprint {
    pub const fn new(rows: u8, cols: u8) -> Footprint {
        Footprint { rows, cols }
    }

    /// The same thing turned a quarter round.
    pub fn turned(self) -> Footprint {
        Footprint {
            rows: self.cols,
            cols: self.rows,
        }
    }

    /// How many cells it covers, whichever way round.
    pub fn cells(self) -> u32 {
        self.rows as u32 * self.cols as u32
    }
}

/// The room one stack of a resource takes in its class. **Every class but
/// the research desk is a grid**: a cold store, a suit locker, a shelf,
/// an armoury or a drug lab is so many cells (`shipdesign::PartDef::capacity`),
/// and each thing kept there covers its footprint of them — a pistol a
/// row of two, a sniper rifle a row of ten, a kevlar vest four by four, a crate
/// of vegetables one by two, a block of tofu four by four. Goods that
/// stack ([`stack_size`]) cover one footprint a stack, however full the
/// stack is, so what a locker holds is its cells times the stacks. The
/// desk's one slot is one cell.
///
/// A `match` rather than a table, like [`storage`], so that a new
/// [`ResourceId`] is a compile error here rather than a thing that
/// quietly takes no room.
pub fn footprint(resource: ResourceId) -> Footprint {
    match resource {
        ResourceId::ResearchKey | ResourceId::ResearchKeyTwo => Footprint::new(1, 1),
        // The guns lie along a row: the pistol short, the shotgun broad,
        // the sniper rifle the whole width of a locker.
        ResourceId::Handgun => Footprint::new(1, 2),
        ResourceId::Shotgun => Footprint::new(2, 5),
        ResourceId::AutoRifle => Footprint::new(1, 7),
        ResourceId::SniperRifle => Footprint::new(1, 10),
        ResourceId::Schword => Footprint::new(1, 5),
        // The armour: the kevlar is a square, a helm lies on its side, the
        // leg guards stand.
        ResourceId::Kevlar => Footprint::new(4, 4),
        ResourceId::Helm => Footprint::new(2, 4),
        ResourceId::LegGuard => Footprint::new(3, 2),
        // The suit folded, a medkit's case, and a box of dressings — a
        // bandage is gauze and tape the size of a medkit's case
        // (feature 87), and five of them go in one box (`stack_size`).
        ResourceId::Suit => Footprint::new(3, 3),
        ResourceId::Medkit => Footprint::new(2, 2),
        ResourceId::Bandage => Footprint::new(2, 2),
        // The engineer's kits: a sack of sandbags, a sentry's crate.
        ResourceId::SandbagKit => Footprint::new(2, 2),
        ResourceId::SentryKit => Footprint::new(2, 3),
        // A grenade, one to a cell.
        ResourceId::Grenade => Footprint::new(1, 1),
        // The food: a crate of vegetables, a block of tofu.
        ResourceId::Vegetable => Footprint::new(1, 2),
        ResourceId::Tofu => Footprint::new(4, 4),
    }
}

/// How many units of a resource go in one stack — one footprint of its
/// class's grid. What bounds a locker: a hundred cells hold a hundred
/// medkits, or one thing each of what does not stack. A `match`, so a new
/// resource is a compile error rather than a stack of nought.
pub fn stack_size(resource: ResourceId) -> u32 {
    match resource {
        // The food, by the crate.
        ResourceId::Vegetable | ResourceId::Tofu => 10,
        // The dressings, by the box: five to a footprint (feature 87), so
        // one 2x2 of a pack or a locker holds a fight's worth of them.
        ResourceId::Bandage => 5,
        // Everything worn, held or dressed with is one to a footprint.
        ResourceId::Suit
        | ResourceId::Handgun
        | ResourceId::Medkit
        | ResourceId::Helm
        | ResourceId::Kevlar
        | ResourceId::LegGuard
        | ResourceId::Shotgun
        | ResourceId::AutoRifle
        | ResourceId::SniperRifle
        | ResourceId::Schword
        | ResourceId::ResearchKey
        | ResourceId::ResearchKeyTwo
        | ResourceId::SandbagKit
        | ResourceId::SentryKit
        | ResourceId::Grenade => 1,
    }
}

/// How many stacks `units` of a resource are: the last one part full.
pub fn stacks_of(resource: ResourceId, units: u32) -> u32 {
    units.div_ceil(stack_size(resource).max(1))
}

/// How many cells of its class one stack of a resource takes: the
/// footprint's area.
pub fn cells(resource: ResourceId) -> u32 {
    footprint(resource).cells()
}

/// Everything in `amounts`, added up, or a refusal if it does not fit.
pub fn total(amounts: impl IntoIterator<Item = Money>) -> Result<Money, EconomyError> {
    amounts.into_iter().try_fold(0, add)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three the lobby offers, at each crew size it can have.
    /// Never a wrap and never a saturation: both would be a lobby quietly
    /// disagreeing with the one next to it about what there is to spend.
    #[test]
    fn the_pool_adds_up_refuses_nobody_and_never_wraps() {
        // --- the_lobby_presets_all_add_up ---
        {
            for &each in &[50_000, 100_000, 200_000] {
                assert_eq!(starting_pool(each, 1), Ok(each + SOLO_BONUS));
                for players in 2..=4 {
                    assert_eq!(starting_pool(each, players), Ok(each * players as Money));
                }
            }
        }

        // --- a_game_with_nobody_in_it_is_refused ---
        {
            assert_eq!(starting_pool(100_000, 0), Err(EconomyError::NoPlayers));
            assert_eq!(starting_pool(0, 0), Err(EconomyError::NoPlayers));
        }

        // --- overflow_is_an_error_rather_than_a_fortune ---
        {
            assert_eq!(starting_pool(Money::MAX, 2), Err(EconomyError::Overflow));
            // The multiply fits and the bonus is what tips it over.
            assert_eq!(starting_pool(Money::MAX, 1), Err(EconomyError::Overflow));
            assert_eq!(
                starting_pool(Money::MAX - SOLO_BONUS, 1),
                Ok(Money::MAX),
                "the largest pool there is still has to be reachable",
            );
            assert_eq!(add(Money::MAX, 1), Err(EconomyError::Overflow));
            assert_eq!(total([Money::MAX, 1]), Err(EconomyError::Overflow));
        }

        // --- nothing_is_nothing ---
        {
            assert_eq!(starting_pool(0, 4), Ok(0));
            assert_eq!(starting_pool(0, 1), Ok(SOLO_BONUS));
            assert_eq!(total([]), Ok(0));
            assert_eq!(total([1, 2, 3]), Ok(6));
        }
    }

    /// Every resource has a price and somewhere to put it. A `match` makes
    /// that a compile error rather than a test failure, and this is here for
    /// the part the compiler cannot check: that no price is nought — a free
    /// resource is one a player would buy an unbounded amount of.
    #[test]
    fn every_resource_is_priced_and_stowable() {
        for &id in ResourceId::ALL.iter() {
            assert!(trade_price(id) > 0, "{id:?} is free");
            assert!(Storage::ALL.contains(&storage(id)), "{id:?}");
        }
        assert_eq!(trade_price(ResourceId::Vegetable), 8);
        assert_eq!(trade_price(ResourceId::Tofu), 12);
        assert_eq!(trade_price(ResourceId::Suit), 2_500);
        assert_eq!(trade_price(ResourceId::Medkit), 32);
        assert_eq!(trade_price(ResourceId::Bandage), 12);
        // The five weapons and the three pieces of armour, as feature 95
        // asked for them.
        assert_eq!(trade_price(ResourceId::Handgun), 1_500);
        assert_eq!(trade_price(ResourceId::Shotgun), 3_000);
        assert_eq!(trade_price(ResourceId::AutoRifle), 4_000);
        assert_eq!(trade_price(ResourceId::SniperRifle), 5_000);
        assert_eq!(trade_price(ResourceId::Schword), 5_000);
        assert_eq!(trade_price(ResourceId::LegGuard), 1_000);
        assert_eq!(trade_price(ResourceId::Helm), 1_500);
        assert_eq!(trade_price(ResourceId::Kevlar), 2_000);

        assert_eq!(storage(ResourceId::Vegetable), Storage::ColdStore);
        assert_eq!(storage(ResourceId::Tofu), Storage::ColdStore);
        assert_eq!(storage(ResourceId::Suit), Storage::Locker);
        assert_eq!(storage(ResourceId::Handgun), Storage::Locker);
        assert_eq!(storage(ResourceId::Medkit), Storage::Locker);
        assert_eq!(storage(ResourceId::Bandage), Storage::Locker);
        assert_eq!(storage(ResourceId::Helm), Storage::Locker);
        assert_eq!(storage(ResourceId::Kevlar), Storage::Locker);
        assert_eq!(storage(ResourceId::LegGuard), Storage::Locker);
        assert_eq!(storage(ResourceId::Shotgun), Storage::Locker);
        assert_eq!(storage(ResourceId::AutoRifle), Storage::Locker);
        assert_eq!(storage(ResourceId::SniperRifle), Storage::Locker);
        assert_eq!(storage(ResourceId::Schword), Storage::Locker);
        assert_eq!(storage(ResourceId::ResearchKey), Storage::Research);
    }

    /// **Armour is a hundred euros a point of the piece's health**
    /// (feature 95). This crate knows no `bims`, so the three healths are
    /// written in here as the numbers `bims::balance` holds — a helm
    /// fifteen, the kevlar twenty, the leg guards ten — and the test is
    /// that the book values are a hundred times them.
    #[test]
    fn armour_is_a_hundred_a_point_of_health() {
        for (piece, health) in [
            (ResourceId::Helm, 15),
            (ResourceId::Kevlar, 20),
            (ResourceId::LegGuard, 10),
        ] {
            assert_eq!(trade_price(piece), 100 * health, "{piece:?}");
        }
    }

    /// The tier table, and the one inequality it exists for: each step is
    /// **more than double** the last, so combining two of a kind at the
    /// workbench is always cheaper than buying one of the tier above.
    /// Only gear is tiered; everything else is worth what it is worth.
    #[test]
    fn tier_price_more_than_doubles_at_every_step() {
        assert_eq!(TIER_PRICE[1], 1);
        assert_eq!(TIER_PRICE[2], 4);
        assert_eq!(TIER_PRICE[3], 16);
        assert!(TIER_PRICE[2] > 2 * TIER_PRICE[1]);
        assert!(TIER_PRICE[3] > 2 * TIER_PRICE[2]);
        // Asked with a number off the room: a tier, and anything that is
        // not one.
        assert_eq!(tier_price(1), 1);
        assert_eq!(tier_price(3), 16);
        assert_eq!(tier_price(0), 1);
        assert_eq!(tier_price(9), 1);
        // The eight things that come at a tier, and nothing else.
        let tiered_count = ResourceId::ALL.iter().filter(|&&r| tiered(r)).count();
        assert_eq!(tiered_count, 8);
        for gear in [
            ResourceId::Handgun,
            ResourceId::Shotgun,
            ResourceId::AutoRifle,
            ResourceId::SniperRifle,
            ResourceId::Schword,
            ResourceId::Helm,
            ResourceId::Kevlar,
            ResourceId::LegGuard,
        ] {
            assert!(tiered(gear), "{gear:?}");
        }
        for plain in [
            ResourceId::Vegetable,
            ResourceId::Suit,
            ResourceId::Medkit,
            ResourceId::Bandage,
            ResourceId::ResearchKey,
        ] {
            assert!(!tiered(plain), "{plain:?}");
        }
    }

    /// The footprints, as they were asked for: a pistol one by two, a
    /// rifle one by seven, a shotgun two by five, a sniper rifle one by
    /// ten; a kevlar vest four by four, a helm two by four, leg guards
    /// three by two. A key is one cell, so the desk's slot counts as it
    /// always did; and turning a thing keeps its area.
    #[test]
    fn a_thing_takes_its_footprint_in_a_locker_and_one_cell_elsewhere() {
        assert_eq!(footprint(ResourceId::Handgun), Footprint::new(1, 2));
        assert_eq!(footprint(ResourceId::AutoRifle), Footprint::new(1, 7));
        assert_eq!(footprint(ResourceId::Shotgun), Footprint::new(2, 5));
        assert_eq!(footprint(ResourceId::SniperRifle), Footprint::new(1, 10));
        assert_eq!(footprint(ResourceId::Kevlar), Footprint::new(4, 4));
        assert_eq!(footprint(ResourceId::Helm), Footprint::new(2, 4));
        assert_eq!(footprint(ResourceId::LegGuard), Footprint::new(3, 2));
        assert_eq!(cells(ResourceId::SniperRifle), 10);
        assert_eq!(cells(ResourceId::Kevlar), 16);
        assert_eq!(
            footprint(ResourceId::Shotgun).turned(),
            Footprint::new(5, 2)
        );
        assert_eq!(footprint(ResourceId::Vegetable), Footprint::new(1, 2));
        assert_eq!(footprint(ResourceId::Tofu), Footprint::new(4, 4));
        for &id in ResourceId::ALL.iter() {
            let f = footprint(id);
            assert!(f.rows >= 1 && f.cols >= 1, "{id:?}");
            assert_eq!(f.turned().cells(), f.cells(), "{id:?}");
            assert!(stack_size(id) >= 1, "{id:?}");
            if storage(id) == Storage::Research {
                assert_eq!(f, Footprint::new(1, 1), "{id:?}: the desk's one slot");
            }
        }
        // Stacks: ten vegetables to a crate, one gun; forty-one
        // vegetables is five crates.
        assert_eq!(stack_size(ResourceId::Vegetable), 10);
        assert_eq!(stack_size(ResourceId::Bandage), 5);
        assert_eq!(stack_size(ResourceId::SniperRifle), 1);
        assert_eq!(stacks_of(ResourceId::Vegetable, 41), 5);
        assert_eq!(stacks_of(ResourceId::Vegetable, 40), 4);
        assert_eq!(stacks_of(ResourceId::Vegetable, 0), 0);
    }

    #[test]
    fn an_order_is_priced_by_the_unit_and_cannot_wrap() {
        assert_eq!(trade_value(ResourceId::Handgun, 0), Ok(0));
        assert_eq!(trade_value(ResourceId::Handgun, 1), Ok(1_500));
        assert_eq!(trade_value(ResourceId::Handgun, 100), Ok(150_000));
        assert_eq!(trade_value(ResourceId::Tofu, 250), Ok(3_000));
        // The largest order the type can express still has to be answerable,
        // and the one that does not fit has to be a refusal.
        assert_eq!(
            trade_value(ResourceId::Vegetable, u32::MAX),
            Ok(8 * u32::MAX as Money),
        );
        assert!(Storage::from_code(3).is_none());
        assert_eq!(Storage::from_code(0), Some(Storage::ColdStore));
        assert_eq!(Storage::from_code(1), Some(Storage::Locker));
        assert_eq!(Storage::from_code(2), Some(Storage::Research));
        assert_eq!(storage(ResourceId::ResearchKey), Storage::Research);
    }
}
