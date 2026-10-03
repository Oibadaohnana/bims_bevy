//! The soldier's state (task 124, `crate::class`): what a crew member
//! that is a soldier holds between steps. One [`Soldier`] a crew member,
//! by index, on `World::soldiers`; a crew member that is not a soldier
//! keeps an empty one. Saved and in `world_checksum` whole.
//!
//! The **Rampage** is kept here — when the last one began and when the
//! one running ends — and the **Stun Shot** (October 2026): the shot
//! charging and when the last one was fired. The rest of the ranked kit
//! lives where it is used: the ranks on `class::Progress`, the grenade's
//! charges on the world's charge list, the planted feet of a charge on
//! the Bim (`Game::set_braced`), and Weak Spot's crits on the world's own
//! stream (`World::crit_rng`) and the room's hits. The rules are all on
//! the world — `can_rampage`, `rampage`, `can_stun_shot`, `stun_shot`,
//! `settle_stun_shots`, `soldier_skill`.

/// One crew member's soldier state.
#[derive(Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Soldier {
    /// The mission minute its last Rampage began: the cooldown runs from
    /// it. `None` until it has gone on one this mission — every mission
    /// starts with the Rampage ready. A relic taking seconds off the class
    /// cooldowns (*Kill Relay*, *Squad Morale*) moves this back, and never
    /// [`Soldier::until`].
    pub began: Option<f64>,
    /// The mission minute the Rampage running ends at, its extension
    /// counted; nought, or in the past, with none running.
    pub until: f64,
    /// Seconds added to the Rampage running by enemies downed during it
    /// (rank four), at most `class::RAMPAGE_EXTEND_MAX`.
    pub extended: f64,
    /// The Stun Shot charging (October 2026); `None` with none.
    #[cfg_attr(feature = "serde", serde(default))]
    pub charging: Option<Charging>,
    /// The mission minute its last Stun Shot was fired: the cooldown runs
    /// from it. `None` until one has been fired this mission; a charge
    /// called off starts nothing. *Kill Relay* moves it back.
    #[cfg_attr(feature = "serde", serde(default))]
    pub last_shot: Option<f64>,
}

/// A Stun Shot charging: when it fires, and the room tile it was aimed
/// at.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Charging {
    /// The mission minute it fires at.
    pub until: f64,
    /// The room tile aimed at when the key was pressed.
    pub tile: (i32, i32),
}

impl Soldier {
    /// Whether a Rampage is running at mission minute `now`.
    pub fn rampaging(&self, now: f64) -> bool {
        self.began.is_some() && now < self.until
    }
}

/// What a critical hit adds (task 124, Weak Spot): the weapon's flat
/// damage — at the distance flown, before any relic or ability factor —
/// times the crit damage less one. Added to the hit after every factor,
/// so a relic's share is never multiplied by the crit.
pub fn crit_bonus(flat: f32, crit_damage: f32) -> f32 {
    flat * (crit_damage - 1.0).max(0.0)
}
