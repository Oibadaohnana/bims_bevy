//! The world's side of the trader (task 114, [`crate::trader`]): which
//! sites of the galaxy are traders, whether one is open, the visit — on
//! the map, no room and no mission — the shelf bought from, an item
//! upgraded, and a thing sold back (October 2026; nothing is combined).
//!
//! A child of `crate::world`, as `mission.rs` is, so it reaches the
//! world's private fields; the trader's own types are `crate::trader`'s.

use super::*;
use crate::items::ItemOffer;
use crate::run::{Phase as RunPhase, Site};
use crate::trader::{self, ShelfItem, Trader};

impl World {
    // --- which sites -----------------------------------------------------------

    /// The sites made traders near home on top of the roll
    /// ([`trader::near_sites`]): what `trader_near` is derived as, at the
    /// start and at every load, behind `manufacturer_near`.
    pub(super) fn trader_near_sites(&self, galaxy: &Galaxy) -> Vec<(u32, u32)> {
        trader::near_sites(galaxy, self.home_star, |star, system, station| {
            self.trader_eligible(star, &system.stations, station)
        })
    }

    /// Whether a station of `star`'s system, whose stations are `stations`,
    /// could be a trader at all ([`trader::eligible`]) — and, since a
    /// system with a trader offers the trader alone (the galaxy-only map),
    /// only the system's primary, never at an elite's and never in the
    /// crew's home system. Under the tests' whole-systems dial any station.
    fn trader_eligible(
        &self,
        star: u32,
        stations: &[worldgen::StationBlueprint],
        station: &worldgen::StationBlueprint,
    ) -> bool {
        (self.whole_systems
            || (star != self.home_star
                && star != self.droid_origin
                && Some(station.id) == super::offered::primary(stations)
                && !crate::elite::holds(self.galaxy_seed, self.home_star, star)))
            && trader::eligible(
                station,
                star == self.home_star && station.id == self.home,
                self.is_manufacturer_site(star, station),
            )
    }

    /// Whether a station of `star`'s system is a **trader**: its system's
    /// [`trader::pick`], and the system rolled one or made up near home.
    /// Stateless: a function of the galaxy and the crew's own star.
    pub fn is_trader_station(
        &self,
        star: u32,
        stations: &[worldgen::StationBlueprint],
        station: &worldgen::StationBlueprint,
    ) -> bool {
        self.trader_of(star, stations) == Some(station.id)
    }

    /// The trader of `star`'s system, whose stations are `stations`, if it
    /// has one: its [`trader::pick`], where the system rolled one or had
    /// one made up near home.
    pub(super) fn trader_of(
        &self,
        star: u32,
        stations: &[worldgen::StationBlueprint],
    ) -> Option<u32> {
        let near = self.trader_near.iter().any(|&(s, _)| s == star);
        if !near && !trader::rolled(self.galaxy_seed, star) {
            return None;
        }
        trader::pick(stations, |s| self.trader_eligible(star, stations, s))
            .filter(|&id| trader::holds(self.galaxy_seed, &self.trader_near, star, id))
    }

    /// The station the machines' jammer stands on in `star`'s system, whose
    /// stations are `stations`, once it falls: the lowest-numbered that is
    /// neither derived, the Manufacturers' nor the system's trader
    /// ([`trader::jammer_candidate`]) — `None` where there is none, and the
    /// machines build their own. **The one rule**: [`World::jammer_station`],
    /// the derived jammer, the list of sites and the quote all ask it.
    pub(crate) fn jammer_site_among(
        &self,
        star: u32,
        stations: &[worldgen::StationBlueprint],
    ) -> Option<u32> {
        trader::jammer_candidate(
            stations,
            |s| self.is_manufacturer_site(star, s),
            self.trader_of(star, stations),
        )
    }

    /// Every star whose system has a trader, in id order — what the galaxy
    /// chart marks. Generates the systems it has to, so it is asked once a
    /// chart rather than every frame: it is a function of the galaxy and
    /// the crew's own star alone.
    pub fn trader_stars(&self, galaxy: &Galaxy) -> Vec<u32> {
        (0..galaxy.stars.len() as u32)
            .filter(|&star| {
                let near = self.trader_near.iter().any(|&(s, _)| s == star);
                if !near && !trader::rolled(self.galaxy_seed, star) {
                    return false;
                }
                if star == self.star_id {
                    return self.trader_of(star, &self.system.stations).is_some();
                }
                galaxy
                    .system(star)
                    .is_some_and(|system| self.trader_of(star, &system.stations).is_some())
            })
            .collect()
    }

