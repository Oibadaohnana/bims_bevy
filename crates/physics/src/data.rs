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

/// What a ship can be built from and carry. Six, no production chains, and
/// no recipe **here** — an id and what a unit of it weighs is the whole of
/// what this crate needs. What a given part is made of is
/// `shipdesign::PartDef::recipe`, which is a fact about a part, and this
/// crate deliberately knows nothing about parts. A part weighs its recipe
/// added up out of the masses below, so building one moves mass from the hold
/// into the hull without changing the total.
///
/// The first four are materials and the last two are food. Food is here
/// rather than somewhere of its own because it is **cargo**: it is bought at
/// a station, it is stowed in a cold store, and the engines have to push it
/// like anything else. What makes it food rather than metal is where it is
/// stowed and what a Bim does with it, and neither of those is this crate's
/// business — see `economy::storage`.
///
/// The discriminants are written out because they cross the wasm boundary as
/// numbers one day, and a reordered enum must not silently renumber a save.
/// A new resource is appended. (Fuel was `2` until September 2026, when
/// the ship went over to reactor power — there is no fuel for spacecraft —
/// and, with no save format yet, the rest were closed up rather than
/// leaving a hole in every table indexed by this.)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ResourceId {
    Ore = 0,
    Metal = 1,
    Components = 2,
    Vegetable = 3,
    Tofu = 4,
    /// The rare one. Sold only at mining outposts, mined only from rich
    /// belts, and wanted only by the emitter — so a crew that never fights
    /// never needs any, and a crew that does has a reason to visit a belt.
    Galvum = 5,
    /// The rare tier of component: metal, components and galvum at a
    /// workbench, and what a turret, a shield, a mining laser and a sensor
    /// array are made of. Made, never sold.
    Emitter = 6,
    /// A pressure suit. Worn for a walk outside — mining ore off a belt —
    /// and kept in a suit locker, which is the locker class of storage. A
    /// resource rather than a thing with a state of its own, until a suit
    /// wears out.
    Suit = 7,
    /// A laser handgun: two components and an emitter at the armoury.
    /// What a Bim will carry into a fight. Made, never sold.
    Handgun = 8,
    /// A vest: metal and components at the armoury. Worn into a fight.
    /// Made, never sold.
    Vest = 9,
    /// A medkit: vegetables and a component at the armoury. What treating a
    /// wound will use up.
    Medkit = 10,
    /// Bare rock: what the outside of an asteroid is made of, and what a
    /// pick brings back from it until it is through to the ore. Worth
    /// almost nothing and heavier than anything — a shelf of it is ballast.
    Rock = 11,
    /// A crop: what the bay grows besides food, and what a bandage is made
    /// of. Stowed cold like the vegetables, because it is a plant, and
    /// worth nothing to anyone but a drug lab.
    Fibre = 12,
    /// A bandage: two fibre at the drug lab. What closes a wound; used up
    /// on the wound. Kept in a locker like the medkit.
    Bandage = 13,
    /// A basic helm: two metal at the workbench. Armour for the head —
    /// the first of three pieces a Bim wears, one to a part of the body.
    /// A resource **in a container** and a thing with a health of its own
    /// everywhere else (`bims::combat::Piece`; the world keeps the two in
    /// step), so buying, selling and crafting it need no new mechanism.
    /// Made, never sold, like the vest.
    Helm = 14,
    /// Basic kevlar: three metal and a galvum at the workbench. Armour for
    /// the body. Not the vest, which is the armoury's and older.
    Kevlar = 15,
    /// Basic leg guards: one metal at the workbench. Armour for the legs.
    LegGuard = 16,
    /// A shotgun: four metal and two components at the armoury. The second
    /// weapon after the handgun; like it, a resource in a container and a
    /// `bims::combat::WeaponKind` in a hand, mapped one to one by
    /// `WeaponKind::resource`. Made, never sold.
    Shotgun = 17,
    /// An auto rifle: three metal, three components and an emitter at the
    /// armoury. Made, never sold.
    AutoRifle = 18,
    /// A sniper rifle: four metal, two components and two emitters at the
    /// armoury. Made, never sold.
    SniperRifle = 19,
    /// The schword, a blade with a laser edge: one metal, one component and
    /// two emitters at the armoury. The one melee weapon. Made, never sold.
    Schword = 20,
    /// A tier-one research key: an artifact found on a friendly station's
    /// research desk, carried off in a pack — where it takes two cells,
    /// one over the other — and put into the crew's own research desk,
    /// which holds exactly one. Consumed there to open the research tree
    /// locked behind it (`shipdesign::research`). A resource so the desk
    /// counts it the way a locker counts a medkit; made nowhere and sold
    /// nowhere, and a station buys one for the curiosity.
    ResearchKey = 21,
    /// A tier-two research key: the same slab, found on the research desk
    /// of a **hostile** station — every one that is not a derelict — and
    /// carried off the same way, two cells in a pack, one in the crew's
    /// desk. Consumed there to open the tier-two node
    /// (`shipdesign::research::Node::Upgrades`).
    ResearchKeyTwo = 22,
    /// An engineer's sandbag kit (feature 74, `world::deploy`): one bar of
    /// metal's worth of sacks and frame, made at the workbench, carried
    /// in the pack and laid as a deployable on a deck tile — never a part
    /// of the ship. Sold nowhere; a station buys one back.
    SandbagKit = 23,
    /// An engineer's sentry kit: a turret in a crate, the auto rifle's
    /// worth of emitter and action on two bars of metal. The same rules
    /// as the sandbag kit's.
    SentryKit = 24,
    /// A soldier's grenade (feature 75, `world::class`): a bar of metal
    /// and a component's worth of fuse, made at the armoury, carried one
    /// to a pack cell and thrown by a soldier alone — anybody may make,
    /// carry and trade one. Sold nowhere; a station buys one back.
    Grenade = 25,
}

