//! What a Bim remembers of its days.
//!
//! A diary rather than a log, and a short one: nothing goes in unless it
//! actually mattered, and the one thing that does is a crewmate dying. A
//! run in which nobody aboard has died leaves **no entry at all**, which is
//! the intended reading of an empty page.
//!
//! **No strings live here.** Every entry is a day, a time, a code and one
//! number, and the host turns those into a sentence — the same rule the rest
//! of this boundary keeps.

/// How many entries a Bim keeps. Long enough to hold a good few days, short
/// enough that a Bim left running for a game year does not grow without bound
/// — the oldest fall off the front. Memory is finite; so is this.
pub const KEEP: usize = 320;

/// What happened. The host names each of these; the numbers are the contract.
///
/// **Only things worth remembering are in here.** The code is the one the
/// host has always read it by, so a crewmate's death is thirty whatever
/// went before it.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum What {
    /// One of the others died. `detail` is which of the crew. Written into
    /// everybody else's diary and not into the dead one's: a Bim does not
    /// record its own end, and there would be nobody to read it back.
    CrewDied = 30,
}

impl What {
    pub fn code(self) -> u32 {
        self as u32
    }
}

/// One line of the diary.
#[derive(Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Moment {
    pub day: u32,
    /// Minutes since midnight, when it happened.
    pub at: f32,
    pub what: What,
    pub detail: u32,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Memory {
    kept: Vec<Moment>,
}

impl Memory {
    pub fn new() -> Memory {
        Memory {
            kept: Vec::with_capacity(KEEP),
        }
    }

    /// Remember something. The oldest is forgotten once the book is full.
    pub fn note(&mut self, day: u32, at: f32, what: What, detail: u32) {
        if self.kept.len() >= KEEP {
            self.kept.remove(0);
        }
        self.kept.push(Moment {
            day,
            at,
            what,
            detail,
        });
    }

    pub fn len(&self) -> usize {
        self.kept.len()
    }

    /// Entry `i`, oldest first. The host reads them in order and groups them
    /// by day; showing the newest day at the top is its business.
    pub fn at(&self, i: usize) -> Option<Moment> {
        self.kept.get(i).copied()
    }

    /// Whether anything of a kind has been remembered. For the probes.
    #[allow(dead_code)]
    pub fn any(&self, what: What) -> bool {
        self.kept.iter().any(|m| m.what == what)
    }
}
