//! The tank's state (feature 77, `crate::class`): what a crew member
//! that is a tank holds between steps. One [`Tank`] a crew member, by
//! index, on `World::tanks`; a crew member that is not a tank keeps an
//! empty one. Saved and in `world_checksum` whole.
//!
//! There is only one thing to keep here — when he last taunted — because
//! everything else a tank is lives where it is used: **Bulwark** is a
//! toggle on the Bim (`bims::bim::Bim::bulwark`, `Game::set_bulwark`),
//! since the room is what has to know where the wall stands when a bolt
//! comes through it; the **hits** that make his experience are a count
//! on the Bim too (`Game::hits_taken`), since the room is where a hit
//! lands; and the armour passive and every talent are read afresh each
//! step off the class and the picks, through `bims::combat::Skill`
//! (`World::skill_of`). The rules are all on the world —
//! `World::can_bulwark`, `bulwark`, `can_taunt`, `taunt`,
//! `hand_the_room_the_tanks`, `settle_tanks`.

/// One crew member's tank state.
#[derive(Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tank {
    /// The clock reading his last taunt began at, in minutes;
    /// `None` until he has taunted. The taunt runs for
    /// `World::taunt_minutes` of the clock from it and the next one
    /// waits `class::TAUNT_COOLDOWN` seconds of the clock from it, so
    /// the one number answers both.
    pub last_taunt: Option<f64>,
}
