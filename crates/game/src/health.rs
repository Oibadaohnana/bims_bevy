//! A body's health: one bar of hit points, and what happens at the
//! bottom of it (task 120).
//!
//! # One bar
//!
//! A Bim has [`MAX_HEALTH`] hit points and nothing else — no blood, no
//! wounds, no traumas, no parts with health of their own. A hit lands on
//! the body and nowhere in particular (October 2026: it used to roll a
//! head, a body or legs, for which piece of armour took it); the armour
//! takes it first (`Game::strike`), and whatever gets past comes off the
//! one bar ([`Health::hit`]). Nothing mends on its own: a medic's beam and a
//! relic put hit points back ([`Health::heal`]), and a mission's start
//! fills the bar ([`Health::restore`]).
//!
//! # Downed, and the countdown
//!
//! At nothing a Bim is **downed** ([`Health::downed`]): it lies where it
//! fell, does nothing, and nothing aims at it, and a countdown of
//! [`DOWNED_SECONDS`] starts ([`Health::down_left`]) — run down by
//! [`Health::update`] with the room's own steps, so a paused game holds
//! it. At nought it is **dead**. Another Bim standing beside it for long
//! enough ([`crate::task::Kind::Revive`]) brings it round
//! ([`Health::revive`]) at [`REVIVED_TO`] of the bar. It walks at its
//! whole pace after: the thirty per cent slow it kept for the rest of the
//! mission went in October 2026 (the player's word: no hidden slowdowns),
//! and [`Health::was_downed`] is only remembered.

/// A whole bar.
pub const MAX_HEALTH: f32 = 100.0;

/// What a bar read from a save that knew nothing of items is: whole.
#[cfg(feature = "serde")]
fn whole_bar() -> f32 {
    MAX_HEALTH
}

/// How long a downed body lies before it is dead, in seconds of the
/// room's steps: 1 800 steps at 1× (task 120).
pub const DOWNED_SECONDS: f32 = 30.0;

/// Where a revived body's bar starts again from: three tenths of it.
pub const REVIVED_TO: f32 = 0.3;

/// What the countdown may be short of nought and still be over, in
/// seconds: far under a step, and far over what 1 800 sixtieths of a second
/// added up in floats miss nought by.
const COUNTDOWN_SLACK: f32 = 1.0e-3;

/// Under this many hit points a body bleeds on the deck (`Bim::tick_drips`),
/// downed or not.
pub const BLEEDS_UNDER: f32 = 20.0;

/// How long a revive takes with nothing to speed it, in seconds: the
/// time a helper stands beside a downed crewmate before it is up. A
/// medic's and a relic's are the world's (`world::data`), handed to the
/// room on the helper's `Skill::revive`.
pub const REVIVE_SECONDS: f32 = 10.0;

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Health {
    /// The bar, nought to [`Health::max`].
    points: f32,
    /// A whole bar: [`MAX_HEALTH`], and more with a *Reactor Heart*
    /// carried (October 2026, `Gear::max_health`), said by the room off
    /// the loadout.
    #[cfg_attr(feature = "serde", serde(default = "whole_bar"))]
    max: f32,
    /// Seconds left before a downed body is dead; `None` while it is up,
    /// and once it is dead.
    down_left: Option<f32>,
    dead: bool,
    /// Downed at least once since the bar was last filled. It slowed the
    /// walk until October 2026; now only remembered (saved and hashed).
    was_downed: bool,
}

impl Default for Health {
    fn default() -> Health {
        Health::new()
    }
}

impl Health {
    pub fn new() -> Health {
        Health {
            points: MAX_HEALTH,
            max: MAX_HEALTH,
            down_left: None,
            dead: false,
            was_downed: false,
        }
    }

    /// The bar: what the panel shows.
    pub fn points(&self) -> f32 {
        self.points
    }

    /// A whole bar for this body (October 2026: a *Reactor Heart* raises
    /// it).
    pub fn max(&self) -> f32 {
        self.max
    }

