//! The commander's state (feature 78, `crate::class`): what a crew
//! member that is a commander holds between steps, and the one squad
//! order the crew is under.
//!
//! Two things are kept here and nothing else, because everything else a
//! commander is is read afresh each step off his class, his picks and
//! where he stands:
//!
//! * one [`Commander`] a crew member, by index, on `World::commanders` —
//!   when he last rallied, which answers both how long the rally has to
//!   run and how long until the next;
//! * one [`SquadOrder`] for the whole world, on `World::squad` — whose
//!   it is, what it is, and which crew members are under it.
//!
//! **The aura is not kept.** It is worked out every step from where the
//! commanders stand (`World::aura_reaching`) and goes to the room
//! through `bims::combat::Skill` like every other class's numbers, so it
//! follows him about with no state to keep in step. So is the rally's
//! reach, off `last_rally` and the clock.
//!
//! **Who an order reaches.** The aura and the rally lift *every friendly
//! Bim* in range — a player's own steered Bim, the crew's bots and the
//! hired hands alike. A squad order commands *only the squad*: every
//! crew member no player is steering. A player's Bim is never moved,
//! held or aimed by one, and a Bim a player starts steering leaves the
//! order the same step.
//!
//! Both are saved and in `world_checksum` whole.

/// One crew member's commander state.
#[derive(Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Commander {
    /// The clock reading his last rally began at, in minutes; `None`
    /// until he has rallied. The rally runs for `World::rally_minutes`
    /// of the clock from it and the next one waits
    /// `class::RALLY_COOLDOWN` seconds of the clock from it, so the one
    /// number answers both — the tank's taunt kept the same way.
    pub last_rally: Option<f64>,
}

/// What a commander's aura does to a Bim standing in it: every one of
/// them a factor, one meaning nothing. Worked out fresh every step
/// (`World::aura_reaching`) and never kept — the talents *strong
/// presence* and *anchor* deepen each of them together
/// (`class::aura_bonus`), so [`Aura::work`] is what they are ranked by
/// when two commanders reach one Bim.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Aura {
    /// What its working steps run at.
    pub work: f32,
    /// What its odds are multiplied by.
    pub aim: f32,
    /// What the hold before it runs is multiplied by
    /// (`class::NERVE_HOLD`).
    pub nerve: f32,
    /// What its wounds bleed at: *steady ranks*, one without it.
    pub bleed: f32,
    /// What its pace is multiplied by: *double time*, one without it.
    pub pace: f32,
}

impl Aura {
    /// No aura at all.
    pub const NONE: Aura = Aura {
        work: 1.0,
        aim: 1.0,
        nerve: 1.0,
        bleed: 1.0,
        pace: 1.0,
    };
}

/// What a player asks the squad to do — [`crate::world::Command::Squad`]'s
/// payload, which is why it carries one enemy and one tile rather than
/// the list [`SquadKind`] keeps: a command is `Copy`.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SquadAsk {
    /// Fire on that resident of the station alongside.
    Attack { enemy: u32 },
    /// Fall back to that tile of the crew's room; `None` is back to the
    /// commander himself, which is what the pointer on nothing valid
    /// means.
    FallBack { tile: Option<(i32, i32)> },
    /// Hold where you stand.
    StandGround,
}

/// What a squad order is. The enemies of an attack are residents of the
/// station the crew's room is joined to, by index; the tile of a fall
/// back is a tile of the crew's room.
///
/// An attack carries a *list* rather than the one enemy, because
/// *pincer* marks two and splits the squad between them; without it the
/// list is one long.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SquadKind {
    /// Fire on these enemies ahead of any nearer one, and advance on
    /// them the way a crew member that sees an enemy for itself does.
    Attack { enemies: Vec<u32> },
    /// Walk to a slot round this tile, holding fire on the way, and
    /// hold there shooting what can be seen.
    FallBack { tile: (i32, i32) },
    /// Hold exactly where each one stands, shooting what it can see:
    /// no walk to cover, and no running.
    StandGround,
}

impl SquadKind {
    /// The number that crosses the seam and names the order in the app.
    pub fn code(&self) -> u32 {
        match self {
            SquadKind::Attack { .. } => 0,
            SquadKind::FallBack { .. } => 1,
            SquadKind::StandGround => 2,
        }
    }

    /// Whether two orders are the same order given again — which
    /// releases the squad. An attack on the same enemies, a fall back on
    /// the same tile, a stand ground.
    pub fn same_as(&self, other: &SquadKind) -> bool {
        match (self, other) {
            (SquadKind::Attack { enemies: a }, SquadKind::Attack { enemies: b }) => a == b,
            (SquadKind::FallBack { tile: a }, SquadKind::FallBack { tile: b }) => a == b,
            (SquadKind::StandGround, SquadKind::StandGround) => true,
            _ => false,
        }
    }
}

/// The one order the squad is under, while it is under one.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SquadOrder {
    /// The player slot whose commander gave it.
    pub by_slot: u32,
    /// What it is.
    pub kind: SquadKind,
    /// The crew members under it, by index, lowest first: those that
    /// were in range when it was given and have not been taken out of it
    /// by an order of their player's own.
    pub members: Vec<u32>,
}

impl SquadOrder {
    /// Whether a crew member is under this order.
    pub fn has(&self, who: u32) -> bool {
        self.members.contains(&who)
    }

    /// The enemy this member of the squad is to fire on, of an attack's
    /// marks: the squad split between them in turn, so a *pincer*'s two
    /// take half the squad each. `None` for any other order.
    pub fn mark_for(&self, who: u32) -> Option<u32> {
        let SquadKind::Attack { enemies } = &self.kind else {
            return None;
        };
        if enemies.is_empty() {
            return None;
        }
        let rank = self.members.iter().position(|&m| m == who)?;
        Some(enemies[rank % enemies.len()])
    }
}
