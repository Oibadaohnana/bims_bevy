//! The medic's state (feature 76, `crate::class`): what a crew member
//! that is a medic holds between steps — who its heal beam is on, how
//! charged its surge is, and whether it has done its one field surgery
//! this fight. One [`Medic`] a crew member, by index, on
//! `World::medics`; a crew member that is not a medic keeps an empty
//! one. Saved and in `world_checksum` whole.
//!
//! The rules live on the world (`World::can_beam`, `beam`, `can_surge`,
//! `surge`, `settle_medics`, `hand_the_room_the_medics`), and what a
//! beam does to a body is the room's `bims::health::Beamed`, handed
//! over every step; the room knows nothing of who holds whom.

/// One crew member's medic state.
#[derive(Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Medic {
    /// The crew members its beam holds, by index: none, one, or two
    /// with *double link*. Cleared whole whenever crew indices change
    /// (a hire, a bot dropped off the crew), since an index is all a
    /// link is.
    pub patients: Vec<u32>,
    /// Minutes of the clock spent beaming a patient that qualifies —
    /// below full blood, or with a wound open — towards
    /// `class::SURGE_CHARGE_MINUTES`, *quick charge* counting half
    /// again; capped at full, emptied by a surge, lost only with the
    /// medic's death.
    pub charge: f64,
    /// Whether the one field surgery a fight allows (*field surgeon*)
    /// has been done this fight. Put back when the fight ends — the
    /// rooms unjoined, or no enemy standing.
    pub field_surgery_used: bool,
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
