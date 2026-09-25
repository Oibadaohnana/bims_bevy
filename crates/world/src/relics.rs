//! The world's side of the relics (feature 106, [`crate::relic`]): what a
//! relic does to the Bim holding it, choosing one together, the caches on
//! the machines' sites, the reward for a site cleared, and the win.
//!
//! A child of `crate::world`, as `mission.rs` is, so it reaches the
//! world's private fields; the relics' own types are `crate::relic`'s.

use super::*;
use crate::relic::{
    self, Action, Hook, Relic, RelicChoice, RelicProposal, Source, Stat, Trigger,
};
use crate::run::Phase as RunPhase;

impl World {
    // --- reading ---------------------------------------------------------------

    /// The relics a player's Bim holds, in the order it was given them —
    /// nothing for a bot or a slot past the players.
    pub fn relics_of(&self, who: u32) -> &[Relic] {
        if who >= self.players() {
            return &[];
        }
        self.run.relics.of(who)
    }

    /// Whether crew member `who` holds `relic`.
    pub fn has_relic(&self, who: u32, relic: Relic) -> bool {
        self.relics_of(who).contains(&relic)
    }

    /// The relic choice being made, if one is: the reward screen's, or a
    /// cache's in the mission.
    pub fn relic_choice(&self) -> Option<&RelicChoice> {
        self.run.relics.choice.as_ref()
    }

    /// Whether the crew are on the reward screen: a site left cleared, a
    /// relic being chosen, the map to come.
    pub fn choosing_reward(&self) -> bool {
        self.run.phase == RunPhase::Reward
    }

    /// What is still to be offered this run.
    pub fn relic_pool(&self) -> &[Relic] {
        &self.run.relics.pool
    }

    /// Relics out of a cache waiting on the site being cleared, with the
    /// player slot each is for.
    pub fn pending_relics(&self) -> &[(u32, Relic)] {
        &self.run.relics.pending
    }

    /// Whether the run is won.
    pub fn is_won(&self) -> bool {
        self.run.won
    }

    /// Whether another player's Bim than `who` is down — out cold, alive.
    fn other_player_down(&self, who: u32) -> bool {
        let crew = self.aboard.crew_count();
        (0..self.players().min(crew)).any(|s| {
            s != who && self.aboard.room.is_alive(s as usize) && self.aboard.room.is_down(s as usize)
        })
    }

    /// The percentage crew member `who`'s relics put on `stat` now —
    /// nought for a bot and for a Bim holding nothing for it.
    pub fn relic_percent(&self, who: u32, stat: Stat) -> i32 {
        let held = self.relics_of(who);
        if held.is_empty() {
            return 0;
        }
        relic::stat_percent(held, stat, self.other_player_down(who))
    }

    /// The same as a factor: 1.1 for ten per cent.
    pub fn relic_factor(&self, who: u32, stat: Stat) -> f64 {
        relic::factor(self.relic_percent(who, stat))
    }

    /// What a crew member's relics do to its skill (`World::skill_of`):
    /// the damage, the odds, the pace, the armour, what a medkit puts
    /// back, and the overcharge. The skill as it was for anybody holding
    /// nothing.
    pub(super) fn lift_by_relics(&self, who: u32, skill: &mut bims::combat::Skill) {
        let held = self.relics_of(who);
        if held.is_empty() {
            return;
        }
        let f = |stat| self.relic_factor(who, stat) as f32;
        skill.damage *= f(Stat::Damage);
        skill.melee *= f(Stat::Damage);
        skill.accuracy *= f(Stat::Accuracy);
        skill.walk *= f(Stat::MoveSpeed);
        skill.armour_protection *= f(Stat::Armour);
        skill.healing *= f(Stat::HealingReceived);
        let (every, damage) = relic::overcharge(held);
        skill.overcharge = every;
        skill.overcharge_damage = damage;
    }

    // --- setting a run up --------------------------------------------------------

    /// The run's pool, fixed at its start: the host's profile's unlocked
    /// relics ([`relic::Profile::pool`]), sent to every client with the
    /// rest of the lobby's numbers. A new world opens with a new
    /// profile's.
    pub fn set_relic_pool(&mut self, pool: Vec<Relic>) {
        let mut pool = pool;
        pool.sort_unstable();
        pool.dedup();
        pool.retain(|&r| !self.run.relics.in_play(r));
        self.run.relics.pool = pool;
    }

    /// A probe's way to give a player's Bim a relic at once
    /// (`BIMS_RELICS`), out of the pool if it is there. Nothing for a bot.
    pub fn give_relic_for_probe(&mut self, slot: u32, relic: Relic) {
        if slot >= self.players() {
            return;
        }
        self.run.relics.pool.retain(|&r| r != relic);
        self.run.relics.give(slot, relic);
    }

