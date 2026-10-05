//! A site's experience (October 2026): its budget for the day, what each
//! enemy of a wave pays of it, what a player gets of that — the *Training
//! Log*, the catch-up — the site's **bonus wave**, and *Clean Sweep*.
//!
//! A site is worth `Rewards::site_xp_on(day)` to every player, twice that
//! at an elite's ([`data::ELITE_XP_PERCENT`]), whatever its waves are:
//! each wave is worth its share of the budget and each of its bodies its
//! share of the wave's ([`World::price_the_wave`], the step a wave is
//! laid), so the waves can be made bigger or smaller, for one player or
//! four, and the levels at the Heart do not move. It was fifteen an enemy,
//! whatever the wave, until then — a crew of four levelled four times as
//! fast as one alone.
//!
//! A child of `crate::world`, as `mission.rs` is, so it reaches the
//! world's private fields. **Nothing here draws from a stream**, and the
//! experience a player gets is read off what the world already hashes.

use super::*;
use crate::relic::Stat;
use crate::run::BonusWave;

impl World {
    // --- the budget ----------------------------------------------------------------

    /// What a site is worth on run day `day` (one the first), to every
    /// player: the rewards' dials (`Rewards::site_xp_on`).
    pub fn site_xp_on(&self, day: u32) -> u32 {
        self.rewards.site_xp_on(day)
    }

    /// The site at `id`'s budget today: the day's, twice that at an
    /// elite's fight (an Area defend's too).
    pub(crate) fn site_budget(&self, id: u32) -> u64 {
        let day = u64::from(self.site_xp_on(self.run_day()));
        if self.is_elite_fight(id) {
            day * u64::from(data::ELITE_XP_PERCENT) / 100
        } else {
            day
        }
    }

    /// How many waves the site at `id` has all told, the one landing
    /// counted: an attack's and a defence's own count, and an Area
    /// defend's as many as land in its hold ([`defense::area_waves`]).
    fn site_waves(&self, id: u32) -> u32 {
        if let Some(it) = self.infestation(id) {
            return it.wave.saturating_add(it.waves_left).max(1);
        }
        match self.defense(id) {
            Some(d) if d.area.is_some() => defense::area_waves(),
            Some(d) => d.wave.saturating_add(d.waves_left).max(1),
            None => 1,
        }
    }

    /// A wave of `bodies` enemies just laid at the site `id`: what each of
    /// them pays (`Run::xp_each`) — the site's budget over its waves, or
    /// for its bonus wave [`data::BONUS_WAVE_XP_PERCENT`] of it, over the
    /// bodies, rounded, one at the least. Asked again when a room built
    /// afresh lays the wave again, and answered the same.
    pub(crate) fn price_the_wave(&mut self, id: u32, bodies: u32) {
        if bodies == 0 {
            return;
        }
        let budget = self.site_budget(id);
        let share = if self.bonus_wave_at(id).running() {
            budget * u64::from(data::BONUS_WAVE_XP_PERCENT) / 100
        } else {
            budget / u64::from(self.site_waves(id))
        };
        let bodies = u64::from(bodies);
        let each = (share + bodies / 2) / bodies;
        self.run.xp_each = Some(each.clamp(1, u64::from(u32::MAX)) as u32);
    }

    /// The experience an enemy down is worth to every classed crew member
    /// in range, before the *Training Log* and the catch-up: its wave's
    /// share of the site's budget ([`World::price_the_wave`]) — at the
    /// Machine Heart its day's budget over [`data::HEART_XP_BODIES`], and
    /// [`class::XP_ENEMY_DOWN`] for an enemy no wave priced (a probe's) —
    /// times the relics', rounded down.
    pub fn xp_per_down(&self) -> u32 {
        let base = if self.at_the_heart() {
            let day = self.site_xp_on(self.run_day());
            (day + data::HEART_XP_BODIES / 2) / data::HEART_XP_BODIES.max(1)
        } else {
            self.run.xp_each.unwrap_or(class::XP_ENEMY_DOWN)
        };
        (f64::from(base) * self.crew_relic_factor(Stat::Experience)) as u32
    }

    /// The best level among the players' own Bims: what the catch-up
    /// measures by.
    pub(crate) fn best_player_level(&self) -> u8 {
        (0..self.players())
            .map(|slot| self.level_of(slot))
            .max()
            .unwrap_or(1)
    }

    /// What crew member `who` gets of `xp`, an enemy's worth: its best
    /// *Training Log*'s per cent more, and — a player's Bim below `best`,
    /// the best level among the players — [`data::CATCH_UP_XP_PERCENT`]
    /// more, added, rounded down.
    pub(crate) fn xp_for(&self, who: u32, xp: u32, best: u8) -> u32 {
        let mut percent = 100 + self.item_xp_percent(who);
        if who < self.players() && self.level_of(who) < best {
            percent += data::CATCH_UP_XP_PERCENT;
        }
        (u64::from(xp) * u64::from(percent) / 100).min(u64::from(u32::MAX)) as u32
    }

    // --- the bonus wave ------------------------------------------------------------

