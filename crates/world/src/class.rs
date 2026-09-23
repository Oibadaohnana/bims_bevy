//! Classes: what a player's crew member is, and what it learns (features
//! 74, 75, 76, 77, 78 and 88).
//!
//! A crew member has **one class**, chosen by its player before the game
//! opens — a [`Class`] per player slot, kept on the world with the start
//! like the seed and changeable by `Command::SetClass` until the ship
//! first leaves its berth — and one [`Progress`] through it: experience,
//! the level that makes, and the talents picked on the way up. A
//! crew member nobody steers, a hire, a station's resident has
//! [`Class::None`] and learns nothing.
//!
//! **A class owns abilities, never jobs or money.** Every Bim can do
//! every job, take every errand, place every site and use every weapon,
//! and every Bim brings the same money to the pool whatever its class.
//! A class only adds its own abilities — its two keys and its talents —
//! which no other class can use: [`can`] is the one rule, and it answers
//! for an [`Ability`] and nothing else.
//!
//! # Nothing is crafted for an ability (feature 90)
//!
//! What an ability spends is a [`Charge`]: a thing the class brings in
//! its pack that comes back on a cooldown of its own. A player never
//! stands at a bench or at a dock to use a skill — the engineer's
//! sandbags and sentry since feature 88, the soldier's grenades since
//! this one ([`GRENADE_CHARGES`] of them at [`GRENADE_COOLDOWN`] a
//! charge) — and what a class added later spends goes in [`Charge`]
//! beside them. The abilities that spend nothing are held instead of
//! thrown — a brace, a beam, a bulwark, a squad order — and the ones
//! that are neither wait out a cooldown of their own: the tank's taunt
//! at [`TAUNT_COOLDOWN`], the commander's rally at [`RALLY_COOLDOWN`],
//! the medic's surge on its charge.
//!
//! # Experience
//!
//! Three things give it, and nothing else:
//!
//! | what | xp | who |
//! |---|---|---|
//! | an enemy goes down within [`VICINITY_TILES`] | [`XP_ENEMY_DOWN`] | every classed crew member in range |
//! | an enemy dies within it | [`XP_ENEMY_DEAD`] | the same |
//! | a construction site finishes, or a kit is laid, by an engineer or anybody within its vicinity | [`XP_BUILT`] | that engineer alone |
//! | a medic finishes bandaging a crewmate, or treating a crewmate's trauma | [`XP_HEALED`] | that medic alone |
//! | an enemy's shot or blow lands on a tank | one a [`TANK_HITS_PER_XP`] | that tank alone |
//! | a hire goes through from the slot steering a commander | [`XP_HIRE`] | that commander alone |
//!
//! Each enemy counts once for going down and once for dying; a crewmate
//! or a mercenary going down gives nothing; a kit laid from a re-used one
//! (`World::reused_kits`, a kit packed up) gives nothing. The
//! vicinity is measured on the deck the fight is on, between the crew
//! member and the enemy, or the crew member and whoever built. The
//! soldier has no source of its own. The medic's counts when the task
//! finishes and the bandage or the kit is used (a field surgery too),
//! once a task, for a crewmate — any crew member but itself,
//! mercenaries included — and never for a non-medic doing the same.
//!
//! The tank's is the one source worth a fraction of a point, and
//! experience is a whole number, so the **count** is what is kept —
//! `hits_taken` on the Bim, saved and checksummed — and every
//! [`TANK_HITS_PER_XP`] hits are one point, with the count starting
//! again. A hit counts when it *lands*: after the roll and any dodge,
//! whether his armour, a surge or his body took it. A miss, a dodge and
//! a hit from his own side (his soldier's grenade) count for nothing,
//! and so does a hit on anybody who is not a tank.
//!
//! The commander's counts when `Command::Hire` sent from the slot
//! steering him goes through and the body joins the crew. A refused hire
//! gives nothing, and a hire sent from anybody else's slot gives him
//! nothing, however near he stands.
//!
//! # Levels
//!
//! Ten, off cumulative experience ([`LEVEL_XP`]): 100 for the second,
//! 250 for the third, up to 3 200 for the tenth. A level reached is
//! `WorldEvent::LevelUp`, said once. A **fixed** level's talent applies at
//! once; a **pick** level ([`pick_at`]) offers two and applies neither
//! until the player chooses — `Command::PickTalent`, only for a level
//! reached with no pick yet, never changed after. A dead crew member's
//! level, experience and picks die with it. Every class climbs the same
//! shape — fixed at one, three and seven, a pick at the rest
//! ([`is_pick_level`]) — and what each level *is* is the class's.
//!
//! # The engineer's ten levels
//!
//! | level | left | right |
//! |---|---|---|
//! | 1 | three sandbag charges, laid; packs deployables up | — |
//! | 2 | *Reinforced sand*: +50 sandbag health | *Site foreman*: build effort ×1.25 |
//! | 3 | *Sentry*: one sentry charge | — |
//! | 4 | *Sandbagger*: sandbag deploy time ×0.5 | *Bulk bags*: one charge lays two adjacent tiles |
//! | 5 | *Armoured sentry*: sentry health ×1.5 | *Enhanced optics*: sentry fire range +10 tiles |
//! | 6 | *Armourer*: repairs armour at the workbench | *Higher quality armour*: his worn armour +5% health, +1 protection |
//! | 7 | *Sentry mark II*: the tier-two factors on its rifle | — |
//! | 8 | *Dug in*: sandbags anywhere between a sentry and the shooter are cover | *Quick build*: sentry deploy time ×0.5 |
//! | 9 | *Extra bags*: one more sandbag charge | *Steady hands*: a hit no longer interrupts a deploy |
//! | 10 | *Second sentry*: two sentry charges | *Sentry mark III*: a tier-three sniper rifle at double the rate and a fifth more damage |
//!
//! # The soldier's ten levels
//!
//! | level | left | right |
//! |---|---|---|
//! | 1 | *Brace* (E): holds its ground, shoots steadier | — |
//! | 2 | *Marksman*: accuracy ×1.15 | *Point blank*: damage within the weapon's sweet range ×1.2 |
//! | 3 | *Grenades* (Q): two charges, 30 seconds each | — |
//! | 4 | *Runner*: pace ×1.2 while an enemy is in sight | *Steady aim*: the walking accuracy penalty halved |
//! | 5 | *Iron nerve*: never flees | *Cover master*: the odds in cover ×1.5 |
//! | 6 | *Long throw*: grenade range ×1.5 | *Short fuse*: grenade fuse ×0.5 |
//! | 7 | *Drill*: fire rate ×1.2 on every weapon | — |
//! | 8 | *Frag*: grenade radius ×1.5 | *Quick draw*: grenade cooldown ×0.5 |
//! | 9 | *Bruiser*: melee damage ×1.5, fists and schword | *Dug in*: dodge +10% while braced |
//! | 10 | *Deadeye*: every weapon's far accuracy equals its near | *Rampage*: each enemy downed raises the fire rate ×1.1, up to three, until the fight ends |
//!
//! # The medic's ten levels
//!
//! | level | left | right |
//! |---|---|---|
//! | 1 | *Heal beam* (E): holds a crewmate's blood | — |
//! | 2 | *Field dressing*: bandages in half the time | *Surgeon*: treats in half the time |
//! | 3 | *Surge* (Q): may trigger it | — |
//! | 4 | *Long beam*: beam range ×1.5 | *Strong beam*: beam blood rate ×1.5 |
//! | 5 | *Clean hands*: a trauma it treats leaves nothing lasting | *Steady hands*: a part it treats comes back to ×1.5 of `TREATED_TO` |
//! | 6 | *Quick charge*: the surge charges ×1.5 faster | *Long surge*: a surge lasts ×1.5 |
//! | 7 | *Mender*: a beamed patient's parts mend at `HEALTH_RECOVER` ×10 | — |
//! | 8 | *Self-care*: its own wounds do not bleed while it beams | *Double link*: two patients at once, each at the full rate |
//! | 9 | *Gunner medic*: fires while beaming, at fire rate ×0.5 | *Closing surge*: a surge ending closes every open wound on the patient |
//! | 10 | *Mass surge*: a surge covers every crew member within 3 tiles of the patient | *Field surgeon*: once a fight, treats a trauma with no medkit in half the time |
//!
//! # The tank's ten levels
//!
//! | level | left | right |
//! |---|---|---|
//! | 1 | armour drains at half rate on him; *Bulwark* (E) | — |
//! | 2 | *Pack mule*: carries two loads a trip when hauling | *Plated*: armour protection ×1.5 on him |
//! | 3 | *Taunt* (Q): may use it | — |
//! | 4 | *Breacher*: forces locked doors in half the time | *Unmovable*: never flees, and loses no pace to low blood while his kevlar holds |
//! | 5 | *Wide wall*: bulwark reach ×2 | *Fast wall*: bulwark pace ×1.5 |
//! | 6 | *Loud taunt*: taunt radius ×1.5 | *Long taunt*: a taunt lasts ×1.5 |
//! | 7 | *Iron frame*: a hit rolled on his head lands on his body | — |
//! | 8 | *Hold fast*: his wounds do not bleed while he taunts | *Guarded*: dodge +10% while Bulwark is on |
//! | 9 | *Interpose*: a bolt that would hit somebody he shields hits him | *Magnet*: a taunt turns every charging blade toward him |
//! | 10 | *Fortress*: armour drain on him ×0.5 again, a quarter in all | *Rallying wall*: while he taunts, crew within 3 tiles drain at half rate too |
//!
//! # The commander's ten levels
//!
//! | level | left | right |
//! |---|---|---|
//! | 1 | the aura; hires at a quarter off; squad orders — attack, fall back, stand ground | — |
//! | 2 | *Wide presence*: aura radius ×1.5 | *Strong presence*: each aura bonus ×1.5 |
//! | 3 | *Rally* (Q): may call it | — |
//! | 4 | *Haggler*: hires at two fifths off | *Outfitter*: a mercenary he hires arrives with one basic piece it lacks |
//! | 5 | *Focus fire*: the squad's odds against the marked enemy ×1.15 | *Pincer*: an attack may mark two enemies, the squad split between them |
//! | 6 | *Long rally*: a rally lasts ×1.5 | *Quick rally*: the rally cooldown ×0.5 |
//! | 7 | *Long reach*: a squad order reaches every squad member in the room | — |
//! | 8 | *Steady ranks*: Bims in his aura bleed ×0.75 | *Double time*: Bims in his aura walk at pace ×1.1 |
//! | 9 | *Relentless*: an attack's mark lasts until the enemy dies, walked towards even unseen | *Grit*: during a rally, Bims in it lose no pace to wounds or traumas |
//! | 10 | *Anchor*: the aura's bonuses double while he stands still | *Warcry*: a rally covers every friendly Bim in the room |
//!
//! **His aura and his rally lift every friendly Bim they reach, a
//! player's own steered Bims included; his squad orders command only the
//! squad** — every crew member no player is steering, the crew's own
//! bots and the hired hands alike. See [`crate::commander`].
//!
//! Every multiplier is a named constant here; what each talent *does* is
//! `crate::deploy` and the world's step for the engineer, the room's
//! one shooter (`bims::combat::Skill`, `World::skill_of`) for the
//! soldier, `crate::medic` with the world's step for the medic, and
//! `crate::tank` with the same `Skill` and the room's own bulwarks for
//! the tank, and `crate::commander` with the same `Skill` and the room's
//! own squad orders for the commander. No
//! strings: the app names the classes and the talents (`CLASS_NAMES`,
//! `TALENT_NAMES`).

