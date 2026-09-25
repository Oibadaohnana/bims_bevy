//! The world's side of the run (feature 103, [`crate::run`]): the two
//! clocks, choosing a destination together, travel resolved in one go,
//! a mission's start and its end, and what dying costs.
//!
//! A child of `crate::world`, so it reaches the world's private fields
//! the way every other `impl World` block does; the run's own types are
//! `crate::run`'s, public, since the app draws them.

use super::*;
use crate::run::{Departure, Fallen, Phase as RunPhase, Proposal, Site, SiteSnapshot, TravelQuote};

impl World {
    // --- the two clocks -----------------------------------------------------

    /// The mission clock in game minutes: [`Run::mission_steps`] at
    /// [`data::STEP_MINUTES`] each. What every in-mission timer counted
    /// in minutes — a class's cooldown, a taunt's length — reads, where it
    /// used to read the world clock.
    pub fn mission_minutes(&self) -> f64 {
        self.run.mission_steps as f64 * data::STEP_MINUTES
    }

    /// Steps since the crew arrived where they are.
    pub fn mission_steps(&self) -> u64 {
        self.run.mission_steps
    }

    /// Whether a mission is running — false between missions, with the
    /// map up.
    pub fn in_mission(&self) -> bool {
        self.run.phase == RunPhase::Mission
    }

    // --- choosing where to go ---------------------------------------------

    /// Every site of a star's system: its stations in id order — the
    /// machines' derived jammer among them where the system is theirs and
    /// has no station of its own — and then its settlements in body
    /// order. The system the crew are in is read off the world; any other
    /// is generated, as the lobby's chart does.
    pub fn sites_at(&self, star: u32) -> Vec<Site> {
        if star == self.star_id {
            return self.sites_in(None, star);
        }
        self.sites_in(Some(&self.galaxy()), star)
    }

    /// [`World::sites_at`] off a galaxy already generated — it is a
    /// thousand stars, and a list of quotes asks about a dozen sites.
    fn sites_in(&self, galaxy: Option<&Galaxy>, star: u32) -> Vec<Site> {
        if star == self.star_id {
            let mut sites: Vec<Site> = self
                .stations
                .iter()
                .map(|s| Site {
                    star,
                    station: s.id,
                })
                .collect();
            sites.extend(self.surfaces.iter().map(|s| Site {
                star,
                station: surface::surface_id(s.body),
            }));
            return sites;
        }
        let Some(system) = galaxy.and_then(|g| g.system(star)) else {
            return Vec::new();
        };
        let mut sites: Vec<Site> = system
            .stations
            .iter()
            .map(|s| Site {
                star,
                station: s.id,
            })
            .collect();
        if system.stations.is_empty() && self.infested(star) {
            sites.push(Site {
                star,
                station: jammer::jammer_id(star),
            });
        }
        sites.extend(
            Surface::all_of(&system, self.galaxy_seed)
                .iter()
                .map(|s| Site {
                    star,
                    station: surface::surface_id(s.body),
                }),
        );
        sites
    }

    /// Every place a trip can go from here: this system's sites, then
    /// those of every star a hyperlane joins to this one, stars in id
    /// order. A jammed lane's sites are among them — the quote is what
    /// refuses one, and says why.
    pub fn destinations(&self) -> Vec<Site> {
        self.destinations_in(&self.galaxy())
    }

    fn destinations_in(&self, galaxy: &Galaxy) -> Vec<Site> {
        let mut sites = self.sites_in(None, self.star_id);
        let mut stars = galaxy.lanes(self.star_id).to_vec();
        stars.sort_unstable();
        stars.dedup();
        for star in stars {
            sites.extend(self.sites_in(Some(galaxy), star));
        }
        sites
    }

    /// Every destination with its quote, or why there is no trip there —
    /// what the world map lists. The galaxy is generated once for all of
    /// them.
    pub fn travel_quotes(&self) -> Vec<(Site, Result<TravelQuote, Refusal>)> {
        let galaxy = self.galaxy();
        self.destinations_in(&galaxy)
            .into_iter()
            .map(|site| (site, self.quote_in(Some(&galaxy), site)))
            .collect()
    }

    /// The site the crew are at — the mission's, or the one they have
    /// just left — if they are at one.
    pub fn current_site(&self) -> Option<Site> {
        self.run
            .site
            .or_else(|| self.ship.state.alongside())
            .map(|station| Site {
                star: self.star_id,
                station,
            })
    }

