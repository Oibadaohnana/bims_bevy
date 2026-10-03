//! The tank's state (feature 77; a ranked kit since task 139, reworked by
//! task 155, `crate::class`): what a crew member that is a tank holds
//! between steps. One [`Tank`] a crew member, by index, on
//! `World::tanks`; a crew member that is not a tank keeps an empty one —
//! bar the [`Tank::hasted`] a tank's Bastion leaves on whoever it
//! reached, which is anybody's. Saved, and in `world_checksum` where set.
//!
//! What is kept is what runs on a clock: the **Riot Shield**'s hit points
//! spent, whether it is up, when it was last struck and when it broke; the **Reflect
//! Barrier** running and its cooldown; the **Bastion**'s cooldown and its
//! haste. The plate itself is handed to the room every step
//! (`Game::set_riot_shields`), the barrier through the tank's
//! `bims::combat::Skill::reflect`, and the Bastion's shield is the room's
//! own (`Game::set_draining_shield`). **Plated** and the armour passive
//! are read afresh each step (`World::skill_of`). The rules are all on
//! the world — `World::can_riot_shield`, `riot_shield`, `can_reflect`,
//! `reflect`, `can_bastion`, `bastion`, `tank_skill`,
//! `hand_the_room_the_tanks`, `settle_tanks`.

/// One ability running on a clock: the mission minute it began, and the
/// one it ends at. Never moved once set — a cooldown relic moves the
/// cooldown's start (`Tank::last_reflect`, `Tank::last_bastion`) and not
/// this.
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
    /// Whether the Riot Shield is held up (task 155). Put down by a press
    /// and when it breaks, and when he goes down.
    #[cfg_attr(feature = "serde", serde(default))]
    pub shield_up: bool,
    /// The shield's hit points spent, off its rank's whole — nought is a
    /// whole shield, so a fresh tank has one.
    #[cfg_attr(feature = "serde", serde(default))]
    pub shield_spent: f32,
    /// The mission minute the shield was last struck, for the wait
    /// before a shield held up restores; `None` when never this mission.
    #[cfg_attr(feature = "serde", serde(default))]
    pub shield_struck: Option<f64>,
    /// The mission minute it last broke, the start of the cooldown before
    /// it may be raised again; `None` once that is over, or never broken
    /// this mission.
    #[cfg_attr(feature = "serde", serde(default))]
    pub shield_broke: Option<f64>,
    /// The mission minute his last Reflect Barrier began, the cooldown's
    /// start; `None` until he has raised one this mission.
    #[cfg_attr(feature = "serde", serde(default))]
    pub last_reflect: Option<f64>,
    /// The Reflect Barrier running, or the last one.
    #[cfg_attr(feature = "serde", serde(default))]
    pub reflect: Window,
    /// The mission minute his last Bastion was thrown, the cooldown's
    /// start.
    #[cfg_attr(feature = "serde", serde(default))]
    pub last_bastion: Option<f64>,
    /// The haste a Bastion at the *Override Core*'s fifth rank left on
    /// this crew member — whoever's it was — for its seconds.
    #[cfg_attr(feature = "serde", serde(default))]
    pub hasted: Window,
}