    /// The bonus wave of the site at `id`: none at a site with no fight.
    pub fn bonus_wave_at(&self, id: u32) -> BonusWave {
        if let Some(it) = self.infestation(id) {
            return it.bonus;
        }
        self.defense(id).map_or(BonusWave::None, |d| d.bonus)
    }

    /// The bonus wave of the site the ship is tied up at.
    pub fn bonus_wave_here(&self) -> BonusWave {
        self.ship
            .state
            .alongside()
            .map_or(BonusWave::None, |id| self.bonus_wave_at(id))
    }

    /// Whether player `slot` may call the bonus wave of the site here, or
    /// why not: a player, in a mission at a site whose fight was met and
    /// cleared (`NoBonusWave` otherwise), an attack's or a defence's —
    /// never an Area defend's, the Machine Heart's or one already called.
    pub fn can_call_bonus_wave(&self, slot: u32) -> Result<(), Refusal> {
        if slot >= self.players() {
            return Err(Refusal::NotAPlayer);
        }
        let Some(id) = self
            .ship
            .state
            .alongside()
            .filter(|_| self.run.phase == run::Phase::Mission && self.aboard.is_joined())
        else {
            return Err(Refusal::NoBonusWave);
        };
        if !self.run.fought || self.run.won || !self.site_cleared(id) || heart::is_heart(id) {
            return Err(Refusal::NoBonusWave);
        }
        if let Some(it) = self.infestation(id) {
            return if it.settled && it.heart.is_none() && it.bonus == BonusWave::None {
                Ok(())
            } else {
                Err(Refusal::NoBonusWave)
            };
        }
        match self.defense(id) {
            Some(d) if d.won && d.area.is_none() && d.bonus == BonusWave::None => Ok(()),
            _ => Err(Refusal::NoBonusWave),
        }
    }

    /// The bonus wave called — see [`Command::CallBonusWave`]: one more
    /// wave to come on the site's own clock, the deck thawed while it is
    /// on (`World::fight_over`), and nobody going home any more — a press
    /// of *Back to ship* before it is taken back.
    pub(crate) fn call_bonus_wave(&mut self, slot: u32) -> Result<(), Refusal> {
        self.can_call_bonus_wave(slot)?;
        let Some(id) = self.ship.state.alongside() else {
            return Err(Refusal::NoBonusWave);
        };
        if let Some(it) = self.infestation_mut(id) {
            it.bonus = BonusWave::Called;
            it.waves_left = 1;
            it.next_wave = None;
        } else if let Some(d) = self.defense_mut(id) {
            d.bonus = BonusWave::Called;
            d.waves_left = 1;
            d.next_in = None;
        }
        let players = self.players() as usize;
        self.run.returning = vec![false; players];
        self.run.recalled = false;
        self.run.departure = None;
        Ok(())
    }

    /// How many a wave of `n` is as it lands: [`data::BONUS_WAVE_SIZE_PERCENT`]
    /// of it, rounded up, while the site's bonus wave is the one landing.
    pub(crate) fn bonus_wave_size(&self, n: u32) -> u32 {
        if !self.bonus_wave_here().running() {
            return n;
        }
        (u64::from(n) * u64::from(data::BONUS_WAVE_SIZE_PERCENT))
            .div_ceil(100)
            .min(u64::from(u32::MAX)) as u32
    }

    /// The bonus wave's last enemy down at `id`: said, and *Clean Sweep*'s
    /// book settled for it.
    pub(crate) fn bonus_wave_cleared(&mut self, id: u32, events: &mut Vec<WorldEvent>) {
        events.push(WorldEvent::BonusWaveCleared { station: id });
        self.pay_the_clean_sweep(events);
    }

    // --- Clean Sweep ---------------------------------------------------------------

    /// `xp` given to crew member `who`: in *Clean Sweep*'s book.
    pub(crate) fn note_clean_xp(&mut self, who: u32, xp: u32) {
        let book = &mut self.run.clean_xp;
        if book.len() <= who as usize {
            book.resize(who as usize + 1, 0);
        }
        book[who as usize] = book[who as usize].saturating_add(xp);
    }

    /// A player's Bim gone down: no clean sweep at this site until its
    /// next clear.
    pub(crate) fn spoil_the_clean_sweep(&mut self, who: u32) {
        if who < self.players() {
            self.run.clean_spoiled = true;
        }
    }

    /// The site cleared (its fight, or its bonus wave): with *Clean Sweep*
    /// held and no player's Bim down since the last clear, every crew
    /// member gets its per cent of what it earned here since then
    /// ([`WorldEvent::CleanSweep`]); then the book starts again.
    pub(crate) fn pay_the_clean_sweep(&mut self, events: &mut Vec<WorldEvent>) {
        let book = std::mem::take(&mut self.run.clean_xp);
        let spoiled = std::mem::take(&mut self.run.clean_spoiled);
        let percent = crate::relic::crew_percent(self.relics(), Stat::CleanExperience);
        if spoiled || percent <= 0 {
            return;
        }
        for (who, earned) in book.into_iter().enumerate() {
            let xp = (u64::from(earned) * percent as u64 / 100).min(u64::from(u32::MAX)) as u32;
            if xp == 0 {
                continue;
            }
            self.award(who, xp, events);
            events.push(WorldEvent::CleanSweep {
                who: who as u32,
                xp,
            });
        }
    }
}
