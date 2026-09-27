//! Defending a site (features 94 and 111).
//!
//! Every site that is neither a trader nor held by an enemy is
//! **threatened** from the first day of a run (task 111; until then only
//! a friendly town one hop outside the infection was). The first time the
//! crew arrive at one, a wave lands twenty seconds later and the fight is
//! on — and it is the one fight in the game that happens **inside** one
//! room rather than between two, since the site's own people, and the
//! armed [`defenders`] who stand with them, are in the residents' room
//! with the machines that came for them.
//!
//! What is kept here is the **schedule**, and it is [`Defense`]: which
//! wave is on the ground, how many are still to come, and how long until
//! the next lands. The wave count and the wave size are the droid step's
//! own formulas ([`crate::droid`]); the room's side of the fight is
//! `bims::game`'s machine target list and its sheltering list.
//!
//! # The clock is minutes left, not a minute of the clock
//!
//! Every other countdown in the world is an absolute clock reading —
//! `Infestation::next_wave`, a hire's month. This one is
//! **how long there still is to wait**, counted down only while the crew
//! are standing in the town, because taking off **pauses** the attack and
//! landing again resumes it where it stood. A clock reading would have
//! the whole wave schedule run while the ship was away and the crew come
//! back to a town that had been overrun without them.

use crate::data;

/// One town's attack, as the world keeps it. Saved, and in
/// `world_checksum`: it is the size and the state of a fight, and two
/// clients have to agree about it.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Defense {
    /// The town's station id (`crate::surface::surface_id`).
    pub station: u32,
    /// How many waves are still to arrive after the one on the ground.
    /// Worked out once, when the crew first land, and only counted down.
    pub waves_left: u32,
    /// Which wave is on the ground, counting from one; **nought before
    /// the first has landed**, which is the hour the crew have to walk
    /// the town, trade and hire before the shooting starts.
    pub wave: u32,
    /// Steps of the mission clock until the next wave lands (feature 103)
    /// — `None` while a machine is still standing, and with none left to
    /// come. Counted down only while the crew are here: see the module
    /// note.
    pub next_in: Option<u64>,
    /// How many machines of the wave on the ground were still standing
    /// when the world last looked. It is what a take-off leaves behind
    /// and a landing puts back: a town's room is built afresh at every
    /// join, so the wave has to be laid again, and laying it again at
    /// full strength would be a fight the crew could never win by
    /// leaving and coming back — or lose by it.
    pub standing: u32,
    /// Whether the wave count has been worked out yet.
    pub settled: bool,
    /// Whether the last machine of the last wave has been destroyed —
    /// said once, and what puts the town on [`crate::World::held_towns`].
    pub won: bool,
    /// Whether the town fell instead: every one of its people dead, or
    /// its system's day come while the crew were away with waves left.
    pub lost: bool,
}

impl Defense {
    /// A town the crew have just landed at, threatened and not attacked
    /// before: the first wave is [`data::DEFENSE_DELAY_STEPS`] off and
    /// nothing has been worked out yet.
    pub fn new(station: u32) -> Defense {
        Defense {
            station,
            waves_left: 0,
            wave: 0,
            next_in: Some(data::DEFENSE_DELAY_STEPS),
            standing: 0,
            settled: false,
            won: false,
            lost: false,
        }
    }

    /// The wave count is fixed now and never worked out again. `waves`
    /// counts the first, which has not landed yet, so what is *left*
    /// after it is one fewer.
    pub fn settle(&mut self, waves: u32) {
        if self.settled {
            return;
        }
        self.settled = true;
        self.waves_left = waves.saturating_sub(1);
    }

    /// Whether any wave is still to come after the one on the ground.
    pub fn more_to_come(&self) -> bool {
        self.waves_left > 0
    }

    /// Whether the fight is over, either way.
    pub fn over(&self) -> bool {
        self.won || self.lost
    }
}

