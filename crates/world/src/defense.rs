//! Defending a site (features 94 and 111).
//!
//! Every site that is neither a trader nor held by an enemy is
//! **threatened** from the first day of a run (task 111; until then only
//! a friendly town one hop outside the infection was). The first time the
//! crew arrive at one, a wave lands five seconds later and the fight is
//! on — and it is the one fight in the game that happens **inside** one
//! room rather than between two, since the site's own people, and the
//! armed defenders who stand with them (the area's,
//! `crate::droid::Area::defenders`), are in the residents' room with the
//! machines that came for them.
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
//! `Infestation::next_wave`. This one is
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
    /// the town and trade before the shooting starts.
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
    /// Whether the town fell instead: the crew leaving it with waves
    /// left, its system's day come while they were away, or an Area
    /// defend's FOB taken. Its own people all dead is no loss (October
    /// 2026).
    pub lost: bool,
    /// **Area defend** (October 2026): a town's defence is holding its FOB
    /// — `None` at a station or a derelict, and in a save from before.
    #[cfg_attr(feature = "serde", serde(default))]
    pub area: Option<Area>,
    /// **Seal the breaches** (October 2026, `crate::world` `missions.rs`):
    /// `None` at every other defence, and in a save from before.
    #[cfg_attr(feature = "serde", serde(default))]
    pub breaches: Option<Breaches>,
    /// **Evacuation** (October 2026, `crate::world` `evacuation.rs`):
    /// `None` at every other defence.
    #[cfg_attr(feature = "serde", serde(default))]
    pub evacuation: Option<Evacuation>,
    /// **Bomb disposal, Hold the doors, Protect the commander** (October
    /// 2026, `crate::world` `defences.rs`): `None` at every other defence.
    #[cfg_attr(feature = "serde", serde(default))]
    pub guard: Option<crate::objective::Guard>,
}

/// An Evacuation's state: how many of the site's people there were, which
/// of them (by body index in the site's room) are aboard the crew's ship,
/// and the flag — who of the crew carries it, or where on the crew's deck
/// it was dropped (room units, whole ones).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Evacuation {
    pub total: u32,
    pub aboard: Vec<u32>,
    pub holder: Option<u32>,
    pub at: (i32, i32),
}

/// A Seal the breaches fight's count of its ways in: how many it has, and
/// how many are not welded yet — said again every step of the fight
/// (`World::count_the_breaches`). The waves come while one is open.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Breaches {
    pub total: u32,
    pub open: u32,
}

/// An **Area defend**: the crew hold a ring of the town's ground, the FOB,
/// for [`data::AREA_HOLD_STEPS`] from the first wave while a wave lands
/// every [`data::AREA_WAVE_STEPS`] on the clock, the last down or not;
/// once the time is up, those on the ground are the last. The
/// machines take the FOB by standing in the ring
/// [`data::AREA_CAPTURE_STEPS`] with nobody of the crew's side in it.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Area {
    /// The FOB's middle, in the town's design units, whole ones.
    pub x: i32,
    pub y: i32,
    /// Steps of the hold still to run: [`data::AREA_HOLD_STEPS`] until the
    /// first wave lands, then counted down every step to nought.
    pub left: u64,
    /// Steps the enemies have stood in the ring uncontested: stopped while
    /// a friend stands in it too, back to nought when none of them does.
    pub held: u64,
    /// Whether the machines took the FOB: the run is lost.
    pub taken: bool,
    /// The sandbags round it, a tile each of the town's design: laid
    /// once, on the open ground of the ring's edge (`World::lay_fob`).
    #[cfg_attr(feature = "serde", serde(default))]
    pub bags: Vec<(u32, u32)>,
    /// Whether an enemy, and a friend, stood in the ring the last step:
    /// the picture's, worked out with the count.
    #[cfg_attr(feature = "serde", serde(default))]
    pub enemy_in: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    pub friend_in: bool,
    /// Whether the ring mended anybody the last step
    /// ([`data::AREA_HEAL_PERCENT`]): the picture's green ring, never
    /// saved or hashed — a world read back shows it from its next step.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub healing: bool,
}

impl Area {
    /// A ring about `(x, y)`, the clock full.
    pub fn new(x: i32, y: i32) -> Area {
        Area {
            x,
            y,
            left: data::AREA_HOLD_STEPS,
            held: 0,
            taken: false,
            bags: Vec::new(),
            enemy_in: false,
            friend_in: false,
            healing: false,
        }
    }

    /// One step of the ring: whether an enemy and a friend stand in it.
    /// True the step the FOB is taken, and only that step.
    pub fn watch(&mut self, enemy_in: bool, friend_in: bool) -> bool {
        self.enemy_in = enemy_in;
        self.friend_in = friend_in;
        if self.taken {
            return false;
        }
        if !enemy_in {
            self.held = 0;
        } else if !friend_in {
            self.held += 1;
        }
        if self.held >= data::AREA_CAPTURE_STEPS {
            self.taken = true;
            return true;
        }
        false
    }
}

