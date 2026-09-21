//! What the room sounds like.
//!
//! A cue is a thing that just happened that a host may want to *play*: a
//! door starting to slide, a knife coming down on the board, a shot
//! leaving a gun, a bolt landing. The same arrangement as the diary and the
//! world's events, and for the same reason: **no sound comes out of the
//! room**. A cue is a code and a place, `crates/app/src/sound.rs` owns the
//! recordings and decides what each is played as, and a native server, which
//! has no speaker, drops them on the floor.
//!
//! A cue is said once, the step the thing happens, and drained by the host
//! with [`crate::game::Game::take_cues`] — a list of things that happened,
//! not a state, because a step at the world's top speed holds a door's
//! whole open-and-shut and a state read afterwards would show nothing.
//! Where a step is far shorter than a frame the host gets the whole
//! step's worth at once, and it is the host's business to thin them; the
//! room says everything.

use crate::combat::WeaponKind;
use crate::math::Vec2;

/// One thing worth hearing.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Cue {
    /// A door's leaves started drawing back — a ship's sliding door, or
    /// the heads' — from shut or from part way there.
    DoorOpens,
    /// And started closing.
    DoorShuts,
    /// One stroke of the knife on the board.
    Chop,
    /// A bolt left a gun. `hostile` is an enemy's, flown in this room at
    /// the crew.
    Shot { weapon: WeaponKind, hostile: bool },
    /// A bolt reached a body. `on_crew` is one of this room's own hit by
    /// an enemy's bolt; otherwise a crew member's bolt on an enemy.
    Impact { on_crew: bool },
    /// A bolt ended in a bulkhead, or a shut door.
    Ricochet,
    /// A swing or a jab landed: `cut` from a blade, a fist otherwise;
    /// `on_crew` as for a bolt.
    Blow { cut: bool, on_crew: bool },
    /// A body throwing itself at a locked door: one heave of the smashing,
    /// every couple of seconds while it goes on — see `door::Smash`.
    DoorSmash,
    /// The door gave: its lock forced, the leaves drawing back.
    DoorForced,
}

/// A cue and where in the room it happened, in room units — the door's
/// middle, the board, the gun's muzzle, the body.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cued {
    pub cue: Cue,
    pub at: Vec2,
}

/// Which way a door's leaves have just started moving, if they have.
///
/// `moving` is the door's own memory of the direction it was going — `1`
/// opening, `-1` shutting, `0` still — kept on the door and handed back
/// here each frame with its `open` before and after the frame's step. A
/// change of direction is a cue; a door that keeps going, or keeps still,
/// says nothing. So a door reversing part way — somebody stepped back out
/// of the opening — is heard shutting, and a door already open that is
/// told to hold is not heard at all.
pub fn door_motion(before: f32, after: f32, moving: &mut i8) -> Option<Cue> {
    let now: i8 = if after > before {
        1
    } else if after < before {
        -1
    } else {
        0
    };
    let was = *moving;
    *moving = now;
    match now {
        1 if was != 1 => Some(Cue::DoorOpens),
        -1 if was != -1 => Some(Cue::DoorShuts),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_door_is_heard_once_each_way() {
        let mut moving = 0i8;
        assert_eq!(door_motion(0.0, 0.1, &mut moving), Some(Cue::DoorOpens));
        assert_eq!(door_motion(0.1, 0.2, &mut moving), None);
        assert_eq!(door_motion(1.0, 1.0, &mut moving), None);
        // Held open and told to hold again: nothing moved, nothing said.
        assert_eq!(door_motion(1.0, 1.0, &mut moving), None);
        assert_eq!(door_motion(1.0, 0.9, &mut moving), Some(Cue::DoorShuts));
        assert_eq!(door_motion(0.9, 0.8, &mut moving), None);
        // Reversed part way: somebody walked back into the opening.
        assert_eq!(door_motion(0.8, 0.9, &mut moving), Some(Cue::DoorOpens));
    }
}
