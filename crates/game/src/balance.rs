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
//! minigun's `10.0` with `burst 1` is ten bolts a second, steadily, until
//! its `magazine` of a hundred is spent and `reload_time` runs out. A `melee` weapon swings within
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
//! A rail lance's slug goes through every body in its line
//! ([`LANCE_RECALL`]); nothing else does.
//!
//! What a body *has* — one bar of hit points — is `crate::health`.

use crate::combat::{ArmourStats, WeaponKind, WeaponStats};

// ---- The melee ----

/// How far a blade reaches, in tiles: the reach of a melee weapon, and
/// how close a melee enemy has to come to lock a gunner.
pub const MELEE_RANGE: f32 = 1.2;

/// The furthest anything shoots, in tiles, whatever lengthens it — a
/// tier, a skill, a relic, an item: the reach of the game view, whose
/// nearer edge is this far from its middle at its default and widest
/// zoom (`ship::game::Game::hold_view_to_the_ground`). October 2026, the
/// player's word: the game is balanced around that view, and nothing
/// fires from off it. Counted on the player's 2560×1440 screen, 14 tiles
/// up and 25 to the side; then the player zoomed in a wheel notch and
/// asked for that as the view (12.5 up, 22.2 across, tiles of 58 px
/// where they had been 51), and every gun's and machine's `range` and
/// `sweet` came down with it by 12.5/14 — fourteen before.
pub const MAX_RANGE: f32 = 12.5;
/// The tiles a bolt has flown within which it is **near** to the crew's
/// relics (*Point Blank*, *Marksman's Creed*, October 2026): their near
/// factor whole.
pub const NEAR_TILES: f32 = 4.0;
/// The tiles past which it is **far**: their far factor whole, a straight
/// line between the two from [`NEAR_TILES`].
pub const FAR_TILES: f32 = 7.0;

/// A factor `near` within [`NEAR_TILES`], `far` past [`FAR_TILES`] and a
/// straight line between, at `flown` tiles.
pub fn near_far(flown: f32, near: f32, far: f32) -> f32 {
    if near == far {
        return near;
    }
    let t = ((flown - NEAR_TILES) / (FAR_TILES - NEAR_TILES)).clamp(0.0, 1.0);
    near + (far - near) * t
}
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
//
// Later still, the view a wheel notch nearer ([`MAX_RANGE`] 12.5 from
// fourteen): every `range` and `sweet` here but the blades' times 12.5/14,
// the shotgun's too. The notes give the numbers before it.
//
// Later in October 2026 the four guns got **magazines**: so many shots,
// then `reload_time` seconds when nothing is fired, and the next magazine
// full — for ever, since nothing in the game carries ammunition
// (`combat::Trigger`). Every body that carries one reloads: a player's,
// a bot, a station's people, a Trooper's arm, and since the minigun got
// one a sentry too. The guns were paid for it: the pistol and the auto rifle 2 a shot more,
// near and far, the shotgun twice the trigger rate, the sniper nothing.
// The rail lance's one slug in five seconds is a reload already, and has
// no magazine (nought); nor has a blade or a machine's built-in arm. The
// minigun's burst and long cool were one too, until it got a magazine of
// its own (`MINIGUN_MAGAZINE`), and the sentries reload with it.
//
// And then **no damage drop over distance**: every weapon's `damage_far`
// (and the Unmaker's `strips_far`) is its near number, so a hit does the
// same out to the end of the range. The odds still fall off. The far
// numbers before: the shotgun 36, the auto rifle 6.4, the sniper 30, the
// minigun 3.2, the rail lance 32, the Unmaker 4 (and 20 stripped).

/// The pistol's magazine and its reload, in seconds.
pub const PISTOL_MAGAZINE: u32 = 12;
pub const PISTOL_RELOAD: f32 = 1.2;
/// The shotgun's: six shells, put in one by one.
pub const SHOTGUN_MAGAZINE: u32 = 6;
pub const SHOTGUN_RELOAD: f32 = 3.5;
/// The auto rifle's: seven and a half seconds of fire.
pub const AUTO_RIFLE_MAGAZINE: u32 = 30;
pub const AUTO_RIFLE_RELOAD: f32 = 1.8;
/// The sniper rifle's.
pub const SNIPER_MAGAZINE: u32 = 4;
pub const SNIPER_RELOAD: f32 = 2.4;
/// The minigun's (October 2026, the player's word): a hundred bolts at
/// ten a second, then four seconds. Its burst and five-second cool went
/// with it, and the sentry that fires it reloads too.
pub const MINIGUN_MAGAZINE: u32 = 100;
pub const MINIGUN_RELOAD: f32 = 4.0;

