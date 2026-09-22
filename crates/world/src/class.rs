//! Classes: what a player's crew member is, and what it learns (feature 74).
//!
//! A crew member has **one class**, chosen by its player before the game
//! opens — a [`Class`] per player slot, kept on the world with the start
//! like the seed and changeable by `Command::SetClass` until the ship
//! first leaves its berth — and one [`Progress`] through it: experience,
//! the level that makes, and the talents picked on the way up. A
//! crew member nobody steers, a hire, a station's resident has
//! [`Class::None`] and learns nothing.
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
//!
//! Each enemy counts once for going down and once for dying; a crewmate
//! or a mercenary going down gives nothing; a kit laid from a re-used one
//! (`World::reused_kits`, a kit packed up or salvaged) gives nothing. The
//! vicinity is measured on the deck the fight is on, between the crew
//! member and the enemy, or the crew member and whoever built.
//!
//! # Levels
//!
//! Ten, off cumulative experience ([`LEVEL_XP`]): 100 for the second,
//! 250 for the third, up to 3 200 for the tenth. A level reached is
//! `WorldEvent::LevelUp`, said once. A **fixed** level's talent applies at
//! once; a **pick** level ([`pick_at`]) offers two and applies neither
//! until the player chooses — `Command::PickTalent`, only for a level
//! reached with no pick yet, never changed after. A dead crew member's
//! level, experience and picks die with it.
//!
//! # The engineer's ten levels
//!
//! | level | left | right |
//! |---|---|---|
//! | 1 | lays sandbags; packs deployables up | — |
//! | 2 | *Quick hands*: craft effort ×1.25 | *Site foreman*: build effort ×1.25 |
//! | 3 | *Sentry*: may lay one sentry | — |
//! | 4 | *Sandbagger*: sandbag deploy time ×0.5 | *Bulk bags*: one kit lays two adjacent tiles |
//! | 5 | *Armoured sentry*: sentry health ×1.5 | *Deep magazine*: sentry shots ×1.5 |
//! | 6 | *Armourer*: repairs armour at the workbench | *Field refit*: refilling a sentry costs no metal |
//! | 7 | *Sentry mark II*: the tier-two factors on its rifle | — |
//! | 8 | *Dug in*: sandbags anywhere between a sentry and the shooter are cover | *Quick build*: sentry deploy time ×0.5 |
//! | 9 | *Salvage*: a destroyed sentry returns its kit to the pack | *Steady hands*: a hit no longer interrupts a deploy |
//! | 10 | *Second sentry*: two at once | *Sentry mark III*: the tier-three factors |
//!
//! Every multiplier is a named constant here; what each talent *does* is
//! `crate::deploy` and the world's step. No strings: the app names the
//! classes and the talents (`CLASS_NAMES`, `TALENT_NAMES`).

use crate::event::Refusal;

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
}