    /// Whether a site is a trader: this system's read off the world, any
    /// other's off `galaxy` (generated here when none is handed in).
    pub fn is_trader(&self, site: Site) -> bool {
        if site.star == self.star_id {
            return self.trader_in(None, site);
        }
        self.trader_in(Some(&self.galaxy()), site)
    }

    /// [`World::is_trader`] off a galaxy already generated.
    pub(super) fn trader_in(&self, galaxy: Option<&Galaxy>, site: Site) -> bool {
        if site.star == self.star_id {
            return self
                .system
                .station(site.station)
                .is_some_and(|s| self.is_trader_station(site.star, &self.system.stations, s));
        }
        let Some(system) = galaxy.and_then(|g| g.system(site.star)) else {
            return false;
        };
        system
            .station(site.station)
            .is_some_and(|s| self.is_trader_station(site.star, &system.stations, s))
    }

    /// Whether a station of this system is a trader: what the crisis
    /// passes by.
    pub fn is_trader_here(&self, id: u32) -> bool {
        self.trader_in(
            None,
            Site {
                star: self.star_id,
                station: id,
            },
        )
    }

    /// Every trader among the places a trip can go from here — what the
    /// map marks. The galaxy is generated once for all of them.
    pub fn trader_sites(&self) -> Vec<Site> {
        let galaxy = self.galaxy();
        self.destinations()
            .into_iter()
            .filter(|&site| self.trader_in(Some(&galaxy), site))
            .collect()
    }

    // --- open and closed ------------------------------------------------------

    /// Whether a star's system is **liberated**: the machines have had it,
    /// and every one of its sites they took — station, jammer, town — is
    /// cleared. This system's is read off the world, any other's off its
    /// memory; a system the crew have never been in since it fell is not.
    /// The Manufacturers' sites are not the machines' and are not asked.
    pub fn liberated(&self, star: u32) -> bool {
        let held: &[Infestation] = if star == self.star_id {
            &self.infested
        } else {
            match self.memories.iter().find(|m| m.star == star) {
                Some(m) => &m.infested,
                None => return false,
            }
        };
        let mut any = false;
        for it in held.iter().filter(|it| !it.manufacturers) {
            if !it.cleared {
                return false;
            }
            any = true;
        }
        any
    }

    /// Whether a trader of `star` is closed on `day`: its system the
    /// machines' by then and not liberated. The crisis is a function of the
    /// day, so the answer for the day the crew would arrive is exact.
    pub fn trader_closed_on(&self, star: u32, day: u32) -> bool {
        let turns = self.infested_on(star);
        turns != u32::MAX && day >= turns && !self.liberated(star)
    }

    // --- the visit --------------------------------------------------------------

    /// Whether the crew are at a trader: arrived, the map up.
    pub fn at_trader(&self) -> bool {
        self.run.phase == RunPhase::Trade
    }

    /// Player `slot`'s own trader where the crew are at one, with what is
    /// left on its shelf.
    pub fn trader_here(&self, slot: u32) -> Option<&Trader> {
        let at = self.trader_index(slot)?;
        self.run.traders.get(at)
    }

    /// Where player `slot`'s trader at the crew's site is kept on the run.
    fn trader_index(&self, slot: u32) -> Option<usize> {
        if !self.at_trader() {
            return None;
        }
        let site = self.current_site()?;
        self.run
            .traders
            .iter()
            .position(|t| t.site == site && t.owner == slot)
    }

    /// Every trader the crew have been to this run, with what is left.
    pub fn traders_met(&self) -> &[Trader] {
        &self.run.traders
    }