impl ResourceId {
    /// Every resource, in discriminant order. `ALL[id as usize].id == id`,
    /// which [`ResourceId::def`] relies on and [`defs_are_sound`] checks.
    pub const ALL: [ResourceId; 26] = [
        ResourceId::Ore,
        ResourceId::Metal,
        ResourceId::Components,
        ResourceId::Vegetable,
        ResourceId::Tofu,
        ResourceId::Galvum,
        ResourceId::Emitter,
        ResourceId::Suit,
        ResourceId::Handgun,
        ResourceId::Vest,
        ResourceId::Medkit,
        ResourceId::Rock,
        ResourceId::Fibre,
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

/// The table. Ore is the raw rock, metal is what it refines to, and
/// components are light and fiddly. A crate of
/// vegetables and a block of tofu are lighter again — a week's meals for two
/// weighs less than one girder, which is the relation that matters.
///
/// The two made things are pinned to their recipes rather than to taste:
/// crafting conserves mass, so an emitter weighs exactly the metal, the two
/// components and the galvum that went into it, and four components weigh
/// one metal. `shipdesign::recipes` is where those recipes live and
/// `every_recipe_conserves_mass` there is what holds this column to them.
pub static RESOURCES: [ResourceDef; 26] = [
    ResourceDef {
        id: ResourceId::Ore,
        mass_per_unit: 10.0,
    },
    ResourceDef {
        id: ResourceId::Metal,
        mass_per_unit: 8.0,
    },
    ResourceDef {
        id: ResourceId::Components,
        mass_per_unit: 2.0,
    },
    ResourceDef {
        id: ResourceId::Vegetable,
        mass_per_unit: 0.5,
    },
    ResourceDef {
        id: ResourceId::Tofu,
        mass_per_unit: 0.5,
    },
    ResourceDef {
        id: ResourceId::Galvum,
        mass_per_unit: 4.0,
    },
    ResourceDef {
        id: ResourceId::Emitter,
        mass_per_unit: 16.0,
    },
    ResourceDef {
        id: ResourceId::Suit,
        mass_per_unit: 6.0,
    },
    // The three the armoury makes weigh their recipes, like the emitter.
    ResourceDef {
        id: ResourceId::Handgun,
        mass_per_unit: 20.0,
    },
    ResourceDef {
        id: ResourceId::Vest,
        mass_per_unit: 36.0,
    },
    ResourceDef {
        id: ResourceId::Medkit,
        mass_per_unit: 3.0,
    },
    ResourceDef {
        id: ResourceId::Rock,
        mass_per_unit: 12.0,
    },
    // A crop, a bale of it to the unit, so heavier than a crate of
    // vegetables; and the bandage weighs the two fibre it is rolled from,
    // like everything else that is made.
    ResourceDef {
        id: ResourceId::Fibre,
        mass_per_unit: 1.0,
    },
    ResourceDef {
        id: ResourceId::Bandage,
        mass_per_unit: 2.0,
    },
    // The three pieces of armour weigh their recipes at the workbench: two
    // metal, three metal and a galvum, one metal.
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
    // The four weapons after the handgun weigh their armoury recipes, like
    // it: four metal and two components; three metal, three components and
    // an emitter; four metal, two components and two emitters; one metal,
    // one component and two emitters. Heavy, because an emitter is.
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
    // made of nothing the crew know, so no recipe holds it to anything.
    ResourceDef {
        id: ResourceId::ResearchKey,
        mass_per_unit: 2.0,
    },
    ResourceDef {
        id: ResourceId::ResearchKeyTwo,
        mass_per_unit: 2.0,
    },
    // The engineer's kits weigh what went into them: a sandbag kit one
    // metal, a sentry kit two metal, two components and an emitter.
    ResourceDef {
        id: ResourceId::SandbagKit,
        mass_per_unit: 8.0,
    },
    ResourceDef {
        id: ResourceId::SentryKit,
        mass_per_unit: 36.0,
    },
    // A grenade weighs what went into it: a bar of metal and a component.
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
