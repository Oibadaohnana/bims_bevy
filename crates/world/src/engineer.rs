//! The engineer's state (task 127, `crate::class`): what a crew member
//! that is an engineer holds between steps. One [`Engineer`] a crew
//! member, by index, on `World::engineers`; a crew member that is not an
//! engineer keeps an empty one. Saved and in `world_checksum` whole.
//!
//! Only the **ultimate's cooldown** is kept here, because the rest of the
//! ranked kit lives where it is used: the ranks on `class::Progress`, the
//! EMP's, the Healing Sentry's and the sandbags' charges on the world's
//! counters (`World::charges_held`), what is laid on
//! `World::deployables` — the sentry stands there until it is destroyed
//! — and a machine's stun on the machine
//! (`bims::droid::Droid::stun`). The rules are all on the world.

/// One crew member's engineer state.
#[derive(Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Engineer {
    /// The mission minute its last sentry was laid: the ultimate's
    /// cooldown runs from it. `None` until it has laid one this mission —
    /// every mission starts with the ultimate ready. A relic taking
    /// seconds off the class cooldowns moves this back.
    pub sentry_laid: Option<f64>,
}