    /// Where a site of `system` is, in the system's units: a station's
    /// place, a settlement's planet's, the derived jammer's roll.
    fn site_position(&self, system: &StarSystem, station: u32) -> Option<DVec2> {
        if let Some(body) = surface::surface_body(station) {
            return system.absolute_position(Node::Body(body));
        }
        if jammer::is_derived(station) && system.station(station).is_none() {
            return Some(jammer::blueprint(system, self.galaxy_seed, system.star_id).position);
        }
        system.absolute_position(Node::Station(station))
    }

    /// Where a trip from here starts, in this system: the site the crew
    /// are at, else wherever the ship is.
    fn here(&self) -> DVec2 {
        self.current_site()
            .and_then(|site| self.site_position(&self.system, site.station))
            .unwrap_or_else(|| self.ship.position())
    }

    /// What a trip to `site` would be — how long, when the crew get there
    /// and what they find — or why there is no such trip: a place that
    /// is not there ([`Refusal::NoSuchPlace`]), a star more than a lane
    /// away ([`Refusal::TooFar`]), a jump inward out of a jammed system
    /// ([`Refusal::Jammed`]), or a ship that cannot move at all
    /// ([`Refusal::CannotTravel`]).
    ///
    /// **The length** is the hyperdrive's charge, for a jump, and
    /// `physics::travel_days` of the leg in the system at the ship's own
    /// accelerations — from the site the crew are at, or from where the
    /// jump lands them (`crate::jump::landing_point`) to the site. The
    /// forward engines push, and whichever way pushes harder brakes: a
    /// ship with nothing aft turns over and brakes on the same engines.
    /// Rounded **up** to whole minutes for the clock, so the day it puts
    /// the world on to is a whole number of minutes on every machine.
    pub fn travel_quote(&self, site: Site) -> Result<TravelQuote, Refusal> {
        if site.star == self.star_id {
            return self.quote_in(None, site);
        }
        self.quote_in(Some(&self.galaxy()), site)
    }

    /// [`World::travel_quote`] off a galaxy already generated. A trip in
    /// this system reads nothing of it.
    fn quote_in(&self, galaxy: Option<&Galaxy>, site: Site) -> Result<TravelQuote, Refusal> {
        let jump = site.star != self.star_id;
        let elsewhere;
        let system = if jump {
            let Some(galaxy) = galaxy else {
                return Err(Refusal::NoSuchPlace);
            };
            let Some(there) = galaxy.system(site.star) else {
                return Err(Refusal::NoSuchPlace);
            };
            if !galaxy.lanes(self.star_id).contains(&site.star) {
                return Err(Refusal::TooFar);
            }
            if self.jammed_step(self.star_id, site.star) {
                return Err(Refusal::Jammed);
            }
            elsewhere = there;
            &elsewhere
        } else {
            &self.system
        };
        let sites = self.sites_in(galaxy, site.star);
        if !sites.contains(&site) {
            return Err(Refusal::NoSuchPlace);
        }
        let to = self
            .site_position(system, site.station)
            .ok_or(Refusal::NoSuchPlace)?;
        let from = if jump {
            crate::jump::landing_point(system)
        } else {
            self.here()
        };
        let dynamics = &self.ship.dynamics;
        let (push, brake) = (
            dynamics.a_forward,
            dynamics.a_forward.max(dynamics.a_backward),
        );
        let leg =
            physics::travel_days(from.distance(to), push, brake).ok_or(Refusal::CannotTravel)?;
        let charge = if jump {
            time::days(data::JUMP_CHARGE_MINUTES)
        } else {
            0.0
        };
        let days = leg + charge;
        let minutes = (days * time::DAY).ceil().max(0.0) as u64;
        let arrival = self.clock_minutes.floor() as u64 + minutes;
        let arrival_day = (arrival / (time::DAY as u64)) as u32;
        let turns = self.infested_on(site.star);
        let infested = turns != u32::MAX && arrival_day >= turns;
        let tier = match self.droid_tier {
            Some(tier) => tier,
            None if self.hops_from_origin(site.star) <= data::DROID_TIER_THREE_HOPS => Tier::Three,
            None => Tier::One,
        };
        // The jammer on arrival: the lowest orbital station of a system
        // the machines have by then, or theirs where it has none.
        let orbital = system
            .stations
            .iter()
            .map(|s| s.id)
            .filter(|&id| !jammer::is_derived(id))
            .min();
        let jammer = infested
            && surface::surface_body(site.station).is_none()
            && match orbital {
                Some(id) => id == site.station,
                None => jammer::is_derived(site.station),
            };
        // A town the machines come for on arrival: on a planet, in a
        // system one hop outside the infection that day, not theirs and
        // not held.
        let front = {
            let hops = self.hops_from_origin(site.star);
            let radius = arrival_day
                .checked_sub(self.crisis_first_day)
                .map(|d| d / data::DROID_SPREAD_DAYS.max(1));
            radius.and_then(|r| {
                let r = r.min(u16::MAX as u32) as u16;
                (hops != u16::MAX && hops > r).then(|| hops - r)
            })
        };
        let (held, cleared) = if jump {
            let memory = self.memories.iter().find(|m| m.star == site.star);
            let cleared = memory.is_some_and(|m| {
                m.infested
                    .iter()
                    .any(|it| it.station == site.station && it.cleared)
            });
            (false, cleared)
        } else {
            let held = self.town_held(site.station);
            let cleared = held || self.infestation(site.station).is_some_and(|it| it.cleared);
            (held, cleared)
        };
        let droid_held = !jump && self.is_droid_held(site.station);
        let threatened = surface::surface_body(site.station).is_some()
            && !infested
            && !held
            && !droid_held
            && front == Some(1);
        Ok(TravelQuote {
            site,
            jump,
            days,
            minutes,
            arrival_day,
            arrival_date: bims::clock::day_at(self.clock_minutes + minutes as f64),
            infested,
            tier,
            jammer,
            threatened,
            cleared,
        })
    }

