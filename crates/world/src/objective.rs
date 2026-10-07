//! The second set of attack missions (October 2026, the player's: A1 to
//! A5 — "for now go ahead with those also make maps fitting to that"):
//! **Kill the Overseer**, **a data heist**, **a prison break**, **a fuel
//! run** and **a salvage sweep**. These are their states, kept on the
//! site's `Infestation` (`objective`), saved, hashed and put back with it;
//! the rules are `crate::world`'s `attacks.rs`.

/// What an attack's mission of the second set has standing.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Objective {
    Overseer(Overseer),
    Heist(Heist),
    Prison(Prison),
    FuelRun(FuelRun),
    Salvage(Salvage),
}

/// Where the Overseer is in his fight.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum OverseerPhase {
    /// In his office, the waves coming while he lives.
    #[default]
    Office,
    /// Hurt past half his health, walking for his airlock.
    Fleeing,
    /// Out of it: the bounty gone with him.
    Escaped,
    /// Down, and the waves with him.
    Down,
}

impl OverseerPhase {
    pub fn code(self) -> u32 {
        self as u32
    }
}

/// **Kill the Overseer**: which of the site's room's Bims he is, his
/// office's tile, the airlock he makes for (an index among the design's
/// airlocks), where he is in it, and when the next wave lands while he
/// lives (a mission step; `u64::MAX` before he is laid).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Overseer {
    pub body: Option<u32>,
    pub office: (u32, u32),
    pub airlock: u32,
    pub phase: OverseerPhase,
    pub next_wave: u64,
}

/// **A data heist**: each terminal's tile, how many steps of hands have
/// been on it, whether it is taken, and when the next wave lands.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Heist {
    pub terminals: Vec<(u32, u32)>,
    pub work: Vec<u32>,
    pub taken: Vec<bool>,
    pub next_wave: u64,
}

impl Heist {
    /// How many are taken.
    pub fn taken_count(&self) -> u32 {
        self.taken.iter().filter(|&&t| t).count() as u32
    }
}

/// **A prison break**: the cell's door (its first tile, where the station
/// has one) and the tile it is cut from, the tile the prisoners stand round, the cutting's steps,
/// whether it is open, and the prisoners by crew index.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Prison {
    pub door: Option<(u32, u32)>,
    /// The corridor's tile just outside the door, where the cutting is
    /// worked from (a door's own tile takes no errand).
    #[cfg_attr(feature = "serde", serde(default))]
    pub outside: Option<(u32, u32)>,
    pub cell: (u32, u32),
    pub cut: u32,
    pub open: bool,
    pub prisoners: Vec<u32>,
}

/// Where a thing carried in a fuel run or a salvage sweep is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LoadState {
    /// Lying where `at` says.
    #[default]
    Lying,
    /// In `carrier`'s arms.
    Carried,
    /// Where it was carried to: the reactor, the ship.
    Home,
    /// Gone up (a drum hit in the arms).
    Lost,
}

/// A drum or a crate: where it lies in the crew's room (whole units), who
/// carries it, where it is, and the carrier's hits when it was taken up —
/// a hit more and a drum goes up.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Load {
    pub at: (i32, i32),
    pub carrier: Option<u32>,
    pub state: LoadState,
    pub hits: u32,
}

impl Load {
    pub fn lying(at: (i32, i32)) -> Load {
        Load {
            at,
            carrier: None,
            state: LoadState::Lying,
            hits: 0,
        }
    }
}

/// **A fuel run**: the drums, where a drum lost is stood again (the
/// depot's first spot, the crew's room), the reactor's core middle (the
/// site's design units, whole), how many are in, whether it has gone
/// critical, when the next wave lands and when a lost drum is stood
/// again.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FuelRun {
    pub drums: Vec<Load>,
    pub depot: (i32, i32),
    pub reactor: (i32, i32),
    pub delivered: u32,
    pub critical: bool,
    pub next_wave: u64,
    pub restock_at: Option<u64>,
}

/// **A salvage sweep**: the crates, how many have been taken up (each a
/// bigger wave) and how many are home on the ship.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Salvage {
    pub crates: Vec<Load>,
    pub taken: u32,
    pub home: u32,
}
