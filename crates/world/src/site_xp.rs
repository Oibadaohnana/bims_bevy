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
            // The Overseer, a heist and a fuel run land waves on a clock
            // until their objective is done (October 2026): about so many.
            if self.objective_on_a_clock(id) {
                return data::OBJECTIVE_WAVES;
            }
            return it.wave.saturating_add(it.waves_left).max(1);
        }
        match self.defense(id) {
            Some(d) if d.area.is_some() => defense::area_waves(),
            // Seal the breaches: a wave a breach and one more, near enough.
            Some(d) if d.breaches.is_some() => d.breaches.map_or(1, |b| b.total + 1),
            // An Evacuation: about four waves before its people are aboard.
            Some(d) if d.evacuation.is_some() => 4,
            Some(d) => d.wave.saturating_add(d.waves_left).max(1),
            None => 1,
        }
    }

    /// A wave of `bodies` enemies just laid at the site `id`: what each of
    /// them pays in experience (`Run::xp_each`) and in money
    /// (`Run::money_each`) — each budget over the site's own waves (a
    /// bonus wave to come not counted), or for the bonus wave
    /// [`data::BONUS_WAVE_XP_PERCENT`] of the experience's and
    /// [`data::BONUS_WAVE_MONEY_PERCENT`] of the money's, over the
    /// bodies, rounded, one at the least. The experience's budget is the
    /// elite's twice already; the money's is lifted at an elite where it is paid
    /// (`World::bounty_here`). Asked again when a room built afresh lays
    /// the wave again, and answered the same.
    pub(crate) fn price_the_wave(&mut self, id: u32, bodies: u32) {
        if bodies == 0 {
            return;
        }
        let money = self.site_money_here();
        let xp = self.site_budget(id);
        let bonus = self.run.bonus == BonusWave::Landed;
        let waves = u64::from(
            self.site_waves(id)
                .saturating_sub(self.bonus_waves_to_come())
                .max(1),
        );
        let bodies = u64::from(bodies);
        // *Overtime* (October 2026): the bonus wave's shares lifted.
        let overtime = crate::relic::factor(crate::relic::crew_percent(
            self.relics(),
            Stat::BonusWavePay,
        ));
        let each = |budget: u64, bonus_percent: u32| {
            let share = if bonus {
                let percent = (f64::from(bonus_percent) * overtime) as u64;
                budget * percent / 100
            } else {
                budget / waves
            };
            ((share + bodies / 2) / bodies).max(1)
        };
        self.run.xp_each =
            Some(each(xp, data::BONUS_WAVE_XP_PERCENT).min(u64::from(u32::MAX)) as u32);
        self.run.money_each = Some(each(money, data::BONUS_WAVE_MONEY_PERCENT));
    }

    /// What a site pays on run day `day`, a player: the rewards' dials
    /// (`Rewards::site_money_on`).
    pub fn site_money_on(&self, day: u32) -> Money {
        self.rewards.site_money_on(day)
    }

    /// What a site pays today, all told: a player's share for every
    /// player, since the money goes into the one pool every wallet shares
    /// (`World::share_out`) — so each player is paid as one alone is,
    /// however many there are.
    pub fn site_money_here(&self) -> Money {
        self.site_money_on(self.run_day())
            .saturating_mul(Money::from(self.players().max(1)))
    }

    /// The money an enemy down is worth before who took it down, its kind
    /// and the site say theirs: its wave's share of the site's money
    /// ([`World::price_the_wave`]) — at the Machine Heart, and for an
    /// enemy no wave priced (a probe's), the day's over
    /// [`data::HEART_XP_BODIES`].
    pub fn money_per_down(&self) -> Money {
        match self.run.money_each.filter(|_| !self.at_the_heart()) {
            Some(each) => each,
            None => {
                let day = self.site_money_here();
                let bodies = Money::from(data::HEART_XP_BODIES.max(1));
                (day + bodies / 2) / bodies
            }
        }
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

    /// This mission's bonus wave ([`Run::bonus`](crate::run::Run::bonus)).
    pub fn bonus_wave_here(&self) -> BonusWave {
        self.run.bonus
    }

    /// One while the crew have chosen a bonus wave that has not landed:
    /// what a site's wave count is settled with, and what its own waves
    /// share their budget without.
    pub(crate) fn bonus_waves_to_come(&self) -> u32 {
        u32::from(self.run.bonus == BonusWave::Chosen)
    }

    /// Whether player `slot` may choose this mission's bonus wave, or take
    /// it back, or why not: a player (`NotAPlayer`), in the ready check
    /// before the fight (`NoBonusWave` otherwise), at a site with a fight
    /// to come ([`World::bonus_wave_allowed`]). With *Overtime* held the
    /// wave is the relic's (`BonusWaveHeld`).
    pub fn can_choose_bonus_wave(&self, slot: u32) -> Result<(), Refusal> {
        if slot >= self.players() {
            return Err(Refusal::NotAPlayer);
        }
        if self.run.phase != run::Phase::Mission || !self.run.briefing {
            return Err(Refusal::NoBonusWave);
        }
        if !self.bonus_wave_allowed() {
            return Err(Refusal::NoBonusWave);
        }
        if self.bonus_wave_forced() {
            return Err(Refusal::BonusWaveHeld);
        }
        Ok(())
    }

    /// Whether the site the ship is at may have a bonus wave: one with a
    /// fight to come — the machines or the Manufacturers hold it, not yet
    /// cleared, or a defence threatened — never the Machine Heart's or an
    /// Area defend's, and not with the run won.
    fn bonus_wave_allowed(&self) -> bool {
        let Some(id) = self.ship.state.alongside() else {
            return false;
        };
        if self.run.won || self.site_cleared(id) || heart::is_heart(id) {
            return false;
        }
        // A prison break and a salvage sweep keep the site's counted
        // waves (October 2026), so they may have one more.
        let attack = self.infestation(id).is_some_and(|it| it.heart.is_none())
            && matches!(
                self.mission_here(id),
                crate::run::Mission::Plain | crate::run::Mission::Prison | crate::run::Mission::Salvage
            );
        // Not at an Area defend nor a mission the map shapes, which run
        // on a clock rather than a count of waves.
        let defence = self.site_threatened(id)
            && !self.is_area_defense(id)
            && self.mission_here(id) == crate::run::Mission::Plain;
        attack || defence
    }

    /// A mission begun: with *Overtime* held (October 2026), the bonus
    /// wave chosen for the crew wherever one may be — in the ready check
    /// or without one.
    pub(crate) fn force_the_bonus_wave(&mut self) {
        if self.bonus_wave_forced() && self.bonus_wave_allowed() {
            self.run.bonus = BonusWave::Chosen;
        }
    }

    /// The bonus wave chosen, or taken back — see [`Command::BonusWave`]:
    /// one wave more at the end of the site's own when its count is
    /// settled, and every *Ready* pressed taken back, so the crew start
    /// the fight they all agreed to.
    pub(crate) fn choose_bonus_wave(&mut self, slot: u32, on: bool) -> Result<(), Refusal> {
        self.can_choose_bonus_wave(slot)?;
        self.run.bonus = if on {
            BonusWave::Chosen
        } else {
            BonusWave::None
        };
        for ready in &mut self.run.ready {
            *ready = false;
        }
        Ok(())
    }

    /// The site's last wave about to land: the bonus wave, when one was
    /// chosen.
    pub(crate) fn land_the_bonus_wave(&mut self) {
        if self.run.bonus == BonusWave::Chosen {
            self.run.bonus = BonusWave::Landed;
        }
    }

    /// The site cleared: the bonus wave, if it landed, fought.
    pub(crate) fn bonus_wave_fought(&mut self) {
        if self.run.bonus == BonusWave::Landed {
            self.run.bonus = BonusWave::Done;
        }
    }

    /// How many a wave of `n` is as it lands: [`data::BONUS_WAVE_SIZE_PERCENT`]
    /// of it, rounded up, for the bonus wave.
    pub(crate) fn bonus_wave_size(&self, n: u32) -> u32 {
        if self.run.bonus != BonusWave::Landed {
            return n;
        }
        (u64::from(n) * u64::from(data::BONUS_WAVE_SIZE_PERCENT))
            .div_ceil(100)
            .min(u64::from(u32::MAX)) as u32
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

    /// The site cleared: with *Clean Sweep*
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