use crate::deploy::Kit;
use crate::event::Refusal;
use physics::ResourceId;

/// What a crew member is. Codes cross the seam and are never renumbered.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Class {
    /// No class: learns nothing, lays nothing. Everybody's until chosen.
    #[default]
    None = 0,
    /// The engineer: sandbags, a sentry, and the workbench's friend.
    Engineer = 1,
    /// The soldier: a line held, and grenades.
    Soldier = 2,
    /// The medic: a heal beam, and a surge.
    Medic = 3,
    /// The tank: a wall the crew shelter behind, and a taunt.
    Tank = 4,
    /// The commander: an aura the crew round him fight better in, orders
    /// for the squad, and a cheaper hand at the dock.
    Commander = 5,
}

impl Class {
    pub const ALL: [Class; 6] = [
        Class::None,
        Class::Engineer,
        Class::Soldier,
        Class::Medic,
        Class::Tank,
        Class::Commander,
    ];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<Class> {
        Class::ALL.get(code as usize).copied()
    }

    /// What it wears on the deck (feature 81): the kit the room draws
    /// over the coverall. Drawing only — `World::hand_the_room_the_outfits`
    /// says it every step and nothing else reads it — so a class added
    /// later wants an arm here or it looks like everybody else.
    pub fn outfit(self) -> bims::character::Outfit {
        use bims::character::Outfit;
        match self {
            Class::None => Outfit::Plain,
            Class::Engineer => Outfit::Engineer,
            Class::Soldier => Outfit::Soldier,
            Class::Medic => Outfit::Medic,
            Class::Tank => Outfit::Tank,
            Class::Commander => Outfit::Commander,
        }
    }
}

/// What a class alone may do: the only thing [`can`] ever refuses for a
/// class. Never a job, an errand, a site or a weapon.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ability {
    /// Lay a kit, pack a deployable up, refill a sentry: the engineer's.
    Deploy,
    /// Hold a line: the soldier's.
    Brace,
    /// Throw a grenade: the soldier's.
    Throw,
    /// Hold a heal beam on a crewmate: the medic's.
    Beam,
    /// Trigger a surge: the medic's.
    Surge,
    /// Stand as a wall the crew behind shelter against: the tank's.
    Bulwark,
    /// Draw the enemy's fire onto himself: the tank's.
    Taunt,
    /// Send the squad — attack, fall back, stand ground: the commander's.
    SquadOrder,
    /// Call a rally: the commander's.
    Rally,
}

