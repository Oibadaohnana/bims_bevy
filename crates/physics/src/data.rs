//! The numbers, kept apart from the arithmetic that uses them.
//!
//! Everything here is a **placeholder**. Nothing in this file has been
//! balanced against anything; the values exist so the functions in `lib.rs`
//! have something to be exercised with, and so the world generator has a
//! reference ship to measure its layouts against. Expect all of them to move.
//!
//! What must *not* move quietly is the relationship between them. The world
//! generator validates every system layout by asking how many days a trip
//! takes, and that answer is built out of these masses. Changing one changes
//! which layouts are legal, which changes the world a seed produces — so a
//! change here is a `generator_version` bump, not a tweak.

/// What a crew member weighs, engines included in nothing: a body on the ship
/// is mass the engines have to push and that is the whole of its role here.
pub const PLAYER_MASS: f64 = 100.0;

/// The floor under a ship's hull and structure.
///
/// A ship with no mass accelerates infinitely, so the contract refuses one.
/// This is the configurable floor rather than a clamp: a hull below it is a
/// validation error and the caller is told, because a ship that weighs
/// nothing is a bug upstream and quietly rounding it up would hide that.
pub const MIN_HULL_MASS: f64 = 1.0;

/// What a ship can carry, and what the crew fight, eat and heal with.
/// Eighteen, **no production chains and no recipes**: since the money
/// rework (feature 95) nothing is mined and nothing is refined, so there
/// are no materials at all — an id and what a unit of it weighs is the
/// whole of what this crate needs. A part's weight is
/// `shipdesign::PartDef::mass`, a fact about the part, and this crate
/// deliberately knows nothing about parts.
///
/// The first two are food. Food is here rather than somewhere of its own
/// because it is **cargo**: it is bought at a station, it is stowed in a
/// cold store, and the engines have to push it like anything else. What
/// makes it food rather than a gun is where it is stowed and what a Bim
/// does with it, and neither of those is this crate's business — see
/// `economy::storage`.
///
/// The discriminants are written out because they cross the wasm boundary
/// as numbers one day, and a reordered enum must not silently renumber a
/// save. A new resource is appended. (Fuel was `2` until September 2026,
/// when the ship went over to reactor power; the eight materials — ore,
/// metal, components, galvum, emitters, rock, fibre and the armoury's
/// vest — went with the money rework, and the rest were closed up rather
/// than leaving eight holes in every table indexed by this. No save has
/// to load, so nothing was owed a hole.)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ResourceId {
    Vegetable = 0,
    Tofu = 1,
    /// A pressure suit. Worn for a walk outside — building beyond the
    /// hull — and kept in a suit locker, which is the locker class of
    /// storage. A resource rather than a thing with a state of its own,
    /// until a suit wears out.
    Suit = 2,
    /// A laser handgun. What a Bim will carry into a fight. Bought, never
    /// made.
    Handgun = 3,
    /// A medkit: two vegetables at the drug lab, the one thing the crew
    /// still make. What treating a wound will use up.
    Medkit = 4,
    /// A bandage: gauze and tape, bought at a desk like everything else.
    /// What closes a wound; used up on the wound. Kept in a locker like
    /// the medkit, five to a box.
    Bandage = 5,
    /// A basic helm. Armour for the head — the first of three pieces a
    /// Bim wears, one to a part of the body. A resource **in a
    /// container** and a thing with a health of its own everywhere else
    /// (`bims::combat::Piece`; the world keeps the two in step), so
    /// buying, selling and combining it need no new mechanism.
    Helm = 6,
    /// Basic kevlar. Armour for the body.
    Kevlar = 7,
    /// Basic leg guards. Armour for the legs.
    LegGuard = 8,
    /// A shotgun: the second weapon after the handgun; like it, a
    /// resource in a container and a `bims::combat::WeaponKind` in a
    /// hand, mapped one to one by `WeaponKind::resource`.
    Shotgun = 9,
    /// An auto rifle.
    AutoRifle = 10,
    /// A sniper rifle.
    SniperRifle = 11,
    /// The schword, a blade with a laser edge. The one melee weapon.
    Schword = 12,
    /// A tier-one research key: an artifact found on a friendly station's
    /// research desk, carried off in a pack — where it takes two cells,
    /// one over the other — and put into the crew's own research desk,
    /// which holds exactly one. Consumed there to open the research tree
    /// locked behind it (`shipdesign::research`). A resource so the desk
    /// counts it the way a locker counts a medkit; sold nowhere, and a
    /// station buys one for the curiosity.
    ResearchKey = 13,
    /// A tier-two research key: the same slab, found on the research desk
    /// of a **hostile** station — every one that is not a derelict — and
    /// carried off the same way, two cells in a pack, one in the crew's
    /// desk. Consumed there to open the tier-two node
    /// (`shipdesign::research::Node::Upgrades`).
    ResearchKeyTwo = 14,
    /// An engineer's sandbag kit (feature 74, `world::deploy`): sacks and
    /// a frame, carried in the pack and laid as a deployable on a deck
    /// tile — never a part of the ship. Since feature 88 an engineer's
    /// own come back on a cooldown rather than being made or bought.
    SandbagKit = 15,
    /// An engineer's sentry kit: a turret in a crate. The same rules as
    /// the sandbag kit's.
    SentryKit = 16,
    /// A soldier's grenade (feature 75, `world::class`), carried one to a
    /// pack cell and thrown by a soldier alone; since feature 90 a charge
    /// on a cooldown like the engineer's kits.
    Grenade = 17,
}

