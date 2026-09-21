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
//! that the same ore is cheap at the outpost that digs it and dear at
//! the relay that has to have it shipped in. *Whether* a station sells a
//! thing is a different question again and is the station's —
//! `worldgen::StationKind::sells` — since this crate knows no stations.
//!
//! Supply is unlimited. What bounds a purchase is **the ship**: goods are
//! stowed, and [`storage`] says in what — food in a cold store, fuel in a
//! tank, everything else on a shelf. A ship with nowhere to put a thing
//! cannot buy it.
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
/// Not a part: several parts can be a shelf between them, and the question a
/// purchase asks is "is there room in the **class**", not "is there room in
/// that cupboard". `PartDef::capacity` in `shipdesign` is what says which
/// part provides which class and how much of it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Storage {
    /// Racking. Ore, metal, components — anything that keeps.
    Shelf = 0,
    /// Food, which goes off.
    ColdStore = 1,
    /// Things worn or carried: suits, and later weapons and medkits. A
    /// suit locker provides it.
    Locker = 2,
    /// A research desk's own slot: where a research key sits until it is
    /// consumed. One a desk, and nothing else goes in it.
    Research = 3,
}

impl Storage {
    /// Every class, in discriminant order. The host builds one capacity
    /// readout per entry.
    pub const ALL: [Storage; 4] = [
        Storage::Shelf,
        Storage::ColdStore,
        Storage::Locker,
        Storage::Research,
    ];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<Storage> {
        Storage::ALL.get(code as usize).copied()
    }
}

/// What one unit is **worth**, in whole euros: the book value. **Valuation
/// only** — `Budget::spent` and `World::worth` are what it is for, and
/// nothing is bought or sold at it: every transaction goes through
/// [`market::quote`], which leans on this and splits it. The same
/// everywhere, which is what makes it a valuation. Placeholder numbers.
///
/// A `match` rather than a table so that a new [`ResourceId`] is a compile
/// error here rather than a resource that is quietly worth nothing.
pub fn trade_price(resource: ResourceId) -> Money {
    match resource {
        ResourceId::Ore => 20,
        ResourceId::Metal => 60,
        ResourceId::Components => 150,
        ResourceId::Vegetable => 8,
        ResourceId::Tofu => 12,
        ResourceId::Galvum => 400,
        // Nobody sells one — `worldgen::StationKind::sells` — but a station
        // will buy one, and a price is what it pays.
        ResourceId::Emitter => 900,
        ResourceId::Suit => 2_500,
        // Made, never sold, like the emitter; a station buys them.
        ResourceId::Handgun => 1_500,
        ResourceId::Vest => 800,
        ResourceId::Medkit => 120,
        // What an asteroid is skinned in. Nobody sells it and a station pays
        // next to nothing for it; it is what a pick brings back on the way to
        // the ore.
        ResourceId::Rock => 2,
        // A crop, cheaper than the vegetables it grows beside; and what the
        // drug lab rolls two of into a dressing for a wound.
        ResourceId::Fibre => 6,
        ResourceId::Bandage => 40,
        // Armour, made at the workbench and never sold; a station buys a
        // piece, and pays for the metal in it rather than the fit.
        ResourceId::Helm => 300,
        ResourceId::Kevlar => 900,
        ResourceId::LegGuard => 150,
        // The four weapons after the handgun, made at the armoury and never
        // sold, priced as it is: what went into each, and something for the
        // making. The emitters are most of it.
        ResourceId::Shotgun => 1_000,
        ResourceId::AutoRifle => 2_000,
        ResourceId::SniperRifle => 3_000,
        ResourceId::Schword => 2_500,
        // Found, never made and never sold; a station pays for one as a
        // curiosity, which is a great deal less than what it opens.
        ResourceId::ResearchKey => 5_000,
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
        ResourceId::Ore
        | ResourceId::Metal
        | ResourceId::Components
        | ResourceId::Galvum
        | ResourceId::Emitter
        | ResourceId::Rock => Storage::Shelf,
        // Fibre is a crop, and goes cold with the rest of the harvest.
        ResourceId::Vegetable | ResourceId::Tofu | ResourceId::Fibre => Storage::ColdStore,
        ResourceId::Suit
        | ResourceId::Handgun
        | ResourceId::Vest
        | ResourceId::Medkit
        | ResourceId::Bandage
        | ResourceId::Helm
        | ResourceId::Kevlar
        | ResourceId::LegGuard
        | ResourceId::Shotgun
        | ResourceId::AutoRifle
        | ResourceId::SniperRifle
        | ResourceId::Schword => Storage::Locker,
        ResourceId::ResearchKey => Storage::Research,
    }
}

