//! The world's side of the relics (feature 106, rebuilt October 2026,
//! [`crate::relic`]): what the crew's relics do to every crew member and
//! to the run, choosing one together off an elite's clear, and the win.
//!
//! A child of `crate::world`, as `mission.rs` is, so it reaches the
//! world's private fields; the relics' own types are `crate::relic`'s.
//! **Nothing here draws from a stream a fight draws from**, and a crew
//! holding no relic plays exactly as it did.

use super::*;
use crate::relic::{self, Relic, RelicChoice, RelicProposal, Stat};
use crate::run::Phase as RunPhase;

impl World {
    // --- reading ---------------------------------------------------------------

    /// The relics the crew hold, in the order they took them.
    pub fn relics(&self) -> &[Relic] {
        &self.run.relics.held
    }

    /// Whether the crew hold `relic`.
    pub fn has_relic(&self, relic: Relic) -> bool {
        self.relics().contains(&relic)
    }

    /// The relic choice being made, if one is: the reward screen's.
    pub fn relic_choice(&self) -> Option<&RelicChoice> {
        self.run.relics.choice.as_ref()
    }

    /// Whether the crew are on the reward screen: an elite's site left
    /// cleared, a relic being chosen, the map to come.
    pub fn choosing_reward(&self) -> bool {
        self.run.phase == RunPhase::Reward
    }

    /// Whether the run is won.
    pub fn is_won(&self) -> bool {
        self.run.won
    }

    /// Whether crew member `who` is a bot to the relics: anybody no player
    /// steers ([`relic::Who::Bots`]).
    fn relic_bot(&self, who: u32) -> bool {
        who >= self.players()
    }

    /// The percentage the crew's relics put on crew member `who`'s `stat`
    /// now — and, for [`Stat::Cooldowns`], what its *Coolant Loop*s carried
    /// take off (October 2026: the old relic in item form), so every class
    /// cooldown that reads this reads both.
    pub fn relic_percent(&self, who: u32, stat: Stat) -> i32 {
        let items = if stat == Stat::Cooldowns {
            -self.item_cooldown_cut(who)
        } else {
            0
        };
        items + relic::percent(self.relics(), stat, self.relic_bot(who))
    }

    /// The same as a factor: 1.1 for ten per cent.
    pub fn relic_factor(&self, who: u32, stat: Stat) -> f64 {
        relic::factor(self.relic_percent(who, stat))
    }

    /// The factor the crew's relics put on a stat of the run's own: the
    /// bounty, the experience, the waves, the trader's prices, the damage
    /// to a machine.
    pub fn crew_relic_factor(&self, stat: Stat) -> f64 {
        relic::factor(relic::crew_percent(self.relics(), stat))
    }

    /// What the crew's relics do to crew member `who`'s skill
    /// (`World::skill_of`): the damage, the fire rate, the pace and the
    /// share of every hit it takes. The skill as it was with none held.
    pub(super) fn lift_by_relics(&self, who: u32, skill: &mut bims::combat::Skill) {
        if self.relics().is_empty() {
            return;
        }
        let f = |stat| self.relic_factor(who, stat) as f32;
        let damage = f(Stat::Damage);
        skill.damage *= damage;
        skill.melee *= damage;
        skill.fire_rate *= f(Stat::FireRate);
        skill.walk *= f(Stat::MoveSpeed);
        skill.damage_taken *= f(Stat::DamageTaken);
    }

    /// The experience an enemy down is worth to the crew: the dial's,
    /// times the relics', rounded down.
    pub fn xp_per_down(&self) -> u32 {
        (f64::from(self.rewards.xp_per_down) * self.crew_relic_factor(Stat::Experience)) as u32
    }

    /// A bounty as the crew's relics pay it, rounded down.
    pub(crate) fn bounty_by_relics(&self, amount: Money) -> Money {
        if self.relics().is_empty() {
            return amount;
        }
        (amount as f64 * self.crew_relic_factor(Stat::Bounty)) as Money
    }

    /// A wave of `n` machines as the crew's relics make it, rounded up.
    pub(crate) fn wave_by_relics(&self, n: u32) -> u32 {
        if self.relics().is_empty() {
            return n;
        }
        (f64::from(n) * self.crew_relic_factor(Stat::WaveSize)).ceil() as u32
    }

    /// How many more machines a player the crew's relics put in one more
    /// wave at every elite (*Black Market*): nought for a crew holding none.
    pub fn elite_wave_extra(&self) -> u32 {
        relic::crew_percent(self.relics(), Stat::EliteWave).max(0) as u32
    }

    /// Whether the wave aboard at `station` is the relics' wave: the last
    /// at an elite while the crew hold one that adds it — past the first,
    /// and none left to come.
    pub(crate) fn is_relic_wave(&self, station: u32) -> bool {
        self.elite_wave_extra() > 0
            && self.is_elite_here(station)
            && self
                .infestation(station)
                .is_some_and(|it| it.wave > 1 && it.waves_left == 0)
    }