    /// A destination put to the crew — see [`Command::Propose`]. Between
    /// missions only, and only somewhere a trip can go; a player whose
    /// Bim is dead still has a say.
    pub(super) fn propose(&mut self, slot: u32, site: Site, events: &mut Vec<WorldEvent>) {
        if self.run.phase != RunPhase::Map {
            events.push(refused(slot, Refusal::MidMission));
            return;
        }
        if let Err(why) = self.travel_quote(site) {
            events.push(refused(slot, why));
            return;
        }
        self.run.proposal = Some(Proposal::new(site, slot, self.players()));
        events.push(WorldEvent::Proposed {
            slot,
            star: site.star,
            station: site.station,
        });
        self.go_if_carried(events);
    }

    /// A yes to the destination on the table, or one taken back — see
    /// [`Command::Accept`].
    pub(super) fn accept_proposal(&mut self, slot: u32, yes: bool, events: &mut Vec<WorldEvent>) {
        if self.run.phase != RunPhase::Map {
            events.push(refused(slot, Refusal::MidMission));
            return;
        }
        let Some(proposal) = self.run.proposal.as_mut() else {
            events.push(refused(slot, Refusal::NoProposal));
            return;
        };
        if let Some(a) = proposal.accepted.get_mut(slot as usize) {
            *a = yes;
        }
        events.push(WorldEvent::ProposalAccepted { slot, yes });
        self.go_if_carried(events);
    }

    /// The trip, the moment every connected player has said yes to it.
    fn go_if_carried(&mut self, events: &mut Vec<WorldEvent>) {
        let Some(proposal) = self.run.proposal.clone() else {
            return;
        };
        if !proposal.carried(&self.run.connected) {
            return;
        }
        self.run.proposal = None;
        match self.travel_quote(proposal.site) {
            Ok(quote) => self.travel(quote, events),
            // The world moved under the proposal — nothing does between
            // missions, but a refusal is said rather than a trip flown.
            Err(why) => events.push(refused(proposal.by, why)),
        }
    }

    /// A player gone from the game — see [`Command::PlayerGone`]. A vote
    /// that was waiting only on them is carried.
    pub(super) fn player_gone(&mut self, slot: u32, events: &mut Vec<WorldEvent>) {
        let Some(connected) = self.run.connected.get_mut(slot as usize) else {
            return;
        };
        if !*connected {
            return;
        }
        *connected = false;
        events.push(WorldEvent::PlayerGone { slot });
        if self.run.phase == RunPhase::Map {
            self.go_if_carried(events);
        }
    }

    // --- travel ---------------------------------------------------------------