    /// A whole bar of `max` from now on, what is in it kept at the same
    /// share of the bar, as Dota keeps it — a body at full stays at full
    /// with an item put on. Nothing for the dead or a body down.
    pub fn set_max(&mut self, max: f32) {
        let max = max.max(1.0);
        if max == self.max {
            return;
        }
        if !self.dead && self.down_left.is_none() {
            self.points = (self.points * max / self.max).clamp(0.0, max);
        } else {
            self.points = self.points.min(max);
        }
        self.max = max;
    }

    pub fn is_dead(&self) -> bool {
        self.dead
    }

    /// At nothing and not yet dead: lying on the deck with the countdown
    /// running.
    pub fn downed(&self) -> bool {
        !self.dead && self.down_left.is_some()
    }

    /// The seconds a downed body has left, `None` for one that is up or
    /// dead.
    pub fn down_left(&self) -> Option<f32> {
        if self.dead { None } else { self.down_left }
    }

    /// Whether it has been downed since the bar was last filled.
    pub fn was_downed(&self) -> bool {
        self.was_downed
    }

    /// Short of a whole bar, and alive.
    pub fn is_hurt(&self) -> bool {
        !self.dead && self.points < self.max
    }

    /// Whether it bleeds on the deck: alive — downed or up — and under
    /// [`BLEEDS_UNDER`].
    pub fn bleeds(&self) -> bool {
        !self.dead && self.points < BLEEDS_UNDER
    }

    /// A hit of `damage` on the bar — whatever got past the armour. How
    /// much came off it: nothing on a body already downed or dead, which
    /// nothing aims at. Reaching nothing is downed, and the countdown
    /// starts.
    pub fn hit(&mut self, damage: f32) -> f32 {
        if self.dead || self.down_left.is_some() || damage <= 0.0 {
            return 0.0;
        }
        let taken = damage.min(self.points);
        self.points -= taken;
        if self.points <= 0.0 {
            self.points = 0.0;
            self.down_left = Some(DOWNED_SECONDS);
        }
        taken
    }

    /// Brought round where it lies: up again at [`REVIVED_TO`] of the bar,
    /// the countdown over, and remembered as downed this mission.
    /// Whether it was downed to be revived.
    pub fn revive(&mut self) -> bool {
        self.revive_at(REVIVED_TO)
    }

    /// [`Health::revive`] at `share` of the bar — a relic's *Second Wind*
    /// (feature 106) brings a body round a little higher.
    pub fn revive_at(&mut self, share: f32) -> bool {
        if !self.downed() {
            return false;
        }
        self.down_left = None;
        self.points = (self.max * share).clamp(1.0, self.max);
        self.was_downed = true;
        true
    }

    /// `points` of health put back at once — a medic's beam, a relic —
    /// never past a whole bar, and nothing to a body downed or dead: only
    /// a revive gets one of those up. How much went in.
    pub fn heal(&mut self, points: f32) -> f32 {
        if self.dead || self.down_left.is_some() || points <= 0.0 {
            return 0.0;
        }
        let given = points.min(self.max - self.points).max(0.0);
        self.points += given;
        given
    }

    /// A whole bar again, and the downing forgotten: a
    /// mission's start. Nothing for the dead.
    pub fn restore(&mut self) {
        if self.dead {
            return;
        }
        self.points = self.max;
        self.down_left = None;
        self.was_downed = false;
    }

    /// The downing forgotten, the bar as it is: a
    /// mission's end.
    pub fn forget_downed(&mut self) {
        self.was_downed = false;
    }

    /// The end of it, at once: dead at the top of the next tick. What
    /// finishes a body the world says is dead (`Game::kill_now`, a grave
    /// laid out).
    pub fn give_up(&mut self) {
        self.points = 0.0;
        self.down_left = None;
        self.dead = true;
    }

    /// Back from the dead with a whole bar: a player's Bim respawning at
    /// its mission's end (`Game::revive`).
    pub fn respawn(&mut self) {
        let max = self.max;
        *self = Health::new();
        self.max = max;
        self.points = max;
    }

