//! The medic's state (feature 76; a ranked kit since task 130,
//! `crate::class`): what a crew member that is a medic holds between
//! steps — who its heal beam is on, and when it last set off a Nanite
//! Burst and last cloaked somebody. One [`Medic`] a crew member, by
//! index, on `World::medics`; a crew member that is not a medic keeps an
//! empty one. Beside it, one [`Cloak`] a crew member on `World::cloaks`:
//! whoever it is, a player's Bim, a bot, a hired hand or a reinforcement,
//! cloaked by whichever medic. Both saved and in `world_checksum`, the
//! timestamps and the cloaks only where any is set.
//!
//! **There is no surge** (task 130): the charge, the level it was learnt
//! at and the room's timer's medic half went with it; the room's own
//! surge timer stays, since two relics (*Phase Harness*, *Lifeline*) still
//! use it.
//!
//! **The Healing Aura keeps nothing**: `World::heal_factor(who)` works it
//! out from where the medics stand whenever a heal is given — the beam,
//! the burst, a Healing Sentry, a relic — and a revive, which is not a
//! heal, never asks it.
//!
//! The rules live on the world (`World::can_beam`, `beam`,
//! `can_nanite_burst`, `nanite_burst`, `can_cloak`, `cloak`,
//! `heal_factor`, `hand_the_room_the_medics`), and what a beam does to a
//! body is the room's `Game::heal`, handed the step's hit points; the
//! room knows nothing of who holds whom.

/// One crew member's medic state.
#[derive(Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Medic {
    /// The crew members its beam holds, by index: none, one, or two at
    /// the beam's fourth rank. Cleared whole whenever crew indices change
    /// (a hire, a bot dropped off the crew), since an index is all a link
    /// is.
    pub patients: Vec<u32>,
    /// The mission minute of its last Nanite Burst (task 130): what its
    /// cooldown runs from. `None` at every mission's start.
    #[cfg_attr(feature = "serde", serde(default))]
    pub last_burst: Option<f64>,
    /// The mission minute it last cloaked somebody (task 130): what the
    /// ultimate's cooldown runs from. `None` at every mission's start.
    #[cfg_attr(feature = "serde", serde(default))]
    pub last_cloak: Option<f64>,
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

/// A crew member under a medic's **cloak** (task 130): no enemy picks it,
/// it fires nothing and uses no ability, and it walks faster. Kept by the
/// crew member cloaked, whoever cast it; the default is no cloak.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cloak {
    /// The mission minute it ends; `None` for none. A cloak on a Bim
    /// already cloaked keeps the later of the two ends, never their sum.
    pub until: Option<f64>,
    /// What its pace is multiplied by while it lasts: the rank of the
    /// medic who cast it, the higher of two.
    pub pace: f32,
    /// Seconds from its last cast to its end: what the ring under a
    /// cloaked Bim empties over. Drawing only, and not hashed — it is
    /// what `until` was set from.
    #[cfg_attr(feature = "serde", serde(default))]
    pub seconds: f64,
}
