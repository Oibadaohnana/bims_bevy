//! Deployables: what an engineer lays on a deck (feature 74, task 127).
//!
//! A [`Deployable`] is a **room object, never a part of the ship**: it
//! never touches the design, its hash, its mass or its flight. The world
//! keeps the list ([`crate::World::deployables`], saved and in
//! `world_checksum`), each one a kind, whose it is, which deck it stands
//! on and which tile of that deck's design, what it has left, and — for
//! the ultimate's sentry — when it goes.
//!
//! # Four kinds
//!
//! A **mine** (Q, task 154) lies on its tile until an enemy — a body of
//! the crew's room's targets, a machine or a hostile Bim — stands within
//! `class::MINE_TRIGGER` of it, and then goes off (`World::settle_mines`,
//! before the room steps): the room bursts it at once
//! (`bims::game::Game::detonate`), every enemy in `class::MINE_RADIUS`
//! with a clear line taking `class::MINE_DAMAGE`, half at the edge — never
//! the crew, never a sentry. It is nobody's target and stops nothing.
//! `class::MINE_STANDING` of one engineer's lie at once; one more laid
//! takes the oldest up.
//!
//! A **satchel charge** (E, task 154) is thrown, never laid: the room
//! lobs it the grenade's way (`Game::throw_satchel`) and says where it
//! landed (`Game::take_satchels_landed`), and it lies there — on a tile
//! another deployable stands on as happily, and on top of other satchels,
//! which **stack** — until its engineer's remote trigger
//! (`Command::Detonate`) sets off every one of his in the room, each its
//! own burst of the mine's kind at `class::SATCHEL_DAMAGE` and
//! `class::SATCHEL_RADIUS`. It is never packed up. The sandbags (code 0)
//! were E's until then: low cover laid on a tile, gone with task 154.
//!
//! A **Healing Sentry** (C) is a sentry body with no barrel: each step it
//! heals every crew Bim on its feet, short of its full bar, within
//! `class::HEALING_SENTRY_RADIUS` and in its sight from its tile, at
//! `class::HEALING_SENTRY_RATE` of the medic's beam — several reaching
//! one Bim do not stack, the highest rate applies (`World::healing_links`).
//! It never heals an enemy, never revives and never mends a deployable.
//! Its charges are the standing limit: one more laid destroys that
//! engineer's oldest. Its health was doubled in task 154.
//!
//! The **sentry** (R, the engineer's ultimate) is a shooter with no body
//! — `bims::combat::Sentry`, handed to the crew's room every step and
//! fired there through the one trigger and the one hit calculation a Bim
//! uses — a minigun at `class::SENTRY_TIER` of the rank, its fire rate
//! times `class::SENTRY_FIRE_RATE`, `class::SENTRY_HEALTH` in one pool.
//! Its reach is the minigun's and `class::SENTRY_RANGE` tiles more; its
//! health was doubled in task 154, the sandbags gone. One
//! stands at a time, and it stands until it is destroyed, another is
//! laid, the mission ends or the rooms unjoin — there is no timer. It
//! cannot be packed up.
//!
//! Both sentries are the enemies' targets: a station's people and its
//! machines are handed them after the crew, their hits drain its health,
//! and at nought it is destroyed and removed.
//!
//! # Charges are counters, and there are no kits (task 127)
//!
//! What a mine, a Healing Sentry and a satchel spend is a
//! `class::Charge`: a **counter** a crew member, kept by the world
//! (`World::charges_held`), never a thing in a pack. A spent charge
//! comes back on its rank's cooldown (`World::restock_charges`). The
//! ultimate's sentry spends no charge: it is on a cooldown of its own,
//! counted from the laying (`crate::engineer::Engineer`).
//!
//! # Laying one, and taking it up
//!
//! `Command::Deploy` (a mine, a Healing Sentry) and `Command::Sentry`
//! want the slot's own Bim fit to act, an engineer, the ability learnt,
//! a charge (or the ultimate ready), and a tile of reachable deck floor
//! that is not a door or an airlock, holds no blocking part and no
//! deployable. The Bim does it as an errand (`bims::task::Kind::Deploy`):
//! it walks towards the tile until within `bims::task::DEPLOY_REACH`
//! (two tiles, a clear line) and works the rank's minutes there, `effort`
//! applying, its weapon holstered; a hit never drops it, and another
//! order — a second kit placed — drops it rather than queueing it. The
//! charge is spent only when the work is done. `Command::PackUp` is an
//! engineer beside one of the crew's mines or Healing Sentries taking it
//! up and getting that charge back, capped at its charges.
//!
//! # Lifetime
//!
//! On the ship's deck mines, satchels and a Healing Sentry stay until
//! packed up, set off or destroyed — across docking, undocking, joins and unjoins — so a
//! crew can prepare for the next site on the way to it. On a station's
//! deck every deployable is lost when the rooms unjoin, and the
//! ultimate's sentry is taken off at every mission's start.

