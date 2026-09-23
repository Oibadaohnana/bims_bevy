//! Deployables: what an engineer lays on a deck (feature 74).
//!
//! A [`Deployable`] is a **room object, never a part of the ship**: it
//! never touches the design, its hash, its mass or its flight. The world
//! keeps the list ([`crate::World::deployables`], saved and in
//! `world_checksum`), each one a kind, whose it is, which deck it stands
//! on and which tile of that deck's design, and what it has left.
//!
//! # Two kinds
//!
//! **Sandbags** are the sandbag part's cover, laid: a tile of low cover
//! (`bims::sight::Sight::set_laid_cover`) a body ducks behind, on
//! **both** rooms of a docked fight — the crew's, and the residents' with
//! its mirror of the ship — said again after every relayout, join and
//! unjoin, since a fresh `Sight` starts with none. Enemies using them as
//! cover is intended. They have [`SANDBAG_HEALTH`], and every bolt a body
//! dodges behind them lands *in* them (`Game::take_cover_hits`); at
//! nothing they are gone.
//!
//! A **sentry** is a shooter with no body — `bims::combat::Sentry`,
//! handed to the crew's room every step and fired there through the one
//! trigger and the one hit calculation a Bim uses — with
//! [`SENTRY_HEALTH`] in one pool, no parts and no armour, and the auto
//! rifle's stats at the tier its owner's level gives, or a **tier-three
//! sniper rifle** with *sentry mark III*. It fires at the nearest visible
//! enemy in range for as long as it stands: **nothing in this game
//! carries ammunition** (feature 88), so there are no shots to count and
//! no refill. The enemies' nearest-target rule includes it: a station's
//! people are handed the sentries after the crew, their hits drain its
//! health, and at nought it is destroyed and removed.
//!
//! # Charges, not crafting (feature 88)
//!
//! An engineer does not make its kits: it has **charges**, and a charge
//! that has been spent comes back into its pack on a cooldown.
//! [`SANDBAG_CHARGES`] dressings' worth of sandbag kits (one more with
//! *extra bags*) at [`SANDBAG_COOLDOWN`] a charge, and
//! [`SENTRY_CHARGES`] sentry kit at [`SENTRY_COOLDOWN`] (two with
//! *second sentry*). `World::restock_kits` runs every step: while an
//! engineer holds fewer kits of a kind than its charges, a cooldown
//! runs, and when it runs out one kit goes into the pack — which is what
//! "the skill cooldown dropping to nought" means here, since the kit in
//! the pack is what the ability spends.
//!
//! **The sentry charges are also the world limit.** Laying a sentry with
//! as many standing as the engineer has charges **destroys the oldest**
//! rather than being refused, so an engineer's sentries can be moved
//! about the deck freely and never outnumber its charges. Sandbags have
//! no such limit: every bag laid stays until it is shot to pieces or
//! packed up.
//!
//! # Laying one, and taking it up
//!
//! `Command::Deploy` wants the slot's own Bim fit to act, an engineer,
//! the kit in its pack — `ResourceId::SandbagKit` or `SentryKit` — and a
//! tile of reachable deck floor that is not a door
//! or an airlock, holds no blocking part and no deployable. The Bim does
//! it as an errand (`bims::task::Kind::Deploy`): it walks beside the
//! tile and works [`DEPLOY_SANDBAG_MINUTES`] or [`DEPLOY_SENTRY_MINUTES`]
//! there, `effort` applying; a hit on it drops the errand and the kit
//! stays in the pack. The kit leaves the pack only when the work is done.
//! `Command::PackUp` is an engineer beside one taking it back as a kit,
//! counted in `World::reused_kits`: a kit laid again gives no experience.
//!
//! # Lifetime
//!
//! On the ship's deck a deployable stays until packed up or destroyed —
//! across docking, undocking, joins and unjoins — so a crew can prepare
//! for a raid while holding. On a station's deck it is lost when the
//! rooms unjoin.

use physics::ResourceId;

/// A kit in a pack: which of the two.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Kit {
    Sandbag = 0,
    Sentry = 1,
}

