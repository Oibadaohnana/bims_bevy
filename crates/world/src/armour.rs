//! Armour, as the world keeps it: every piece there is, wherever it is.
//!
//! A piece of armour is two things at once, and the split is deliberate.
//! **In a container it is a resource** — `ResourceId::Helm`, `Kevlar`,
//! `LegGuard`, locker class like a medkit — so buying, selling, crafting,
//! mass and the station shelves work on it with no new mechanism. **Anywhere
//! else it is an instance**: a piece has health, loses it under fire and
//! keeps what it has lost wherever it goes next, and a count cannot say
//! that. So beside the hold's count the world keeps [`World::pieces`]
//! (`crate::World::pieces`): one [`Piece`] per piece of armour aboard, with
//! an id that only climbs, its kind, what it has left, and [`Where`] it is.
//!
//! **The invariant**: the hold's count of each armour resource is always
//! the number of pieces `at == Hold` of that kind. `World::settle_pieces`
//! holds it — every cargo change goes through `on_ship_changed`, and that
//! is where a count that has grown gets a fresh piece pushed and one that
//! has shrunk loses its most damaged piece, which is the sell rule. A
//! piece in a pack or on a body is the room's (`bims::combat::Gear`, since
//! the room's `Health` reads it), and the world's copy of it is mirrored
//! back from the room every step and after every command that moves gear,
//! so the checksum sees what the room sees.
//!
//! A **broken** piece — at nought — is still worn and does nothing, and it
//! goes nowhere: the room's `take` refuses it, so it cannot be stowed or
//! sold, only discarded. That is why no piece at `Hold` is ever broken.

use bims::combat::Piece as RoomPiece;
use bims::combat::{ArmourKind, Item, Tier, Weapon, WeaponKind};
use physics::ResourceId;

/// Where a piece of armour is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Where {
    /// In the hold, counted there as its resource.
    Hold,
    /// In a crew member's pack, in that cell of the three by three.
    Pack { who: u32, cell: u8 },
    /// On a crew member, on the part its kind is cut for.
    Worn { who: u32 },
}

impl Where {
    /// The number that goes in the checksum.
    pub fn code(self) -> u32 {
        match self {
            Where::Hold => 0,
            Where::Pack { .. } => 1,
            Where::Worn { .. } => 2,
        }
    }
}

/// One piece of armour aboard. The room's `bims::combat::Piece` is the
/// same three numbers without the `at`, and [`Piece::item`] is the one
/// turned into the other.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Piece {
    /// Only ever climbs — `World::next_piece` — so a piece is the same
    /// piece in the hold, in a pack and on a body, and two clients name
    /// the next one alike.
    pub id: u32,
    pub kind: ArmourKind,
    /// How good it is — `bims::combat::Tier`. A purchase or a bench makes
    /// tier one; the workbench combines two of a tier into one of the next
    /// (`World::upgrade`).
    pub tier: Tier,
    /// What it has left, out of `stats().health`. Kept wherever it goes.
    pub health: f32,
    pub at: Where,
}

impl Piece {
    /// A fresh piece, whole at its tier's health, in the hold: what a
    /// purchase or a bench pushes at tier one, and an upgrade above it.
    pub fn new(id: u32, kind: ArmourKind, tier: Tier) -> Piece {
        Piece {
            id,
            kind,
            tier,
            health: RoomPiece::new(id, kind, tier).health,
            at: Where::Hold,
        }
    }

    pub fn broken(&self) -> bool {
        self.health <= 0.0
    }

    /// What it is in the hold.
    pub fn resource(&self) -> ResourceId {
        resource_of(self.kind)
    }

    /// The piece as the room carries it: a pack item.
    pub fn item(&self) -> Item {
        Item::Armour(RoomPiece {
            id: self.id,
            kind: self.kind,
            tier: self.tier,
            health: self.health,
        })
    }
}

