//! The world's side of the trader (task 114, [`crate::trader`]): which
//! sites of the galaxy are traders, whether one is open, the visit — on
//! the map, no room and no mission — the shelf bought from, the relic's
//! vote, and two things combined into one.
//!
//! A child of `crate::world`, as `mission.rs` is, so it reaches the
//! world's private fields; the trader's own types are `crate::trader`'s.

use super::*;
use crate::relic::Relic;
use crate::run::{Phase as RunPhase, Site};
use crate::trader::{self, CombineError, ShelfItem, Trader};

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
    /// could be a trader at all ([`trader::eligible`]) — and, since a system
    /// offers one station besides its trader (task 135), never that one,
    /// the system's primary, and never in the crew's home system.
    fn trader_eligible(
        &self,
        star: u32,
        stations: &[worldgen::StationBlueprint],
        station: &worldgen::StationBlueprint,
    ) -> bool {
        (self.whole_systems
            || (star != self.home_star && Some(station.id) != super::offered::primary(stations)))
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
    /// left on its shelf and its relic.
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
    /// does. The first time the crew are here the trader is met: its shelf
    /// rolled and its relic drawn by the day's odds (task 117).
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
        // *Restock Codes* (task 118) is once a visit, and this is one.
        self.run.relics.restocked = false;
        // A trader of their own for every player not met here yet: its relic
        // drawn by the day's odds like a reward's (task 117), and kept out
        // of every other draw while it is on the table — never out of the
        // pool until it is bought — so no two players are offered one relic.
        for owner in 0..self.players() {
            if self
                .run
                .traders
                .iter()
                .any(|t| t.site == site && t.owner == owner)
            {
                continue;
            }
            let relic = self.draw_relics(1, site.station).first().copied();
            let at = self
                .run
                .traders
                .partition_point(|t| (t.site, t.owner) < (site, owner));
            self.run
                .traders
                .insert(at, Trader::new(self.galaxy_seed, site, relic, owner));
        }
    }

    /// What a thing off the shelf costs: the trader's own ask for it at its
    /// tier (`World::quote_at`, the existing tier pricing), else the book at
    /// the tier.
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
        // The reward dials' shelf per cent, then *Trade License* (task 118),
        // then the players' share.
        self.trader_share(self.trader_discount(self.rewards.shelf_price(ask)))
    }

    /// A trader's price shared by the players: each has money of their
    /// own now, a share of what the crew earn, so a thing costs each the
    /// price over the number of players, rounded up.
    pub fn trader_share(&self, price: Money) -> Money {
        price.div_ceil(Money::from(self.players().max(1)))
    }

    /// What a combining costs a player: the dials' fee, the players'
    /// share of it.
    pub fn combine_fee(&self) -> Money {
        self.trader_share(self.rewards.combine_fee)
    }

    // --- buying ---------------------------------------------------------------

    /// [`Command::BuyShelf`]: the thing in the slot `index` of player
    /// `slot`'s own shelf paid for out of its own wallet and onto crew member `to`'s loadout — what was there
    /// into the armory — or into the armory with `None`. Any player, no
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
        let price = self.shelf_price(item);
        if !self.pay_from(slot, price) {
            events.push(refused(slot, Refusal::Unaffordable));
            return;
        }
        self.run.traders[at].shelf[index as usize] = None;
        let thing = match item.weapon() {
            Some(weapon) => Item::Weapon(weapon),
            None => {
                let kind = item.armour().unwrap_or(ArmourKind::BasicHelm);
                Item::Armour(self.holdings.new_piece(kind, item.tier))
            }
        };
        match (to, GearSlot::of_item(thing)) {
            (Some(who), Some(part)) => {
                if let Some(old) = self.set_slot(who, part, Some(thing), events) {
                    self.holdings.put(old);
                }
                events.push(WorldEvent::GearChanged {
                    who,
                    part: part.code(),
                });
            }
            _ => {
                self.holdings.put(thing);
            }
        }
        events.push(WorldEvent::ShelfBought {
            slot,
            index,
            to: to.unwrap_or(u32::MAX),
        });
    }

    // --- the relic ----------------------------------------------------------------

    /// [`Command::ProposeRelic`] at a trader: player `slot` buys its own
    /// trader's relic outright for its own Bim — no vote, since the money
    /// is its own — for [`World::trader_relic_price`] out of its wallet.
    /// The relic leaves the pool and the trader for good. `None` does
    /// nothing: there is no proposal to take back.
    pub(super) fn buy_trader_relic(
        &mut self,
        slot: u32,
        relic: Option<Relic>,
        events: &mut Vec<WorldEvent>,
    ) {
        let Some(at) = self.trader_index(slot) else {
            events.push(refused(slot, Refusal::NotAtATrader));
            return;
        };
        let Some(relic) = relic else {
            return;
        };
        let Some(here) = self.run.traders[at].relic else {
            events.push(refused(slot, Refusal::SoldOut));
            return;
        };
        if relic != here {
            events.push(refused(slot, Refusal::NotOnOffer));
            return;
        }
        if slot >= self.players() {
            events.push(refused(slot, Refusal::NotAPlayer));
            return;
        }
        let price = self.trader_relic_price(relic);
        if !self.pay_from(slot, price) {
            events.push(refused(slot, Refusal::Unaffordable));
            return;
        }
        self.run.traders[at].relic = None;
        self.run.relics.take_from_pool(relic);
        self.run.relics.give(slot, relic);
        events.push(WorldEvent::RelicBought {
            slot,
            relic: relic.code(),
            price,
        });
    }

    // --- combining ------------------------------------------------------------

    /// What a combining's input is, if player `slot` may use it: a thing in
    /// the armory, or on its own Bim's or a bot's slot — never another
    /// player's.
    fn combine_input(&self, slot: u32, from: GearSource) -> Result<Item, Refusal> {
        match from {
            GearSource::Armory { id } => self
                .holdings
                .get(id)
                .map(|s| s.item)
                .ok_or(Refusal::NoSuchGear),
            GearSource::Worn { who, slot: part } => {
                if who >= self.aboard.crew_count() {
                    return Err(Refusal::NotAboard);
                }
                if !self.may_change(slot, who) {
                    return Err(Refusal::NotYours);
                }
                self.worn_on(who, part).ok_or(Refusal::NoSuchGear)
            }
        }
    }

    /// [`Command::Combine`]: two weapons or two pieces of one kind at one
    /// tier into one of the next, at once, for [`data::COMBINE_FEE`] out of
    /// the pool. Where one of the two was worn the result is on that Bim in
    /// its place; otherwise it goes into the armory. Whole, as anything
    /// made is.
    pub(super) fn combine(
        &mut self,
        slot: u32,
        a: GearSource,
        b: GearSource,
        events: &mut Vec<WorldEvent>,
    ) {
        if !self.at_trader() {
            events.push(refused(slot, Refusal::NotAtATrader));
            return;
        }
        if a == b {
            events.push(refused(slot, Refusal::NoSuchGear));
            return;
        }
        let (first, second) = match (self.combine_input(slot, a), self.combine_input(slot, b)) {
            (Ok(x), Ok(y)) => (x, y),
            (Err(why), _) | (_, Err(why)) => {
                events.push(refused(slot, why));
                return;
            }
        };
        let made = match trader::combined(first, second, 0) {
            Ok(made) => made,
            Err(CombineError::NotAPair) => {
                events.push(refused(slot, Refusal::NotAPair));
                return;
            }
            Err(CombineError::TopTier) => {
                events.push(refused(slot, Refusal::TopTier));
                return;
            }
        };
        let fee = self.combine_fee();
        if !self.pay_from(slot, fee) {
            events.push(refused(slot, Refusal::Unaffordable));
            return;
        }
        // A piece made is numbered off the holdings like one bought.
        let made = match made {
            Item::Armour(mut piece) => {
                piece.id = self.holdings.take_id();
                Item::Armour(piece)
            }
            other => other,
        };
        let tier = match made {
            Item::Weapon(w) => w.tier.code(),
            Item::Armour(p) => p.tier.code(),
            Item::Stack(_) => 0,
        };
        // The worn one, if either was, takes the result; the other goes.
        let (keep, drop) = match (a, b) {
            (GearSource::Worn { .. }, _) => (a, b),
            (_, GearSource::Worn { .. }) => (b, a),
            _ => (a, b),
        };
        let take = |world: &mut World, from: GearSource, events: &mut Vec<WorldEvent>| match from {
            GearSource::Armory { id } => {
                world.holdings.take(id);
            }
            GearSource::Worn { who, slot: part } => {
                world.set_slot(who, part, None, events);
            }
        };
        take(self, drop, events);
        let onto = match keep {
            GearSource::Worn { who, slot: part } => {
                self.set_slot(who, part, Some(made), events);
                events.push(WorldEvent::GearChanged {
                    who,
                    part: part.code(),
                });
                who
            }
            GearSource::Armory { .. } => {
                take(self, keep, events);
                self.holdings.put(made);
                u32::MAX
            }
        };
        events.push(WorldEvent::Combined {
            slot,
            who: onto,
            tier,
        });
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
