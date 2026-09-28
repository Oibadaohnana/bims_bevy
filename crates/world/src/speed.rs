//! How fast the world runs, and who decides.
//!
//! **1×, or paused, and nothing else** (task 119): a minute of the clock a
//! real second, sixty steps of it. The faster speeds — 3×, 10×, 24× and the
//! top speed — went, with their keys; every timer, cooldown and rate in a
//! mission is read at 1× now.
//!
//! **Everybody, and the slowest wins.** There is one world and one clock in
//! it, so a speed is not a per-player view setting the way a camera is — it is
//! a change to what happens. With two speeds left that is the one rule that
//! matters: a pause by anybody is a pause for everybody, which is the one
//! control that has to work the instant it is asked for.

/// The speeds the world will run at.
///
/// The discriminants cross the wire in [`crate::Command::SetSpeed`], so they
/// are written out and not renumbered. The multiplier is deliberately not
/// the discriminant: a pause is a speed of nothing, not a missing speed.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Speed {
    Paused = 0,
    /// A minute of the clock a real second: the only speed there is.
    Real = 1,
}

impl Speed {
    /// Every speed.
    pub const ALL: [Speed; 2] = [Speed::Paused, Speed::Real];

    /// How many game minutes a minute of real time is worth.
    pub fn multiplier(self) -> u32 {
        match self {
            Speed::Paused => 0,
            Speed::Real => 1,
        }
    }

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<Speed> {
        Speed::ALL.get(code as usize).copied()
    }
}

/// What the world actually runs at: paused if anybody has asked for a
/// pause, 1× otherwise.
///
/// An empty list is a paused world rather than a running one — a game with
/// nobody in it should not be running.
pub fn effective(requests: &[Speed]) -> Speed {
    requests.iter().copied().min().unwrap_or(Speed::Paused)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The world runs at 1× or not at all: no speed has any other
    /// multiplier, and no code past the two is a speed.
    #[test]
    fn the_world_only_runs_at_one_times_or_paused() {
        assert_eq!(Speed::ALL.len(), 2);
        for speed in Speed::ALL {
            assert!(speed.multiplier() <= 1, "{speed:?}");
            assert_eq!(Speed::from_code(speed.code()), Some(speed));
        }
        assert_eq!(Speed::Real.multiplier(), 1);
        assert_eq!(Speed::Paused.multiplier(), 0);
        for code in 2..8 {
            assert_eq!(Speed::from_code(code), None, "code {code}");
        }
    }

    #[test]
    fn a_pause_by_anybody_is_a_pause() {
        assert_eq!(effective(&[Speed::Real, Speed::Real]), Speed::Real);
        assert_eq!(effective(&[Speed::Real, Speed::Paused]), Speed::Paused);
        assert_eq!(effective(&[Speed::Paused, Speed::Real]), Speed::Paused);
        assert_eq!(effective(&[Speed::Paused]).multiplier(), 0);
        assert_eq!(effective(&[]), Speed::Paused);
    }
}