    /// The probes' dial (`BIMS_WIN=1`): the run is won the next time a
    /// site is cleared with machines in it.
    pub fn set_win_on_clear(&mut self, on: bool) {
        self.run.win_on_clear = on;
    }

    // --- the win ------------------------------------------------------------------

    /// **The one place a run is won.** Said once, as
    /// [`WorldEvent::RunWon`], and kept; the app ends the run on it, the
    /// way it does on [`WorldEvent::CrewLost`], and every player's
    /// profile unlocks relics off it. The end boss calls it; until there
    /// is one, the probes' `BIMS_WIN` does, on the next clear.
    pub fn run_won(&mut self, events: &mut Vec<WorldEvent>) {
        if self.run.won || self.lost {
            return;
        }
        self.run.won = true;
        events.push(WorldEvent::RunWon);
    }

    // --- offers ------------------------------------------------------------------

    /// `n` relics drawn for a site of `tier` and taken out of the pool:
    /// offered once, never again this run. Seeded off the galaxy, the
    /// site and how many offers came before, so every client draws the
    /// same.
    fn draw_relics(&mut self, tier: u8, n: usize, station: u32) -> Vec<Relic> {
        let seed = worldgen::rng::mix(
            self.galaxy_seed
                ^ worldgen::rng::mix(u64::from(self.star_id) << 32 | u64::from(station))
                ^ worldgen::rng::mix(0x_4F46_4645_5200 + u64::from(self.run.relics.offers)),
        );
        self.run.relics.offers = self.run.relics.offers.wrapping_add(1);
        let drawn = relic::offer(&self.run.relics.pool, tier, n, seed);
        self.run.relics.pool.retain(|r| !drawn.contains(r));
        drawn
    }

    /// The tier the machines at a site come at, as a number: what its
    /// relics are drawn at.
    fn site_tier_code(&self) -> u8 {
        self.droid_tier().code() as u8
    }

    /// A choice put to the crew: `options` off `source`. False, and no
    /// choice, with nothing to offer.
    fn put_choice(
        &mut self,
        source: Source,
        tier: u8,
        options: Vec<Relic>,
        events: &mut Vec<WorldEvent>,
    ) -> bool {
        if options.is_empty() {
            return false;
        }
        events.push(WorldEvent::RelicsOffered {
            source: source.code(),
            count: options.len() as u32,
        });
        self.run.relics.choice = Some(RelicChoice {
            source,
            tier,
            options,
            proposal: None,
        });
        true
    }

    /// The reward for the site just left, cleared with machines in it
    /// (called by `leave_mission` before the rooms part, the tier read
    /// while the ship is still tied up there): [`data::RELIC_OFFER`]
    /// relics of the site's tier, and the reward screen up. False, and
    /// straight to the map, with the pool empty at and below that tier.
    pub(super) fn offer_reward(&mut self, station: u32, events: &mut Vec<WorldEvent>) -> bool {
        let tier = self.site_tier_code();
        let options = self.draw_relics(tier, data::RELIC_OFFER, station);
        self.put_choice(Source::Reward, tier, options, events)
    }

    // --- choosing together ---------------------------------------------------------

    /// A relic — or none — put to the crew for a player's Bim, see
    /// [`Command::ProposeRelic`]: a choice being made, the relic among its
    /// options, and a Bim a player steers. Every acceptance goes with the
    /// last proposal; the proposer's own yes is counted.
    pub(super) fn propose_relic(
        &mut self,
        slot: u32,
        relic: Option<Relic>,
        to: u32,
        events: &mut Vec<WorldEvent>,
    ) {
        let players = self.players();
        let Some(choice) = self.run.relics.choice.as_mut() else {
            events.push(refused(slot, Refusal::NoRelicChoice));
            return;
        };
        if relic.is_some_and(|r| !choice.options.contains(&r)) {
            events.push(refused(slot, Refusal::NotOnOffer));
            return;
        }
        if to >= players {
            events.push(refused(slot, Refusal::NotAPlayer));
            return;
        }
        let mut accepted = vec![false; players as usize];
        if let Some(a) = accepted.get_mut(slot as usize) {
            *a = true;
        }
        choice.proposal = Some(RelicProposal {
            relic,
            to,
            by: slot,
            accepted,
        });
        events.push(WorldEvent::RelicProposed {
            slot,
            relic: relic.map_or(u32::MAX, Relic::code),
            to,
        });
        self.relic_if_carried(events);
    }