    /// The trip, resolved (feature 103): nothing is flown. The world clock
    /// goes on by the trip's length in one go, what that brought due is
    /// paid, a jump is made, the crisis is read at the new day — a
    /// system whose day has come is the machines' before the crew arrive
    /// in it — and the ship is docked or set down at the site, where a
    /// mission begins.
    ///
    /// Everything that runs on days is read off the day rather than
    /// accumulated, so the days skipped spread the crisis exactly as the
    /// same days stepped through would have: a star is the machines' from
    /// its day on, whatever happened in between.
    pub(super) fn travel(&mut self, quote: TravelQuote, events: &mut Vec<WorldEvent>) {
        let site = quote.site;
        self.clock_minutes += quote.minutes as f64;
        // The hired hands' months that fell due on the way.
        self.pay_wages_due(events);
        if quote.jump && !self.jump(site.star, events) {
            return;
        }
        // The whole system charted, as at the start: the crew arrive with
        // the map they chose the site from.
        let mut nodes = self.system.nodes();
        nodes.sort_by_key(node_key);
        for node in nodes {
            if !self.discovered.contains(&node) {
                self.discovered.push(node);
            }
        }
        self.discovered.sort_by_key(node_key);
        // The crisis at the new day, before the ship is tied up anywhere.
        self.spread_crisis(events);
        self.arrive_at(site.station);
        self.begin_mission(events);
        events.push(WorldEvent::Travelled {
            star: site.star,
            station: site.station,
            minutes: quote.minutes,
        });
    }

    /// Every hired hand's month that the clock has reached, paid — as
    /// many months as a long trip went by, one at a time
    /// (`World::pay_wages`).
    fn pay_wages_due(&mut self, events: &mut Vec<WorldEvent>) {
        // A month a pass, and no more passes than there are months in the
        // longest possible gap: every pass either pays somebody's month
        // (moving its due on) or finds nothing more to do.
        for _ in 0..1_000 {
            let due = self
                .hired
                .iter()
                .any(|h| !h.owed && mercenary::owed(h, self.clock_minutes));
            if !due {
                break;
            }
            self.pay_wages(events);
        }
    }

    /// Tied up at `station` — docked at a station, or set down at a
    /// settlement — the rooms joined, the station's people in theirs,
    /// and the view about it.
    fn arrive_at(&mut self, station: u32) {
        let node = match surface::surface_body(station) {
            Some(body) => Node::Body(body),
            None => Node::Station(station),
        };
        self.ship.state = ShipState::Docked { station };
        self.ship.frame = Frame::Local(node);
        self.dock_at(station);
        self.settle_residents();
        self.mark_visited();
    }

    // --- a mission's start ------------------------------------------------------

    /// A mission begins, at the site the ship has just arrived at: the
    /// mission clock at nought, nothing pending, nobody going home, the
    /// site to be photographed on its first step; the dead players the
    /// pool can pay for bought back, longest dead first; and every crew
    /// member whole, its class charges full and every cooldown ready.
    pub(super) fn begin_mission(&mut self, events: &mut Vec<WorldEvent>) {
        let players = self.players();
        self.run.phase = RunPhase::Mission;
        self.run.mission_steps = 0;
        self.run.missions += 1;
        self.run.snapshot = None;
        self.run.snapped = false;
        self.run.site = self.ship.state.alongside();
        self.run.pending_bounty = 0;
        self.run.proposal = None;
        self.run.returning = vec![false; players as usize];
        self.run.recalled = false;
        self.run.departure = None;
        self.buy_back(events);
        self.make_whole();
        // Every player's bots following again: the last mission ended
        // with them sent home.
        for order in &mut self.standing {
            *order = Standing::Follow;
        }
    }

    /// The dead players bought back, oldest death first, while the pool
    /// holds [`data::BUYBACK_COST`] — each up again where its body lies
    /// aboard, carrying nothing. The rest stay out and are said so.
    fn buy_back(&mut self, events: &mut Vec<WorldEvent>) {
        let mut still = Vec::new();
        for fallen in std::mem::take(&mut self.run.fallen) {
            let who = fallen.slot as usize;
            if who >= self.aboard.crew_count() as usize {
                continue;
            }
            if self.money < data::BUYBACK_COST {
                still.push(fallen);
                events.push(WorldEvent::StillOut { who: fallen.slot });
                continue;
            }
            self.money -= data::BUYBACK_COST;
            self.strip_the_dead(fallen.slot);
            self.aboard.room.revive(who);
            if let Some(down) = self.crew_down.get_mut(who) {
                *down = false;
            }
            events.push(WorldEvent::BoughtBack { who: fallen.slot });
        }
        self.run.fallen = still;
    }

    /// Every living crew member made whole (`Game::restore_health`), and
    /// every class charge and cooldown fresh: a mission starts at the top
    /// of the mission clock, and a cooldown begun in the last one would
    /// otherwise read as running on for however long the last one lasted.
    fn make_whole(&mut self) {
        let crew = self.aboard.crew_count() as usize;
        for who in 0..crew {
            if !self.aboard.room.is_alive(who) {
                continue;
            }
            self.aboard.room.restore_health(who);
        }
        self.clear_beams();
        self.clear_carries();
        self.charge_timers = vec![[None; Charge::ALL.len()]; crew];
        for tank in &mut self.tanks {
            tank.last_taunt = None;
        }
        for commander in &mut self.commanders {
            commander.last_rally = None;
        }
        for who in 0..crew as u32 {
            if self.aboard.room.is_alive(who as usize) {
                self.fill_charges(who);
            }
        }
    }

