//! The numbers of the fight, in one place, for tuning.
//!
//! Every weapon's stats, every piece of armour's, and the melee's
//! constants live here and nowhere else: `WeaponKind::stats` and
//! `ArmourKind::stats` in `crate::combat` are lookups into this file, and
//! the melee constants there and `SWING_TIME` in `crate::character` are
//! re-exports of these. Change a number here and every room — the crew's,
//! a station's, a probe's — fights by it; nothing has to be changed
//! anywhere else, and the tests that pin the curves
//! (`combat::tests::the_curves_pin_the_numbers_the_guns_were_asked_for`)
//! say what moved.
//!
//! # How to read a weapon
//!
//! Range and `sweet` are in **tiles**, speed in tiles a second. A weapon's
//! odds of a hit are `accuracy` out to `sweet` tiles and `accuracy_far`
//! at `range`, a straight line between; `damage` and `damage_far` the
//! same. A `burst` is that many shots to one trigger pull, `burst_gap`
//! seconds apart, and `fire_rate` is trigger pulls a second — the auto
//! rifle's `0.25` with `burst 8` is eight shots in two seconds, then two
//! seconds' recharge. A `melee` weapon swings within [`MELEE_RANGE`]
//! instead of firing, and its `fire_rate` is one over [`MELEE_PERIOD`].
//! Seconds are real seconds at 1x.
//!
//! # How to read a piece of armour
//!
//! `protection` comes off every hit's damage on that part before anything
//! else; what is left drains the piece's own `health`, and only what the
//! piece cannot take reaches the body. At nothing the piece is broken and
//! does nothing.
//!
//! What a body *has* — a head of 5, a body of 75, legs of 20, the odds a
//! hit lands on each, and what a cut bleeds — is `crate::health`
//! (`Part::max`, `Part::HIT_ODDS`, `CUT_WOUND`, `BLEED_PER_WOUND`).

use crate::combat::{ArmourStats, WeaponStats};

// ---- The melee ----

/// How far a blade reaches, in tiles: the reach of a melee weapon, and
/// how close a melee enemy has to come to lock a gunner.
pub const MELEE_RANGE: f32 = 1.2;
/// What a fist does, once a [`MELEE_PERIOD`], to whoever has a gunner
/// locked.
pub const FIST_DAMAGE: f32 = 20.0;
/// Seconds between blows in a melee, fist or blade.
pub const MELEE_PERIOD: f32 = 2.0;
/// How long a swing or a punch takes, in seconds. The blow lands when the
/// animation ends, so this is also how long a body has to step back out
/// of one.
pub const SWING_TIME: f32 = 0.5;
/// The odds a bolt reaching a body in cover is dodged: one peeking from
/// beside a wall, or one standing close behind sandbags.
pub const DODGE_IN_COVER: f32 = 0.5;
/// What a shooter's odds are multiplied by while it walks: half. A bot
/// therefore stands still to shoot unless the stand it wants is cover.
pub const WALKING_ACCURACY: f32 = 0.5;

// ---- The weapons ----
//
// The second tuning of September 2026: every gun's damage up a fifth and
// its odds down a tenth, the pistol's and the auto rifle's range up ten
// tiles. The blade keeps its odds — a swing lands by reach, not by a roll.

/// The pistol everybody is issued: quick, light, twenty-two tiles.
pub const LASER_PISTOL: WeaponStats = WeaponStats {
    range: 22.0,
    sweet: 0.0,
    accuracy: 0.855,
    accuracy_far: 0.585,
    damage: 7.2,
    damage_far: 7.2,
    speed: 18.0,
    fire_rate: 1.5,
    burst: 1,
    burst_gap: 0.0,
    melee: false,
};

/// Everything it has inside four tiles, a good deal less at ten.
pub const SHOTGUN: WeaponStats = WeaponStats {
    range: 10.0,
    sweet: 4.0,
    accuracy: 0.81,
    accuracy_far: 0.54,
    damage: 60.0,
    damage_far: 36.0,
    speed: 20.0,
    fire_rate: 0.25,
    burst: 1,
    burst_gap: 0.0,
    melee: false,
};

/// Eight light shots in two seconds, then two seconds' recharge; full
/// out to eight tiles, reaching twenty-six.
pub const AUTO_RIFLE: WeaponStats = WeaponStats {
    range: 26.0,
    sweet: 8.0,
    accuracy: 0.765,
    accuracy_far: 0.45,
    damage: 6.0,
    damage_far: 4.8,
    speed: 22.0,
    fire_rate: 0.25,
    burst: 8,
    burst_gap: 0.25,
    melee: false,
};

/// Nine in ten at twenty tiles, fewer at thirty-five; one shot every
/// four seconds.
pub const SNIPER_RIFLE: WeaponStats = WeaponStats {
    range: 35.0,
    sweet: 20.0,
    accuracy: 0.9,
    accuracy_far: 0.63,
    damage: 54.0,
    damage_far: 30.0,
    speed: 60.0,
    fire_rate: 0.25,
    burst: 1,
    burst_gap: 0.0,
    melee: false,
};

/// The blade: a swing every [`MELEE_PERIOD`] within [`MELEE_RANGE`], and
/// its wound is a cut that bleeds. Nothing flies, so no speed.
pub const SCHWORD: WeaponStats = WeaponStats {
    range: MELEE_RANGE,
    sweet: MELEE_RANGE,
    accuracy: 1.0,
    accuracy_far: 1.0,
    damage: 42.0,
    damage_far: 42.0,
    speed: 0.0,
    fire_rate: 1.0 / MELEE_PERIOD,
    burst: 1,
    burst_gap: 0.0,
    melee: true,
};

// ---- The armour ----

/// A cap over the head.
pub const BASIC_HELM: ArmourStats = ArmourStats {
    health: 15.0,
    protection: 2.0,
};

/// A plate over the torso.
pub const BASIC_KEVLAR: ArmourStats = ArmourStats {
    health: 20.0,
    protection: 2.0,
};

/// Guards on the shins.
pub const BASIC_LEGS: ArmourStats = ArmourStats {
    health: 10.0,
    protection: 1.0,
};

// ---- The tiers ----
//
// Every weapon and every piece of armour is one of three tiers
// (`crate::combat::Tier`), and the tier scales the kind's numbers above:
// tier one is the baseline, and the factors here are what two and three
// multiply in, three on top of two. See `Tier::weapon_factors` and
// `Tier::armour_factor`.

/// Tier two: damage and accuracy up by a quarter.
pub const TIER_TWO_DAMAGE: f32 = 1.25;
pub const TIER_TWO_ACCURACY: f32 = 1.25;
/// Tier three, on top of tier two: another quarter of damage, a twentieth
/// of accuracy, and a fifth more range.
pub const TIER_THREE_DAMAGE: f32 = 1.25;
pub const TIER_THREE_ACCURACY: f32 = 1.05;
pub const TIER_THREE_RANGE: f32 = 1.2;
/// Each tier of armour has half again the health and the protection of
/// the one below.
pub const ARMOUR_TIER_STEP: f32 = 1.5;
/// The odds a bolt reaching a body is dodged for each whole tier-three
/// piece it wears, combined across the pieces (`Gear::dodge`).
pub const TIER_THREE_DODGE: f32 = 0.10;