    /// A yes to the relic on the table, or one taken back — see
    /// [`Command::AcceptRelic`].
    pub(super) fn accept_relic(&mut self, slot: u32, yes: bool, events: &mut Vec<WorldEvent>) {
        let Some(proposal) = self
            .run
            .relics
            .choice
            .as_mut()
            .and_then(|c| c.proposal.as_mut())
        else {
            events.push(refused(slot, Refusal::NoRelicChoice));
            return;
        };
        if let Some(a) = proposal.accepted.get_mut(slot as usize) {
            *a = yes;
        }
        events.push(WorldEvent::RelicAccepted { slot, yes });
        self.relic_if_carried(events);
    }

    /// The choice made, the moment every connected player has said yes:
    /// the relic to its Bim — for good off a reward or a cache on a site
    /// already cleared, pending off a cache on one still to clear — or
    /// none, and the map up after a reward.
    pub(super) fn relic_if_carried(&mut self, events: &mut Vec<WorldEvent>) {
        let carried = self
            .run
            .relics
            .choice
            .as_ref()
            .and_then(|c| c.proposal.as_ref())
            .is_some_and(|p| p.carried(&self.run.connected));
        if !carried {
            return;
        }
        let Some(choice) = self.run.relics.choice.take() else {
            return;
        };
        let Some(proposal) = choice.proposal else {
            return;
        };
        match proposal.relic {
            None => events.push(WorldEvent::RelicsDeclined),
            Some(relic) => {
                let keep = choice.source == Source::Reward || self.mission_cleared();
                if keep {
                    self.run.relics.give(proposal.to, relic);
                    events.push(WorldEvent::RelicGiven {
                        slot: proposal.to,
                        relic: relic.code(),
                    });
                } else {
                    self.run.relics.pending.push((proposal.to, relic));
                    events.push(WorldEvent::RelicPending {
                        slot: proposal.to,
                        relic: relic.code(),
                    });
                }
            }
        }
        if choice.source == Source::Reward && self.run.phase == RunPhase::Reward {
            self.run.phase = RunPhase::Map;
        }
    }

    // --- caches ----------------------------------------------------------------------

    /// Whether a relic cache lies on the research desk of the site the
    /// crew are tied up at, with a desk on the deck to lie on.
    pub fn cache_here(&self) -> bool {
        self.in_mission()
            && self
                .ship
                .state
                .alongside()
                .and_then(|id| self.infestation(id))
                .is_some_and(|it| it.cache)
            && self.station_desk().is_some()
    }

    /// Whether a relic cache lies on a station's research desk: what the
    /// painter lights the desk for.
    pub fn cache_at(&self, station: u32) -> bool {
        self.infestation(station).is_some_and(|it| it.cache)
    }

    /// Where a crew member stands to open the cache, in the room's units.
    pub fn cache_spot(&self) -> Option<bims::math::Vec2> {
        if !self.cache_here() {
            return None;
        }
        self.research_desk_spot()
    }

    /// Whether crew member `who` may open the cache: one here, it alive,
    /// awake and within [`data::REACH`] of the desk, and no relic choice
    /// being made already.
    pub fn can_open_cache(&self, who: u32) -> Result<(), Refusal> {
        if !self.cache_here() {
            return Err(Refusal::NoCache);
        }
        if self.run.relics.choice.is_some() {
            return Err(Refusal::ChoosingRelic);
        }
        if !self.research_desk_in_reach(who) || !self.aboard.room.is_alive(who as usize) {
            return Err(Refusal::OutOfReach);
        }
        Ok(())
    }

    /// A cache opened — see [`Command::OpenCache`]: gone off the desk, and
    /// one relic of the site's tier put to the crew. An empty pool is an
    /// empty cache.
    pub(super) fn open_cache(&mut self, slot: u32, who: u32, events: &mut Vec<WorldEvent>) {
        if let Err(why) = self.can_open_cache(who) {
            events.push(refused(slot, why));
            return;
        }
        let Some(station) = self.ship.state.alongside() else {
            return;
        };
        if let Some(it) = self.infested.iter_mut().find(|it| it.station == station) {
            it.cache = false;
        }
        events.push(WorldEvent::CacheOpened { who });
        let tier = self.site_tier_code();
        let options = self.draw_relics(tier, 1, station);
        self.put_choice(Source::Cache, tier, options, events);
    }

    // --- a mission's relics -------------------------------------------------------------

    /// A mission begins: every once-a-mission relic ready again, and the
    /// trigger said to every relic that waits on it.
    pub(super) fn relics_at_mission_start(&mut self, events: &mut Vec<WorldEvent>) {
        let players = self.players();
        self.run.relics.new_mission(players);
        self.run.relics.choice = None;
        self.run.fought = false;
        self.run.cleared_here = false;
        for slot in 0..players {
            self.relic_trigger(slot, Trigger::MissionStart, events);
        }
    }