    /// Crew member `who`'s pack topped up to every charge it has — its
    /// class's kits and grenades and everybody's medicine — at once.
    fn fill_charges(&mut self, who: u32) {
        if who >= self.aboard.crew_count() {
            return;
        }
        for charge in Charge::ALL {
            if self.medicine_off && charge.everybody() {
                continue;
            }
            let short = self
                .charges(who, charge)
                .saturating_sub(self.charges_of(who, charge));
            let item = Item::Stack(charge.resource() as u32);
            if charge.everybody() {
                self.aboard.room.give_stack(who as usize, item, short);
            } else {
                for _ in 0..short {
                    self.aboard.room.give(who as usize, None, item);
                }
            }
        }
    }

    // --- a mission's steps ---------------------------------------------------

    /// The top of a mission step: the site photographed on the first one,
    /// before anything has moved — the state the mission met it in.
    pub(super) fn open_the_mission(&mut self) {
        if self.run.snapped {
            return;
        }
        self.run.snapped = true;
        let Some(station) = self.ship.state.alongside() else {
            return;
        };
        self.run.site = Some(station);
        self.run.snapshot = Some(self.snapshot_of(station));
    }

    /// Everything the world keeps about one site of this system.
    fn snapshot_of(&self, station: u32) -> SiteSnapshot {
        let losses = memory::losses_at(&self.losses, station);
        SiteSnapshot {
            station,
            infestation: self.infestation(station).cloned(),
            defense: self.defense(station).cloned(),
            losses: (!losses.is_empty()).then_some(losses),
            graves: memory::graves_at(&self.graves, station).to_vec(),
            lamps: self
                .lamps
                .iter()
                .filter(|l| l.station == Some(station))
                .copied()
                .collect(),
        }
    }

    /// A site put back as a snapshot has it: everything the world keeps
    /// about it replaced by what was photographed.
    fn restore_site(&mut self, snapshot: SiteSnapshot) {
        let id = snapshot.station;
        self.infested.retain(|it| it.station != id);
        if let Some(it) = snapshot.infestation {
            self.infested.push(it);
            self.infested.sort_by_key(|it| it.station);
        }
        self.defenses.retain(|d| d.station != id);
        if let Some(d) = snapshot.defense {
            let at = self.defenses.partition_point(|d| d.station < id);
            self.defenses.insert(at, d);
        }
        self.losses.retain(|l| l.station != id);
        if let Some(l) = snapshot.losses {
            let at = self.losses.partition_point(|l| l.station < id);
            self.losses.insert(at, l);
        }
        memory::set_graves(&mut self.graves, id, snapshot.graves);
        // The lamps in the order they were: the other stations' where
        // they stand, this one's as photographed, after them.
        self.lamps.retain(|l| l.station != Some(id));
        self.lamps.extend(snapshot.lamps);
    }

    /// Whether a site is **cleared**: no machine left there and none still
    /// to come. A held station, when its last wave is destroyed; a town
    /// the machines are coming for, when the crew have held it; and every
    /// other site from the start, there being nothing there to clear.
    pub fn site_cleared(&self, station: u32) -> bool {
        if let Some(it) = self.infestation(station) {
            return it.cleared;
        }
        if let Some(d) = self.defense(station) {
            return d.won;
        }
        !self.town_threatened(station)
    }

    /// Whether the site of this mission is cleared: where the ship is
    /// tied up, else — out in space — nowhere to clear.
    pub fn mission_cleared(&self) -> bool {
        self.ship
            .state
            .alongside()
            .is_none_or(|id| self.site_cleared(id))
    }

    /// The Republic's bounty for enemies taken down, earned: paid at once
    /// at a site that is cleared — nothing is waiting on it — and
    /// otherwise pending until it is.
    pub(super) fn earn_bounty(&mut self, amount: Money, events: &mut Vec<WorldEvent>) {
        if amount == 0 {
            return;
        }
        if self.mission_cleared() {
            self.money = self.money.saturating_add(amount);
            events.push(WorldEvent::Bounty { amount });
        } else {
            self.run.pending_bounty = self.run.pending_bounty.saturating_add(amount);
            events.push(WorldEvent::BountyPending { amount });
        }
    }