impl Ability {
    pub const ALL: [Ability; 9] = [
        Ability::Deploy,
        Ability::Brace,
        Ability::Throw,
        Ability::Beam,
        Ability::Surge,
        Ability::Bulwark,
        Ability::Taunt,
        Ability::SquadOrder,
        Ability::Rally,
    ];
}

/// Whether a class may use an ability. The one rule a class gates
/// anything by: everything not an [`Ability`] is everybody's.
pub fn can(class: Class, ability: Ability) -> bool {
    match ability {
        Ability::Deploy => class == Class::Engineer,
        Ability::Brace | Ability::Throw => class == Class::Soldier,
        Ability::Beam | Ability::Surge => class == Class::Medic,
        Ability::Bulwark | Ability::Taunt => class == Class::Tank,
        Ability::SquadOrder | Ability::Rally => class == Class::Commander,
    }
}

/// What an ability **spends**: a thing in the pack that comes back on a
/// cooldown of its own rather than being made at a bench or bought at a
/// dock (features 88 and 90). **No class crafts for its abilities**: a
/// class brings its charges with it, spends them, and waits — the
/// engineer's two kits, the soldier's grenade, and whatever a class
/// added later spends.
///
/// The number of charges and the seconds one takes to come back are
/// [`World::charges`](crate::World::charges) and
/// [`World::charge_cooldown`](crate::World::charge_cooldown), since both
/// are a level and a talent away from the constants here;
/// `World::restock_charges` is the one step that fills a pack back up.
///
/// Codes cross the seam and are never renumbered.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Charge {
    /// The engineer's sandbag kit.
    Sandbag = 0,
    /// The engineer's sentry kit.
    Sentry = 1,
    /// The soldier's grenade (feature 90).
    Grenade = 2,
}

impl Charge {
    pub const ALL: [Charge; 3] = [Charge::Sandbag, Charge::Sentry, Charge::Grenade];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<Charge> {
        Charge::ALL.get(code as usize).copied()
    }

    /// The thing it is in a pack.
    pub fn resource(self) -> ResourceId {
        match self {
            Charge::Sandbag => ResourceId::SandbagKit,
            Charge::Sentry => ResourceId::SentryKit,
            Charge::Grenade => ResourceId::Grenade,
        }
    }

    /// The one class that spends it.
    pub fn class(self) -> Class {
        match self {
            Charge::Sandbag | Charge::Sentry => Class::Engineer,
            Charge::Grenade => Class::Soldier,
        }
    }

    /// The level it may be spent from: a class's ability level, since a
    /// charge nothing can spend yet does not come back either.
    pub fn level(self) -> u8 {
        match self {
            Charge::Sandbag => 1,
            Charge::Sentry => SENTRY_LEVEL,
            Charge::Grenade => GRENADE_LEVEL,
        }
    }

    /// The engineer's kit it is, if it is one.
    pub fn kit(self) -> Option<Kit> {
        match self {
            Charge::Sandbag => Some(Kit::Sandbag),
            Charge::Sentry => Some(Kit::Sentry),
            Charge::Grenade => None,
        }
    }

    /// The charge an engineer's kit is.
    pub fn of_kit(kit: Kit) -> Charge {
        match kit {
            Kit::Sandbag => Charge::Sandbag,
            Kit::Sentry => Charge::Sentry,
        }
    }
}

/// Which of a pick level's two talents.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Side {
    Left = 0,
    Right = 1,
}

impl Side {
    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<Side> {
        match code {
            0 => Some(Side::Left),
            1 => Some(Side::Right),
            _ => None,
        }
    }
}

/// The talents that are picked — each class's seven pick levels' two
/// each, in level order, left before right, the engineer's fourteen, then
/// the soldier's, then the medic's, then the tank's, then the
/// commander's. The fixed levels (1, 3, 7) are not talents: they are
/// the level itself, asked of `Progress::level`. Codes cross the seam
/// and index `TALENT_NAMES` in the app.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Talent {
    // The engineer's (features 74 and 88). Codes are never renumbered:
    // the four the eighth-eighth feature replaced — *quick hands*, *deep
    // magazine*, *field refit* and *salvage*, all of them about a thing
    // an engineer no longer does — kept their places.
    ReinforcedSand = 0,
    SiteForeman = 1,
    Sandbagger = 2,
    BulkBags = 3,
    ArmouredSentry = 4,
    EnhancedOptics = 5,
    Armourer = 6,
    BetterArmour = 7,
    DugIn = 8,
    QuickBuild = 9,
    ExtraBags = 10,
    SteadyHands = 11,
    SecondSentry = 12,
    SentryMarkThree = 13,
    // The soldier's (feature 75).
    Marksman = 14,
    PointBlank = 15,
    Runner = 16,
    SteadyAim = 17,
    IronNerve = 18,
    CoverMaster = 19,
    LongThrow = 20,
    ShortFuse = 21,
    Frag = 22,
    QuickDraw = 23,
    Bruiser = 24,
    /// The soldier's *dug in* — the engineer's is [`Talent::DugIn`].
    DugInBraced = 25,
    Deadeye = 26,
    Rampage = 27,
    // The medic's (feature 76).
    FieldDressing = 28,
    Surgeon = 29,
    LongBeam = 30,
    StrongBeam = 31,
    CleanHands = 32,
    /// The medic's *steady hands* — the engineer's is [`Talent::SteadyHands`].
    SteadyHandsMedic = 33,
    QuickCharge = 34,
    LongSurge = 35,
    SelfCare = 36,
    DoubleLink = 37,
    GunnerMedic = 38,
    ClosingSurge = 39,
    MassSurge = 40,
    FieldSurgeon = 41,
    // The tank's (feature 77).
    Plated = 42,
    Breacher = 43,
    Unmovable = 44,
    WideWall = 45,
    FastWall = 46,
    LoudTaunt = 47,
    LongTaunt = 48,
    HoldFast = 49,
    Guarded = 50,
    Interpose = 51,
    Magnet = 52,
    Fortress = 53,
    RallyingWall = 54,
    // The commander's (feature 78).
    WidePresence = 55,
    StrongPresence = 56,
    Haggler = 57,
    Outfitter = 58,
    FocusFire = 59,
    Pincer = 60,
    LongRally = 61,
    QuickRally = 62,
    SteadyRanks = 63,
    DoubleTime = 64,
    Relentless = 65,
    Grit = 66,
    Anchor = 67,
    Warcry = 68,
}

