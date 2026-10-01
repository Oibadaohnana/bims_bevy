//! Classes: what a player's crew member is, and what it learns (features
//! 74 to 78 and 88; every class a ranked kit since task 139).
//!
//! A crew member has **one class**, chosen by its player before the game
//! opens — a [`Class`] per player slot, kept on the world with the start
//! like the seed and changeable by `Command::SetClass` until the ship
//! first leaves its berth — and one [`Progress`] through it: experience,
//! the level that makes, and the ranks bought on the way up. A crew
//! member nobody steers, a hire, a station's resident has
//! [`Class::None`] and learns nothing.
//!
//! **A class owns abilities, never jobs or money.** Every Bim can do
//! every job, take every errand, place every site and use every weapon,
//! and every Bim brings the same money to the pool whatever its class.
//! A class only adds its own four abilities, which no other class can
//! use: [`can`] is the one rule, and it answers for an [`Ability`] and
//! nothing else.
//!
//! # Nothing is crafted for an ability (feature 90)
//!
//! What an ability spends is a [`Charge`]: a **counter** the world keeps
//! for each crew member (`World::charges_held`, task 127), never a thing
//! in a pack, that comes back on a cooldown of its own. A player never
//! stands at a bench or at a dock to use a skill — the engineer's
//! sandbags, Healing Sentry and EMP, the soldier's grenades
//! ([`GRENADE_CHARGES`] of them at [`GRENADE_COOLDOWN`] a charge) — and
//! what a class added later spends goes in [`Charge`] beside them. There
//! is no medicine to carry since task 120: a downed crewmate is revived
//! by standing beside it, and a medic's heal beam puts hit points back.
//! The abilities that spend nothing are held instead of thrown — a
//! brace, a beam, a bulwark — and the ones that are
//! neither wait out a cooldown of their own: the tank's Taunt and
//! Juggernaut at [`TAUNT_COOLDOWN`] and [`JUGGERNAUT_COOLDOWN`] of their
//! ranks, the commander's Battle Cry and Rally at [`BATTLE_CRY_COOLDOWN`]
//! and [`RALLY_COOLDOWN`] of theirs, the medic's Nanite Burst and Cloak
//! at [`NANITE_BURST_COOLDOWN`] and [`CLOAK_COOLDOWN`] of theirs.
//!
//! # Experience
//!
//! **The same for every class** (task 119). One thing gives it, and
//! nothing else:
//!
//! | what | xp | who |
//! |---|---|---|
//! | an enemy goes down within [`VICINITY_TILES`] — downed, or dead without being down first | [`XP_ENEMY_DOWN`] | every classed crew member in range |
//!
//! Each enemy counts once, at its first down or death: a machine
//! destroyed and a Manufacturer downed are worth the same twenty, and a
//! downed one dying after is worth nothing more. A crewmate or a
//! mercenary going down gives nothing. The vicinity is measured on the deck the fight is
//! on, between the crew member and the enemy. No class has a source of
//! its own: building, laying a kit, healing, taking hits and hiring gave
//! the engineer, the medic, the tank and the commander experience of
//! their own until task 119, and give nobody any now.
//!
//! # Levels and ranks
//!
//! **Every class is a ranked kit** — the soldier since task 124, the
//! engineer 127, the commander 129, the medic 130 and the tank 139, which
//! took the old left-and-right talents away whole. Every class climbs
//! the same **sixteen** levels ([`LEVELS`]) on [`LEVEL_XP`] — 100 for the
//! second, up to 3 200 for the sixteenth — and earns **one skill point a
//! level**, the first included. A point buys one **rank** of one of four
//! abilities — Q, C, E and R, [`MAX_RANK`] ranks each — with
//! `Command::RankUp` ([`Progress::rank_up`]): in the game **Ctrl and the
//! ability's key, or a Ctrl-click on its box**, or the button on the
//! character sheet's Skills tab. A rank of Q, C or E wants level `2n − 1`
//! (1, 3, 5, 7); a rank of the ultimate R wants level 6, 9, 12 or 15
//! ([`rank_level`]). Points not spent carry over, without a limit, and a
//! rank is never taken back. A level reached is `WorldEvent::LevelUp`,
//! said once. Levels and ranks are kept through a death. A classless
//! crew member has no progression at all.
//!
//! # The engineer's four slots (task 127)
//!
//! A ranked kit like the soldier's: sixteen levels on
//! [`LEVEL_XP`], a skill point a level, Q, C and E rank `n` at
//! level `2n − 1` and the ultimate R at 6, 9, 12 and 15
//! ([`rank_level`]). Every number is a table here, one a rank, read with
//! [`by_rank`]; the deploy times are game minutes of working steps.
//!
//! **Q, EMP** — thrown like a grenade ([`GRENADE_RANGE`],
//! [`GRENADE_FUSE`]), no damage; every enemy machine within the radius
//! is stunned (`bims::droid::Droid::stun`), bar the Machine Heart's:
//!
//! | rank | radius | stun | charges | cooldown a charge |
//! |---|---|---|---|---|
//! | 1 | 2.0 tiles | 1.5 s | 1 | 30 s |
//! | 2 | 2.5 tiles | 2.0 s | 2 | 30 s |
//! | 3 | 2.5 tiles | 2.5 s | 2 | 25 s |
//! | 4 | 3.0 tiles | 3.0 s | 2 | 20 s — and a machine stunned by it takes +25% from everyone |
//!
//! **C, Healing Sentry** — laid like sandbags; heals every crew Bim on
//! its feet within its radius and its sight, below its full bar, at a
//! share of the medic's beam ([`HEAL_BEAM_HP`]); several reaching one
//! Bim do not stack, and the charges are the standing limit:
//!
//! | rank | heal (× beam) | radius | health | deploy | charges | cooldown a charge |
//! |---|---|---|---|---|---|---|
//! | 1 | 0.5 | 3 tiles | 60 | 6 min | 1 | 60 s |
//! | 2 | 0.75 | 4 tiles | 80 | 6 min | 1 | 60 s |
//! | 3 | 1.0 | 4 tiles | 100 | 4 min | 1 | 50 s |
//! | 4 | 1.25 | 5 tiles | 120 | 4 min | 1 | 40 s |
//!
//! **E, Sandbags** — no limit on how many stand:
//!
//! | rank | charges | bag health | deploy | cooldown a charge | extra |
//! |---|---|---|---|---|---|
//! | 1 | 2 | 150 | 4 min | 45 s | — |
//! | 2 | 3 | 200 | 4 min | 45 s | — |
//! | 3 | 3 | 250 | 2 min | 40 s | — |
//! | 4 | 4 | 250 | 2 min | 35 s | one charge lays two tiles |
//!
//! **R, Sentry** (the ultimate) — laid in 3 min that a hit does not
//! interrupt, one standing, gone when its time runs out, never packed
//! up; the cooldown runs from the laying, ready at every mission's start:
//!
//! | rank | weapon | fire rate | health | lasts | cooldown |
//! |---|---|---|---|---|---|
//! | 1 | minigun, tier 2 | ×1.0 | 200 | 30 s | 150 s |
//! | 2 | minigun, tier 3 | ×1.0 | 250 | 35 s | 140 s |
//! | 3 | minigun, tier 3 | ×1.5 | 300 | 40 s | 130 s |
//! | 4 | minigun, tier 3 | ×2.0 | 400 | 45 s | 120 s |
//!
//! A hit on the engineer interrupts laying sandbags or a Healing Sentry
//! at every rank.
//!
//! # The soldier's four abilities (task 124)
//!
//! The first ranked kit: sixteen levels on [`LEVEL_XP`], a skill point a
//! level, the gates of "Levels and ranks" above.
//!
//! **Q, Frag Grenade** (active, charges; range [`GRENADE_RANGE`] tiles,
//! fuse [`GRENADE_FUSE`] seconds):
//!
//! | rank | damage | radius | charges | cooldown a charge |
//! |---|---|---|---|---|
//! | 1 | 60 | 2.0 | 1 | 30 s |
//! | 2 | 75 | 2.5 | 2 | 30 s |
//! | 3 | 90 | 2.5 | 2 | 24 s |
//! | 4 | 110 | 3.0 | 2 | 20 s |
//!
//! **C, Weak Spot** (passive): every weapon hit it lands on an enemy — a
//! bolt, each bolt of a burst on its own, a schword's or a fist's blow,
//! never a grenade — may be **critical**: the weapon's flat damage at the
//! distance flown times the crit damage less one is added, after every
//! relic and ability factor and before the armour, so a relic's share is
//! never multiplied by the crit.
//!
//! | rank | crit chance | crit damage |
//! |---|---|---|
//! | 1 | 10% | 150% |
//! | 2 | 12% | 175% |
//! | 3 | 15% | 200% |
//! | 4 | 20% | 225% |
//!
//! **E, Brace** (toggle): holds where it stands. Its misses are cut by a
//! share — the miss chance times one less the share, near and far, the
//! hit chance never past one — and it takes less damage, before armour.
//!
//! | rank | misses cut by | damage taken |
//! |---|---|---|
//! | 1 | 20% | — |
//! | 2 | 30% | ×0.90 |
//! | 3 | 40% | ×0.85 |
//! | 4 | 50%, and far aim equals near | ×0.80 |
//!
//! **R, Rampage** (ultimate, active; ready at every mission's start, and
//! may be used braced): the fire rate up, the damage taken down and full
//! aim on the move, for its seconds of the mission clock.
//!
//! | rank | duration | fire rate | damage taken | cooldown |
//! |---|---|---|---|---|
//! | 1 | 8 s | ×1.5 | ×0.80 | 150 s |
//! | 2 | 10 s | ×1.75 | ×0.75 | 135 s |
//! | 3 | 12 s | ×2.0 | ×0.70 | 120 s |
//! | 4 | 12 s, +1 s a machine it downs during it, +6 s at most | ×2.0 | ×0.70 | 120 s |
//!
//! # The medic's four slots (task 130)
//!
//! A ranked kit like the soldier's, the engineer's and the commander's:
//! sixteen levels on [`LEVEL_XP`], a skill point a level, Q, C and
//! E rank `n` at level `2n − 1` and the ultimate R at 6, 9, 12 and 15.
//! **Two base traits** are his whatever his ranks: he revives a downed
//! crewmate in [`MEDIC_REVIVE_SECONDS`], and a Bim he revives gets up at
//! [`MEDIC_REVIVED_TO`] of its bar where anybody else's is at
//! `bims::health::REVIVED_TO`. There is no surge.
//!
//! **Q, Nanite Burst** (active, cooldown): every friendly Bim on its feet
//! within its radius of him and in his sight — walls block — himself
//! included, is healed at once. It revives nobody.
//!
//! | rank | heal | radius | cooldown |
//! |---|---|---|---|
//! | 1 | 30 HP | 4 tiles | 25 s |
//! | 2 | 40 HP | 4 tiles | 22 s |
//! | 3 | 50 HP | 5 tiles | 20 s |
//! | 4 | 60 HP | 6 tiles | 18 s |
//!
//! **C, Healing Aura** (passive): every friendly Bim within its radius of
//! a medic on his feet, himself included, takes more from every heal —
//! the beam, the burst, a Healing Sentry, a relic: everything that goes
//! through `Health::heal` (`World::heal_factor`). Where the healed Bim
//! stands is what counts, not where the heal comes from; two medics
//! reaching one Bim, the higher factor; a revive is not a heal.
//!
//! | rank | healing received | radius |
//! |---|---|---|
//! | 1 | ×1.15 | 5 tiles |
//! | 2 | ×1.20 | 6 tiles |
//! | 3 | ×1.25 | 7 tiles |
//! | 4 | ×1.30 | 8 tiles |
//!
//! **E, Heal Beam** (active, toggle): the beam of feature 76, on a
//! crewmate or himself, at [`HEAL_BEAM_HP`] an hour of the clock times
//! the rank's rate:
//!
//! | rank | rate | a second at 1× | range | patients | fires while beaming |
//! |---|---|---|---|---|---|
//! | 1 | ×1.0 | 2 HP | 6 tiles | 1 | no |
//! | 2 | ×1.5 | 3 HP | 7 tiles | 1 | no |
//! | 3 | ×2.0 | 4 HP | 8 tiles | 1 | yes, at fire rate ×0.5 |
//! | 4 | ×2.5 | 5 HP | 9 tiles | 2, each at the full rate | yes, at fire rate ×0.5 |
//!
//! **R, Cloak** (ultimate, cooldown): the friendly Bim under the pointer
//! within [`CLOAK_RANGE`] tiles and in his sight — downed or not — or,
//! with nobody there, himself. While it lasts no enemy picks it, it fires
//! nothing and uses no ability, and it walks faster; what targets nobody —
//! a sweep, a burst — still hits it. A cloak on a Bim already cloaked
//! takes the longer of the two times.
//!
//! | rank | lasts | move speed | cooldown |
//! |---|---|---|---|
//! | 1 | 6 s | ×1.10 | 60 s |
//! | 2 | 7 s | ×1.15 | 55 s |
//! | 3 | 8 s | ×1.20 | 50 s |
//! | 4 | 10 s | ×1.25 | 45 s |
//!
//! Both cooldowns run on the mission clock, stop while paused, are ready
//! at every mission's start and are shortened by the cooldown relics as
//! every class cooldown is. See [`crate::medic`].
//!
//! # The tank's four slots (task 139)
//!
//! A ranked kit like the others': sixteen levels on [`LEVEL_XP`], a
//! skill point a level, Q, C and E rank `n` at level `2n − 1` and the
//! ultimate R at 6, 9, 12 and 15. **Two base traits** are his whatever
//! his ranks: armour he wears drains at [`TANK_DRAIN`] — half the rate,
//! so the same kevlar takes twice as much on him — and he sets out with
//! the pistol and a basic helm, kevlar and leg guards on.
//!
//! **Q, Taunt** (active, cooldown): every enemy within the radius that
//! can see him shoots at him and nobody else for its seconds. An enemy
//! taunted by two tanks follows the most recent taunt.
//!
//! | rank | radius | lasts | cooldown | extra |
//! |---|---|---|---|---|
//! | 1 | 6 tiles | 3 s | 20 s | — |
//! | 2 | 8 tiles | 4 s | 18 s | — |
//! | 3 | 10 tiles | 5 s | 16 s | — |
//! | 4 | 12 tiles | 6 s | 14 s | every charging blade within the radius turns toward him |
//!
//! **C, Plated** (passive): the damage of every hit on him multiplied
//! down, before the armour takes its share — with Brace, Rally and
//! Juggernaut, every factor multiplied together.
//!
//! | rank | damage taken | extra |
//! |---|---|---|
//! | 1 | ×0.90 | — |
//! | 2 | ×0.85 | — |
//! | 3 | ×0.80 | — |
//! | 4 | ×0.75 | armour drain on him ×0.5 again, a quarter in all |
//!
//! **E, Bulwark** (toggle): he stands as a wall, and crew within its
//! reach close behind him are in cover against anything shot through
//! him.
//!
//! | rank | reach | move speed while on | extra |
//! |---|---|---|---|
//! | 1 | 1.5 tiles | ×0.5 | — |
//! | 2 | 2.0 tiles | ×0.6 | — |
//! | 3 | 2.5 tiles | ×0.7 | dodge +10% while on |
//! | 4 | 3.0 tiles | ×0.8 | dodge +10% while on; a bolt that would hit a Bim he shields hits him instead |
//!
//! **R, Juggernaut** (ultimate, cooldown): for its seconds every enemy
//! that can see him shoots at him and nobody else, at any distance —
//! the taunt's rule without its radius — and he takes less damage. He
//! moves at his own pace, or at Bulwark's with the wall up. It runs
//! beside a Taunt, each on its own timer.
//!
//! | rank | lasts | damage taken | cooldown |
//! |---|---|---|---|
//! | 1 | 6 s | ×0.50 | 150 s |
//! | 2 | 7 s | ×0.40 | 140 s |
//! | 3 | 8 s | ×0.35 | 130 s |
//! | 4 | 10 s | ×0.30 | 120 s |
//!
//! Both cooldowns run on the mission clock, stop while paused, are ready
//! at every mission's start and are shortened by the cooldown relics as
//! every class cooldown is. A taunt or a Juggernaut on a tank a medic
//! has cloaked forces nothing while the cloak lasts. See [`crate::tank`].
//!
//! # The commander's four slots (task 129)
//!
//! A ranked kit like the soldier's and the engineer's: sixteen levels on
//! [`LEVEL_XP`], a skill point a level, Q, C and E rank `n` at
//! level `2n − 1` and the ultimate R at 6, 9, 12 and 15. **One base
//! trait** is his whatever his ranks: every mercenary he hires at
//! [`HIRE_DISCOUNT_PERCENT`] off. (His squad orders — attack, fall back,
//! stand ground — were removed.)
//!
//! **Q, Battle Cry** (active, cooldown): every friendly Bim within
//! [`BATTLE_CRY_TILES`] of him **when he calls it** — himself, a player's
//! Bim, a bot, a hired hand, a reinforcement; never a sentry — fires
//! faster for its seconds. The reach is fixed at the call: one that walks
//! out keeps it, one that walks in does not get it.
//!
//! | rank | fire rate | lasts | cooldown |
//! |---|---|---|---|
//! | 1 | ×1.25 | 3 s | 20 s |
//! | 2 | ×1.30 | 4 s | 18 s |
//! | 3 | ×1.35 | 5 s | 16 s |
//! | 4 | ×1.40 | 6 s | 14 s |
//!
//! **C, Medivac** (active, cooldown): he calls a medic of the Republic's
//! in, on the free deck nearest him within [`REINFORCEMENT_REACH_TILES`]
//! — a classless crew member with the pistol, for that mission alone, a
//! reinforcement like the R's (`World::reinforcements`, marked `medic`),
//! who fights as any bot does and **runs to a player who goes down and
//! revives him** whatever the fight round the body, in a medic's time.
//! Those called before stay.
//!
//! | rank | armour | cooldown |
//! |---|---|---|
//! | 1 | none | 140 s |
//! | 2 | a tier-one plate vest | 130 s |
//! | 3 | a tier-three plate vest | 120 s |
//! | 4 | tier-three helm, vest and leg guards | 110 s |
//!
//! **E, Rally** (active, cooldown): every friendly Bim within
//! [`RALLY_TILES`] of him when he calls it, himself included, takes less
//! damage and moves faster for its seconds, the reach fixed at the call as
//! Battle Cry's is.
//!
//! | rank | damage taken | move speed | lasts | cooldown |
//! |---|---|---|---|---|
//! | 1 | ×0.85 | ×1.10 | 6 s | 45 s |
//! | 2 | ×0.80 | ×1.15 | 7 s | 40 s |
//! | 3 | ×0.75 | ×1.20 | 8 s | 35 s |
//! | 4 | ×0.70 | ×1.20 | 9 s | 30 s |
//!
//! **R, Reinforcements** (the ultimate, pressed): he calls Bims of the
//! Republic's in, on free deck next to him within
//! [`REINFORCEMENT_REACH_TILES`] — fewer where the tiles are short — each
//! a classless crew member with the rank's auto rifle and nothing to wear,
//! for that mission alone (`World::reinforcements`), as often as
//! [`REINFORCEMENT_COOLDOWN`] (140 s at every rank) lets him — those already
//! called stay:
//!
//! | rank | Bims | weapon |
//! |---|---|---|
//! | 1 | 2 | auto rifle, tier 1 |
//! | 2 | 3 | auto rifle, tier 1 |
//! | 3 | 3 | auto rifle, tier 2 |
//! | 4 | 4 | auto rifle, tier 3 |
//!
//! Both cooldowns run on the mission clock, stop while paused, are ready
//! at every mission's start and are shortened by the cooldown relics as
//! every class cooldown is.
//!
//! **His cry and his rally lift every friendly Bim they reach, a
//! player's own steered Bims included** — the crew's own bots, the hired
//! hands and the reinforcements alike. See [`crate::commander`].
//!
//! Every multiplier is a named constant here; what each rank *does* is
//! `crate::deploy` and the world's step for the engineer, the room's
//! one shooter (`bims::combat::Skill`, `World::skill_of`) for the
//! soldier, `crate::medic` with the world's step for the medic, and
//! `crate::tank` with the same `Skill`, the room's own bulwarks and its
//! taunts for the tank, and `crate::commander` with the same `Skill` for
//! the commander. No strings: the app
//! names the classes and the abilities (`CLASS_NAMES`,
//! `ranked_ability`).

