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
//! seconds apart, and `fire_rate` is trigger pulls a second — the
//! minigun's `0.2` with `burst 20` is twenty bolts in two seconds, then
//! three seconds' cooling; the auto rifle's `4.0` with `burst 1` is four
//! shots a second, steadily. A `melee` weapon swings within
//! [`MELEE_RANGE`] instead of firing, and its `fire_rate` is one over
//! [`MELEE_PERIOD`].
//! Seconds are real seconds at 1x.
//!
//! # How to read a piece of armour
//!
//! There is one piece, [`ARMOUR`], over the whole body. `protection`
//! comes off every hit's damage before anything else; what is left
//! drains the piece's own `health`, and only what the piece cannot take
//! reaches the body. At nothing the piece is broken and does nothing.
//!
//! A rail lance's slug goes through bodies ([`LANCE_PIERCE`],
//! [`LANCE_FALLOFF`]); nothing else does.
//!
//! What a body *has* — one bar of hit points — is `crate::health`.

use crate::combat::{ArmourStats, WeaponKind, WeaponStats};

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
/// How far a shot its player aims (task 144) may stray from where the
/// body faces, in radians, at no odds at all: the miss is this times
/// what the odds fall short of one, either side — a pistol's 0.855
/// strays two degrees standing, eight on the move at half the odds.
/// A player's own Bim shoots at full odds (`Skill::sure`), so its aimed
/// shot never strays; this is for any other body steered.
pub const AIM_SPREAD: f32 = 0.25;

// ---- The weapons ----
//
// The second tuning of September 2026: every gun's damage up a fifth and
// its odds down a tenth, the pistol's and the auto rifle's range up ten
// tiles. The blade keeps its odds — a swing lands by reach, not by a roll.
//
// October 2026: every weapon's `range` and `sweet` cut by three tenths —
// the guns here, the minigun and the rail lance, and the machines'
// Unmaker and Sweeper — all but the shotgun and the blades (the Schword,
// the Husk's claws). The notes give the numbers they had before.

/// The pistol everybody is issued: quick, light, fifteen and a half
/// tiles (twenty-two before October 2026). In a player's hand it fires
/// a shot every click, and held one every [`SEMI_AUTO_COOLDOWN`]
/// (`WeaponKind::semi_automatic`), so its `fire_rate` is only the pace
/// of a body nobody steers. 6 a shot since October 2026, when every
/// click became a shot (7.2 before).
/// A click is held back until [`SEMI_AUTO_COOLDOWN`] has passed since the
/// last shot.
pub const LASER_PISTOL: WeaponStats = WeaponStats {
    range: 15.4,
    sweet: 0.0,
    accuracy: 0.855,
    accuracy_far: 0.585,
    damage: 6.0,
    damage_far: 6.0,
    speed: 18.0,
    fire_rate: 1.5,
    burst: 1,
    burst_gap: 0.0,
    melee: false,
    strips: 0.0,
    strips_far: 0.0,
};

/// Seconds between two shots of a semi-automatic (the pistol) in a
/// player's hand: a click sooner is fired the moment it has passed, and
/// a fire-rate skill or relic shortens it (`Skill::fire_rate`).
pub const SEMI_AUTO_COOLDOWN: f32 = 0.3;

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

/// Four light shots a second for as long as the trigger is held, no
/// burst and no recharge; full out to 5.6 tiles, reaching 18.2 (eight
/// and twenty-six before October 2026's cut).
/// It fired eight-shot bursts of 6 (4.8 far) every four seconds until
/// October 2026; the bursts went and each shot lost 3, the far one in
/// proportion, so a second's damage was the twelve it had been. Later in
/// October 2026 each shot gained 2, near and far (3 and 2.4 before):
/// twenty a second in its sweet range.
pub const AUTO_RIFLE: WeaponStats = WeaponStats {
    range: 18.2,
    sweet: 5.6,
    accuracy: 0.765,
    accuracy_far: 0.45,
    damage: 5.0,
    damage_far: 4.4,
    speed: 22.0,
    fire_rate: 4.0,
    burst: 1,
    burst_gap: 0.0,
    melee: false,
    strips: 0.0,
    strips_far: 0.0,
};