impl Kit {
    pub const ALL: [Kit; 2] = [Kit::Sandbag, Kit::Sentry];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<Kit> {
        Kit::ALL.get(code as usize).copied()
    }

    /// The resource the kit is in the hold and the pack.
    pub fn resource(self) -> ResourceId {
        match self {
            Kit::Sandbag => ResourceId::SandbagKit,
            Kit::Sentry => ResourceId::SentryKit,
        }
    }

    /// The kit a resource is, if it is one.
    pub fn of_resource(resource: ResourceId) -> Option<Kit> {
        Kit::ALL.into_iter().find(|k| k.resource() == resource)
    }

    /// What it lays.
    pub fn lays(self) -> DeployKind {
        match self {
            Kit::Sandbag => DeployKind::Sandbags,
            Kit::Sentry => DeployKind::Sentry,
        }
    }
}

/// What a deployable is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DeployKind {
    Sandbags = 0,
    Sentry = 1,
}

impl DeployKind {
    pub fn code(self) -> u32 {
        self as u32
    }

    /// The kit it packs up into.
    pub fn kit(self) -> Kit {
        match self {
            DeployKind::Sandbags => Kit::Sandbag,
            DeployKind::Sentry => Kit::Sentry,
        }
    }
}

/// Which deck a deployable stands on: the ship's own, or a station's by
/// its id — a berth's, a settlement's, a raider's.
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
    /// The slot whose engineer laid it: whose talents a sentry fires with.
    pub owner_slot: u32,
    pub deck: Deck,
    /// The tile of that deck's design it stands on.
    pub tile: (u32, u32),
    /// What it has left: [`SANDBAG_HEALTH`] or [`SENTRY_HEALTH`], plus or
    /// times what the owner's talents made of it when laid.
    pub health: f32,
}

/// How long laying sandbags takes, in game minutes of working steps.
pub const DEPLOY_SANDBAG_MINUTES: f64 = 4.0;
/// How long laying a sentry takes, the same way.
pub const DEPLOY_SENTRY_MINUTES: f64 = 8.0;
/// What laid sandbags can take before they are gone — each bag its own
/// pool, and gone for good at nothing.
pub const SANDBAG_HEALTH: f32 = 200.0;
/// A sentry's health: one pool, no parts, no armour.
pub const SENTRY_HEALTH: f32 = 60.0;
/// **Sandbag charges** an engineer has (feature 88): how many sandbag
/// kits its pack fills back up to, and what it sets out with. One more
/// with *extra bags*. There is no world limit on bags laid.
pub const SANDBAG_CHARGES: u32 = 3;
/// **Sentry charges**: one, two with *second sentry* — and the number of
/// sentries that engineer may have standing, a further one laid
/// destroying its oldest.
pub const SENTRY_CHARGES: u32 = 1;
/// Seconds of the clock one spent sandbag charge takes to come back.
pub const SANDBAG_COOLDOWN: f64 = 45.0;
/// Seconds of the clock a spent sentry charge takes to come back.
pub const SENTRY_COOLDOWN: f64 = 60.0;
/// Metal a repair at the workbench takes.
pub const ARMOUR_REPAIR_METAL: u32 = 1;

/// The room's `Order.recipe` for one session of an armour repair at the
/// workbench — the engineer's *armourer* talent — the way
/// `crate::UPGRADE_ORDER` is one of an upgrade: past every recipe, so
/// `finish_craft` knows it for what it is.
pub const REPAIR_ORDER: u32 = 1_001;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kit_is_its_resource_and_lays_its_kind() {
        for kit in Kit::ALL {
            assert_eq!(Kit::from_code(kit.code()), Some(kit));
            assert_eq!(Kit::of_resource(kit.resource()), Some(kit));
            assert_eq!(kit.lays().kit(), kit);
        }
        assert_eq!(Kit::of_resource(ResourceId::Metal), None);
        assert_eq!(Deck::Ship.code(), u32::MAX);
        assert_eq!(Deck::Station(7).code(), 7);
    }
}