use crate::event::Refusal;

/// What a crew member is. Codes cross the seam and are never renumbered.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Class {
    /// No class: learns nothing, lays nothing. Everybody's until chosen.
    #[default]
    None = 0,
    /// The engineer: an EMP, a Healing Sentry, sandbags and a sentry.
    Engineer = 1,
    /// The soldier: a line held, and grenades.
    Soldier = 2,
    /// The medic: a Nanite Burst, a healing aura, a heal beam and a
    /// cloak.
    Medic = 3,
    /// The tank: a taunt, plating, a wall the crew shelter behind, and a
    /// Juggernaut.
    Tank = 4,
    /// The commander: a battle cry, a medic called in, a rally,
    /// reinforcements, and a cheaper hand at the dock.
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

    /// Whether a player has to **unlock** it before it can be picked
    /// (feature 106, `crate::relic::Profile`): a won run unlocks relics,
    /// and a class added later can be marked here to be unlocked the same
    /// way. Every class there is now is open from the start.
    pub fn unlockable(self) -> bool {
        match self {
            Class::None
            | Class::Engineer
            | Class::Soldier
            | Class::Medic
            | Class::Tank
            | Class::Commander => false,
        }
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
    /// Lay sandbags or a Healing Sentry, pack one up: the engineer's.
    Deploy,
    /// Throw an EMP: the engineer's (task 127).
    Emp,
    /// Lay the sentry: the engineer's ultimate (task 127).
    Sentry,
    /// Hold a line: the soldier's.
    Brace,
    /// Throw a grenade: the soldier's.
    Throw,
    /// Hold a heal beam on a crewmate: the medic's.
    Beam,
    /// Set off a Nanite Burst: the medic's (task 130).
    NaniteBurst,
    /// Cloak a crewmate or himself: the medic's ultimate (task 130).
    Cloak,
    /// Stand as a wall the crew behind shelter against: the tank's.
    Bulwark,
    /// Draw the enemy's fire onto himself: the tank's.
    Taunt,
    /// Draw every enemy's fire at any distance and shrug it off: the
    /// tank's ultimate (task 139).
    Juggernaut,
    /// Call a rally: the commander's.
    Rally,
    /// Call a battle cry: the commander's (task 129).
    BattleCry,
    /// Call reinforcements in: the commander's ultimate.
    Reinforce,
    /// Call a medic of the Republic's in: the commander's C.
    Medivac,
    /// Go on a rampage: the soldier's ultimate (task 124).
    Rampage,
}