    /// `seconds` of the room's own steps: the countdown of a downed body.
    /// Nothing else runs on its own — there is no mending.
    pub fn update(&mut self, seconds: f32) {
        if self.dead {
            return;
        }
        if let Some(left) = self.down_left.as_mut() {
            *left -= seconds;
            if *left <= COUNTDOWN_SLACK {
                self.down_left = None;
                self.dead = true;
            }
        }
    }

    /// Set the bar outright, for a probe: nought is downed.
    #[allow(dead_code)]
    pub fn set_points_for_probe(&mut self, points: f32) {
        self.points = points.clamp(0.0, self.max);
        self.dead = false;
        self.down_left = (self.points <= 0.0).then_some(DOWNED_SECONDS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Nought is downed, not dead**, and a downed body takes nothing
    /// more.
    #[test]
    fn hp_at_nothing_is_downed() {
        let mut h = Health::new();
        assert_eq!(h.hit(30.0), 30.0);
        assert_eq!(h.points(), 70.0);
        assert!(!h.downed());
        assert_eq!(h.hit(500.0), 70.0, "only what was left comes off");
        assert!(h.downed());
        assert!(!h.is_dead());
        assert_eq!(h.down_left(), Some(DOWNED_SECONDS));
        assert_eq!(h.hit(10.0), 0.0, "nothing more comes off a downed body");
        assert_eq!(h.heal(50.0), 0.0, "and nothing heals it up");
        assert!(h.downed());
    }

    /// **The countdown is 1 800 steps at 1×**, and nothing runs it but
    /// the steps — a paused room steps nothing.
    #[test]
    fn a_downed_body_dies_after_eighteen_hundred_steps_and_waits_while_paused() {
        let mut h = Health::new();
        h.hit(MAX_HEALTH);
        let step = 1.0 / 60.0;
        for _ in 0..1_799 {
            h.update(step);
        }
        assert!(h.downed(), "one step short");
        // A pause is no steps at all: however long, nothing moves.
        let held = h.down_left();
        h.update(0.0);
        assert_eq!(h.down_left(), held);
        h.update(step);
        assert!(h.is_dead(), "{:?}", h.down_left());
        assert!(!h.downed());
    }

    /// **A revive is three tenths of the bar**, remembered as a downing
    /// (no slow since October 2026), and a mission's start takes both away.
    #[test]
    fn a_revived_body_is_up_at_thirty_and_remembered_until_the_bar_is_filled() {
        let mut h = Health::new();
        assert!(!h.was_downed());
        assert!(!h.revive(), "nothing to revive");
        h.hit(MAX_HEALTH);
        assert!(h.revive());
        assert_eq!(h.points(), MAX_HEALTH * REVIVED_TO);
        assert!(!h.downed());
        assert!(h.was_downed());
        // Down again and up again: still once.
        h.hit(MAX_HEALTH);
        h.revive();
        assert!(h.was_downed());
        h.restore();
        assert_eq!(h.points(), MAX_HEALTH);
        assert!(!h.was_downed());
        // And a mission's end forgets it where the bar stands.
        h.hit(MAX_HEALTH);
        h.revive();
        h.forget_downed();
        assert!(!h.was_downed());
        assert_eq!(h.points(), MAX_HEALTH * REVIVED_TO);
    }

    #[test]
    fn nothing_mends_on_its_own_and_a_heal_never_passes_the_bar() {
        let mut h = Health::new();
        h.hit(40.0);
        h.update(3_600.0);
        assert_eq!(h.points(), 60.0, "no passive regeneration");
        assert_eq!(h.heal(10.0), 10.0);
        assert_eq!(h.heal(100.0), 30.0);
        assert_eq!(h.points(), MAX_HEALTH);
    }

    #[test]
    fn a_body_bleeds_under_twenty_downed_or_not() {
        let mut h = Health::new();
        assert!(!h.bleeds());
        h.hit(MAX_HEALTH - BLEEDS_UNDER);
        assert!(!h.bleeds(), "twenty is not under twenty");
        h.hit(1.0);
        assert!(h.bleeds());
        h.hit(100.0);
        assert!(h.downed() && h.bleeds());
        h.give_up();
        assert!(!h.bleeds(), "the dead have stopped");
    }
}