impl ResourceId {
    /// Every resource, in discriminant order. `ALL[id as usize].id == id`,
    /// which [`ResourceId::def`] relies on and [`defs_are_sound`] checks.
    pub const ALL: [ResourceId; 18] = [
        ResourceId::Vegetable,
        ResourceId::Tofu,
        ResourceId::Suit,
        ResourceId::Handgun,
        ResourceId::Medkit,
        ResourceId::Bandage,
        ResourceId::Helm,
        ResourceId::Kevlar,
        ResourceId::LegGuard,
        ResourceId::Shotgun,
        ResourceId::AutoRifle,
        ResourceId::SniperRifle,
        ResourceId::Schword,
        ResourceId::ResearchKey,
        ResourceId::ResearchKeyTwo,
        ResourceId::SandbagKit,
        ResourceId::SentryKit,
        ResourceId::Grenade,
    ];

    pub fn def(self) -> &'static ResourceDef {
        &RESOURCES[self as usize]
    }

    /// What one unit of it weighs. The one number the mass function wants.
    pub fn mass_per_unit(self) -> f64 {
        self.def().mass_per_unit
    }
}

/// A resource, as data. There is deliberately nothing else on it: no name (no
/// strings cross the wasm boundary — the host holds the words), no recipe, no
/// stack size.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResourceDef {
    pub id: ResourceId,
    /// Strictly greater than zero. A resource that weighs nothing would let a
    /// ship carry an unbounded cargo for free.
    pub mass_per_unit: f64,
}

/// The table. A crate of vegetables and a block of tofu are light — a
/// week's meals for two weighs less than one girder of the hull, which is
/// the relation that matters — and the guns and armour are heavy in
/// proportion to what they are.
///
/// The one made thing is pinned to its recipe rather than to taste:
/// crafting conserves mass, so a medkit weighs exactly the two vegetables
/// that went into it. `shipdesign::recipes` is where that recipe lives
/// and `every_recipe_holds_together` there is what holds this column to
/// it.
pub static RESOURCES: [ResourceDef; 18] = [
    ResourceDef {
        id: ResourceId::Vegetable,
        mass_per_unit: 0.5,
    },
    ResourceDef {
        id: ResourceId::Tofu,
        mass_per_unit: 0.5,
    },
    ResourceDef {
        id: ResourceId::Suit,
        mass_per_unit: 6.0,
    },
    ResourceDef {
        id: ResourceId::Handgun,
        mass_per_unit: 20.0,
    },
    // Two vegetables at the drug lab, and it weighs them.
    ResourceDef {
        id: ResourceId::Medkit,
        mass_per_unit: 1.0,
    },
    ResourceDef {
        id: ResourceId::Bandage,
        mass_per_unit: 2.0,
    },
    // The three pieces of armour: a helm, a kevlar vest, a pair of leg
    // guards.
    ResourceDef {
        id: ResourceId::Helm,
        mass_per_unit: 16.0,
    },
    ResourceDef {
        id: ResourceId::Kevlar,
        mass_per_unit: 28.0,
    },
    ResourceDef {
        id: ResourceId::LegGuard,
        mass_per_unit: 8.0,
    },
    // The four weapons after the handgun. Heavy, because a laser's
    // emitter is.
    ResourceDef {
        id: ResourceId::Shotgun,
        mass_per_unit: 36.0,
    },
    ResourceDef {
        id: ResourceId::AutoRifle,
        mass_per_unit: 46.0,
    },
    ResourceDef {
        id: ResourceId::SniperRifle,
        mass_per_unit: 68.0,
    },
    ResourceDef {
        id: ResourceId::Schword,
        mass_per_unit: 42.0,
    },
    // A research key is a slab of somebody else's circuitry: light, and
    // made of nothing the crew know.
    ResourceDef {
        id: ResourceId::ResearchKey,
        mass_per_unit: 2.0,
    },
    ResourceDef {
        id: ResourceId::ResearchKeyTwo,
        mass_per_unit: 2.0,
    },
    // The engineer's kits and the soldier's grenade.
    ResourceDef {
        id: ResourceId::SandbagKit,
        mass_per_unit: 8.0,
    },
    ResourceDef {
        id: ResourceId::SentryKit,
        mass_per_unit: 36.0,
    },
    ResourceDef {
        id: ResourceId::Grenade,
        mass_per_unit: 10.0,
    },
];

/// Whether the table above holds together: one entry per id, in order, each
/// weighing something. The unit tests assert it, and so can anything that
/// loads a resource table from somewhere else later.
pub fn defs_are_sound() -> bool {
    RESOURCES.len() == ResourceId::ALL.len()
        && ResourceId::ALL.iter().enumerate().all(|(i, &id)| {
            RESOURCES[i].id == id
                && RESOURCES[i].mass_per_unit > 0.0
                && RESOURCES[i].mass_per_unit.is_finite()
        })
}