impl Ability {
    pub const ALL: [Ability; 16] = [
        Ability::Deploy,
        Ability::Emp,
        Ability::Sentry,
        Ability::Brace,
        Ability::Throw,
        Ability::Beam,
        Ability::NaniteBurst,
        Ability::Cloak,
        Ability::Bulwark,
        Ability::Taunt,
        Ability::Juggernaut,
        Ability::Rally,
        Ability::Rampage,
        Ability::BattleCry,
        Ability::Reinforce,
        Ability::Medivac,
    ];
}

/// Whether a class may use an ability. The one rule a class gates
/// anything by: everything not an [`Ability`] is everybody's.
pub fn can(class: Class, ability: Ability) -> bool {
    match ability {
        Ability::Deploy | Ability::Emp | Ability::Sentry => class == Class::Engineer,
        Ability::Brace | Ability::Throw | Ability::Rampage => class == Class::Soldier,
        Ability::Beam | Ability::NaniteBurst | Ability::Cloak => class == Class::Medic,
        Ability::Bulwark | Ability::Taunt | Ability::Juggernaut => class == Class::Tank,
        Ability::Rally | Ability::BattleCry | Ability::Reinforce | Ability::Medivac => {
            class == Class::Commander
        }
    }
}

/// What an ability **spends**: a **counter** the world keeps for each
/// crew member (`World::charges_held`, task 127) — never a thing in a
/// pack — that comes back on a cooldown of its own rather than being
/// made at a bench or bought at a dock (features 88 and 90). **No class
/// crafts for its abilities**: a class brings its charges with it,
/// spends them, and waits — the engineer's sandbags, Healing Sentry and
/// EMP, the soldier's grenade, and whatever a class added later spends.
///
/// The number of charges and the seconds one takes to come back are
/// [`World::charges`](crate::World::charges) and
/// [`World::charge_cooldown`](crate::World::charge_cooldown), since both
/// are a rank away from the tables here; `World::restock_charges` is the
/// one step that raises a counter back.
///
/// The engineer's sentry kit (1) went with the kits in task 127 — the
/// sentry is an ultimate on a cooldown now — and the medkit and the
/// bandage were everybody's charges until task 120 took the medicine
/// out of the game. Codes cross the seam and are never renumbered, and a
/// counter is indexed by the code ([`Charge::CODES`] of them).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Charge {
    /// The engineer's sandbags (E).
    Sandbag = 0,
    /// The soldier's grenade (feature 90, Q).
    Grenade = 2,
    /// The engineer's Healing Sentry (C, task 127).
    HealingSentry = 3,
    /// The engineer's EMP (Q, task 127).
    Emp = 4,
}

