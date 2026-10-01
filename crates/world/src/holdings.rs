//! What the crew own, and nothing is stored anywhere (task 113).
//!
//! There is no hold for gear, no pack, no locker and no workbench. What
//! the crew own is abstract, and it is two things:
//!
//! - **The ship's holdings** ([`Holdings`]): the money — the shared pool,
//!   `World::money`, which is not kept here — the **armory**, every weapon
//!   and piece of armour nobody wears, each a [`Stored`] with an id that
//!   only climbs; and the **research keys**, a count, added the moment one
//!   is picked up. Saved, and in `world_checksum` whole.
//! - **Each Bim's loadout**: one weapon slot and one slot a part of the
//!   body (`bims::combat::Gear`, the room's, since the room's health and
//!   aim read it) — that is everything a Bim carries. Its class kits and
//!   grenades are **charges** on the body, not things (`Gear::charges`).
//!
//! A loadout changes **only between missions** — on the map and the
//! reward screen — and by the rule of who may change what
//! ([`World::may_change`](crate::World::may_change)): a player their own
//! Bim and any bot, never another player's. A thing off a slot goes into
//! the armory; a thing onto a filled slot puts the old one there. A thing
//! passes to another player's Bim only as an [`Offer`] the owner makes and
//! the recipient accepts.
//!
//! Armour is **never destroyed**: a piece at nought stays worn and stops
//! protecting for the rest of the fight, and every piece — worn or in
//! the armory — is whole again the moment the site is cleared, when the
//! ship leaves it, and at the next mission's start.

use bims::combat::{Item, Piece};

/// One slot of a loadout: the weapon, or the armour — one piece over the
/// whole body since October 2026, where there were a head, a body and
/// legs. Codes across the seam, like everything else.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GearSlot {
    Weapon = 0,
    Armour = 1,
}

impl GearSlot {
    pub const ALL: [GearSlot; 2] = [GearSlot::Weapon, GearSlot::Armour];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<GearSlot> {
        GearSlot::ALL.get(code as usize).copied()
    }

    /// The slot a thing goes on: a weapon in the hand, a piece of armour
    /// on the body. `None` for a charge, which is never a thing in a
    /// slot.
    pub fn of_item(item: Item) -> Option<GearSlot> {
        match item {
            Item::Weapon(_) => Some(GearSlot::Weapon),
            Item::Armour(_) => Some(GearSlot::Armour),
            Item::Stack(_) => None,
        }
    }

    /// What is in this slot of a loadout.
    pub fn read(self, gear: &bims::combat::Gear) -> Option<Item> {
        match self {
            GearSlot::Weapon => gear.weapon.map(Item::Weapon),
            GearSlot::Armour => gear.armour.map(Item::Armour),
        }
    }

    /// Put `item` in this slot of a loadout — or empty it with `None` —
    /// and hand back what was there. A thing that does not go on this
    /// slot is refused, `Err`, and nothing moves.
    pub fn write(
        self,
        gear: &mut bims::combat::Gear,
        item: Option<Item>,
    ) -> Result<Option<Item>, ()> {
        if let Some(item) = item
            && GearSlot::of_item(item) != Some(self)
        {
            return Err(());
        }
        let was = self.read(gear);
        match (self, item) {
            (GearSlot::Weapon, Some(Item::Weapon(w))) => gear.weapon = Some(w),
            (GearSlot::Weapon, _) => gear.weapon = None,
            (GearSlot::Armour, Some(Item::Armour(piece))) => gear.armour = Some(piece),
            (GearSlot::Armour, _) => gear.armour = None,
        }
        Ok(was)
    }
}

/// One thing in the armory: an id the armory numbers it by — only ever
/// climbing, so two clients name the same thing alike — and the thing: a
/// weapon at its tier, or a piece of armour with its tier and health.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stored {
    pub id: u32,
    pub item: Item,
}

impl Stored {
    /// Its tier's code.
    pub fn tier(&self) -> u32 {
        match self.item {
            Item::Weapon(w) => w.tier.code(),
            Item::Armour(p) => p.tier.code(),
            Item::Stack(_) => 0,
        }
    }

    /// What a piece has left; nought for a weapon, which wears nothing.
    pub fn health(&self) -> f32 {
        match self.item {
            Item::Armour(p) => p.health,
            _ => 0.0,
        }
    }

    /// The slot it goes on.
    pub fn slot(&self) -> Option<GearSlot> {
        GearSlot::of_item(self.item)
    }
}

/// A thing one player offers another (task 113): what `from`'s own Bim
/// has on `slot`, to go onto the same slot of `to`'s. It moves only when
/// `to` accepts; it is withdrawn when either side's `slot` changes, the
/// offerer takes it back, or a mission starts. Both are player slots —
/// a player's slot is its Bim's index.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Offer {
    pub from: u32,
    pub slot: GearSlot,
    pub to: u32,
}