    /// Arrived at a trader (from `travel`): the ship holding off it, the
    /// run in [`RunPhase::Trade`] and nothing else — no room built, no
    /// mission begun, neither clock touched, nothing a mission's start
    /// does. The first time the crew are here the trader is met, its shelf
    /// rolled; every visit after rolls it again.
    pub(super) fn arrive_at_trader(&mut self, site: Site) {
        let at = self.site_position(&self.system, site.station);
        self.ship.state = ShipState::Holding;
        self.ship.frame = Frame::Local(Node::Station(site.station));
        if let Some(at) = at {
            self.ship.set_position(at);
        }
        self.mark_visited();
        self.run.phase = RunPhase::Trade;
        self.run.site = Some(site.station);
        self.run.snapshot = None;
        self.run.snapped = false;
        self.run.proposal = None;
        self.run.departure = None;
        // A trader of their own for every player not met here yet. The
        // shelf is put up afresh every visit, every kind at its tier for
        // the player (October 2026). No trader sells a relic.
        for at in 0..self.run.traders.len() {
            if self.run.traders[at].site == site {
                let shelf = self.shelf_for(self.run.traders[at].owner);
                self.run.traders[at].restock(shelf);
            }
        }
        for owner in 0..self.players() {
            if self
                .run
                .traders
                .iter()
                .any(|t| t.site == site && t.owner == owner)
            {
                continue;
            }
            let at = self
                .run
                .traders
                .partition_point(|t| (t.site, t.owner) < (site, owner));
            self.run
                .traders
                .insert(at, Trader::new(site, owner, self.shelf_for(owner)));
        }
    }

    /// What a thing off the shelf costs a player: the trader's own ask
    /// for it at its tier (`World::quote_at`, the existing tier pricing),
    /// else the book at the tier.
    pub fn shelf_price(&self, item: ShelfItem) -> Money {
        let tier = item.tier.code();
        let ask = self
            .run
            .site
            .and_then(|id| self.quote_at(id, item.resource, tier))
            .map(|q| q.ask)
            .unwrap_or_else(|| {
                economy::trade_price(item.resource).saturating_mul(economy::tier_price(tier))
            });
        // The minigun at three times its ask (October 2026).
        let ask = if item.resource == ResourceId::Minigun {
            ask.saturating_mul(data::MINIGUN_SHELF_PRICE)
        } else {
            ask
        };
        // The reward dials' shelf per cent, then the crew's relics. The
        // whole price to every player: each wallet already holds only its
        // share of what the crew earn, so the price was over the players
        // until October 2026 (the player's word), and four players bought
        // what a lone one would with four times the money.
        self.trader_price_by_relics(self.rewards.shelf_price(ask))
    }

    // --- buying ---------------------------------------------------------------

    /// [`Command::BuyShelf`]: the thing in the slot `index` of player
    /// `slot`'s own shelf paid for out of its own wallet and onto crew member `to`'s loadout — what was there
    /// into the player's own armory — or into it with `None`. Any player, no
    /// vote; the first command to want a thing has it.
    pub(super) fn buy_shelf(
        &mut self,
        slot: u32,
        index: u32,
        to: Option<u32>,
        events: &mut Vec<WorldEvent>,
    ) {
        let Some(at) = self.trader_index(slot) else {
            events.push(refused(slot, Refusal::NotAtATrader));
            return;
        };
        let Some(item) = self.run.traders[at]
            .shelf
            .get(index as usize)
            .copied()
            .flatten()
        else {
            events.push(refused(slot, Refusal::SoldOut));
            return;
        };
        if let Some(who) = to {
            if who >= self.aboard.crew_count() {
                events.push(refused(slot, Refusal::NotAboard));
                return;
            }
            if !self.may_change(slot, who) {
                events.push(refused(slot, Refusal::NotYours));
                return;
            }
        }
        if !self.pay_from(slot, self.shelf_price(item)) {
            events.push(refused(slot, Refusal::Unaffordable));
            return;
        }
        self.run.traders[at].shelf[index as usize] = None;
        // Bought, every later shelf of the player's sells that kind — and
        // that kind alone — a tier past it (October 2026).
        let slot_at = slot as usize;
        if self.run.shelf_bought.len() <= slot_at {
            self.run.shelf_bought.resize(slot_at + 1, Vec::new());
        }
        let bought = &mut self.run.shelf_bought[slot_at];
        match bought.iter_mut().find(|b| b.resource == item.resource) {
            Some(best) => best.tier = best.tier.max(item.tier),
            None => {
                let at = bought.partition_point(|b| (b.resource as u32) < (item.resource as u32));
                bought.insert(at, item);
            }
        }
        let thing = match item.weapon() {
            Some(weapon) => Item::Weapon(weapon),
            None => {
                let kind = item.armour().unwrap_or(ArmourKind::Armour);
                Item::Armour(self.holdings.new_piece(kind, item.tier))
            }
        };
        match (to, GearSlot::of_item(thing)) {
            (Some(who), Some(part)) => {
                let old = self.set_slot(who, part, Some(thing), events);
                events.push(WorldEvent::GearChanged {
                    who,
                    part: part.code(),
                });
                match old {
                    // The same kind a tier under is sold at once into the
                    // buyer's wallet rather than left in the armory, the
                    // player's word (October 2026).
                    Some(old) if superseded(old, thing) => {
                        let value = self.sell_value(old).unwrap_or(0);
                        self.credit(slot, value);
                        events.push(WorldEvent::Sold { slot, who, value });
                    }
                    Some(old) => {
                        self.holdings.put(slot, old);
                    }
                    None => {}
                }
            }
            _ => {
                self.holdings.put(slot, thing);
            }
        }
        events.push(WorldEvent::ShelfBought {
            slot,
            index,
            to: to.unwrap_or(u32::MAX),
        });
    }