impl Charge {
    pub const ALL: [Charge; 4] = [
        Charge::Sandbag,
        Charge::Grenade,
        Charge::HealingSentry,
        Charge::Emp,
    ];

    /// How many codes there are, the free ones counted: what a counter or
    /// a cooldown kept a charge is sized by, indexed by [`Charge::code`].
    pub const CODES: usize = 5;

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<Charge> {
        Charge::ALL.into_iter().find(|c| c.code() == code)
    }

    /// The one class that spends it.
    pub fn class(self) -> Class {
        match self {
            Charge::Sandbag | Charge::HealingSentry | Charge::Emp => Class::Engineer,
            Charge::Grenade => Class::Soldier,
        }
    }

    /// The ability slot whose rank says how many there are and how fast
    /// they come back — a charge nothing can spend yet, at rank nought,
    /// does not come back either.
    pub fn slot(self) -> u8 {
        match self {
            Charge::Grenade | Charge::Emp => SLOT_Q,
            Charge::HealingSentry => SLOT_C,
            Charge::Sandbag => SLOT_E,
        }
    }
}

// The left-and-right talents went with the tank's ranked kit (task 139),
// the last class that had them: `Side`, `Talent` (codes 0 to 68, never
// to be reused), the pick levels and `Command::PickTalent`.

/// How many levels every class climbs (task 139; the ranked kits'
/// sixteen since task 124).
pub const LEVELS: u8 = 16;

/// Cumulative experience for each level, by level less one: nothing for
/// the first, 100 for the second, up to 3 200 for the sixteenth — every
/// class's (task 139; `RANKED_LEVEL_XP` until then).
pub const LEVEL_XP: [u32; LEVELS as usize] = [
    0, 100, 220, 360, 520, 700, 900, 1_110, 1_330, 1_560, 1_800, 2_050, 2_310, 2_580, 2_870, 3_200,
];

/// The ranks an ability of a ranked kit has.
pub const MAX_RANK: u8 = 4;

/// The four ability slots of a ranked kit, by key: Q, C, E and R. What
/// `Command::RankUp`'s `ability_slot` and `Progress::ranks` count by.
pub const SLOT_Q: u8 = 0;
pub const SLOT_C: u8 = 1;
pub const SLOT_E: u8 = 2;
pub const SLOT_R: u8 = 3;
/// How many slots a ranked kit has.
pub const SLOTS: usize = 4;

/// The level each rank of the ultimate — slot R — wants.
pub const ULTIMATE_LEVELS: [u8; MAX_RANK as usize] = [6, 9, 12, 15];

/// Whether a class has a **ranked kit**: four abilities bought a rank at
/// a time with a skill point a level. Every class since the tank's
/// (task 139); [`Class::None`] alone has nothing to learn.
pub fn ranked(class: Class) -> bool {
    class != Class::None
}

/// The level a rank of an ability slot of a ranked kit wants: Q, C and E
/// rank `n` at level `2n − 1`, the ultimate R at [`ULTIMATE_LEVELS`].
/// `None` for a class with no ranked kit, a slot past R, and a rank that
/// is not one to buy (nought, or past [`MAX_RANK`]).
pub fn rank_level(class: Class, ability_slot: u8, rank: u8) -> Option<u8> {
    if !ranked(class) || ability_slot as usize >= SLOTS || !(1..=MAX_RANK).contains(&rank) {
        return None;
    }
    Some(if ability_slot == SLOT_R {
        ULTIMATE_LEVELS[rank as usize - 1]
    } else {
        2 * rank - 1
    })
}

/// A rank's number off a table, one a rank: `None` at rank nought, the
/// ability not learnt, and the top at anything past it. The tables of
/// the ultimates are [`OVERRIDE_RANK`] long, the rest [`MAX_RANK`].
pub fn by_rank<T: Copy, const N: usize>(table: [T; N], rank: u8) -> Option<T> {
    match rank {
        0 => None,
        r => Some(table[(r as usize).min(N) - 1]),
    }
}

/// The rank of the ultimate no skill point buys (October 2026): what an
/// *Override Core* carried makes of a fourth (`bims::module`), one over
/// whatever is bought. Every ultimate's table has a row for it.
pub const OVERRIDE_RANK: u8 = 5;

/// The ultimate's rank with `bought` ranks of it and an *Override Core*
/// carried or not: one higher with it, up to [`OVERRIDE_RANK`], and
/// nothing without a rank bought.
pub fn ultimate_rank(bought: u8, override_core: bool) -> u8 {
    if override_core && bought > 0 {
        (bought + 1).min(OVERRIDE_RANK)
    } else {
        bought
    }
}

/// What an enemy going down within the vicinity is worth, to every
/// classed crew member in range, whatever its class: once an enemy — out
/// cold, or dead without being down first, which is every machine. A
/// Manufacturer downed is worth it, a machine destroyed the same, and
/// neither is worth anything more when it dies after: the death's own
/// `XP_ENEMY_DEAD` (task 119) went, and this went from ten to twenty,
/// then down to fifteen.
pub const XP_ENEMY_DOWN: u32 = 15;
/// How far the vicinity reaches, in tiles.
pub const VICINITY_TILES: f32 = 50.0;

// --- the engineer's numbers (task 127) ----------------------------------------
//
// One number a rank, ranks one to four, read with [`by_rank`] like the
// soldier's. The deploy times are game minutes of working steps, one a
// second at 1× (what `deploy::DEPLOY_SANDBAG_MINUTES` was).