/// What a fetch takes out of a container: one particular piece; one unit
/// of a resource by its `ResourceId` code — for an armour resource the
/// least damaged piece of that kind in the hold, for a weapon the
/// highest tier of it; or one weapon of exactly that tier.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FetchKind {
    Piece(u32),
    Resource(u32),
    Tiered { resource: u32, tier: u32 },
}

/// The tiers a resource's units in the hold can differ by: a weapon by its
/// tier, in `World::guns`. `None` for anything that is not a weapon.
pub fn weapon_at(resource: ResourceId, tier: Tier) -> Option<Weapon> {
    weapon_of(resource).map(|kind| kind.at(tier))
}

/// Whose body a loot takes from: one of the crew, or one of the station's
/// people — each an index into its own room, since the two keep separate
/// rooms while docked (`crate::crew::Residents`). What a body has to be
/// to be looted is *down*, dead or out cold (`Game::is_down`), and that
/// is asked when the command lands, not when the window opened.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

/// The resource a kind of armour is in the hold. The room says the code
/// (`ArmourKind::resource`, since it does not know `physics`), and this is
/// the code looked up.
pub fn resource_of(kind: ArmourKind) -> ResourceId {
    ResourceId::ALL[kind.resource() as usize]
}

/// The kind of armour a resource is, if it is one.
pub fn kind_of(resource: ResourceId) -> Option<ArmourKind> {
    ArmourKind::ALL
        .iter()
        .copied()
        .find(|k| resource_of(*k) == resource)
}

/// The resource a weapon is in the hold: the armoury's handgun *is* the
/// laser pistol everybody is issued, and the four after it are their
/// own. The room says the code (`WeaponKind::resource`, since it does
/// not know `physics`), and this is the code looked up — one table, the
/// room's, rather than a second one here to drift from it.
pub fn weapon_resource(weapon: WeaponKind) -> ResourceId {
    ResourceId::ALL[weapon.resource() as usize]
}

/// The weapon a resource is, if it is one.
pub fn weapon_of(resource: ResourceId) -> Option<WeaponKind> {
    WeaponKind::ALL
        .iter()
        .copied()
        .find(|w| weapon_resource(*w) == resource)
}

/// Whether a resource is something worn or held — a piece of armour or a
/// weapon — which the shelves take as well as the lockers.
pub fn is_gear(resource: ResourceId) -> bool {
    kind_of(resource).is_some() || weapon_of(resource).is_some()
}

/// The tier of research key a resource is, if it is one: the tier-one key
/// is the one there is. The room carries a key as `Item::Key(tier)` — two
/// cells tall — and the hold counts it as this resource.
pub fn key_tier_of(resource: ResourceId) -> Option<u8> {
    match resource {
        ResourceId::ResearchKey => Some(1),
        _ => None,
    }
}

/// The resource a research key of `tier` is in the hold, if the tier has
/// one.
pub fn key_resource(tier: u8) -> Option<ResourceId> {
    match tier {
        1 => Some(ResourceId::ResearchKey),
        _ => None,
    }
}
/// What one unit of a resource is as a pack item: a weapon by its kind at
/// tier one (the hold's tiers are `World::guns`, and a fetch reads them), a
/// What one unit of a resource is as a pack item: a weapon by its kind, a
/// research key by its tier, anything else a stack of one. A piece of
/// armour is never made this way — it has an id and a health, and comes
/// out of [`Piece::item`].
pub fn item_of(resource: ResourceId) -> Item {
    if let Some(weapon) = weapon_of(resource) {
        return Item::Weapon(weapon.basic());
    }
    if let Some(tier) = key_tier_of(resource) {
        return Item::Key(tier);
    }
    Item::Stack(resource as u32)
}

/// What a pack item is in the hold: the resource one unit of it counts
/// as. `None` for a resource code the hold does not know.
pub fn resource_of_item(item: Item) -> Option<ResourceId> {
    match item {
        Item::Armour(piece) => Some(resource_of(piece.kind)),
        Item::Weapon(weapon) => Some(weapon_resource(weapon.kind)),
        Item::Stack(code) => ResourceId::ALL.get(code as usize).copied(),
        Item::Key(tier) => key_resource(tier),
    }
}