/// How many of a town's survivors join the crew: the larger of one and
/// `survivors × DEFENSE_JOIN_PERCENT / 100` rounded down, but never more
/// than the survivors **other than the guard** — so a town with nobody
/// left, or only the guard, sends none.
///
/// `survivors` counts the town's own people alive, the guard included;
/// mercenaries are none of it, being nobody's townsfolk.
pub fn joiners(survivors: u32, guard_alive: bool) -> u32 {
    let spare = survivors.saturating_sub(u32::from(guard_alive));
    if spare == 0 {
        return 0;
    }
    let share = survivors * data::DEFENSE_JOIN_PERCENT / 100;
    share.max(1).min(spare)
}

/// How many armed **defenders** stand with a site's own people while the
/// machines come for it (task 111): [`data::DEFENDERS_BASE`] and one more
/// every [`data::DEFENDER_DAYS`] of the world clock, never more than
/// [`data::DEFENDERS_MAX`]. A pure function of the day, so nothing about
/// it is saved: `World::defenders_of` asks it when the room opens.
pub fn defenders(days_gone: u32) -> u32 {
    let more = days_gone / data::DEFENDER_DAYS.max(1);
    data::DEFENDERS_BASE
        .saturating_add(more)
        .min(data::DEFENDERS_MAX)
}

/// The seed a site's defender number `n` is kitted off: the station's
/// map seed and its place among them, kept apart from the residents'
/// (`map_seed ^ who`) and the mercenaries' (`mercenary::seed_for`) by a
/// salt of its own.
pub fn defender_seed(map_seed: u64, n: u32) -> u64 {
    map_seed ^ 0x_4445_4645_4e44_0000 ^ (n as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two on day nought, one more every spread's worth of days, and
    /// never past the cap.
    #[test]
    fn defenders_grow_with_the_days_and_stop_at_the_cap() {
        assert_eq!(defenders(0), data::DEFENDERS_BASE);
        assert_eq!(defenders(data::DEFENDER_DAYS - 1), data::DEFENDERS_BASE);
        assert_eq!(defenders(data::DEFENDER_DAYS), data::DEFENDERS_BASE + 1);
        assert_eq!(defenders(u32::MAX), data::DEFENDERS_MAX);
        let mut last = 0;
        for day in 0..400 {
            let n = defenders(day);
            assert!(n >= last && n <= data::DEFENDERS_MAX, "{day}");
            last = n;
        }
        assert_ne!(defender_seed(7, 0), crate::mercenary::seed_for(7, 0));
    }

    /// The share the feature asked for, the floor of one, and the guard
    /// never counted among who actually goes.
    #[test]
    fn a_fifth_of_the_survivors_join_and_never_the_guard_alone() {
        // Nobody left, or only the guard: none.
        assert_eq!(joiners(0, false), 0);
        assert_eq!(joiners(1, true), 0);
        // One townsperson besides the guard: the floor of one.
        assert_eq!(joiners(2, true), 1);
        assert_eq!(joiners(1, false), 1);
        // A fifth, rounded down, once there is a fifth to have.
        assert_eq!(joiners(10, true), 2);
        assert_eq!(joiners(14, true), 2);
        assert_eq!(joiners(15, true), 3);
        assert_eq!(joiners(30, true), 6);
        // And never more than the survivors other than the guard.
        assert_eq!(joiners(6, true), 1);
        assert_eq!(joiners(6, false), 1);
        for survivors in 0..60u32 {
            for guard in [false, true] {
                let n = joiners(survivors, guard);
                assert!(
                    n <= survivors.saturating_sub(u32::from(guard)),
                    "{survivors}"
                );
            }
        }
    }

    /// The schedule: an hour before the first wave, the count fixed once.
    #[test]
    fn the_first_wave_is_an_hour_off_and_the_count_is_fixed_once() {
        let mut d = Defense::new(7);
        assert_eq!(d.wave, 0);
        assert_eq!(d.next_in, Some(data::DEFENSE_DELAY_STEPS));
        assert!(!d.settled && !d.over());
        d.settle(3);
        assert_eq!(d.waves_left, 2);
        d.settle(9);
        assert_eq!(d.waves_left, 2, "settled once and never again");
        assert!(d.more_to_come());
        d.waves_left = 0;
        assert!(!d.more_to_come());
    }
}
