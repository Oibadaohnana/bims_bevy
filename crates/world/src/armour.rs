//! Gear as the trade and the hold's old tables name it: which
//! `ResourceId` a piece of armour or a weapon is, and back.
//!
//! Since task 113 nothing is stored: a piece or a gun is either on a Bim
//! (its loadout, `bims::combat::Gear`) or in the ship's armory
//! (`crate::holdings`). A **resource** is only what a desk quotes and
//! sells — so buying a helm is buying `ResourceId::Armour` at a tier, and
//! what arrives is a piece in the armory. This module is the one table
//! between the two ways of naming a thing.

use bims::combat::{ArmourKind, Item, Tier, Weapon, WeaponKind};
use physics::ResourceId;

/// The tiers a weapon resource can be bought at: a weapon by its tier.
/// `None` for anything that is not a weapon.
pub fn weapon_at(resource: ResourceId, tier: Tier) -> Option<Weapon> {
    weapon_of(resource)
        .filter(|kind| kind.made_at(tier))
        .map(|kind| kind.at(tier))
}

/// Whose body a command points at: one of the crew, or one of the
/// station's people — each an index into its own room, since the two
/// keep separate rooms while docked (`crate::crew::Residents`). What a
/// hire, the commander's attack order and a body's position name. (It was
/// a loot's source until task 113 took the looting away.)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LootSource {
    Crew(u32),
    Resident(u32),
}

impl LootSource {
    /// The kind, as a number: what `WorldEvent::Looted` carries.
    pub fn code(self) -> u32 {
        match self {
            LootSource::Crew(_) => 0,
            LootSource::Resident(_) => 1,
        }
    }

    /// The index into the source's own room.
    pub fn who(self) -> u32 {
        match self {
            LootSource::Crew(who) | LootSource::Resident(who) => who,
        }
    }

    /// The source a kind code and an index name; `None` for a code that
    /// is neither.
    pub fn from_code(kind: u32, who: u32) -> Option<LootSource> {
        match kind {
            0 => Some(LootSource::Crew(who)),
            1 => Some(LootSource::Resident(who)),
            _ => None,
        }
    }
}

/// The resource a kind of armour is at the desk. The room says the code
/// (`ArmourKind::resource`, since it does not know `physics`), and this is
/// the code looked up.
pub fn resource_of(kind: ArmourKind) -> ResourceId {
    ResourceId::from_code(kind.resource()).expect("every armour kind is a resource")
}

/// The kind of armour a resource is, if it is one.
pub fn kind_of(resource: ResourceId) -> Option<ArmourKind> {
    ArmourKind::ALL
        .iter()
        .copied()
        .find(|k| resource_of(*k) == resource)
}

/// The resource a weapon is at the desk: the armoury's handgun *is* the
/// laser pistol everybody is issued, and the four after it are their
/// own. The room says the code (`WeaponKind::resource`, since it does
/// not know `physics`), and this is the code looked up — one table, the
/// room's, rather than a second one here to drift from it.
/// A droid's built-in arm is no resource at all (feature 83) and never
/// reaches this: nothing the armory or a slot can hold is one, which is what `WeaponKind::carried` says and
/// `no_built_in_arm_is_ever_a_thing` pins. Asked for one anyway, the
/// answer is `Nothing`, the way an empty slot is everywhere else.
pub fn weapon_resource(weapon: WeaponKind) -> ResourceId {
    let code = weapon
        .resource()
        .expect("a built-in arm is never a resource; see WeaponKind::carried");
    ResourceId::from_code(code).expect("every carried weapon is a resource")
}

/// The weapon a resource is, if it is one.
pub fn weapon_of(resource: ResourceId) -> Option<WeaponKind> {
    WeaponKind::ALL
        .iter()
        .copied()
        .find(|w| weapon_resource(*w) == resource)
}

/// Whether a resource is something worn or held — a piece of armour or a
/// weapon — which is what the armory keeps and a slot holds, where
/// everything else the desk sells is a count on the ship.
pub fn is_gear(resource: ResourceId) -> bool {
    kind_of(resource).is_some() || weapon_of(resource).is_some()
}

/// What a thing is at the desk: the resource one of it counts as. `None`
/// for a resource code the trade does not know.
pub fn resource_of_item(item: Item) -> Option<ResourceId> {
    match item {
        Item::Armour(piece) => Some(resource_of(piece.kind)),
        Item::Weapon(weapon) => Some(weapon_resource(weapon.kind)),
        Item::Stack(code) => ResourceId::from_code(code),
        // An item is no resource of the desk's (October 2026).
        Item::Module(_) => None,
    }
}