/// The pistol everybody is issued: quick, light, ten tiles (11.2 at
/// a fourteen-tile view, 15.4 before [`MAX_RANGE`], twenty-two before October 2026). In a player's hand it fires
/// a shot every click, and held one every [`SEMI_AUTO_COOLDOWN`]
/// (`WeaponKind::semi_automatic`), so its `fire_rate` is only the pace
/// of a body nobody steers. 6 a shot since October 2026, when every
/// click became a shot (7.2 before), and 8 since its magazine of
/// [`PISTOL_MAGAZINE`].
/// A click is held back until [`SEMI_AUTO_COOLDOWN`] has passed since the
/// last shot.
pub const LASER_PISTOL: WeaponStats = WeaponStats {
    range: 10.0,
    sweet: 0.0,
    accuracy: 0.855,
    accuracy_far: 0.585,
    damage: 8.0,
    damage_far: 8.0,
    speed: 18.0,
    fire_rate: 1.5,
    burst: 1,
    burst_gap: 0.0,
    melee: false,
    strips: 0.0,
    strips_far: 0.0,
    magazine: PISTOL_MAGAZINE,
    reload_time: PISTOL_RELOAD,
};

/// Seconds between two shots of a semi-automatic (the pistol) in a
/// player's hand: a click sooner is fired the moment it has passed, and
/// a fire-rate skill or relic shortens it (`Skill::fire_rate`).
pub const SEMI_AUTO_COOLDOWN: f32 = 0.3;

/// Sixty a hit out to 8.9 tiles, surer inside 3.6 (ten and four before
/// the view came a notch nearer). One
/// pull every two seconds since its magazine of [`SHOTGUN_MAGAZINE`]
/// (every four before).
pub const SHOTGUN: WeaponStats = WeaponStats {
    range: 8.9,
    sweet: 3.6,
    accuracy: 0.81,
    accuracy_far: 0.54,
    damage: 60.0,
    damage_far: 60.0,
    speed: 20.0,
    fire_rate: 0.5,
    burst: 1,
    burst_gap: 0.0,
    melee: false,
    strips: 0.0,
    strips_far: 0.0,
    magazine: SHOTGUN_MAGAZINE,
    reload_time: SHOTGUN_RELOAD,
};

/// Four light shots a second for as long as the trigger is held, no
/// burst and no recharge; full out to 3.75 tiles, reaching 11.25 (4.2 and
/// 12.6 at a fourteen-tile view, 5.6 and 18.2 before [`MAX_RANGE`], eight and twenty-six before October
/// 2026's cut).
/// It fired eight-shot bursts of 6 (4.8 far) every four seconds until
/// October 2026; the bursts went and each shot lost 3, the far one in
/// proportion, so a second's damage was the twelve it had been. Later in
/// October 2026 each shot gained 2, near and far (3 and 2.4 before):
/// twenty a second in its sweet range. And 2 more again with its
/// magazine of [`AUTO_RIFLE_MAGAZINE`] (5 and 4.4 before): twenty-eight.
pub const AUTO_RIFLE: WeaponStats = WeaponStats {
    range: 11.25,
    sweet: 3.75,
    accuracy: 0.765,
    accuracy_far: 0.45,
    damage: 7.0,
    damage_far: 7.0,
    speed: 22.0,
    fire_rate: 4.0,
    burst: 1,
    burst_gap: 0.0,
    melee: false,
    strips: 0.0,
    strips_far: 0.0,
    magazine: AUTO_RIFLE_MAGAZINE,
    reload_time: AUTO_RIFLE_RELOAD,
};

/// Nine in ten at 8.75 tiles, fewer at 12.5 — [`MAX_RANGE`], the
/// edge of the view (9.8 and fourteen at a fourteen-tile view,
/// fourteen and 24.5 before it, twenty and thirty-five
/// before October 2026); one shot every four seconds.
pub const SNIPER_RIFLE: WeaponStats = WeaponStats {
    range: MAX_RANGE,
    sweet: 8.75,
    accuracy: 0.9,
    accuracy_far: 0.63,
    damage: 54.0,
    damage_far: 54.0,
    speed: 60.0,
    fire_rate: 0.25,
    burst: 1,
    burst_gap: 0.0,
    melee: false,
    strips: 0.0,
    strips_far: 0.0,
    magazine: SNIPER_MAGAZINE,
    reload_time: SNIPER_RELOAD,
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
    magazine: 0,
    reload_time: 0.0,
};