/// **Q, EMP**: how far the burst reaches, in tiles, a rank. It is thrown
/// the grenade's way — [`GRENADE_RANGE`], [`GRENADE_FUSE`] — and does no
/// damage.
pub const EMP_RADIUS: [f32; 4] = [2.0, 2.5, 2.5, 3.0];
/// Seconds of the mission clock a machine in the burst is stunned, a rank.
pub const EMP_STUN: [f32; 4] = [1.5, 2.0, 2.5, 3.0];
/// **EMP charges** a rank ([`Charge::Emp`]).
pub const EMP_CHARGES: [u32; 4] = [1, 2, 2, 2];
/// Seconds one spent EMP charge takes to come back, a rank.
pub const EMP_COOLDOWN: [f64; 4] = [30.0, 30.0, 25.0, 20.0];
/// The rank from which a machine stunned by the EMP takes more from
/// everyone while it is stunned.
pub const EMP_EXPOSE_RANK: u8 = 4;
/// How much more, in whole per cent — added to the `Stat::MachineDamage`
/// sum the relics use.
pub const EMP_EXPOSE_PERCENT: i32 = 25;

/// **C, Healing Sentry**: what it heals, a share of the medic's beam
/// ([`HEAL_BEAM_HP`] an hour), a rank — so the two stay in proportion
/// when either is tuned.
pub const HEALING_SENTRY_RATE: [f32; 4] = [0.5, 0.75, 1.0, 1.25];
/// How far it heals, in tiles, a rank.
pub const HEALING_SENTRY_RADIUS: [f32; 4] = [3.0, 4.0, 4.0, 5.0];
/// Its health, one pool, a rank.
pub const HEALING_SENTRY_HEALTH: [f32; 4] = [60.0, 80.0, 100.0, 120.0];
/// Game minutes of working steps to lay one, a rank.
pub const HEALING_SENTRY_MINUTES: [f64; 4] = [6.0, 6.0, 4.0, 4.0];
/// **Healing Sentry charges** a rank — and how many of that engineer's
/// may stand, a further one laid destroying its oldest.
pub const HEALING_SENTRY_CHARGES: [u32; 4] = [1, 1, 1, 1];
/// Seconds one spent Healing Sentry charge takes to come back, a rank.
pub const HEALING_SENTRY_COOLDOWN: [f64; 4] = [60.0, 60.0, 50.0, 40.0];

/// **E, Sandbags**: charges a rank. There is no limit on how many stand.
pub const SANDBAG_CHARGES: [u32; 4] = [2, 3, 3, 4];
/// What a laid bag can take before it is gone, a rank.
pub const SANDBAG_HEALTH: [f32; 4] = [150.0, 200.0, 250.0, 250.0];
/// Game minutes of working steps to lay one, a rank.
pub const SANDBAG_MINUTES: [f64; 4] = [4.0, 4.0, 2.0, 2.0];
/// Seconds one spent sandbag charge takes to come back, a rank.
pub const SANDBAG_COOLDOWN: [f64; 4] = [45.0, 45.0, 40.0, 35.0];
/// The rank from which one charge lays two tiles: the second on the
/// first free neighbour, north, east, south, west.
pub const SANDBAG_DOUBLE_RANK: u8 = 4;

/// **R, Sentry** (the ultimate): the minigun's tier, a rank.
pub const SENTRY_TIER: [bims::combat::Tier; 5] = [
    bims::combat::Tier::Two,
    bims::combat::Tier::Three,
    bims::combat::Tier::Three,
    bims::combat::Tier::Three,
    bims::combat::Tier::Three,
];
/// What its fire rate is multiplied by, a rank.
pub const SENTRY_FIRE_RATE: [f32; 5] = [1.0, 1.0, 1.5, 2.0, 2.0];
/// Its health, one pool, a rank.
pub const SENTRY_HEALTH: [f32; 5] = [200.0, 250.0, 300.0, 400.0, 500.0];
/// Tiles added to its minigun's range, a rank: five from the first. It
/// stands until it is destroyed — there is no timer.
pub const SENTRY_RANGE: [f32; 5] = [5.0, 5.0, 5.0, 5.0, 5.0];
/// Seconds of the mission clock from one laid to the next, a rank —
/// counted from the laying.
pub const SENTRY_COOLDOWN: [f64; 5] = ULTIMATE_COOLDOWN;
/// How many sentries one engineer may have standing, a rank: one, and two
/// at the [`OVERRIDE_RANK`] (October 2026) — the oldest goes when another
/// is laid past it.
pub const SENTRY_STANDING: [usize; 5] = [1, 1, 1, 1, 2];
/// Game minutes of working steps to lay it, at every rank. A hit does
/// not interrupt it.
pub const SENTRY_MINUTES: f64 = 3.0;

// --- the soldier's numbers (task 124) -----------------------------------------
//
// Every table is one number a rank, ranks one to four; read it with
// [`by_rank`], which answers `None` at rank nought.

/// **Q, Frag Grenade**: how far a grenade is thrown, in tiles, at every
/// rank.
pub const GRENADE_RANGE: f32 = 8.0;
/// Seconds from the throw to the burst, at every rank.
pub const GRENADE_FUSE: f32 = 2.0;
/// What the burst does at its centre, a rank; half that at the edge.
pub const GRENADE_DAMAGE: [f32; 4] = [60.0, 75.0, 90.0, 110.0];
/// How far the burst reaches, in tiles, a rank.
pub const GRENADE_RADIUS: [f32; 4] = [2.0, 2.5, 2.5, 3.0];
/// **Grenade charges** a rank (feature 90): how many grenades the soldier
/// fills back up to, one at a time on [`GRENADE_COOLDOWN`] a charge. A
/// soldier makes none and buys none — see [`Charge`].
pub const GRENADE_CHARGES: [u32; 4] = [1, 2, 2, 2];
/// Seconds of the mission clock one spent grenade charge takes to come
/// back, a rank.
pub const GRENADE_COOLDOWN: [f64; 4] = [30.0, 30.0, 24.0, 20.0];

/// **C, Weak Spot**: the chance a weapon hit on an enemy is critical, a
/// rank.
pub const WEAK_SPOT_CHANCE: [f32; 4] = [0.10, 0.12, 0.15, 0.20];
/// What a critical hit's flat damage is worth, a rank: 1.5 is 150%, so
/// half the flat damage again is added to the hit.
pub const WEAK_SPOT_DAMAGE: [f32; 4] = [1.50, 1.75, 2.00, 2.25];

/// **E, Brace**: the share of the misses a braced soldier's aim takes
/// away, a rank — the miss chance times one less it, near and far.
pub const BRACE_MISS_CUT: [f32; 4] = [0.20, 0.30, 0.40, 0.50];
/// What the damage a braced soldier takes is multiplied by, a rank.
pub const BRACE_DAMAGE_TAKEN: [f32; 4] = [1.0, 0.90, 0.85, 0.80];
/// The rank from which a braced soldier's far aim equals its near.
pub const BRACE_DEADEYE_RANK: u8 = 4;

/// **R, Rampage**: how long it runs, in seconds of the mission clock, a
/// rank.
pub const RAMPAGE_SECONDS: [f64; 5] = [8.0, 10.0, 12.0, 12.0, 14.0];
/// What the fire rate is multiplied by while it runs, a rank.
pub const RAMPAGE_FIRE_RATE: [f32; 5] = [1.5, 1.75, 2.0, 2.0, 2.25];
/// What the damage taken is multiplied by while it runs, a rank.
pub const RAMPAGE_DAMAGE_TAKEN: [f32; 5] = [0.80, 0.75, 0.70, 0.70, 0.65];
/// Seconds of the mission clock from one Rampage to the next, a rank.
pub const RAMPAGE_COOLDOWN: [f64; 5] = ULTIMATE_COOLDOWN;
/// The rank from which a machine the soldier downs during a Rampage adds
/// [`RAMPAGE_EXTEND_SECONDS`] to it.
pub const RAMPAGE_EXTEND_RANK: u8 = 4;
/// Seconds a machine downed adds to a Rampage from its fourth rank.
pub const RAMPAGE_EXTEND_SECONDS: f64 = 1.0;
/// The most one Rampage is lengthened by, in seconds.
pub const RAMPAGE_EXTEND_MAX: f64 = 6.0;

