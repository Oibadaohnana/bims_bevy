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
//!   (`droid::MissionWeights`, `scaling.ron`'s `missions`), at least one.
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
    /// state** where there is one: a defence's FOB, breaches, people to
    /// evacuate or guarded thing (`Defense::{area, breaches, evacuation,
    /// guard}`), an attack's charge or nests (`Infestation::{sabotage,
    /// nests}`) or its objective of the second set — and, for those five,
    /// before the objective is laid after the garrison, the mission the
    /// site is (`World::mission_here`). `None` once the defence is over or
    /// the site cleared, at the Machine Heart, under the probes' forced
    /// kinds, at an elite's fight and at a plain one.
    pub(crate) fn budgeted_here(&self, id: u32) -> Option<Budgeted> {
        use crate::objective::{Guard, Objective};
        use crate::run::Mission;
        if heart::is_heart(id) || self.droid_kinds_forced.is_some() || self.is_elite_here(id) {
            return None;
        }
        if let Some(d) = self.defense(id).filter(|d| !d.over()) {
            return if d.area.is_some() {
                Some(Budgeted::AreaDefend)
            } else if d.breaches.is_some() {
                Some(Budgeted::Breaches)
            } else if d.evacuation.is_some() {
                Some(Budgeted::Evacuation)
            } else {
                d.guard.as_ref().map(|g| match g {
                    Guard::Bombs(_) => Budgeted::Bombs,
                    Guard::Doors(_) => Budgeted::Doors,
                    Guard::Chief(_) => Budgeted::Chief,
                })
            };
        }
        let it = self.infestation(id)?;
        if it.heart.is_some() || it.cleared {
            return None;
        }
        if it.sabotage.is_some() {
            return Some(Budgeted::Sabotage);
        }
        if it.nests.is_some() {
            return Some(Budgeted::Nests);
        }
        if let Some(o) = &it.objective {
            return Some(match o {
                Objective::Overseer(_) => Budgeted::Overseer,
                Objective::Heist(_) => Budgeted::Heist,
                Objective::Prison(_) => Budgeted::Prison,
                Objective::FuelRun(_) => Budgeted::FuelRun,
                Objective::Salvage(_) => Budgeted::Salvage,
            });
        }
        match self.mission_here(id) {
            Mission::Overseer => Some(Budgeted::Overseer),
            Mission::Heist => Some(Budgeted::Heist),
            Mission::Prison => Some(Budgeted::Prison),
            Mission::FuelRun => Some(Budgeted::FuelRun),
            Mission::Salvage => Some(Budgeted::Salvage),
            _ => None,
        }
    }

    /// The per cent of the area's budget the fight `of` brings: its weight
    /// in the scaling (`droid::MissionWeights`, `scaling.ron`'s
    /// `missions`).
    pub(crate) fn budget_weight_percent(&self, of: Budgeted) -> u64 {
        self.scaling().missions.percent(of)
    }

    /// Every landing a crew at par meet in the fight `of`, the first one
    /// too: what [`World::budget_even`] slices the budget over. Sabotage
    /// its garrison and a landing every `SABOTAGE_WAVE_STEPS` of the hold
    /// and the escape; Hold the doors and Protect the commander a landing
    /// every gap of their clock; an Area defend the waves of its hold; an
    /// Evacuation, Bomb disposal and the clocked attacks so many par waves;
    /// a salvage sweep its garrison and a wave a crate laid; Seal the
    /// breaches its breaches (its own slice reads them); a nest hunt one —
    /// its rounds slice the wave, not the landings.
    pub(crate) fn budget_expected(&self, of: Budgeted) -> u32 {
        use crate::objective::Objective;
        let here = self
            .residents
            .as_ref()
            .map(|r| r.station)
            .or_else(|| self.ship.state.alongside());
        let steps = |total: u64, gap: u64| total.div_ceil(gap.max(1)) as u32;
        match of {
            Budgeted::Sabotage => {
                1 + steps(
                    data::SABOTAGE_HOLD_STEPS + data::SABOTAGE_ESCAPE_STEPS,
                    data::SABOTAGE_WAVE_STEPS,
                )
            }
            Budgeted::Doors => steps(data::DOORS_STEPS, data::DOORS_WAVE_STEPS),
            Budgeted::Chief => steps(data::CHIEF_STEPS, data::CHIEF_WAVE_STEPS),
            Budgeted::AreaDefend => defense::area_waves(),
            Budgeted::Evacuation => data::EVAC_PAR_WAVES,
            Budgeted::Bombs => data::BOMB_PAR_WAVES,
            Budgeted::Overseer | Budgeted::Heist | Budgeted::FuelRun | Budgeted::Prison => {
                data::OBJECTIVE_WAVES
            }
            Budgeted::Salvage => {
                let crates = here
                    .and_then(|id| self.infestation(id))
                    .and_then(|it| match &it.objective {
                        Some(Objective::Salvage(s)) => Some(s.crates.len() as u32),
                        _ => None,
                    })
                    .unwrap_or(data::SALVAGE_CRATES);
                1 + crates
            }
            Budgeted::Breaches => here
                .and_then(|id| self.defense(id))
                .and_then(|d| d.breaches)
                .map_or(1, |b| b.total.max(1)),
            Budgeted::Nests => 1,
        }
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
        let planned =
            ((u64::from(wave) * area.budget_hundredths() * self.budget_weight_percent(of) + 5_000)
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
    /// first one too. (The missions on a clock slice by it; the nests do
    /// not.)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::Mission;
    use bims::combat::{Gear, WeaponKind};
    use shipdesign::fixture::{COMBAT_CREW, combat_ship};
    use worldgen::GalaxyType;

    /// The combat ship's crew docked at the arena, the machines holding
    /// it, the mission `mission` forced and three waves to the site's own
    /// count — `tests_droid`'s arena, made by hand.
    fn held_arena(mission: Mission) -> (World, u32) {
        let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
        let (star, station) = crate::spawn(&galaxy).unwrap();
        let mut world = World::start_with_crew(
            combat_ship(),
            crate::fixture::REFERENCE_MONEY,
            1,
            COMBAT_CREW,
            data::DEFAULT_SEED,
            GalaxyType::SpiralTwoArm,
            star,
            station,
        )
        .unwrap();
        world.set_machines_only_for_probe();
        world.arena_dock_for_probe();
        let crew = world.aboard.room.crew_count() as usize;
        for (who, kind) in WeaponKind::ALL
            .iter()
            .copied()
            .cycle()
            .take(crew)
            .enumerate()
        {
            let gear = world.aboard.room.gear(who);
            world.aboard.room.issue(
                who,
                Gear {
                    weapon: Some(kind.basic()),
                    ..gear
                },
            );
        }
        world.infest(station);
        world.set_mission_for_probe(Some(mission));
        world.set_droid_waves_for_probe(3);
        // Up to the first dock settled, and no further.
        for _ in 0..40 {
            world.step(&[]);
            if world.infestation(station).is_some_and(|it| it.settled) {
                return (world, station);
            }
        }
        panic!("the site was never settled");
    }

    /// Every fight's landings at par, the first included (October 2026,
    /// the spec's numbers): Sabotage its garrison and eight, Hold the
    /// doors eight, Protect the commander five, an Area defend five, an
    /// Evacuation four, Bomb disposal three, the clocked attacks and a
    /// prison break four, a salvage sweep its garrison and a wave a crate
    /// (six where none is laid yet), Seal the breaches one with none, a
    /// nest hunt one.
    #[test]
    fn the_budget_s_landings_expected_are_each_fight_s_par() {
        let world = crate::fixture::simulation_world(combat_ship(), 0, 1);
        for (of, n) in [
            (Budgeted::Sabotage, 9),
            (Budgeted::Doors, 8),
            (Budgeted::Chief, 5),
            (Budgeted::AreaDefend, 5),
            (Budgeted::Evacuation, 4),
            (Budgeted::Bombs, 3),
            (Budgeted::Overseer, 4),
            (Budgeted::Heist, 4),
            (Budgeted::FuelRun, 4),
            (Budgeted::Prison, 4),
            (Budgeted::Salvage, 7),
            (Budgeted::Breaches, 1),
            (Budgeted::Nests, 1),
        ] {
            assert_eq!(world.budget_expected(of), n, "{of:?}");
        }
    }

    /// The weight a fight's budget is multiplied by is the scaling's, read
    /// live: nine tenths at a Sabotage built in, a half where the file
    /// says so.
    #[test]
    fn the_budget_s_weight_is_the_scaling_s() {
        let mut world = crate::fixture::simulation_world(combat_ship(), 0, 1);
        assert_eq!(world.budget_weight_percent(Budgeted::Sabotage), 90);
        assert_eq!(world.budget_weight_percent(Budgeted::Heist), 100);
        let mut scaling = world.wave_scaling();
        scaling.missions.heist = 0.5;
        world.set_wave_scaling(scaling);
        assert_eq!(world.budget_weight_percent(Budgeted::Heist), 50);
    }

    /// A fight with a budget settles one wave, its garrison, and opens
    /// the budget on it; a plain one keeps the site's count.
    #[test]
    fn a_budgeted_attack_settles_one_wave_and_a_plain_one_its_count() {
        for (mission, of) in [
            (Mission::Sabotage, Some(Budgeted::Sabotage)),
            (Mission::Heist, Some(Budgeted::Heist)),
            (Mission::Plain, None),
        ] {
            let (world, station) = held_arena(mission);
            let it = world.infestation(station).unwrap();
            assert_eq!(world.budgeted_here(station), of, "{mission:?}");
            assert_eq!(world.run.budget.map(|b| b.of), of, "{mission:?}");
            let left = if of.is_some() { 0 } else { 2 };
            assert_eq!((it.wave, it.waves_left), (1, left), "{mission:?}");
        }
    }
}