    // --- the items (October 2026) ------------------------------------------------

    /// The tier a trader sells at today: [`crate::items::shop_tier`] off the
    /// scaling's tier days and the run day — its gun, its armour and its
    /// items alike.
    pub fn shop_tier(&self) -> Tier {
        crate::items::shop_tier(&self.scaling(), self.run_day())
    }

    /// The tier player `slot`'s shelf sells the kind `resource` at
    /// (October 2026): **tier one until that kind is bought**, then one
    /// past the best of it the player has bought off a shelf this run —
    /// tier three at most, so a tier-three thing bought leaves it at
    /// three. The day does not lift it. **A class's start counts as
    /// bought at tier one** (October 2026, the player's word, putting
    /// back what 27afab0 took out): the soldier sets out with a tier-one
    /// auto rifle and the tank in tier-one armour, so their shelves sell
    /// that kind at tier two from the first trader on.
    /// ([`trader::shelf`] puts a kind not made that low up to its own.)
    pub fn shelf_tier(&self, slot: u32, resource: ResourceId) -> Tier {
        let start = match self.class_of(slot) {
            Class::Soldier => Some(ResourceId::AutoRifle),
            Class::Tank => Some(ResourceId::Armour),
            _ => None,
        };
        self.run
            .shelf_bought
            .get(slot as usize)
            .and_then(|b| b.iter().find(|b| b.resource == resource))
            .map(|b| b.tier)
            .or_else(|| (start == Some(resource)).then_some(Tier::One))
            .map_or(Tier::One, |t| t.next().unwrap_or(t))
    }

    /// Player `slot`'s shelf for a visit ([`trader::shelf`]): every gun but
    /// the pistol and the armour, each at [`World::shelf_tier`].
    pub fn shelf_for(&self, slot: u32) -> Vec<ShelfItem> {
        trader::shelf(|resource| self.shelf_tier(slot, resource))
    }

    /// The trader's item shelf today ([`crate::items::shop`]): every kind
    /// at the day's tier, the same for every player; one of each a visit
    /// ([`World::item_sold`]).
    pub fn item_shelf(&self) -> Vec<bims::module::Module> {
        crate::items::shop(self.shop_tier())
    }

    /// What an item costs a player: [`crate::items::price`], through
    /// the reward dials' shelf per cent and the crew's relics, as a thing
    /// off the shelf is.
    pub fn item_price(&self, item: bims::module::Module) -> Money {
        let price = self.rewards.shelf_price(crate::items::price(item));
        self.trader_price_by_relics(price)
    }

    /// Whether player `slot` has bought the item of `kind` (its code) off
    /// its trader this visit: sold out until the next.
    pub fn item_sold(&self, slot: u32, kind: u32) -> bool {
        self.trader_here(slot)
            .is_some_and(|t| t.items_sold.contains(&kind))
    }