impl Class {
    pub const ALL: [Class; 2] = [Class::None, Class::Engineer];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<Class> {
        Class::ALL.get(code as usize).copied()
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

/// The engineer's talents that are picked — the seven pick levels' two
/// each, in level order, left before right. The fixed levels (1, 3, 7)
/// are not talents: they are the level itself, asked of `Progress::level`.
/// Codes cross the seam and index `TALENT_NAMES` in the app.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Talent {
    QuickHands = 0,
    SiteForeman = 1,
    Sandbagger = 2,
    BulkBags = 3,
    ArmouredSentry = 4,
    DeepMagazine = 5,
    Armourer = 6,
    FieldRefit = 7,
    DugIn = 8,
    QuickBuild = 9,
    Salvage = 10,
    SteadyHands = 11,
    SecondSentry = 12,
    SentryMarkThree = 13,
}

impl Talent {
    pub const ALL: [Talent; 14] = [
        Talent::QuickHands,
        Talent::SiteForeman,
        Talent::Sandbagger,
        Talent::BulkBags,
        Talent::ArmouredSentry,
        Talent::DeepMagazine,
        Talent::Armourer,
        Talent::FieldRefit,
        Talent::DugIn,
        Talent::QuickBuild,
        Talent::Salvage,
        Talent::SteadyHands,
        Talent::SecondSentry,
        Talent::SentryMarkThree,
    ];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<Talent> {
        Talent::ALL.get(code as usize).copied()
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

/// The level a sentry may be laid from: the engineer's third.
pub const SENTRY_LEVEL: u8 = 3;
/// The level a sentry's rifle takes the tier-two factors from: the
/// engineer's seventh.
pub const SENTRY_MARK_TWO_LEVEL: u8 = 7;

/// *Quick hands*: what a craft's working steps run at.
pub const QUICK_HANDS_EFFORT: f32 = 1.25;
/// *Site foreman*: what a build's working steps run at.
pub const SITE_FOREMAN_EFFORT: f32 = 1.25;
/// *Sandbagger*: what the sandbag deploy time is multiplied by.
pub const SANDBAGGER_TIME: f64 = 0.5;
/// *Armoured sentry*: what a sentry's health is multiplied by.
pub const ARMOURED_SENTRY_HEALTH: f32 = 1.5;
/// *Deep magazine*: what a sentry's shots are multiplied by.
pub const DEEP_MAGAZINE_SHOTS: f32 = 1.5;
/// *Quick build*: what the sentry deploy time is multiplied by.
pub const QUICK_BUILD_TIME: f64 = 0.5;
/// *Armourer*: what one metal at the workbench puts back on a piece.
pub const ARMOUR_REPAIR_PER_METAL: f32 = 10.0;
/// *Armourer*: how long the repair takes at the bench, in game minutes.
pub const ARMOUR_REPAIR_MINUTES: u32 = 10;

/// The two talents on offer at a pick level, left and right, or `None`
/// for a fixed level and for no level at all.
pub fn pick_at(level: u8) -> Option<(Talent, Talent)> {
    Some(match level {
        2 => (Talent::QuickHands, Talent::SiteForeman),
        4 => (Talent::Sandbagger, Talent::BulkBags),
        5 => (Talent::ArmouredSentry, Talent::DeepMagazine),
        6 => (Talent::Armourer, Talent::FieldRefit),
        8 => (Talent::DugIn, Talent::QuickBuild),
        9 => (Talent::Salvage, Talent::SteadyHands),
        10 => (Talent::SecondSentry, Talent::SentryMarkThree),
        _ => return None,
    })
}

/// The level `xp` makes, one to ten.
pub fn level_of(xp: u32) -> u8 {
    LEVEL_XP.iter().filter(|&&need| xp >= need).count().max(1) as u8
}

/// One crew member's way through its class: what it has learnt.
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

    /// Whether a talent has been picked.
    pub fn has(&self, talent: Talent) -> bool {
        self.picks
            .iter()
            .any(|&(level, side)| talent_of(level, side) == Some(talent))
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
    pub fn pending_pick(&self) -> Option<u8> {
        (2..=self.level()).find(|&l| pick_at(l).is_some() && self.picked_at(l).is_none())
    }

    /// Choose a side at a level: a pick level, reached, not yet picked.
    /// The talent it is, or why not.
    pub fn pick(&mut self, level: u8, side: Side) -> Result<Talent, Refusal> {
        let (left, right) = pick_at(level).ok_or(Refusal::NotAPickLevel)?;
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
}

/// The talent a side of a level is.
pub fn talent_of(level: u8, side: Side) -> Option<Talent> {
    pick_at(level).map(|(left, right)| match side {
        Side::Left => left,
        Side::Right => right,
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
        assert_eq!(p.pending_pick(), Some(2));
        assert_eq!(p.pick(3, Side::Left), Err(Refusal::NotAPickLevel));
        assert_eq!(p.pick(6, Side::Left), Err(Refusal::LevelNotReached));
        assert_eq!(p.pick(2, Side::Right), Ok(Talent::SiteForeman));
        assert_eq!(p.pick(2, Side::Left), Err(Refusal::AlreadyPicked));
        assert!(p.has(Talent::SiteForeman) && !p.has(Talent::QuickHands));
        assert_eq!(p.pending_pick(), Some(4));
        let mut all = Vec::new();
        for level in 1..=LEVELS {
            if let Some((l, r)) = pick_at(level) {
                all.push(l);
                all.push(r);
            }
        }
        assert_eq!(all, Talent::ALL.to_vec(), "every talent is on one pick level");
        for talent in Talent::ALL {
            assert_eq!(Talent::from_code(talent.code()), Some(talent));
        }
        for class in Class::ALL {
            assert_eq!(Class::from_code(class.code()), Some(class));
        }
    }
}