/// Nine in ten at fourteen tiles, fewer at 24.5 (twenty and thirty-five
/// before October 2026); one shot every four seconds.
pub const SNIPER_RIFLE: WeaponStats = WeaponStats {
    range: 24.5,
    sweet: 14.0,
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

// ---- The minigun and the rail lance (task 115) ----
//
// Two kinds made only from a tier up (`WeaponKind::min_tier`): the
// minigun from tier two, the rail lance at tier three alone. Their base
// numbers are what the tier factors below are multiplied onto, chosen so
// each lands where it was asked for **at its own lowest tier** — which is
// the only tier nobody can combine one up to.

/// Twenty bolts to a trigger pull, a tenth of a second apart — 1.9
/// seconds of fire — and the rest of a five-second cycle to cool. No
/// spin-up, no heat, no pace penalty: the burst, the trigger and the hit
/// are every gun's. At tier two (its lowest) that is 5.5 a bolt at 0.85
/// odds out to its sweet 4.2 tiles, reaching fourteen (six and twenty
/// before October 2026).
///
/// What it does a second in its sweet range, body hits on whole armour,
/// against the tier-two auto rifle beside it:
///
/// | against | tier-2 minigun | tier-2 auto rifle |
/// | --- | --- | --- |
/// | a droid (no armour) | 18.7 | 23.9 |
/// | tier-2 kevlar (protection 3) | 8.5 | 12.4 |
/// | tier-3 kevlar (protection 4.5) | 3.4 | 6.7 |
///
/// So it shreds the machines and bounces off good armour: many light
/// bolts each lose the protection. (The auto rifle's column was 14.3,
/// 2.9 and 0.0 until its shots gained 2 in October 2026.)
pub const MINIGUN: WeaponStats = WeaponStats {
    range: 14.0,
    sweet: 4.2,
    accuracy: 0.68,
    accuracy_far: 0.36,
    damage: 4.4,
    damage_far: 3.2,
    speed: 24.0,
    fire_rate: 0.2,
    burst: 20,
    burst_gap: 0.1,
    melee: false,
    strips: 0.0,
    strips_far: 0.0,
};

/// One slug every five seconds that **goes through**: it strikes up to
/// [`LANCE_PIERCE`] bodies along its line, the n-th (from nought) at the
/// damage at the distance flown times [`LANCE_FALLOFF`] to the n. At tier
/// three (its only tier) that is 75 a slug at 0.945 odds out to 16.8
/// tiles, reaching 28.56 (24 and 40.8 before October 2026).
///
/// Against one target the tier-three sniper rifle does about 21 a second
/// (84.4 at certain odds, one every four) and the lance about 14 (75 at
/// 0.945, one every five); into three bodies in a line the lance does
/// about 28 (75 + 45 + 27). A wall, a lamp and a Guardian's shield from
/// the front stop it; a tank's *interpose* spends it. See
/// `crate::combat::Combat::step`.
pub const RAIL_LANCE: WeaponStats = WeaponStats {
    range: 23.8,
    sweet: 14.0,
    accuracy: 0.72,
    accuracy_far: 0.52,
    damage: 48.0,
    damage_far: 32.0,
    speed: 70.0,
    fire_rate: 0.2,
    burst: 1,
    burst_gap: 0.0,
    melee: false,
    strips: 0.0,
    strips_far: 0.0,
};

/// How many bodies one rail lance slug strikes, at most.
pub const LANCE_PIERCE: usize = 3;
/// What each body struck after the first multiplies the slug's damage by:
/// the n-th body (from nought) takes `LANCE_FALLOFF`ⁿ of it — 1, 0.6,
/// 0.36.
pub const LANCE_FALLOFF: f32 = 0.6;

// ---- The damage as dials (October 2026) ----

/// Every weapon's damage, near (within `sweet`) and far (at `range`), as
/// dials the app tunes while the game runs — `weapons.ron` at the root,
/// beside `rewards.ron` (`BIMS_WEAPONS` names another). What the dials
/// say stands over the kind's `damage` and `damage_far` above in
/// [`WeaponKind::stats`], so a tier, a talent and a broken arm scale the
/// tuned number the way they scaled the constant.
///
/// The default is the constants, so a room never told fights as they
/// say. The dials are one table for the whole process
/// ([`WeaponDamage::arm`]), **neither saved nor hashed**: what they
/// decide — the wounds — is. In a two-player run each game reads its own
/// file, and the two have to agree.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(default)
)]
pub struct WeaponDamage {
    /// The crew's weapons, each `(near, far)`.
    pub laser_pistol: (f32, f32),
    pub shotgun: (f32, f32),
    pub auto_rifle: (f32, f32),
    pub sniper_rifle: (f32, f32),
    pub schword: (f32, f32),
    pub minigun: (f32, f32),
    pub rail_lance: (f32, f32),
    /// The machines' built-in arms.
    pub claw: (f32, f32),
    pub unmaker: (f32, f32),
    pub sweeper: (f32, f32),
}

