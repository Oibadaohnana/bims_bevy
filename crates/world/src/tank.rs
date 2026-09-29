//! The tank's state (feature 77; a ranked kit since task 139,
//! `crate::class`): what a crew member that is a tank holds between
//! steps. One [`Tank`] a crew member, by index, on `World::tanks`; a crew
//! member that is not a tank keeps an empty one. Saved, and in
//! `world_checksum` where set.
//!
//! Only the two abilities on a clock are kept here — the **Taunt** and
//! the **Juggernaut**, when each last began and the window each running
//! one covers — because everything else a tank is lives where it is
//! used: the ranks on `class::Progress`; **Bulwark** a toggle on the Bim
//! (`bims::bim::Bim::bulwark`, `Game::set_bulwark`), since the room is
//! what has to know where the wall stands when a bolt comes through it;
//! **Plated** and the armour passive read afresh each step through
//! `bims::combat::Skill` (`World::skill_of`). The rules are all on the
//! world — `World::can_taunt`, `taunt`, `can_juggernaut`, `juggernaut`,
//! `can_bulwark`, `bulwark`, `tank_skill`, `hand_the_room_the_tanks`.

/// One ability running on a clock: the mission minute it began, and the
/// one it ends at. Never moved once set — a cooldown relic moves the
/// cooldown's start (`Tank::last_taunt`, `Tank::last_juggernaut`) and not
/// this — so `began` says which of two taunts is the more recent.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Window {
    pub began: f64,
    pub until: f64,
}

impl Window {
    /// Whether it runs at mission minute `now`. The default window — both
    /// nought — runs at no minute a mission has.
    pub fn running(&self, now: f64) -> bool {
        now < self.until
    }
}

/// One crew member's tank state.
#[derive(Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tank {
    /// The mission minute his last taunt began, the cooldown's start;
    /// `None` until he has taunted this mission — every mission starts
    /// with the taunt ready. A relic taking seconds off the class
    /// cooldowns moves this back, and never [`Tank::taunt`].
    pub last_taunt: Option<f64>,
    /// The taunt running, or the last one.
    #[cfg_attr(feature = "serde", serde(default))]
    pub taunt: Window,
    /// The mission minute his last Juggernaut began, the cooldown's
    /// start, as [`Tank::last_taunt`] is the taunt's.
    #[cfg_attr(feature = "serde", serde(default))]
    pub last_juggernaut: Option<f64>,
    /// The Juggernaut running, or the last one.
    #[cfg_attr(feature = "serde", serde(default))]
    pub juggernaut: Window,
}

impl Tank {
    /// When the most recent of his taunt and his Juggernaut running at
    /// `now` began — which of two tanks an enemy follows — or `None` with
    /// neither running.
    pub fn forcing_since(&self, now: f64) -> Option<f64> {
        [self.taunt, self.juggernaut]
            .into_iter()
            .filter(|w| w.running(now))
            .map(|w| w.began)
            .reduce(f64::max)
    }
}