/// How much of a locker a thing takes up: so many rows by so many
/// columns of the lockers' grid, the way an inventory in a survival game
/// lays gear out — a rifle lies along a row, a vest is a square. Either
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
/// the research desk is a grid**: a shelf, a cold store, a suit locker,
/// an armoury or a drug lab is so many cells (`shipdesign::PartDef::capacity`),
/// and each thing kept there covers its footprint of them — a pistol a
/// row of two, a sniper rifle a row of ten, a vest four by four, a crate
/// of vegetables one by two, a block of tofu four by four. Goods that
/// stack ([`stack_size`]) cover one footprint a stack, however full the
/// stack is, so what a shelf holds is its cells times the stacks. The
/// desk's one slot is one cell.
///
/// A `match` rather than a table, like [`storage`], so that a new
/// [`ResourceId`] is a compile error here rather than a thing that
/// quietly takes no room.
pub fn footprint(resource: ResourceId) -> Footprint {
    match resource {
        ResourceId::Ore
        | ResourceId::Metal
        | ResourceId::Components
        | ResourceId::Galvum
        | ResourceId::Emitter
        | ResourceId::Rock
        | ResourceId::Fibre
        | ResourceId::ResearchKey => Footprint::new(1, 1),
        // The guns lie along a row: the pistol short, the shotgun broad,
        // the sniper rifle the whole width of a locker.
        ResourceId::Handgun => Footprint::new(1, 2),
        ResourceId::Shotgun => Footprint::new(2, 5),
        ResourceId::AutoRifle => Footprint::new(1, 7),
        ResourceId::SniperRifle => Footprint::new(1, 10),
        ResourceId::Schword => Footprint::new(1, 5),
        // The armour: a vest is a square, a helm lies on its side, the
        // leg guards stand.
        ResourceId::Kevlar => Footprint::new(4, 4),
        ResourceId::Helm => Footprint::new(2, 4),
        ResourceId::LegGuard => Footprint::new(3, 2),
        ResourceId::Vest => Footprint::new(3, 3),
        // The suit folded, a medkit's case, a bandage rolled.
        ResourceId::Suit => Footprint::new(3, 3),
        ResourceId::Medkit => Footprint::new(2, 2),
        ResourceId::Bandage => Footprint::new(1, 1),
        // The food: a crate of vegetables, a block of tofu.
        ResourceId::Vegetable => Footprint::new(1, 2),
        ResourceId::Tofu => Footprint::new(4, 4),
    }
}