    /// What a trader's `price` comes to with the crew's relics, whole euros
    /// rounded down.
    pub fn trader_price_by_relics(&self, price: Money) -> Money {
        if self.relics().is_empty() {
            return price;
        }
        (price as f64 * self.crew_relic_factor(Stat::TraderPrices)) as Money
    }

    // --- setting a run up --------------------------------------------------------

    /// A probe's way to give the crew a relic at once (`BIMS_RELICS`).
    pub fn give_relic_for_probe(&mut self, relic: Relic) {
        self.run.relics.give(relic);
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
    /// way it does on [`WorldEvent::CrewLost`]. The Machine Heart's core
    /// destroyed calls it (feature 108, `World::heart_step`), and so does
    /// the probes' `BIMS_WIN`, on the next clear.
    pub fn run_won(&mut self, events: &mut Vec<WorldEvent>) {
        if self.run.won || self.lost {
            return;
        }
        self.run.won = true;
        events.push(WorldEvent::RunWon);
    }

    // --- the offer ------------------------------------------------------------------

    /// `n` relics drawn evenly off what the crew do not hold
    /// ([`relic::offer`]), seeded off the galaxy, the site and how many
    /// offers came before, so every client draws the same.
    pub(crate) fn draw_relics(&mut self, n: usize, station: u32) -> Vec<Relic> {
        let seed = worldgen::rng::mix(
            self.galaxy_seed
                ^ worldgen::rng::mix(u64::from(self.star_id) << 32 | u64::from(station))
                ^ worldgen::rng::mix(0x_4F46_4645_5200 + u64::from(self.run.relics.offers)),
        );
        self.run.relics.offers = self.run.relics.offers.wrapping_add(1);
        relic::offer(&self.run.relics.drawable(), n, seed)
    }

    /// The reward for an elite's site just left cleared (called by
    /// `leave_mission` before the rooms part): [`data::RELIC_OFFER`]
    /// relics put to the crew and the reward screen up. False, and
    /// straight to the map, with nothing left to offer.
    pub(super) fn offer_reward(&mut self, station: u32, events: &mut Vec<WorldEvent>) -> bool {
        let options = self.draw_relics(data::RELIC_OFFER, station);
        if options.is_empty() {
            return false;
        }
        events.push(WorldEvent::RelicsOffered {
            count: options.len() as u32,
        });
        self.run.relics.choice = Some(RelicChoice::new(options));
        true
    }

    // --- choosing together ---------------------------------------------------------

    /// A relic — or none — put to the crew, see [`Command::ProposeRelic`]:
    /// a choice being made and the relic among its options. Every
    /// acceptance goes with the last proposal; the proposer's own yes is
    /// counted.
    pub(super) fn propose_relic(
        &mut self,
        slot: u32,
        relic: Option<Relic>,
        events: &mut Vec<WorldEvent>,
    ) {
        let players = self.players();
        if slot >= players {
            events.push(refused(slot, Refusal::NotAPlayer));
            return;
        }
        let Some(choice) = self.run.relics.choice.as_mut() else {
            events.push(refused(slot, Refusal::NoRelicChoice));
            return;
        };
        if relic.is_some_and(|r| !choice.options.contains(&r)) {
            events.push(refused(slot, Refusal::NotOnOffer));
            return;
        }
        let mut accepted = vec![false; players as usize];
        accepted[slot as usize] = true;
        choice.proposal = Some(RelicProposal {
            relic,
            by: slot,
            accepted,
        });
        events.push(WorldEvent::RelicProposed {
            slot,
            relic: relic.map_or(u32::MAX, Relic::code),
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
    /// the relic the crew's for good — or none — and the map up.
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
        let Some(proposal) = self.run.relics.choice.take().and_then(|c| c.proposal) else {
            return;
        };
        match proposal.relic {
            None => events.push(WorldEvent::RelicsDeclined),
            Some(relic) => {
                self.run.relics.give(relic);
                events.push(WorldEvent::RelicGiven {
                    relic: relic.code(),
                });
            }
        }
        if self.run.phase == RunPhase::Reward {
            self.run.phase = RunPhase::Map;
        }
    }

    // --- a mission's relics -------------------------------------------------------------

    /// A mission begins: no choice left standing, and nothing fought or
    /// cleared yet.
    pub(super) fn relics_at_mission_start(&mut self) {
        self.run.relics.choice = None;
        self.run.fought = false;
        self.run.cleared_here = false;
    }

    /// Every class cooldown running on crew member `who` made `seconds`
    /// shorter: the start of each moved back. True when one was running.
    /// The *Reset Capacitor*'s (`item_use.rs`).
    pub(crate) fn cooldowns_less(&mut self, who: u32, seconds: f64) -> bool {
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
        // A Stun Shot's cooldown (October 2026), never a charge.
        if let Some(soldier) = self.soldiers.get_mut(who as usize)
            && let Some(fired) = soldier.last_shot.as_mut()
        {
            *fired -= minutes;
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

    /// The machines destroyed this step, each with who hit it last and
    /// what the Republic pays for it: the bounty summed — the relics'
    /// share is `earn_bounty`'s — and a Rampage at its fourth rank
    /// lengthened by a kill its soldier is credited with (task 124).
    pub(crate) fn machine_kills_noted(&mut self, kills: &[(Option<usize>, Money)]) -> Money {
        let mut total: Money = 0;
        for &(by, bounty) in kills {
            total = total.saturating_add(bounty);
            if let Some(b) = by.map(|b| b as u32).filter(|&b| b < self.players()) {
                self.rampage_kill(b);
            }
        }
        total
    }

    /// How many hits each player's Bim had taken, before the rooms step:
    /// what the items read a hit off (`settle_items`).
    pub(crate) fn hits_before_the_step(&self) -> Vec<u32> {
        let players = self.players().min(self.aboard.crew_count());
        (0..players)
            .map(|who| self.aboard.room.hits_taken(who as usize))
            .collect()
    }

    /// Every crew hit this step that landed on one of the residents'
    /// machines, delivered — the part read off the hit's own roll, the
    /// relics' share on the damage, a top-rank EMP's exposure on the same
    /// sum (task 127), then a Weak Spot's crit (task 124) — and the rest,
    /// the hits on the residents' Bims, handed back for `visit` to land.
    pub(crate) fn land_on_machines(
        &mut self,
        hits: Vec<bims::combat::Hit>,
    ) -> Vec<bims::combat::Hit> {
        if self.residents.is_none() {
            return hits;
        }
        let relics = relic::crew_percent(self.relics(), Stat::MachineDamage);
        // What each hit did to which machine, by whom: what the items
        // read after (October 2026, `item_use.rs`).
        let mut landed: Vec<(Option<usize>, usize, f32)> = Vec::new();
        let mut rest = Vec::new();
        for hit in hits {
            let Some(residents) = self.residents.as_ref() else {
                break;
            };
            let room = &residents.aboard.room;
            let bims = room.crew_count() as usize;
            let Some(i) = hit.who.checked_sub(bims) else {
                rest.push(hit);
                continue;
            };
            if hit.who >= room.body_count() as usize || !room.is_alive(hit.who) {
                continue;
            }
            let part = bims::droid::DroidPart::hit_by(hit.roll);
            let exposed = room.droid(i).is_some_and(|d| d.is_exposed());
            let percent = relics
                + if exposed {
                    class::EMP_EXPOSE_PERCENT
                } else {
                    0
                };
            let damage = if percent == 0 {
                hit.damage
            } else {
                hit.damage * relic::factor(percent) as f32
            };
            // Weak Spot's crit, after every other factor.
            let damage = damage + self.crit_extra(&hit);
            let Some(residents) = self.residents.as_mut() else {
                break;
            };
            residents.aboard.room.strike_droid(i, part, damage);
            landed.push((hit.by, i, damage));
            // And what it did, for the number over the machine.
            self.shown_hits.push((hit.who as u32, damage, hit.crit));
            if let Some(last) = residents.last_hit_by.get_mut(hit.who) {
                *last = hit.by;
            }
        }
        self.items_on_machine_hits(&landed);
        rest
    }

    /// The hit points the crew's relics put back this step: every crew
    /// member on its feet, through `heal_crew` (a medic's aura lifts it).
    pub(crate) fn relics_mend(&mut self) {
        let crew = self.aboard.crew_count();
        let seconds = (data::STEP_MINUTES / time::MINUTES_PER_SECOND) as f32;
        for who in 0..crew {
            let rate = relic::percent(self.relics(), Stat::Regen, self.relic_bot(who));
            let at = who as usize;
            if rate <= 0 || !self.aboard.room.is_alive(at) || self.aboard.room.is_down(at) {
                continue;
            }
            self.heal_crew(who, rate as f32 * seconds);
        }
    }

    /// The step the site is cleared, said once: the armour mended, the
    /// run's summary counted and — with `BIMS_WIN` — the run won. Part of
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
        if self.run.win_on_clear {
            self.run_won(events);
        }
    }

    /// The crew leaving a site (from `leave_mission`, while the ship is
    /// still tied up there): a site cleared with machines in it said
    /// cleared, and — **an elite's alone** (`crate::elite`) — its reward
    /// offered. True when the reward screen is to come up.
    pub(super) fn relics_on_leaving(
        &mut self,
        station: Option<u32>,
        cleared: bool,
        events: &mut Vec<WorldEvent>,
    ) -> bool {
        self.run.relics.choice = None;
        if !cleared {
            return false;
        }
        self.settle_clear(events);
        match station {
            Some(id) if self.run.fought && self.is_elite_here(id) => self.offer_reward(id, events),
            _ => false,
        }
    }
}