// --- the medic's numbers (task 130) -------------------------------------------
//
// One number a rank, ranks one to four, read with [`by_rank`] like the
// soldier's, the engineer's and the commander's. The seconds are seconds
// of the mission clock, one a real second at 1×.

/// How far the heal beam reaches at its first rank, in tiles: the first
/// of [`HEAL_BEAM_RANGES`].
pub const HEAL_BEAM_RANGE: f32 = 6.0;
/// Hit points a beamed patient gains an hour of the clock at the beam's
/// first rank: two a second at 1× (task 130; thirty until then). The ranks
/// multiply it ([`HEAL_BEAM_RATE`]); the engineer's Healing Sentry reads
/// it unranked.
pub const HEAL_BEAM_HP: f32 = 120.0;
/// **Base trait**: how long a medic takes to revive a downed crewmate, in
/// seconds — anybody else's is `bims::health::REVIVE_SECONDS` (task 120).
/// A medic of the class and a hired field medic alike.
pub const MEDIC_REVIVE_SECONDS: f32 = 4.0;
/// **Base trait**: the share of its bar a Bim a medic of the class
/// revives gets up at — anybody else's is `bims::health::REVIVED_TO`.
/// A relic's *Rally Point* is its own.
pub const MEDIC_REVIVED_TO: f32 = 0.4;

/// **Q, Nanite Burst**: hit points put back at once, a rank.
pub const NANITE_BURST_HEAL: [f32; 4] = [30.0, 40.0, 50.0, 60.0];
/// How far it reaches from the medic, in tiles, a rank.
pub const NANITE_BURST_RADIUS: [f32; 4] = [4.0, 4.0, 5.0, 6.0];
/// Seconds of the mission clock from one burst to the next, a rank.
pub const NANITE_BURST_COOLDOWN: [f64; 4] = [25.0, 22.0, 20.0, 18.0];

/// **C, Healing Aura**: what every heal a Bim in it takes is multiplied
/// by, a rank.
pub const HEALING_AURA_FACTOR: [f32; 4] = [1.15, 1.20, 1.25, 1.30];
/// How far the aura reaches, in tiles, a rank.
pub const HEALING_AURA_RADIUS: [f32; 4] = [5.0, 6.0, 7.0, 8.0];

/// **E, Heal Beam**: what [`HEAL_BEAM_HP`] is multiplied by, a rank.
pub const HEAL_BEAM_RATE: [f32; 4] = [1.0, 1.5, 2.0, 2.5];
/// How far the beam reaches, in tiles, a rank.
pub const HEAL_BEAM_RANGES: [f32; 4] = [HEAL_BEAM_RANGE, 7.0, 8.0, 9.0];
/// How many patients the beam holds at once, each at the full rate, a
/// rank.
pub const HEAL_BEAM_PATIENTS: [usize; 4] = [1, 1, 1, 2];
/// The rank from which a medic beaming fires as well.
pub const HEAL_BEAM_FIRE_RANK: u8 = 3;
/// What the fire rate of a medic beaming is multiplied by, from
/// [`HEAL_BEAM_FIRE_RANK`].
pub const HEAL_BEAM_FIRE_RATE: f32 = 0.5;

/// **R, Cloak**: how far from the medic a crewmate may be cloaked, in
/// tiles, at every rank.
pub const CLOAK_RANGE: f32 = 8.0;
/// Seconds of the mission clock a cloak lasts, a rank.
pub const CLOAK_SECONDS: [f64; 5] = [6.0, 7.0, 8.0, 10.0, 12.0];
/// What a cloaked Bim's pace is multiplied by, a rank.
pub const CLOAK_PACE: [f32; 5] = [1.10, 1.15, 1.20, 1.25, 1.30];
/// Seconds of the mission clock from one cloak to the next, a rank.
pub const CLOAK_COOLDOWN: [f64; 5] = [60.0, 55.0, 50.0, 45.0, 40.0];
/// At the [`OVERRIDE_RANK`] (October 2026) a cloak takes every friendly
/// Bim within this many tiles of its target as well.
pub const CLOAK_SPREAD_TILES: f32 = 3.0;

/// How long a revive takes (task 120): ten seconds, four for a `medic`,
/// less `quicker` seconds — a relic's *Trauma Kit* — and never under
/// `crate::data::REVIVE_FLOOR_SECONDS`.
pub fn revive_time(medic: bool, quicker: f32) -> f32 {
    let base = if medic {
        MEDIC_REVIVE_SECONDS
    } else {
        bims::health::REVIVE_SECONDS
    };
    (base - quicker).max(crate::data::REVIVE_FLOOR_SECONDS)
}

// --- the tank's numbers (task 139) --------------------------------------------
//
// One number a rank, ranks one to four, read with [`by_rank`] like the
// others'. The seconds are seconds of the mission clock, one a real
// second at 1×.

/// **Base trait**: what a piece of armour worn by a tank drains at: half
/// the damage it takes past its protection, so a piece absorbs twice as
/// much on him. Never doubled in the piece's own health, which moves
/// between Bims unchanged.
pub const TANK_DRAIN: f32 = 0.5;

/// **Q, Taunt**: how far it reaches, in tiles, a rank.
pub const TAUNT_RADIUS: [f32; 4] = [6.0, 8.0, 10.0, 12.0];
/// Seconds of the mission clock it runs, a rank.
pub const TAUNT_SECONDS: [f64; 4] = [3.0, 4.0, 5.0, 6.0];
/// Seconds of the mission clock from one taunt to the next, a rank.
pub const TAUNT_COOLDOWN: [f64; 4] = [20.0, 18.0, 16.0, 14.0];
/// The rank from which a taunt turns every charging blade within its
/// radius toward him (what *magnet* was).
pub const TAUNT_MAGNET_RANK: u8 = 4;

/// **C, Plated**: what the damage of a hit on him is multiplied by, a
/// rank — before the armour, as Brace's is.
pub const PLATED_DAMAGE_TAKEN: [f32; 4] = [0.90, 0.85, 0.80, 0.75];
/// The rank from which his armour drain is multiplied by
/// [`FORTRESS_DRAIN`] again (what *fortress* was).
pub const FORTRESS_RANK: u8 = 4;
/// What his armour drain is multiplied by again from [`FORTRESS_RANK`]:
/// a quarter of the rate in all.
pub const FORTRESS_DRAIN: f32 = 0.5;

/// **E, Bulwark**: how far it reaches, in tiles, a rank — how near the
/// tank a crew member must stand to shelter behind him, and how near the
/// line from the shooter he must stand to be between them.
pub const BULWARK_REACH: [f32; 4] = [1.5, 2.0, 2.5, 3.0];
/// What his pace is multiplied by while the wall is up, a rank.
pub const BULWARK_PACE: [f32; 4] = [0.5, 0.6, 0.7, 0.8];
/// The rank from which the wall adds [`GUARDED_DODGE`] to his dodge
/// while it is up (what *guarded* was).
pub const GUARDED_RANK: u8 = 3;
/// What is added to the dodge while the wall is up, from
/// [`GUARDED_RANK`].
pub const GUARDED_DODGE: f32 = 0.10;
/// The rank from which a bolt that would hit a Bim he shields hits him
/// instead (what *interpose* was).
pub const INTERPOSE_RANK: u8 = 4;

