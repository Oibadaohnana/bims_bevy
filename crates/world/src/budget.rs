//! **A mission's enemy budget** (October 2026, the player's: "give every
//! mission the same total … let each mission spend that total in its own
//! way"). A fight that slices one ([`run::Budgeted`]) brings
//! `budget × wave` enemies over the whole of it, whatever its clock, its
//! objective or its luck:
//!
//! - **The budget opens the step the mission's wave freezes**
//!   ([`World::open_the_budget`], from the first `landing_wave_size`): the
//!   frozen wave (base bodies, the area's Bombers and Lancers beside it),
//!   and `planned` — the wave times the area's budget
//!   (`droid::Area::budget`, read through `World::area_now` so the Heart
//!   and a probe's tier read the right area) times the mission's weight
//!   (every one a hundred per cent until the weights come), at least one.
//! - **The mission is priced once, there**: every enemy of it pays the
//!   site's experience budget over `planned` and the extras it brings
//!   (`Run::xp_each`), so a mission that lands its bodies in many slices
//!   pays what one that lands them at once does. Nothing reprices it.
//! - **Each landing takes its slice** ([`World::budget_take`]): as many
//!   base bodies as it asks, never past what is left, and the extras'
//!   cumulative share of them, so the extras come in step with the
//!   bodies. [`World::budget_even`] is the even share of what is left over
//!   the landings still to come.
//! - **Spent, only a trickle comes**: one a player a landing, no extras,
//!   every body marked **unpaid** (`bims::droid::Droid::unpaid`,
//!   `bims::bim::Bim::unpaid`) — down, it pays nothing. Being slow still
//!   costs something; it cannot snowball.
//!
//! A nest hunt (the first fight on it) takes its garrison as the first
//! slice and each round of its nests as one more, a third of a wave
//! between them a round (`World::nest_build`), the extras the garrison's
//! alone ([`World::budget_take_plain`]).
//!
//! The budget is the run's ([`run::Budget`] on `Run::budget`), saved and
//! hashed where there is one, cleared at every mission's start and end. A
//! child of `crate::world`, as `mission.rs` is. **Nothing here draws from a
//! stream.**

use super::*;
use crate::run::{Budget, Budgeted};

pub use crate::run::Landing;

impl World {
    /// What the fight at `id` slices its budget over, read off its **laid
    /// state**: a nest hunt's nests. `None` at the Machine Heart, under the
    /// probes' forced kinds, at an elite's fight and at a plain one.
    pub(crate) fn budgeted_here(&self, id: u32) -> Option<Budgeted> {
        if heart::is_heart(id) || self.droid_kinds_forced.is_some() || self.is_elite_here(id) {
            return None;
        }
        let it = self.infestation(id)?;
        if it.heart.is_some() || it.cleared {
            return None;
        }
        it.nests.is_some().then_some(Budgeted::Nests)
    }

    /// The per cent of the area's budget the fight `of` brings: a hundred
    /// for every one, until the weights come.
    pub(crate) fn budget_weight_percent(&self, of: Budgeted) -> u64 {
        let _ = of;
        100
    }

    /// The budget opened, the step the mission's `wave` froze, where the
    /// fight at `id` slices one; and the mission priced once over it. See
    /// the module note.
    pub(crate) fn open_the_budget(&mut self, id: u32, wave: u32) {
        let Some(of) = self.budgeted_here(id) else {
            return;
        };
        let wave = wave.max(1);
        let (index, area) = self.area_now();
        // The extras as `wave_kinds_for` would put them on top of a wave.
        let (bombers, lancers) = if index >= 1 && self.droid_kinds_forced.is_none() {
            (area.bombers, area.lancers)
        } else {
            (0, 0)
        };
        let planned = ((u64::from(wave) * area.budget_hundredths() * self.budget_weight_percent(of)
            + 5_000)
            / 10_000)
            .clamp(1, u64::from(u32::MAX)) as u32;
        let share = |extra: u32, of: u32| u64::from(extra) * u64::from(of) / u64::from(wave);
        let extras = match of {
            // A nest builds none: the garrison's are the hunt's.
            Budgeted::Nests => {
                let garrison = wave.min(planned);
                share(bombers, garrison) + share(lancers, garrison)
            }
            _ => share(bombers, planned) + share(lancers, planned),
        };
        let bodies = (u64::from(planned) + extras).max(1);
        let each = ((self.site_budget(id) + bodies / 2) / bodies).max(1);
        self.run.xp_each = Some(each.min(u64::from(u32::MAX)) as u32);
        self.run.budget = Some(Budget {
            of,
            wave,
            bombers,
            lancers,
            planned,
            laid: 0,
            bombers_laid: 0,
            lancers_laid: 0,
            landings: 0,
            garrison: 0,
        });
    }

    /// What is left of the mission's budget, in base bodies; nought with
    /// none.
    pub(crate) fn budget_left(&self) -> u32 {
        self.run
            .budget
            .map_or(0, |b| b.planned.saturating_sub(b.laid))
    }

    /// The even share of what is left over the landings still to come,
    /// rounded up, `expected` counting every landing of the mission, the
    /// first one too.
    pub(crate) fn budget_even(&self, expected: u32) -> u32 {
        let Some(b) = self.run.budget else {
            return 0;
        };
        let to_come = expected.saturating_sub(b.landings).max(1);
        self.budget_left().div_ceil(to_come)
    }

    /// The landing about to be laid: `want` base bodies, never past what is
    /// left, with the extras' share of them — or, with nothing left, the
    /// trickle. See the module note.
    pub(crate) fn budget_take(&mut self, want: u32) -> Landing {
        self.take_from_the_budget(want, true)
    }

    /// [`World::budget_take`] laying no extras and counting none: a nest
    /// builds its bodies bare.
    pub(crate) fn budget_take_plain(&mut self, want: u32) -> Landing {
        self.take_from_the_budget(want, false)
    }

    fn take_from_the_budget(&mut self, want: u32, extras: bool) -> Landing {
        let left = self.budget_left();
        let players = self.players().max(1);
        let Some(b) = self.run.budget.as_mut() else {
            return Landing {
                base: want.max(1),
                bombers: 0,
                lancers: 0,
                unpaid: false,
            };
        };
        let first = b.landings == 0;
        b.landings = b.landings.saturating_add(1);
        let landing = if left == 0 {
            // The trickle: one a player, nothing on top, paying nothing.
            Landing {
                base: players,
                bombers: 0,
                lancers: 0,
                unpaid: true,
            }
        } else {
            let base = want.max(1).min(left);
            b.laid += base;
            let (bombers, lancers) = if extras {
                let due = |extra: u32| {
                    (u64::from(extra) * u64::from(b.laid) / u64::from(b.wave.max(1))) as u32
                };
                let bombers = due(b.bombers).saturating_sub(b.bombers_laid);
                let lancers = due(b.lancers).saturating_sub(b.lancers_laid);
                b.bombers_laid += bombers;
                b.lancers_laid += lancers;
                (bombers, lancers)
            } else {
                (0, 0)
            };
            Landing {
                base,
                bombers,
                lancers,
                unpaid: false,
            }
        };
        if first {
            b.garrison = landing.base;
        }
        landing
    }
}