    /// What the item line of `kind` (its code) at player `slot`'s trader
    /// sells it today ([`ItemOffer`]): the kind's upgrade where the
    /// player's own Bim carries one — the next tier whatever the day, or
    /// nothing past the top — else the kind off today's shelf. `None` for
    /// a kind not on the shelf and not carried, or for no Bim of its own.
    pub fn item_offer(&self, slot: u32, kind: u32) -> Option<ItemOffer> {
        let kind = bims::module::ModuleKind::from_code(kind)?;
        if slot >= self.players() || slot >= self.aboard.crew_count() {
            return None;
        }
        let gear = self.aboard.room.gear(slot as usize);
        let held = gear
            .items
            .iter()
            .enumerate()
            .find_map(|(at, m)| m.filter(|m| m.kind == kind).map(|m| (at as u32, m)));
        if let Some((at, from)) = held {
            return Some(match crate::items::upgraded(from) {
                Some(to) => ItemOffer::Upgrade { at, from, to },
                None => ItemOffer::Top(from),
            });
        }
        self.item_shelf()
            .into_iter()
            .find(|m| m.kind == kind)
            .map(ItemOffer::Buy)
    }

    /// What an offer costs player `slot`: the item's price at the tier it
    /// would be bought or upgraded to — the next tier's whole price for an
    /// upgrade — and nothing for one at its top.
    pub fn item_offer_price(&self, offer: ItemOffer) -> Option<Money> {
        match offer {
            ItemOffer::Buy(item) => Some(self.item_price(item)),
            ItemOffer::Upgrade { to, .. } => Some(self.item_price(to)),
            ItemOffer::Top(_) => None,
        }
    }

    /// [`Command::BuyItem`]: the item line of `kind` at player `slot`'s
    /// trader ([`World::item_offer`]), paid out of its own wallet — a new
    /// item onto its own Bim's first free item slot, or the one it carries
    /// a tier up in its slot. Never into the armory, never onto a bot.
    /// One of a kind a visit, bought or upgraded.
    pub(super) fn buy_item(&mut self, slot: u32, kind: u32, events: &mut Vec<WorldEvent>) {
        let Some(at) = self.trader_index(slot) else {
            events.push(refused(slot, Refusal::NotAtATrader));
            return;
        };
        if self.run.traders[at].items_sold.contains(&kind) {
            events.push(refused(slot, Refusal::SoldOut));
            return;
        }
        let Some(offer) = self.item_offer(slot, kind) else {
            events.push(refused(slot, Refusal::NotForSale));
            return;
        };
        let (part, item, upgrade) = match offer {
            ItemOffer::Top(_) => {
                events.push(refused(slot, Refusal::TopTier));
                return;
            }
            ItemOffer::Buy(item) => match self.item_slot_for(slot, None) {
                Ok(part) => (part, item, false),
                Err(why) => {
                    events.push(refused(slot, why));
                    return;
                }
            },
            ItemOffer::Upgrade { at, to, .. } => {
                let Some(part) = GearSlot::item(at as usize) else {
                    events.push(refused(slot, Refusal::NoSuchGear));
                    return;
                };
                (part, to, true)
            }
        };
        let price = self.item_price(item);
        if !self.pay_from(slot, price) {
            events.push(refused(slot, Refusal::Unaffordable));
            return;
        }
        self.run.traders[at].items_sold.push(kind);
        let made = bims::module::Module {
            paid: item.paid.saturating_add(price),
            ..item
        };
        self.set_slot(slot, part, Some(Item::Module(made)), events);
        events.push(WorldEvent::GearChanged {
            who: slot,
            part: part.code(),
        });
        events.push(WorldEvent::ItemBought {
            slot,
            kind: made.kind.code(),
            tier: made.tier.code(),
            to: slot,
            upgrade,
        });
    }

    // --- selling (October 2026) -------------------------------------------------