/// How many units of a resource go in one stack — one footprint of its
/// class's grid. What bounds a shelf: a hundred cells hold a thousand ore
/// in stacks of ten, or a hundred medkits, or one thing each of what does
/// not stack. A `match`, so a new resource is a compile error rather
/// than a stack of nought.
pub fn stack_size(resource: ResourceId) -> u32 {
    match resource {
        // The materials, by the sack: ore, metal, rock and fibre ten, the
        // small components twenty, the precious galvum and emitters five.
        ResourceId::Ore | ResourceId::Metal | ResourceId::Rock | ResourceId::Fibre => 10,
        ResourceId::Components => 20,
        ResourceId::Galvum | ResourceId::Emitter => 5,
        // The food, by the crate.
        ResourceId::Vegetable | ResourceId::Tofu => 10,
        // Everything worn, held or dressed with is one to a footprint.
        ResourceId::Suit
        | ResourceId::Handgun
        | ResourceId::Vest
        | ResourceId::Medkit
        | ResourceId::Bandage
        | ResourceId::Helm
        | ResourceId::Kevlar
        | ResourceId::LegGuard
        | ResourceId::Shotgun
        | ResourceId::AutoRifle
        | ResourceId::SniperRifle
        | ResourceId::Schword
        | ResourceId::ResearchKey => 1,
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
        assert_eq!(trade_price(ResourceId::Ore), 20);
        assert_eq!(trade_price(ResourceId::Metal), 60);
        assert_eq!(trade_price(ResourceId::Components), 150);
        assert_eq!(trade_price(ResourceId::Vegetable), 8);
        assert_eq!(trade_price(ResourceId::Tofu), 12);
        assert_eq!(trade_price(ResourceId::Galvum), 400);
        assert_eq!(trade_price(ResourceId::Emitter), 900);
        assert_eq!(trade_price(ResourceId::Suit), 2_500);
        assert_eq!(trade_price(ResourceId::Handgun), 1_500);
        assert_eq!(trade_price(ResourceId::Vest), 800);
        assert_eq!(trade_price(ResourceId::Medkit), 120);
        assert_eq!(trade_price(ResourceId::Rock), 2);
        assert_eq!(trade_price(ResourceId::Fibre), 6);
        assert_eq!(trade_price(ResourceId::Bandage), 40);
        assert_eq!(trade_price(ResourceId::Helm), 300);
        assert_eq!(trade_price(ResourceId::Kevlar), 900);
        assert_eq!(trade_price(ResourceId::LegGuard), 150);
        assert_eq!(trade_price(ResourceId::Shotgun), 1_000);
        assert_eq!(trade_price(ResourceId::AutoRifle), 2_000);
        assert_eq!(trade_price(ResourceId::SniperRifle), 3_000);
        assert_eq!(trade_price(ResourceId::Schword), 2_500);

        assert_eq!(storage(ResourceId::Ore), Storage::Shelf);
        assert_eq!(storage(ResourceId::Metal), Storage::Shelf);
        assert_eq!(storage(ResourceId::Components), Storage::Shelf);
        assert_eq!(storage(ResourceId::Vegetable), Storage::ColdStore);
        assert_eq!(storage(ResourceId::Tofu), Storage::ColdStore);
        assert_eq!(storage(ResourceId::Galvum), Storage::Shelf);
        assert_eq!(storage(ResourceId::Emitter), Storage::Shelf);
        assert_eq!(storage(ResourceId::Suit), Storage::Locker);
        assert_eq!(storage(ResourceId::Handgun), Storage::Locker);
        assert_eq!(storage(ResourceId::Vest), Storage::Locker);
        assert_eq!(storage(ResourceId::Medkit), Storage::Locker);
        assert_eq!(storage(ResourceId::Rock), Storage::Shelf);
        assert_eq!(storage(ResourceId::Fibre), Storage::ColdStore);
        assert_eq!(storage(ResourceId::Bandage), Storage::Locker);
        assert_eq!(storage(ResourceId::Helm), Storage::Locker);
        assert_eq!(storage(ResourceId::Kevlar), Storage::Locker);
        assert_eq!(storage(ResourceId::LegGuard), Storage::Locker);
        assert_eq!(storage(ResourceId::Shotgun), Storage::Locker);
        assert_eq!(storage(ResourceId::AutoRifle), Storage::Locker);
        assert_eq!(storage(ResourceId::SniperRifle), Storage::Locker);
        assert_eq!(storage(ResourceId::Schword), Storage::Locker);
    }

    /// The footprints, as they were asked for: a pistol one by two, a
    /// rifle one by seven, a shotgun two by five, a sniper rifle one by
    /// ten; a vest four by four, a helm two by four, leg guards three by
    /// two. Everything outside the lockers is one cell, so those classes
    /// count as they always did; and turning a thing keeps its area.
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
        // Stacks: ten ore to a cell, one gun; forty-one ore is five stacks.
        assert_eq!(stack_size(ResourceId::Ore), 10);
        assert_eq!(stack_size(ResourceId::SniperRifle), 1);
        assert_eq!(stacks_of(ResourceId::Ore, 41), 5);
        assert_eq!(stacks_of(ResourceId::Ore, 40), 4);
        assert_eq!(stacks_of(ResourceId::Ore, 0), 0);
    }

    #[test]
    fn an_order_is_priced_by_the_unit_and_cannot_wrap() {
        assert_eq!(trade_value(ResourceId::Metal, 0), Ok(0));
        assert_eq!(trade_value(ResourceId::Metal, 1), Ok(60));
        assert_eq!(trade_value(ResourceId::Metal, 100), Ok(6_000));
        assert_eq!(trade_value(ResourceId::Tofu, 250), Ok(3_000));
        // The largest order the type can express still has to be answerable,
        // and the one that does not fit has to be a refusal.
        assert_eq!(
            trade_value(ResourceId::Components, u32::MAX),
            Ok(150 * u32::MAX as Money),
        );
        assert!(Storage::from_code(4).is_none());
        assert_eq!(Storage::from_code(1), Some(Storage::ColdStore));
        assert_eq!(Storage::from_code(2), Some(Storage::Locker));
        assert_eq!(Storage::from_code(3), Some(Storage::Research));
        assert_eq!(storage(ResourceId::ResearchKey), Storage::Research);
    }
}
