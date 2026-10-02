//! The world's side of the relics (feature 106, [`crate::relic`]): what a
//! relic does to the Bim holding it, choosing one together, the caches on
//! the machines' sites, the reward for a site cleared, and the win.
//!
//! A child of `crate::world`, as `mission.rs` is, so it reaches the
//! world's private fields; the relics' own types are `crate::relic`'s.

use super::*;
use crate::relic::{self, Action, Hook, Relic, RelicChoice, RelicProposal, Source, Stat, Trigger};
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

    /// What may still be drawn this run: every relic unlocked that no Bim
    /// has got (task 117) — what is on offer or on a trader's table
    /// included, since an offer takes nothing out of it.
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

    /// Whether another player's Bim than `who` is down — downed, alive.
    fn other_player_down(&self, who: u32) -> bool {
        let crew = self.aboard.crew_count();
        (0..self.players().min(crew)).any(|s| {
            s != who
                && self.aboard.room.is_alive(s as usize)
                && self.aboard.room.is_down(s as usize)
        })
    }

    /// The percentage crew member `who`'s relics put on `stat` now —
    /// nought for a bot and for a Bim holding nothing for it.
    pub fn relic_percent(&self, who: u32, stat: Stat) -> i32 {
        // A *Coolant Loop* carried (October 2026: the relic in item form)
        // comes off the class's cooldowns with the relics', so every
        // cooldown that reads this reads it.
        let items = if stat == Stat::Cooldowns {
            -self.item_cooldown_cut(who)
        } else {
            0
        };
        let held = self.relics_of(who);
        if held.is_empty() {
            return items;
        }
        items
            + relic::stat_percent(
                held,
                stat,
                relic::Situation {
                    other_down: self.other_player_down(who),
                    ..relic::Situation::default()
                },
            )
    }

    /// The same as a factor: 1.1 for ten per cent.
    pub fn relic_factor(&self, who: u32, stat: Stat) -> f64 {
        relic::factor(self.relic_percent(who, stat))
    }

    /// What a crew member's relics do to its skill (`World::skill_of`):
    /// the damage, the fire rate, the pace, the armour and the overcharge. The skill as it was for anybody holding
    /// nothing.
    pub(super) fn lift_by_relics(&self, who: u32, skill: &mut bims::combat::Skill) {
        let held = self.relics_of(who);
        if held.is_empty() {
            return;
        }
        let f = |stat| self.relic_factor(who, stat) as f32;
        skill.damage *= f(Stat::Damage);
        skill.melee *= f(Stat::Damage);
        skill.fire_rate *= f(Stat::FireRate);
        skill.walk *= f(Stat::MoveSpeed);
        skill.armour_protection *= f(Stat::Armour);
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
        // The relics in item form (October 2026) are never drawn.
        pool.retain(|r| !r.retired());
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

    /// A probe's way through a fight: the site's waves stepped in and
    /// wrecked where they stand, one after another, until it is cleared —
    /// at most `steps` steps. Whether it was. `BIMS_REWARD=1`, for looking
    /// at the reward screen without the fight in front of it.
    pub fn clear_the_site_for_probe(&mut self, steps: u32) -> bool {
        let Some(station) = self.ship.state.alongside() else {
            return false;
        };
        for _ in 0..steps {
            self.step(&[]);
            if let Some(residents) = self.residents.as_mut() {
                let room = &mut residents.aboard.room;
                for i in 0..room.droid_count() as usize {
                    if room.droid(i).is_some_and(|d| !d.destroyed) {
                        room.strike_droid(i, bims::droid::DroidPart::Chassis, 1e6);
                    }
                }
            }
            if self.site_cleared(station) {
                return true;
            }
        }
        false
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
    /// profile unlocks relics off it. The Machine Heart's core destroyed
    /// calls it (feature 108, `World::heart_step`), and so does the
    /// probes' `BIMS_WIN`, on the next clear.
    pub fn run_won(&mut self, events: &mut Vec<WorldEvent>) {
        if self.run.won || self.lost {
            return;
        }
        self.run.won = true;
        events.push(WorldEvent::RunWon);
    }

    // --- offers ------------------------------------------------------------------

    /// `n` relics drawn by the one roll every source uses (task 117,
    /// [`relic::offer`]): the tier by the odds of the world clock's day
    /// now, off what the pool holds that is not in play — on offer,
    /// pending, held, or on the table of a trader still open. **The pool is
    /// not touched**: a relic leaves it when a Bim gets it. Seeded off the
    /// galaxy, the site and how many offers came before, so every client
    /// draws the same.
    pub(crate) fn draw_relics(&mut self, n: usize, station: u32) -> Vec<Relic> {
        self.release_closed_traders_relics();
        let seed = worldgen::rng::mix(
            self.galaxy_seed
                ^ worldgen::rng::mix(u64::from(self.star_id) << 32 | u64::from(station))
                ^ worldgen::rng::mix(0x_4F46_4645_5200 + u64::from(self.run.relics.offers)),
        );
        self.run.relics.offers = self.run.relics.offers.wrapping_add(1);
        let on_tables: Vec<Relic> = self.run.traders.iter().filter_map(|t| t.relic).collect();
        let drawable: Vec<Relic> = self
            .run
            .relics
            .pool
            .iter()
            .copied()
            .filter(|&r| !self.run.relics.in_play(r) && !on_tables.contains(&r))
            .collect();
        relic::offer(&drawable, self.days_gone(), n, seed)
    }

    /// A trader closed today puts the relic on its table back in the
    /// running (task 117): it is taken off the table, and a draw may pick
    /// it from then on. It never left the pool, so nothing is put back.
    fn release_closed_traders_relics(&mut self) {
        let day = self.days_gone();
        let closed: Vec<usize> = (0..self.run.traders.len())
            .filter(|&at| {
                let t = &self.run.traders[at];
                t.relic.is_some() && self.trader_closed_on(t.site.star, day)
            })
            .collect();
        for at in closed {
            self.run.traders[at].relic = None;
        }
    }

    /// A choice put to the crew: `options` off `source`. False, and no
    /// choice, with nothing to offer.
    fn put_choice(
        &mut self,
        source: Source,
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
        self.run.relics.choice = Some(RelicChoice::new(source, options));
        true
    }

    /// The reward for the site just left, cleared with machines in it
    /// (called by `leave_mission` before the rooms part): a relic for
    /// every player (task 146) — [`data::RELIC_OFFER`] by the day's odds,
    /// or one more than there are players where that is more, so the last
    /// to pick still has a choice — and the reward screen up. False, and
    /// straight to the map, with nothing left to draw.
    pub(super) fn offer_reward(&mut self, station: u32, events: &mut Vec<WorldEvent>) -> bool {
        let n = data::RELIC_OFFER.max(self.players() as usize + 1);
        let options = self.draw_relics(n, station);
        self.put_choice(Source::Reward, options, events)
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
        // At a trader the relic is the player's own trader's (task 114),
        // bought outright with its own money: no vote.
        if self.run.phase == RunPhase::Trade {
            self.buy_trader_relic(slot, relic, events);
            return;
        }
        // On the reward screen every player picks its own (task 146).
        if self
            .run
            .relics
            .choice
            .as_ref()
            .is_some_and(|c| c.source == Source::Reward)
        {
            self.pick_reward_relic(slot, relic, events);
            return;
        }
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
                // Got, for good or pending: out of the pool (task 117). The
                // options not taken were never out of it.
                self.run.relics.take_from_pool(relic);
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

    /// Player `slot`'s pick off the reward, for its own Bim (task 146): a
    /// relic still to be won, by a player that has won none yet. It
    /// replaces the player's last pick this round; the last pick in
    /// settles the round.
    fn pick_reward_relic(&mut self, slot: u32, relic: Option<Relic>, events: &mut Vec<WorldEvent>) {
        let players = self.players();
        let Some(choice) = self.run.relics.choice.as_mut() else {
            events.push(refused(slot, Refusal::NoRelicChoice));
            return;
        };
        if slot >= players {
            events.push(refused(slot, Refusal::NotAPlayer));
            return;
        }
        if choice.won_by(slot).is_some() {
            events.push(refused(slot, Refusal::NoRelicChoice));
            return;
        }
        let Some(relic) = relic.filter(|r| choice.left().contains(r)) else {
            events.push(refused(slot, Refusal::NotOnOffer));
            return;
        };
        choice.picks.resize(players as usize, None);
        choice.picks[slot as usize] = Some(relic);
        events.push(WorldEvent::RelicPicked {
            slot,
            relic: relic.code(),
        });
        self.settle_reward_picks(events);
    }

    /// The reward's round settled once every connected player still
    /// without a relic off it has picked (task 146): a relic one player
    /// picked is that player's; one picked by more goes by the dice
    /// ([`relic::dice`]: two each, in slot order, the highest sum, the
    /// tied again), each throw said as [`WorldEvent::RelicDice`]. Who lost
    /// picks again out of what is left. Once every connected player has
    /// one — or nothing is left — the choice is over and the map up. A
    /// player gone is not waited for, and wins nothing more.
    pub(super) fn settle_reward_picks(&mut self, events: &mut Vec<WorldEvent>) {
        let players = self.players();
        let connected: Vec<bool> = (0..players).map(|s| self.run.is_connected(s)).collect();
        let galaxy = self.galaxy_seed;
        let offers = self.run.relics.offers;
        let Some(choice) = self.run.relics.choice.as_mut() else {
            return;
        };
        if choice.source != Source::Reward {
            return;
        }
        choice.picks.resize(players as usize, None);
        choice.won.resize(players as usize, None);
        let mut given: Vec<(u32, Relic)> = Vec::new();
        let waiting = |c: &RelicChoice| -> Vec<u32> {
            (0..players)
                .filter(|&s| connected[s as usize] && c.won_by(s).is_none())
                .collect()
        };
        let waiting_now = waiting(choice);
        let left = choice.left();
        if !waiting_now.is_empty() && !left.is_empty() {
            if waiting_now.iter().any(|&s| choice.pick_of(s).is_none()) {
                return;
            }
            for relic in left {
                let by: Vec<u32> = waiting_now
                    .iter()
                    .copied()
                    .filter(|&s| choice.pick_of(s) == Some(relic))
                    .collect();
                let winner = match by.len() {
                    0 => continue,
                    1 => by[0],
                    _ => {
                        let seed = worldgen::rng::mix(
                            galaxy
                                ^ worldgen::rng::mix(0x_4449_4345_0000 + u64::from(offers))
                                ^ worldgen::rng::mix(
                                    u64::from(choice.round) << 32 | u64::from(relic.code()),
                                ),
                        );
                        let (winner, throws) = relic::dice(&by, relic, seed);
                        for t in throws {
                            events.push(WorldEvent::RelicDice {
                                slot: t.slot,
                                relic: t.relic.code(),
                                a: t.a,
                                b: t.b,
                            });
                        }
                        winner
                    }
                };
                choice.won[winner as usize] = Some(relic);
                given.push((winner, relic));
            }
            choice.picks = vec![None; players as usize];
            choice.round += 1;
        }
        let over = waiting(choice).is_empty() || choice.left().is_empty();
        for (slot, relic) in given {
            self.run.relics.take_from_pool(relic);
            self.run.relics.give(slot, relic);
            events.push(WorldEvent::RelicGiven {
                slot,
                relic: relic.code(),
            });
        }
        if over {
            self.run.relics.choice = None;
            if self.run.phase == RunPhase::Reward {
                self.run.phase = RunPhase::Map;
            }
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
    /// one relic, drawn by the day's odds, put to the crew. Nothing left to
    /// draw is an empty cache.
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
        let options = self.draw_relics(1, station);
        self.put_choice(Source::Cache, options, events);
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
    pub(super) fn relic_trigger(
        &mut self,
        who: u32,
        trigger: Trigger,
        events: &mut Vec<WorldEvent>,
    ) {
        self.relic_trigger_on(who, trigger, None, events);
    }

    /// [`World::relic_trigger`] about another crew member, `other` — the
    /// crewmate dressed, the crewmate gone down (task 118) — and under a
    /// hook's cooldown as well.
    pub(crate) fn relic_trigger_on(
        &mut self,
        who: u32,
        trigger: Trigger,
        other: Option<u32>,
        events: &mut Vec<WorldEvent>,
    ) {
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
            if hook.cooldown.is_some() && !self.relic_ready(who, relic) {
                continue;
            }
            if self.fire_relic(who, relic, hook.action, other, events) {
                if hook.once_per_mission {
                    self.run.relics.mark_fired(who, relic);
                }
                if let Some(seconds) = hook.cooldown {
                    self.relic_cools(who, relic, seconds);
                }
            }
        }
    }

    /// What a relic's action does, now. True when it did something.
    fn fire_relic(
        &mut self,
        who: u32,
        relic: Relic,
        action: Action,
        other: Option<u32>,
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
                self.aboard.room.set_surge(who as usize, seconds);
                true
            }
            // Task 118's.
            Action::Sprint { seconds, .. } | Action::Unseen { seconds } => {
                self.put_relic_buff(who, relic, seconds);
                true
            }
            Action::Tether { seconds, .. } => match other {
                Some(patient) => {
                    self.put_relic_buff(patient, relic, seconds);
                    true
                }
                None => false,
            },
            Action::Shield { tiles, hp, seconds } => {
                self.relic_shield(who, other, tiles, hp, seconds)
            }
            Action::RallyUp {
                tiles,
                health_percent,
            } => self.relic_rally_up(who, tiles, health_percent),
            Action::Heal { points } => match other {
                // Times the Healing Aura where the patient stands (task 130).
                Some(patient) => self.heal_crew(patient, points) > 0.0,
                None => false,
            },
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
    pub(super) fn cooldowns_less(&mut self, who: u32, seconds: f64) -> bool {
        let minutes = seconds * time::MINUTES_PER_SECOND;
        let mut any = false;
        if let Some(timers) = self.charge_timers.get_mut(who as usize) {
            for charge in Charge::ALL {
                if let Some(began) = timers[charge.code() as usize].as_mut() {
                    *began -= minutes;
                    any = true;
                }
            }
        }
        // The tank's Taunt and Juggernaut (task 139): each timestamp is
        // the cooldown alone — the window running is its own, and does
        // not move.
        if let Some(tank) = self.tanks.get_mut(who as usize) {
            for last in [tank.last_taunt.as_mut(), tank.last_juggernaut.as_mut()]
                .into_iter()
                .flatten()
            {
                *last -= minutes;
                any = true;
            }
        }
        // The commander's Rally and Battle Cry (task 129): the start of a
        // cooldown moves back once the shout itself is over — moved while
        // it runs, it would cut the shout short, which is the one thing
        // their single timestamp cannot tell apart.
        let rallying = self.is_rallying(who);
        let crying = self.is_crying(who);
        if let Some(commander) = self.commanders.get_mut(who as usize) {
            if !rallying && let Some(last) = commander.last_rally.as_mut() {
                *last -= minutes;
                any = true;
            }
            if !crying && let Some(last) = commander.last_battle_cry.as_mut() {
                *last -= minutes;
                any = true;
            }
            // His call for reinforcements has nothing that runs: its
            // cooldown alone.
            if let Some(last) = commander.last_reinforcement.as_mut() {
                *last -= minutes;
                any = true;
            }
            // Nor has his call for a medic.
            if let Some(last) = commander.last_medivac.as_mut() {
                *last -= minutes;
                any = true;
            }
        }
        // The engineer's sentry's cooldown (task 127), never its time.
        if let Some(engineer) = self.engineers.get_mut(who as usize)
            && let Some(laid) = engineer.sentry_laid.as_mut()
        {
            *laid -= minutes;
            any = true;
        }
        // A Rampage's cooldown (task 124), never the one running.
        if let Some(soldier) = self.soldiers.get_mut(who as usize)
            && let Some(began) = soldier.began.as_mut()
        {
            *began -= minutes;
            any = true;
        }
        // A medic's Nanite Burst and Cloak (task 130): each a timestamp
        // that is the cooldown alone — the cloak it cast is kept on the
        // crew member it covers, and does not move.
        if let Some(medic) = self.medics.get_mut(who as usize) {
            for last in [medic.last_burst.as_mut(), medic.last_cloak.as_mut()]
                .into_iter()
                .flatten()
            {
                *last -= minutes;
                any = true;
            }
        }
        any
    }

    /// A crew member's health as a share of its full, nought to one: the
    /// one bar, items and level included.
    pub fn health_share(&self, who: u32) -> f32 {
        if who >= self.aboard.crew_count() || !self.aboard.room.is_alive(who as usize) {
            return 0.0;
        }
        let room = &self.aboard.room;
        room.health(who as usize) / room.max_health(who as usize)
    }

    /// The machines destroyed this step, each with who hit it last: the
    /// Republic's bounty for them — *Salvage Beacon*'s share on top of a
    /// holder's own kills — and every *Kill* trigger. The tests' way in;
    /// `visit` says how each went too, through
    /// [`World::machine_kills_noted`].
    #[cfg(test)]
    pub(crate) fn machine_kills(
        &mut self,
        kills: &[(Option<usize>, Money)],
        events: &mut Vec<WorldEvent>,
    ) -> Money {
        let kills: Vec<relic_hooks::MachineKill> = kills
            .iter()
            .map(|&(by, bounty)| relic_hooks::MachineKill {
                by,
                bounty,
                crippled: false,
                flanked: false,
            })
            .collect();
        self.machine_kills_noted(&kills, events)
    }

    /// [`World::machine_kills`] with how each machine went (task 118):
    /// *Parts Broker*'s share on one missing a limb, and the triggers
    /// `relics_on_a_kill` says.
    pub(crate) fn machine_kills_noted(
        &mut self,
        kills: &[relic_hooks::MachineKill],
        events: &mut Vec<WorldEvent>,
    ) -> Money {
        let mut total: Money = 0;
        for kill in kills {
            let bounty = kill.bounty;
            let by = kill.by.map(|b| b as u32).filter(|&b| b < self.players());
            let paid = match by {
                Some(b) => {
                    let percent = relic::stat_percent(
                        self.relics_of(b),
                        Stat::Bounty,
                        relic::Situation {
                            other_down: self.other_player_down(b),
                            crippled: kill.crippled,
                            ..relic::Situation::default()
                        },
                    );
                    bounty.saturating_add(bounty * percent.max(0) as Money / 100)
                }
                None => bounty,
            };
            total = total.saturating_add(paid);
            if let Some(b) = by {
                self.relic_trigger(b, Trigger::Kill, events);
                // A Rampage at its fourth rank is lengthened by a kill
                // credited the same way (task 124).
                self.rampage_kill(b);
            }
            if self.any_relics() {
                self.relics_on_a_kill(kill, events);
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
                let waiting = self
                    .run
                    .relics
                    .down_since
                    .get(at)
                    .copied()
                    .flatten()
                    .is_some();
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
        // The fight over, the armour whole again: mended only at the next
        // mission's start, the crew sheet, the reward screen and a trader
        // showed a broken piece until then.
        self.mend_all_armour();
        // The run's summary (feature 108): a site cleared of machines, and
        // a system liberated when the site was its jammer.
        self.run.sites_cleared = self.run.sites_cleared.saturating_add(1);
        let here = self.run.site.or_else(|| self.ship.state.alongside());
        if here.is_some() && here == self.jammer_station() {
            self.run.systems_liberated = self.run.systems_liberated.saturating_add(1);
        }
        for (slot, relic) in std::mem::take(&mut self.run.relics.pending) {
            self.run.relics.give(slot, relic);
            events.push(WorldEvent::RelicGiven {
                slot,
                relic: relic.code(),
            });
        }
        // *Hazard Pay* (task 118).
        self.relics_pay_the_clear(events);
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
            // Lost, and back in the pool (task 117).
            for (slot, relic) in std::mem::take(&mut self.run.relics.pending) {
                self.run.relics.return_to_pool(relic);
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
        // Only an elite drops relics (`crate::elite`): every other fight
        // goes straight to the map.
        match station {
            Some(id) if self.run.fought && self.is_elite_here(id) => self.offer_reward(id, events),
            _ => false,
        }
    }
}