impl WeaponDamage {
    /// The constants: the fight as it plays untuned.
    pub const DEFAULT: WeaponDamage = WeaponDamage {
        laser_pistol: (LASER_PISTOL.damage, LASER_PISTOL.damage_far),
        shotgun: (SHOTGUN.damage, SHOTGUN.damage_far),
        auto_rifle: (AUTO_RIFLE.damage, AUTO_RIFLE.damage_far),
        sniper_rifle: (SNIPER_RIFLE.damage, SNIPER_RIFLE.damage_far),
        schword: (SCHWORD.damage, SCHWORD.damage_far),
        minigun: (MINIGUN.damage, MINIGUN.damage_far),
        rail_lance: (RAIL_LANCE.damage, RAIL_LANCE.damage_far),
        claw: (CLAW.damage, CLAW.damage_far),
        unmaker: (UNMAKER.damage, UNMAKER.damage_far),
        sweeper: (SWEEPER.damage, SWEEPER.damage_far),
    };

    /// A kind's `(near, far)`.
    pub fn of(&self, kind: WeaponKind) -> (f32, f32) {
        match kind {
            WeaponKind::LaserPistol => self.laser_pistol,
            WeaponKind::Shotgun => self.shotgun,
            WeaponKind::AutoRifle => self.auto_rifle,
            WeaponKind::SniperRifle => self.sniper_rifle,
            WeaponKind::Schword => self.schword,
            WeaponKind::Minigun => self.minigun,
            WeaponKind::RailLance => self.rail_lance,
            WeaponKind::Claw => self.claw,
            WeaponKind::Unmaker => self.unmaker,
            WeaponKind::Sweeper => self.sweeper,
        }
    }

    /// The dials every room fights by now.
    pub fn armed() -> WeaponDamage {
        *ARMED.read().unwrap_or_else(|e| e.into_inner())
    }

    /// Make these the dials every room fights by, from the next shot.
    pub fn arm(self) {
        *ARMED.write().unwrap_or_else(|e| e.into_inner()) = self;
    }
}

impl Default for WeaponDamage {
    fn default() -> WeaponDamage {
        WeaponDamage::DEFAULT
    }
}

/// What [`WeaponDamage::armed`] hands out: the constants until the app
/// arms others.
static ARMED: std::sync::RwLock<WeaponDamage> = std::sync::RwLock::new(WeaponDamage::DEFAULT);

// ---- The armour ----

