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
    strips: 0.0,
    strips_far: 0.0,
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
    strips: 0.0,
    strips_far: 0.0,
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
    strips: 0.0,
    strips_far: 0.0,
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
    strips: 0.0,
    strips_far: 0.0,
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
    strips: 0.0,
    strips_far: 0.0,
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

// ---- The droids (feature 83) ----
//
// A droid is not a Bim: its arms are built in, and so is its body. The
// two weapons here are never made, never bought and never carried — they
// are part of the machine — and the four part healths below are the
// machine's, not `crate::health`'s. See `crate::droid`.

/// The Husk's claws: a snap within [`MELEE_RANGE`] every
/// [`MELEE_PERIOD`], and the wound is a crush rather than a cut, so it
/// does not bleed the way a blade's does.
pub const CLAW: WeaponStats = WeaponStats {
    range: MELEE_RANGE,
    sweet: MELEE_RANGE,
    accuracy: 1.0,
    accuracy_far: 1.0,
    damage: 20.0,
    damage_far: 20.0,
    speed: 0.0,
    fire_rate: 1.0 / MELEE_PERIOD,
    burst: 1,
    burst_gap: 0.0,
    melee: true,
    strips: 0.0,
    strips_far: 0.0,
};

/// The Warden's lance: it does little to a body and a great deal to what
/// the body is wearing. A bolt that reaches a part in unbroken armour
/// takes [`WeaponStats::strips`] off the **piece**, its protection
/// ignored, and the part itself takes nothing; a bare part takes the
/// plain damage. See `crate::combat::Combat::strip`.
pub const UNMAKER: WeaponStats = WeaponStats {
    range: 20.0,
    sweet: 10.0,
    accuracy: 0.8,
    accuracy_far: 0.55,
    damage: 6.0,
    damage_far: 4.0,
    speed: 25.0,
    fire_rate: 0.5,
    burst: 1,
    burst_gap: 0.0,
    melee: false,
    strips: 30.0,
    strips_far: 20.0,
};

/// What each of a droid's four parts has at tier one — head, chassis,
/// arms, legs, in `crate::droid::DroidPart::ALL` order. Every tier above
/// one multiplies all four by [`ARMOUR_TIER_STEP`], the way a piece of
/// armour's health climbs.
pub const HUSK_BODY: [f32; 4] = [8.0, 40.0, 12.0, 15.0];
pub const TROOPER_BODY: [f32; 4] = [10.0, 60.0, 15.0, 20.0];
pub const WARDEN_BODY: [f32; 4] = [20.0, 120.0, 25.0, 30.0];

/// The odds a hit lands on each of the four. They add to one.
pub const DROID_HIT_ODDS: [f32; 4] = [0.05, 0.60, 0.15, 0.20];

/// The Guardian's body (feature 100): the largest of them, with a chassis
/// nearly a Warden's and a head that is the lens, smaller and easier to
/// lose.
pub const GUARDIAN_BODY: [f32; 4] = [16.0, 110.0, 25.0, 30.0];

/// The Guardian's beam, the **Sweeper** (feature 100): it winds up for
/// [`SWEEPER_WINDUP`], sweeps [`SWEEPER_ARC_DEGREES`] across the aim in
/// [`SWEEPER_SWEEP`], and cools for [`SWEEPER_COOLDOWN`]. A beam rolls no
/// odds — whatever it crosses it reaches — so `accuracy` and
/// `accuracy_far` are **the tactics' alone**: they make its worth fall off
/// past `sweet` (`WeaponStats::dps_at`), which is what keeps a Guardian
/// walking in to the beam's sweet range rather than standing off at its
/// full reach. `speed` is a bolt's pace, for a beam that is nothing; it is
/// what a Sweeper would fly at were it fired as a bolt.
pub const SWEEPER: WeaponStats = WeaponStats {
    range: 20.0,
    sweet: 8.0,
    accuracy: 1.0,
    accuracy_far: 0.4,
    damage: 30.0,
    damage_far: 30.0,
    speed: 40.0,
    fire_rate: 1.0 / (SWEEPER_WINDUP + SWEEPER_SWEEP + SWEEPER_COOLDOWN),
    burst: 1,
    burst_gap: 0.0,
    melee: false,
    strips: 0.0,
    strips_far: 0.0,
};

/// The Sweeper's rhythm, in seconds: the lens brightening on a fixed aim,
/// the beam crossing its arc, and the wait before the next wind-up.
pub const SWEEPER_WINDUP: f32 = 1.2;
pub const SWEEPER_SWEEP: f32 = 0.5;
pub const SWEEPER_COOLDOWN: f32 = 3.5;
/// How far the beam turns in a sweep, from half of it one side of the aim
/// to half of it the other. Degrees, for the reader: the arithmetic is the
/// literal cosines and sines in `crate::droid`, never this number.
pub const SWEEPER_ARC_DEGREES: f32 = 20.0;

/// How fast a Guardian turns, in degrees a second. Like the arc, a number
/// for the reader: the turn is made of fixed sub-steps whose cosine and
/// sine are written out in `crate::droid`.
pub const GUARDIAN_TURN_DEGREES: f32 = 75.0;
/// The Guardian's shield: a bolt or a blow coming in within this cosine
/// of its heading — the ±60° front arc — is stopped. The edge itself is
/// stopped.
pub const GUARDIAN_SHIELD_COS: f32 = 0.5;
/// How far out from the Guardian's middle the shield's plate stands, in
/// room units: where a stopped bolt stops and where the plate is drawn.
pub const GUARDIAN_SHIELD_RADIUS: f32 = 34.0;
/// What a Guardian walks at, as a share of a Bim's marching pace.
pub const GUARDIAN_PACE: f32 = 0.7;

/// What a droid with its arms shot away fires and strikes at: a gun's
/// odds and a claw's damage, halved. The Unmaker counts as a gun; the
/// Guardian's Sweeper, which rolls no odds, loses the damage instead.
pub const DROID_ARMS_ACCURACY: f32 = 0.5;
pub const DROID_ARMS_DAMAGE: f32 = 0.5;

/// What each kind walks at, as a share of a Bim's marching pace.
pub const HUSK_PACE: f32 = 1.3;
pub const TROOPER_PACE: f32 = 1.0;
pub const WARDEN_PACE: f32 = 0.8;