impl Talent {
    pub const ALL: [Talent; 69] = [
        Talent::ReinforcedSand,
        Talent::SiteForeman,
        Talent::Sandbagger,
        Talent::BulkBags,
        Talent::ArmouredSentry,
        Talent::EnhancedOptics,
        Talent::Armourer,
        Talent::BetterArmour,
        Talent::DugIn,
        Talent::QuickBuild,
        Talent::ExtraBags,
        Talent::SteadyHands,
        Talent::SecondSentry,
        Talent::SentryMarkThree,
        Talent::Marksman,
        Talent::PointBlank,
        Talent::Runner,
        Talent::SteadyAim,
        Talent::IronNerve,
        Talent::CoverMaster,
        Talent::LongThrow,
        Talent::ShortFuse,
        Talent::Frag,
        Talent::QuickDraw,
        Talent::Bruiser,
        Talent::DugInBraced,
        Talent::Deadeye,
        Talent::Rampage,
        Talent::FieldDressing,
        Talent::Surgeon,
        Talent::LongBeam,
        Talent::StrongBeam,
        Talent::CleanHands,
        Talent::SteadyHandsMedic,
        Talent::QuickCharge,
        Talent::LongSurge,
        Talent::SelfCare,
        Talent::DoubleLink,
        Talent::GunnerMedic,
        Talent::ClosingSurge,
        Talent::MassSurge,
        Talent::FieldSurgeon,
        Talent::Plated,
        Talent::Breacher,
        Talent::Unmovable,
        Talent::WideWall,
        Talent::FastWall,
        Talent::LoudTaunt,
        Talent::LongTaunt,
        Talent::HoldFast,
        Talent::Guarded,
        Talent::Interpose,
        Talent::Magnet,
        Talent::Fortress,
        Talent::RallyingWall,
        Talent::WidePresence,
        Talent::StrongPresence,
        Talent::Haggler,
        Talent::Outfitter,
        Talent::FocusFire,
        Talent::Pincer,
        Talent::LongRally,
        Talent::QuickRally,
        Talent::SteadyRanks,
        Talent::DoubleTime,
        Talent::Relentless,
        Talent::Grit,
        Talent::Anchor,
        Talent::Warcry,
    ];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<Talent> {
        Talent::ALL.get(code as usize).copied()
    }

    /// Whose talent it is. Fourteen a class, bar the tank's **thirteen**
    /// since the money rework (feature 95) took *pack mule* off its
    /// second level, which is a fixed level now.
    pub fn class(self) -> Class {
        if self.code() < 14 {
            Class::Engineer
        } else if self.code() < 28 {
            Class::Soldier
        } else if self.code() < 42 {
            Class::Medic
        } else if self.code() < 55 {
            Class::Tank
        } else {
            Class::Commander
        }
    }
}

/// How many levels there are.
pub const LEVELS: u8 = 10;

/// Cumulative experience for each level, by level less one: nothing for
/// the first, 100 for the second, up to 3 200 for the tenth.
pub const LEVEL_XP: [u32; LEVELS as usize] =
    [0, 100, 250, 450, 700, 1_000, 1_400, 1_900, 2_500, 3_200];

/// What an enemy going down within the vicinity is worth, to every
/// classed crew member in range.
pub const XP_ENEMY_DOWN: u32 = 10;
/// What an enemy dying within it is worth, the same way.
pub const XP_ENEMY_DEAD: u32 = 5;
/// What a site finished or a kit laid within an engineer's vicinity is
/// worth, to that engineer.
pub const XP_BUILT: u32 = 2;
/// How far the vicinity reaches, in tiles.
pub const VICINITY_TILES: f32 = 50.0;

// --- the engineer's numbers (features 74 and 88) ------------------------------

/// The level a sentry may be laid from: the engineer's third.
pub const SENTRY_LEVEL: u8 = 3;
/// The level a sentry's rifle takes the tier-two factors from: the
/// engineer's seventh.
pub const SENTRY_MARK_TWO_LEVEL: u8 = 7;

/// *Site foreman*: what a build's working steps run at.
pub const SITE_FOREMAN_EFFORT: f32 = 1.25;
/// *Sandbagger*: what the sandbag deploy time is multiplied by.
pub const SANDBAGGER_TIME: f64 = 0.5;
/// *Armoured sentry*: what a sentry's health is multiplied by.
pub const ARMOURED_SENTRY_HEALTH: f32 = 1.5;
/// *Reinforced sand* (feature 88): what is **added** to a laid bag's
/// health, where every other sentry and sandbag factor multiplies.
pub const REINFORCED_SAND_HEALTH: f32 = 50.0;
/// *Enhanced optics* (feature 88): tiles **added** to the sentry's fire
/// range. It is added to the weapon's range, so it lengthens what the
/// sentry shoots at and the span the odds and the damage fall across.
pub const ENHANCED_OPTICS_RANGE: f32 = 10.0;
/// *Higher quality armour* (feature 88): what a piece worn by this
/// engineer has of health, which is applied as the piece draining at the
/// reciprocal — the tank's mechanism, so a piece's stored health never
/// changes meaning as it moves between bodies.
pub const BETTER_ARMOUR_HEALTH: f32 = 1.05;
/// *Higher quality armour*: what is added to a worn piece's protection.
pub const BETTER_ARMOUR_PROTECTION: f32 = 1.0;
/// *Extra bags* (feature 88): sandbag charges added.
pub const EXTRA_BAGS_CHARGES: u32 = 1;
/// *Second sentry*: sentry charges in all — and so how many may stand.
pub const SECOND_SENTRY_CHARGES: u32 = 2;
/// *Sentry mark III* (feature 88): what its tier-three sniper rifle's
/// fire rate is multiplied by.
pub const SENTRY_MARK_THREE_FIRE_RATE: f32 = 2.0;
/// *Sentry mark III*: what its damage is multiplied by, near and far.
pub const SENTRY_MARK_THREE_DAMAGE: f32 = 1.2;
/// *Quick build*: what the sentry deploy time is multiplied by.
pub const QUICK_BUILD_TIME: f64 = 0.5;
/// *Armourer*: what one repair at the workbench puts back on a piece.
pub const ARMOUR_REPAIR_HEALTH: f32 = 10.0;
/// *Armourer*: how long the repair takes at the bench, in game minutes.
pub const ARMOUR_REPAIR_MINUTES: u32 = 10;

// --- the soldier's numbers (feature 75) --------------------------------------