/// **The armour**: one piece over the whole body, the only thing a Bim
/// wears. Its tier-one numbers are the three pieces it replaced put
/// together (October 2026): the health is the helm's 15, the kevlar's 20
/// and the leg guards' 10 summed — what a fully armoured body's blue bar
/// was — and the protection is the three pieces' 2, 2 and 1 weighed by
/// the odds a hit landed on each (a twentieth, three quarters, a fifth),
/// so a hit is stopped by as much as it was on average.
pub const ARMOUR: ArmourStats = ArmourStats {
    health: 45.0,
    protection: 1.8,
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
/// The odds a bolt reaching a body in whole tier-three armour is dodged
/// (`Gear::dodge`): what the three tier-three pieces it replaced did
/// together, `1 − 0.9³`.
pub const TIER_THREE_DODGE: f32 = 0.271;

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
    range: 14.0,
    sweet: 7.0,
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
    range: 14.0,
    sweet: 5.6,
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
/// How much the Guardian's plate stops before it breaks: every bolt and
/// blow it stops takes its damage (at the distance flown, unarmoured)
/// off this, and at nothing the plate is gone for good and the machine
/// stands open from every side.
pub const GUARDIAN_SHIELD_HP: f32 = 1000.0;
/// What a Guardian walks at, as a share of a Bim's marching pace.
pub const GUARDIAN_PACE: f32 = 0.7;

/// The Guardian's **grenade** (October 2026): a body of the crew's side
/// coming within [`GUARDIAN_GRENADE_TRIGGER`] tiles of its middle, with a
/// clear line, and it drops one at its feet — once every
/// [`GUARDIAN_GRENADE_COOLDOWN`] seconds — lying there
/// [`GUARDIAN_GRENADE_FUSE`] seconds before it bursts
/// [`GUARDIAN_GRENADE_RADIUS`] tiles wide on the crew's side alone, the
/// machines untouched. What it does at the centre is the Guardian's
/// Sweeper damage (its tier and arms counted) times
/// [`GUARDIAN_GRENADE_DAMAGE`], half that at the edge and halved again in
/// cover, as a soldier's grenade.
pub const GUARDIAN_GRENADE_TRIGGER: f32 = 2.0;
pub const GUARDIAN_GRENADE_COOLDOWN: f32 = 10.0;
pub const GUARDIAN_GRENADE_FUSE: f32 = 1.5;
pub const GUARDIAN_GRENADE_RADIUS: f32 = 2.5;
pub const GUARDIAN_GRENADE_DAMAGE: f32 = 1.5;

/// What a droid with its arms shot away fires and strikes at: a gun's
/// odds and a claw's damage, halved. The Unmaker counts as a gun; the
/// Guardian's Sweeper, which rolls no odds, loses the damage instead.
pub const DROID_ARMS_ACCURACY: f32 = 0.5;
pub const DROID_ARMS_DAMAGE: f32 = 0.5;

/// What a body marches at, in room units a second: every Bim — a player's,
/// a bot, a station's people, a Manufacturer — and, times its kind's pace
/// below, every machine. It was 96 until October 2026, when everybody
/// was made half again as quick.
pub const MARCH_SPEED: f32 = 144.0;

/// A player's own Bim sprinting under Shift (task 150): its walk times
/// this, its weapon held across the chest and silent.
pub const SPRINT: f32 = 1.8;
/// A dodge roll under Alt (task 150): how long it lasts, in seconds, how
/// far it carries the body, in room units (two and a half tiles), and the
/// seconds from one roll's start before the next may start. Bolts and a
/// beam reaching the body while it rolls are dodged; a blow is not.
pub const ROLL_TIME: f32 = 0.38;
pub const ROLL_DISTANCE: f32 = 130.0;
pub const ROLL_COOLDOWN: f32 = 1.1;

/// A Husk with its legs shot off drags itself along on its claws at this
/// share of its pace, and keeps coming: before it lay where it was and,
/// having nothing but a claw, did nothing at all — "husks sometimes just
/// stand still". The others, legless, stand and shoot.
pub const HUSK_CRAWL: f32 = 0.35;
/// What each kind walks at, as a share of a Bim's marching pace.
pub const HUSK_PACE: f32 = 1.3;
pub const TROOPER_PACE: f32 = 1.0;
pub const WARDEN_PACE: f32 = 0.8;
