//! The world's side of the run (feature 103, [`crate::run`]): the two
//! clocks, choosing a destination together, travel resolved in one go,
//! a mission's start and its end, and what dying costs.
//!
//! A child of `crate::world`, so it reaches the world's private fields
//! the way every other `impl World` block does; the run's own types are
//! `crate::run`'s, public, since the app draws them.

use super::*;
use crate::run::{
    Departure, Fallen, Phase as RunPhase, Proposal, Site, SiteLook, SiteSnapshot, SystemLook,
    TravelQuote,
};

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
        let Some(mut system) = galaxy.and_then(|g| g.system(star)) else {
            return Vec::new();
        };
        // What it offers alone (task 135): its mission or its trader.
        self.trim_system(star, &mut system);
        self.sites_of(star, &system)
    }

    /// The sites of another star's `system`, generated and trimmed
    /// ([`World::trim_system`]) already: [`World::sites_in`]'s own half.
    fn sites_of(&self, star: u32, system: &StarSystem) -> Vec<Site> {
        let mut sites: Vec<Site> = system
            .stations
            .iter()
            .map(|s| Site {
                star,
                station: s.id,
            })
            .collect();
        // No station the jammer could be on: none at all, or only the
        // Manufacturers', who have none (feature 109), and the system's
        // trader, which is never one — under the tests' whole-systems dial
        // alone: in a run the jammer is the system's one site.
        let none = self.jammer_site_among(star, &system.stations).is_none();
        if self.whole_systems && none && self.infested(star) {
            sites.push(Site {
                star,
                station: jammer::jammer_id(star),
            });
        }
        // The Machine Heart's fortress at the origin (feature 108), past
        // every other station of it as its id is.
        if star == self.droid_origin {
            sites.push(Site {
                star,
                station: heart::heart_id(star),
            });
        }
        sites.extend(self.offered_surfaces(system).iter().map(|s| Site {
            star,
            station: surface::surface_id(s.body),
        }));
        sites
    }

    /// Every place a trip can go from here: this system's sites, then
    /// those of every star a hyperlane joins to this one, stars in id
    /// order, then those of every star two lanes off (the second map rework), in id
    /// order too. A jammed lane's sites are among them — the quote is what
    /// refuses one, and says why.
    pub fn destinations(&self) -> Vec<Site> {
        self.destinations_in(&self.galaxy())
    }

    fn destinations_in(&self, galaxy: &Galaxy) -> Vec<Site> {
        let mut sites = self.sites_in(None, self.star_id);
        // On the floor (October 2026): the places the crew's place leads
        // to, and no lane's.
        if self.floor().is_some() {
            for site in self.floor_next() {
                if !sites.contains(&site) {
                    sites.push(site);
                }
            }
            return sites;
        }
        let mut stars = galaxy.lanes(self.star_id).to_vec();
        stars.sort_unstable();
        stars.dedup();
        stars.extend(self.two_lanes_off(galaxy));
        for star in stars {
            sites.extend(self.sites_in(Some(galaxy), star));
        }
        sites
    }

    /// The stars two hyperlanes off and no nearer, in id order (task
    /// 139): where a trip through a star between can go — off a galaxy
    /// already generated, which the chart has.
    pub fn two_lanes_off(&self, galaxy: &Galaxy) -> Vec<u32> {
        let near = galaxy.lanes(self.star_id);
        let mut far: Vec<u32> = near
            .iter()
            .flat_map(|&via| galaxy.lanes(via).iter().copied())
            .filter(|&star| star != self.star_id && !near.contains(&star))
            .collect();
        far.sort_unstable();
        far.dedup();
        far
    }

    /// The stars a trip could reach two lanes off (the second map rework), in id
    /// order: what the chart rings beyond the lit lanes.
    pub fn stars_two_lanes_off(&self) -> Vec<u32> {
        self.two_lanes_off(&self.galaxy())
    }

    /// The way a trip to `star` goes (the second map rework): the stars it passes, both
    /// ends in, and whether a jammer turns it back — `None` for a star more
    /// than [`data::MAX_TRIP_HOPS`] lanes off. Two lanes off, the star
    /// between is the first in id order whose two steps no jammer shuts,
    /// else the first; so a way round a jammer is taken where there is
    /// one. What the quote refuses by and the chart draws.
    pub fn trip_route(&self, star: u32) -> Option<(Vec<u32>, bool)> {
        self.trip_route_in(&self.galaxy(), star)
    }

    /// [`World::trip_route`] off a galaxy already generated.
    pub fn trip_route_in(&self, galaxy: &Galaxy, to: u32) -> Option<(Vec<u32>, bool)> {
        // The shape below is two lanes, and no more.
        const _: () = assert!(data::MAX_TRIP_HOPS == 2);
        let from = self.star_id;
        if to == from {
            return Some((vec![from], false));
        }
        let near = galaxy.lanes(from);
        if near.contains(&to) {
            return Some((vec![from, to], self.jammed_step(from, to)));
        }
        let mut ways: Vec<u32> = near
            .iter()
            .copied()
            .filter(|&via| galaxy.lanes(via).contains(&to))
            .collect();
        ways.sort_unstable();
        ways.dedup();
        let shut = |via: u32| self.jammed_step(from, via) || self.jammed_step(via, to);
        let via = ways
            .iter()
            .copied()
            .find(|&via| !shut(via))
            .or_else(|| ways.first().copied())?;
        Some((vec![from, via, to], shut(via)))
    }

    /// How many days a trip to `star` costs, as its quote would say: a
    /// day a hyperlane crossed, nothing within the system — `None` for a
    /// star past [`data::MAX_TRIP_HOPS`] lanes. What the chart and the
    /// system map write beside a star; a jammer shutting the way does not
    /// change the count.
    pub fn trip_days_in(&self, galaxy: &Galaxy, star: u32) -> Option<f64> {
        let (route, _) = self.trip_route_in(galaxy, star)?;
        let minutes = data::JUMP_MINUTES * (route.len() as u64 - 1);
        Some(minutes as f64 / time::DAY)
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
    pub(super) fn site_position(&self, system: &StarSystem, station: u32) -> Option<DVec2> {
        if let Some(body) = surface::surface_body(station) {
            return system.absolute_position(Node::Body(body));
        }
        if jammer::is_derived(station) && system.station(station).is_none() {
            return Some(jammer::blueprint(system, self.galaxy_seed, system.star_id).position);
        }
        if heart::is_heart(station) && system.station(station).is_none() {
            return Some(heart::blueprint(system, self.galaxy_seed, system.star_id).position);
        }
        system.absolute_position(Node::Station(station))
    }

    /// What a trip to `site` would be — how long, when the crew get there
    /// and what they find — or why there is no such trip: a place that
    /// is not there ([`Refusal::NoSuchPlace`]), a star more than two lanes
    /// away ([`Refusal::TooFar`], the second map rework), a jump inward out of a jammed
    /// system — every way there, two lanes off ([`Refusal::Jammed`],
    /// [`World::trip_route`]), a ship that cannot move at all
    /// ([`Refusal::CannotTravel`]), or **the site the crew are at**
    /// ([`Refusal::AlreadyHere`], feature 105): a site left uncleared is
    /// put back as the crew met it, so going back into it without the
    /// clock moving would be the same fight again at the same strength.
    ///
    /// **The length** is a day a lane for a jump and nothing for a trip in
    /// the system (the map rework): [`data::JUMP_MINUTES`] on the world clock
    /// for each hyperlane crossed, however far the site lies from where the jump
    /// lands, and not a minute for moving between the sites of one system.
    /// A ship with no engine to stop it at the far end goes nowhere either
    /// way.
    pub fn travel_quote(&self, site: Site) -> Result<TravelQuote, Refusal> {
        if site.star == self.star_id {
            return self.quote_in(None, site);
        }
        self.quote_in(Some(&self.galaxy()), site)
    }

    /// Every site of `star` as the map draws it (the map rework) — where it
    /// lies in that system and what it is — whether or not a trip could go
    /// there: this system's, a neighbour's, or a star across the galaxy
    /// picked on the chart to be looked at. The system is what it offers
    /// (task 135). `None` for a star the galaxy has not got.
    pub fn system_look(&self, star: u32) -> Option<SystemLook> {
        let generated;
        let (galaxy, system) = if star == self.star_id {
            (None, self.system.clone())
        } else {
            generated = self.galaxy();
            let mut system = generated.system(star)?;
            self.trim_system(star, &mut system);
            (Some(&generated), system)
        };
        let sites = self
            .sites_in(galaxy, star)
            .into_iter()
            .filter_map(|site| {
                let at = self.site_position(&system, site.station)?;
                let quote = self.quote_with(galaxy, site, true).ok()?;
                Some(SiteLook { site, at, quote })
            })
            .collect();
        Some(SystemLook { system, sites })
    }

    /// Every star's one site as the galaxy chart marks it (the galaxy-only
    /// map): [`World::mission_site`] — its mission, its trader or the
    /// Heart — quoted as [`World::system_look`] does, on arrival, with
    /// every question about whether the crew may go there left unasked.
    /// One a star, in star order, off `galaxy` already
    /// generated, and each system generated once: it is every star of the
    /// galaxy, asked again whenever the run moves on.
    pub fn star_missions(&self, galaxy: &Galaxy) -> Vec<run::StarMission> {
        let mut missions = Vec::new();
        for star in 0..galaxy.stars.len() as u32 {
            let generated;
            let system = if star == self.star_id {
                &self.system
            } else {
                let Some(mut system) = galaxy.system(star) else {
                    continue;
                };
                self.trim_system(star, &mut system);
                generated = system;
                &generated
            };
            let Some(station) = self.mission_site(star, system) else {
                continue;
            };
            let site = Site { star, station };
            let given = (star != self.star_id).then_some(system);
            if let Ok(quote) = self.quote_given(Some(galaxy), site, true, given) {
                missions.push(run::StarMission {
                    site,
                    kind: quote.kind,
                    cleared: quote.cleared,
                });
            }
        }
        missions
    }

    /// [`World::travel_quote`] off a galaxy already generated. A trip in
    /// this system reads nothing of it.
    fn quote_in(&self, galaxy: Option<&Galaxy>, site: Site) -> Result<TravelQuote, Refusal> {
        self.quote_with(galaxy, site, false)
    }

    /// The quote, or — `looking` — what the site would be on arrival
    /// with every question about whether the crew may go there left
    /// unasked: here, too far, jammed, the other fight chosen, the ship
    /// unable to move, the trader shut. Only a place that is not there is
    /// refused then.
    fn quote_with(
        &self,
        galaxy: Option<&Galaxy>,
        site: Site,
        looking: bool,
    ) -> Result<TravelQuote, Refusal> {
        self.quote_given(galaxy, site, looking, None)
    }

    /// [`World::quote_with`] off another star's system generated and
    /// trimmed already, `given` — or generated here where it is `None`.
    /// The galaxy chart's marks quote every star off one generation each
    /// ([`World::star_missions`]).
    pub(super) fn quote_given(
        &self,
        galaxy: Option<&Galaxy>,
        site: Site,
        looking: bool,
        given: Option<&StarSystem>,
    ) -> Result<TravelQuote, Refusal> {
        if !looking && self.current_site() == Some(site) {
            return Err(Refusal::AlreadyHere);
        }
        // On the floor (October 2026) a trip goes only up it, to a place
        // the crew's place leads to.
        let on_floor = self.floor().is_some();
        if on_floor && !looking && !self.floor_next().contains(&site) {
            return Err(Refusal::TooFar);
        }
        let jump = site.star != self.star_id;
        let mut hops = 0;
        let elsewhere;
        let system = if jump {
            let Some(galaxy) = galaxy else {
                return Err(Refusal::NoSuchPlace);
            };
            let there = match given {
                Some(there) => there,
                None => {
                    let Some(mut there) = galaxy.system(site.star) else {
                        return Err(Refusal::NoSuchPlace);
                    };
                    // What it offers alone (task 135).
                    self.trim_system(site.star, &mut there);
                    elsewhere = there;
                    &elsewhere
                }
            };
            // Two lanes a trip at most (the second map rework), and none a jammer shuts.
            // The floor asks neither: a trip up it is a hop.
            match self.trip_route_in(galaxy, site.star).filter(|_| !on_floor) {
                Some((route, shut)) => {
                    hops = route.len() as u32 - 1;
                    if !looking && shut {
                        return Err(Refusal::Jammed);
                    }
                }
                None if on_floor => hops = 1,
                None if looking => {
                    hops = galaxy
                        .route(self.star_id, site.star)
                        .map_or(1, |route| route.len() as u32 - 1);
                }
                None => return Err(Refusal::TooFar),
            }
            there
        } else {
            &self.system
        };
        let sites = if jump {
            self.sites_of(site.star, system)
        } else {
            self.sites_in(None, site.star)
        };
        if !sites.contains(&site) {
            return Err(Refusal::NoSuchPlace);
        }
        // One fight a system (task 135): the other one fought already.
        if !looking && self.other_site_chosen(site, system) {
            return Err(Refusal::OtherSiteChosen);
        }
        self.site_position(system, site.station)
            .ok_or(Refusal::NoSuchPlace)?;
        // A ship with no engine to stop it at the far end goes nowhere;
        // how far the far end is no longer matters.
        let dynamics = &self.ship.dynamics;
        let (push, brake) = (
            dynamics.a_forward,
            dynamics.a_forward.max(dynamics.a_backward),
        );
        if !looking {
            physics::travel_days(1.0, push, brake).ok_or(Refusal::CannotTravel)?;
        }
        // A day a hyperlane crossed, nothing within a system (the map
        // rework, the second map rework).
        // On the floor, to the day of the place's row.
        let minutes = match self.floor_minutes(site.star).filter(|_| on_floor) {
            Some(minutes) => minutes,
            None => data::JUMP_MINUTES * hops as u64,
        };
        let days = minutes as f64 / time::DAY;
        let arrival = self.clock_minutes.floor() as u64 + minutes;
        let arrival_day = (arrival / (time::DAY as u64)) as u32;
        // A trader (task 114) is closed while its system is the machines'
        // and not liberated — now, or by the day the crew would get there,
        // which the crisis being a function of the day makes exact. Under
        // the tests' whole-systems dial alone: in a run a fallen trader is
        // its system's jammer, an attack (`World::traders_fall`).
        let trader = if jump {
            system
                .station(site.station)
                .is_some_and(|s| self.is_trader_station(site.star, &system.stations, s))
        } else {
            self.trader_in(None, site)
        };
        if !looking
            && trader
            && !self.traders_fall()
            && self.trader_closed_on(site.star, self.days_gone())
        {
            return Err(Refusal::TraderClosed);
        }
        if !looking
            && trader
            && !self.traders_fall()
            && self.trader_closed_on(site.star, arrival_day)
        {
            return Err(Refusal::ClosedOnArrival);
        }
        // A site of the Manufacturers' (feature 109): theirs whatever the
        // crisis has done round it, so never infested and never a jammer,
        // and the tier said is what their people will carry on arrival.
        let manufacturers = system
            .station(site.station)
            .is_some_and(|s| self.is_manufacturer_site(site.star, s));
        let turns = self.infested_on(site.star);
        let infested = !manufacturers
            && (!trader || self.traders_fall())
            && turns != u32::MAX
            && arrival_day >= turns;
        // The tier most of the enemy come at on the arrival day (task 147),
        // the machines' and the Manufacturers' gear alike.
        // The Machine Heart's fortress is tier three whatever the day.
        let tier = if heart::is_heart(site.station) {
            Tier::Three
        } else if let Some(tier) = self.floor_tier_of(site.star).filter(|_| on_floor) {
            // On the floor, the tier its row is marked.
            tier
        } else {
            self.tier_on(arrival as f64)
        };
        // The jammer on arrival: the system's one site, where the machines
        // have it by then (`World::jammer_site_of`).
        let jammer = infested && self.jammer_site_of(site.star, system) == Some(site.station);
        // What the site is and how its fight stands, off that system's own
        // lists: this one's off the world, another's off its memory (a
        // station id is only its system's), and nothing for a system never
        // visited.
        let (infestations, defenses, held_towns): (&[Infestation], &[Defense], &[u32]) = if jump {
            match self.memories.iter().find(|m| m.star == site.star) {
                Some(m) => (&m.infested, &m.defenses, &m.held_towns),
                None => (&[], &[], &[]),
            }
        } else {
            (&self.infested, &self.defenses, &self.held_towns)
        };
        let held = held_towns.binary_search(&site.station).is_ok();
        let infestation = infestations.iter().find(|it| it.station == site.station);
        // One of the machines' outposts (task 136) in a system never
        // visited: theirs on arrival as its memory would say, had it one.
        // The system's elite (`crate::elite`): the machines' from the
        // first day, an attack.
        let elite =
            !trader && self.elite_station(site.star, &system.stations) == Some(site.station);
        let outpost = jump
            && !self.memories.iter().any(|m| m.star == site.star)
            && self.outposts_of(site.star, system).contains(&site.station);
        let defense = defenses.iter().find(|d| d.station == site.station);
        let won = defense.is_some_and(|d| d.won);
        let cleared = held || won || infestation.is_some_and(|it| it.cleared);
        // **Every site is exactly one kind** (task 111): a trader, an
        // enemy's — the machines' on arrival, the Manufacturers', the
        // fortress, a derived jammer — or else a site to defend.
        // A fallen trader (`World::traders_fall`) is an attack until it is
        // cleared.
        let fought =
            trader && self.traders_fall() && (infested || infestation.is_some()) && !cleared;
        let kind = if trader && !fought {
            SiteKind::Trader
        } else if infested
            || elite
            || manufacturers
            || infestation.is_some()
            || outpost
            || heart::is_heart(site.station)
            || jammer::is_derived(site.station)
        {
            SiteKind::Attack
        } else {
            SiteKind::Defend
        };
        // A defence starts on arrival: a defence site whose fight is not
        // over and that is not a town held — from the first day, wherever
        // the crisis stands.
        let threatened = !self.quiet_sites
            && kind == SiteKind::Defend
            && !held
            && defense.is_none_or(|d| !d.over());
        Ok(TravelQuote {
            site,
            jump,
            hops,
            days,
            minutes,
            arrival_day,
            arrival_date: bims::clock::day_at(self.clock_minutes + minutes as f64),
            infested,
            tier,
            jammer,
            threatened,
            cleared,
            manufacturers,
            elite,
            trader,
            kind,
            // The Machine Heart's strength on arrival (feature 108).
            heart: self.heart_preview(site.station),
        })
    }

    /// A destination put to the crew — see [`Command::Propose`]. Between
    /// missions only, and only somewhere a trip can go; a player whose
    /// Bim is dead still has a say.
    pub(super) fn propose(&mut self, slot: u32, site: Site, events: &mut Vec<WorldEvent>) {
        if self.run.phase == RunPhase::Reward {
            events.push(refused(slot, Refusal::ChoosingRelic));
            return;
        }
        // On the map, or at a trader (task 114), whose vote on where next
        // is the map's.
        if !matches!(self.run.phase, RunPhase::Map | RunPhase::Trade) {
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
        if !matches!(self.run.phase, RunPhase::Map | RunPhase::Trade) {
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

    /// Whether `command`, applied now, would carry the vote and take the
    /// crew on the trip — the question [`World::go_if_carried`] answers,
    /// asked without changing anything. A trip builds the site it lands
    /// at, which is seconds, so the app asks first and applies such a
    /// command off the window's thread behind a loading screen. A
    /// refusal the command would meet on the way is not looked for
    /// beyond the quote: a wrong yes is a loading screen for a moment.
    pub fn would_travel(&self, command: &Command) -> bool {
        if !matches!(self.run.phase, RunPhase::Map | RunPhase::Trade) {
            return false;
        }
        let proposal = match *command {
            Command::Propose {
                slot,
                star,
                station,
            } => {
                let site = Site { star, station };
                if self.travel_quote(site).is_err() {
                    return false;
                }
                Proposal::new(site, slot, self.players())
            }
            Command::Accept { slot, yes: true } => {
                let Some(mut proposal) = self.run.proposal.clone() else {
                    return false;
                };
                if let Some(a) = proposal.accepted.get_mut(slot as usize) {
                    *a = true;
                }
                proposal
            }
            Command::PlayerGone { slot } => {
                let Some(proposal) = self.run.proposal.clone() else {
                    return false;
                };
                let mut connected = self.run.connected.clone();
                if let Some(c) = connected.get_mut(slot as usize) {
                    *c = false;
                }
                return proposal.carried(&connected) && self.travel_quote(proposal.site).is_ok();
            }
            _ => return false,
        };
        proposal.carried(&self.run.connected) && self.travel_quote(proposal.site).is_ok()
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
        if matches!(self.run.phase, RunPhase::Map | RunPhase::Trade) {
            self.go_if_carried(events);
        }
        // And a ready check that was waiting only on them.
        self.start_if_ready(events);
        // And the relic vote, which waits on every connected player.
        self.relic_if_carried(events);
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
        // A trader is visited on the map (task 114): no room, no mission,
        // nothing a mission's start does — unless the machines hold it
        // (`World::traders_fall`), when it is a fight like any other.
        // Anywhere else a mission begins.
        if quote.kind == SiteKind::Trader {
            self.arrive_at_trader(site);
        } else {
            self.arrive_at(site.station);
            self.begin_mission(events);
        }
        events.push(WorldEvent::Travelled {
            star: site.star,
            station: site.station,
            minutes: quote.minutes,
        });
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
    /// site to be photographed on its first step; every offer between
    /// players withdrawn; every piece of armour whole again, worn or in
    /// the armory (task 113); and every crew member whole, its charges
    /// set to their start amounts and every cooldown ready.
    pub(super) fn begin_mission(&mut self, events: &mut Vec<WorldEvent>) {
        let players = self.players();
        self.run.phase = RunPhase::Mission;
        self.run.mission_steps = 0;
        self.run.missions += 1;
        self.run.snapshot = None;
        self.run.snapped = false;
        self.run.site = self.ship.state.alongside();
        if let Some(station) = self.run.site {
            self.choose_site(station);
        }
        self.run.pending_bounty = 0;
        self.run.proposal = None;
        self.run.returning = vec![false; players as usize];
        self.run.recalled = false;
        self.run.departure = None;
        // Every item ready again, nobody hit yet (October 2026).
        self.run.items.new_mission();
        for offer in std::mem::take(&mut self.holdings.offers) {
            events.push(WorldEvent::OfferWithdrawn {
                from: offer.from,
                part: offer.slot.code(),
                to: offer.to,
            });
        }
        self.mend_all_armour();
        self.make_whole();
        // Everybody aboard round the gangway, wherever the last site
        // left them.
        self.stand_the_crew_aboard();
        // No relic choice left standing (feature 106).
        self.relics_at_mission_start();
        // Every player's bots following again: the last mission ended
        // with them sent home.
        for order in &mut self.standing {
            *order = Standing::Follow;
        }
        // A commander's Reinforcements are his to call in (his R, on its
        // cooldown, which `make_whole` just made ready).
        // And held for the ready check, if there is a fight here.
        self.open_briefing();
    }

    // --- the ready check ------------------------------------------------------

    /// Switch the ready check on or off (`Run::ready_check`). Switched on
    /// at the top of a mission — the `game` run does it before the
    /// world's first step — the mission it is in is held too.
    pub fn set_ready_check(&mut self, on: bool) {
        self.run.ready_check = on;
        if !on {
            self.run.briefing = false;
        } else if self.run.phase == RunPhase::Mission && self.run.mission_steps == 0 {
            self.open_briefing();
        }
    }

    /// Whether this mission is held for the ready check.
    pub fn awaiting_ready(&self) -> bool {
        self.run.briefing
    }

    /// Hold the mission just begun for the ready check: with the switch
    /// on and a fight at the site — an Attack not yet cleared, a Defend
    /// threatened — nobody ready yet. A peaceful stop starts at once.
    fn open_briefing(&mut self) {
        let fight = self
            .ship
            .state
            .alongside()
            .is_some_and(|station| !self.site_cleared(station));
        self.run.briefing = self.run.ready_check && fight;
        self.run.ready = vec![false; self.players() as usize];
    }

    /// *Ready* pressed, or taken back — see [`Command::Ready`]. The last
    /// yes of every connected player starts the mission.
    pub(super) fn press_ready(&mut self, slot: u32, yes: bool, events: &mut Vec<WorldEvent>) {
        if !self.run.briefing {
            events.push(refused(slot, Refusal::NoReadyCheck));
            return;
        }
        if let Some(r) = self.run.ready.get_mut(slot as usize) {
            *r = yes;
        }
        events.push(WorldEvent::Readied { slot, yes });
        self.start_if_ready(events);
    }

    /// The mission under way, the moment every connected player is ready.
    fn start_if_ready(&mut self, events: &mut Vec<WorldEvent>) {
        if !self.run.briefing {
            return;
        }
        let all = (0..self.players())
            .filter(|&slot| self.run.is_connected(slot))
            .all(|slot| self.run.is_ready(slot));
        if all {
            self.run.briefing = false;
            events.push(WorldEvent::AllReady);
        }
    }

    /// How many of the connected players are ready, of how many: what the
    /// ready check's window counts.
    pub fn ready_count(&self) -> (u32, u32) {
        let connected: Vec<u32> = (0..self.players())
            .filter(|&slot| self.run.is_connected(slot))
            .collect();
        let ready = connected.iter().filter(|&&s| self.run.is_ready(s)).count();
        (ready as u32, connected.len() as u32)
    }

    /// The dead players back, at the end of the mission they died in
    /// (task 113): each up again where its body lies aboard with its whole
    /// loadout, relics, class, level and talents, and its buyback paid
    /// ([`World::buy_back`]: its own money, else the players' pooled).
    fn respawn_the_fallen(&mut self, events: &mut Vec<WorldEvent>) {
        for fallen in std::mem::take(&mut self.run.fallen) {
            let who = fallen.slot as usize;
            if who >= self.aboard.crew_count() as usize {
                continue;
            }
            let paid = self.buy_back(fallen.slot);
            self.aboard.room.revive(who);
            if let Some(down) = self.crew_down.get_mut(who) {
                *down = false;
            }
            events.push(WorldEvent::Respawned {
                who: fallen.slot,
                paid,
            });
        }
    }

    /// Player `slot`'s buyback paid: out of its own wallet when that holds
    /// the whole [`Rewards::buyback`](crate::rewards::Rewards); otherwise
    /// every player's money is pooled — the fallen's with it — the
    /// buyback taken out of the pool (or what the pool holds, down to
    /// nought, since a respawn never waits for money), and what is left
    /// handed back to the **others** in proportion to what each put in.
    /// The fallen's own is spent. What the division leaves over goes into
    /// the takings. Answers what was paid.
    ///
    /// Three players, C fallen with 2000, A with 1000 and B with 10 000:
    /// 13 000 pooled, 5000 paid, and of the 8000 left A has 1/11 and B
    /// 10/11 back.
    pub(crate) fn buy_back(&mut self, slot: u32) -> Money {
        let cost = self.rewards.buyback;
        let players = self.players().max(1) as usize;
        self.wallets.resize(players, 0);
        let fallen = slot as usize;
        if self.wallet(slot) >= cost {
            self.wallets[fallen] -= cost;
            return cost;
        }
        let pool = self
            .wallets
            .iter()
            .fold(0 as Money, |sum, &w| sum.saturating_add(w));
        let paid = pool.min(cost);
        let left = pool - paid;
        let put_in: Money = self
            .wallets
            .iter()
            .enumerate()
            .filter(|&(i, _)| i != fallen)
            .fold(0, |sum, (_, &w)| sum.saturating_add(w));
        let shares: Vec<Money> = self
            .wallets
            .iter()
            .enumerate()
            .map(|(i, &w)| {
                if i == fallen || put_in == 0 {
                    0
                } else {
                    (u128::from(left) * u128::from(w) / u128::from(put_in)) as Money
                }
            })
            .collect();
        let handed = shares
            .iter()
            .fold(0 as Money, |sum, &s| sum.saturating_add(s));
        self.wallets = shares;
        self.money = self.money.saturating_add(left - handed);
        paid
    }

    /// Player `slot`'s own money.
    pub fn wallet(&self, slot: u32) -> Money {
        self.wallets.get(slot as usize).copied().unwrap_or(0)
    }

    /// What player `slot` has to its name: its own money and its share of
    /// the takings not yet shared out — what it will have when the mission
    /// ends. What the screens show as a player's money.
    pub fn share_of(&self, slot: u32) -> Money {
        let players = Money::from(self.players().max(1));
        self.wallet(slot).saturating_add(self.money / players)
    }

    /// The crew's takings shared out evenly into every player's wallet —
    /// at the world's opening and at the end of every mission — what
    /// does not divide left in the takings for the next time.
    pub(super) fn share_out(&mut self) {
        let players = self.players().max(1) as usize;
        self.wallets.resize(players, 0);
        let each = self.money / players as Money;
        if each == 0 {
            return;
        }
        for wallet in &mut self.wallets {
            *wallet = wallet.saturating_add(each);
        }
        self.money -= each * players as Money;
    }

    /// `price` out of player `slot`'s wallet, if it holds it: false, and
    /// nothing taken, if not.
    pub(crate) fn pay_from(&mut self, slot: u32, price: Money) -> bool {
        if price == 0 {
            return true;
        }
        match self.wallets.get_mut(slot as usize) {
            Some(wallet) if *wallet >= price => {
                *wallet -= price;
                true
            }
            _ => false,
        }
    }

    /// `amount` into player `slot`'s wallet: a sale at a trader.
    pub(crate) fn credit(&mut self, slot: u32, amount: Money) {
        if let Some(wallet) = self.wallets.get_mut(slot as usize) {
            *wallet = wallet.saturating_add(amount);
        }
    }

    /// The crew's money set outright, as a test wants it: every wallet
    /// emptied and `money` shared out into them evenly, the way the
    /// world's opening shares its pool.
    pub fn set_money_for_probe(&mut self, money: Money) {
        for wallet in &mut self.wallets {
            *wallet = 0;
        }
        self.money = money;
        self.share_out();
    }

    /// Every living crew member made whole (`Game::restore_health`), and
    /// every class charge and cooldown fresh: a mission starts at the top
    /// of the mission clock, and a cooldown begun in the last one would
    /// otherwise read as running on for however long the last one lasted.
    fn make_whole(&mut self) {
        let crew = self.aboard.crew_count() as usize;
        // No throw a crew member was walking out to make outlives the
        // mission it was ordered in.
        self.throws.clear();
        for who in 0..crew {
            if !self.aboard.room.is_alive(who) {
                continue;
            }
            self.aboard.room.restore_health(who);
        }
        self.clear_beams();
        self.clear_carries();
        self.charge_timers = vec![[None; Charge::CODES]; crew];
        // The ultimate's sentry ready (task 127), and any left standing
        // from the last mission gone: its lifetime was that mission's.
        for engineer in &mut self.engineers {
            *engineer = crate::engineer::Engineer::default();
        }
        self.deployables
            .retain(|d| d.kind != crate::deploy::DeployKind::Sentry);
        // Every tank's shield whole and down, and his barrier and
        // Bastion ready (task 155).
        for tank in &mut self.tanks {
            *tank = crate::tank::Tank::default();
        }
        // Every Battle Cry and Rally ready (task 129).
        for commander in &mut self.commanders {
            *commander = crate::commander::Commander::default();
        }
        // Every Rampage ready (task 124).
        for soldier in &mut self.soldiers {
            *soldier = crate::soldier::Soldier::default();
        }
        // Every Heal Drone ready and none in the air, and every healing
        // circle off (task 153); the beams were let go above.
        for medic in &mut self.medics {
            medic.last_drone = None;
            medic.drone = None;
            medic.circle = false;
            medic.last_burn = None;
        }
        for who in 0..crew as u32 {
            if self.aboard.room.is_alive(who as usize) {
                self.fill_charges(who);
            }
        }
    }

    /// Crew member `who`'s charges **set to their start amounts** — every
    /// counter its class's ranks give it (tasks 113 and 127): what was left
    /// over from the last mission is neither kept nor added to.
    fn fill_charges(&mut self, who: u32) {
        if who >= self.aboard.crew_count() {
            return;
        }
        for charge in Charge::ALL {
            let start = self.charges(who, charge);
            self.set_charges_held(who, charge, start);
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
        // Whether there is anything here to clear (feature 106): what makes
        // the clear worth a relic.
        self.run.fought = !self.site_cleared(station);
        self.run.cleared_here = false;
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
    /// to come. A held station, when its last wave is destroyed; a site
    /// the machines are coming for, when the crew have held it (a town, a
    /// station or a derelict alike since task 111); and every other site
    /// — a trader, or anywhere under the tests' quiet dial — from the
    /// start, there being nothing there to clear.
    pub fn site_cleared(&self, station: u32) -> bool {
        if let Some(it) = self.infestation(station) {
            return it.cleared;
        }
        if let Some(d) = self.defense(station) {
            return d.won;
        }
        !self.site_threatened(station)
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
    /// A bounty as the site the crew are at pays it: a defence its share
    /// (`Rewards::defense_bounty_percent`), anywhere else the whole — and
    /// the crew's relics' share on it either way.
    pub(crate) fn bounty_here(&self, amount: Money) -> Money {
        let amount = self.bounty_by_relics(amount);
        if self
            .ship
            .state
            .alongside()
            .is_some_and(|id| self.site_kind(id) == SiteKind::Defend)
        {
            self.rewards.at_defense(amount)
        } else {
            amount
        }
    }

    pub(super) fn earn_bounty(&mut self, amount: Money, events: &mut Vec<WorldEvent>) {
        // **A defence pays its share** (`Rewards::defense_bounty_percent`):
        // task 136 made it nothing, the survivors being the reward, and
        // the player then asked for money for every enemy down wherever
        // it falls — a hundred per cent, untuned.
        let amount = self.bounty_here(amount);
        if amount == 0 {
            return;
        }
        if self.mission_cleared() || !self.rewards.bounty_waits_for_clear {
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
    /// being left behind. A player's Bim is **out** for the rest of the
    /// mission and back at its end with everything it wore (task 113),
    /// its class and progress kept; a bot's is gone for good and costs
    /// nothing — only a player's Bim is paid for (its buyback).
    pub(super) fn fall(&mut self, who: u32, events: &mut Vec<WorldEvent>) {
        // A commander's reinforcement (task 129) costs nothing, dead or
        // alive: it is off the deck at once and off the crew at the end.
        if self.is_reinforcement(who) {
            return;
        }
        if who < self.players() {
            if !self.run.is_out(who) {
                let order = self.run.deaths;
                self.run.deaths += 1;
                self.run.fallen.push(Fallen { slot: who, order });
            }
            return;
        }
        events.push(WorldEvent::BotLost { who });
    }

    /// The run is lost when every player's Bim is dead at once — out and
    /// waiting for the mission's end counts, since that is dead too — said once
    /// as [`WorldEvent::CrewLost`] and kept. Bots and joiners do not
    /// keep a run going: a crew is its players. And it is lost at once
    /// when nobody of the whole crew — players and bots — is standing
    /// and no defender of the site is either: every one down or dead,
    /// nobody is left to revive anybody or hold the machines off, so the
    /// countdowns are not waited out.
    pub(super) fn check_run_lost(&mut self, events: &mut Vec<WorldEvent>) {
        // A run won stays won (feature 108): the crew dying after the
        // core is down loses nothing.
        if self.lost || self.run.won {
            return;
        }
        let players = self.players().min(self.aboard.crew_count()) as usize;
        let crew = self.aboard.crew_count() as usize;
        let room = &self.aboard.room;
        let anybody = (0..players).any(|who| room.is_alive(who));
        let standing = (0..crew).any(|who| room.is_alive(who) && !room.is_downed(who))
            || self.defender_standing();
        if !anybody || !standing {
            self.lost = true;
            events.push(WorldEvent::CrewLost);
        }
    }

    /// Every dead bot off the crew for good, highest index first so the
    /// indices below stay right: out of the room with its loadout — a
    /// dead bot's kit is lost with it (October 2026; it went into the
    /// armory, task 113) — and every list the world keeps a crew member
    /// by with it.
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
    pub(super) fn drop_crew_member(&mut self, who: u32) {
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
        // A bot's class, where it has one: a player's is never dropped,
        // and `classes` is otherwise no longer than the players.
        if index < self.classes.len() && index >= self.players() as usize {
            self.classes.remove(index);
        }
        if index < self.charges_held.len() {
            self.charges_held.remove(index);
        }
        if index < self.engineers.len() {
            self.engineers.remove(index);
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
        if index < self.soldiers.len() {
            self.soldiers.remove(index);
        }
        if index < self.commanders.len() {
            self.commanders.remove(index);
        }
        // Whom a cry or a rally reached, and the reinforcements, are crew
        // indices too (task 129).
        for commander in &mut self.commanders {
            commander.forget(who);
        }
        self.reinforcements.retain(|r| r.who != who);
        for r in &mut self.reinforcements {
            if r.who > who {
                r.who -= 1;
            }
        }
        self.field_medics.retain(|&m| m != who);
        for m in &mut self.field_medics {
            if *m > who {
                *m -= 1;
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
        // After a fight won nobody walks (task 133): the deck is frozen
        // and the ship takes everybody from where they stand.
        if !self.fight_over() {
            self.walk_the_player_home(slot);
        }
    }

    /// The player's own Bim sent walking to the deck just inside the
    /// ship's airlock, where the bots' recall gathers — every press of
    /// *Back to ship*, so a player who wandered off and presses again is
    /// walked home again. Not one already inside the ship, and not one
    /// downed; any order the player gives afterwards takes over.
    fn walk_the_player_home(&mut self, slot: u32) {
        let who = slot as usize;
        if slot >= self.aboard.crew_count()
            || self.inside_ship(slot)
            || self.aboard.room.is_down(who)
        {
            return;
        }
        let at = match self.aboard.gangway {
            Some(at) => bims::math::vec2(at.x as f32, at.y as f32),
            None => self.aboard.room.fall_back_point(),
        };
        self.aboard.room.walk_to(who, at);
    }

    /// Every living crew member stood **aboard, round the gangway** — the
    /// deck just inside the ship's airlock — as a mission opens: the
    /// players first by slot, then the bots, each on the nearest free
    /// deck tile of the ship itself (never the passage or the station),
    /// nearest first, then by row and column. Without it a crew starts a
    /// mission wherever the last one left it — a body that was off the
    /// ship when the rooms came apart snapped to whatever corner of the
    /// deck lay nearest — or at bunks scattered over the bridge. A
    /// reinforcement is left where it stands; a ship with no airlock
    /// gathers round its anchor.
    pub(super) fn stand_the_crew_aboard(&mut self) {
        let at = match self.aboard.gangway {
            Some(at) => bims::math::vec2(at.x as f32, at.y as f32),
            None => self.aboard.room.fall_back_point(),
        };
        let crew: Vec<u32> = (0..self.aboard.crew_count())
            .filter(|&who| self.aboard.room.is_alive(who as usize) && !self.is_reinforcement(who))
            .collect();
        if crew.is_empty() {
            return;
        }
        let t = shipdesign::TILE as f64;
        let grid = self.ship.design.grid();
        let on_ship = |p: bims::math::Vec2| {
            let d = self.aboard.to_design(p);
            let tile = ((d.x / t).floor() as i32, (d.y / t).floor() as i32);
            grid.get(shipdesign::parts::Layer::Structure, tile) != 0
        };
        // Wider until there is a tile apiece, or the whole ship has been
        // looked at; anybody past the tiles found stands at the gangway
        // itself and the room's pushing spreads them.
        let mut tiles = Vec::new();
        for reach in [4.0, 8.0, 16.0, 64.0] {
            tiles = self
                .aboard
                .room
                .free_tiles_near(at, reach * shipdesign::TILE as f32)
                .into_iter()
                .filter(|&p| on_ship(p))
                .collect();
            if tiles.len() >= crew.len() {
                break;
            }
        }
        for (i, who) in crew.into_iter().enumerate() {
            let spot = tiles.get(i).copied().unwrap_or(at);
            self.aboard.room.stand_still_at(who as usize, spot);
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
    /// still at the keyboard, alive, not out and not downed — downed too
    /// after a fight won (task 133), when a downed player is going home
    /// with the rest and has the button like them.
    fn waited_for(&self, slot: u32) -> bool {
        let who = slot as usize;
        slot < self.aboard.crew_count()
            && self.run.is_connected(slot)
            && !self.run.is_out(slot)
            && self.aboard.room.is_alive(who)
            && (!self.aboard.room.is_down(who) || self.fight_over())
    }

    /// Whether this mission's fight is **over and won** (task 133): there
    /// was one (`Run::fought`) and the site is cleared. From the step
    /// after, the deck is frozen — the room is not stepped, so nobody
    /// moves and nobody downed bleeds out — and *Back to ship* takes
    /// every crew member alive home from wherever it lies, without a walk
    /// ([`World::comes_home`]).
    /// Tied up at the site, and not `mission_cleared`'s "nowhere counts
    /// as clear": a probe that undocks mid-fight is not a fight won.
    pub fn fight_over(&self) -> bool {
        self.run.phase == RunPhase::Mission
            && self.run.fought
            && self
                .ship
                .state
                .alongside()
                .is_some_and(|id| self.site_cleared(id))
    }

    /// Whether a player it waits for counts as home for the departure:
    /// aboard, or anywhere once the fight is won (task 133).
    fn home_for_departure(&self, slot: u32) -> bool {
        self.fight_over() || self.inside_ship(slot)
    }

    /// Whether crew member `who` is inside the ship: on a tile of the
    /// ship's own frame, or in the passage between the two collars.
    pub fn inside_ship(&self, who: u32) -> bool {
        who < self.aboard.crew_count() && self.aboard.on_ship(who, &self.ship.design)
    }

    /// Everybody the ship would leave behind if it went now: every crew
    /// member still alive — on its feet or down — outside the ship, bar
    /// those it takes home anyway after a fight won
    /// ([`World::comes_home`]).
    pub fn left_behind(&self) -> Vec<u32> {
        (0..self.aboard.crew_count())
            .filter(|&who| {
                self.aboard.room.is_alive(who as usize)
                    && !self.inside_ship(who)
                    && !self.comes_home(who)
                    // A reinforcement goes wherever it stands (task 129).
                    && !self.is_reinforcement(who)
            })
            .collect()
    }

    /// Whether the ship takes crew member `who` home with it wherever it
    /// stands: once the mission's fight is **won** — there was one
    /// (`Run::fought`) and the site is cleared — every crew member alive,
    /// on its feet or **downed**: the deck is frozen from then on and
    /// nobody bleeds out on it (task 133, where task 120 left the downed
    /// behind). The next mission makes them whole.
    pub fn comes_home(&self, who: u32) -> bool {
        who < self.aboard.crew_count()
            && self.fight_over()
            && self.aboard.room.is_alive(who as usize)
    }

    /// The crew outside the ship after a fight won, stood just inside its
    /// airlock as it leaves ([`World::comes_home`]), so the rooms come
    /// apart with them aboard.
    fn bring_home(&mut self) {
        let Some(at) = self.aboard.gangway else {
            return;
        };
        let at = bims::math::vec2(at.x as f32, at.y as f32);
        for who in 0..self.aboard.crew_count() {
            if !self.inside_ship(who) && self.comes_home(who) {
                self.aboard.room.stand_at(who as usize, at);
            }
        }
    }

    /// The players the departure waits for, and how many of them have
    /// pressed *Back to ship* and are aboard: `(aboard, waited)`.
    pub fn returning_count(&self) -> (u32, u32) {
        let waited: Vec<u32> = (0..self.players())
            .filter(|&s| self.waited_for(s))
            .collect();
        let home = waited
            .iter()
            .filter(|&&s| self.run.is_returning(s) && self.home_for_departure(s))
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
                .all(|&s| self.run.is_returning(s) && self.home_for_departure(s));
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
    /// Everybody outside the ship is left behind, and dead for it — bar,
    /// after a fight won, everybody alive, who are stood aboard first
    /// ([`World::comes_home`]). The site is kept as it is if it was
    /// cleared — its bounty paid by then — and otherwise put back as the
    /// mission met it, the bounty thrown
    /// away; experience is kept either way. A town the machines were
    /// attacking falls to them instead, an infested site like any other.
    /// The dead bots are gone, their loadouts with them, the dead
    /// players back with theirs, the pool paying for each, and the ship holds off the site with the rooms
    /// apart, nothing moving, until the crew have chosen where next.
    pub(super) fn leave_mission(&mut self, events: &mut Vec<WorldEvent>) {
        let station = self.ship.state.alongside().or(self.run.site);
        // A mission left is not waiting for anybody's *Ready*.
        self.run.briefing = false;
        // The commanders' reinforcements off the crew first, alive or not
        // (task 129): nothing of theirs is left behind, paid for or kept.
        self.send_reinforcements_home();
        self.bring_home();
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
        // The relics (feature 106), while the ship is still tied up: an
        // elite's site cleared with machines in it offering its reward.
        let reward = self.relics_on_leaving(station, cleared, events);
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
        // The mission's takings shared out evenly between the players —
        // the fallen's share with the rest, since it goes to the pool
        // that buys them back — and then the dead bots go, their
        // loadouts lost with them, and the dead players come back with
        // theirs (task 113).
        self.share_out();
        self.bury_the_bots();
        self.respawn_the_fallen(events);
        // And every piece whole again for the map, the reward screen and
        // a trader, the fight won or not: nothing stays broken past its
        // mission.
        self.mend_all_armour();
        // The slow a downing left is the mission's and ends with it (task
        // 120).
        for who in 0..self.aboard.crew_count() {
            self.aboard.room.forget_downed(who as usize);
        }
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
        // The reward screen first, when there is a relic to choose; the
        // map when the choice is made.
        self.run.phase = if reward {
            RunPhase::Reward
        } else {
            RunPhase::Map
        };
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
        // A trader won back from the machines (`World::traders_fall`) is
        // open the moment the crew are back aboard: the visit begins where
        // the fight ended.
        if cleared
            && !reward
            && let Some(id) = station
            && self.is_trader_here(id)
        {
            self.arrive_at_trader(Site {
                star: self.star_id,
                station: id,
            });
        }
    }

    /// The run's stage at the end of a mission step: the pending bounty
    /// paid if the site has just been cleared, and the departure check.
    pub(super) fn settle_run(&mut self, events: &mut Vec<WorldEvent>) {
        self.settle_bounty(events);
        // The clear itself, said once (feature 106), and the probes' win.
        self.settle_clear(events);
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