/// What a braced soldier's odds are multiplied by.
pub const BRACE_ACCURACY: f32 = 1.15;
/// **Grenade charges** a soldier has (feature 90): how many grenades its
/// pack fills back up to on [`GRENADE_COOLDOWN`] a charge, and what it
/// sets out with. A soldier makes none and buys none — see [`Charge`].
pub const GRENADE_CHARGES: u32 = 2;
/// The level a grenade may be thrown from: the soldier's third.
pub const GRENADE_LEVEL: u8 = 3;
/// The level *drill* applies from: the soldier's seventh.
pub const DRILL_LEVEL: u8 = 7;
/// Seconds of the clock one spent grenade charge takes to come back into
/// the pack (feature 90). Two charges are thrown one after the other and
/// then waited for, the way the engineer's kits are: the cooldown is the
/// supply, and nothing gates a throw but a grenade in the pack.
pub const GRENADE_COOLDOWN: f64 = 30.0;
/// How far a grenade is thrown, in tiles.
pub const GRENADE_RANGE: f32 = 8.0;
/// Seconds from the throw to the burst.
pub const GRENADE_FUSE: f32 = 2.0;
/// How far the burst reaches, in tiles.
pub const GRENADE_RADIUS: f32 = 2.5;
/// What the burst does at its centre; half that at the edge.
pub const GRENADE_DAMAGE: f32 = 40.0;
/// *Marksman*: what the odds are multiplied by.
pub const MARKSMAN_ACCURACY: f32 = 1.15;
/// *Point blank*: what the damage within the sweet range is multiplied by.
pub const POINT_BLANK_DAMAGE: f32 = 1.2;
/// *Runner*: what the pace is multiplied by while an enemy is in sight.
pub const RUNNER_PACE: f32 = 1.2;
/// *Cover master*: what the odds in cover are multiplied by.
pub const COVER_MASTER_DODGE: f32 = 1.5;
/// *Long throw*: what the grenade range is multiplied by.
pub const LONG_THROW_RANGE: f32 = 1.5;
/// *Short fuse*: what the grenade fuse is multiplied by.
pub const SHORT_FUSE_TIME: f32 = 0.5;
/// *Drill*: what every weapon's fire rate is multiplied by.
pub const DRILL_FIRE_RATE: f32 = 1.2;
/// *Frag*: what the grenade radius is multiplied by.
pub const FRAG_RADIUS: f32 = 1.5;
/// *Quick draw*: what the grenade charge's cooldown is multiplied by.
pub const QUICK_DRAW_COOLDOWN: f64 = 0.5;
/// *Bruiser*: what a blow's damage is multiplied by, fist or blade.
pub const BRUISER_MELEE: f32 = 1.5;
/// *Dug in* (the soldier's): what is added to the dodge while braced.
pub const DUG_IN_DODGE: f32 = 0.10;
/// *Rampage*: what the fire rate is multiplied by a stack.
pub const RAMPAGE_FIRE_RATE: f32 = 1.1;
/// *Rampage*: how many stacks at most.
pub const RAMPAGE_STACKS: u32 = 3;

/// *Steady aim*: the odds on the move, where everybody else's are
/// `bims::combat::WALKING_ACCURACY` — the penalty halved.
pub fn steady_aim_walking() -> f32 {
    1.0 - (1.0 - bims::combat::WALKING_ACCURACY) / 2.0
}

// --- the medic's numbers (feature 76) ----------------------------------------

/// What a medic finishing a bandage or a treatment on a crewmate is
/// worth, to that medic.
pub const XP_HEALED: u32 = 5;
/// Medkits a medic sets out with in its pack, and bandages beside them.
pub const MEDIC_START_MEDKITS: u32 = 2;
pub const MEDIC_START_BANDAGES: u32 = 4;
/// How far the heal beam reaches, in tiles.
pub const HEAL_BEAM_RANGE: f32 = 6.0;
/// Blood a beamed patient gains an hour.
pub const HEAL_BEAM_BLOOD: f32 = 30.0;
/// Minutes of the clock beaming a patient that qualifies — below full
/// blood, or with a wound open — until the surge is charged.
pub const SURGE_CHARGE_MINUTES: f64 = 40.0;
/// Minutes of the clock a surge runs.
pub const SURGE_MINUTES: f64 = 8.0;
/// The level a surge may be triggered from: the medic's third.
pub const SURGE_LEVEL: u8 = 3;
/// The level *mender* applies from: the medic's seventh.
pub const MENDER_LEVEL: u8 = 7;
/// *Mender*: what a beamed patient's parts mend at, times
/// `bims::health::HEALTH_RECOVER`.
pub const MENDER_RECOVER: f32 = 10.0;
/// *Field dressing*: what the medic's bandaging time is multiplied by.
pub const FIELD_DRESSING_TIME: f32 = 0.5;
/// *Surgeon*: what the medic's treating time is multiplied by.
pub const SURGEON_TIME: f32 = 0.5;
/// *Long beam*: what the beam's range is multiplied by.
pub const LONG_BEAM_RANGE: f32 = 1.5;
/// *Strong beam*: what the beam's blood rate is multiplied by.
pub const STRONG_BEAM_BLOOD: f32 = 1.5;
/// *Steady hands* (the medic's): what `bims::health::TREATED_TO` is
/// multiplied by for a part it treats.
pub const STEADY_HANDS_TREATED: f32 = 1.5;
/// *Quick charge*: what the surge's charging rate is multiplied by.
pub const QUICK_CHARGE_RATE: f64 = 1.5;
/// *Long surge*: what a surge's minutes are multiplied by.
pub const LONG_SURGE_TIME: f64 = 1.5;
/// *Double link*: how many patients the beam holds at once.
pub const DOUBLE_LINK_PATIENTS: usize = 2;
/// *Gunner medic*: what the fire rate is multiplied by while beaming.
pub const GUNNER_MEDIC_FIRE_RATE: f32 = 0.5;
/// *Mass surge*: how far round the patient a surge reaches, in tiles.
pub const MASS_SURGE_TILES: f32 = 3.0;
/// *Field surgeon*: what a treatment with no kit takes, of the ordinary
/// time.
pub const FIELD_SURGEON_TIME: f32 = 0.5;

// --- the tank's numbers (feature 77) -----------------------------------------

/// Enemy hits landing on a tank that make one point of experience. The
/// count itself lives on the Bim (`Game::hits_taken`), since a fifth of
/// a point is not a whole number.
pub const TANK_HITS_PER_XP: u32 = 5;
/// What a piece of armour worn by a tank drains at: half the damage it
/// takes past its protection, so a piece absorbs twice as much on him.
/// Never doubled in the piece's own health, which moves between Bims
/// unchanged.
pub const TANK_DRAIN: f32 = 0.5;
/// What a tank's pace is multiplied by while Bulwark is on.
pub const BULWARK_PACE: f32 = 0.5;
/// How far Bulwark reaches, in tiles: how near the tank a crew member
/// must stand to shelter behind him, and how near the line from the
/// shooter he must stand to be between them.
pub const BULWARK_REACH: f32 = 1.5;
/// The level a taunt may be used from: the tank's third.
pub const TAUNT_LEVEL: u8 = 3;
/// The level *iron frame* applies from: the tank's seventh.
pub const IRON_FRAME_LEVEL: u8 = 7;
/// Seconds of the clock between one taunt and the next.
pub const TAUNT_COOLDOWN: f64 = 20.0;
/// Minutes of the clock a taunt runs.
pub const TAUNT_MINUTES: f64 = 6.0;
/// How far a taunt reaches, in tiles.
pub const TAUNT_RADIUS: f32 = 10.0;
/// *Plated*: what a worn piece's protection is multiplied by on him.
pub const PLATED_PROTECTION: f32 = 1.5;
/// *Breacher*: what forcing a locked door takes, of the ordinary time.
pub const BREACHER_TIME: f32 = 0.5;
/// *Wide wall*: what the bulwark's reach is multiplied by.
pub const WIDE_WALL_REACH: f32 = 2.0;
/// *Fast wall*: what the bulwark's pace is multiplied by.
pub const FAST_WALL_PACE: f32 = 1.5;
/// *Loud taunt*: what the taunt's radius is multiplied by.
pub const LOUD_TAUNT_RADIUS: f32 = 1.5;
/// *Long taunt*: what a taunt's minutes are multiplied by.
pub const LONG_TAUNT_TIME: f64 = 1.5;
/// *Guarded*: what is added to the dodge while Bulwark is on.
pub const GUARDED_DODGE: f32 = 0.10;
/// *Fortress*: what the tank's armour drain is multiplied by again.
pub const FORTRESS_DRAIN: f32 = 0.5;
/// *Rallying wall*: how far round a taunting tank it reaches, in tiles.
pub const RALLYING_WALL_TILES: f32 = 3.0;
/// *Rallying wall*: what a sheltered crewmate's armour drains at.
pub const RALLYING_WALL_DRAIN: f32 = 0.5;