    /// Every hook crew member `who`'s relics have on `trigger`, fired
    /// where its conditions hold: once a mission, and under its share of
    /// health.
    pub(super) fn relic_trigger(&mut self, who: u32, trigger: Trigger, events: &mut Vec<WorldEvent>) {
        let hooks: Vec<(Relic, Hook)> = relic::hooks(self.relics_of(who), trigger).collect();
        for (relic, hook) in hooks {
            if hook.once_per_mission && self.run.relics.has_fired(who, relic) {
                continue;
            }
            if let Some(below) = hook.below_health {
                let share = self.health_share(who);
                if share * 100.0 >= below as f32 {
                    continue;
                }
            }
            if self.fire_relic(who, relic, hook.action, events) && hook.once_per_mission {
                self.run.relics.mark_fired(who, relic);
            }
        }
    }

    /// What a relic's action does, now. True when it did something.
    fn fire_relic(
        &mut self,
        who: u32,
        relic: Relic,
        action: Action,
        events: &mut Vec<WorldEvent>,
    ) -> bool {
        let fired = match action {
            // Getting up waits: the clock starts now and `settle_relics`
            // brings it round when it runs out.
            Action::GetUp { .. } => {
                let at = who as usize;
                if self.run.relics.down_since.len() <= at {
                    self.run.relics.down_since.resize(at + 1, None);
                }
                if self.run.relics.down_since[at].is_none() {
                    self.run.relics.down_since[at] = Some(self.mission_minutes());
                }
                return false;
            }
            Action::CooldownsLess { seconds } => self.cooldowns_less(who, seconds),
            Action::Untouchable { seconds } => {
                self.aboard.room.set_surge(who as usize, seconds, false);
                true
            }
        };
        if fired {
            events.push(WorldEvent::RelicFired {
                who,
                relic: relic.code(),
            });
        }
        fired
    }

    /// Every class cooldown running on crew member `who` made `seconds`
    /// shorter: the start of each moved back. True when one was running.
    fn cooldowns_less(&mut self, who: u32, seconds: f64) -> bool {
        let minutes = seconds * time::MINUTES_PER_SECOND;
        let mut any = false;
        if let Some(timers) = self.charge_timers.get_mut(who as usize) {
            for charge in Charge::ALL {
                if charge.everybody() {
                    continue;
                }
                if let Some(began) = timers[charge.code() as usize].as_mut() {
                    *began -= minutes;
                    any = true;
                }
            }
        }
        if let Some(tank) = self.tanks.get_mut(who as usize)
            && let Some(last) = tank.last_taunt.as_mut()
        {
            *last -= minutes;
            any = true;
        }
        if let Some(commander) = self.commanders.get_mut(who as usize)
            && let Some(last) = commander.last_rally.as_mut()
        {
            *last -= minutes;
            any = true;
        }
        any
    }

    /// A crew member's health as a share of its full, nought to one: the
    /// three parts added up, the one bar.
    pub fn health_share(&self, who: u32) -> f32 {
        if who >= self.aboard.crew_count() || !self.aboard.room.is_alive(who as usize) {
            return 0.0;
        }
        self.aboard.room.health(who as usize) / bims::health::MAX_HEALTH
    }

    /// The machines destroyed this step, each with who hit it last: the
    /// Republic's bounty for them — *Salvage Beacon*'s share on top of a
    /// holder's own kills — and every *Kill* trigger. What `visit` adds
    /// to the pending bounty.
    pub(crate) fn machine_kills(
        &mut self,
        kills: &[(Option<usize>, Money)],
        events: &mut Vec<WorldEvent>,
    ) -> Money {
        let mut total: Money = 0;
        for &(by, bounty) in kills {
            let by = by.map(|b| b as u32).filter(|&b| b < self.players());
            let paid = match by {
                Some(b) => {
                    let percent = self.relic_percent(b, Stat::Bounty);
                    bounty.saturating_add(bounty * percent.max(0) as Money / 100)
                }
                None => bounty,
            };
            total = total.saturating_add(paid);
            if let Some(b) = by {
                self.relic_trigger(b, Trigger::Kill, events);
            }
        }
        total
    }

    /// How many hits each player's Bim had taken, before the rooms step:
    /// what [`World::settle_relics`] reads a hit off.
    pub(crate) fn hits_before_the_step(&self) -> Vec<u32> {
        let players = self.players().min(self.aboard.crew_count());
        (0..players)
            .map(|who| self.aboard.room.hits_taken(who as usize))
            .collect()
    }

