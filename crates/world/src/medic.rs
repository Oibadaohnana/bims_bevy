//! The medic's state (feature 76; a ranked kit since task 130, `crate::
//! class`, reworked by task 153): what a crew member that is a medic
//! holds between steps — who its heal beam is on, its heal drone in the
//! air and when it last dropped one, and whether its healing circle is
//! on and when it last burned. One [`Medic`] a crew member, by index, on
//! `World::medics`; a crew member that is not a medic keeps an empty one.
//! Saved and in `world_checksum`, the drone and the circle only where
//! any is.
//!
//! **There is no surge** (task 130), and since task 153 no Nanite Burst,
//! no Healing Aura and no cloak.
//!
//! **Triage keeps nothing**: `World::medic_heal_factor(medic, who)` works
//! it out from where the healed Bim stands on its bar whenever a heal of
//! the medic's is given — the beam, the drone, the circle.
//!
//! The rules live on the world (`World::can_beam`, `beam`,
//! `can_heal_drone`, `heal_drone`, `fly_the_drones`,
//! `can_healing_circle`, `healing_circle`, `hand_the_room_the_circles`,
//! `hand_the_room_the_medics`), and what a heal does to a body is the
//! room's `Game::heal`, a circle's drain `Game::drain` and its burn
//! `Game::scorch`; the room knows nothing of who holds whom.

/// What a medic's beam or drone names a site's **defender** by (one of
/// `Residents::defender`, standing with the crew in the residents' room):
/// `GUEST + i` for that room's body `i` — the room's own way of naming a
/// visitor ([`bims::game::GUEST`]) — beside the crew's own indices.
pub const GUEST: u32 = bims::game::GUEST as u32;

/// The residents' room's body a patient names, if it names one rather
/// than a crew member.
pub fn guest_of(patient: u32) -> Option<u32> {
    patient.checked_sub(GUEST)
}

/// One crew member's medic state.
#[derive(Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Medic {
    /// The crew members its beam holds, by index — or a site's defenders,
    /// [`GUEST`]` + i`: none, one, or two at the beam's fourth rank.
    /// Cleared whole whenever crew indices change
    /// (a joiner, a bot dropped off the crew), since an index is all a link
    /// is.
    pub patients: Vec<u32>,
    /// The mission minute it last dropped a Heal Drone (task 153): what
    /// its cooldown runs from. `None` at every mission's start.
    #[cfg_attr(feature = "serde", serde(default))]
    pub last_drone: Option<f64>,
    /// Its Heal Drone in the air, if any (task 153).
    #[cfg_attr(feature = "serde", serde(default))]
    pub drone: Option<Drone>,
    /// Whether its Healing Circle is on (task 153). Off at every
    /// mission's start.
    #[cfg_attr(feature = "serde", serde(default))]
    pub circle: bool,
    /// The mission minute of the circle's last burn on the enemy in it —
    /// the moment it was switched on before the first — `None` with it
    /// off.
    #[cfg_attr(feature = "serde", serde(default))]
    pub last_burn: Option<f64>,
}

impl Medic {
    /// Whether the beam holds anybody.
    pub fn is_linked(&self) -> bool {
        !self.patients.is_empty()
    }

    /// Whether the beam holds `who`.
    pub fn holds(&self, who: u32) -> bool {
        self.patients.contains(&who)
    }

    /// The link broken, every patient let go.
    pub fn unlink(&mut self) {
        self.patients.clear();
    }
}

/// A medic's **Heal Drone** in the air (task 153): where it is on the
/// crew's deck, in the room's units, whom it is over or flying to, and
/// when it is gone.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Drone {
    /// Where it is, in the crew's room's units.
    pub x: f32,
    pub y: f32,
    /// The crew member it is flying to or hovering over — or a site's
    /// defender, [`GUEST`]` + i`, while no crew member is hurt; `None`
    /// with nobody hurt, when it keeps by its medic.
    pub patient: Option<u32>,
    /// The mission minute it is gone at.
    pub until: f64,
}