/// **R, Juggernaut**: seconds of the mission clock it runs, a rank.
pub const JUGGERNAUT_SECONDS: [f64; 5] = [6.0, 7.0, 8.0, 10.0, 12.0];
/// What the damage he takes is multiplied by while it runs, a rank.
pub const JUGGERNAUT_DAMAGE_TAKEN: [f32; 5] = [0.50, 0.40, 0.35, 0.30, 0.25];
/// Seconds of the mission clock from one Juggernaut to the next, a rank.
pub const JUGGERNAUT_COOLDOWN: [f64; 5] = ULTIMATE_COOLDOWN;

/// Every timed ultimate's cooldown, a rank: seventy seconds at the first
/// and ten fewer a rank after it (the Sentry, Rampage and Juggernaut; the
/// Cloak's own [`CLOAK_COOLDOWN`] is under it already, and Reinforcements
/// come once a mission).
pub const ULTIMATE_COOLDOWN: [f64; 5] = [70.0, 60.0, 50.0, 40.0, 35.0];

// --- the commander's numbers (task 129) ---------------------------------------
//
// One number a rank, ranks one to four, read with [`by_rank`] like the
// soldier's and the engineer's. The seconds are seconds of the mission
// clock, one a real second at 1×.

/// **Base trait**: what a commander takes off a mercenary's fee, in whole
/// per cent, at every level.
pub const HIRE_DISCOUNT_PERCENT: u32 = 25;

/// **Q, Battle Cry**: how far it reaches when he calls it, in tiles, at
/// every rank.
pub const BATTLE_CRY_TILES: f32 = 8.0;
/// What the fire rate of a Bim it reached is multiplied by, a rank.
pub const BATTLE_CRY_FIRE_RATE: [f32; 4] = [1.25, 1.30, 1.35, 1.40];
/// Seconds of the mission clock it runs, a rank.
pub const BATTLE_CRY_SECONDS: [f64; 4] = [3.0, 4.0, 5.0, 6.0];
/// Seconds of the mission clock from one cry to the next, a rank.
pub const BATTLE_CRY_COOLDOWN: [f64; 4] = [20.0, 18.0, 16.0, 14.0];

/// **C, Medivac**: seconds of the mission clock from one medic called in
/// to the next, a rank — ready at every mission's start, shortened by the
/// cooldown relics as every class cooldown is.
pub const MEDIVAC_COOLDOWN: [f64; 4] = [140.0, 130.0, 120.0, 110.0];
/// The tier of the medic's armour, a rank: none at the first, tier one
/// at the second, tier three from the third. (A helm and leg guards came
/// on at the fourth too, until a Bim wore one armour, October 2026.)
pub const MEDIVAC_VEST: [Option<bims::combat::Tier>; 4] = [
    None,
    Some(bims::combat::Tier::One),
    Some(bims::combat::Tier::Three),
    Some(bims::combat::Tier::Three),
];

/// **E, Rally**: how far it reaches when he calls it, in tiles, at every
/// rank.
pub const RALLY_TILES: f32 = 8.0;
/// What the damage a Bim it reached takes is multiplied by, a rank.
pub const RALLY_DAMAGE_TAKEN: [f32; 4] = [0.85, 0.80, 0.75, 0.70];
/// What the pace of a Bim it reached is multiplied by, a rank.
pub const RALLY_PACE: [f32; 4] = [1.10, 1.15, 1.20, 1.20];
/// Seconds of the mission clock it runs, a rank.
pub const RALLY_SECONDS: [f64; 4] = [6.0, 7.0, 8.0, 9.0];
/// Seconds of the mission clock from one rally to the next, a rank.
pub const RALLY_COOLDOWN: [f64; 4] = [45.0, 40.0, 35.0, 30.0];

/// **R, Reinforcements**: how many Bims one call brings in, a rank.
pub const REINFORCEMENTS: [u32; 5] = [2, 3, 3, 4, 5];
/// The tier of the auto rifle each carries, a rank.
pub const REINFORCEMENT_TIER: [bims::combat::Tier; 5] = [
    bims::combat::Tier::One,
    bims::combat::Tier::One,
    bims::combat::Tier::Two,
    bims::combat::Tier::Three,
    bims::combat::Tier::Three,
];
/// The armour each reinforcement wears, a rank: none until the
/// [`OVERRIDE_RANK`] (October 2026), a tier-one plate there.
pub const REINFORCEMENT_VEST: [Option<bims::combat::Tier>; 5] =
    [None, None, None, None, Some(bims::combat::Tier::One)];
/// How far from him, in tiles, a free tile of deck is looked for to stand
/// one on: fewer arrive where fewer are found.
pub const REINFORCEMENT_REACH_TILES: f32 = 5.0;
/// Seconds of the mission clock from one call for reinforcements to the
/// next, at every rank: ready at every mission's start, shortened by the
/// cooldown relics as every class cooldown is.
pub const REINFORCEMENT_COOLDOWN: f64 = 140.0;

/// The level `xp` makes: one to sixteen on [`LEVEL_XP`], whatever the
/// class.
pub fn level_of(xp: u32) -> u8 {
    LEVEL_XP.iter().filter(|&&need| xp >= need).count().max(1) as u8
}

/// One crew member's way through its class: its experience, and the
/// ranks it has bought, a count a slot of its kit — Q, C, E and R.
#[derive(Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Progress {
    /// Cumulative experience.
    pub xp: u32,
    /// The ranks bought, a slot: Q, C, E and R ([`SLOT_Q`] …), nought to
    /// [`MAX_RANK`] each. Nought for a classless crew member.
    #[cfg_attr(feature = "serde", serde(default))]
    pub ranks: [u8; SLOTS],
}

impl Progress {
    /// The level the experience makes.
    pub fn level(&self) -> u8 {
        level_of(self.xp)
    }

    /// Experience still wanted for the next level; nought at the top.
    pub fn to_next(&self) -> u32 {
        let level = self.level();
        if level as usize >= LEVEL_XP.len() {
            return 0;
        }
        LEVEL_XP[level as usize].saturating_sub(self.xp)
    }

    /// `xp` more: every level reached by it, lowest first, for the world
    /// to say. Nothing past the top level.
    pub fn gain(&mut self, xp: u32) -> Vec<u8> {
        let was = self.level();
        self.xp = self.xp.saturating_add(xp);
        let now = self.level();
        ((was + 1)..=now).collect()
    }

    /// The rank bought of an ability slot: nought for none, and for a
    /// slot past R.
    pub fn rank(&self, ability_slot: u8) -> u8 {
        self.ranks.get(ability_slot as usize).copied().unwrap_or(0)
    }

    /// Every rank bought, of every slot: the skill points spent.
    pub fn ranks_bought(&self) -> u8 {
        self.ranks.iter().sum()
    }

    /// Skill points not spent (task 124): a point a level reached, the
    /// first included, less every rank bought. Nought for a classless
    /// crew member.
    pub fn points(&self, class: Class) -> u8 {
        if !ranked(class) {
            return 0;
        }
        self.level().saturating_sub(self.ranks_bought())
    }

    /// Whether a rank of that slot may be bought now, and the rank it
    /// would be, or why not — in the order the refusals are said: a class
    /// (`NoClass`), a slot of the kit (`NoRankedKit`), a point to spend
    /// (`NoSkillPoint`), the slot not at [`MAX_RANK`] (`TopRank`), and the
    /// level the rank wants reached (`RankLocked`).
    pub fn can_rank_up(&self, class: Class, ability_slot: u8) -> Result<u8, Refusal> {
        if !ranked(class) {
            return Err(Refusal::NoClass);
        }
        if ability_slot as usize >= SLOTS {
            return Err(Refusal::NoRankedKit);
        }
        if self.points(class) == 0 {
            return Err(Refusal::NoSkillPoint);
        }
        let rank = self.rank(ability_slot);
        if rank >= MAX_RANK {
            return Err(Refusal::TopRank);
        }
        match rank_level(class, ability_slot, rank + 1) {
            Some(want) if self.level() >= want => Ok(rank + 1),
            _ => Err(Refusal::RankLocked),
        }
    }