// --- the commander's numbers (feature 78) ------------------------------------

/// What a hire made from the slot steering a commander is worth, to that
/// commander alone.
pub const XP_HIRE: u32 = 10;
/// How far the aura reaches, in tiles.
pub const AURA_TILES: f32 = 8.0;
/// *Aura*: what a Bim in it works at.
pub const AURA_WORK: f32 = 1.1;
/// *Aura*: what a Bim in it shoots at.
pub const AURA_AIM: f32 = 1.1;
/// *Aura*: how much longer a Bim in it holds its ground before it runs —
/// [`NERVE_HOLD`] times this.
pub const AURA_NERVE: f32 = 1.5;
/// How long a dying body holds its ground before it runs, in seconds of
/// the clock: nought for a body with no commander near it — which is
/// why every Bim in the game before the commander ran the moment it was
/// dying, and still does — and this, times the aura's [`AURA_NERVE`],
/// for one in an aura.
pub const NERVE_HOLD: f32 = 8.0;
/// What a commander takes off a mercenary's fee, in whole per cent.
pub const HIRE_DISCOUNT_PERCENT: u32 = 25;
/// *Haggler*: what he takes off it instead.
pub const HAGGLER_DISCOUNT_PERCENT: u32 = 40;
/// How far a squad order reaches from the commander, in tiles; *long
/// reach* is the whole room.
pub const SQUAD_RANGE: f32 = 20.0;
/// The level a rally may be called from: the commander's third.
pub const RALLY_LEVEL: u8 = 3;
/// The level *long reach* applies from: the commander's seventh.
pub const LONG_REACH_LEVEL: u8 = 7;
/// Seconds of the clock between one rally and the next.
pub const RALLY_COOLDOWN: f64 = 30.0;
/// Minutes of the clock a rally runs.
pub const RALLY_MINUTES: f64 = 6.0;
/// *Rally*: what a Bim in it shoots at. It stacks with the aura's, and
/// nothing in it ever runs.
pub const RALLY_AIM: f32 = 1.3;
/// *Wide presence*: what the aura's radius is multiplied by.
pub const WIDE_PRESENCE_RADIUS: f32 = 1.5;
/// *Strong presence*: what each of the aura's bonuses is multiplied by —
/// of what it adds, so a tenth becomes three twentieths
/// ([`aura_bonus`]).
pub const STRONG_PRESENCE: f32 = 1.5;
/// *Anchor*: the same, again, while he stands still.
pub const ANCHOR_BONUS: f32 = 2.0;
/// *Focus fire*: what the squad's odds against the enemy it is attacking
/// are multiplied by.
pub const FOCUS_FIRE_ACCURACY: f32 = 1.15;
/// *Pincer*: how many enemies an attack may mark at once; one without
/// it.
pub const PINCER_MARKS: usize = 2;
/// *Long rally*: what a rally's minutes are multiplied by.
pub const LONG_RALLY_TIME: f64 = 1.5;
/// *Quick rally*: what the rally's cooldown is multiplied by.
pub const QUICK_RALLY_COOLDOWN: f64 = 0.5;
/// *Steady ranks*: what a Bim in the aura bleeds at.
pub const STEADY_RANKS_BLEED: f32 = 0.75;
/// *Double time*: what a Bim in the aura's pace is multiplied by.
pub const DOUBLE_TIME_PACE: f32 = 1.1;

/// One of the aura's bonuses through *strong presence* and *anchor*:
/// what the bonus *adds* is multiplied, so [`AURA_WORK`]'s tenth becomes
/// three twentieths under [`STRONG_PRESENCE`] and a fifth under both.
/// A bonus below one — [`STEADY_RANKS_BLEED`] — deepens the same way,
/// never past nothing.
pub fn aura_bonus(bonus: f32, factor: f32) -> f32 {
    (1.0 + (bonus - 1.0) * factor).max(0.0)
}

/// Whether a level is a pick level **for this class**: every class is
/// fixed at one, three and seven and a pick at the rest, bar the tank's
/// second, which the money rework (feature 95) made a fixed level when
/// hauling went and *pack mule* with it — *plated* stands alone there.
///
/// Asked of `pick_at`, so the two can never disagree about the shape of
/// a tree.
pub fn is_pick_level(class: Class, level: u8) -> bool {
    pick_at(class, level).is_some()
}

/// The one talent a **fixed** level of a class gives outright, if it
/// gives one: the tank's *plated* at the second, and nothing anywhere
/// else so far. A fixed level costs no skill point and is never picked
/// at; `Progress::has` counts it from the level it sits at.
pub fn fixed_at(class: Class, level: u8) -> Option<Talent> {
    match (class, level) {
        (Class::Tank, 2) => Some(Talent::Plated),
        _ => None,
    }
}