    /// The pending bounty paid, the step the site is cleared — once: it
    /// is nought after.
    fn settle_bounty(&mut self, events: &mut Vec<WorldEvent>) {
        if self.run.pending_bounty == 0 || !self.mission_cleared() {
            return;
        }
        let amount = std::mem::take(&mut self.run.pending_bounty);
        self.money = self.money.saturating_add(amount);
        events.push(WorldEvent::Bounty { amount });
    }

    // --- dying ---------------------------------------------------------------

    /// A crew member has died — said once, from `casualties` or from
    /// being left behind. A player's Bim is **out** until bought back, its
    /// class and progress kept; a bot's is gone for good, and the pool
    /// pays [`data::BOT_DEATH_PENALTY`] for it, as much as it holds.
    pub(super) fn fall(&mut self, who: u32, events: &mut Vec<WorldEvent>) {
        if who < self.players() {
            if !self.run.is_out(who) {
                let order = self.run.deaths;
                self.run.deaths += 1;
                self.run.fallen.push(Fallen { slot: who, order });
            }
            return;
        }
        let paid = self.money.min(data::BOT_DEATH_PENALTY);
        self.money -= paid;
        events.push(WorldEvent::BotLost { who, paid });
    }

    /// The run is lost when every player's Bim is dead at once — out and
    /// waiting to be bought back counts, since that is dead too — said once
    /// as [`WorldEvent::CrewLost`] and kept. Bots and hired hands do not
    /// keep a run going: a crew is its players.
    pub(super) fn check_run_lost(&mut self, events: &mut Vec<WorldEvent>) {
        if self.lost {
            return;
        }
        let players = self.players().min(self.aboard.crew_count()) as usize;
        let room = &self.aboard.room;
        let anybody = (0..players).any(|who| room.is_alive(who));
        if !anybody {
            self.lost = true;
            events.push(WorldEvent::CrewLost);
        }
    }

    /// What a dead player's Bim carried, gone with the body: its gun, its
    /// armour and its pack, and the world's record of each piece.
    fn strip_the_dead(&mut self, slot: u32) {
        let who = slot as usize;
        if who >= self.aboard.crew_count() as usize {
            return;
        }
        self.aboard.room.issue(who, bims::combat::Gear::default());
        self.pieces.retain(
            |p| !matches!(p.at, Where::Worn { who: w } | Where::Pack { who: w, .. } if w == slot),
        );
    }

    /// Every dead bot off the crew for good, highest index first so the
    /// indices below stay right: out of the room, and every list the
    /// world keeps a crew member by with it.
    fn bury_the_bots(&mut self) {
        let players = self.players();
        let crew = self.aboard.crew_count();
        for who in (players..crew).rev() {
            if !self.aboard.room.is_alive(who as usize) {
                self.drop_crew_member(who);
            }
        }
    }

    /// One crew member out of the crew — a dead bot's body gone with the
    /// site it lay at — and every crew member after it moved down one.
    fn drop_crew_member(&mut self, who: u32) {
        let index = who as usize;
        if index >= self.aboard.room.crew_count() as usize {
            return;
        }
        let mut everybody = self.aboard.room.take_crew();
        everybody.remove(index);
        self.aboard.room.adopt(everybody, bims::math::Vec2::ZERO);
        self.aboard.crew = self.aboard.room.crew_count();
        self.ship.crew_count = self.aboard.crew;
        if index < self.crew_down.len() {
            self.crew_down.remove(index);
        }
        if index < self.crew_locked.len() {
            self.crew_locked.remove(index);
        }
        if index < self.progress.len() {
            self.progress.remove(index);
        }
        if index < self.reused_kits.len() {
            self.reused_kits.remove(index);
        }
        if index < self.charge_timers.len() {
            self.charge_timers.remove(index);
        }
        self.clear_beams();
        self.clear_carries();
        if index < self.medics.len() {
            self.medics.remove(index);
        }
        if index < self.tanks.len() {
            self.tanks.remove(index);
        }
        if index < self.commanders.len() {
            self.commanders.remove(index);
        }
        self.clear_squad();
        self.pieces.retain(
            |p| !matches!(p.at, Where::Worn { who: w } | Where::Pack { who: w, .. } if w == who),
        );
        for piece in &mut self.pieces {
            match &mut piece.at {
                Where::Worn { who: w } | Where::Pack { who: w, .. } if *w > who => *w -= 1,
                _ => {}
            }
        }
        self.hired.retain(|h| h.who != who);
        for h in &mut self.hired {
            if h.who > who {
                h.who -= 1;
            }
        }
        self.on_ship_changed();
    }