    /// Buy a rank of that slot with a skill point: the rank it is now,
    /// or why not ([`Progress::can_rank_up`]). Never taken back.
    pub fn rank_up(&mut self, class: Class, ability_slot: u8) -> Result<u8, Refusal> {
        let rank = self.can_rank_up(class, ability_slot)?;
        self.ranks[ability_slot as usize] = rank;
        Ok(rank)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_class_owns_its_abilities_and_nothing_else() {
        assert!(can(Class::Engineer, Ability::Deploy));
        assert!(!can(Class::Engineer, Ability::Brace));
        assert!(!can(Class::Engineer, Ability::Throw));
        assert!(can(Class::Engineer, Ability::Emp) && can(Class::Engineer, Ability::Sentry));
        assert!(!can(Class::Soldier, Ability::Emp) && !can(Class::Medic, Ability::Sentry));
        assert!(!can(Class::Soldier, Ability::Deploy));
        assert!(can(Class::Soldier, Ability::Brace));
        assert!(can(Class::Soldier, Ability::Throw));
        assert!(!can(Class::Soldier, Ability::Beam));
        assert!(can(Class::Medic, Ability::Beam));
        assert!(can(Class::Medic, Ability::NaniteBurst) && can(Class::Medic, Ability::Cloak));
        assert!(!can(Class::Medic, Ability::Deploy));
        assert!(!can(Class::Medic, Ability::Throw));
        assert!(!can(Class::Engineer, Ability::NaniteBurst));
        assert!(!can(Class::Soldier, Ability::Cloak));
        assert!(can(Class::Tank, Ability::Bulwark));
        assert!(can(Class::Tank, Ability::Taunt));
        assert!(can(Class::Tank, Ability::Juggernaut));
        assert!(!can(Class::Soldier, Ability::Juggernaut));
        assert!(!can(Class::Tank, Ability::Beam));
        assert!(!can(Class::Tank, Ability::Deploy));
        assert!(!can(Class::Medic, Ability::Bulwark));
        assert!(!can(Class::Soldier, Ability::Taunt));
        assert!(can(Class::Commander, Ability::Rally));
        assert!(can(Class::Commander, Ability::BattleCry));
        assert!(!can(Class::Soldier, Ability::BattleCry));
        assert!(!can(Class::Commander, Ability::Taunt));
        assert!(!can(Class::Commander, Ability::Deploy));
        assert!(!can(Class::Medic, Ability::Rally));
        for ability in Ability::ALL {
            assert!(!can(Class::None, ability));
        }
        for class in Class::ALL {
            assert_eq!(Class::from_code(class.code()), Some(class));
        }
    }

    /// Every class is a ranked kit (task 139): sixteen levels, the top at
    /// 3 200, a point a level, and every rank's gate refusing a level
    /// early and allowing on the level — the tank's as the soldier's.
    #[test]
    fn every_class_climbs_sixteen_levels_and_buys_a_rank_a_point() {
        assert_eq!(LEVELS, 16);
        assert_eq!(LEVEL_XP.len(), 16);
        assert_eq!(LEVEL_XP[15], 3_200, "the top at 3 200");
        assert_eq!(level_of(0), 1);
        assert_eq!(level_of(99), 1);
        assert_eq!(level_of(3_199), 15);
        assert_eq!(level_of(3_200), 16);
        assert_eq!(level_of(1_000_000), 16);
        for class in Class::ALL {
            assert_eq!(ranked(class), class != Class::None, "{class:?}");
        }
        let mut p = Progress::default();
        assert_eq!(p.points(Class::None), 0, "a classless bot learns nothing");
        assert_eq!(
            p.can_rank_up(Class::None, SLOT_Q),
            Err(Refusal::NoClass),
            "nor ranks anything"
        );
        for class in Class::ALL.into_iter().filter(|&c| c != Class::None) {
            assert_eq!(p.points(class), 1, "{class:?}: a point at the first level");
            assert_eq!(p.can_rank_up(class, SLOT_Q), Ok(1), "{class:?}");
            assert_eq!(p.can_rank_up(class, SLOT_R), Err(Refusal::RankLocked));
            assert_eq!(p.can_rank_up(class, 4), Err(Refusal::NoRankedKit));
        }
        let s = Class::Tank;
        assert_eq!(p.rank_up(s, SLOT_Q), Ok(1));
        assert_eq!(p.rank_up(s, SLOT_C), Err(Refusal::NoSkillPoint));
        assert_eq!(p.gain(3_200), (2..=16).collect::<Vec<u8>>());
        assert_eq!(p.to_next(), 0);
        assert_eq!(p.points(s), 15);
        for _ in 0..3 {
            p.rank_up(s, SLOT_Q).unwrap();
        }
        assert_eq!(p.rank(SLOT_Q), MAX_RANK);
        assert_eq!(p.rank_up(s, SLOT_Q), Err(Refusal::TopRank));
        // Every gate, of every class: refused a level early, allowed on
        // the level.
        for class in Class::ALL.into_iter().filter(|&c| c != Class::None) {
            for slot in 0..SLOTS as u8 {
                for rank in 1..=MAX_RANK {
                    let want = rank_level(class, slot, rank).unwrap();
                    let mut q = Progress::default();
                    q.ranks[slot as usize] = rank - 1;
                    if want > 1 {
                        q.xp = LEVEL_XP[want as usize - 2];
                        assert_eq!(
                            q.can_rank_up(class, slot),
                            Err(Refusal::RankLocked),
                            "{class:?} {slot} {rank}"
                        );
                    }
                    q.xp = LEVEL_XP[want as usize - 1];
                    assert_eq!(
                        q.can_rank_up(class, slot),
                        Ok(rank),
                        "{class:?} {slot} {rank}"
                    );
                }
            }
        }
        assert_eq!(rank_level(s, SLOT_E, 3), Some(5));
        assert_eq!(rank_level(s, SLOT_R, 1), Some(6));
        assert_eq!(rank_level(s, SLOT_R, 4), Some(15));
        assert_eq!(rank_level(s, SLOT_Q, 5), None);
        assert_eq!(rank_level(Class::None, SLOT_Q, 1), None);
        assert_eq!(by_rank(GRENADE_DAMAGE, 0), None);
        assert_eq!(by_rank(GRENADE_DAMAGE, 4), Some(110.0));
    }

    /// The tank's tables (task 139), as the spec gives them.
    #[test]
    fn the_tank_s_tables_are_the_spec_s() {
        assert_eq!(TAUNT_RADIUS, [6.0, 8.0, 10.0, 12.0]);
        assert_eq!(TAUNT_SECONDS, [3.0, 4.0, 5.0, 6.0]);
        assert_eq!(TAUNT_COOLDOWN, [20.0, 18.0, 16.0, 14.0]);
        assert_eq!(PLATED_DAMAGE_TAKEN, [0.90, 0.85, 0.80, 0.75]);
        assert_eq!(TANK_DRAIN * FORTRESS_DRAIN, 0.25, "a quarter in all");
        assert_eq!(BULWARK_REACH, [1.5, 2.0, 2.5, 3.0]);
        assert_eq!(BULWARK_PACE, [0.5, 0.6, 0.7, 0.8]);
        assert_eq!(JUGGERNAUT_SECONDS, [6.0, 7.0, 8.0, 10.0, 12.0]);
        assert_eq!(JUGGERNAUT_DAMAGE_TAKEN, [0.50, 0.40, 0.35, 0.30, 0.25]);
        assert_eq!(JUGGERNAUT_COOLDOWN, [70.0, 60.0, 50.0, 40.0, 35.0]);
    }
}