/// The two talents a class offers at a pick level, left and right, or
/// `None` for a fixed level, for no level at all, and for no class.
pub fn pick_at(class: Class, level: u8) -> Option<(Talent, Talent)> {
    Some(match (class, level) {
        (Class::Engineer, 2) => (Talent::ReinforcedSand, Talent::SiteForeman),
        (Class::Engineer, 4) => (Talent::Sandbagger, Talent::BulkBags),
        (Class::Engineer, 5) => (Talent::ArmouredSentry, Talent::EnhancedOptics),
        (Class::Engineer, 6) => (Talent::Armourer, Talent::BetterArmour),
        (Class::Engineer, 8) => (Talent::DugIn, Talent::QuickBuild),
        (Class::Engineer, 9) => (Talent::ExtraBags, Talent::SteadyHands),
        (Class::Engineer, 10) => (Talent::SecondSentry, Talent::SentryMarkThree),
        (Class::Soldier, 2) => (Talent::Marksman, Talent::PointBlank),
        (Class::Soldier, 4) => (Talent::Runner, Talent::SteadyAim),
        (Class::Soldier, 5) => (Talent::IronNerve, Talent::CoverMaster),
        (Class::Soldier, 6) => (Talent::LongThrow, Talent::ShortFuse),
        (Class::Soldier, 8) => (Talent::Frag, Talent::QuickDraw),
        (Class::Soldier, 9) => (Talent::Bruiser, Talent::DugInBraced),
        (Class::Soldier, 10) => (Talent::Deadeye, Talent::Rampage),
        (Class::Medic, 2) => (Talent::FieldDressing, Talent::Surgeon),
        (Class::Medic, 4) => (Talent::LongBeam, Talent::StrongBeam),
        (Class::Medic, 5) => (Talent::CleanHands, Talent::SteadyHandsMedic),
        (Class::Medic, 6) => (Talent::QuickCharge, Talent::LongSurge),
        (Class::Medic, 8) => (Talent::SelfCare, Talent::DoubleLink),
        (Class::Medic, 9) => (Talent::GunnerMedic, Talent::ClosingSurge),
        (Class::Medic, 10) => (Talent::MassSurge, Talent::FieldSurgeon),
        (Class::Tank, 4) => (Talent::Breacher, Talent::Unmovable),
        (Class::Tank, 5) => (Talent::WideWall, Talent::FastWall),
        (Class::Tank, 6) => (Talent::LoudTaunt, Talent::LongTaunt),
        (Class::Tank, 8) => (Talent::HoldFast, Talent::Guarded),
        (Class::Tank, 9) => (Talent::Interpose, Talent::Magnet),
        (Class::Tank, 10) => (Talent::Fortress, Talent::RallyingWall),
        (Class::Commander, 2) => (Talent::WidePresence, Talent::StrongPresence),
        (Class::Commander, 4) => (Talent::Haggler, Talent::Outfitter),
        (Class::Commander, 5) => (Talent::FocusFire, Talent::Pincer),
        (Class::Commander, 6) => (Talent::LongRally, Talent::QuickRally),
        (Class::Commander, 8) => (Talent::SteadyRanks, Talent::DoubleTime),
        (Class::Commander, 9) => (Talent::Relentless, Talent::Grit),
        (Class::Commander, 10) => (Talent::Anchor, Talent::Warcry),
        _ => return None,
    })
}

/// The level `xp` makes, one to ten.
pub fn level_of(xp: u32) -> u8 {
    LEVEL_XP.iter().filter(|&&need| xp >= need).count().max(1) as u8
}

/// One crew member's way through its class: what it has learnt. The
/// picks are a level and a side, and which talent each is depends on
/// the class the crew member has — asked of every reading.
#[derive(Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Progress {
    /// Cumulative experience.
    pub xp: u32,
    /// The picks made, in level order: the level and which side.
    pub picks: Vec<(u8, Side)>,
}

impl Progress {
    /// The level the experience makes.
    pub fn level(&self) -> u8 {
        level_of(self.xp)
    }

    /// Experience still wanted for the next level; nought at the top.
    pub fn to_next(&self) -> u32 {
        let level = self.level();
        if level >= LEVELS {
            return 0;
        }
        LEVEL_XP[level as usize].saturating_sub(self.xp)
    }

    /// `xp` more: every level reached by it, lowest first, for the world
    /// to say. Nothing past the tenth.
    pub fn gain(&mut self, xp: u32) -> Vec<u8> {
        let was = self.level();
        self.xp = self.xp.saturating_add(xp);
        let now = self.level();
        ((was + 1)..=now).collect()
    }

    /// Whether a talent has been picked, for a crew member of `class`.
    pub fn has(&self, class: Class, talent: Talent) -> bool {
        self.picks
            .iter()
            .any(|&(level, side)| talent_of(class, level, side) == Some(talent))
            || (1..=self.level()).any(|l| fixed_at(class, l) == Some(talent))
    }

    /// The pick made at a level, if any.
    pub fn picked_at(&self, level: u8) -> Option<Side> {
        self.picks
            .iter()
            .find(|&&(l, _)| l == level)
            .map(|&(_, side)| side)
    }

    /// The lowest reached pick level with no pick yet, if any: what the
    /// panel offers, and what a level-up leaves pending.
    pub fn pending_pick(&self, class: Class) -> Option<u8> {
        (2..=self.level()).find(|&l| is_pick_level(class, l) && self.picked_at(l).is_none())
    }

    /// Choose a side at a level for a crew member of `class`: a pick
    /// level, reached, not yet picked. The talent it is, or why not.
    pub fn pick(&mut self, class: Class, level: u8, side: Side) -> Result<Talent, Refusal> {
        let (left, right) = pick_at(class, level).ok_or(Refusal::NotAPickLevel)?;
        if level > self.level() {
            return Err(Refusal::LevelNotReached);
        }
        if self.picked_at(level).is_some() {
            return Err(Refusal::AlreadyPicked);
        }
        self.picks.push((level, side));
        self.picks.sort_by_key(|&(l, _)| l);
        Ok(match side {
            Side::Left => left,
            Side::Right => right,
        })
    }

    /// Every talent picked, for a crew member of `class`, in level order.
    pub fn talents(&self, class: Class) -> Vec<Talent> {
        (1..=self.level())
            .filter_map(|l| fixed_at(class, l))
            .chain(
                self.picks
                    .iter()
                    .filter_map(|&(level, side)| talent_of(class, level, side)),
            )
            .collect()
    }
}

/// The talent a side of a level is, for a class.
pub fn talent_of(class: Class, level: u8, side: Side) -> Option<Talent> {
    pick_at(class, level).map(|(left, right)| match side {
        Side::Left => left,
        Side::Right => right,
    })
}