    // --- ending a mission ------------------------------------------------------

    /// *Back to ship* — see [`Command::Return`]. The first press of the
    /// mission sends every bot home; a press by a player already
    /// returning asks a turned-down departure again.
    pub(super) fn press_return(&mut self, slot: u32, events: &mut Vec<WorldEvent>) {
        if slot >= self.players() {
            return;
        }
        if self.run.is_out(slot) || !self.aboard.room.is_alive(slot as usize) {
            events.push(refused(slot, Refusal::PlayerOut));
            return;
        }
        let players = self.players() as usize;
        if self.run.returning.len() < players {
            self.run.returning.resize(players, false);
        }
        let again = self.run.returning[slot as usize];
        self.run.returning[slot as usize] = true;
        self.run.recalled = true;
        if again && matches!(self.run.departure, Some(Departure::Declined { .. })) {
            self.run.departure = None;
        }
        if !again {
            events.push(WorldEvent::Returning { slot });
        }
    }

    /// A player's answer to the departure check — see
    /// [`Command::LeaveBehind`]. One no turns it down; the last yes
    /// is the ship leaving, if the list it asked about is still the list.
    pub(super) fn answer_departure(&mut self, slot: u32, yes: bool, events: &mut Vec<WorldEvent>) {
        let Some(Departure::Asking { answers, .. }) = self.run.departure.as_mut() else {
            events.push(refused(slot, Refusal::NotAsked));
            return;
        };
        if let Some(answer) = answers.get_mut(slot as usize) {
            *answer = Some(yes);
        }
        if !yes {
            let behind = self
                .run
                .departure
                .as_ref()
                .map(|d| d.behind().to_vec())
                .unwrap_or_default();
            self.run.departure = Some(Departure::Declined { behind });
            events.push(WorldEvent::DepartureDeclined { slot });
        }
    }

    /// Whether a player's Bim is one the departure waits for: a player
    /// still at the keyboard, alive, not out and not downed.
    fn waited_for(&self, slot: u32) -> bool {
        let who = slot as usize;
        slot < self.aboard.crew_count()
            && self.run.is_connected(slot)
            && !self.run.is_out(slot)
            && self.aboard.room.is_alive(who)
            && !self.aboard.room.is_down(who)
    }

    /// Whether crew member `who` is inside the ship: on a tile of the
    /// ship's own frame, or in the passage between the two collars.
    pub fn inside_ship(&self, who: u32) -> bool {
        who < self.aboard.crew_count() && self.aboard.on_ship(who, &self.ship.design)
    }

    /// Everybody the ship would leave behind if it went now: every crew
    /// member still alive — on its feet or down — outside the ship.
    pub fn left_behind(&self) -> Vec<u32> {
        (0..self.aboard.crew_count())
            .filter(|&who| self.aboard.room.is_alive(who as usize) && !self.inside_ship(who))
            .collect()
    }

    /// The players the departure waits for, and how many of them have
    /// pressed *Back to ship* and are aboard: `(aboard, waited)`.
    pub fn returning_count(&self) -> (u32, u32) {
        let waited: Vec<u32> = (0..self.players())
            .filter(|&s| self.waited_for(s))
            .collect();
        let home = waited
            .iter()
            .filter(|&&s| self.run.is_returning(s) && self.inside_ship(s))
            .count() as u32;
        (home, waited.len() as u32)
    }

    /// The departure check, a stage at the end of a mission step: once
    /// every player it waits for has pressed *Back to ship* and is aboard
    /// — and at least one has — the ship goes if nobody would be left
    /// behind, and asks everybody if anybody would. A question already
    /// asked goes when every connected player has said yes; one turned
    /// down is not asked again about the same people.
    pub(super) fn settle_departure(&mut self, events: &mut Vec<WorldEvent>) {
        let waited: Vec<u32> = (0..self.players())
            .filter(|&s| self.waited_for(s))
            .collect();
        let ready = !waited.is_empty()
            && waited
                .iter()
                .all(|&s| self.run.is_returning(s) && self.inside_ship(s));
        if !ready {
            // A question still asking when somebody has walked back out
            // is no longer the question: dropped, to be asked afresh.
            if matches!(self.run.departure, Some(Departure::Asking { .. })) {
                self.run.departure = None;
            }
            return;
        }
        let behind = self.left_behind();
        match self.run.departure.clone() {
            Some(Departure::Declined { behind: declined }) if declined == behind => {}
            Some(Departure::Asking {
                behind: asked,
                answers,
            }) if asked == behind => {
                let all_yes = (0..self.players()).all(|slot| {
                    !self.run.is_connected(slot)
                        || answers.get(slot as usize).copied().flatten() == Some(true)
                });
                if all_yes {
                    self.leave_mission(events);
                }
            }
            _ if behind.is_empty() => self.leave_mission(events),
            _ => {
                events.push(WorldEvent::DepartureAsked {
                    behind: behind.len() as u32,
                });
                self.run.departure = Some(Departure::ask(behind, self.players()));
            }
        }
    }

