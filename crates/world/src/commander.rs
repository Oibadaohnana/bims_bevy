//! The commander's state (feature 78, reworked by task 129 into a ranked
//! kit, `crate::class`): what a crew member that is a commander holds
//! between steps, the one squad order the crew is under, and the
//! reinforcements he brought to the mission.
//!
//! Three things are kept here and nothing else, because everything else a
//! commander is is read afresh each step off his ranks and where he
//! stands:
//!
//! * one [`Commander`] a crew member, by index, on `World::commanders` —
//!   when his last **Battle Cry** and his last **Rally** began, as
//!   mission minutes, and whom each reached: the one timestamp answers
//!   both how long it has to run and how long until the next, and **the
//!   reach is fixed at the call**, so a Bim that walks out keeps what it
//!   was given and one that walks in is given nothing;
//! * one [`SquadOrder`] for the whole world, on `World::squad` — whose
//!   it is, what it is, and which crew members are under it;
//! * one [`Reinforcement`] a Bim he brought, on `World::reinforcements`:
//!   a crew member marked with the commander who brought it, **for one
//!   mission** — laid at its start, gone from the deck the moment it dies
//!   and off the crew at its end, alive or not.
//!
//! **The aura is not kept.** It is worked out every step from where the
//! commanders stand (`World::aura_reaching`) and goes to the room
//! through `bims::combat::Skill` like every other class's numbers, so it
//! follows him about with no state to keep in step. It is **damage and
//! nothing else** since task 129.
//!
//! **Who each reaches.** The aura, the cry and the rally lift *every
//! friendly Bim* in range — a player's own steered Bim, the crew's bots,
//! the hired hands and the reinforcements alike, the commander himself
//! among them, and never a sentry. A squad order commands *only the
//! squad*: every crew member no player is steering. A player's Bim is
//! never moved, held or aimed by one, and a Bim a player starts steering
//! leaves the order the same step.
//!
//! All three are saved and in `world_checksum` — the cries, the rallies'
//! reach and the reinforcements only where there are any.

/// One crew member's commander state.
#[derive(Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Commander {
    /// The mission minute his last rally began at; `None` until he has
    /// rallied this mission — every mission starts with it ready. The
    /// rally runs for `World::rally_seconds` from it and the next one
    /// waits `World::rally_cooldown` from it, so the one number answers
    /// both. A relic taking seconds off the class cooldowns moves it back.
    pub last_rally: Option<f64>,
    /// Whom the last rally reached, by crew index, lowest first: every
    /// friendly Bim within [`crate::class::RALLY_TILES`] of him when he
    /// called it, himself included.
    #[cfg_attr(feature = "serde", serde(default))]
    pub rallied: Vec<u32>,
    /// The mission minute his last Battle Cry began at (task 129), kept
    /// the way the rally is.
    #[cfg_attr(feature = "serde", serde(default))]
    pub last_battle_cry: Option<f64>,
    /// Whom the last Battle Cry reached, by crew index, lowest first.
    #[cfg_attr(feature = "serde", serde(default))]
    pub cried: Vec<u32>,
}

impl Commander {
    /// A crew index gone from the crew (`World::drop_crew_member`): out
    /// of both reaches, and every index past it one lower.
    pub fn forget(&mut self, gone: u32) {
        for list in [&mut self.rallied, &mut self.cried] {
            list.retain(|&who| who != gone);
            for who in list.iter_mut() {
                if *who > gone {
                    *who -= 1;
                }
            }
        }
    }
}

/// What a commander's **Command Aura** does to a Bim standing in it: a
/// factor on its damage, one meaning nothing. Worked out fresh every step
/// (`World::aura_reaching`) and never kept; two commanders reaching one
/// Bim hold it by the higher factor.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Aura {
    /// What its damage is multiplied by, bolt and blow alike.
    pub damage: f32,
}

impl Aura {
    /// No aura at all.
    pub const NONE: Aura = Aura { damage: 1.0 };
}

/// A Bim a commander brought to the mission (task 129, his
/// Reinforcements): a crew member marked with the commander who brought
/// it. It fights, follows the squad's orders and is revived like any bot;
/// it earns nothing, costs nothing, drops nothing and keeps no run going.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Reinforcement {
    /// The crew member it is.
    pub who: u32,
    /// The player slot of the commander who brought it.
    pub by: u32,
}

/// What a player asks the squad to do — [`crate::world::Command::Squad`]'s
/// payload.
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

/// What a squad order is. The enemy of an attack is a resident of the
/// station the crew's room is joined to, by index — **one** since task
/// 129 took *pincer*'s second mark away; the tile of a fall back is a
/// tile of the crew's room.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SquadKind {
    /// Fire on this enemy ahead of any nearer one, and advance on it the
    /// way a crew member that sees an enemy for itself does.
    Attack { enemy: u32 },
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
    /// releases the squad. An attack on the same enemy, a fall back on
    /// the same tile, a stand ground.
    pub fn same_as(&self, other: &SquadKind) -> bool {
        match (self, other) {
            (SquadKind::Attack { enemy: a }, SquadKind::Attack { enemy: b }) => a == b,
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

    /// The enemy this member of the squad is to fire on: an attack's
    /// mark, the same for every member. `None` for any other order and
    /// for a crew member not under it.
    pub fn mark_for(&self, who: u32) -> Option<u32> {
        match self.kind {
            SquadKind::Attack { enemy } if self.has(who) => Some(enemy),
            _ => None,
        }
    }
}