/// How long after wave `wave` of an Area defend lands the next does, in
/// steps: [`data::AREA_WAVE_STEPS`] after each of the first
/// [`data::AREA_STEADY_WAVES`], then a second sooner a wave, never under
/// [`data::AREA_WAVE_MIN_STEPS`].
pub fn area_gap(wave: u32) -> u64 {
    let past = u64::from(wave.saturating_sub(data::AREA_STEADY_WAVES));
    data::AREA_WAVE_STEPS
        .saturating_sub(past * data::AREA_WAVE_SOONER_STEPS)
        .max(data::AREA_WAVE_MIN_STEPS)
}

/// How many waves land in an Area defend's hold: the first as it starts,
/// then one every [`area_gap`] while time is left — five in the three
/// minutes. What its experience is shared over (`World::price_the_wave`).
pub fn area_waves() -> u32 {
    let mut waves = 1;
    let mut at = 0;
    loop {
        at += area_gap(waves);
        if at >= data::AREA_HOLD_STEPS {
            return waves;
        }
        waves += 1;
    }
}

/// Where the `k`-th enemy of an Area defend makes for, in tiles off the
/// ring's middle: round it by the golden angle, from half a tile out to a
/// little over two, so a wave spreads over the middle of the ring rather
/// than standing on one tile — an attack order on the middle, spread a
/// little a body.
pub fn enemy_spot(k: usize) -> bims::math::Vec2 {
    const GOLDEN: f32 = 2.399_963;
    let reach = 0.5 + (k % 4) as f32 * 0.6;
    bims::math::Vec2::from_angle(k as f32 * GOLDEN) * reach
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
            area: None,
            breaches: None,
            evacuation: None,
            guard: None,
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

    /// Whether any wave is still to come after the one on the ground —
    /// at an Area defend, while its hold has time to run.
    pub fn more_to_come(&self) -> bool {
        if let Some(guard) = &self.guard {
            return guard.more_to_come() && !self.over();
        }
        match (&self.area, &self.breaches, &self.evacuation) {
            (Some(area), _, _) => area.left > 0,
            (None, Some(breaches), _) => breaches.open > 0,
            // An Evacuation's waves come until it is over.
            (None, None, Some(_)) => !self.over(),
            (None, None, None) => self.waves_left > 0,
        }
    }

    /// Whether the fight is over, either way.
    pub fn over(&self) -> bool {
        self.won || self.lost
    }
}

/// How many of a town's survivors join the crew: always
/// [`data::DEFENSE_JOINERS`] (two), however big the town — but never more
/// than the survivors **other than the guard**, so a town with nobody
/// left, or only the guard, sends none.
///
/// `survivors` counts the town's own people alive, the guard included.
pub fn joiners(survivors: u32, guard_alive: bool) -> u32 {
    let spare = survivors.saturating_sub(u32::from(guard_alive));
    data::DEFENSE_JOINERS.min(spare)
}

/// The seed a site's defender number `n` is kitted off: the station's
/// map seed and its place among them, kept apart from the residents'
/// (`map_seed ^ who`) by a salt of its own.
pub fn defender_seed(map_seed: u64, n: u32) -> u64 {
    map_seed ^ 0x_4445_4645_4e44_0000 ^ (n as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two, not more and not less, and the guard never counted among who
    /// actually goes.
    #[test]
    fn two_of_the_survivors_join_and_never_the_guard_alone() {
        // Nobody left, or only the guard: none.
        assert_eq!(joiners(0, false), 0);
        assert_eq!(joiners(1, true), 0);
        // One townsperson besides the guard: that one.
        assert_eq!(joiners(2, true), 1);
        assert_eq!(joiners(1, false), 1);
        // Two, however big the town.
        assert_eq!(data::DEFENSE_JOINERS, 2);
        assert_eq!(joiners(3, true), 2);
        assert_eq!(joiners(2, false), 2);
        assert_eq!(joiners(10, true), 2);
        assert_eq!(joiners(30, true), 2);
        // And never more than the survivors other than the guard.
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

    /// The ring: an enemy alone counts up, a friend stops the count, the
    /// enemies gone put it back, and the full count takes the FOB once.
    #[test]
    fn the_ring_is_taken_by_enemies_standing_in_it_alone() {
        let mut a = Area::new(0, 0);
        for _ in 0..10 {
            assert!(!a.watch(true, false));
        }
        assert_eq!(a.held, 10);
        assert!(!a.watch(true, true));
        assert_eq!(a.held, 10, "a friend in the ring stops the count");
        assert!(!a.watch(false, false));
        assert_eq!(a.held, 0, "nobody of theirs in it puts it back");
        let mut taken = 0;
        for _ in 0..data::AREA_CAPTURE_STEPS + 5 {
            taken += u32::from(a.watch(true, false));
        }
        assert_eq!(taken, 1);
        assert!(a.taken);
    }

    /// Forty-five seconds after the first wave, then a second sooner a
    /// wave, never under five; five waves in the three minutes.
    #[test]
    fn the_waves_come_forty_five_seconds_apart_then_a_second_sooner_a_wave() {
        let seconds: Vec<u64> = (1..=5).map(|w| area_gap(w) / 60).collect();
        assert_eq!(seconds, [45, 44, 43, 42, 41]);
        assert_eq!(area_waves(), 5);
        assert_eq!(area_gap(41) / 60, 5);
        assert_eq!(area_gap(400) / 60, 5);
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