/// Where an [`Command::Equip`](crate::Command::Equip) takes the thing
/// from: the armory by the id it is stored under, or another Bim's slot —
/// a bot's, or the player's own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GearSource {
    Armory { id: u32 },
    Worn { who: u32, slot: GearSlot },
}

/// The ship's holdings bar the money: the armory, the research keys, and
/// the offers between players standing. Saved, and in `world_checksum`
/// whole.
#[derive(Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Holdings {
    /// Every weapon and piece of armour nobody wears, in the order they
    /// came in.
    pub armory: Vec<Stored>,
    /// How many research keys the crew have picked up.
    pub keys: u32,
    /// The next id: an armory entry's, and a piece of armour's made new.
    /// Only ever climbs.
    pub next_id: u32,
    /// The offers between players standing, in the order they were made.
    pub offers: Vec<Offer>,
}

impl Holdings {
    /// Empty, the ids from one.
    pub fn new() -> Holdings {
        Holdings {
            next_id: 1,
            ..Holdings::default()
        }
    }

    /// A fresh id.
    pub fn take_id(&mut self) -> u32 {
        let id = self.next_id.max(1);
        self.next_id = id + 1;
        id
    }

    /// Put a thing in the armory; its id there. A charge is never a thing
    /// and is refused (`None`).
    pub fn put(&mut self, item: Item) -> Option<u32> {
        GearSlot::of_item(item)?;
        let id = self.take_id();
        self.armory.push(Stored { id, item });
        Some(id)
    }

    /// The thing stored under `id`.
    pub fn get(&self, id: u32) -> Option<Stored> {
        self.armory.iter().copied().find(|s| s.id == id)
    }

    /// Take the thing stored under `id` out of the armory.
    pub fn take(&mut self, id: u32) -> Option<Item> {
        let at = self.armory.iter().position(|s| s.id == id)?;
        Some(self.armory.remove(at).item)
    }

    /// A fresh piece of armour, whole at its tier, numbered off the
    /// holdings: what a purchase makes.
    pub fn new_piece(&mut self, kind: bims::combat::ArmourKind, tier: bims::combat::Tier) -> Piece {
        let id = self.take_id();
        Piece::new(id, kind, tier)
    }

    /// Every offer touching `who`'s `slot`, on either side, withdrawn:
    /// what a change to that slot does. The ones withdrawn.
    pub fn withdraw_touching(&mut self, who: u32, slot: GearSlot) -> Vec<Offer> {
        let (gone, kept) = self
            .offers
            .iter()
            .partition(|o| o.slot == slot && (o.from == who || o.to == who));
        self.offers = kept;
        gone
    }
}

/// A piece made whole again: what a mission's start does to every piece
/// there is (task 113).
pub fn mend(item: Item) -> Item {
    match item {
        Item::Armour(mut piece) => {
            piece.health = piece.stats().health;
            Item::Armour(piece)
        }
        other => other,
    }
}

/// A loadout with every piece on it made whole.
pub fn mend_gear(gear: &mut bims::combat::Gear) {
    if let Some(piece) = gear.worn_mut() {
        piece.health = piece.stats().health;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bims::combat::{ArmourKind, Gear, Tier, WeaponKind};

    #[test]
    fn a_slot_takes_only_what_goes_on_it_and_hands_back_what_was_there() {
        let mut gear = Gear::issued();
        let pistol = Item::Weapon(WeaponKind::LaserPistol.basic());
        let rifle = Item::Weapon(WeaponKind::AutoRifle.basic());
        let helm = Item::Armour(Piece::new(4, ArmourKind::Armour, Tier::One));
        assert_eq!(GearSlot::Weapon.read(&gear), Some(pistol));
        assert_eq!(
            GearSlot::Weapon.write(&mut gear, Some(rifle)),
            Ok(Some(pistol))
        );
        assert_eq!(GearSlot::Weapon.write(&mut gear, Some(helm)), Err(()));
        assert_eq!(GearSlot::Armour.write(&mut gear, Some(rifle)), Err(()));
        assert_eq!(GearSlot::Armour.write(&mut gear, Some(helm)), Ok(None));
        assert_eq!(GearSlot::Armour.write(&mut gear, None), Ok(Some(helm)));
        assert_eq!(gear.armour, None);
        for slot in GearSlot::ALL {
            assert_eq!(GearSlot::from_code(slot.code()), Some(slot));
        }
    }

    #[test]
    fn the_armory_numbers_what_it_keeps_and_a_charge_is_no_thing() {
        let mut h = Holdings::new();
        let a = h.put(Item::Weapon(WeaponKind::Shotgun.basic())).unwrap();
        let b = h.put(Item::Weapon(WeaponKind::Shotgun.basic())).unwrap();
        assert!(b > a);
        assert_eq!(h.put(Item::Stack(5)), None);
        assert!(h.take(a).is_some());
        assert!(h.take(a).is_none());
        assert_eq!(h.armory.len(), 1);
    }
}