    /// The relics' stage, after the rooms have stepped: every player's
    /// Bim that went down or was hit since, its triggers — and the
    /// *Second Wind* whose wait is up brought round. `hits_before` is
    /// [`World::hits_before_the_step`]: the room counts every enemy hit
    /// landed on a body, a surge's included, and a count gone up is a hit.
    pub(crate) fn settle_relics(&mut self, hits_before: &[u32], events: &mut Vec<WorldEvent>) {
        let players = self.players().min(self.aboard.crew_count());
        for who in 0..players {
            if self.relics_of(who).is_empty() {
                continue;
            }
            let at = who as usize;
            let alive = self.aboard.room.is_alive(at);
            let down = alive && self.aboard.room.is_down(at);
            // Going down: the trigger, and — for a getting up — the wait
            // begun. A body back on its feet, or dead, waits for nothing.
            if down {
                let waiting = self.run.relics.down_since.get(at).copied().flatten().is_some();
                if !waiting {
                    self.relic_trigger(who, Trigger::Downed, events);
                }
            } else if let Some(d) = self.run.relics.down_since.get_mut(at) {
                *d = None;
            }
            // A hit taken.
            let hit = hits_before
                .get(at)
                .is_some_and(|&before| self.aboard.room.hits_taken(at) > before);
            if alive && hit {
                self.relic_trigger(who, Trigger::HitTaken, events);
            }
            self.get_up_if_due(who, events);
        }
    }

    /// A *Second Wind* whose wait has run out, brought round — once a
    /// mission — at its share of health.
    fn get_up_if_due(&mut self, who: u32, events: &mut Vec<WorldEvent>) {
        let at = who as usize;
        let Some(since) = self.run.relics.down_since.get(at).copied().flatten() else {
            return;
        };
        let hooks: Vec<(Relic, Hook)> =
            relic::hooks(self.relics_of(who), Trigger::Downed).collect();
        for (relic, hook) in hooks {
            let Action::GetUp {
                after,
                health_percent,
            } = hook.action
            else {
                continue;
            };
            if hook.once_per_mission && self.run.relics.has_fired(who, relic) {
                continue;
            }
            let waited = (self.mission_minutes() - since) / time::MINUTES_PER_SECOND;
            if waited < after {
                // Still waiting.
                return;
            }
            if self
                .aboard
                .room
                .bring_round(at, health_percent as f32 / 100.0)
            {
                self.run.relics.mark_fired(who, relic);
                if let Some(down) = self.crew_down.get_mut(at) {
                    *down = false;
                }
                events.push(WorldEvent::RelicFired {
                    who,
                    relic: relic.code(),
                });
            }
            self.run.relics.down_since[at] = None;
            return;
        }
        // Nothing left to get it up this mission: stop waiting.
        self.run.relics.down_since[at] = None;
    }

    /// The step the site is cleared, said once: the relics pending off a
    /// cache kept for good, and — with `BIMS_WIN` — the run won. Part of
    /// the run's last stage, after the pending bounty.
    pub(super) fn settle_clear(&mut self, events: &mut Vec<WorldEvent>) {
        if !self.run.fought || self.run.cleared_here || !self.mission_cleared() {
            return;
        }
        self.run.cleared_here = true;
        for (slot, relic) in std::mem::take(&mut self.run.relics.pending) {
            self.run.relics.give(slot, relic);
            events.push(WorldEvent::RelicGiven {
                slot,
                relic: relic.code(),
            });
        }
        if self.run.win_on_clear {
            self.run_won(events);
        }
    }

    /// The crew leaving a site (from `leave_mission`, while the ship is
    /// still tied up there): a cache's choice not made is dropped, the
    /// relics pending on a site left uncleared lost — the site, its cache
    /// with it, is put back — and a site cleared with machines in it
    /// offers its reward. True when the reward screen is to come up.
    pub(super) fn relics_on_leaving(
        &mut self,
        station: Option<u32>,
        cleared: bool,
        events: &mut Vec<WorldEvent>,
    ) -> bool {
        self.run.relics.choice = None;
        if !cleared {
            for (slot, relic) in std::mem::take(&mut self.run.relics.pending) {
                events.push(WorldEvent::RelicLost {
                    slot,
                    relic: relic.code(),
                });
            }
            return false;
        }
        // Cleared by now whatever `settle_clear` last saw: what is pending
        // is kept.
        self.settle_clear(events);
        match station {
            Some(id) if self.run.fought => self.offer_reward(id, events),
            _ => false,
        }
    }
}