// ---- The minigun and the rail lance (task 115) ----
//
// Two kinds made only from a tier up (`WeaponKind::min_tier`): the
// minigun from tier two, the rail lance at tier three alone. Their base
// numbers are what the tier factors below are multiplied onto, chosen so
// each lands where it was asked for **at its own lowest tier** — which is
// the only tier nobody can combine one up to.

/// Ten bolts a second for as long as the trigger is held, a hundred to a
/// magazine ([`MINIGUN_MAGAZINE`]) — ten seconds of fire — then four
/// seconds to reload ([`MINIGUN_RELOAD`]). No spin-up, no heat, no pace
/// penalty. At tier two (its lowest) that is 5.5 a bolt at 0.85 odds out
/// to its sweet 3.75 tiles, reaching ten (4.2 and 11.2 at a
/// fourteen-tile view, fourteen before [`MAX_RANGE`],
/// six and twenty before October 2026). Until its magazine (October 2026, the player's word:
/// the damage a bolt kept, the trader's price tripled) a pull was twenty
/// bolts a tenth apart and the rest of a five-second cycle to cool.
///
/// What it does a second in its sweet range, body hits on whole armour,
/// against the tier-two auto rifle beside it — over its magazine and
/// reload, a hundred bolts in fourteen seconds:
///
/// | against | tier-2 minigun | tier-2 auto rifle |
/// | --- | --- | --- |
/// | a droid (no armour) | 33.4 | 27.0 |
/// | tier-2 kevlar (protection 3) | 15.2 | 17.7 |
/// | tier-3 kevlar (protection 4.5) | 6.1 | 13.1 |
///
/// So it shreds the machines and bounces off good armour: many light
/// bolts each lose the protection. (The minigun's column was 18.7, 8.5
/// and 3.4 at twenty bolts in five seconds. The auto rifle's was 14.3,
/// 2.9 and 0.0 until its shots gained 2 in October 2026, and 23.9, 12.4
/// and 6.7 until they gained 2 again with its magazine, the rifle's
/// second now over thirty shots and their reload.)
pub const MINIGUN: WeaponStats = WeaponStats {
    range: 10.0,
    sweet: 3.75,
    accuracy: 0.68,
    accuracy_far: 0.36,
    damage: 4.4,
    damage_far: 4.4,
    speed: 24.0,
    fire_rate: 10.0,
    burst: 1,
    burst_gap: 0.0,
    melee: false,
    strips: 0.0,
    strips_far: 0.0,
    magazine: MINIGUN_MAGAZINE,
    reload_time: MINIGUN_RELOAD,
};

/// One slug every five seconds that **goes through**: it strikes every
/// body along its line, each at the whole damage at the distance flown
/// (up to three before October 2026, the n-th at 0.6ⁿ of it). At tier
/// three (its only tier) that is 75 a slug at 0.945 odds out to 8.78
/// tiles, reaching [`MAX_RANGE`] (9.84 and fourteen at a fourteen-tile
/// view, 16.8 and 28.56 before it, 24 and 40.8
/// before October 2026).
///
/// Against one target the tier-three sniper rifle does about 18 a second
/// (84.4 at certain odds, one every four, four to a magazine and 2.4 s to
/// reload it; 21 before the magazines) and the lance about 14 (75 at
/// 0.945, one every five); into three bodies in a line the lance does
/// about 42 (14 on each), and 14 more for every body more. A wall, a
/// lamp and a Guardian's shield from the front stop it; a tank's
/// *interpose* spends it. See `crate::combat::Combat::step`.
pub const RAIL_LANCE: WeaponStats = WeaponStats {
    range: 10.45,
    sweet: 7.32,
    accuracy: 0.72,
    accuracy_far: 0.52,
    damage: 48.0,
    damage_far: 48.0,
    speed: 70.0,
    fire_rate: 0.2,
    burst: 1,
    burst_gap: 0.0,
    melee: false,
    strips: 0.0,
    strips_far: 0.0,
    magazine: 0,
    reload_time: 0.0,
};

/// How many of the bodies it went through last a rail lance slug
/// remembers, so it never strikes one twice while still crossing it. It
/// strikes every body in its line however many there are (it stopped at
/// three before October 2026).
pub const LANCE_RECALL: usize = 3;

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
    /// The Lancer's rail (task 157). Left out of a file written before
    /// it, it is the constant.
    pub rail: (f32, f32),
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
        rail: (RAIL.damage, RAIL.damage_far),
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
            WeaponKind::Rail => self.rail,
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

