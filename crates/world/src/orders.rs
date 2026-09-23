//! The two standing orders a player gives the bots that follow them
//! (feature 84): **attack** and **retreat**, and the following itself,
//! which is what every slot starts on and goes back to.
//!
//! One [`Standing`] a player slot, on `World::standing`. It is the
//! world's — in the checksum, in a save and across the wire — because it
//! moves bodies: the room is handed one `bims::game::Standing` a slot
//! every step (`World::hand_the_room_the_standing`) and the crew's bots
//! read it there.
//!
//! **Whose order a bot is under** is not decided here. The room decides
//! it, and it decides it by distance: a bot takes the order of the
//! player whose own Bim is nearest. With one player that is the one
//! order there is; with several, each player leads the bots about them.
//!
//! **What an order does not reach.** A Bim a player steers is never
//! moved by one — a player's own Bim is the player's. Neither is a crew
//! member on a chain, one running for its life, one a commander's squad
//! order has claimed, or one holding a post its player right-clicked for
//! it: an order to one crew member outranks the standing order to the
//! rest, which is how a player picks a bot out of the line and sends it
//! somewhere else.
//!
//! **The ship is the last stand.** No order takes a bot out of a fight
//! aboard the ship: cornered in its own hull it fights, retreat or no
//! retreat, and a dying one does not run past the gangway. That rule is
//! the room's (`bims::game::Game::cornered`), since the room is what
//! knows where the enemy are standing.

/// What one player's bots are doing. The tile of an attack is a tile of
/// the crew's room — the deck the crew walk, the joined station's deck
/// and a settlement's ground included — the way a commander's fall back
/// carries one.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Standing {
    /// Keep to the player's side, and fight for yourselves the moment
    /// you see an enemy. What everybody starts on.
    #[default]
    Follow,
    /// Fight your way to that tile and hold it.
    Attack { tile: (i32, i32) },
    /// Back to the ship.
    Retreat,
}

impl Standing {
    /// The number that crosses the seam and names the order in the app.
    pub fn code(self) -> u32 {
        match self {
            Standing::Follow => 0,
            Standing::Attack { .. } => 1,
            Standing::Retreat => 2,
        }
    }

    /// Whether two orders are the same order given again, which is what
    /// puts the bots back to following: an attack on the same tile, a
    /// retreat called twice.
    pub fn same_as(self, other: Standing) -> bool {
        match (self, other) {
            (Standing::Attack { tile: a }, Standing::Attack { tile: b }) => a == b,
            (Standing::Retreat, Standing::Retreat) => true,
            (Standing::Follow, Standing::Follow) => true,
            _ => false,
        }
    }
}