    /// What a thing fetches sold back at a trader: half of what was paid
    /// for it ([`data::SELL_BACK_PERCENT`]) — an item's own
    /// [`bims::module::Module::paid`], every upgrade in it, or for one
    /// never bought (a probe's, a save's from before) its tier's price
    /// today; a weapon or a piece its shelf price at its tier today. A
    /// charge is not sold.
    pub fn sell_value(&self, thing: Item) -> Option<Money> {
        let paid = match thing {
            Item::Module(m) if m.paid > 0 => m.paid,
            Item::Module(m) => self.item_price(m),
            Item::Weapon(w) => self.shelf_price(ShelfItem {
                resource: armour::weapon_resource(w.kind),
                tier: w.tier,
            }),
            Item::Armour(p) => self.shelf_price(ShelfItem {
                resource: armour::resource_of(p.kind),
                tier: p.tier,
            }),
            Item::Stack(_) => return None,
        };
        Some(paid.saturating_mul(data::SELL_BACK_PERCENT) / 100)
    }

    /// What player `slot` may sell from `from`, if it may: a thing in its
    /// own armory, or on its own Bim's slot — never a bot's or another
    /// player's — and never a charge or a laser pistol (`NotSellable`).
    pub fn sellable(&self, slot: u32, from: GearSource) -> Result<(Item, Money), Refusal> {
        let thing = match from {
            GearSource::Armory { id } => {
                let stored = self.holdings.get(id).ok_or(Refusal::NoSuchGear)?;
                if stored.owner != slot {
                    return Err(Refusal::NotYours);
                }
                stored.item
            }
            GearSource::Worn { who, slot: part } => {
                if who >= self.aboard.crew_count() {
                    return Err(Refusal::NotAboard);
                }
                if !self.may_change(slot, who) {
                    return Err(Refusal::NotYours);
                }
                self.worn_on(who, part).ok_or(Refusal::NoSuchGear)?
            }
        };
        // The laser pistol every Bim sets out with — a player's and a
        // bot's — is never sold (October 2026); no trader sells one either.
        if matches!(thing, Item::Weapon(w) if w.kind == WeaponKind::LaserPistol) {
            return Err(Refusal::NotSellable);
        }
        let value = self.sell_value(thing).ok_or(Refusal::NoSuchGear)?;
        Ok((thing, value))
    }

    /// [`Command::Sell`]: the thing at `from` sold at the trader player
    /// `slot` is at, for [`World::sell_value`] into its own wallet — out of
    /// its own armory, or off its own Bim, the slot left empty.
    pub(super) fn sell(&mut self, slot: u32, from: GearSource, events: &mut Vec<WorldEvent>) {
        if self.trader_index(slot).is_none() {
            events.push(refused(slot, Refusal::NotAtATrader));
            return;
        }
        let (_, value) = match self.sellable(slot, from) {
            Ok(sold) => sold,
            Err(why) => {
                events.push(refused(slot, why));
                return;
            }
        };
        let who = match from {
            GearSource::Armory { id } => {
                self.holdings.take(id);
                u32::MAX
            }
            GearSource::Worn { who, slot: part } => {
                self.set_slot(who, part, None, events);
                events.push(WorldEvent::GearChanged {
                    who,
                    part: part.code(),
                });
                who
            }
        };
        self.credit(slot, value);
        events.push(WorldEvent::Sold { slot, who, value });
    }
}

impl World {
    /// A probe's way to a trader (`BIMS_TRADER=1`): the mission left as
    /// the button leaves it, and the trip to the first open trader a trip
    /// can go to taken at once, without the vote. The trader, or `None`
    /// with none in reach.
    pub fn trader_for_probe(&mut self) -> Option<Site> {
        if self.in_mission() {
            self.leave_for_probe();
        }
        self.run.phase = RunPhase::Map;
        self.run.relics.choice = None;
        let site = self
            .trader_sites()
            .into_iter()
            .find(|&s| self.travel_quote(s).is_ok())?;
        let quote = self.travel_quote(site).ok()?;
        let mut events = Vec::new();
        self.travel(quote, &mut events);
        self.at_trader().then_some(site)
    }
}

/// Whether `new`, bought onto a slot, supersedes `old` that was there: the
/// same kind of weapon or armour at a lower tier (October 2026).
fn superseded(old: Item, new: Item) -> bool {
    match (old, new) {
        (Item::Weapon(o), Item::Weapon(n)) => o.kind == n.kind && o.tier < n.tier,
        (Item::Armour(o), Item::Armour(n)) => o.kind == n.kind && o.tier < n.tier,
        _ => false,
    }
}