use crate::class::Charge;

/// What a deployable is. Codes cross the seam and are never renumbered.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DeployKind {
    // The sandbags (0) went in task 154; the code is left free.
    /// The ultimate's sentry (R).
    Sentry = 1,
    /// The Healing Sentry (C, task 127).
    HealingSentry = 2,
    /// A mine (Q, task 154).
    Mine = 3,
    /// A satchel charge (E, task 154), thrown.
    Satchel = 4,
}

impl DeployKind {
    pub const ALL: [DeployKind; 4] = [
        DeployKind::Sentry,
        DeployKind::HealingSentry,
        DeployKind::Mine,
        DeployKind::Satchel,
    ];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<DeployKind> {
        DeployKind::ALL.into_iter().find(|k| k.code() == code)
    }

    /// The charge it is laid or thrown from, or `None` for the ultimate's
    /// sentry, which is on a cooldown.
    pub fn charge(self) -> Option<Charge> {
        match self {
            DeployKind::Mine => Some(Charge::Mine),
            DeployKind::HealingSentry => Some(Charge::HealingSentry),
            DeployKind::Satchel => Some(Charge::Satchel),
            DeployKind::Sentry => None,
        }
    }

    /// Whether `Command::Deploy` lays it: a mine and a Healing Sentry —
    /// the sentry is `Command::Sentry`'s, a satchel is thrown.
    pub fn is_laid(self) -> bool {
        matches!(self, DeployKind::Mine | DeployKind::HealingSentry)
    }

    /// Whether `Command::PackUp` takes it back up: what is laid.
    pub fn packs_up(self) -> bool {
        self.is_laid()
    }

    /// Whether it is one of the two sentries: a body the enemies aim at.
    pub fn is_sentry(self) -> bool {
        matches!(self, DeployKind::Sentry | DeployKind::HealingSentry)
    }
}

/// Which deck a deployable stands on: the ship's own, or a station's by
/// its id — a berth's or a settlement's.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Deck {
    Ship,
    Station(u32),
}

impl Deck {
    /// The station, or `u32::MAX` for the ship: what the checksum eats.
    pub fn code(self) -> u32 {
        match self {
            Deck::Ship => u32::MAX,
            Deck::Station(id) => id,
        }
    }
}

/// One thing laid on a deck. See the module note.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Deployable {
    /// Its identity: ids only climb and are never reissued, like a site's.
    pub id: u32,
    pub kind: DeployKind,
    /// The slot whose engineer laid it: whose ranks a sentry fires and
    /// heals with.
    pub owner_slot: u32,
    pub deck: Deck,
    /// The tile of that deck's design it stands on.
    pub tile: (u32, u32),
    /// What it has left: its kind's health of the owner's rank when laid.
    pub health: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kind_is_its_code_and_its_charge() {
        for kind in DeployKind::ALL {
            assert_eq!(DeployKind::from_code(kind.code()), Some(kind));
        }
        assert_eq!(DeployKind::Mine.charge(), Some(Charge::Mine));
        assert_eq!(DeployKind::Satchel.charge(), Some(Charge::Satchel));
        assert_eq!(DeployKind::from_code(0), None, "the sandbags' code is free");
        assert!(DeployKind::Mine.is_laid() && !DeployKind::Satchel.is_laid());
        assert!(!DeployKind::Sentry.is_laid() && !DeployKind::Satchel.packs_up());
        assert_eq!(
            DeployKind::HealingSentry.charge(),
            Some(Charge::HealingSentry)
        );
        assert_eq!(DeployKind::Sentry.charge(), None, "never packed up");
        assert!(DeployKind::Sentry.is_sentry() && DeployKind::HealingSentry.is_sentry());
        assert!(!DeployKind::Mine.is_sentry() && !DeployKind::Satchel.is_sentry());
        assert_eq!(Deck::Ship.code(), u32::MAX);
        assert_eq!(Deck::Station(7).code(), 7);
    }
}