/// The level a class's own key is learnt at: its **E** from the first
/// level — sandbags, the brace, the beam, the wall, the squad — and its
/// **Q** from the third for every class — a sentry, grenades, the surge,
/// the taunt, the rally. `None` for [`Class::None`], which has no keys
/// at all. What the two boxes at the foot of the screen grey themselves
/// out by (feature 80).
pub fn key_level(class: Class, primary: bool) -> Option<u8> {
    Some(match (class, primary) {
        (Class::None, _) => return None,
        (_, false) => 1,
        (Class::Engineer, true) => SENTRY_LEVEL,
        (Class::Soldier, true) => GRENADE_LEVEL,
        (Class::Medic, true) => SURGE_LEVEL,
        (Class::Tank, true) => TAUNT_LEVEL,
        (Class::Commander, true) => RALLY_LEVEL,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_levels_climb_at_the_thresholds_and_a_pick_is_one_each() {
        assert_eq!(level_of(0), 1);
        assert_eq!(level_of(99), 1);
        for (i, &need) in LEVEL_XP.iter().enumerate().skip(1) {
            assert_eq!(level_of(need - 1), i as u8, "just under {need}");
            assert_eq!(level_of(need), i as u8 + 1, "at {need}");
        }
        assert_eq!(level_of(100_000), 10);
        let mut p = Progress::default();
        assert_eq!(p.to_next(), 100);
        assert_eq!(p.gain(99), Vec::<u8>::new());
        assert_eq!(p.gain(1), vec![2]);
        assert_eq!(p.gain(600), vec![3, 4, 5]);
        assert_eq!(p.pending_pick(Class::Engineer), Some(2));
        assert_eq!(
            p.pick(Class::Engineer, 3, Side::Left),
            Err(Refusal::NotAPickLevel)
        );
        assert_eq!(
            p.pick(Class::Engineer, 6, Side::Left),
            Err(Refusal::LevelNotReached)
        );
        assert_eq!(
            p.pick(Class::None, 2, Side::Left),
            Err(Refusal::NotAPickLevel),
            "no class, no picks"
        );
        assert_eq!(
            p.pick(Class::Engineer, 2, Side::Right),
            Ok(Talent::SiteForeman)
        );
        assert_eq!(
            p.pick(Class::Engineer, 2, Side::Left),
            Err(Refusal::AlreadyPicked)
        );
        assert!(
            p.has(Class::Engineer, Talent::SiteForeman)
                && !p.has(Class::Engineer, Talent::ReinforcedSand)
        );
        // The same pick read as a soldier's is the soldier's right-hand
        // talent at that level: which talent a pick is depends on the
        // class.
        assert!(p.has(Class::Soldier, Talent::PointBlank));
        assert_eq!(p.talents(Class::Soldier), vec![Talent::PointBlank]);
        assert_eq!(p.pending_pick(Class::Engineer), Some(4));
        for class in [
            Class::Engineer,
            Class::Soldier,
            Class::Medic,
            Class::Tank,
            Class::Commander,
        ] {
            let mut all = Vec::new();
            for level in 1..=LEVELS {
                // A fixed level's talent is the class's too, given rather
                // than chosen at.
                if let Some(fixed) = fixed_at(class, level) {
                    all.push(fixed);
                }
                assert_eq!(pick_at(class, level).is_some(), is_pick_level(class, level));
                // A fixed level gives its talent outright and costs no
                // point: the tank's second since the money rework.
                if let Some(fixed) = fixed_at(class, level) {
                    assert_eq!(fixed.class(), class);
                    assert!(pick_at(class, level).is_none());
                }
                if let Some((l, r)) = pick_at(class, level) {
                    assert_eq!(l.class(), class);
                    assert_eq!(r.class(), class);
                    all.push(l);
                    all.push(r);
                }
            }
            let own: Vec<Talent> = Talent::ALL
                .into_iter()
                .filter(|t| t.class() == class)
                .collect();
            assert_eq!(all, own, "every talent of {class:?} is on one pick level");
        }
        for level in 1..=LEVELS {
            assert_eq!(pick_at(Class::None, level), None);
        }
        for talent in Talent::ALL {
            assert_eq!(Talent::from_code(talent.code()), Some(talent));
        }
        for class in Class::ALL {
            assert_eq!(Class::from_code(class.code()), Some(class));
        }
    }

    #[test]
    fn a_class_owns_its_abilities_and_nothing_else() {
        assert!(can(Class::Engineer, Ability::Deploy));
        assert!(!can(Class::Engineer, Ability::Brace));
        assert!(!can(Class::Engineer, Ability::Throw));
        assert!(!can(Class::Soldier, Ability::Deploy));
        assert!(can(Class::Soldier, Ability::Brace));
        assert!(can(Class::Soldier, Ability::Throw));
        assert!(!can(Class::Soldier, Ability::Beam));
        assert!(can(Class::Medic, Ability::Beam));
        assert!(can(Class::Medic, Ability::Surge));
        assert!(!can(Class::Medic, Ability::Deploy));
        assert!(!can(Class::Medic, Ability::Throw));
        assert!(!can(Class::Engineer, Ability::Surge));
        assert!(can(Class::Tank, Ability::Bulwark));
        assert!(can(Class::Tank, Ability::Taunt));
        assert!(!can(Class::Tank, Ability::Beam));
        assert!(!can(Class::Tank, Ability::Deploy));
        assert!(!can(Class::Medic, Ability::Bulwark));
        assert!(!can(Class::Soldier, Ability::Taunt));
        assert!(can(Class::Commander, Ability::SquadOrder));
        assert!(can(Class::Commander, Ability::Rally));
        assert!(!can(Class::Commander, Ability::Taunt));
        assert!(!can(Class::Commander, Ability::Deploy));
        assert!(!can(Class::Tank, Ability::SquadOrder));
        assert!(!can(Class::Medic, Ability::Rally));
        for ability in Ability::ALL {
            assert!(!can(Class::None, ability));
        }
        assert_eq!(Talent::FieldDressing.class(), Class::Medic);
        assert_eq!(Talent::Rampage.class(), Class::Soldier);
        assert_eq!(Talent::Plated.class(), Class::Tank);
        assert_eq!(Talent::RallyingWall.class(), Class::Tank);
        assert_eq!(Talent::WidePresence.class(), Class::Commander);
        assert_eq!(Talent::Warcry.class(), Class::Commander);
        assert_eq!(steady_aim_walking(), 0.75);
        // An aura bonus deepens by what it adds, up and down.
        assert!((aura_bonus(AURA_WORK, STRONG_PRESENCE) - 1.15).abs() < 1e-6);
        assert!((aura_bonus(AURA_WORK, ANCHOR_BONUS) - 1.2).abs() < 1e-6);
        assert!((aura_bonus(STEADY_RANKS_BLEED, STRONG_PRESENCE) - 0.625).abs() < 1e-6);
        assert_eq!(aura_bonus(0.0, 100.0), 0.0, "never past nothing");
    }

    /// Every class's own two keys are learnt at the same two levels —
    /// the E from the first, the Q from the third — and a classless
    /// crew member has neither.
    #[test]
    fn a_class_s_e_is_its_first_level_and_its_q_its_third() {
        for class in Class::ALL {
            if class == Class::None {
                assert_eq!(key_level(class, true), None);
                assert_eq!(key_level(class, false), None);
                continue;
            }
            assert_eq!(key_level(class, false), Some(1), "{class:?}'s E");
            assert_eq!(key_level(class, true), Some(3), "{class:?}'s Q");
            // And the third is a fixed level, so nobody has to pick it.
            assert!(!is_pick_level(class, 3));
        }
        assert_eq!(key_level(Class::Engineer, true), Some(SENTRY_LEVEL));
        assert_eq!(key_level(Class::Soldier, true), Some(GRENADE_LEVEL));
        assert_eq!(key_level(Class::Medic, true), Some(SURGE_LEVEL));
        assert_eq!(key_level(Class::Tank, true), Some(TAUNT_LEVEL));
        assert_eq!(key_level(Class::Commander, true), Some(RALLY_LEVEL));
    }
}
