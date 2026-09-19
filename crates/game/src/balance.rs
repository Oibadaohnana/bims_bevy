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
/// The odds a bolt reaching a body peeking from cover is dodged.
pub const DODGE_IN_COVER: f32 = 0.5;

// ---- The weapons ----

/// The pistol everybody is issued: short range, quick, light.
pub const LASER_PISTOL: WeaponStats = WeaponStats {
    range: 12.0,
    sweet: 0.0,
    accuracy: 0.95,
    accuracy_far: 0.65,
    damage: 6.0,
    damage_far: 6.0,
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
    accuracy: 0.90,
    accuracy_far: 0.60,
    damage: 50.0,
    damage_far: 30.0,
    speed: 20.0,
    fire_rate: 0.25,
    burst: 1,
    burst_gap: 0.0,
    melee: false,
};

/// Eight light shots in two seconds, then two seconds' recharge.
pub const AUTO_RIFLE: WeaponStats = WeaponStats {
    range: 16.0,
    sweet: 8.0,
    accuracy: 0.85,
    accuracy_far: 0.50,
    damage: 5.0,
    damage_far: 4.0,
    speed: 22.0,
    fire_rate: 0.25,
    burst: 8,
    burst_gap: 0.25,
    melee: false,
};

/// Cannot miss at twenty tiles, can at thirty-five; one shot every four
/// seconds.
pub const SNIPER_RIFLE: WeaponStats = WeaponStats {
    range: 35.0,
    sweet: 20.0,
    accuracy: 1.0,
    accuracy_far: 0.70,
    damage: 45.0,
    damage_far: 25.0,
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
    damage: 35.0,
    damage_far: 35.0,
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