/// **Armour regenerates** (October 2026): once its wearer has taken no
/// hit for this many seconds, a worn piece puts its own health back at
/// [`ARMOUR_REGEN`] — the piece's bar, never the body's — and a piece run
/// down to nothing comes back the same way rather than staying broken for
/// the mission.
pub const ARMOUR_REGEN_DELAY: f32 = 8.0;
/// What a piece puts back a second once [`ARMOUR_REGEN_DELAY`] has run,
/// in its own health points, whatever its tier.
pub const ARMOUR_REGEN: f32 = 5.0;

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
    magazine: 0,
    reload_time: 0.0,
};

/// The Warden's lance: it does little to a body and a great deal to what
/// the body is wearing. A bolt that reaches a part in unbroken armour
/// takes [`WeaponStats::strips`] off the **piece**, its protection
/// ignored, and the part itself takes nothing; a bare part takes the
/// plain damage. See `crate::combat::Combat::strip`.
pub const UNMAKER: WeaponStats = WeaponStats {
    range: MAX_RANGE,
    sweet: 6.25,
    accuracy: 0.8,
    accuracy_far: 0.55,
    damage: 6.0,
    damage_far: 6.0,
    speed: 25.0,
    fire_rate: 0.5,
    burst: 1,
    burst_gap: 0.0,
    melee: false,
    strips: 30.0,
    strips_far: 30.0,
    magazine: 0,
    reload_time: 0.0,
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
    range: MAX_RANGE,
    sweet: 5.0,
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
    magazine: 0,
    reload_time: 0.0,
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
/// was made half again as quick, and 144 until later that month, when
/// they were made half again as quick once more (the player's word:
/// "increase movement speed of all by 50%").
pub const MARCH_SPEED: f32 = 216.0;

/// A player's own Bim sprinting under Shift (task 150): its walk times
/// this, its weapon held across the chest and silent.
pub const SPRINT: f32 = 1.8;
/// How long a sprint takes to come up from the walk to its whole pace, in
/// seconds (October 2026, the player's word: "ease into sprints with
/// 100ms"; it was at once). Letting go of Shift is the walk at once.
pub const SPRINT_EASE: f32 = 0.1;
/// A dodge roll under Alt (task 150): how long it lasts, in seconds, how
/// far it carries the body, in room units (three and three quarter
/// tiles), and the seconds from one roll's start before the next may
/// start. Bolts and a beam reaching the body while it rolls are dodged; a
/// blow is not. The distance was 130 until October 2026, when the roll
/// went half again as fast with the walk (in the same time, so the dodge
/// lasts as long).
pub const ROLL_TIME: f32 = 0.38;
pub const ROLL_DISTANCE: f32 = 195.0;
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

// ---- The tier-two machines (task 157) ----
//
// Three more machines, from the floor's tier-two rows on: the **Bomber**
// and the **Lancer** on top of a wave (the world's `WaveScaling`), and
// the **Conductor** in an elite's Guardian wave. Each asks the crew for
// something the first four do not: to leave a circle, to leave a line,
// and to go through the wave for the one at its back.

/// The Bomber's body: a Trooper's and a little more, a squat thing with a
/// rack of bombs on its back.
pub const BOMBER_BODY: [f32; 4] = [10.0, 70.0, 15.0, 22.0];
/// What it walks at.
pub const BOMBER_PACE: f32 = 0.9;
/// Its **bomb**: rolled along the deck at the nearest body of the crew's
/// side it sees within [`BOMB_TRIGGER`] tiles, at most [`BOMB_REACH`]
/// tiles out — it stops short at a wall, a shut door or low cover in its
/// way — and burst [`BOMB_FUSE`] seconds after the throw, the first
/// [`BOMB_ROLL`] of them rolling and the rest spinning up where it
/// stopped, [`BOMB_RADIUS`] tiles wide. Where it will stop is known the
/// moment it leaves, and that circle is drawn from then. It bursts on
/// everybody in it, **the machines too** (`Game::burst`), with
/// [`BOMB_DAMAGE`] at the centre times the tier's damage factor, half
/// at the edge, halved again in cover. One every [`BOMB_COOLDOWN`]
/// seconds, the first [`BOMB_FIRST`] after it has a mark.
pub const BOMB_TRIGGER: f32 = 7.0;
pub const BOMB_REACH: f32 = 5.0;
pub const BOMB_FUSE: f32 = 1.5;
pub const BOMB_ROLL: f32 = 0.6;
pub const BOMB_RADIUS: f32 = 2.25;
pub const BOMB_DAMAGE: f32 = 40.0;
pub const BOMB_COOLDOWN: f32 = 6.0;
pub const BOMB_FIRST: f32 = 1.5;
/// Where a Bomber stands: it walks in to about [`BOMBER_STAND`] tiles of
/// its mark — its pistol's reach cut to that for the stand it picks — and
/// backs off a body of the crew's side that comes within [`BOMBER_SHY`].
pub const BOMBER_STAND: f32 = 6.0;
pub const BOMBER_SHY: f32 = 3.0;

/// The Lancer's body: fragile, the lightest of them that shoots.
pub const LANCER_BODY: [f32; 4] = [8.0, 45.0, 12.0, 15.0];
/// What it walks at.
pub const LANCER_PACE: f32 = 0.9;
/// The Lancer's **rail**: one slug through every body on its line,
/// fired along a line it **charges** first — [`LANCER_TRACK`] seconds the
/// line following its mark, then [`LANCER_LOCK`] held where it fixed,
/// then the slug, flown straight along it (it rolls no odds: what stands
/// on the line is struck, what stepped off it is not). Then
/// [`LANCER_COOLDOWN`] before the next charge, or [`LANCER_CANCELLED`]
/// after a stun or its arms shot away put a charge out. Accuracy one at
/// every range for that reason; the odds a body in cover dodges it are a
/// bolt's.
/// 135 a slug before the tier's factor (45 before October 2026; tripled).
pub const RAIL: WeaponStats = WeaponStats {
    range: MAX_RANGE,
    sweet: 10.0,
    accuracy: 1.0,
    accuracy_far: 1.0,
    damage: 135.0,
    damage_far: 135.0,
    speed: 60.0,
    fire_rate: 1.0 / (LANCER_TRACK + LANCER_LOCK + LANCER_COOLDOWN),
    burst: 1,
    burst_gap: 0.0,
    melee: false,
    strips: 0.0,
    strips_far: 0.0,
    magazine: 0,
    reload_time: 0.0,
};
pub const LANCER_TRACK: f32 = 0.8;
pub const LANCER_LOCK: f32 = 0.4;
pub const LANCER_COOLDOWN: f32 = 3.0;
pub const LANCER_CANCELLED: f32 = 4.0;
/// How far ahead of its mark a Lancer begins a charge: within this many
/// tiles of where it stands, with a clear line from its own eye.
pub const LANCER_REACH: f32 = 12.0;
/// Where a Lancer stands: back off a body of the crew's side that comes
/// within [`LANCER_SHY`] tiles.
pub const LANCER_SHY: f32 = 6.0;

/// The Conductor's body (the tier-two elite): about two Wardens'.
pub const CONDUCTOR_BODY: [f32; 4] = [30.0, 260.0, 45.0, 55.0];
/// What it walks at.
pub const CONDUCTOR_PACE: f32 = 0.8;
/// Its **link**: every other machine within [`LINK_RADIUS`] tiles of a
/// Conductor standing takes [`LINK_TAKEN`] of what a hit would do to it.
pub const LINK_RADIUS: f32 = 6.0;
pub const LINK_TAKEN: f32 = 0.6;
/// Its **mark**: every [`MARK_EVERY`] seconds it puts a reticle on a body
/// of the crew's side it sees, [`MARK_WARNING`] seconds of warning, then
/// for [`MARK_HOLD`] every machine that can see that body fires at it and
/// at nobody else (a tank's taunt reaching one still comes first). The
/// first mark is [`MARK_FIRST`] after it first sees somebody.
pub const MARK_EVERY: f32 = 8.0;
pub const MARK_WARNING: f32 = 1.0;
pub const MARK_HOLD: f32 = 4.0;
pub const MARK_FIRST: f32 = 3.0;
/// Its **blink**: a body of the crew's side within [`BLINK_NEAR`] tiles
/// and it plants for [`BLINK_WINDUP`] seconds and is gone to a spot up to
/// [`BLINK_REACH`] tiles off, away from it, once every
/// [`BLINK_COOLDOWN`].
pub const BLINK_NEAR: f32 = 3.0;
pub const BLINK_WINDUP: f32 = 0.4;
pub const BLINK_REACH: f32 = 6.0;
pub const BLINK_COOLDOWN: f32 = 10.0;
/// Its **strike call**, once, the first time its health falls to
/// [`STRIKE_AT`] of the whole: [`STRIKE_BOMBS`] bombs' circles laid round
/// its mark (or the nearest it sees), [`STRIKE_SPREAD`] tiles out, each
/// bursting [`BOMB_FUSE`] after with a bomb's damage.
pub const STRIKE_AT: f32 = 0.5;
pub const STRIKE_BOMBS: u32 = 4;
pub const STRIKE_SPREAD: f32 = 1.75;
/// Where a Conductor stands: it takes cover like a Warden and backs off a
/// body of the crew's side that comes within [`CONDUCTOR_SHY`] tiles.
pub const CONDUCTOR_SHY: f32 = 6.0;