    /// The ship leaves the site and the map comes up (feature 103).
    ///
    /// Everybody outside the ship is left behind, and dead for it. The
    /// site is kept as it is if it was cleared — its bounty paid by then —
    /// and otherwise put back as the mission met it, the bounty thrown
    /// away; experience is kept either way. A town the machines were
    /// attacking falls to them instead, an infested site like any other.
    /// The dead players lose what they carried with the body, the dead
    /// bots are gone, and the ship holds off the site with the rooms
    /// apart, nothing moving, until the crew have chosen where next.
    pub(super) fn leave_mission(&mut self, events: &mut Vec<WorldEvent>) {
        let station = self.ship.state.alongside().or(self.run.site);
        for who in self.left_behind() {
            self.aboard.room.kill_now(who as usize);
            events.push(WorldEvent::LeftBehind { who });
        }
        // The deaths of this step said and paid for, the left behind with
        // them, before anybody is moved.
        self.casualties(events);
        let cleared = self.mission_cleared();
        if cleared {
            self.settle_bounty(events);
        }
        self.run.pending_bounty = 0;
        let falls = station
            .filter(|_| !cleared)
            .filter(|&id| self.defense(id).is_some_and(|d| !d.over()));
        // The rooms apart: the crew into the ship's own room, and the
        // station's closed with its dead counted and filed.
        self.unjoin_rooms();
        self.close_residents();
        if let Some(id) = falls {
            self.infest(id);
            events.push(WorldEvent::TownFell { station: id });
        } else if !cleared && let Some(snapshot) = self.run.snapshot.take() {
            self.restore_site(snapshot);
        }
        // What the dead carried goes with them, and the dead bots go.
        let out: Vec<u32> = self.run.fallen.iter().map(|f| f.slot).collect();
        for slot in out {
            self.strip_the_dead(slot);
        }
        self.bury_the_bots();
        self.mirror_pieces(events);
        // Off the berth, holding where the site is, the map up.
        let at = station.and_then(|id| {
            let system = &self.system;
            self.site_position(system, id)
        });
        self.ship.state = ShipState::Holding;
        // In the site's own frame, so the map says where the crew are.
        self.ship.frame = match station {
            Some(id) => match surface::surface_body(id) {
                Some(body) => Frame::Local(Node::Body(body)),
                None => Frame::Local(Node::Station(id)),
            },
            None => Frame::Space,
        };
        if let Some(at) = at {
            self.ship.set_position(at);
        }
        self.undocked_once = true;
        self.run.phase = RunPhase::Map;
        self.run.site = station;
        self.run.snapshot = None;
        self.run.proposal = None;
        self.run.departure = None;
        self.run.recalled = false;
        for r in &mut self.run.returning {
            *r = false;
        }
        events.push(WorldEvent::LeftSite {
            station: station.unwrap_or(u32::MAX),
            cleared,
        });
    }

    /// The run's stage at the end of a mission step: the pending bounty
    /// paid if the site has just been cleared, and the departure check.
    pub(super) fn settle_run(&mut self, events: &mut Vec<WorldEvent>) {
        self.settle_bounty(events);
        self.settle_departure(events);
    }

    /// A probe's way to start the mission at wherever it has put the
    /// ship, after it docked, landed or infested by hand: the site
    /// photographed afresh on the next step, the mission clock at nought
    /// — and nothing else, so what the probe did to the crew stands.
    pub fn restart_mission_for_probe(&mut self) {
        self.run.phase = RunPhase::Mission;
        self.run.mission_steps = 0;
        self.run.snapshot = None;
        self.run.snapped = false;
        self.run.site = self.ship.state.alongside();
        self.run.pending_bounty = 0;
    }

    /// A probe's way to the map: the mission left as the button leaves
    /// it, whoever is where. For a test that wants to travel without
    /// walking everybody home first.
    pub fn leave_for_probe(&mut self) -> Vec<WorldEvent> {
        let mut events = Vec::new();
        self.leave_mission(&mut events);
        events
    }
}
