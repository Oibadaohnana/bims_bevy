//! Arms, armour, and the shots in the air.
//!
//! Every Bim carries a [`Gear`]: an armour slot, a weapon slot with
//! the hand laser everybody is issued, and its charges. A Bim under orders
//! — recruited — is in **combat mode**: it draws the weapon and, whenever
//! an enemy is in range and in its own line of sight (`crate::sight`, the
//! peek round a wall included), fires. The shot is a [`Bolt`]: a thing
//! that flies, at the weapon's speed, until it reaches a body, a wall or
//! the end of its range. Nothing here decides who is an enemy — the world
//! hands the room the targets (`Game::set_hostiles`) and takes the hits
//! back (`Game::take_hits`), since the enemies live in a room of their own.
//!
//! # Shots and bolts: the fight is drawn in one room
//!
//! The crew and a station's people are in two rooms, and a bolt that flew
//! in both would be two bolts. So a **friendly** bolt hits the targets the
//! world named, and a **hostile** bolt — red — hits this room's own bodies;
//! and a room whose bodies are hostile (`Game::set_hostile_bodies`) fires
//! no bolts at all but records a [`Shot`], which the world carries into
//! the crew's room and fires there as a hostile bolt (`Game::enemy_fire`).
//! Every bolt therefore flies, lands and is drawn on the crew's deck, and
//! the wound is applied where the body is: a hit on a target goes out
//! through [`Combat::take_hits`] for the world to carry to the residents'
//! room, a hit on one of our own goes onto [`Combat::wounds_taken`] and
//! the game applies it itself. A [`Hit`] carries the **damage**, worked
//! out the instant the bolt reaches the body: a weapon's damage falls off
//! with the distance the bolt has flown ([`WeaponStats::damage_at`]), and
//! the bolt remembers where it was fired from for exactly that.
//!
//! # Melee: a lock is a distance
//!
//! The schword is a blade, and a blade is used within [`MELEE_RANGE`]. A
//! body with a melee weapon swings at any enemy that close; and a body
//! with a gun that has a *melee* enemy that close is **locked** — it
//! cannot fire, and brawls with its fists ([`FIST_DAMAGE`]) instead. Either
//! starts a blow every [`MELEE_PERIOD`] seconds — a [`Blow`], which
//! lands when the swing's animation ends (`crate::character::SWING_TIME`,
//! half a second on) and only if the target is still within reach then
//! — as [`Combat::brawl`], which is a [`Hit`] straight onto the hits in
//! the crew's room, or a melee [`Shot`] in a hostile one for the world to
//! deliver — no bolt, since a blow has nothing to fly. The lock is
//! nothing but the distance: walking out of reach ends it, and a swing
//! already begun then lands on nobody. A schword's wound is a **cut**, which bleeds
//! three times what a shot does (`crate::health`), and `Hit` and `Shot`
//! carry whether they are one. The world says which of the targets carry
//! a blade: each target comes with its weapon kind.
//!
//! # A peek exposes the peek
//!
//! A body aiming from a peek beside a wall (`crate::sight`) leans out to
//! do it, and is shot at where it leans: the world hands the enemies the
//! peek position while it peeks (`Game::exposed_at`). Being in cover, a
//! bolt reaching it is **dodged** half the time ([`DODGE_IN_COVER`]) and
//! flies on past. The same for an enemy peeking at the crew — the world
//! says which targets are (`Game::set_hostiles_peeking`).
//!
//! # The enemy's tactics
//!
//! An enemy knows where the crew are — the world tells its room, the way
//! it tells the crew's — and [`Tactics::stand`] is where it chooses to
//! stand, scored in **tiles of walking** so the trade-offs are numbers:
//! a spot in range of a crew member from which it can shoot, and for
//! choice one **against a wall** where only the peek round it sees the
//! target, so the target's own shot has a wall in the way — cover, worth
//! walking [`COVER_WORTH`] tiles for, which is what makes it take the
//! cover near it and not the cover across the station. On top of that
//! the weapon in its hand **plays to its strength** ([`Tactics::fit`]):
//! what it would do a second at that distance against what the target's
//! weapon would do back, so a sniper rifle hangs back at fourteen tiles
//! and more, where the pistol is running out, a shotgun closes to four, an auto rifle
//! stands just beyond a pistol's reach, and any gun keeps out of a
//! blade's; two pistols are a match and keeping away is what is left.
//! Less half a tile a tile of walking, and the spot it stands on already
//! gets a bonus so it does not dither. It **shoots when it can**: on the
//! move too, from its own eyes, without turning off its route — the crew
//! hold their fire while they walk, since a walk has the facing. A
//! schword's tactics are a **charge**: the free cell nearest the nearest
//! target, and cover be damned.
//!
//! No strings: a weapon is a code and its stats are numbers, and the app
//! names them (`WEAPON_NAMES`, `ARMOUR_NAMES` in `crates/app/src/names.rs`).
//!
//! # Armour is clothes, and a piece is a thing
//!
//! A piece of armour ([`Piece`]) goes over the whole body — there is one
//! slot for it — and stands between the body and every hit: its
//! **protection** comes off every hit's damage before anything else, and
//! what is left drains the piece's own **health** first — only what the
//! piece cannot take reaches the body and opens a wound (`Game::wound`).
//! At nothing the piece is **broken**: still worn, still drawn, doing
//! nothing — for the rest of the mission, since the world makes every piece
//! whole again at the next one (task 113). A piece keeps its damage
//! wherever it goes — the armory, another Bim — which is why it has an
//! `id` and is moved about as an [`Item`] rather than counted. The world
//! keeps the pieces nobody wears in the ship's armory; the room keeps the
//! one on a body (`Gear`'s armour slot), because the room's health reads
//! them.
//!
//! # Accuracy and damage are two points and a line
//!
//! A weapon's odds of a hit are `accuracy` out to `sweet` tiles and
//! `accuracy_far` at `range`, a straight line between, and the same for
//! `damage` and `damage_far` ([`WeaponStats::hit_chance`],
//! [`WeaponStats::damage_at`]): a shotgun does everything it can at four
//! tiles and a good deal less at ten, a sniper rifle lands nine in ten at
//! fourteen and fewer at 24.5. A shot that is going to hit is aimed at the
//! body; one that is going to miss is aimed a little wide of it and flies
//! past, which is what a miss should look like. The roll is drawn from the
//! combat's own stream, so a fight re-rolls nothing in the rest of the
//! room — see `crates/game/CLAUDE.md` on what re-rolling does to probes.
//! A `burst` is several shots to one trigger pull, `burst_gap` apart,
//! each rolled on its own; `fire_rate` is then the rate of trigger pulls,
//! and [`WeaponStats::dps`] counts the burst in.
//!
//! # Time
//!
//! The room's `dt` is real seconds at 1x, so a weapon's speed and fire rate
//! are per real second at 1x and the world's 24x is twenty-four times as
//! fast, the way everything else aboard is.

use crate::balance;
use crate::cue::{Cue, Cued};
use crate::door;
use crate::draw::{Color, DrawList};
use crate::fx::{self, Fx};
use crate::math::{Rect, Vec2, vec2};
use crate::nav::Nav;
use crate::rng::Rng;
use crate::room::TILE;
use crate::sight::{LAMP_RADIUS, Sight};

/// How near a bolt has to pass a body's middle to hit it, in room units:
/// the body's own half-width, near enough.
pub const HIT_RADIUS: f32 = 17.0;

/// How wide of the body a miss is aimed, in room units: past the hit
/// radius by a clear margin, so a miss visibly whistles by.
const MISS_BY: f32 = HIT_RADIUS + 22.0;

/// The melee's numbers, kept with every other number of the fight in
/// `crate::balance` and read from here as they always were.
pub use crate::balance::{
    DODGE_IN_COVER, FIST_DAMAGE, MELEE_PERIOD, MELEE_RANGE, WALKING_ACCURACY,
};

/// The Unmaker's bolt (feature 83): longer than a pistol's, with the
/// discharge jagging off the core in [`LANCE_KINKS`] steps, each thrown
/// [`LANCE_THROW`] room units off the line.
const LANCE_LENGTH: f32 = 46.0;
const LANCE_GLOW: f32 = 9.0;
const LANCE_KINKS: usize = 5;
const LANCE_THROW: f32 = 5.0;

/// **Every gun is a laser** (feature 98), drawn in one language: a core
/// past white that blooms (`crate::fx::laser`, feature 97's emissive), in
/// the side's colour — the crew's blue, the enemy's red, whatever the gun
/// — and the guns told apart by **shape, length, thickness and rhythm**,
/// never by hue. A tier above the first draws a little thicker and a
/// little hotter (`crate::fx::tier_look`). A picture only: nothing the
/// fight reads is a colour or a length here.
///
/// The pistol: one short clean bolt, as long and as thick as this.
const BOLT_LENGTH: f32 = 24.0;
const BOLT_CORE: f32 = 2.0;
const BOLT_GLOW: f32 = 6.0;
/// The shotgun: a spread of five short pulses that opens as it flies —
/// `PELLET_FAN` room units either side of the flight at the muzzle, a
/// further `PELLET_FAN_GROW` a unit flown, out to `PELLET_FAN_MAX` — each
/// set back along the flight by its own `PELLET_STAGGER`, so five of them
/// read as a scatter rather than a comb. The bolt is one bolt, drawn five
/// times.
const PELLETS: usize = 5;
const PELLET_LENGTH: f32 = 8.0;
const PELLET_CORE: f32 = 1.5;
const PELLET_SPREAD: f32 = 6.0 * (std::f32::consts::PI / 180.0);
const PELLET_FAN: f32 = 2.0;
const PELLET_FAN_GROW: f32 = 0.06;
const PELLET_FAN_MAX: f32 = 15.0;
const PELLET_STAGGER: [f32; PELLETS] = [5.0, 1.0, 8.0, 0.0, 4.0];
/// The auto rifle: thin rapid pulses — a dash, a gap and a shorter dash
/// behind it, so eight of them in the air read as a stutter.
const PULSE_LEAD: f32 = 13.0;
const PULSE_GAP: f32 = 5.0;
const PULSE_TAIL: f32 = 6.0;
const PULSE_CORE: f32 = 1.2;
/// The sniper: a beam from the muzzle to the bolt, faint where it left
/// and bright over the last `STREAK_LENGTH` to the head, which hangs in
/// the air a moment after it lands (`crate::fx::BEAM_LINGER`).
const STREAK_LENGTH: f32 = 90.0;
const BEAM_CORE: f32 = 1.5;
/// The minigun (task 115): one short thin dash a bolt — shorter than the
/// pistol's, thinner than the auto rifle's pulses, and no tail — so it
/// reads by its rhythm, ten a second, rather than by any one bolt.
const DASH_LENGTH: f32 = 10.0;
const DASH_CORE: f32 = 0.9;
const DASH_GLOW: f32 = 2.6;
/// The rail lance (task 115): the thickest core of any gun, evenly bright
/// from the muzzle to the head — no faint tail like the sniper's streak
/// and no jag like the Unmaker's — and a hot head.
const RAIL_CORE: f32 = 3.4;
const RAIL_GLOW: f32 = 9.0;
const RAIL_HEAD: f32 = 6.5;

/// How long the flash where a bolt lands lasts, in seconds — the fight's
/// own, on the simulation's clock, and drawn only for a host that ages no
/// effects of its own (`crate::fx`); the rest see the fx flash instead.
const SPARK_LIFE: f32 = 0.18;

/// Friendly fire is blue and an enemy's is red — always, so a glance at
/// the air says who is shooting whom.
pub const FRIENDLY_BOLT: Color = Color::rgb(0.40, 0.72, 1.0);
pub const HOSTILE_BOLT: Color = Color::rgb(1.0, 0.28, 0.22);
const BOLT_CORE_WHITE: Color = Color::rgb(0.92, 0.97, 1.0);

/// What a Bim can shoot with. The discriminants are the codes the app
/// names, written out so a reordering cannot renumber anything; `0` is
/// "nothing in the slot", as everywhere else.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WeaponKind {
    /// The hand laser everybody starts with.
    LaserPistol = 1,
    /// Close and hard: everything it has inside four tiles, little past ten.
    Shotgun = 2,
    /// Eight light shots to a trigger pull, then a wait to recharge.
    AutoRifle = 3,
    /// Nine in ten at fourteen tiles, and still shoots at 24.5.
    SniperRifle = 4,
    /// A blade with a laser edge: used within reach, and a cut bleeds.
    Schword = 5,
    /// A Husk's claws, built into the machine (feature 83): they snap
    /// shut within reach, and are never a thing anybody carries.
    Claw = 6,
    /// A Warden's lance, built in the same way: it strips the armour off
    /// a part rather than opening the body under it.
    Unmaker = 7,
    /// A Guardian's beam (feature 100): wound up, swept across an arc,
    /// and never a bolt — see `Combat::sweep`.
    Sweeper = 8,
    /// Twenty thin bolts to a trigger pull, ten a second, then a long
    /// cool (task 115). Made from tier two up ([`WeaponKind::min_tier`]).
    Minigun = 9,
    /// A slug that goes through a body and on into the next, up to
    /// [`balance::LANCE_PIERCE`] of them (task 115). Tier three only.
    RailLance = 10,
}

impl WeaponKind {
    /// The seven a **body** can carry: what a trader sells, what the
    /// armory keeps and what a hand holds. A droid's arms are not among
    /// them — see [`WeaponKind::BUILT_IN`]. The last two, the minigun and
    /// the rail lance (task 115), exist only from a tier up
    /// ([`WeaponKind::min_tier`]), and only the crew ever carry one: no
    /// issue, hire or garrison table names them.
    pub const ALL: [WeaponKind; 7] = [
        WeaponKind::LaserPistol,
        WeaponKind::Shotgun,
        WeaponKind::AutoRifle,
        WeaponKind::SniperRifle,
        WeaponKind::Schword,
        WeaponKind::Minigun,
        WeaponKind::RailLance,
    ];

    /// The arms that are part of a machine (feature 83): never made,
    /// never bought, never in the armory or a slot. [`WeaponKind::resource`]
    /// is `None` for each, which is what keeps them out of everything
    /// the hold does.
    pub const BUILT_IN: [WeaponKind; 3] =
        [WeaponKind::Claw, WeaponKind::Unmaker, WeaponKind::Sweeper];

    /// Every kind there is: the carried seven and the built-in three, in
    /// code order. What the app names, and what a code is read back
    /// against.
    pub const EVERY: [WeaponKind; 10] = [
        WeaponKind::LaserPistol,
        WeaponKind::Shotgun,
        WeaponKind::AutoRifle,
        WeaponKind::SniperRifle,
        WeaponKind::Schword,
        WeaponKind::Claw,
        WeaponKind::Unmaker,
        WeaponKind::Sweeper,
        WeaponKind::Minigun,
        WeaponKind::RailLance,
    ];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<WeaponKind> {
        WeaponKind::EVERY.iter().copied().find(|k| k.code() == code)
    }

    /// Whether a body can carry one: false for a droid's built-in arms,
    /// which is the one thing keeping them out of the armory and the
    /// trade.
    pub fn carried(self) -> bool {
        WeaponKind::ALL.contains(&self)
    }

    /// Whether a player's hand fires it one shot a click (October 2026):
    /// the pistol. Every press of the button is a shot the step it
    /// reaches the room, once `SEMI_AUTO_COOLDOWN` has passed since the
    /// last — the `fire_rate` is no limit on it — and a button held down
    /// fires again every time that cooldown runs out. A body nobody
    /// steers (a bot, a station's people) still fires it at its
    /// `fire_rate`, since nobody clicks for it.
    pub fn semi_automatic(self) -> bool {
        self == WeaponKind::LaserPistol
    }

    /// What the weapon is in the hold: the `ResourceId` code, the way a
    /// piece of armour has one ([`ArmourKind::resource`]) — a number,
    /// since this crate does not know `physics`.
    pub fn resource(self) -> Option<u32> {
        Some(match self {
            WeaponKind::LaserPistol => 3,
            WeaponKind::Shotgun => 9,
            WeaponKind::AutoRifle => 10,
            WeaponKind::SniperRifle => 11,
            WeaponKind::Schword => 12,
            WeaponKind::Minigun => 18,
            WeaponKind::RailLance => 19,
            // A machine's arm is no resource: it is part of the machine,
            // and there is nothing to put in a hold.
            WeaponKind::Claw | WeaponKind::Unmaker | WeaponKind::Sweeper => return None,
        })
    }

    /// The weapon a resource code is, if it is one. Never a built-in.
    pub fn from_resource(code: u32) -> Option<WeaponKind> {
        WeaponKind::ALL
            .iter()
            .copied()
            .find(|k| k.resource() == Some(code))
    }

    /// The weapon's numbers: one table, `crate::balance`, for a native
    /// server to agree with and for tuning. See the module note on the
    /// two-point curves. The damage, near and far, is the armed
    /// [`balance::WeaponDamage`]'s — the constants unless the app's
    /// `weapons.ron` says otherwise.
    pub fn stats(self) -> WeaponStats {
        let (damage, damage_far) = balance::WeaponDamage::armed().of(self);
        WeaponStats {
            damage,
            damage_far,
            ..self.untuned()
        }
    }

    /// The kind's `balance` row as written, the damage untuned.
    fn untuned(self) -> WeaponStats {
        match self {
            WeaponKind::LaserPistol => balance::LASER_PISTOL,
            WeaponKind::Shotgun => balance::SHOTGUN,
            WeaponKind::AutoRifle => balance::AUTO_RIFLE,
            WeaponKind::SniperRifle => balance::SNIPER_RIFLE,
            WeaponKind::Schword => balance::SCHWORD,
            WeaponKind::Claw => balance::CLAW,
            WeaponKind::Unmaker => balance::UNMAKER,
            WeaponKind::Sweeper => balance::SWEEPER,
            WeaponKind::Minigun => balance::MINIGUN,
            WeaponKind::RailLance => balance::RAIL_LANCE,
        }
    }

    /// The lowest tier the kind is ever made at (task 115): tier one for
    /// every kind but the minigun, which starts at two, and the rail
    /// lance, which is three and nothing else. **A weapon below it never
    /// exists** — [`WeaponKind::at`] asserts it — so a trader never
    /// shelves one, a dial refuses one and a combine never makes one.
    pub fn min_tier(self) -> Tier {
        match self {
            WeaponKind::Minigun => Tier::Two,
            WeaponKind::RailLance => Tier::Three,
            WeaponKind::LaserPistol
            | WeaponKind::Shotgun
            | WeaponKind::AutoRifle
            | WeaponKind::SniperRifle
            | WeaponKind::Schword
            | WeaponKind::Claw
            | WeaponKind::Unmaker
            | WeaponKind::Sweeper => Tier::One,
        }
    }

    /// Whether the kind is ever made at `tier`: at or above its
    /// [`WeaponKind::min_tier`].
    pub fn made_at(self, tier: Tier) -> bool {
        tier >= self.min_tier()
    }

    /// The kind at its lowest tier ([`WeaponKind::min_tier`]): tier one
    /// for the five everybody is issued, tier two for the minigun and
    /// tier three for the rail lance.
    pub fn basic(self) -> Weapon {
        self.at(self.min_tier())
    }

    /// The kind at `tier`, which is never below its
    /// [`WeaponKind::min_tier`] — every path that makes a weapon asks
    /// [`WeaponKind::made_at`] first.
    pub fn at(self, tier: Tier) -> Weapon {
        debug_assert!(
            self.made_at(tier),
            "{self:?} is never made below {:?}",
            self.min_tier()
        );
        Weapon { kind: self, tier }
    }
}

/// How good a piece of equipment is: one of three. Tier one is the
/// baseline, the kind's own numbers; two of the same kind at the same
/// tier are combined at a workbench into one of the next (the world's
/// `Upgrade`). The discriminants are the codes the world hashes and the
/// app names — a tier is never nought.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Tier {
    One = 1,
    Two = 2,
    Three = 3,
}

impl Tier {
    pub const ALL: [Tier; 3] = [Tier::One, Tier::Two, Tier::Three];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<Tier> {
        Tier::ALL.iter().copied().find(|t| t.code() == code)
    }

    /// The tier above, or `None` from the top.
    pub fn next(self) -> Option<Tier> {
        match self {
            Tier::One => Some(Tier::Two),
            Tier::Two => Some(Tier::Three),
            Tier::Three => None,
        }
    }

    /// What a weapon of this tier multiplies the kind's damage, accuracy
    /// and range by — `crate::balance`, three on top of two.
    fn weapon_factors(self) -> (f32, f32, f32) {
        match self {
            Tier::One => (1.0, 1.0, 1.0),
            Tier::Two => (balance::TIER_TWO_DAMAGE, balance::TIER_TWO_ACCURACY, 1.0),
            Tier::Three => (
                balance::TIER_TWO_DAMAGE * balance::TIER_THREE_DAMAGE,
                balance::TIER_TWO_ACCURACY * balance::TIER_THREE_ACCURACY,
                balance::TIER_THREE_RANGE,
            ),
        }
    }

    /// What a piece of this tier multiplies the kind's health and
    /// protection by: [`balance::ARMOUR_TIER_STEP`] a tier.
    pub fn armour_factor(self) -> f32 {
        match self {
            Tier::One => 1.0,
            Tier::Two => balance::ARMOUR_TIER_STEP,
            Tier::Three => balance::ARMOUR_TIER_STEP * balance::ARMOUR_TIER_STEP,
        }
    }
}

/// A weapon: what it is and how good. What a hand holds, the ship's
/// armory keeps, and a shot or a bolt was fired from. A weapon
/// has no wear and no id — two pistols of a tier are the same pistol —
/// which is why it is a value and a piece of armour is an instance.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Weapon {
    pub kind: WeaponKind,
    pub tier: Tier,
}

impl Weapon {
    /// The kind's numbers scaled by the tier: damage and accuracy (the
    /// odds clamped to one), and the range with its sweet spot. Every
    /// curve, the tactics and the tooltips read this and know nothing of
    /// tiers.
    pub fn stats(self) -> WeaponStats {
        let base = self.kind.stats();
        let (damage, accuracy, range) = self.tier.weapon_factors();
        WeaponStats {
            range: base.range * range,
            sweet: base.sweet * range,
            accuracy: (base.accuracy * accuracy).min(1.0),
            accuracy_far: (base.accuracy_far * accuracy).min(1.0),
            damage: base.damage * damage,
            damage_far: base.damage_far * damage,
            // A tier scales what a bolt strips the way it scales what a
            // bolt wounds.
            strips: base.strips * damage,
            strips_far: base.strips_far * damage,
            ..base
        }
    }
}

/// What a weapon does, in the units the app prints them in.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WeaponStats {
    /// How far it reaches, in tiles. Nothing beyond is shot at, and a bolt
    /// dies there.
    pub range: f32,
    /// Out to this many tiles it does its best; from here to `range` the
    /// odds and the damage fall in a straight line to the `_far` numbers.
    pub sweet: f32,
    /// Odds of a hit within `sweet`, 0 to 1, and at `range`.
    pub accuracy: f32,
    pub accuracy_far: f32,
    /// Health points taken off per shot that lands, within `sweet` and at
    /// `range`.
    pub damage: f32,
    pub damage_far: f32,
    /// How fast the shot flies, in tiles a second.
    pub speed: f32,
    /// Trigger pulls a second.
    pub fire_rate: f32,
    /// Shots a trigger pull, `burst_gap` seconds apart.
    pub burst: u32,
    pub burst_gap: f32,
    /// A blade: swung within `range`, never fired.
    pub melee: bool,
    /// What a hit takes off the **piece of armour** over the part it
    /// lands on, that piece's protection ignored, instead of taking
    /// anything off the part itself: the Warden's Unmaker (feature 83),
    /// and nought for every weapon that wounds the body rather than what
    /// is worn over it. Within `sweet` and at `range`, the two-point
    /// curve again.
    pub strips: f32,
    pub strips_far: f32,
}

impl WeaponStats {
    /// What it could do a second with every shot landing, at its best:
    /// the burst, the damage and the trigger rate multiplied.
    pub fn dps(&self) -> f32 {
        self.burst as f32 * self.fire_rate * self.damage
    }

    /// Odds of a hit on a body `tiles` away. See the module note.
    pub fn hit_chance(&self, tiles: f32) -> f32 {
        self.along(tiles, self.accuracy, self.accuracy_far)
    }

    /// What a shot landing on a body `tiles` away takes off it.
    pub fn damage_at(&self, tiles: f32) -> f32 {
        self.along(tiles, self.damage, self.damage_far)
    }

    /// What a shot landing `tiles` away takes off the piece of armour
    /// over the part it reached: nought for everything but the Unmaker.
    pub fn strips_at(&self, tiles: f32) -> f32 {
        self.along(tiles, self.strips, self.strips_far)
    }

    /// What it would do a second to a body `tiles` away with the odds
    /// taken in: the burst, the trigger rate, and the damage and the hit
    /// chance at that distance. Nothing beyond the reach — a blade is
    /// nothing past arm's length. What the tactics weigh a distance by.
    pub fn dps_at(&self, tiles: f32) -> f32 {
        if tiles > self.range {
            return 0.0;
        }
        self.burst as f32 * self.fire_rate * self.damage_at(tiles) * self.hit_chance(tiles)
    }

    /// `near` out to `sweet`, `far` at `range`, a straight line between.
    fn along(&self, tiles: f32, near: f32, far: f32) -> f32 {
        let span = self.range - self.sweet;
        if tiles <= self.sweet || span <= 0.0 {
            near
        } else if tiles >= self.range {
            far
        } else {
            near + (far - near) * ((tiles - self.sweet) / span)
        }
    }

    /// Range and speed in room units.
    pub fn reach(&self) -> f32 {
        self.range * TILE
    }

    fn pace(&self) -> f32 {
        self.speed * TILE
    }
}

/// The trigger of anything that fires: how long until it can be pulled
/// again, and the burst a pull started — shots left of it and the
/// seconds until the next. **One of these for a Bim and one for a
/// sentry** (feature 74), so there is exactly one rule for when a shot
/// goes out: a pull is the first of the weapon's `burst` shots now, the
/// rest `burst_gap` apart, and then `1 / fire_rate` to wait out. What a
/// shot *does* — the roll against the distance, the dodge, the damage
/// where it lands — is [`Combat::fire`] and [`Combat::step`], shared the
/// same way; a shooter is a position, a weapon and one of these.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Trigger {
    /// Seconds until the weapon can fire again.
    pub reload: f32,
    /// Shots left of the burst a trigger pull started, and seconds until
    /// the next of them.
    pub burst_left: u32,
    pub burst_timer: f32,
}

impl Trigger {
    /// The clock: the reload runs down every step, aimed or not.
    pub fn tick(&mut self, dt: f32) {
        self.reload = (self.reload - dt).max(0.0);
    }

    /// A semi-automatic's click (`WeaponKind::semi_automatic`): a shot if
    /// the last one's `cooldown` has run out, and the next cooldown begun.
    pub fn press(&mut self, cooldown: f32) -> bool {
        if self.reload > 0.0 {
            return false;
        }
        self.reload = cooldown;
        self.burst_left = 0;
        true
    }

    /// Nothing to shoot at, or nothing to shoot with: whatever burst was
    /// under way is over. The reload keeps running.
    pub fn hold(&mut self) {
        self.burst_left = 0;
    }

    /// One shot a pull and no burst, at the trigger rate: finishing a
    /// body off, where the picture is the point and a minigun's twenty
    /// into a body on the deck would not be.
    pub fn pull_single(&mut self, stats: &WeaponStats) -> bool {
        if self.reload > 0.0 {
            return false;
        }
        self.reload = 1.0 / stats.fire_rate.max(1e-3);
        self.burst_left = 0;
        true
    }

    /// Aimed at something this step: whether a shot goes out now — the
    /// trigger pulled if the weapon is ready, else the next of the burst
    /// when its gap has run out.
    pub fn pull(&mut self, dt: f32, stats: &WeaponStats) -> bool {
        if self.reload <= 0.0 {
            self.reload = 1.0 / stats.fire_rate.max(1e-3);
            self.burst_left = stats.burst.saturating_sub(1);
            self.burst_timer = stats.burst_gap;
            true
        } else if self.burst_left > 0 {
            self.burst_timer -= dt;
            if self.burst_timer <= 0.0 {
                self.burst_left -= 1;
                self.burst_timer = stats.burst_gap;
                true
            } else {
                false
            }
        } else {
            false
        }
    }
}

/// What a soldier's talents and its brace do to the one shooter (feature
/// 75, `world::class`): every factor a named constant there, handed to
/// the room a Bim each step (`Game::set_skills`) and applied in exactly
/// one place each — [`Skill::stats`] on the weapon's numbers before
/// `Trigger::pull` and [`Combat::fire_as`] read them, the walking odds and
/// the point-blank factor in `fire_as`, the cover odds and the dodge in
/// [`Combat::step`], the melee factor on a blow — so there is still one
/// hit calculation. [`Skill::NONE`] is everybody else's, and changes
/// nothing.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Skill {
    /// What the weapon's odds are multiplied by, near and far — the
    /// brace, *marksman*.
    pub accuracy: f32,
    /// Far odds equal to the near: *deadeye*.
    pub deadeye: bool,
    /// What a bolt's damage is multiplied by within the weapon's `sweet`
    /// range: *point blank*.
    pub point_blank: f32,
    /// What the trigger rate is multiplied by: *drill*, *rampage*.
    pub fire_rate: f32,
    /// The odds on the move, in place of [`WALKING_ACCURACY`]: *steady
    /// aim*.
    pub walking: f32,
    /// What a blow's damage is multiplied by, fist or blade: *bruiser*.
    pub melee: f32,
    /// The odds a bolt is dodged in cover, in place of
    /// [`DODGE_IN_COVER`]: *cover master*.
    pub cover_dodge: f32,
    /// Added to the armour's odds of slipping a bolt in the open: *dug
    /// in*, while braced.
    pub dodge: f32,
    /// What the pace is multiplied by while an enemy is in sight:
    /// *runner*.
    pub pace: f32,
    /// Holds its fire: the weapon stays holstered, whatever it sees — a
    /// medic beaming without *gunner medic* (feature 76).
    pub holds_fire: bool,
    /// What the pace is multiplied by at all times, enemy in sight or
    /// not: a Bastion's haste (task 155) and the relics'.
    pub walk: f32,
    /// What a worn piece's health loses of the damage that gets past its
    /// protection: one for everybody, less for a tank, so the same piece
    /// absorbs more on him. The piece's stored health is never doubled.
    pub armour_drain: f32,
    /// What a worn piece's protection is multiplied by: a relic's
    /// *Field Plating*. (The tank's *iron frame* and *breacher* went with
    /// his talents, task 139.)
    pub armour_protection: f32,
    /// What its working steps run at, over everything else that sets the
    /// effort: a commander's aura (feature 78). One for everybody out of
    /// one.
    pub effort: f32,
    /// What every bolt's damage is multiplied by at any distance, near
    /// and far alike — where [`Skill::point_blank`] bites only within the
    /// weapon's sweet range: an engineer's *sentry mark III* (feature
    /// 88). One for everybody else.
    pub damage: f32,
    /// **Tiles added** to the weapon's range: an engineer's *enhanced
    /// optics* (feature 88). Nought for everybody else. It lengthens what
    /// the shooter aims at and how far the bolt flies, and with it the
    /// span the odds and the damage fall across, so a longer reach is
    /// also a gentler falloff.
    pub range: f32,
    /// **Tiles added** to [`Skill::range`] while the shooter stands still:
    /// a relic's *Wide Angle Optics* (task 142). The room adds it where a
    /// Bim's skill is read for its shot (`Game::tick_combat`). Nought for
    /// everybody else.
    pub still_range: f32,
    /// **Added** to a worn piece's protection, after
    /// [`Skill::armour_protection`] has multiplied it: an engineer's
    /// *higher quality armour* (feature 88). Nought for everybody else.
    pub armour_protection_add: f32,
    /// How long this body takes to revive a downed crewmate, in seconds
    /// (task 120): [`crate::health::REVIVE_SECONDS`] for anybody, a medic's
    /// shorter, and a relic's *Trauma Kit* shorter again — the world's
    /// sum, never under a second.
    #[cfg_attr(feature = "serde", serde(default = "revive_seconds"))]
    pub revive: f32,
    /// Whether this body is a **medic** — of the class, or a hired field
    /// medic — as the world says (task 125): a bot that is not one leaves
    /// a downed crewmate to a medic bot free to go to it
    /// (`Game::revive_on_offer`).
    #[cfg_attr(feature = "serde", serde(default))]
    pub medic: bool,
    /// The share of its bar a crewmate this body revives gets up at
    /// (task 130): [`crate::health::REVIVED_TO`] for anybody, more for a
    /// medic of the class — the world's number.
    #[cfg_attr(feature = "serde", serde(default = "revived_to"))]
    pub revived_to: f32,
    /// Every how many of its shots is **overcharged**, nought for none: a
    /// relic's *Overcharge Cell* (feature 106). The room counts the shots
    /// on the body (`Bim::shots`).
    pub overcharge: u32,
    /// What an overcharged shot's damage is multiplied by.
    pub overcharge_damage: f32,
    /// What every hit that lands on this body is multiplied by, before
    /// the armour takes its share: a relic's *Tether Field* on a crewmate
    /// revived, *Cover Formation* on a bot beside its holder (task 118).
    /// One for everybody else.
    #[cfg_attr(feature = "serde", serde(default = "one"))]
    pub damage_taken: f32,
    /// The chance a weapon hit this body lands on an enemy is
    /// **critical**: a soldier's Weak Spot (task 124). Nought for
    /// everybody else. Rolled where the hit lands, off a stream of the
    /// world's own ([`Combat::lend_crit_rng`]).
    #[cfg_attr(feature = "serde", serde(default))]
    pub crit_chance: f32,
    /// The share of the misses taken away, near and far: the miss chance
    /// times one less it, the hit chance never past one — a braced
    /// soldier's Brace (task 124). Nought for everybody else.
    #[cfg_attr(feature = "serde", serde(default))]
    pub miss_cut: f32,
    /// Whether this body may lock a door from its panel (task 138):
    /// nobody may since the lock stopped being everybody's — the code is
    /// kept for a class that will use it. Unlocking is anybody's, so no
    /// door stays shut against the crew.
    #[cfg_attr(feature = "serde", serde(default))]
    pub locks_doors: bool,
    /// Whether no hit takes this body under one hit point: a tank's
    /// Juggernaut at the Override Core's fifth rank (October 2026).
    #[cfg_attr(feature = "serde", serde(default))]
    pub unyielding: bool,
    /// Whether nothing strips this body's armour: an *Ablative Shell* on
    /// (October 2026) — a lance's bolt lands as damage, as any bolt does.
    #[cfg_attr(feature = "serde", serde(default))]
    pub unstrippable: bool,
    /// The share of every enemy hit on this body that goes back onto
    /// whoever struck it, as a hit by this body: a tank's Reflect Barrier
    /// while it runs (task 155). Nought for everybody else.
    #[cfg_attr(feature = "serde", serde(default))]
    pub reflect: f32,
}

impl Skill {
    /// No talent at all: what everybody with no class of their own
    /// fights with.
    pub const NONE: Skill = Skill {
        accuracy: 1.0,
        deadeye: false,
        point_blank: 1.0,
        fire_rate: 1.0,
        walking: WALKING_ACCURACY,
        melee: 1.0,
        cover_dodge: DODGE_IN_COVER,
        dodge: 0.0,
        pace: 1.0,
        holds_fire: false,
        walk: 1.0,
        armour_drain: 1.0,
        armour_protection: 1.0,
        effort: 1.0,
        damage: 1.0,
        range: 0.0,
        still_range: 0.0,
        armour_protection_add: 0.0,
        revive: crate::health::REVIVE_SECONDS,
        medic: false,
        revived_to: crate::health::REVIVED_TO,
        overcharge: 0,
        overcharge_damage: 1.0,
        damage_taken: 1.0,
        crit_chance: 0.0,
        miss_cut: 0.0,
        locks_doors: false,
        unyielding: false,
        unstrippable: false,
        reflect: 0.0,
    };

    /// The skill for this body's next shot, with `shots` fired before it:
    /// an overcharged one when it is the `overcharge`th, the damage
    /// multiplied; itself otherwise.
    pub fn for_shot(&self, shots: u32) -> Skill {
        let mut skill = *self;
        if self.overcharge > 0 && (shots + 1).is_multiple_of(self.overcharge) {
            skill.damage *= self.overcharge_damage;
        }
        skill
    }

    /// A player's own shot: every weapon at full odds, near and far,
    /// standing or walking — all the misses cut and the walk costing
    /// nothing. A bot's shot keeps the odds its weapon and skill give it.
    pub fn sure(&self) -> Skill {
        Skill {
            miss_cut: 1.0,
            walking: 1.0,
            ..*self
        }
    }

    /// The weapon's numbers through the skill: the odds multiplied (and
    /// clamped to one), the far odds the near with *deadeye*, the trigger
    /// rate multiplied. The range, the damage curve, the burst are the
    /// weapon's own.
    pub fn stats(&self, weapon: Weapon) -> WeaponStats {
        let base = weapon.stats();
        let odds = self.accuracy;
        // A braced soldier's misses cut by a share (task 124): the miss
        // chance times one less it, after every other factor, the hit
        // chance never past one. With none the odds are left as they were,
        // to the bit, so no other fight rolls differently.
        let keep = 1.0 - self.miss_cut.clamp(0.0, 1.0);
        let cut = self.miss_cut > 0.0;
        let steadied = |p: f32| {
            if cut {
                1.0 - (1.0 - p.min(1.0)) * keep
            } else {
                p
            }
        };
        let accuracy = steadied(base.accuracy * odds).min(1.0);
        let accuracy_far = if self.deadeye {
            accuracy
        } else {
            steadied(base.accuracy_far * odds).min(1.0)
        };
        WeaponStats {
            accuracy,
            accuracy_far,
            fire_rate: base.fire_rate * self.fire_rate,
            // *Sentry mark III* and *enhanced optics* (feature 88): the
            // damage at both ends of the curve, and the tiles the curve
            // runs out to.
            damage: base.damage * self.damage,
            damage_far: base.damage_far * self.damage,
            range: base.range + self.range,
            ..base
        }
    }
}

impl Default for Skill {
    fn default() -> Skill {
        Skill::NONE
    }
}

/// A grenade in the air, or lying where it landed with its fuse burning
/// (feature 75): thrown by crew member `by` from `from` at `at`, bursting
/// when `left` runs out — [`GRENADE_FLIGHT`] of that in the air, the rest
/// on the tile — with the radius and the centre damage the thrower's
/// talents gave it. What the burst does is `Game::burst`.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Grenade {
    pub by: usize,
    pub from: Vec2,
    pub at: Vec2,
    /// Seconds until it bursts.
    pub left: f32,
    /// The fuse it was thrown with, for the picture.
    pub fuse: f32,
    /// Room units from the burst everything within takes it.
    pub radius: f32,
    /// What it does at the centre; half that at the edge.
    pub damage: f32,
    /// An engineer's **EMP** (task 127) rather than a grenade: seconds
    /// every machine in the radius is stunned for, nought for a grenade.
    /// An EMP does no damage at all.
    #[cfg_attr(feature = "serde", serde(default))]
    pub stun: f32,
    /// Whether an EMP's stun leaves a machine taking more from everyone
    /// while it lasts (its top rank).
    #[cfg_attr(feature = "serde", serde(default))]
    pub expose: bool,
    /// A soldier's **Stun Shot** (October 2026) rather than a grenade: it
    /// flies the whole of its fuse and bursts where it lands, every
    /// target in the burst taking `damage` — the same at the edge — and
    /// stunned for `stun` seconds, none of the room's own touched.
    #[cfg_attr(feature = "serde", serde(default))]
    pub shot: bool,
    /// An engineer's **mine or satchel charge** going off (task 154): it
    /// bursts where it lies the moment it is set off, on the targets
    /// alone — the grenade's fall-off and cover, none of the room's own,
    /// no sentry and no sandbag touched (`Game::detonate`).
    #[cfg_attr(feature = "serde", serde(default))]
    pub laid: bool,
    /// An engineer's **satchel charge** in the air (task 154): it flies
    /// the grenade's flight, bursts nothing and lands, for the world to lay
    /// on the deck (`Game::take_satchels_landed`).
    #[cfg_attr(feature = "serde", serde(default))]
    pub satchel: bool,
}

impl Grenade {
    /// Where it is now: along the throw for the first
    /// [`GRENADE_FLIGHT`] seconds, then on the tile — a shot along the
    /// whole of its fuse.
    pub fn pos(&self) -> Vec2 {
        let flown = ((self.fuse - self.left) / self.flight()).clamp(0.0, 1.0);
        self.from + (self.at - self.from) * flown
    }

    /// Seconds it is in the air.
    pub fn flight(&self) -> f32 {
        if self.shot {
            self.fuse.max(1e-3)
        } else {
            GRENADE_FLIGHT
        }
    }
}

/// How long a grenade is in the air before it lands on its tile, in
/// seconds: the picture, not the fuse.
pub const GRENADE_FLIGHT: f32 = 0.6;

/// A burst that has happened, briefly lit: where, how wide, how old.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct Blast {
    pos: Vec2,
    radius: f32,
    age: f32,
    /// An EMP's burst (task 127): a pale blue ring and nothing else.
    #[cfg_attr(feature = "serde", serde(default))]
    emp: bool,
}

/// How long a burst's flash lasts, in seconds.
const BLAST_LIFE: f32 = 0.45;

/// How far back along its flight a bolt's wake is laid each frame, in
/// room units (`Combat::wakes`).
const WAKE_LENGTH: f32 = 22.0;

/// An engineer's sentry as the room keeps it (feature 74): a shooter
/// that is not a body — a position on the deck, a weapon, a [`Skill`]
/// its owner's talents made and a trigger. The world owns the sentry —
/// its health, whose it is — and hands the room the list every
/// step (`Game::set_sentries`); the room fires each at the nearest
/// visible enemy in range through the same [`Combat::aim`] and
/// [`Combat::fire_as`] a Bim uses,
/// and puts every sentry after the crew on the list of bodies a hostile
/// bolt looks for, so a hit on one comes back as a hit on that index
/// (`Game::take_sentry_hits`). **It never runs out of shots** (feature
/// 88): nothing in the game carries ammunition, so a sentry fires for as
/// long as it stands.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Sentry {
    /// The world's id for it.
    pub id: u32,
    pub at: Vec2,
    pub weapon: Weapon,
    /// What its owner's talents do to its shooting (feature 88): the
    /// range *enhanced optics* adds, and the fire rate and the damage
    /// *sentry mark III* multiplies. [`Skill::NONE`] without either.
    pub skill: Skill,
    /// Whether it is the engineer's **Healing Sentry** (task 127): a
    /// sentry body with no barrel, which the enemies aim at like any
    /// other and which never fires. What it heals is the world's.
    #[cfg_attr(feature = "serde", serde(default))]
    pub heals: bool,
    /// Its trigger, kept between steps the way a Bim's is.
    pub trigger: Trigger,
    /// Which way its barrel points, radians in the room's frame (nought
    /// is east): it swings toward what it aims at by [`SENTRY_TURN`] a
    /// second, and holds where it was left while nothing is in sight.
    /// **Drawing only** — it never decides a shot — so it is neither saved
    /// nor hashed, and kept between steps as the trigger is.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub facing: f32,
    /// Seconds left of the flash at its muzzle after a shot, for the
    /// picture ([`SENTRY_FLASH`] when it fires). Drawing only, like
    /// `facing`.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub flash: f32,
}

/// How fast a sentry's barrel swings round to its target, radians a
/// second — a half turn in a quarter of a second.
pub const SENTRY_TURN: f32 = 4.0 * core::f32::consts::PI;
/// How long the flash at a sentry's muzzle shows after a shot, seconds.
pub const SENTRY_FLASH: f32 = 0.06;

impl Sentry {
    /// Swing the barrel toward the bearing `to` (radians) by at most `dt`
    /// seconds of [`SENTRY_TURN`], the short way round.
    pub fn turn_toward(&mut self, to: f32, dt: f32) {
        let tau = core::f32::consts::TAU;
        let mut off = (to - self.facing) % tau;
        if off > core::f32::consts::PI {
            off -= tau;
        } else if off < -core::f32::consts::PI {
            off += tau;
        }
        let most = SENTRY_TURN * dt;
        self.facing = (self.facing + off.max(-most).min(most)) % tau;
    }
}
/// What a Bim can wear. The discriminants are the codes the app names
/// (`ARMOUR_NAMES`), written out like the weapons'; `0` is the empty
/// slot. There is one kind since October 2026: **the armour**, over the
/// whole body, where there were a helm, a kevlar vest and leg guards (and
/// task 116's arc greaves and Reflective plate) — a hit lands on the body
/// and nowhere in particular.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ArmourKind {
    /// The armour: one piece, at a tier.
    Armour = 1,
}

impl ArmourKind {
    pub const ALL: [ArmourKind; 1] = [ArmourKind::Armour];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<ArmourKind> {
        ArmourKind::ALL.iter().copied().find(|k| k.code() == code)
    }

    /// What the piece is in the hold: the `ResourceId` code, since a piece
    /// in a container is a resource and one anywhere else is an instance.
    /// A code and not the id — this crate does not know `physics`, and a
    /// number is enough for the world to match the two.
    pub fn resource(self) -> u32 {
        match self {
            ArmourKind::Armour => 7,
        }
    }

    /// The piece's numbers: in `crate::balance`, like the weapons'.
    pub fn stats(self) -> ArmourStats {
        match self {
            ArmourKind::Armour => balance::ARMOUR,
        }
    }
}

/// What a piece of armour does, in health points.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ArmourStats {
    /// What the piece can take before it is broken, and what it adds to
    /// the part's health while it is whole.
    pub health: f32,
    /// Taken off every hit's damage before anything else, while whole.
    pub protection: f32,
}

/// One piece of armour, wherever it is: its `id` (the world's, only ever
/// climbing, so a piece is the same piece in the hold and on a body), what
/// it is, and what it has left. See the module note.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Piece {
    pub id: u32,
    pub kind: ArmourKind,
    pub tier: Tier,
    pub health: f32,
}

impl Piece {
    /// A fresh piece, whole at the tier's health.
    pub fn new(id: u32, kind: ArmourKind, tier: Tier) -> Piece {
        let mut piece = Piece {
            id,
            kind,
            tier,
            health: 0.0,
        };
        piece.health = piece.stats().health;
        piece
    }

    /// The kind's numbers scaled by the tier — what the health bars, the
    /// protection and the tooltips read, never `kind.stats()`.
    pub fn stats(&self) -> ArmourStats {
        let base = self.kind.stats();
        let factor = self.tier.armour_factor();
        ArmourStats {
            health: base.health * factor,
            protection: base.protection * factor,
        }
    }

    /// Worn out: still worn, doing nothing.
    pub fn broken(&self) -> bool {
        self.health <= 0.0
    }

    /// What it takes off a hit: the tier's protection, or nothing once it
    /// is broken.
    pub fn effective_protection(&self) -> f32 {
        if self.broken() {
            0.0
        } else {
            self.stats().protection
        }
    }

    /// The odds a bolt reaching the body wearing it is dodged: a whole
    /// tier-three piece's [`balance::TIER_THREE_DODGE`], anything else
    /// nought.
    pub fn dodge(&self) -> f32 {
        if self.tier == Tier::Three && !self.broken() {
            balance::TIER_THREE_DODGE
        } else {
            0.0
        }
    }

    /// What it adds to the body's health: what it has left, or nothing
    /// once it is broken.
    pub fn bonus(&self) -> f32 {
        if self.broken() { 0.0 } else { self.health }
    }
}

/// A thing of the crew's, as the room holds it: a piece of armour, a
/// weapon, or a **charge** by its `ResourceId` code — an engineer's kit, a
/// grenade (the world knows what the number is; the
/// room only counts it). A piece or a weapon is worn or held
/// ([`Gear`]'s slots); a charge is a count on the body
/// ([`Gear::charges`]) and never a thing lying anywhere (task 113: nothing
/// is carried in a pack any more).
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Item {
    Armour(Piece),
    Weapon(Weapon),
    Stack(u32),
    /// One of a player's Bim's four items (October 2026,
    /// [`crate::module`]).
    Module(crate::module::Module),
}

/// How many resource codes a body keeps a charge count for: every
/// `ResourceId` code is under it, which a world test pins.
pub const CHARGE_CODES: usize = 24;

/// What one Bim has on it — its **loadout** (task 113): one armour slot
/// (October 2026: there were three, head, body and legs), the weapon in
/// its hand, and how many of each charge it has left. That is everything
/// a body carries: there is no pack, and nothing is picked up, dropped or
/// looted.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Gear {
    pub armour: Option<Piece>,
    pub weapon: Option<Weapon>,
    /// How many of each charge the body has, by the charge's resource
    /// code: an engineer's kits, a grenade. The world sets them at a
    /// mission's start and fills them on their cooldowns.
    pub charges: [u32; CHARGE_CODES],
    /// The four items (October 2026, [`crate::module`]): a player's Bim's
    /// alone, empty on everybody else.
    #[cfg_attr(feature = "serde", serde(default))]
    pub items: [Option<crate::module::Module>; crate::module::ITEM_SLOTS],
}

impl Gear {
    /// What everybody is issued: nothing to wear, a hand laser, and no
    /// charges.
    pub fn issued() -> Gear {
        Gear {
            weapon: Some(WeaponKind::LaserPistol.basic()),
            ..Gear::default()
        }
    }

    /// What a station's resident is issued: a weapon rolled off `seed` —
    /// half of them the pistol, a fifth a shotgun, a few a rifle, fewer a
    /// sniper rifle, and one in ten a schword — and nothing else, unless
    /// the weapon is the schword: **a melee bot always wears the basic
    /// armour** ([`Gear::basic_armour`]), since a body of 75 with a blade
    /// is dead before it closes. Off a stream of its own so the roll is a
    /// function of the seed alone, which is what lets the world derive it
    /// rather than keep it.
    pub fn issued_for(seed: u64) -> Gear {
        let mut rng = Rng::new(seed);
        let weapon = roll_weapon(&mut rng, &ISSUE_ODDS).basic();
        let mut gear = Gear {
            weapon: Some(weapon),
            ..Gear::default()
        };
        if weapon.stats().melee {
            gear.basic_armour(1);
        }
        gear
    }

    /// Put a fresh tier-one armour on, numbered `id`, over whatever was
    /// worn — what every melee bot gets, enemy or hired. The id is the
    /// room's own until the world takes the piece over.
    pub fn basic_armour(&mut self, id: u32) {
        self.armour = Some(Piece::new(id, ArmourKind::Armour, Tier::One));
    }

    /// What a mercenary carries: better arms than a resident's, rolled off
    /// `seed` against [`MERCENARY_ODDS`] — a third the pistol, the rest
    /// the heavier guns and a few the schword — and the armour with the
    /// odds [`MERCENARY_ARMOUR_ODDS`], fresh, numbered `piece_id` (the
    /// room's pieces are only its own until the world takes one over) —
    /// always, for one carrying the schword, since a melee bot always
    /// wears the basic armour. A function of the seed alone, like
    /// [`Gear::issued_for`], so the world derives it.
    pub fn hired_for(seed: u64, piece_id: u32) -> Gear {
        let mut rng = Rng::new(seed ^ 0x_4D45_5243);
        let weapon = roll_weapon(&mut rng, &MERCENARY_ODDS).basic();
        // Rolled either way, so the stream is the same whatever the
        // weapon was.
        let worn = rng.chance(MERCENARY_ARMOUR_ODDS) || weapon.stats().melee;
        Gear {
            armour: worn.then(|| Piece::new(piece_id, ArmourKind::Armour, Tier::One)),
            weapon: Some(weapon),
            ..Gear::default()
        }
    }

    /// What a **Manufacturer** carries (feature 109), off `seed`: with
    /// `weapon` `None`, the laser pistol and nothing else, since that is
    /// all the first days of a run arm them with; with a tier, a gun rolled
    /// against [`MANUFACTURER_ODDS`] at it — a gun and never the schword,
    /// since a blade with no armour on is dead before it closes — and a
    /// fresh armour at `armour`'s tier, numbered `piece_id`, where there is
    /// one. A function of its arguments alone, like [`Gear::issued_for`],
    /// so the world derives it.
    pub fn manufacturer(
        seed: u64,
        weapon: Option<Tier>,
        armour: Option<Tier>,
        piece_id: u32,
    ) -> Gear {
        let mut rng = Rng::new(seed ^ 0x_4D41_4E55);
        let weapon = match weapon {
            None => WeaponKind::LaserPistol.basic(),
            Some(tier) => roll_weapon(&mut rng, &MANUFACTURER_ODDS).at(tier),
        };
        Gear {
            armour: armour.map(|t| Piece::new(piece_id, ArmourKind::Armour, t)),
            weapon: Some(weapon),
            ..Gear::default()
        }
    }

    /// The armour worn, if any — broken or not.
    pub fn worn(&self) -> Option<Piece> {
        self.armour
    }

    pub fn worn_mut(&mut self) -> &mut Option<Piece> {
        &mut self.armour
    }

    /// The odds a bolt reaching this body is dodged for its armour: the
    /// worn piece's [`Piece::dodge`]. Nought for anything below tier
    /// three.
    pub fn dodge(&self) -> f32 {
        self.armour.map_or(0.0, |p| p.dodge())
    }

    /// What the armour adds to the body's health — its health left,
    /// nothing when there is none or it is broken — which is what the
    /// panel's blue bar is.
    pub fn armour_health(&self) -> f32 {
        self.armour.map_or(0.0, |p| p.bonus())
    }

    /// How many of a charge the body has left.
    pub fn units_of(&self, item: Item) -> u32 {
        match item {
            Item::Stack(code) => self.charges.get(code as usize).copied().unwrap_or(0),
            _ => 0,
        }
    }

    /// Put `n` more of a charge on the body; how many went on (nought
    /// for a thing that is not a charge).
    pub fn add(&mut self, item: Item, n: u32) -> u32 {
        match item {
            Item::Stack(code) => match self.charges.get_mut(code as usize) {
                Some(have) => {
                    *have = have.saturating_add(n);
                    n
                }
                None => 0,
            },
            _ => 0,
        }
    }

    /// Spend one of a charge: `false` with none left.
    pub fn spend(&mut self, item: Item) -> bool {
        match item {
            Item::Stack(code) => match self.charges.get_mut(code as usize) {
                Some(have) if *have > 0 => {
                    *have -= 1;
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    /// Leave exactly `n` of a charge on the body.
    pub fn set_units(&mut self, item: Item, n: u32) {
        if let Item::Stack(code) = item
            && let Some(have) = self.charges.get_mut(code as usize)
        {
            *have = n;
        }
    }
}

/// What a resident is issued, by the odds: the shares add to one, and
/// whatever is left over by rounding goes to the last.
const ISSUE_ODDS: [(WeaponKind, f32); 5] = [
    (WeaponKind::LaserPistol, 0.50),
    (WeaponKind::Shotgun, 0.20),
    (WeaponKind::AutoRifle, 0.15),
    (WeaponKind::SniperRifle, 0.05),
    (WeaponKind::Schword, 0.10),
];

/// What a mercenary is armed with, by share: the heavier guns are the
/// trade. See [`Gear::hired_for`].
pub const MERCENARY_ODDS: [(WeaponKind, f32); 5] = [
    (WeaponKind::LaserPistol, 0.35),
    (WeaponKind::Shotgun, 0.25),
    (WeaponKind::AutoRifle, 0.20),
    (WeaponKind::SniperRifle, 0.12),
    (WeaponKind::Schword, 0.08),
];
/// What a Manufacturer is armed with once it carries more than the pistol
/// (feature 109), by share: the guns alone. See [`Gear::manufacturer`].
/// The schword is first at nothing rather than left out, so the table is
/// every kind, and never last, since the last row takes whatever the
/// rounding leaves.
pub const MANUFACTURER_ODDS: [(WeaponKind, f32); 5] = [
    (WeaponKind::Schword, 0.0),
    (WeaponKind::LaserPistol, 0.30),
    (WeaponKind::Shotgun, 0.25),
    (WeaponKind::AutoRifle, 0.30),
    (WeaponKind::SniperRifle, 0.15),
];
/// The odds a mercenary wears armour: the helm's, the kevlar's and the
/// leg guards' odds (a half, three fifths, two fifths) put together.
pub const MERCENARY_ARMOUR_ODDS: f32 = 0.5;

/// One weapon rolled off `rng` against a table of shares that add to
/// one; the last kind takes whatever rounding leaves.
fn roll_weapon(rng: &mut Rng, odds: &[(WeaponKind, f32)]) -> WeaponKind {
    let roll = rng.unit();
    let mut edge = 0.0;
    for &(kind, share) in odds {
        edge += share;
        if roll < edge {
            return kind;
        }
    }
    odds.last()
        .map_or(WeaponKind::LaserPistol, |(kind, _)| *kind)
}

/// One enemy as the room keeps it: where it stands — the peek position
/// while it peeks — what it carries, and whether it is peeking from
/// cover, which is what a bolt reaching it is dodged for.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Target {
    pub at: Vec2,
    pub weapon: Weapon,
    pub peeking: bool,
    /// A belief rather than a sighting: where a hostile room's people
    /// last saw it, nobody seeing it now (`Game::set_hostiles`). The
    /// tactics walk towards it; nobody aims, locks or fires at it.
    pub stale: bool,
    /// The odds a bolt reaching it is dodged for its armour
    /// (`Gear::dodge`, handed across by the world through
    /// `Game::set_hostiles_dodge`); nought until it is said.
    pub dodge: f32,
    /// How far a **taunt** of this target reaches, in room units, while
    /// one is running on it — a tank's, feature 77; nought for anybody
    /// not taunting, which is everybody until the world says otherwise.
    /// A body within it that can see this target and has it in reach
    /// fires at it before any nearer one ([`Combat::aim`]).
    pub taunting: f32,
    /// And whether that taunt pulls charging blades as well as fire: the
    /// tank's Taunt at its fourth rank ([`Tactics::charge`]).
    pub magnet: bool,
    /// Which taunt is the more recent where two reach one shooter (task
    /// 139): the higher is followed. Nought with none running.
    #[cfg_attr(feature = "serde", serde(default))]
    pub taunt_order: u32,
    /// The way a **shield** on this target faces, a unit vector in this
    /// room's frame — a Guardian's (feature 100), said by the world
    /// through [`Combat::set_shields`] — or `None` for everybody else. A
    /// bolt or a blow coming in within [`balance::GUARDIAN_SHIELD_COS`] of
    /// it is stopped at the plate ([`shield_stops`]); a grenade's burst
    /// is not asked.
    #[cfg_attr(feature = "serde", serde(default))]
    pub shield: Option<Vec2>,
}

/// A **taunt** running on a target (features 77 and 139): what the world
/// hands the room of a tank's Taunt and Juggernaut each step, for
/// [`Combat::set_taunting`]. Every enemy within `radius` that can see the
/// target shoots at it and nobody else ([`Combat::aim_among`]).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Taunt {
    /// How far it reaches in room units: nought is no taunt, and
    /// `f32::INFINITY` a Juggernaut's, which reaches every enemy that can
    /// see him.
    pub radius: f32,
    /// Whether it pulls charging blades as well as fire.
    pub magnet: bool,
    /// Which of two is the more recent: the higher is followed.
    pub order: u32,
}

impl Taunt {
    /// No taunt at all.
    pub const NONE: Taunt = Taunt {
        radius: 0.0,
        magnet: false,
        order: 0,
    };

    fn put_on(targets: &mut [Option<Target>], taunts: &[Taunt]) {
        for (i, target) in targets.iter_mut().enumerate() {
            if let Some(t) = target {
                let taunt = taunts.get(i).copied().unwrap_or(Taunt::NONE);
                t.taunting = taunt.radius;
                t.magnet = taunt.magnet;
                t.taunt_order = taunt.order;
            }
        }
    }
}

impl Target {
    /// Whether a shell all the way round guards it — a Machine Heart's
    /// sealed core (feature 108), whose shield is the zero vector.
    pub fn sealed(&self) -> bool {
        self.shield == Some(Vec2::ZERO)
    }
}

/// Whether a shield facing `heading` (a unit vector) stops what comes in
/// **from** `toward` — a unit vector pointing from the shielded body back
/// the way the bolt or the blow came. A dot product against
/// [`balance::GUARDIAN_SHIELD_COS`], the ±60° front arc, and nothing
/// else: no angle is ever worked out, so two platforms agree to the bit,
/// and the edge itself is stopped.
pub fn shield_stops(heading: Vec2, toward: Vec2) -> bool {
    shield_stops_within(heading, toward, balance::GUARDIAN_SHIELD_COS)
}

/// [`shield_stops`] across a front `front_cos` wide — a cosine, the
/// Guardian's own [`balance::GUARDIAN_SHIELD_COS`] for everybody and a
/// narrower one for a shooter with a relic's *Wide Angle Optics* (task
/// 118, [`Combat::set_shield_fronts`]).
pub fn shield_stops_within(heading: Vec2, toward: Vec2, front_cos: f32) -> bool {
    // A heading of nought is a shell all the way round: a Machine Heart's
    // sealed core (feature 108), which stops everything from every side.
    heading == Vec2::ZERO || heading.dot(toward) >= front_cos
}

/// The front a shield turns against a shot of `by`'s, off a list by
/// index ([`Combat::set_shield_fronts`]).
fn shield_front_of(fronts: &[f32], by: Option<usize>) -> f32 {
    by.and_then(|b| fronts.get(b).copied())
        .unwrap_or(balance::GUARDIAN_SHIELD_COS)
}

/// Whether a hit by own body `by` is critical (task 124): drawn off the
/// lent crit stream only for a body with a chance — no draw at all for
/// any other, and none with no stream lent.
fn roll_crit_off(rng: &mut Option<Rng>, chances: &[f32], by: Option<usize>) -> bool {
    let chance = by.and_then(|b| chances.get(b).copied()).unwrap_or(0.0);
    match rng {
        Some(rng) if chance > 0.0 => rng.chance(chance),
        _ => false,
    }
}

/// One shot in the air.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bolt {
    pub pos: Vec2,
    /// Room units a second.
    pub vel: Vec2,
    /// Room units it may still fly.
    pub left: f32,
    /// What fired it: its picture, and its damage curve.
    pub weapon: Weapon,
    /// Where it was fired from, for the damage at the distance it has
    /// flown when it lands.
    pub fired_from: Vec2,
    /// Fired by an enemy: red, and looking for this room's own bodies
    /// rather than for the targets. See the module note.
    pub hostile: bool,
    /// Whose hand fired it: a crew member of this room, by index, for a
    /// friendly bolt a Bim fired — the soldier's *rampage* wants to know
    /// who downed whom — and `None` for a sentry's or an enemy's.
    pub by: Option<usize>,
    /// What its damage is multiplied by while it has flown no further
    /// than the weapon's `sweet` range: the soldier's *point blank*, one
    /// for anybody else.
    pub point_blank: f32,
    /// What its damage is multiplied by however far it has flown: a
    /// sentry's *mark III* (feature 88), one for anybody else.
    pub damage: f32,
    /// Tiles added to the weapon's range for this bolt — *enhanced
    /// optics* (feature 88), nought for anybody else — so the curve it
    /// lands on is the one it was aimed along.
    pub range: f32,
    /// The one body it has already slipped past — a peek that dodged it —
    /// so a bolt crossing a body over several steps is rolled for once.
    dodged: Option<usize>,
    /// The bodies a **rail lance**'s slug has struck so far, in order —
    /// by the same index `dodged` is — so it never strikes one twice and
    /// the next takes [`balance::LANCE_FALLOFF`] once more (task 115). A
    /// fixed array, not a list, so a bolt stays `Copy`; `None` past the
    /// last, and all `None` for every other gun's bolt, which stops at
    /// the first body it strikes.
    #[cfg_attr(feature = "serde", serde(default))]
    struck: [Option<usize>; balance::LANCE_PIERCE],
    /// Who fired a hostile bolt, by its index on this room's targets —
    /// the shooter a Reflect Barrier sends the hit back to (task 155) —
    /// or `None` for a friendly bolt and a hostile one nobody named.
    #[cfg_attr(feature = "serde", serde(default))]
    pub shooter: Option<usize>,
}

impl Bolt {
    /// The curve it flies and lands on: the weapon's own with the tiles
    /// [`Bolt::range`] added, so the damage falls off across the span it
    /// was aimed along. [`Bolt::damage`] is *not* in it — that one is
    /// multiplied in where the bolt lands, beside the point-blank factor.
    pub fn stats(&self) -> WeaponStats {
        let base = self.weapon.stats();
        WeaponStats {
            range: base.range + self.range,
            ..base
        }
    }

    /// Whether it goes through the bodies it strikes: a rail lance's slug
    /// (task 115), and nothing else.
    pub fn pierces(&self) -> bool {
        self.weapon.kind == WeaponKind::RailLance
    }

    /// How many bodies it has struck so far: nought for every bolt in the
    /// air but a lance slug that has gone through somebody.
    pub fn strikes(&self) -> usize {
        self.struck.iter().filter(|s| s.is_some()).count()
    }

    /// Whether it has struck that body already.
    fn has_struck(&self, who: usize) -> bool {
        self.struck.contains(&Some(who))
    }
}

/// A bolt that reached a body, or a blow that landed: whose — an index
/// into the targets for a friendly one, into this room's own crew for a
/// hostile one — how hard, and whether it is a cut, which
/// bleeds three times what a shot does.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Hit {
    pub who: usize,
    pub damage: f32,
    pub cut: bool,
    /// Whose it was: a crew member of the room it landed in, by index,
    /// or `None` for a sentry's bolt and for anything an enemy did.
    pub by: Option<usize>,
    /// A grenade's burst rather than a bolt or a blow: the body splashes
    /// blood the way a cut does, whichever room it lands in.
    pub blast: bool,
    /// A roll, 0 to 1, drawn the instant it landed, for a body with a
    /// **part table** to read where it struck: a droid's four (feature
    /// 83, `crate::droid::DroidPart`). A Bim has none (October 2026) and
    /// ignores it; drawn for every hit either way, so which kind of body
    /// a target turns out to be does not move the combat stream.
    pub roll: f32,
    /// What this hit takes off the armour instead of off the body, the
    /// armour's protection ignored — the Unmaker's
    /// ([`WeaponStats::strips`]). Nought for every other weapon, and
    /// nought on a bare body, which takes `damage`.
    pub strips: f32,
    /// The weapon's own damage at the distance flown — before any skill,
    /// relic or ability factor — for a critical hit to add a share of
    /// (task 124, Weak Spot). Nought where nothing can be critical: a
    /// burst, a hostile's shot, a sentry's.
    #[cfg_attr(feature = "serde", serde(default))]
    pub flat: f32,
    /// Whether it is a **critical** hit (task 124): rolled where it
    /// landed off the world's own crit stream, for a Bim's weapon hit on
    /// a target with a crit chance. The world adds what it is worth.
    #[cfg_attr(feature = "serde", serde(default))]
    pub crit: bool,
}

/// A blow on its way: a swing or a jab that has started, landing on the
/// target with that index — `damage`, a cut or not — when `left` seconds
/// have run out, which is the animation's length
/// (`crate::character::SWING_TIME`). It lands only if the target is still
/// within reach then ([`Combat::within_reach`]): a body that stepped back
/// during the swing is missed. `Game::tick_combat` keeps one on the Bim.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Blow {
    pub target: usize,
    pub left: f32,
    pub damage: f32,
    pub cut: bool,
    /// The blade's or the fist's own damage, before any factor: what a
    /// critical blow adds a share of (task 124, [`Hit::flat`]).
    #[cfg_attr(feature = "serde", serde(default))]
    pub flat: f32,
}

/// A shot an enemy took, for the world to carry into the crew's room and
/// fire there. Where from — the eye it was aimed from, which for a peek
/// is beside the body — where at, and with what. A **melee** one is a
/// blow, not a shot: nothing flies, and the world delivers it straight to
/// the body (`Game::enemy_strike`) with the damage and whether it cuts
/// carried here, since a fist's damage is not the gun's in the hand.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Shot {
    pub from: Vec2,
    pub at: Vec2,
    pub weapon: Weapon,
    pub melee: bool,
    pub damage: f32,
    pub cut: bool,
    /// Whether the shooter was walking as it fired: half the odds.
    pub moving: bool,
    /// A Guardian's **Sweeper** (feature 100) rather than a bolt: the beam
    /// sweeps from the aim at `at` round to the aim at this point, both
    /// out at the beam's reach from `from`, and the receiving room lays a
    /// [`Sweep`] of its own over the same arc ([`Combat::sweep`]). Two
    /// points rather than an angle and a side so the station's frame,
    /// turn or mirror, carries it across by the points alone. `damage` is
    /// what each body it crosses takes, the machine's arms counted.
    #[cfg_attr(feature = "serde", serde(default))]
    pub sweep: Option<Vec2>,
    /// How many times faster than a Guardian's that sweep goes: one for
    /// every beam but a Machine Heart's core in its overload (feature
    /// 108). Carried with the shot so the room it is laid in sweeps it
    /// at the same pace.
    #[cfg_attr(feature = "serde", serde(default = "one"))]
    pub pace: f32,
    /// Who took it, by its index among the bodies of the room it was
    /// taken in — Bims first, then machines, the index the crew's room
    /// has it at on its targets — for a Reflect Barrier to send the hit
    /// back to (task 155). `None` where nobody said.
    #[cfg_attr(feature = "serde", serde(default))]
    pub shooter: Option<usize>,
}

/// One: the pace of every sweep but an overloaded core's.
#[cfg(feature = "serde")]
fn one() -> f32 {
    1.0
}

/// A revive's time with nothing to speed it, for a `Skill` read back
/// without one.
#[cfg(feature = "serde")]
fn revive_seconds() -> f32 {
    crate::health::REVIVE_SECONDS
}

/// Anybody's revive's share of the bar, for a `Skill` read back without
/// one (task 130).
#[cfg(feature = "serde")]
fn revived_to() -> f32 {
    crate::health::REVIVED_TO
}

/// How many fixed sub-steps a Sweeper's sweep is resolved in (feature
/// 100), and the turn between two of them — 2°, the arc's twenty over
/// ten, written out as a cosine and a sine so no angle is ever worked
/// out. A sweep is resolved at `SWEEP_STEPS + 1` places, its two ends
/// included.
pub const SWEEP_STEPS: u32 = 10;
pub const SWEEP_STEP_COS: f32 = 0.999_390_8;
pub const SWEEP_STEP_SIN: f32 = 0.034_899_496;
/// Half the arc, 10°, the same way: how far either side of the aim a
/// sweep starts and ends.
pub const SWEEP_HALF_COS: f32 = 0.984_807_75;
pub const SWEEP_HALF_SIN: f32 = 0.173_648_18;
/// How a swept beam is drawn: the red beam's width, its core's, how far
/// past white the core burns, and the burning point where a wall stops it.
const SWEEP_BEAM: f32 = 5.0;
const SWEEP_CORE: f32 = 2.2;
const SWEEP_HEAT: f32 = 2.2;
const SWEEP_TIP: f32 = 9.0;

/// A Guardian's beam being swept in a room (feature 100, [`Combat::sweep`]):
/// where from, which way it points now and which way it turns, how far
/// it reaches, what it does, how far through the sweep it is and which
/// bodies have been rolled for — each at most once a sweep.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Sweep {
    pub from: Vec2,
    dir: Vec2,
    /// The signed sine of the sub-step's turn: which way round it goes.
    sin: f32,
    reach: f32,
    pub weapon: Weapon,
    pub damage: f32,
    /// Whether this room draws it.
    pub drawn: bool,
    pub elapsed: f32,
    /// How many of its sub-steps have been resolved.
    pub done: u32,
    /// The bodies rolled for this sweep, by index: never twice.
    pub rolled: Vec<usize>,
    /// Where the beam ended at the last sub-step resolved, and whether a
    /// wall or a shut door stopped it there.
    pub end: Vec2,
    pub walled: bool,
    /// How many times faster than a Guardian's it sweeps: [`Shot::pace`].
    #[cfg_attr(feature = "serde", serde(default = "one"))]
    pub pace: f32,
    /// Who swept it, as [`Bolt::shooter`].
    #[cfg_attr(feature = "serde", serde(default))]
    pub shooter: Option<usize>,
}

/// A Reflect Barrier's hit back (task 155): an enemy's `hit` on body
/// `hit.who` of this room, sent back on `shooter` at that body's share in
/// `reflects` as a hit of its own — the part read off the same roll, no
/// crit, nothing stripped. `None` with no share or nobody to send it to.
fn reflected(reflects: &[f32], hit: &Hit, shooter: Option<usize>) -> Option<Hit> {
    let share = reflects.get(hit.who).copied().unwrap_or(0.0);
    let shooter = shooter?;
    (share > 0.0 && hit.damage > 0.0).then_some(Hit {
        who: shooter,
        damage: hit.damage * share,
        cut: false,
        by: Some(hit.who),
        blast: false,
        roll: hit.roll,
        flat: 0.0,
        crit: false,
        strips: 0.0,
    })
}

/// How far `p` lies from the segment `a`–`b`: the nearest point of the
/// segment, clamped to its ends, and plain arithmetic.
fn off_segment(a: Vec2, b: Vec2, p: Vec2) -> f32 {
    let d = b - a;
    let len2 = d.dot(d);
    if len2 <= 1e-9 {
        return (p - a).len();
    }
    let t = ((p - a).dot(d) / len2).clamp(0.0, 1.0);
    (p - (a + d * t)).len()
}

/// Where a bolt ended, briefly lit.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct Spark {
    pos: Vec2,
    age: f32,
    hostile: bool,
}

/// A tank's **Riot Shield** held up (task 155, `world::class`): which of
/// this room's own bodies holds it, the way it faces — a unit vector, the
/// body's aim — and the hit points it still takes. A flat plate
/// [`PLATE_OUT`] in front of the body, [`PLATE_HALF`] either side of the
/// facing: a hostile bolt that crosses it from the front is stopped
/// there, its damage taken off the plate, and **bounced** — a friendly
/// bolt of the holder's leaves the plate along the bolt's way mirrored
/// in it, the angle out the angle in ([`Plate::bounce`]).
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Plate {
    pub who: usize,
    pub facing: Vec2,
    pub hp: f32,
}

/// How far in front of its holder's middle a Riot Shield's plate stands,
/// in room units (task 155): past the body's own [`HIT_RADIUS`], so a bolt
/// from the front meets the plate before the body.
pub const PLATE_OUT: f32 = 22.0;
/// How far the plate reaches either side of its middle, in room units:
/// a little wider than the body it covers.
pub const PLATE_HALF: f32 = 24.0;

impl Plate {
    /// Where a bolt flying from `from` to `to` crosses the plate of a
    /// holder standing at `body`, as a share of the way, coming in from
    /// the front; `None` for one that misses it, passes beside it or
    /// comes from behind.
    pub fn crossing(&self, body: Vec2, from: Vec2, to: Vec2) -> Option<f32> {
        let n = self.facing;
        let centre = body + n * PLATE_OUT;
        let before = (from - centre).dot(n);
        let after = (to - centre).dot(n);
        if before <= 0.0 || after > 0.0 {
            return None;
        }
        let t = before / (before - after);
        let at = from + (to - from) * t;
        ((at - centre).dot(n.perp()).abs() <= PLATE_HALF).then_some(t)
    }

    /// A bolt's way `dir` bounced off the plate: mirrored in its face,
    /// `dir − 2 (dir · n) n`, so it leaves at the angle it came in.
    pub fn bounce(&self, dir: Vec2) -> Vec2 {
        let n = self.facing;
        (dir - n * (2.0 * dir.dot(n))).normalize_or_zero()
    }
}

/// The fight, as the room keeps it: the enemies the world named, the bolts
/// flying, and the hits that landed since the world last asked.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Combat {
    /// Where the enemies stand and what they carry, index for index with
    /// whoever the world says they are; `None` for one that is down.
    /// Empty in peace.
    targets: Vec<Option<Target>>,
    /// **The machines' own targets** (feature 94), for the one fight
    /// that is not one room against another: a town the crew are
    /// defending, where the droids are in the residents' room *with* the
    /// people they came for. Empty everywhere else, and then the
    /// machines read [`Combat::targets`] like everybody else.
    ///
    /// The front [`Combat::machines_cross`] of it are the world's —
    /// across the seam, in the crew's room, shot at with a recorded
    /// [`Shot`] the way a hostile room's people shoot — and the rest are
    /// **this room's own bodies**, by index `i - machines_cross`, which
    /// a machine shoots at with a hostile bolt that flies here.
    #[cfg_attr(feature = "serde", serde(default))]
    machines: Vec<Option<Target>>,
    #[cfg_attr(feature = "serde", serde(default))]
    machines_cross: usize,
    pub bolts: Vec<Bolt>,
    sparks: Vec<Spark>,
    /// The Guardians' beams being swept here (feature 100).
    #[cfg_attr(feature = "serde", serde(default))]
    sweeps: Vec<Sweep>,
    /// The grenades in the air and lying with their fuses burning
    /// (feature 75), and the bursts lately lit. See `Game::burst`.
    pub grenades: Vec<Grenade>,
    blasts: Vec<Blast>,
    /// The odds each of this room's own bodies dodges a bolt in cover,
    /// by index — a soldier's *cover master*, [`DODGE_IN_COVER`] for
    /// anybody not named. Set every step with the skills.
    own_cover_dodge: Vec<f32>,
    /// The Riot Shields held up among this room's own bodies (task 155),
    /// said every step before it: a hostile bolt crossing one from the
    /// front is stopped and bounced back as the holder's own. Never read
    /// for a friendly bolt.
    #[cfg_attr(feature = "serde", serde(default))]
    plates: Vec<Plate>,
    /// What every plate stopped since the world last asked: the holder
    /// and the damage the bolt carried there, for the world to take off
    /// the shield.
    #[cfg_attr(feature = "serde", serde(default))]
    plate_blocks: Vec<(usize, f32)>,
    /// The share of an enemy's hit each of this room's own bodies sends
    /// back on whoever struck it, by index: a tank's Reflect Barrier
    /// (task 155), nought for everybody else. Said every step with the
    /// skills.
    #[cfg_attr(feature = "serde", serde(skip))]
    reflects: Vec<f32>,
    /// How wide a shield's front is against each of this room's own
    /// bodies' bolts and blows, as a cosine, by index: a relic's *Wide
    /// Angle Optics* narrows it (task 118). One past the end, or missing,
    /// is the Guardian's own [`balance::GUARDIAN_SHIELD_COS`]. Said every
    /// step with the skills.
    #[cfg_attr(feature = "serde", serde(default))]
    shield_fronts: Vec<f32>,
    /// Every friendly bolt that landed on a target, and every blow, for
    /// the world to carry to the body it belongs to.
    hits: Vec<Hit>,
    /// Every hostile bolt that landed on one of this room's own bodies,
    /// and every blow the world delivered. The game applies these itself,
    /// every step, and keeps a copy for the world to log.
    pub wounds_taken: Vec<Hit>,
    /// The shots this room's people took while its bodies were hostile —
    /// recorded here rather than flown, for the world to fire in the
    /// crew's room. See the module note.
    pub shots: Vec<Shot>,
    /// Every bolt that landed on a lamp since the game last asked: which
    /// lamp (`Sight::lamps`) and how hard, at the distance it had flown.
    /// Either side's — a lamp does not care whose bolt it was. The game
    /// takes it off the lamp (`Sight::damage_lamp`).
    lamp_hits: Vec<(usize, f32)>,
    /// Every bolt the sandbags stopped since the game last asked: the
    /// tile of low cover a body ducked behind (`Sight::cover_between`)
    /// and the damage the bolt carried there. Either side's. A bolt a
    /// body in cover dodges does not vanish — it is in the bags — and
    /// the world takes it off a laid deployable's health
    /// (`Game::take_cover_hits`); a sandbag *part* shrugs it off.
    cover_hits: Vec<((i32, i32), f32)>,
    /// Every bolt and blow a target's shield stopped since the world
    /// last asked: the target's index and the damage it carried there,
    /// unarmoured (the bolt's curve at the distance flown, its own
    /// factors). The world takes it off a Guardian's plate
    /// (`Droid::strike_plate`); a sealed core's shell is no plate and
    /// shrugs it off.
    #[cfg_attr(feature = "serde", serde(default))]
    plate_hits: Vec<(usize, f32)>,
    /// What was heard: every bolt fired here, every one that landed, and
    /// every blow that did. See `crate::cue`. A hostile room's recorded
    /// `shots` say nothing — they are heard where they are flown.
    pub cues: Vec<Cued>,
    /// How long since anything was fired or landed here, in seconds at
    /// 1x, either side's: a bolt flown or recorded, a blow swung or
    /// carried in. What the room asks before anybody goes to doctor a
    /// crewmate (`Game::calm`). Aged by [`Combat::age`], every step.
    lull: f32,
    rng: Rng,
    /// The world's Weak Spot stream (task 124), lent for a step and taken
    /// back ([`Combat::lend_crit_rng`]): every critical roll is drawn off
    /// it and nothing else is, so a crit never moves a roll of the fight.
    /// `None` outside a step, and in every room but the crew's.
    #[cfg_attr(feature = "serde", serde(skip))]
    crit_rng: Option<Rng>,
    /// Each own body's chance a weapon hit it lands is critical, by
    /// index (`Skill::crit_chance`), said every step with the skills.
    #[cfg_attr(feature = "serde", serde(skip))]
    crit_chances: Vec<f32>,
    /// The passing lights of the fight (feature 98): the muzzles, the
    /// flashes, the scorches, the beams, the cuts, the parts struck and
    /// the machines bursting — drawing only, on the host's own clock, and
    /// never saved. See `crate::fx`.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub fx: Fx,
}

impl Combat {
    pub fn new(seed: u64) -> Combat {
        Combat {
            fx: Fx::default(),
            targets: Vec::new(),
            machines: Vec::new(),
            machines_cross: 0,
            bolts: Vec::new(),
            sparks: Vec::new(),
            sweeps: Vec::new(),
            grenades: Vec::new(),
            blasts: Vec::new(),
            own_cover_dodge: Vec::new(),
            plates: Vec::new(),
            plate_blocks: Vec::new(),
            reflects: Vec::new(),
            shield_fronts: Vec::new(),
            hits: Vec::new(),
            wounds_taken: Vec::new(),
            shots: Vec::new(),
            lamp_hits: Vec::new(),
            cover_hits: Vec::new(),
            plate_hits: Vec::new(),
            cues: Vec::new(),
            // A fresh room has been quiet for ever.
            lull: f32::MAX,
            // Its own stream: a fight must not re-roll the room.
            rng: Rng::new(seed ^ 0xC0B_A7),
            crit_rng: None,
            crit_chances: Vec::new(),
        }
    }

    /// The enemies: where each stands and what it carries. Nobody is
    /// peeking until [`Combat::set_peeking`] says so.
    pub fn set_targets(&mut self, targets: Vec<Option<(Vec2, Weapon)>>) {
        self.targets = targets
            .into_iter()
            .map(|t| {
                t.map(|(at, weapon)| Target {
                    at,
                    weapon,
                    peeking: false,
                    stale: false,
                    dodge: 0.0,
                    taunting: 0.0,
                    magnet: false,
                    taunt_order: 0,
                    shield: None,
                })
            })
            .collect();
    }

    /// The odds each target dodges a bolt for its armour, index for
    /// index; one past the end, or missing, is nought.
    pub fn set_dodge(&mut self, dodge: &[f32]) {
        for (i, target) in self.targets.iter_mut().enumerate() {
            if let Some(t) = target {
                t.dodge = dodge.get(i).copied().unwrap_or(0.0);
            }
        }
    }

    /// Which of the targets are beliefs rather than sightings, index for
    /// index; one past the end, or missing, is a sighting. Nobody aims
    /// at, locks with or fires at a stale one.
    pub fn set_stale(&mut self, stale: &[bool]) {
        for (i, target) in self.targets.iter_mut().enumerate() {
            if let Some(t) = target {
                t.stale = stale.get(i).copied().unwrap_or(false);
            }
        }
    }

    /// Which of the targets a taunt is running on, index for index like
    /// [`Combat::set_peeking`] (feature 77): each one's [`Taunt`]. One
    /// past the end, or missing, is no taunt.
    pub fn set_taunting(&mut self, taunts: &[Taunt]) {
        Taunt::put_on(&mut self.targets, taunts);
    }

    /// The same for the machines' own list (task 139), where the world
    /// gave them one — a town the crew are defending — index for index
    /// with it: the crew across the seam first, as `set_machine_targets`
    /// was handed them.
    pub fn set_machine_taunting(&mut self, taunts: &[Taunt]) {
        Taunt::put_on(&mut self.machines, taunts);
    }

    /// The Riot Shields held up in this room (task 155): which of the
    /// bodies a hostile bolt is looked for among holds one, the way it
    /// faces and what it still takes. Said every step; an empty list is
    /// nobody.
    pub fn set_plates(&mut self, plates: Vec<Plate>) {
        self.plates = plates;
    }

    /// What each of this room's own bodies sends back of an enemy's hit
    /// on it, by index (task 155, `Skill::reflect`).
    pub fn set_reflects(&mut self, reflects: Vec<f32>) {
        self.reflects = reflects;
    }

    /// The Reflect Barrier's hit for an enemy's `hit` on one of this
    /// room's own, if that body sends any back and the shooter is known:
    /// what it sends, as a hit of the body's on the shooter.
    pub fn reflected_hit(&self, hit: &Hit, shooter: Option<usize>) -> Option<Hit> {
        reflected(&self.reflects, hit, shooter)
    }

    /// Put a hit on the hits the world carries across, as a bolt's would
    /// be.
    pub fn land(&mut self, hit: Hit) {
        self.hits.push(hit);
    }

    /// The plates as they stand.
    pub fn plates(&self) -> &[Plate] {
        &self.plates
    }

    /// What the plates stopped since last asked: the holder and the
    /// damage each bolt carried there.
    pub fn take_plate_blocks(&mut self) -> Vec<(usize, f32)> {
        std::mem::take(&mut self.plate_blocks)
    }

    /// How wide a shield's front is against each own body's shots, by
    /// index (task 118): see [`Combat::shield_fronts`].
    pub fn set_shield_fronts(&mut self, fronts: Vec<f32>) {
        self.shield_fronts = fronts;
    }

    /// Lend the room the world's crit stream for a step (task 124).
    pub fn lend_crit_rng(&mut self, rng: Rng) {
        self.crit_rng = Some(rng);
    }

    /// And take it back, drawn on.
    pub fn take_crit_rng(&mut self) -> Option<Rng> {
        self.crit_rng.take()
    }

    /// Each own body's crit chance, by index (`Skill::crit_chance`).
    pub fn set_crit_chances(&mut self, chances: Vec<f32>) {
        self.crit_chances = chances;
    }

    /// Whether a hit by own body `by` is critical: rolled off the lent
    /// stream only for a body with a chance, so a crew with none draws
    /// nothing at all.
    fn roll_crit(&mut self, by: Option<usize>) -> bool {
        roll_crit_off(&mut self.crit_rng, &self.crit_chances, by)
    }

    /// The front a shield turns against a shot of `by`'s.
    fn shield_front(&self, by: Option<usize>) -> f32 {
        shield_front_of(&self.shield_fronts, by)
    }

    /// Which of the targets carry a shield and which way it faces
    /// (feature 100), index for index like [`Combat::set_peeking`]; one
    /// past the end, or missing, carries none.
    pub fn set_shields(&mut self, shields: &[Option<Vec2>]) {
        for (i, target) in self.targets.iter_mut().enumerate() {
            if let Some(t) = target {
                t.shield = shields.get(i).copied().flatten();
            }
        }
    }

    /// Which of the targets are peeking from cover, index for index; one
    /// past the end, or missing, is not.
    pub fn set_peeking(&mut self, peeking: &[bool]) {
        for (i, target) in self.targets.iter_mut().enumerate() {
            if let Some(t) = target {
                t.peeking = peeking.get(i).copied().unwrap_or(false);
            }
        }
    }

    pub fn targets(&self) -> &[Option<Target>] {
        &self.targets
    }

    /// The machines' own targets (feature 94): the world's across the
    /// seam first, then this room's own bodies. An empty list is the
    /// ordinary case — one room against another — and then the machines
    /// shoot at [`Combat::targets`] like everybody else.
    pub fn set_machine_targets(&mut self, targets: Vec<Option<(Vec2, Weapon)>>, cross: usize) {
        self.machines_cross = cross.min(targets.len());
        self.machines = targets
            .into_iter()
            .map(|t| {
                t.map(|(at, weapon)| Target {
                    at,
                    weapon,
                    peeking: false,
                    stale: false,
                    dodge: 0.0,
                    taunting: 0.0,
                    magnet: false,
                    taunt_order: 0,
                    shield: None,
                })
            })
            .collect();
    }

    /// Which of the machines' targets are beliefs rather than sightings
    /// — [`Combat::set_stale`] for the other list.
    pub fn set_machine_stale(&mut self, stale: &[bool]) {
        for (i, target) in self.machines.iter_mut().enumerate() {
            if let Some(t) = target {
                t.stale = stale.get(i).copied().unwrap_or(false);
            }
        }
    }

    /// What a machine aims at: its own list where the world has given it
    /// one, else the room's.
    pub fn machine_targets(&self) -> &[Option<Target>] {
        if self.machines.is_empty() {
            &self.targets
        } else {
            &self.machines
        }
    }

    /// How many of [`Combat::machine_targets`] are across the seam: an
    /// index below it is the world's to deliver a shot to, and one at or
    /// above it is a body of this room, `i - machine_cross()`.
    pub fn machine_cross(&self) -> usize {
        if self.machines.is_empty() {
            self.targets.len()
        } else {
            self.machines_cross
        }
    }

    /// Where the targets stand, for the tactics.
    #[allow(dead_code)]
    pub fn target_positions(&self) -> Vec<Option<Vec2>> {
        self.targets.iter().map(|t| t.map(|t| t.at)).collect()
    }

    pub fn take_hits(&mut self) -> Vec<Hit> {
        std::mem::take(&mut self.hits)
    }

    pub fn take_shots(&mut self) -> Vec<Shot> {
        std::mem::take(&mut self.shots)
    }

    /// The bolts that landed on a lamp since last asked: the lamp and
    /// the damage. See `lamp_hits`.
    pub fn take_lamp_hits(&mut self) -> Vec<(usize, f32)> {
        std::mem::take(&mut self.lamp_hits)
    }

    /// The bolts the sandbags stopped since last asked: the tile of cover
    /// and the damage. See `cover_hits`.
    pub fn take_cover_hits(&mut self) -> Vec<((i32, i32), f32)> {
        std::mem::take(&mut self.cover_hits)
    }

    /// Every bolt and blow a shield stopped since the last call: the
    /// target and the damage. See `plate_hits`.
    pub fn take_plate_hits(&mut self) -> Vec<(usize, f32)> {
        std::mem::take(&mut self.plate_hits)
    }

    /// A hit landed on a target as a bolt or a blow would land it, for
    /// the tests (task 124): what the world then carries across.
    #[allow(dead_code)]
    pub fn land_hit_for_probe(&mut self, hit: Hit) {
        self.hits.push(hit);
    }

    /// A bolt stopped by the bags on `tile`, for the tests.
    #[allow(dead_code)]
    pub fn cover_hit_for_probe(&mut self, tile: (i32, i32), damage: f32) {
        self.cover_hits.push((tile, damage));
    }

    /// The odds each of this room's own bodies dodges a bolt in cover,
    /// by index: the soldier's *cover master*, [`DODGE_IN_COVER`] for
    /// anybody not named.
    pub fn set_own_cover_dodge(&mut self, odds: Vec<f32>) {
        self.own_cover_dodge = odds;
    }

    /// A grenade thrown (feature 75): by crew member `by` from `from` at
    /// `at`, bursting `fuse` seconds on with `radius` and `damage` at the
    /// centre. Nothing is rolled now; the burst is `Game::burst`, when
    /// the fuse runs out.
    pub fn throw(&mut self, by: usize, from: Vec2, at: Vec2, fuse: f32, radius: f32, damage: f32) {
        self.lull = 0.0;
        self.grenades.push(Grenade {
            by,
            from,
            at,
            left: fuse,
            fuse,
            radius,
            damage,
            stun: 0.0,
            expose: false,
            shot: false,
            laid: false,
            satchel: false,
        });
        self.cues.push(Cued {
            cue: Cue::Throw,
            at: from,
        });
    }

    /// A soldier's Stun Shot fired (October 2026): by crew member `by`
    /// from `from` at `at`, flying `flight` seconds and bursting there with
    /// `radius`, `damage` to every target in it and a stun of `stun`
    /// seconds. Heard as the gun in its hand.
    #[allow(clippy::too_many_arguments)]
    pub fn fire_stun_shot(
        &mut self,
        by: usize,
        from: Vec2,
        at: Vec2,
        flight: f32,
        radius: f32,
        damage: f32,
        stun: f32,
        weapon: Weapon,
    ) {
        self.lull = 0.0;
        self.grenades.push(Grenade {
            by,
            from,
            at,
            left: flight,
            fuse: flight,
            radius,
            damage,
            stun: stun.max(f32::MIN_POSITIVE),
            expose: false,
            shot: true,
            laid: false,
            satchel: false,
        });
        self.fx
            .muzzle(from, (at - from).normalize_or_zero(), weapon, false);
        self.cues.push(Cued {
            cue: Cue::Shot {
                weapon: weapon.kind,
                hostile: false,
                by: Some(by),
            },
            at: from,
        });
    }

    /// An engineer's satchel charge lobbed (task 154): by crew member `by`
    /// from `from` at `at`, the grenade's flight and no fuse after it — it
    /// lands rather than bursts (`Game::take_satchels_landed`).
    pub fn lob_satchel(&mut self, by: usize, from: Vec2, at: Vec2) {
        self.throw(by, from, at, GRENADE_FLIGHT, 0.0, 0.0);
        if let Some(g) = self.grenades.last_mut() {
            g.satchel = true;
        }
    }

    /// An engineer's mine or satchel set off where it lies (task 154): by
    /// crew member `by` at `at`, bursting on the next tick with `radius`
    /// and `damage` at the centre, on the targets alone.
    pub fn detonate(&mut self, by: usize, at: Vec2, radius: f32, damage: f32) {
        self.lull = 0.0;
        self.grenades.push(Grenade {
            by,
            from: at,
            at,
            left: 0.0,
            fuse: 0.0,
            radius,
            damage,
            stun: 0.0,
            expose: false,
            shot: false,
            laid: true,
            satchel: false,
        });
    }

    /// The fuses burnt down by `dt`: every grenade whose fuse ran out,
    /// taken off the list in the order thrown, for the game to burst.
    /// The flash of each is lit here.
    pub fn tick_grenades(&mut self, dt: f32) -> Vec<Grenade> {
        for b in &mut self.blasts {
            b.age += dt;
        }
        self.blasts.retain(|b| b.age < BLAST_LIFE);
        let mut burst = Vec::new();
        self.grenades.retain_mut(|g| {
            g.left -= dt;
            if g.left > 0.0 {
                return true;
            }
            burst.push(*g);
            false
        });
        // A satchel landing is no burst: nothing lit, nothing heard.
        for g in burst.iter().filter(|g| !g.satchel) {
            self.blasts.push(Blast {
                pos: g.at,
                radius: g.radius,
                age: 0.0,
                emp: g.stun > 0.0,
            });
            // The particles over the flash: the host's, never read back.
            // A Stun Shot is both: it burns and it stuns.
            if g.shot {
                self.fx.explosion(g.at, g.radius * 0.6);
                self.fx.emp(g.at, g.radius);
            } else if g.stun > 0.0 {
                self.fx.emp(g.at, g.radius);
            } else {
                self.fx.explosion(g.at, g.radius);
            }
            self.cues.push(Cued {
                cue: if g.stun > 0.0 && !g.shot {
                    Cue::EmpBurst
                } else {
                    Cue::Burst
                },
                at: g.at,
            });
            self.lull = 0.0;
        }
        burst
    }

    /// A grenade's hit on a target, rolled now: the part off the combat
    /// stream, the damage as given, a blast for the blood.
    pub fn blast(&mut self, who: usize, damage: f32, by: Option<usize>) -> Hit {
        let roll = self.rng.unit();
        Hit {
            who,
            damage,
            cut: false,
            by,
            blast: true,
            roll,
            flat: 0.0,
            crit: false,
            strips: 0.0,
        }
    }

    /// A grenade's hit on one of the targets, straight onto the hits for
    /// the world to carry to the body.
    pub fn blast_target(&mut self, target: usize, damage: f32, by: Option<usize>) {
        let hit = self.blast(target, damage, by);
        self.hits.push(hit);
        if let Some(at) = self.targets.get(target).copied().flatten().map(|t| t.at) {
            self.cues.push(Cued {
                cue: Cue::Impact { on_crew: false },
                at,
            });
        }
    }

    /// What flies leaves its wake for a frame of the host's (`Game::fade`):
    /// motes behind every bolt along the last stretch of its flight, and a
    /// spark off every lit fuse lying on the deck. Particles only.
    pub fn wakes(&mut self) {
        if !self.fx.is_on() {
            return;
        }
        for bolt in &self.bolts {
            let dir = bolt.vel.normalize_or_zero();
            let flown = (bolt.pos - bolt.fired_from).len();
            let tail = bolt.pos - dir * flown.min(WAKE_LENGTH);
            self.fx.trail(tail, bolt.pos, bolt.weapon, bolt.hostile);
        }
        for g in &self.grenades {
            if !g.shot && g.fuse - g.left >= GRENADE_FLIGHT {
                self.fx.fuse(g.at);
            }
        }
    }

    /// Whether anything is lit or on its way: a grenade thrown counts.
    pub fn grenades_out(&self) -> bool {
        !self.grenades.is_empty() || !self.blasts.is_empty()
    }

    /// How long since the last shot or blow here, either side's, in
    /// seconds at 1x.
    pub fn lull(&self) -> f32 {
        self.lull
    }

    /// A step of the lull. Apart from [`Combat::step`], which only runs
    /// while something is in the air or somebody is a target.
    pub fn age(&mut self, dt: f32) {
        self.lull = (self.lull + dt).min(f32::MAX);
    }

    /// Something landed here that went through none of the methods
    /// above — a blow the world carried onto a sentry: the lull is over.
    pub fn lull_break(&mut self) {
        self.lull = 0.0;
    }

    /// A shot taken but not flown: what a hostile room's people do
    /// instead of firing, so the bolt flies where the crew are. `moving`
    /// while the shooter walks, for the odds where it is fired.
    pub fn shoot(&mut self, from: Vec2, at: Vec2, weapon: Weapon, moving: bool) {
        let stats = weapon.stats();
        self.lull = 0.0;
        self.shots.push(Shot {
            from,
            at,
            weapon,
            melee: false,
            damage: stats.damage,
            cut: false,
            moving,
            sweep: None,
            pace: 1.0,
            shooter: None,
        });
    }

    /// Sign the shot recorded last with who took it: its body's index in
    /// this room, Bims first (task 155, [`Shot::shooter`]).
    pub fn sign_last_shot(&mut self, shooter: usize) {
        if let Some(shot) = self.shots.last_mut() {
            shot.shooter = Some(shooter);
        }
    }

    /// Fly a hostile bolt here signed with who fired it, by its index on
    /// this room's targets ([`Bolt::shooter`]): [`Combat::fire`] with the
    /// shooter noted on the bolt it put in the air, if it put one.
    pub fn fire_hostile(
        &mut self,
        from: Vec2,
        at: Vec2,
        weapon: Weapon,
        moving: bool,
        shooter: Option<usize>,
    ) {
        let before = self.bolts.len();
        self.fire(from, at, weapon, true, moving);
        if self.bolts.len() > before
            && let Some(bolt) = self.bolts.last_mut()
        {
            bolt.shooter = shooter;
        }
    }

    /// A Guardian's Sweeper let go across the seam (feature 100): a
    /// recorded [`Shot`] from the lens at `from`, the beam's arc from the
    /// aim at `start` round to the aim at `end`, `damage` to each body it
    /// crosses. The world lays it as a [`Sweep`] in the crew's room.
    pub fn shoot_sweep(
        &mut self,
        from: Vec2,
        start: Vec2,
        end: Vec2,
        weapon: Weapon,
        damage: f32,
        pace: f32,
    ) {
        self.lull = 0.0;
        self.shots.push(Shot {
            from,
            at: start,
            weapon,
            melee: false,
            damage,
            cut: false,
            moving: false,
            sweep: Some(end),
            pace,
            shooter: None,
        });
    }

    /// A Sweeper's beam laid in **this** room (feature 100): from the lens
    /// at `from`, turning from the aim at `start` round to the aim at
    /// `end`, the reach the distance to `start`, crossing this room's own
    /// bodies — the crew's, flown by the world off a recorded
    /// [`Shot`], or a town's people in their own room. `drawn` is whether
    /// this room draws it: one room draws a beam and no other, so a
    /// town's own sweep, laid beside the one the crew's room flies, is
    /// not.
    pub fn sweep(
        &mut self,
        from: Vec2,
        start: Vec2,
        end: Vec2,
        weapon: Weapon,
        damage: f32,
        drawn: bool,
        pace: f32,
    ) {
        let out = start - from;
        let reach = out.len();
        let dir = out.normalize_or_zero();
        if dir == Vec2::ZERO {
            return;
        }
        // Which way round it turns is a cross product, never an angle.
        let sin = if out.perp_dot(end - from) >= 0.0 {
            SWEEP_STEP_SIN
        } else {
            -SWEEP_STEP_SIN
        };
        self.lull = 0.0;
        self.cues.push(Cued {
            cue: Cue::Shot {
                weapon: weapon.kind,
                hostile: true,
                by: None,
            },
            at: from,
        });
        self.sweeps.push(Sweep {
            from,
            dir,
            sin,
            reach,
            weapon,
            damage,
            drawn,
            elapsed: 0.0,
            done: 0,
            rolled: Vec::new(),
            end: from,
            walled: false,
            pace: pace.max(0.01),
            shooter: None,
        });
    }

    /// Sign the beam laid last with who swept it, by its index on this
    /// room's targets ([`Sweep::shooter`], task 155).
    pub fn sign_last_sweep(&mut self, shooter: Option<usize>) {
        if let Some(sweep) = self.sweeps.last_mut() {
            sweep.shooter = shooter;
        }
    }

    /// The beams being swept in this room: for the picture and the tests.
    pub fn sweeps(&self) -> &[Sweep] {
        &self.sweeps
    }

    /// Every beam on by `dt` (feature 100): each fixed sub-step whose time
    /// has come is resolved in turn — the beam laid along its direction out
    /// to its reach and stopped at the first wall or shut door, every body
    /// of this room's own within [`HIT_RADIUS`] of it and not already
    /// rolled this sweep rolled once, and the direction turned on by the
    /// written-out sub-step. A body in cover and not peeking is not hit —
    /// the beam goes over the bags; one peeking dodges as a bolt would; a
    /// tier-three body's own dodge is rolled after. What lands is
    /// a hostile [`Hit`] on `wounds_taken`, applied by the room — so a
    /// surge takes it whole, as it does a bolt. It never looks for a
    /// target: a machine is never among a room's own bodies, so the beam
    /// hurts no machine.
    fn step_sweeps(&mut self, dt: f32, sight: &Sight, bodies: &[Option<(Vec2, bool, f32)>]) {
        if self.sweeps.is_empty() {
            return;
        }
        let own_cover_dodge = &self.own_cover_dodge;
        let reflects = &self.reflects;
        let rng = &mut self.rng;
        let fx = &mut self.fx;
        let mut taken: Vec<Hit> = Vec::new();
        let mut sent_back: Vec<Hit> = Vec::new();
        let mut heard: Vec<Cued> = Vec::new();
        for s in &mut self.sweeps {
            // An overloaded core sweeps faster (feature 108): the same
            // sub-steps, closer together.
            let every = balance::SWEEPER_SWEEP / SWEEP_STEPS as f32 / s.pace;
            s.elapsed += dt;
            while s.done <= SWEEP_STEPS && s.done as f32 * every <= s.elapsed + 1e-5 {
                let far = s.from + s.dir * s.reach;
                let (end, walled) = match sight.first_opaque_along(s.from, far) {
                    Some(wall) => (wall, true),
                    None => (far, false),
                };
                s.end = end;
                s.walled = walled;
                if walled && s.drawn {
                    fx.burn(end, s.dir);
                }
                for (i, body) in bodies.iter().enumerate() {
                    let Some((at, peeking, dodge)) = *body else {
                        continue;
                    };
                    if s.rolled.contains(&i) || off_segment(s.from, end, at) > HIT_RADIUS {
                        continue;
                    }
                    s.rolled.push(i);
                    // Behind bags and not leaning out: the beam goes over.
                    if !peeking && sight.cover_between(at, s.from).is_some() {
                        continue;
                    }
                    let odds = own_cover_dodge.get(i).copied().unwrap_or(DODGE_IN_COVER);
                    if peeking && rng.chance(odds) {
                        continue;
                    }
                    if dodge > 0.0 && rng.chance(dodge) {
                        continue;
                    }
                    let roll = rng.unit();
                    let hit = Hit {
                        who: i,
                        damage: s.damage,
                        cut: false,
                        by: None,
                        blast: false,
                        roll,
                        flat: 0.0,
                        crit: false,
                        strips: 0.0,
                    };
                    // A Reflect Barrier sends its share back (task 155).
                    sent_back.extend(reflected(reflects, &hit, s.shooter));
                    taken.push(hit);
                    heard.push(Cued {
                        cue: Cue::Impact { on_crew: true },
                        at,
                    });
                    fx.landed(s.from, at, s.dir, s.weapon, true, true);
                }
                s.dir = s.dir.rotate_by(SWEEP_STEP_COS, s.sin).normalize_or_zero();
                s.done += 1;
            }
        }
        self.sweeps.retain(|s| s.done <= SWEEP_STEPS);
        self.wounds_taken.extend(taken);
        self.hits.extend(sent_back);
        self.cues.extend(heard);
    }

    /// The nearest enemy a shooter standing at `from` can see, within the
    /// weapon's range: which one, the eye it is seen from — the body, or the
    /// peek beside a wall — and where it stands.
    ///
    /// **A taunt is him and nobody else** (features 77 and 139): a target
    /// with one running on it, seen, and within the taunt's own radius of
    /// the shooter, is the only one it picks — shot at in reach, and
    /// nobody shot at beyond it; the most recent taunt of two, the nearer
    /// of two as recent.
    pub fn aim(
        &self,
        sight: &Sight,
        from: Vec2,
        stats: &WeaponStats,
    ) -> Option<(usize, Vec2, Vec2)> {
        Combat::aim_among(&self.targets, sight, from, stats)
    }

    /// [`Combat::aim`] at one target and no other — a player's attack
    /// order, task 126: its index, the eye and where it is aimed at if
    /// that one is up, seen and in reach, and nothing otherwise, so
    /// nobody else is fired at in its place. A sealed core may be aimed
    /// at this way.
    pub fn aim_only(
        &self,
        sight: &Sight,
        from: Vec2,
        stats: &WeaponStats,
        which: usize,
    ) -> Option<(usize, Vec2, Vec2)> {
        let t = self.targets.get(which).copied().flatten()?;
        if t.stale || (t.at - from).len() > stats.reach() {
            return None;
        }
        sight.sees_from(from, t.at).map(|eye| (which, eye, t.at))
    }

    /// [`Combat::aim`] over a target list of the caller's — what
    /// a machine aims with, since it has a list of its own in a town the
    /// crew are defending (feature 94).
    pub fn aim_among(
        targets: &[Option<Target>],
        sight: &Sight,
        from: Vec2,
        stats: &WeaponStats,
    ) -> Option<(usize, Vec2, Vec2)> {
        Combat::aim_seeing(targets, from, stats, |a, b| sight.sees_from(a, b))
    }

    /// [`Combat::aim`] for a sentry, whose sensor the dark does not stop
    /// (`Sight::sees_from_in_the_dark`): a machine in reach with a clear
    /// line to it is fired at, lamps or no lamps.
    pub fn aim_in_the_dark(
        &self,
        sight: &Sight,
        from: Vec2,
        stats: &WeaponStats,
    ) -> Option<(usize, Vec2, Vec2)> {
        Combat::aim_seeing(&self.targets, from, stats, |a, b| {
            sight.sees_from_in_the_dark(a, b)
        })
    }

    /// The aim's one rule, with `sees` saying which eye (if any) sees a
    /// point from `from`.
    fn aim_seeing(
        targets: &[Option<Target>],
        from: Vec2,
        stats: &WeaponStats,
        sees: impl Fn(Vec2, Vec2) -> Option<Vec2>,
    ) -> Option<(usize, Vec2, Vec2)> {
        let reach = stats.reach();
        // **A taunt is him and nobody else** (task 139): a target taunting
        // within its own radius of the shooter that the shooter can see —
        // the most recent of two, the nearer of two as recent — is the
        // only one it may pick. In the weapon's reach it is shot at;
        // beyond it, nobody is.
        let mut forced: Option<((u32, f32), usize, Vec2, Vec2)> = None;
        for (i, target) in targets.iter().enumerate() {
            let Some(t) = target.filter(|t| !t.stale && t.taunting > 0.0) else {
                continue;
            };
            let d = (t.at - from).len();
            if d > t.taunting {
                continue;
            }
            let key = (t.taunt_order, -d);
            if forced.is_some_and(|f| f.0 >= key) {
                continue;
            }
            if let Some(eye) = sees(from, t.at) {
                forced = Some((key, i, eye, t.at));
            }
        }
        if let Some(((_, d), i, eye, at)) = forced {
            return (-d <= reach).then_some((i, eye, at));
        }
        // Otherwise the nearest.
        let mut best: Option<(f32, usize, Vec2, Vec2)> = None;
        for (i, target) in targets.iter().enumerate() {
            let Some(t) = target.filter(|t| !t.stale) else {
                continue;
            };
            // A sealed core is nothing to spend a shot on (feature 108):
            // nobody aims at one of their own accord, though a player's
            // attack order still may (`aim_only`).
            if t.sealed() {
                continue;
            }
            let at = t.at;
            let key = (at - from).len();
            if key > reach {
                continue;
            }
            if best.is_some_and(|b| b.0 <= key) {
                continue;
            }
            if let Some(eye) = sees(from, at) {
                best = Some((key, i, eye, at));
            }
        }
        best.map(|(_, i, eye, at)| (i, eye, at))
    }

    /// Whether a body at `from` can see any target at all, at any range —
    /// what tells a crew member under the alarm to act on its own rather
    /// than keep to the player's side.
    pub fn sees_any(&self, sight: &Sight, from: Vec2) -> bool {
        self.targets
            .iter()
            .flatten()
            .any(|t| !t.stale && sight.sees_from(from, t.at).is_some())
    }

    /// The enemy a body at `from` is in a melee with, if any: the nearest
    /// target within [`MELEE_RANGE`] that it can see — any of them for a
    /// body with a blade, and only one *carrying* a blade for a body with
    /// a gun, since a gunner is locked by a blade at its throat and not by
    /// a pistol beside it. See the module note.
    pub fn melee_with(&self, sight: &Sight, from: Vec2, own: &WeaponStats) -> Option<usize> {
        Combat::melee_among(&self.targets, sight, from, own)
    }

    /// [`Combat::melee_with`] over a target list of the caller's.
    pub fn melee_among(
        targets: &[Option<Target>],
        sight: &Sight,
        from: Vec2,
        own: &WeaponStats,
    ) -> Option<usize> {
        let reach = MELEE_RANGE * TILE;
        let mut best: Option<(f32, usize)> = None;
        for (i, target) in targets.iter().enumerate() {
            let Some(t) = target.filter(|t| !t.stale && !t.sealed()) else {
                continue;
            };
            if !own.melee && !t.weapon.stats().melee {
                continue;
            }
            let d = (t.at - from).len();
            if d > reach || best.is_some_and(|b| b.0 <= d) {
                continue;
            }
            if sight.sees_from(from, t.at).is_some() {
                best = Some((d, i));
            }
        }
        best.map(|(_, i)| i)
    }

    /// Whether the target with that index is still within a blade's reach
    /// of a body at `from`: what a [`Blow`] asks when its swing ends.
    pub fn within_reach(&self, from: Vec2, target: usize) -> bool {
        Combat::within_reach_among(&self.targets, from, target)
    }

    /// [`Combat::within_reach`] over a target list of the caller's.
    pub fn within_reach_among(targets: &[Option<Target>], from: Vec2, target: usize) -> bool {
        targets
            .get(target)
            .copied()
            .flatten()
            .is_some_and(|t| (t.at - from).len() <= MELEE_RANGE * TILE)
    }

    /// A shot from `from` at a body at `at`. Whether it will hit is rolled
    /// now, against the distance — and at half the odds
    /// ([`WALKING_ACCURACY`]) when the shooter is `moving` — and a miss
    /// is aimed wide. The damage is not rolled now: it is the weapon's at
    /// the distance the bolt has flown when it lands.
    pub fn fire(&mut self, from: Vec2, at: Vec2, weapon: Weapon, hostile: bool, moving: bool) {
        self.fire_as(from, at, weapon, hostile, moving, &Skill::NONE, None);
    }

    /// [`Combat::fire`] by a shooter with a [`Skill`] — a soldier's, or
    /// [`Skill::NONE`] — and, for a Bim's own bolt, whose it is. The one
    /// hit calculation: the odds are the weapon's through the skill at
    /// the distance, times the skill's walking odds on the move, and the
    /// bolt carries the point-blank factor to where it lands.
    pub fn fire_as(
        &mut self,
        from: Vec2,
        at: Vec2,
        weapon: Weapon,
        hostile: bool,
        moving: bool,
        skill: &Skill,
        by: Option<usize>,
    ) {
        let stats = skill.stats(weapon);
        self.lull = 0.0;
        let to = at - from;
        let tiles = to.len() / TILE;
        let hits = self
            .rng
            .chance(stats.hit_chance(tiles) * if moving { skill.walking } else { 1.0 });
        let aim = if hits {
            at
        } else {
            let side = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
            at + to.normalize_or_zero().perp() * (MISS_BY * side)
        };
        let dir = (aim - from).normalize_or_zero();
        if dir == Vec2::ZERO {
            return;
        }
        self.loose(from, dir, weapon, hostile, skill, by);
    }

    /// A shot its player aimed (task 144): along `angle` from `from`,
    /// nobody picked, and what is struck is whatever the bolt reaches
    /// first. The odds are not a hit rolled but how straight it flies:
    /// it strays up to [`balance::AIM_SPREAD`] times what the near odds
    /// (through the skill, and the walking odds on the move) fall short
    /// of one, either side, off the combat stream. A player's own Bim
    /// fires with [`Skill::sure`], so its stray is nought and the bolt
    /// flies exactly along the heading; the roll is drawn all the same.
    pub fn fire_along(
        &mut self,
        from: Vec2,
        angle: f32,
        weapon: Weapon,
        moving: bool,
        skill: &Skill,
        by: Option<usize>,
    ) {
        let stats = skill.stats(weapon);
        self.lull = 0.0;
        let odds = stats.accuracy.min(1.0) * if moving { skill.walking } else { 1.0 };
        let stray = self.rng.signed() * (1.0 - odds).max(0.0) * balance::AIM_SPREAD;
        self.loose(
            from,
            Vec2::from_angle(angle + stray),
            weapon,
            false,
            skill,
            by,
        );
    }

    /// A bolt off along `dir` (a unit vector): heard, its muzzle lit for
    /// our own side, and put in the air at the weapon's pace and reach.
    fn loose(
        &mut self,
        from: Vec2,
        dir: Vec2,
        weapon: Weapon,
        hostile: bool,
        skill: &Skill,
        by: Option<usize>,
    ) {
        let stats = skill.stats(weapon);
        self.cues.push(Cued {
            cue: Cue::Shot {
                weapon: weapon.kind,
                hostile,
                by,
            },
            at: from,
        });
        // The muzzle glows where the shot left — for our own side. A
        // hostile shooter lit its own muzzle where its body stands
        // (`Game::lit_muzzle`): mostly in another room, where `from` here
        // is only the seam's point for it, and a machine's shot leaves
        // from its middle rather than from its gun.
        if !hostile {
            self.fx.muzzle(from, dir, weapon, false);
        }
        self.bolts.push(Bolt {
            pos: from,
            vel: dir * stats.pace(),
            left: stats.reach(),
            weapon,
            fired_from: from,
            hostile,
            by,
            point_blank: skill.point_blank,
            damage: skill.damage,
            range: skill.range,
            dodged: None,
            struck: [None; balance::LANCE_PIERCE],
            shooter: None,
        });
    }

    /// A blow in a melee, from a body at `from` on the target with that
    /// index: `damage` on a part rolled now, a cut or not. In the crew's
    /// room it is a [`Hit`] straight onto the hits, for the world to carry
    /// to the enemy's body; in a hostile room (`as_shot`) it is a melee
    /// [`Shot`] for the world to deliver to the crew member. Nothing
    /// flies either way.
    #[allow(clippy::too_many_arguments)]
    pub fn brawl(
        &mut self,
        from: Vec2,
        target: usize,
        weapon: Weapon,
        damage: f32,
        cut: bool,
        as_shot: bool,
        by: Option<usize>,
        flat: f32,
    ) {
        let Some(at) = self.targets.get(target).copied().flatten().map(|t| t.at) else {
            return;
        };
        self.brawl_at(from, target, at, weapon, damage, cut, as_shot, by, flat);
    }

    /// [`Combat::brawl`] with the target's position handed in rather
    /// than looked up, for a body swinging at a list of its own — a
    /// machine in a town the crew are defending (feature 94).
    #[allow(clippy::too_many_arguments)]
    pub fn brawl_at(
        &mut self,
        from: Vec2,
        target: usize,
        at: Vec2,
        weapon: Weapon,
        damage: f32,
        cut: bool,
        as_shot: bool,
        by: Option<usize>,
        flat: f32,
    ) {
        self.lull = 0.0;
        if as_shot {
            self.shots.push(Shot {
                from,
                at,
                weapon,
                melee: true,
                damage,
                cut,
                moving: false,
                sweep: None,
                pace: 1.0,
                shooter: None,
            });
        } else {
            // A blow from the front of a shield is stopped at the plate
            // (feature 100), as a bolt is: nothing rolled, nothing landed,
            // and the plate flares where the blade met it.
            let toward = (from - at).normalize_or_zero();
            let front = self.shield_front(by);
            if let Some(heading) = self
                .targets
                .get(target)
                .copied()
                .flatten()
                .and_then(|t| t.shield)
                && shield_stops_within(heading, toward, front)
            {
                let plate = at + toward * balance::GUARDIAN_SHIELD_RADIUS;
                self.fx.shield(at, plate);
                self.plate_hits.push((target, damage));
                self.cues.push(Cued {
                    cue: Cue::Shielded,
                    at: plate,
                });
                return;
            }
            let roll = self.rng.unit();
            // Weak Spot (task 124): a Bim's blow, fist or blade, may be
            // critical — off the lent stream, after the part's roll.
            let crit = self.roll_crit(by);
            if crit {
                self.fx.critical(at, at - from, weapon, false);
            }
            self.hits.push(Hit {
                who: target,
                damage,
                cut,
                by,
                blast: false,
                roll,
                flat,
                crit,
                // A blow strips nothing: the Unmaker is a lance, not a
                // claw, and a claw is not a lance.
                strips: 0.0,
            });
            self.cues.push(Cued {
                cue: Cue::Blow {
                    cut,
                    on_crew: false,
                },
                at,
            });
            // A blade's blow glows where it cut (feature 98); a fist or a
            // claw lands with nothing to light.
            if cut {
                self.fx.cut(from, at, weapon, false);
            }
        }
    }

    /// One roll off the combat stream, 0 to 1: what the game rolls where a
    /// carried body is set down with, so a fight re-rolls nothing of the
    /// room's.
    pub fn roll(&mut self) -> f32 {
        self.rng.unit()
    }

    /// A blow the world delivered to one of this room's own bodies — an
    /// enemy's melee [`Shot`], carried across — as the hit it is, on a
    /// part rolled now from the combat stream. The game applies it.
    pub fn struck(&mut self, who: usize, damage: f32, cut: bool) -> Hit {
        self.lull = 0.0;
        let roll = self.rng.unit();
        Hit {
            who,
            damage,
            cut,
            by: None,
            blast: false,
            roll,
            flat: 0.0,
            crit: false,
            strips: 0.0,
        }
    }

    /// Fly every bolt on by `dt`: into a body, into a wall, or out to the
    /// end of its range, whichever comes first. A friendly bolt looks for
    /// the targets; a hostile one for `bodies`, this room's own — each
    /// Bim's position while it is alive and on the deck, conscious or
    /// not, the peek position while it peeks, and whether it is peeking;
    /// `None` for one dead or outside. Where a bolt reaches a body the
    /// part it lands on is rolled then, from the combat stream, and the
    /// damage is the weapon's at the distance flown; a body in cover —
    /// peeking, or close behind sandbags on the side the bolt comes from
    /// (`Sight::covered`) — dodges it half the time and it flies on.
    pub fn step(&mut self, dt: f32, sight: &Sight, bodies: &[Option<(Vec2, bool, f32)>]) {
        for s in &mut self.sparks {
            s.age += dt;
        }
        self.sparks.retain(|s| s.age < SPARK_LIFE);

        let targets: Vec<Option<(Vec2, bool, f32)>> = self
            .targets
            .iter()
            .map(|t| t.map(|t| (t.at, t.peeking, t.dodge)))
            .collect();
        // And the shields among the targets (feature 100), for a friendly
        // bolt alone: this room's own bodies carry none.
        let shields: Vec<Option<Vec2>> = self
            .targets
            .iter()
            .map(|t| t.and_then(|t| t.shield))
            .collect();
        let own_cover_dodge = &self.own_cover_dodge;
        let plates = &self.plates;
        let reflects = &self.reflects;
        let shield_fronts = &self.shield_fronts;
        let rng = &mut self.rng;
        let crit_rng = &mut self.crit_rng;
        let crit_chances = &self.crit_chances;
        let fx = &mut self.fx;
        let mut landed: Vec<Hit> = Vec::new();
        let mut taken: Vec<Hit> = Vec::new();
        let mut sparks: Vec<Spark> = Vec::new();
        let mut heard: Vec<Cued> = Vec::new();
        let mut broken: Vec<(usize, f32)> = Vec::new();
        let mut bagged: Vec<((i32, i32), f32)> = Vec::new();
        let mut plated: Vec<(usize, f32)> = Vec::new();
        // What each Riot Shield still takes this step, and what it
        // stopped, and the bolts it bounced (task 155) — flown from the
        // next step on.
        let mut plate_left: Vec<f32> = plates.iter().map(|p| p.hp).collect();
        let mut blocked: Vec<(usize, f32)> = Vec::new();
        let mut bounced: Vec<Bolt> = Vec::new();
        self.bolts.retain_mut(|bolt| {
            let mut flight = bolt.vel * dt;
            let mut span = flight.len();
            if span > bolt.left {
                flight = flight * (bolt.left / span.max(1e-6));
                span = bolt.left;
            }
            let from = bolt.pos;
            let to = from + flight;
            // The first thing along the way: a wall, or a body. Each as a
            // fraction of this step's flight.
            let mut stop: Option<(f32, Option<usize>)> = None;
            if let Some(wall) = sight.first_opaque_along(from, to) {
                stop = Some(((wall - from).len() / span.max(1e-6), None));
            }
            // A lamp on the way, if the bolt passes close enough: it
            // stops there like at a wall, and the lamp takes the
            // damage. One already out is glass nobody misses.
            let mut lamp: Option<usize> = None;
            for (i, l) in sight.lamps().iter().enumerate() {
                if l.is_out() {
                    continue;
                }
                if let Some(t) = along(from, to, l.at, LAMP_RADIUS)
                    && stop.is_none_or(|(s, _)| t < s)
                {
                    stop = Some((t, None));
                    lamp = Some(i);
                }
            }
            let looking_for: &[Option<(Vec2, bool, f32)>] =
                if bolt.hostile { bodies } else { &targets };
            // **A shield facing the bolt stops it at the plate** (feature
            // 100): a bolt that would reach the body — its line within
            // `HIT_RADIUS` of it, so a miss flies on as a miss does —
            // coming in from inside the front arc stops where it crosses
            // the plate's radius, and nothing is rolled for it. Asked
            // before the bodies, since the plate stands out in front of
            // whatever it guards.
            let mut shielded: Option<usize> = None;
            if !bolt.hostile {
                let dir = bolt.vel.normalize_or_zero();
                for (i, shield) in shields.iter().enumerate() {
                    let (Some(heading), Some((body, _, _))) =
                        (*shield, targets.get(i).copied().flatten())
                    else {
                        continue;
                    };
                    // A lance slug already through this one is past its
                    // plate.
                    if bolt.has_struck(i) {
                        continue;
                    }
                    let off = body - from;
                    let would_hit = off.dot(dir) > 0.0 && dir.perp_dot(off).abs() <= HIT_RADIUS;
                    let front = shield_front_of(shield_fronts, bolt.by);
                    if !would_hit || !shield_stops_within(heading, -dir, front) {
                        continue;
                    }
                    if let Some(t) = along(from, to, body, balance::GUARDIAN_SHIELD_RADIUS)
                        && stop.is_none_or(|(s, _)| t < s)
                    {
                        stop = Some((t, None));
                        lamp = None;
                        shielded = Some(i);
                    }
                }
            }
            // **A Riot Shield held up stops a hostile bolt at its plate**
            // (task 155): one crossing the plate from the front stops
            // there, nothing rolled — unless a body stands nearer on the
            // way, which the loop below finds first.
            let mut on_plate: Option<usize> = None;
            if bolt.hostile {
                for (k, plate) in plates.iter().enumerate() {
                    if plate_left[k] <= 0.0 {
                        continue;
                    }
                    let Some((body, _, _)) = bodies.get(plate.who).copied().flatten() else {
                        continue;
                    };
                    if let Some(t) = plate.crossing(body, from, to)
                        && stop.is_none_or(|(s, _)| t < s)
                    {
                        stop = Some((t, None));
                        lamp = None;
                        on_plate = Some(k);
                    }
                }
            }
            for (i, body) in looking_for.iter().enumerate() {
                let Some((body, peeking, dodge)) = *body else {
                    continue;
                };
                if bolt.dodged == Some(i) || bolt.has_struck(i) {
                    continue;
                }
                // The holder of a plate the bolt crossed this step is
                // behind it: the bolt never reaches him.
                if on_plate.is_some_and(|k| plates[k].who == i) {
                    continue;
                }
                if let Some(t) = along(from, to, body, HIT_RADIUS)
                    && stop.is_none_or(|(s, _)| t < s)
                {
                    // In cover — leaning back in from a peek, or ducking
                    // behind the sandbags between it and the shooter —
                    // the bolt flies on past, and is not rolled for this
                    // body again.
                    let bags = sight.cover_between(body, bolt.fired_from);
                    let covered = peeking || bags.is_some();
                    // Half the time — or a soldier's own odds in cover,
                    // for one of this room's own bodies (`own_cover_dodge`).
                    let odds = if bolt.hostile {
                        own_cover_dodge.get(i).copied().unwrap_or(DODGE_IN_COVER)
                    } else {
                        DODGE_IN_COVER
                    };
                    if covered && rng.chance(odds) {
                        bolt.dodged = Some(i);
                        // Ducked behind sandbags rather than back in from
                        // a peek: the bags took it, at the distance flown
                        // to the body.
                        if !peeking && let Some(tile) = bags {
                            let flown = (body - bolt.fired_from).len() / TILE;
                            bagged.push((tile, bolt.stats().damage_at(flown) * bolt.damage));
                        }
                        continue;
                    }
                    // And its armour: a tier-three piece gives its wearer
                    // a chance of slipping the bolt in the open. Rolled
                    // only for a body that has any, so a fight with none
                    // draws exactly what it always did.
                    if dodge > 0.0 && rng.chance(dodge) {
                        bolt.dodged = Some(i);
                        continue;
                    }
                    stop = Some((t, Some(i)));
                    on_plate = None;
                }
            }
            match stop {
                Some((t, who)) => {
                    let at = from + flight * t;
                    // Whether a lance slug goes on past the body it struck
                    // here (task 115).
                    let mut through = false;
                    // Whether it landed a critical hit (task 124): its flare
                    // twice the size and the brightness.
                    let mut critical = false;
                    if let Some(who) = who {
                        let flown = (at - bolt.fired_from).len() / TILE;
                        let stats = bolt.stats();
                        // *Point blank*: the factor while the bolt has
                        // flown no further than the sweet range. The
                        // bolt's own factor (*sentry mark III*) is on
                        // top of it, at any distance.
                        let close = bolt.damage
                            * if flown <= stats.sweet {
                                bolt.point_blank
                            } else {
                                1.0
                            };
                        // A lance slug's n-th body takes the falloff n
                        // times over — multiplied out rather than a
                        // `powi`, so every platform agrees to the bit.
                        // One for every other bolt, which has struck
                        // nobody before this.
                        let mut falloff = 1.0;
                        for _ in 0..bolt.strikes() {
                            falloff *= balance::LANCE_FALLOFF;
                        }
                        let roll = rng.unit();
                        let hit = Hit {
                            who,
                            damage: stats.damage_at(flown) * close * falloff,
                            cut: false,
                            by: bolt.by,
                            blast: false,
                            roll,
                            flat: bolt.weapon.stats().damage_at(flown),
                            crit: false,
                            // What the Unmaker takes off the armour instead
                            // of off the body; nought for every other gun.
                            strips: stats.strips_at(flown) * falloff,
                        };
                        if bolt.hostile {
                            // A Reflect Barrier on the body (task 155):
                            // its share of the hit back on the shooter.
                            landed.extend(reflected(reflects, &hit, bolt.shooter));
                            taken.push(hit);
                        } else {
                            // Weak Spot (task 124): each bolt rolled on its
                            // own, off the lent stream alone.
                            let mut hit = hit;
                            hit.crit = roll_crit_off(crit_rng, crit_chances, bolt.by);
                            critical = hit.crit;
                            landed.push(hit);
                        }
                        // The slug goes through as long as it has bodies
                        // left to strike.
                        if bolt.pierces() {
                            if let Some(slot) = bolt.struck.iter_mut().find(|s| s.is_none()) {
                                *slot = Some(who);
                            }
                            through = bolt.strikes() < balance::LANCE_PIERCE;
                        }
                    } else if let Some(lamp) = lamp {
                        // Nothing nearer than the lamp: the lamp took it.
                        let flown = (at - bolt.fired_from).len() / TILE;
                        broken.push((lamp, bolt.stats().damage_at(flown) * bolt.damage));
                    } else if let Some(i) = shielded {
                        // The plate took it, at the distance flown, as
                        // hard as it would have struck the body.
                        let flown = (at - bolt.fired_from).len() / TILE;
                        let close = if flown <= bolt.stats().sweet {
                            bolt.point_blank
                        } else {
                            1.0
                        };
                        plated.push((i, bolt.stats().damage_at(flown) * bolt.damage * close));
                    } else if let Some(k) = on_plate {
                        // A Riot Shield took it (task 155), as hard as it
                        // would have struck the holder, and **bounces**
                        // it: a bolt of the holder's off the plate along
                        // the way mirrored in it, the weapon's whole
                        // reach again from there.
                        let plate = plates[k];
                        let flown = (at - bolt.fired_from).len() / TILE;
                        let close = if flown <= bolt.stats().sweet {
                            bolt.point_blank
                        } else {
                            1.0
                        };
                        let damage = bolt.stats().damage_at(flown) * bolt.damage * close;
                        plate_left[k] -= damage;
                        blocked.push((plate.who, damage));
                        let out = plate.bounce(bolt.vel.normalize_or_zero());
                        if out != Vec2::ZERO {
                            bounced.push(Bolt {
                                pos: at,
                                vel: out * bolt.vel.len(),
                                left: bolt.stats().reach(),
                                weapon: bolt.weapon,
                                fired_from: at,
                                hostile: false,
                                by: Some(plate.who),
                                point_blank: 1.0,
                                damage: bolt.damage,
                                range: bolt.range,
                                dodged: None,
                                struck: [None; balance::LANCE_PIERCE],
                                shooter: None,
                            });
                        }
                        fx.plate(at, out);
                    }
                    heard.push(Cued {
                        cue: match who {
                            Some(_) => Cue::Impact {
                                on_crew: bolt.hostile,
                            },
                            None if shielded.is_some() || on_plate.is_some() => Cue::Shielded,
                            None => Cue::Ricochet,
                        },
                        at,
                    });
                    sparks.push(Spark {
                        pos: at,
                        age: 0.0,
                        hostile: bolt.hostile,
                    });
                    // Through: the step ends at the body, and the slug
                    // flies on from there next step with the reach it
                    // has left, a flare where it went through.
                    if through {
                        fx.pierced(at, bolt.vel, bolt.weapon, bolt.hostile);
                        bolt.pos = at;
                        bolt.left -= span * t;
                        if bolt.left <= 0.0 {
                            fx.spent(bolt.fired_from, at, bolt.weapon, bolt.hostile);
                            return false;
                        }
                        return true;
                    }
                    // The picture's own flash, on the host's clock — and a
                    // scorch where a wall or a lamp stopped it. A shield
                    // leaves no scorch: the plate flares where it struck.
                    fx.landed(
                        bolt.fired_from,
                        at,
                        bolt.vel,
                        bolt.weapon,
                        bolt.hostile,
                        who.is_some() || shielded.is_some() || on_plate.is_some(),
                    );
                    if critical {
                        fx.critical(at, bolt.vel, bolt.weapon, bolt.hostile);
                    }
                    if let Some((centre, _, _)) = shielded.and_then(|i| targets[i]) {
                        fx.shield(centre, at);
                    }
                    false
                }
                None => {
                    bolt.pos = to;
                    bolt.left -= span;
                    if bolt.left <= 0.0 {
                        // Spent: it fades where it got to, without a flash
                        // — a sniper's beam hanging a moment after it.
                        fx.spent(bolt.fired_from, bolt.pos, bolt.weapon, bolt.hostile);
                        return false;
                    }
                    true
                }
            }
        });
        self.hits.extend(landed);
        self.wounds_taken.extend(taken);
        self.lamp_hits.extend(broken);
        self.cover_hits.extend(bagged);
        self.plate_hits.extend(plated);
        self.plate_blocks.extend(blocked);
        self.bolts.extend(bounced);
        self.sparks.extend(sparks);
        self.cues.extend(heard);
        // And the beams, after the bolts (feature 100).
        self.step_sweeps(dt, sight, bodies);
    }

    /// Whether anything is in the air or lit.
    pub fn quiet(&self) -> bool {
        self.bolts.is_empty()
            && self.sparks.is_empty()
            && self.sweeps.is_empty()
            && !self.grenades_out()
    }

    /// The bolts, the passing lights round them (`crate::fx`) and the
    /// grenades. Over the fog — a shot is always seen. Every gun is a
    /// laser in its side's colour, and each has a shape of its own.
    pub fn draw(&self, list: &mut DrawList) {
        for bolt in &self.bolts {
            let dir = bolt.vel.normalize_or_zero();
            let head = bolt.pos;
            let hostile = bolt.hostile;
            let side = if hostile { HOSTILE_BOLT } else { FRIENDLY_BOLT };
            let (w, h) = fx::tier_look(bolt.weapon.tier);
            let heat = fx::CORE_HEAT + h;
            match bolt.weapon.kind {
                WeaponKind::Shotgun => {
                    // A spread of short pulses about the flight, opening
                    // the further it has come.
                    let flown = (head - bolt.fired_from).len();
                    let fan = (PELLET_FAN + flown * PELLET_FAN_GROW).min(PELLET_FAN_MAX);
                    for (i, back) in PELLET_STAGGER.iter().enumerate() {
                        let f = (i as f32 - (PELLETS as f32 - 1.0) * 0.5)
                            / ((PELLETS - 1) as f32 * 0.5);
                        let d = dir.rotate(f * PELLET_SPREAD);
                        let tip = head - dir * *back + dir.perp() * (f * fan);
                        fx::laser(
                            list,
                            tip - d * PELLET_LENGTH,
                            tip,
                            3.5 * w,
                            PELLET_CORE * w,
                            hostile,
                            heat,
                            1.0,
                        );
                    }
                }
                WeaponKind::AutoRifle => {
                    // Thin, and in two: a dash with a shorter one behind.
                    let lead = head - dir * PULSE_LEAD;
                    fx::laser(
                        list,
                        lead,
                        head,
                        3.5 * w,
                        PULSE_CORE * w,
                        hostile,
                        heat,
                        1.0,
                    );
                    let trail = lead - dir * PULSE_GAP;
                    fx::laser(
                        list,
                        trail - dir * PULSE_TAIL,
                        trail,
                        3.0 * w,
                        PULSE_CORE * w,
                        hostile,
                        heat,
                        0.6,
                    );
                }
                WeaponKind::SniperRifle => {
                    // The beam, tied to the bolt: from the muzzle it left
                    // — faint there, stronger along the way — to the last
                    // stretch, bright, and the head brightest.
                    let span = (head - bolt.fired_from).len();
                    let bright = STREAK_LENGTH.min(span);
                    let mid = head - dir * bright;
                    if span > bright {
                        let half = bolt.fired_from.lerp(mid, 0.5);
                        let faint = BEAM_CORE * 0.6 * w;
                        fx::laser(
                            list,
                            bolt.fired_from,
                            half,
                            4.0 * w,
                            faint,
                            hostile,
                            1.4 + h,
                            0.3,
                        );
                        fx::laser(list, half, mid, 4.0 * w, faint, hostile, 1.7 + h, 0.55);
                    }
                    fx::laser(
                        list,
                        mid,
                        head,
                        6.0 * w,
                        BEAM_CORE * w,
                        hostile,
                        heat + 0.4,
                        1.0,
                    );
                    list.circle(head, 5.0 * w, fx::hot(hostile, heat + 0.4));
                }
                WeaponKind::Minigun => {
                    // A short thin dash and nothing behind it.
                    fx::laser(
                        list,
                        head - dir * DASH_LENGTH,
                        head,
                        DASH_GLOW * w,
                        DASH_CORE * w,
                        hostile,
                        heat,
                        1.0,
                    );
                }
                WeaponKind::RailLance => {
                    // One even line from the muzzle to the head, the
                    // thickest core there is, whatever it has gone
                    // through on the way.
                    fx::laser(
                        list,
                        bolt.fired_from,
                        head,
                        RAIL_GLOW * w,
                        RAIL_CORE * w,
                        hostile,
                        heat + 0.4 - fx::LANCE_COOLING,
                        1.0,
                    );
                    list.circle(
                        head,
                        RAIL_HEAD * w,
                        fx::hot(hostile, heat + 0.6 - fx::LANCE_COOLING),
                    );
                }
                // The Unmaker's bolt (feature 83): a long crackling line
                // rather than a clean one — a straight core with the
                // discharge jagging off it either side, drawn in
                // whichever side's colour fired it, which for a droid is
                // always the hostile red.
                WeaponKind::Unmaker => {
                    let tail = head - dir * LANCE_LENGTH;
                    list.line(tail, head, LANCE_GLOW, side.alpha(0.22));
                    list.line(tail, head, BOLT_CORE, side.alpha(0.9));
                    let perp = dir.perp();
                    let mut at = tail;
                    let mut side_of = 1.0f32;
                    for i in 0..LANCE_KINKS {
                        let f = (i + 1) as f32 / LANCE_KINKS as f32;
                        // The kink's throw narrows towards the head, so
                        // the discharge reads as coming off the tip.
                        let throw = LANCE_THROW * (1.0 - f * 0.6) * side_of;
                        let next = tail + dir * (LANCE_LENGTH * f) + perp * throw;
                        list.line(at, next, 2.0, side.alpha(0.75));
                        at = next;
                        side_of = -side_of;
                    }
                    list.line(at, head, 2.0, side.alpha(0.75));
                    list.circle(head, 7.0, side.alpha(0.5));
                    list.circle(head, 3.5, BOLT_CORE_WHITE.alpha(0.95));
                }
                // The pistol: the short clean bolt it has always been, its
                // core the emissive one (feature 97). Neither a blade nor a
                // claw ever flies, and a Guardian's Sweeper is drawn as the
                // plain bolt too wherever one is flown as one.
                WeaponKind::LaserPistol
                | WeaponKind::Schword
                | WeaponKind::Claw
                | WeaponKind::Sweeper => {
                    let tail = head - dir * BOLT_LENGTH;
                    list.line(tail, head, BOLT_GLOW * w, side.alpha(0.30));
                    list.line(tail, head, (BOLT_CORE + 1.5) * w, side.alpha(0.85));
                    list.line(
                        tail + dir * (BOLT_LENGTH * 0.35),
                        head,
                        BOLT_CORE * w,
                        fx::hot(hostile, heat),
                    );
                }
            }
        }
        // The Guardians' beams (feature 100), in this room if it draws them:
        // laid where the last sub-step laid them.
        // The beam itself is the machines' red, and a **white-hot core**
        // down it that flickers — the core past white, so the bloom is what
        // makes it glow, and no halo is drawn by hand. Where a wall or a
        // shut door stops it, a bright point burns (the scorch it leaves
        // along the sweep is the passing lights', `Fx::burn`).
        for s in self.sweeps.iter().filter(|s| s.drawn && s.done > 0) {
            let flick = fx::scatter((s.elapsed * 60.0) as u32, s.done);
            list.line(s.from, s.end, SWEEP_BEAM, HOSTILE_BOLT.alpha(0.85));
            list.line(
                s.from,
                s.end,
                SWEEP_CORE * (0.75 + 0.5 * flick),
                BOLT_CORE_WHITE.glowing(SWEEP_HEAT + flick),
            );
            if s.walled {
                list.circle(
                    s.end,
                    SWEEP_TIP * (0.8 + 0.4 * flick),
                    fx::BURST_HOT.glowing(SWEEP_HEAT),
                );
            }
        }
        // Where the bolts ended: the fight's own flash for a host that ages
        // no effects, the passing lights for one that does.
        if self.fx.is_on() {
            self.fx.draw_air(list);
        } else {
            for spark in &self.sparks {
                let t = 1.0 - spark.age / SPARK_LIFE;
                let colour = if spark.hostile {
                    HOSTILE_BOLT
                } else {
                    FRIENDLY_BOLT
                };
                list.circle(spark.pos, 8.0 + 14.0 * (1.0 - t), colour.alpha(0.55 * t));
                list.circle(spark.pos, 5.0 * t, BOLT_CORE_WHITE.alpha(0.9 * t));
            }
        }
        // A grenade (feature 75): a dark canister with a lit fuse, lifted
        // and shadowed while it flies, blinking faster as the fuse runs
        // down once it lies on its tile.
        for g in &self.grenades {
            let pos = g.pos();
            // A Stun Shot (October 2026): a glowing slug of the stun's
            // blue with a streak behind it, flat on the deck.
            if g.shot {
                let dir = (g.at - g.from).normalize_or_zero();
                let tail = pos - dir * ((pos - g.from).len().min(70.0));
                list.line(
                    tail,
                    pos,
                    9.0,
                    crate::droid::STUNNED.glowing(1.2).alpha(0.45),
                );
                list.circle(pos, 22.0, crate::droid::STUNNED.glowing(1.6).alpha(0.9));
                list.circle(pos, 10.0, BOLT_CORE_WHITE);
                continue;
            }
            let flying = g.fuse - g.left < GRENADE_FLIGHT;
            let lift = if flying {
                let f = ((g.fuse - g.left) / GRENADE_FLIGHT).clamp(0.0, 1.0);
                (f * std::f32::consts::PI).sin()
            } else {
                0.0
            };
            // A satchel charge in the air (task 154): a canvas pack with
            // its strap and a red light, lifted and shadowed like a
            // grenade.
            if g.satchel {
                list.ellipse(
                    pos + vec2(0.0, 4.0 + 10.0 * lift),
                    vec2(16.0, 9.0) * (1.0 - 0.3 * lift),
                    0.0,
                    Color::rgba(0.0, 0.0, 0.0, 0.35),
                );
                let body = pos - vec2(0.0, 12.0 * lift);
                let size = vec2(30.0, 22.0) * (1.0 + 0.3 * lift);
                list.rect(body, size, 0.0, 4.0, SATCHEL_CANVAS);
                list.rect(body, vec2(size.x, size.y * 0.25), 0.0, 0.0, SATCHEL_STRAP);
                list.circle(body + vec2(size.x * 0.3, -size.y * 0.3), 8.0, FUSE_RED);
                continue;
            }
            list.ellipse(
                pos + vec2(0.0, 4.0 + 10.0 * lift),
                vec2(12.0, 8.0) * (1.0 - 0.3 * lift),
                0.0,
                Color::rgba(0.0, 0.0, 0.0, 0.35),
            );
            let body = pos - vec2(0.0, 12.0 * lift);
            list.circle(body, 18.0 * (1.0 + 0.3 * lift), GRENADE_SHELL);
            list.circle(body, 11.0 * (1.0 + 0.3 * lift), GRENADE_BAND);
            // The fuse: bright, and flashing quicker near the end.
            let rate = 2.0 + 8.0 * (1.0 - g.left / g.fuse.max(1e-3));
            let on = ((g.fuse - g.left) * rate * std::f32::consts::TAU).sin() > 0.0 || flying;
            if on {
                list.circle(body + vec2(4.0, -4.0), 5.0 + 2.0 * lift, FUSE_LIT);
            }
        }
        // And a burst: a white flash swelling out to the radius and
        // fading, with the hot core going first.
        for b in &self.blasts {
            let t = (b.age / BLAST_LIFE).clamp(0.0, 1.0);
            let out = 1.0 - (1.0 - t) * (1.0 - t);
            // An EMP (task 127) is a pale blue ring expanding to its radius,
            // and no flash: it burns nothing.
            if b.emp {
                list.ring(
                    b.pos,
                    b.radius * 2.0 * out,
                    4.0,
                    crate::droid::STUNNED.glowing(1.3).alpha(0.9 * (1.0 - t)),
                );
                list.circle(
                    b.pos,
                    b.radius * 2.0 * out,
                    crate::droid::STUNNED.alpha(0.12 * (1.0 - t)),
                );
                continue;
            }
            let (glow, rim) = (BLAST_GLOW, BLAST_RIM);
            list.circle(b.pos, b.radius * 2.0 * out, glow.alpha(0.6 * (1.0 - t)));
            list.ring(b.pos, b.radius * 2.0 * out, 3.0, rim.alpha(0.8 * (1.0 - t)));
            list.circle(
                b.pos,
                b.radius * 0.7 * (1.0 - t),
                BOLT_CORE_WHITE.alpha(0.95 * (1.0 - t)),
            );
        }
    }
}

/// A grenade's shell and band, its fuse, and the burst's colours.
const GRENADE_SHELL: Color = Color::rgb(0.18, 0.20, 0.16);
const GRENADE_BAND: Color = Color::rgb(0.42, 0.45, 0.30);
const FUSE_LIT: Color = Color::rgb(1.0, 0.85, 0.35);
const BLAST_GLOW: Color = Color::rgb(1.0, 0.78, 0.40);
const BLAST_RIM: Color = Color::rgb(1.0, 0.55, 0.20);
/// A satchel charge's canvas, its strap and its armed light (task 154).
pub const SATCHEL_CANVAS: Color = Color::rgb(0.42, 0.33, 0.20);
pub const SATCHEL_STRAP: Color = Color::rgb(0.22, 0.17, 0.11);
pub const FUSE_RED: Color = Color::rgb(1.0, 0.25, 0.20);

/// Where along the segment `a`–`b`, as a fraction, a circle of `radius` at
/// `centre` is first touched, if it is.
fn along(a: Vec2, b: Vec2, centre: Vec2, radius: f32) -> Option<f32> {
    let d = b - a;
    let len2 = d.dot(d);
    if len2 <= 1e-9 {
        return ((centre - a).len() <= radius).then_some(0.0);
    }
    // The nearest point of the line to the centre, clamped to the segment.
    let t = ((centre - a).dot(d) / len2).clamp(0.0, 1.0);
    let near = a + d * t;
    if (centre - near).len() > radius {
        return None;
    }
    // Back off along the segment to where the circle's edge is crossed, so
    // the flash sits on the body's edge rather than in its middle.
    let back = (radius * radius - (centre - near).dot(centre - near))
        .max(0.0)
        .sqrt()
        / len2.sqrt();
    Some((t - back).max(0.0))
}

/// Whether a bolt from `from` gets to `to` with nothing in its way: no
/// wall, and no lamp still lit that it would pass near enough to break —
/// the two things a flying bolt stops at before the bodies
/// (`Combat::step`). What `Game::shot_from` picks a shot's start
/// by.
pub fn line_of_fire(sight: &Sight, from: Vec2, to: Vec2) -> bool {
    sight.first_opaque_along(from, to).is_none()
        && sight
            .lamps()
            .iter()
            .all(|l| l.is_out() || along(from, to, l.at, LAMP_RADIUS).is_none())
}
/// What a candidate stand is worth to the enemy, in **tiles of walking**
/// — every term is in that one unit, so the trade-offs read off the
/// numbers. Cover is worth walking [`COVER_WORTH`] tiles for, so cover a
/// few tiles off beats the open anywhere and cover across the station
/// loses to the open here. How the weapon fits the distance against
/// what the target holds ([`Tactics::fit`]) is worth up to [`FIT_WORTH`]
/// either way, and outweighs the plain preference for distance
/// ([`DISTANCE_WORTH`] a tile), which is what is left when the two
/// weapons are a match: keep away, all else equal.
pub const COVER_WORTH: f32 = 15.0;
const FIT_WORTH: f32 = 30.0;
pub const DISTANCE_WORTH: f32 = 1.0;
/// What a tile of distance is worth to a body whose business is to be
/// **out of the fight** (feature 86): a field medic, which keeps to the
/// far end of its weapon's reach so that it is still standing when
/// somebody needs fetching. Six tiles of walking a tile of distance, so
/// it walks the length of its reach for it and still ducks behind cover
/// within a couple of tiles of as far back as it can get
/// ([`COVER_WORTH`] over this is two and a half tiles).
pub const KEEP_BACK_WORTH: f32 = 6.0;
/// What a tile of walking costs against those: half of one, since a
/// stand is worth getting to.
const WALK_COST_PER_TILE: f32 = 0.5;
/// What the spot the enemy already stands on is worth over a fresh one,
/// so that two spots as good as each other do not have it walking
/// between them every plan.
const STAY_BONUS: f32 = 2.0;

/// How far round a target a charging blade looks for a cell to stand on,
/// in tiles. Wide enough that a target hemmed in by furniture still has
/// a free cell beside it somewhere.
const CHARGE_LOOK: f32 = 3.0;

/// How far a field medic with a body in its arms looks for somewhere to
/// carry it to, in tiles (nobody else runs since task 120).
const FLEE_LOOK: f32 = 10.0;

/// How far ahead a body fighting its way to a point looks for the next
/// spot to make for, in tiles ([`Tactics::advance`], feature 84). Far
/// enough that the cover across a corridor is a candidate, near enough
/// that the walk between two plans is short enough to read the fight
/// off.
const ADVANCE_LOOK: f32 = 8.0;
/// What a tile of ground made good towards that point is worth against
/// the cover on the way, in the same tiles of walking every other weight
/// here is in: cover a couple of tiles off the straight line is taken,
/// cover half the way back is not.
const GROUND_WORTH: f32 = 4.0;

/// A planned walk to a point, read as how far is left of it
/// ([`Tactics::advance`]): the waypoints, the walk left from each, and
/// the whole of it from where the body stands.
struct Walk<'a> {
    route: &'a [Vec2],
    left: Vec<f32>,
    whole: f32,
}

impl<'a> Walk<'a> {
    /// `None` for no route.
    fn along(from: Vec2, route: &'a [Vec2]) -> Option<Walk<'a>> {
        let first = *route.first()?;
        let mut left = vec![0.0; route.len()];
        for i in (0..route.len() - 1).rev() {
            left[i] = left[i + 1] + (route[i + 1] - route[i]).len();
        }
        let whole = (first - from).len() + left[0];
        Some(Walk { route, left, whole })
    }

    /// The walk left from `c`: straight to a waypoint it can walk
    /// straight to and the route from there, the shortest of those —
    /// never shorter than the walk really is, since it is a walk. Only
    /// the waypoints within twice [`ADVANCE_LOOK`] of the walk's start
    /// are asked, the cells being within the look of it; `None` when `c`
    /// can walk straight to none of them.
    fn left_from(&self, nav: &Nav, c: Vec2) -> Option<f32> {
        let reach = self.whole - 2.0 * ADVANCE_LOOK * TILE;
        let mut best: Option<f32> = None;
        for (i, &p) in self.route.iter().enumerate() {
            if i > 0 && self.left[i - 1] < reach {
                break;
            }
            let walk = (p - c).len() + self.left[i];
            if best.is_some_and(|b| walk >= b) || !nav.line_clear(c, p) {
                continue;
            }
            best = Some(walk);
        }
        best
    }
}

/// Where the tactics say to stand, and whether it is cover: a peek beside
/// a wall, or close behind sandbags.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stand {
    pub at: Vec2,
    pub cover: bool,
}

/// What a spot is weighed by in [`Tactics::stand_scored`]: which eyes
/// count, and whether the dark does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Seeing {
    /// A clear line from the body's eye or a peek beside a wall, lit or
    /// not — every Bim's, as it always was.
    Line,
    /// The same eyes, and only a target the body could make out there
    /// ([`Sight::makes_out`]): a Warden, which peeks.
    MadeOut,
    /// The body's own eye alone, never a peek, and only a target it
    /// could make out: a Trooper and a Guardian.
    OwnEye,
}

/// The enemy's choice of where to stand. See the module note.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tactics;

impl Tactics {
    /// Where a field medic at `from` carrying a body out of the fire
    /// should make for (feature 86): away from the enemy. The enemy is
    /// one place — the average of every target that is up — and the best cell is the reachable one within
    /// [`FLEE_LOOK`] tiles that is furthest from it, less
    /// [`WALK_COST_PER_TILE`] a tile of the walk, so it runs the opposite
    /// way and round a wall if it has to. `None` with no target up, or
    /// nowhere further off than where it stands.
    pub fn flee(nav: &Nav, from: Vec2, targets: &[Option<Target>]) -> Option<Vec2> {
        let up: Vec<Vec2> = targets.iter().flatten().map(|t| t.at).collect();
        if up.is_empty() {
            return None;
        }
        let centre = up.iter().fold(Vec2::ZERO, |a, &b| a + b) * (1.0 / up.len() as f32);
        let here = (from - centre).len() / TILE;
        let mut best: Option<(f32, Vec2)> = None;
        for c in nav.free_cells_within(from, FLEE_LOOK * TILE, TILE) {
            if !nav.can_reach(from, c) {
                continue;
            }
            let away = (c - centre).len() / TILE;
            let walk = (c - from).len() / TILE * WALK_COST_PER_TILE;
            let score = away - walk;
            if score > here && best.is_none_or(|(b, _)| score > b) {
                best = Some((score, c));
            }
        }
        best.map(|(_, at)| at)
    }

    /// The targets with every sealed core (feature 108) taken off the
    /// list — `None` in its place, so the indices hold. What a body picks
    /// a stand or a charge from: a shell that stops everything from every
    /// side is nothing to go after until it opens.
    fn open_targets(targets: &[Option<Target>]) -> Vec<Option<Target>> {
        targets.iter().map(|t| t.filter(|t| !t.sealed())).collect()
    }

    /// Where a body at `from` with a blade should go: the free cell it
    /// can reach nearest the nearest target — a charge. Cover means
    /// nothing to a blade, and distance is the one thing it wants less
    /// of. Every cell of the grid is a candidate, not the tile-spaced
    /// lattice a stand is chosen from: a blade wants to stand *beside*
    /// the target, and the lattice's nearest cell can be half a tile
    /// short of reach. `None` with no target it can get near.
    ///
    /// **A taunt that pulls blades comes first** (feature 77; the tank's
    /// Taunt at its fourth rank since task 139): a target taunting within
    /// its own radius of `from` is gone for before any nearer one. A
    /// plain taunt, and a Juggernaut, do not move a blade.
    pub fn charge(nav: &Nav, from: Vec2, targets: &[Option<Target>]) -> Option<Vec2> {
        // Never at a sealed core, which no blade gets through.
        let open = Tactics::open_targets(targets);
        let targets = &open[..];
        let by_distance_to = |to: Vec2| {
            move |a: &Vec2, b: &Vec2| {
                (*a - to)
                    .len()
                    .partial_cmp(&(*b - to).len())
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
        };
        let mut order: Vec<Vec2> = targets.iter().flatten().map(|t| t.at).collect();
        order.sort_by(by_distance_to(from));
        let pulled: Vec<Vec2> = targets
            .iter()
            .flatten()
            .filter(|t| t.magnet && t.taunting > 0.0 && (t.at - from).len() <= t.taunting)
            .map(|t| t.at)
            .collect();
        if !pulled.is_empty() {
            order.retain(|at| !pulled.contains(at));
            let mut first = pulled;
            first.sort_by(by_distance_to(from));
            first.extend(order);
            order = first;
        }
        for target in order {
            let best = nav
                .free_cells_within(target, CHARGE_LOOK * TILE, 0.0)
                .into_iter()
                .filter(|&c| nav.can_reach(from, c))
                .min_by(by_distance_to(target));
            if best.is_some() {
                return best;
            }
        }
        None
    }

    /// Where a body fighting its way to a point should make for next
    /// (feature 84): the reachable free cell within [`ADVANCE_LOOK`]
    /// tiles of `from` that gets it nearest the point, with the cover
    /// between it and the enemy worth [`COVER_WORTH`] tiles of the walk
    /// and a tile of ground made good worth [`GROUND_WORTH`] of it — so a
    /// body sent across a station goes by the barricades rather than
    /// straight down the middle, and goes nonetheless where there are
    /// none.
    ///
    /// **Nearest is by the walk, not as the crow flies.** `route` is the
    /// walk from `from` to the point as [`Nav::path`] plans it, and a
    /// cell's distance is the straight line to the furthest-on waypoint
    /// of it the cell can walk straight to, and the route from there
    /// ([`Walk::left_from`]). Measured as the crow flies, the corner of a room
    /// nearest a banner beyond its wall was ground made good: a bot
    /// walked into it, set off round by the door when nothing nearer was
    /// left, and was turned back into the corner at the next plan — and
    /// stood there for ever once the corner was where it stood. The walk
    /// left can only shrink, so every pick is ground truly made good. In
    /// the open the route is one leg and this is the straight line it
    /// always was.
    ///
    /// Only a cell nearer by the walk than `from` is is a candidate, and
    /// never a doorway or a cell one of its own side has taken. `None`
    /// when nothing within the look is nearer or there is no route: the
    /// caller walks the whole way instead.
    ///
    /// Cover is judged against the nearest target that is up, since that
    /// is the one the ground is being made good under; with no target at
    /// all it is plain ground.
    #[allow(clippy::too_many_arguments)]
    pub fn advance(
        sight: &Sight,
        nav: &Nav,
        from: Vec2,
        route: &[Vec2],
        targets: &[Option<Target>],
        doors: &[Rect],
        taken: &[Vec2],
        cover_worth: f32,
    ) -> Option<Vec2> {
        let way = Walk::along(from, route)?;
        let here = way.whole / TILE;
        let enemy = targets
            .iter()
            .flatten()
            .map(|t| t.at)
            .min_by(|a, b| (*a - from).len().total_cmp(&(*b - from).len()));
        let in_a_doorway = |c: Vec2| doors.iter().any(|d| d.expand(door::REACH).contains(c));
        let is_taken = |c: Vec2| taken.iter().any(|&t| (t - c).len() < TILE);
        let mut best: Option<(f32, Vec2)> = None;
        for c in nav.free_cells_within(from, ADVANCE_LOOK * TILE, TILE) {
            if in_a_doorway(c) || is_taken(c) || !nav.can_reach(from, c) {
                continue;
            }
            let Some(left) = way.left_from(nav, c) else {
                continue;
            };
            let ground = here - left / TILE;
            if ground <= 0.0 {
                continue;
            }
            let cover = match enemy {
                Some(at) if sight.covered(c, at) => cover_worth,
                _ => 0.0,
            };
            let walk = (c - from).len() / TILE * WALK_COST_PER_TILE;
            let score = ground * GROUND_WORTH + cover - walk;
            if best.is_none_or(|(b, _)| score > b) {
                best = Some((score, c));
            }
        }
        best.map(|(_, at)| at)
    }

    /// Where a body at `from`, armed with `stats`, should stand to shoot
    /// at `targets` — `None` for a target that is down — or `None` when
    /// nowhere in reach of any of them can be shot from. A blade does not
    /// stand: it [`Tactics::charge`]s.
    ///
    /// The candidates are a tile-spaced lattice of free cells within the
    /// weapon's reach of each target (`Nav::free_cells_within`), kept to
    /// the ones the body can walk to, plus the spot it stands on now. Each
    /// is scored against every target in reach of it, in tiles of walking
    /// ([`Tactics::view_from`]): what is seen from there — the body's own
    /// eye blind to the target but a peek beside a wall seeing it is
    /// **cover**, worth [`COVER_WORTH`], and so is the body's eye seeing
    /// it from close behind sandbags (`Sight::covered`); the body's eye
    /// seeing it in the open is worth nothing; neither is no stand at
    /// all — plus how the
    /// weapon **fits** the distance ([`Tactics::fit`]), up to
    /// [`FIT_WORTH`] either way, plus [`DISTANCE_WORTH`] a tile of it. The best target's score stands for the
    /// spot, less [`WALK_COST_PER_TILE`] a tile from `from`, plus
    /// [`STAY_BONUS`] for the spot it is on.
    ///
    /// **A doorway is never a stand, nor is a cell within a door's reach
    /// of one** (`doors`: every powered door and airlock, as rects). A
    /// door opens for a body that near ([`door::REACH`]), so a shut leaf
    /// that reads as a wall from afar — one to peek beside, or one to
    /// stand in with the other leaf for cover — is gone by the time the
    /// body gets there, and a spot scored as cover for it is a spot
    /// nothing can be shot from. An enemy that picked the doorway at a
    /// corridor's crossing walked between its two mirror doorways for
    /// ever, opening each as it arrived. The spot it stands on now is
    /// scored as it is, doors open or not: that is the real view.
    pub fn stand(
        sight: &Sight,
        nav: &Nav,
        from: Vec2,
        targets: &[Option<Target>],
        stats: &WeaponStats,
        doors: &[Rect],
    ) -> Option<Vec2> {
        Tactics::stand_with_cover(
            sight,
            nav,
            from,
            targets,
            stats,
            doors,
            &[],
            false,
            DISTANCE_WORTH,
        )
        .map(|s| s.at)
    }

    /// [`Tactics::stand`], and whether the stand it picked is cover — the
    /// one thing a bot with a shot from where it is will walk for, since
    /// walking halves its odds ([`WALKING_ACCURACY`]) — keeping off
    /// `taken`: where the others of its side stand or are walking to, no
    /// cell within a tile of one being a stand, so a squad does not pick
    /// the one best tile and pile onto it. A blade's charge is never
    /// cover.
    ///
    /// `closing` is a hunter's rule (September 2026): no stand farther
    /// from the nearest target it can see than the body is now, a tile's
    /// slack allowed — it holds or closes, never gives ground. A hostile
    /// gunner on the hunt is scored so (`Bim::hunting`, `Game::plan_stand`);
    /// without it one that had hunted its quarry through a passage stepped
    /// back to cover at the far end of its range, lost sight of it there,
    /// hunted again, and walked that loop for ever without ever coming
    /// through the door. Everybody else is scored as they were: a rifle
    /// with its target in sight walks off to its range.
    ///
    /// `distance_worth` is what a tile of distance from the target is
    /// worth against the cover and the fit — [`DISTANCE_WORTH`] for
    /// everybody, and [`KEEP_BACK_WORTH`] for a field medic (feature
    /// 86), which is the one body that would rather be out of the fight
    /// than in it.
    #[allow(clippy::too_many_arguments)]
    pub fn stand_with_cover(
        sight: &Sight,
        nav: &Nav,
        from: Vec2,
        targets: &[Option<Target>],
        stats: &WeaponStats,
        doors: &[Rect],
        taken: &[Vec2],
        closing: bool,
        distance_worth: f32,
    ) -> Option<Stand> {
        Tactics::stand_scored(
            sight,
            nav,
            from,
            targets,
            stats,
            doors,
            taken,
            closing,
            COVER_WORTH,
            distance_worth,
            Seeing::Line,
        )
    }

    /// [`Tactics::stand_with_cover`] with the **worth of cover** said
    /// rather than taken as read. Everything a body picks a stand by is
    /// the same bar that one number, and a `cover_worth` of nought is a
    /// body that weighs a spot by its weapon's fit and the distance
    /// alone: it walks into the open and shoots from wherever it gets
    /// to. That is a Trooper (feature 83, `crate::droid`), and nothing
    /// else so far; a Warden and every Bim take the cover.
    ///
    /// `seeing` says what a spot is weighed by ([`Seeing`]). The
    /// machines' is the rule their triggers are pulled by
    /// (`Combat::aim_among`): only a target they could make out there —
    /// lit or within `DARK_RANGE` ([`Sight::makes_out`]) — and for a
    /// Trooper and a Guardian, which never peek, from their own eye
    /// alone. Before it a Trooper walked off to a spot it could not see
    /// a crew in the dark from, or saw only round a corner it does not
    /// shoot round, and stood there not shooting.
    #[allow(clippy::too_many_arguments)]
    pub fn stand_scored(
        sight: &Sight,
        nav: &Nav,
        from: Vec2,
        targets: &[Option<Target>],
        stats: &WeaponStats,
        doors: &[Rect],
        taken: &[Vec2],
        closing: bool,
        cover_worth: f32,
        distance_worth: f32,
        seeing: Seeing,
    ) -> Option<Stand> {
        // A sealed core is nothing to stand against (feature 108): its
        // shell stops everything, so nobody picks a spot for a shot at
        // it — only the targets open to a shot are weighed.
        let open = Tactics::open_targets(targets);
        let targets = &open[..];
        if stats.melee {
            return Tactics::charge(nav, from, targets).map(|at| Stand { at, cover: false });
        }
        let reach = stats.reach();
        let here = nav.nearest_free(from);
        // How near the body is to the nearest target it can see, for the
        // hunter's rule; nothing in sight, and every stand is allowed.
        let nearest_seen = targets
            .iter()
            .flatten()
            .filter(|t| !t.stale)
            .map(|t| (t.at - from).len())
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let gives_ground = |c: Vec2| {
            closing
                && nearest_seen.is_some_and(|near| {
                    targets
                        .iter()
                        .flatten()
                        .filter(|t| !t.stale)
                        .all(|t| (t.at - c).len() > near + TILE)
                })
        };
        let in_a_doorway = |c: Vec2| doors.iter().any(|d| d.expand(door::REACH).contains(c));
        let mut lattice: Vec<Vec2> = targets
            .iter()
            .flatten()
            .flat_map(|t| nav.free_cells_within(t.at, reach, TILE))
            .filter(|&c| !in_a_doorway(c))
            .collect();
        // Two targets in reach of one another offer the same spots
        // twice, and a spot scored twice is a trace wasted: the lattice is
        // the grid's, so equal spots are equal to the bit.
        lattice.sort_by(|a, b| {
            (a.y, a.x)
                .partial_cmp(&(b.y, b.x))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        lattice.dedup_by(|a, b| (*a - *b).len() <= 1e-3);
        let is_taken = |c: Vec2| taken.iter().any(|&t| (t - c).len() < TILE);
        let mut candidates: Vec<Vec2> = vec![here];
        candidates.extend(lattice.into_iter().filter(|&c| {
            (c - here).len() > 1e-3 && !is_taken(c) && !gives_ground(c) && nav.can_reach(from, c)
        }));
        let mut best: Option<(f32, Stand)> = None;
        for (i, &c) in candidates.iter().enumerate() {
            let Some((view, cover)) = Tactics::view_from(
                sight,
                c,
                targets,
                stats,
                cover_worth,
                distance_worth,
                seeing,
            ) else {
                continue;
            };
            let walk = (c - from).len() / TILE * WALK_COST_PER_TILE;
            let stay = if i == 0 { STAY_BONUS } else { 0.0 };
            let score = view - walk + stay;
            if best.is_none_or(|(b, _)| score > b) {
                best = Some((score, Stand { at: c, cover }));
            }
        }
        best.map(|(_, stand)| stand)
    }

    /// How well a weapon likes a distance against what the target holds,
    /// −1 to 1: what it would do a second from there against what would
    /// come back, each as a share of the better of the two weapons' best.
    /// A sniper rifle against a pistol is at its best at twenty tiles,
    /// where its own curve is still full and the pistol's has run out; a
    /// shotgun against the same wants four tiles or less; a pistol
    /// against a pistol is indifferent, and cover decides. A blade in
    /// the target's hand does nothing at range and everything at reach,
    /// which is what keeps a gunner away from it.
    pub fn fit(stats: &WeaponStats, target: &WeaponStats, tiles: f32) -> f32 {
        let out = stats.dps_at(tiles);
        let back = target.dps_at(tiles);
        let most = stats.dps_at(0.0).max(target.dps_at(0.0)).max(1e-3);
        ((out - back) / most).clamp(-1.0, 1.0)
    }

    /// The best a spot is worth against any one target in reach of it,
    /// in tiles of walking: cover or the open by what is seen from there,
    /// the weapon's fit at that distance, and the distance itself — and
    /// whether that best is cover. `None` when no target can be shot at
    /// from the spot. `seeing`: which eyes count, and the dark ([`Seeing`]).
    fn view_from(
        sight: &Sight,
        c: Vec2,
        targets: &[Option<Target>],
        stats: &WeaponStats,
        cover_worth: f32,
        distance_worth: f32,
        seeing: Seeing,
    ) -> Option<(f32, bool)> {
        let reach = stats.reach();
        let eyes = sight.eyes_from(c);
        let mut best: Option<(f32, bool)> = None;
        for target in targets.iter().flatten() {
            let d = (target.at - c).len();
            if d > reach || (seeing != Seeing::Line && !sight.makes_out(c, target.at)) {
                continue;
            }
            let tile = sight.tile_of(target.at);
            let mut body_sees = false;
            let mut peek_sees = false;
            for eye in &eyes {
                if (seeing == Seeing::OwnEye && eye.is_peek())
                    || !eye.admits(tile)
                    || !sight.clear_line(eye.at, tile)
                {
                    continue;
                }
                if eye.is_peek() {
                    peek_sees = true;
                } else {
                    body_sees = true;
                }
            }
            // Cover is a peek that sees what the body does not, or the
            // body seeing over the sandbags between it and the target.
            let in_cover = if !body_sees && peek_sees {
                true
            } else if body_sees {
                sight.covered(c, target.at)
            } else {
                continue;
            };
            let cover = if in_cover { cover_worth } else { 0.0 };
            let tiles = d / TILE;
            let fit = Tactics::fit(stats, &target.weapon.stats(), tiles) * FIT_WORTH;
            let score = cover + fit + tiles * distance_worth;
            if best.is_none_or(|(b, _)| score > b) {
                best = Some((score, in_cover));
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::BODY_MARGIN;
    use crate::math::{Rect, vec2};

    /// A room twenty tiles by ten with one wall down its middle, from the
    /// top to three tiles short of the bottom, so the two halves join
    /// through the gap. Tile-aligned, the way a ship's room is.
    /// A target standing at `at` with the common weapon, not peeking.
    fn pistol_at(at: Vec2) -> Target {
        Target {
            at,
            weapon: WeaponKind::LaserPistol.basic(),
            peeking: false,
            stale: false,
            dodge: 0.0,
            taunting: 0.0,
            magnet: false,
            taunt_order: 0,
            shield: None,
        }
    }

    fn walled_room() -> (Sight, Nav) {
        let wall = Rect::from_min_size(vec2(10.0 * TILE, 0.0), vec2(TILE, 7.0 * TILE));
        room_with(&[wall])
    }

    fn room_with(solids: &[Rect]) -> (Sight, Nav) {
        let interior = Rect::from_min_size(Vec2::ZERO, vec2(20.0 * TILE, 10.0 * TILE));
        let sight = Sight::new(interior, interior, TILE, solids, &[]);
        let nav = Nav::tiled(interior, solids, BODY_MARGIN, TILE);
        (sight, nav)
    }

    fn middle(x: f32, y: f32) -> Vec2 {
        vec2((x + 0.5) * TILE, (y + 0.5) * TILE)
    }

    /// Whether the body's own eye at `c` sees `target`, and whether a peek
    /// beside a wall does: the two halves of what the tactics score.
    fn views(sight: &Sight, c: Vec2, target: Vec2) -> (bool, bool) {
        let tile = sight.tile_of(target);
        let mut body = false;
        let mut peek = false;
        for eye in sight.eyes_from(c) {
            if eye.admits(tile) && sight.clear_line(eye.at, tile) {
                if eye.is_peek() {
                    peek = true;
                } else {
                    body = true;
                }
            }
        }
        (body, peek)
    }

    /// A body's charges are counts (task 113): added, spent one at a
    /// time and set outright, nought for a thing that is not a charge,
    /// and nothing is lost off the end.
    #[test]
    fn a_charge_is_a_count_on_the_body_and_a_piece_is_cut_for_one_part() {
        // --- a_charge_is_a_count_on_the_body ---
        {
            let mut gear = Gear::issued();
            // An engineer's sentry kit, by its `ResourceId` code.
            let kit = Item::Stack(16);
            let rifle = Item::Weapon(WeaponKind::AutoRifle.basic());
            assert_eq!(gear.units_of(kit), 0);
            assert!(!gear.spend(kit));
            assert_eq!(gear.add(kit, 7), 7);
            assert_eq!(gear.units_of(kit), 7);
            assert!(gear.spend(kit));
            assert_eq!(gear.units_of(kit), 6);
            gear.set_units(kit, 2);
            assert_eq!(gear.units_of(kit), 2);
            assert_eq!(gear.add(rifle, 1), 0, "a gun is no charge");
            assert_eq!(gear.units_of(rifle), 0);
            assert_eq!(gear.add(Item::Stack(CHARGE_CODES as u32), 1), 0);
        }

        // --- the_armour_is_one_piece_and_does_nothing_once_broken ---
        {
            for kind in ArmourKind::ALL {
                assert_eq!(ArmourKind::from_code(kind.code()), Some(kind));
                assert!(kind.stats().health > 0.0 && kind.stats().protection > 0.0);
            }
            assert_eq!(ArmourKind::from_code(0), None);
            assert_eq!(ArmourKind::ALL, [ArmourKind::Armour]);
            assert_eq!(ArmourKind::Armour.resource(), 7);

            let mut gear = Gear::issued();
            assert_eq!(gear.armour_health(), 0.0);
            let vest = Piece::new(1, ArmourKind::Armour, Tier::One);
            *gear.worn_mut() = Some(vest);
            assert_eq!(gear.worn(), Some(vest));
            assert_eq!(gear.armour_health(), 45.0);
            assert_eq!(vest.effective_protection(), 1.8);
            // Worn down to nothing: still worn, worth nothing.
            gear.worn_mut().as_mut().unwrap().health = 0.0;
            let worn = gear.worn().unwrap();
            assert!(worn.broken());
            assert_eq!(worn.effective_protection(), 0.0);
            assert_eq!(gear.armour_health(), 0.0);
        }
    }

    #[test]
    fn the_enemy_stands_in_cover_where_its_weapon_beats_the_target_s_and_never_in_a_doorway() {
        // --- the_enemy_takes_cover_where_there_is_some_and_the_open_at_range_where_there_is_none ---
        {
            let (sight, nav) = walled_room();
            let stats = WeaponKind::LaserPistol.stats();
            // The target on the right of the wall, the enemy on the left.
            let target = middle(14.0, 8.0);
            let from = middle(3.0, 2.0);
            let stand = Tactics::stand(&sight, &nav, from, &[Some(pistol_at(target))], &stats, &[])
                .expect("somewhere in reach can be shot from");
            let (body, peek) = views(&sight, stand, target);
            assert!(
                !body && peek,
                "cover: the body's eye blind to the target and a peek seeing it, at {stand:?}"
            );
            assert!((stand - target).len() <= stats.reach());
            // The spot is against the wall — a tile beside it.
            let (x, _) = sight.tile_of(stand);
            assert!(x == 9 || x == 11, "against the wall, not {x}");

            // A room with nothing in it: nowhere to take cover, so the open at
            // the greatest distance there is — the pistol reaches 15.4
            // tiles, as far as this room goes, so that is its far corner.
            let (sight, nav) = room_with(&[]);
            let target = middle(15.0, 8.5);
            let from = middle(17.0, 9.0);
            let stand = Tactics::stand(&sight, &nav, from, &[Some(pistol_at(target))], &stats, &[])
                .unwrap();
            let (body, _) = views(&sight, stand, target);
            assert!(body, "the open");
            let d = (stand - target).len();
            assert!(d <= stats.reach() && d > 14.0 * TILE, "{d}");

            // Nothing named: nowhere to stand.
            assert!(Tactics::stand(&sight, &nav, from, &[None], &stats, &[]).is_none());
        }

        // --- an_enemy_stands_where_its_weapon_beats_the_target_s ---
        {
            // A long hall with nothing in it — nowhere to take cover, so the
            // distance is all the weapon has to play with — the target at one
            // end with a pistol, the enemy starting a few tiles from it.
            let interior = Rect::from_min_size(Vec2::ZERO, vec2(40.0 * TILE, 6.0 * TILE));
            let sight = Sight::new(interior, interior, TILE, &[], &[]);
            let nav = Nav::tiled(interior, &[], BODY_MARGIN, TILE);
            let target = pistol_at(middle(1.0, 3.0));
            let from = middle(5.0, 3.0);
            let tiles_off = |stand: Vec2| (stand - target.at).len() / TILE;

            // A sniper rifle is full out to fourteen tiles, short of where
            // the pistol runs out at 15.4: it hangs back just past that, and
            // not at its own 24.5, where its curve has fallen off.
            let sniper = WeaponKind::SniperRifle.stats();
            let stand = Tactics::stand(&sight, &nav, from, &[Some(target)], &sniper, &[]).unwrap();
            let d = tiles_off(stand);
            assert!((13.0..=17.0).contains(&d), "the sniper at {d} tiles");

            // A shotgun is at its best inside four tiles and a pistol is not
            // much worse there than anywhere: it closes.
            let shotgun = WeaponKind::Shotgun.stats();
            let stand = Tactics::stand(&sight, &nav, from, &[Some(target)], &shotgun, &[]).unwrap();
            let d = tiles_off(stand);
            assert!(d <= 4.5, "the shotgun at {d} tiles");

            // An auto rifle reaches 18.2 tiles to the pistol's 15.4:
            // it stands where it can shoot and cannot be shot back at.
            let rifle = WeaponKind::AutoRifle.stats();
            let stand = Tactics::stand(&sight, &nav, from, &[Some(target)], &rifle, &[]).unwrap();
            let d = tiles_off(stand);
            assert!(d > 15.4 && d <= 18.2, "the rifle at {d} tiles");

            // Pistol against pistol is a match, and keeping away is what is
            // left: the far end of its own reach.
            let pistol = WeaponKind::LaserPistol.stats();
            let stand = Tactics::stand(&sight, &nav, from, &[Some(target)], &pistol, &[]).unwrap();
            let d = tiles_off(stand);
            assert!(d > 14.0 && d <= 15.4, "the pistol at {d} tiles");

            // And against a blade a gun keeps out of arm's reach, where the
            // fit is the worst there is, and otherwise stands where it is
            // strongest — the shotgun at its four tiles.
            let blade = Target {
                weapon: WeaponKind::Schword.basic(),
                ..target
            };
            let stand = Tactics::stand(&sight, &nav, from, &[Some(blade)], &shotgun, &[]).unwrap();
            let d = tiles_off(stand);
            assert!(
                d > MELEE_RANGE + 0.5 && d <= 4.5,
                "the shotgun off a blade at {d}"
            );
            assert!(Tactics::fit(&shotgun, &blade.weapon.stats(), 1.0) < 0.0);
            // Past the pistol's 15.4 tiles the sniper has it all its own
            // way; inside them the pistol answers, a little.
            assert!(Tactics::fit(&sniper, &target.weapon.stats(), 17.0) > 0.7);
            assert!(Tactics::fit(&sniper, &target.weapon.stats(), 14.0) > 0.2);
        }

        // --- a_doorway_is_never_a_stand_since_it_opens_for_whoever_comes ---
        {
            // The walled room again, but the bottom two tiles of the wall are
            // a shut door: opaque to the eye, open to the walk.
            let interior = Rect::from_min_size(Vec2::ZERO, vec2(20.0 * TILE, 10.0 * TILE));
            let wall = Rect::from_min_size(vec2(10.0 * TILE, 0.0), vec2(TILE, 5.0 * TILE));
            let door = Rect::from_min_size(vec2(10.0 * TILE, 5.0 * TILE), vec2(TILE, 2.0 * TILE));
            let mut sight = Sight::new(interior, interior, TILE, &[wall], &[]);
            sight.set_shut(&[door]);
            let nav = Nav::tiled(interior, &[wall], BODY_MARGIN, TILE);
            let stats = WeaponKind::LaserPistol.stats();
            let target = middle(14.0, 8.0);
            let from = middle(3.0, 2.0);
            // Told of no door, the tactics take the shut leaf for a wall and
            // stand beside it to peek — a spot the body's own arrival opens.
            let naive = Tactics::stand(&sight, &nav, from, &[Some(pistol_at(target))], &stats, &[])
                .unwrap();
            assert!(
                door.expand(door::REACH).contains(naive),
                "beside the door leaf, at {naive:?}"
            );
            // Told of it, nowhere within the door's reach is a stand, and
            // with no other wall to peek round the open at range it is.
            let stand = Tactics::stand(
                &sight,
                &nav,
                from,
                &[Some(pistol_at(target))],
                &stats,
                &[door],
            )
            .unwrap();
            assert!(
                !door.expand(door::REACH).contains(stand),
                "clear of the doorway, not {stand:?}"
            );
            let (body, _) = views(&sight, stand, target);
            assert!(body, "the open");
            assert!((stand - target).len() <= stats.reach());
        }
    }

    #[test]
    fn a_hostile_bolt_finds_our_own_bodies_and_a_friendly_one_the_targets() {
        let (sight, _) = walled_room();
        let mut combat = Combat::new(7);
        let ours = middle(15.0, 8.0);
        let theirs = middle(15.0, 2.0);
        combat.set_targets(vec![Some((theirs, WeaponKind::LaserPistol.basic()))]);
        // Straight down the corridor at each, from four tiles off, until
        // one lands: the roll is the combat stream's and a miss flies wide.
        let mut own = 0;
        let mut hits = 0;
        for _ in 0..40 {
            combat.fire(
                middle(19.0, 8.0),
                ours,
                WeaponKind::LaserPistol.basic(),
                true,
                false,
            );
            combat.fire(
                middle(19.0, 2.0),
                theirs,
                WeaponKind::LaserPistol.basic(),
                false,
                false,
            );
            for _ in 0..60 {
                combat.step(0.05, &sight, &[Some((ours, false, 0.0))]);
            }
            own += combat.wounds_taken.len();
            combat.wounds_taken.clear();
            hits += combat.take_hits().len();
        }
        assert!(
            own > 20 && own < 40,
            "{own} of 40 hostile bolts landed on us"
        );
        assert!(
            hits > 20 && hits < 40,
            "{hits} of 40 friendly bolts landed on them"
        );

        // A hostile bolt never touches a target, nor a friendly one us.
        combat.fire(
            middle(19.0, 2.0),
            theirs,
            WeaponKind::LaserPistol.basic(),
            true,
            false,
        );
        combat.fire(
            middle(19.0, 8.0),
            ours,
            WeaponKind::LaserPistol.basic(),
            false,
            false,
        );
        for _ in 0..60 {
            combat.step(0.05, &sight, &[Some((ours, false, 0.0))]);
        }
        assert!(combat.wounds_taken.is_empty() && combat.take_hits().is_empty());
        assert!(combat.quiet());

        // A shot is recorded, not flown.
        combat.shoot(ours, theirs, WeaponKind::LaserPistol.basic(), false);
        assert!(combat.bolts.is_empty());
        let shots = combat.take_shots();
        assert_eq!(shots.len(), 1);
        assert_eq!(shots[0].at, theirs);
        assert!(combat.take_shots().is_empty());
    }

    /// Feature 83: a droid's arms are part of the machine. Nothing may
    /// make one, buy one or hold one, and the one thing that holds all
    /// of that is `resource()` being `None` — the armory and the trade
    /// go by the resource.
    #[test]
    fn no_built_in_arm_is_ever_a_thing() {
        for kind in WeaponKind::BUILT_IN {
            assert!(!kind.carried(), "{kind:?} is not carried");
            assert_eq!(kind.resource(), None, "{kind:?} has no resource");
            assert!(!WeaponKind::ALL.contains(&kind));
            assert!(WeaponKind::EVERY.contains(&kind));
            // Its code still reads back, so the app can name it.
            assert_eq!(WeaponKind::from_code(kind.code()), Some(kind));
        }
        // And no resource code anywhere answers with one.
        for code in 0..64 {
            let found = WeaponKind::from_resource(code);
            assert!(
                found.is_none_or(|k| k.carried()),
                "resource {code} answered {found:?}"
            );
        }
        // The seven carried kinds are the whole of what a body holds.
        assert_eq!(WeaponKind::ALL.len(), 7);
        assert_eq!(WeaponKind::ALL.len() + WeaponKind::BUILT_IN.len(), 10);
        assert_eq!(WeaponKind::EVERY.len(), 10);
        for (i, kind) in WeaponKind::EVERY.iter().enumerate() {
            assert_eq!(kind.code(), i as u32 + 1, "EVERY is in code order");
        }
        for kind in WeaponKind::ALL {
            assert!(kind.carried(), "{kind:?} is carried");
        }
    }

    /// The Unmaker's strip, and that no other weapon has one.
    #[test]
    fn only_the_unmaker_strips_and_a_tier_scales_it_like_damage() {
        for kind in WeaponKind::EVERY {
            let stats = kind.stats();
            if kind == WeaponKind::Unmaker {
                assert_eq!(stats.strips, 30.0);
                assert_eq!(stats.strips_far, 20.0);
            } else {
                assert_eq!(stats.strips, 0.0, "{kind:?} strips nothing");
                assert_eq!(stats.strips_far, 0.0);
            }
        }
        let one = WeaponKind::Unmaker.basic().stats();
        let two = WeaponKind::Unmaker.at(Tier::Two).stats();
        // The tier scales the strip by exactly what it scales the damage.
        assert!((two.strips / one.strips - two.damage / one.damage).abs() < 1e-5);
        assert!((two.strips - 30.0 * crate::balance::TIER_TWO_DAMAGE).abs() < 1e-4);
        // Out to the sweet spot it strips its best; at the range, its far.
        assert_eq!(one.strips_at(0.0), 30.0);
        assert_eq!(one.strips_at(7.0), 30.0);
        assert_eq!(one.strips_at(14.0), 20.0);
        assert!((one.strips_at(10.5) - 25.0).abs() < 1e-4);
    }

    #[test]
    fn the_curves_pin_the_numbers_the_guns_were_asked_for() {
        // Every kind is its own code and its own resource, both ways.
        for kind in WeaponKind::ALL {
            assert_eq!(WeaponKind::from_code(kind.code()), Some(kind));
            let resource = kind.resource().expect("a carried weapon is a resource");
            assert_eq!(WeaponKind::from_resource(resource), Some(kind));
        }
        assert_eq!(WeaponKind::from_code(0), None);
        assert_eq!(WeaponKind::from_resource(6), None, "a helm is not a gun");
        assert_eq!(WeaponKind::LaserPistol.resource(), Some(3));

        // The second tuning of September 2026: the pistol's odds a tenth
        // down from the 95% and 65% it had, its damage a fifth up from 6,
        // and ten tiles more range — 72% at ten tiles then, where the app
        // used to print 70%; 68% since October 2026 cut its reach to 15.4. Its
        // damage back to 6 the same month, when every click became a shot.
        let pistol = WeaponKind::LaserPistol.stats();
        assert_eq!(pistol.range, 15.4);
        assert!((pistol.hit_chance(10.0) - 0.680).abs() < 0.01);
        assert_eq!(pistol.hit_chance(0.0), 0.855);
        assert_eq!(pistol.hit_chance(30.0), 0.585, "no worse past the range");
        assert_eq!(pistol.damage_at(11.0), 6.0);
        assert!((pistol.dps() - 9.0).abs() < 1e-5);

        let shotgun = WeaponKind::Shotgun.stats();
        assert_eq!(shotgun.damage_at(4.0), 60.0);
        assert_eq!(shotgun.damage_at(2.0), 60.0);
        assert!((shotgun.damage_at(7.0) - 48.0).abs() < 1e-3);
        assert_eq!(shotgun.damage_at(10.0), 36.0);
        assert_eq!(shotgun.hit_chance(4.0), 0.81);
        assert!((shotgun.hit_chance(10.0) - 0.54).abs() < 1e-6);

        let sniper = WeaponKind::SniperRifle.stats();
        assert_eq!(sniper.hit_chance(14.0), 0.9);
        assert_eq!(sniper.damage_at(14.0), 54.0);
        assert!((sniper.hit_chance(24.5) - 0.63).abs() < 1e-6);
        assert_eq!(sniper.damage_at(24.5), 30.0);

        // No burst: four fives a second, steadily (four threes, as much
        // a second as the eight sixes every four seconds it had, until
        // each shot gained 2); the rifle reaches 18.2 tiles, full to 5.6
        // (twenty-six and eight before October 2026 took three tenths
        // off every reach bar the shotgun's).
        let rifle = WeaponKind::AutoRifle.stats();
        assert_eq!((rifle.burst, rifle.fire_rate), (1, 4.0));
        assert_eq!((rifle.damage, rifle.damage_far), (5.0, 4.4));
        assert_eq!((rifle.range, rifle.sweet), (18.2, 5.6));
        assert!((rifle.dps() - 20.0).abs() < 1e-5);

        // A blade reaches a tile and a bit, and swings every two seconds.
        let blade = WeaponKind::Schword.stats();
        assert!(blade.melee);
        assert_eq!(blade.reach(), MELEE_RANGE * TILE);
        assert_eq!(blade.damage_at(1.0), 42.0);
        assert!((1.0 / blade.fire_rate - MELEE_PERIOD).abs() < 1e-6);
        assert!(!pistol.melee && !shotgun.melee && !sniper.melee && !rifle.melee);

        // Task 115: the minigun at its lowest tier, two, and at three.
        let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
        let mini = WeaponKind::Minigun.basic();
        assert_eq!(mini.tier, Tier::Two);
        let two = mini.stats();
        assert!(
            close(two.damage, 5.5) && close(two.damage_far, 4.0),
            "{two:?}"
        );
        assert!(close(two.accuracy, 0.85) && close(two.accuracy_far, 0.45));
        assert_eq!((two.range, two.sweet), (14.0, 4.2));
        assert_eq!((two.burst, two.burst_gap, two.fire_rate), (20, 0.1, 0.2));
        assert_eq!(two.speed, 24.0);
        assert!(!two.melee && two.strips == 0.0);
        assert!((two.dps_at(4.2) - 18.7).abs() < 0.01, "{}", two.dps_at(4.2));
        let three = WeaponKind::Minigun.at(Tier::Three).stats();
        assert!(close(three.damage, 6.875) && close(three.damage_far, 5.0));
        assert!(close(three.accuracy, 0.8925) && close(three.accuracy_far, 0.4725));
        assert!(close(three.range, 16.8) && close(three.sweet, 5.04));

        // The rail lance at its only tier, three.
        let lance = WeaponKind::RailLance.basic();
        assert_eq!(lance.tier, Tier::Three);
        let three = lance.stats();
        assert!(
            close(three.damage, 75.0) && close(three.damage_far, 50.0),
            "{three:?}"
        );
        assert!(close(three.accuracy, 0.945) && close(three.accuracy_far, 0.6825));
        assert!(close(three.sweet, 16.8) && close(three.range, 28.56));
        assert_eq!((three.burst, three.fire_rate, three.speed), (1, 0.2, 70.0));
        assert!(!three.melee && three.strips == 0.0);

        // The worked figures in `balance`'s notes: a second's damage in
        // the sweet range, body hits on whole armour.
        let dps = |w: Weapon, protection: f32| {
            let s = w.stats();
            s.burst as f32 * s.fire_rate * (s.damage - protection).max(0.0) * s.accuracy.min(1.0)
        };
        let rifle_two = WeaponKind::AutoRifle.at(Tier::Two);
        assert!((dps(mini, 0.0) - 18.7).abs() < 0.05);
        assert!((dps(rifle_two, 0.0) - 23.9).abs() < 0.05);
        assert!((dps(mini, 3.0) - 8.5).abs() < 0.05);
        assert!((dps(rifle_two, 3.0) - 12.4).abs() < 0.05);
        assert!((dps(mini, 4.5) - 3.4).abs() < 0.05);
        assert!((dps(rifle_two, 4.5) - 6.7).abs() < 0.05);
        let sniper_three = WeaponKind::SniperRifle.at(Tier::Three);
        assert!((dps(sniper_three, 0.0) - 21.1).abs() < 0.05);
        assert!((dps(lance, 0.0) - 14.2).abs() < 0.05);
        assert!((dps(lance, 0.0) * (1.0 + 0.6 + 0.36) - 27.8).abs() < 0.05);
    }

    /// Task 115: every kind has a lowest tier, and its `basic` is at it.
    #[test]
    fn a_kind_is_never_made_below_its_lowest_tier() {
        for kind in [
            WeaponKind::LaserPistol,
            WeaponKind::Shotgun,
            WeaponKind::AutoRifle,
            WeaponKind::SniperRifle,
            WeaponKind::Schword,
        ] {
            assert_eq!(kind.min_tier(), Tier::One);
            assert_eq!(
                kind.basic(),
                Weapon {
                    kind,
                    tier: Tier::One
                }
            );
        }
        assert_eq!(WeaponKind::Minigun.min_tier(), Tier::Two);
        assert_eq!(WeaponKind::RailLance.min_tier(), Tier::Three);
        assert!(!WeaponKind::Minigun.made_at(Tier::One));
        assert!(WeaponKind::Minigun.made_at(Tier::Two));
        assert!(!WeaponKind::RailLance.made_at(Tier::Two));
        assert!(WeaponKind::RailLance.made_at(Tier::Three));
        // Nobody else carries one: the issue, the hire and the garrison
        // tables name neither, so no roll can land on one.
        for kind in [WeaponKind::Minigun, WeaponKind::RailLance] {
            assert!(ISSUE_ODDS.iter().all(|(k, _)| *k != kind));
            assert!(MERCENARY_ODDS.iter().all(|(k, _)| *k != kind));
            assert!(MANUFACTURER_ODDS.iter().all(|(k, _)| *k != kind));
        }
        for seed in 0..500u64 {
            for gear in [
                Gear::issued_for(seed),
                Gear::hired_for(seed, 1),
                Gear::manufacturer(seed, Some(Tier::Two), None, 1),
            ] {
                let kind = gear.weapon.map(|w| w.kind);
                assert!(
                    !matches!(kind, Some(WeaponKind::Minigun | WeaponKind::RailLance)),
                    "seed {seed}: {kind:?}"
                );
            }
        }
        // And no machine's arm is one, whatever its place in a wave.
        for droid in crate::droid::DroidKind::EVERY {
            for index in 0..64 {
                let arm = droid.arm(index);
                assert!(
                    !matches!(arm, WeaponKind::Minigun | WeaponKind::RailLance),
                    "{droid:?} {index}: {arm:?}"
                );
            }
        }
    }

    /// A pistol below its lowest tier is fine; a minigun at tier one is
    /// never made, and a debug build says so.
    #[test]
    #[should_panic(expected = "never made below")]
    #[cfg(debug_assertions)]
    fn a_minigun_at_tier_one_is_refused() {
        let _ = WeaponKind::Minigun.at(Tier::One);
    }

    /// Task 115: one pull of a minigun is twenty bolts a tenth of a second
    /// apart, and the next pull comes five seconds after the first.
    #[test]
    fn a_minigun_pull_is_twenty_bolts_a_tenth_apart_and_the_next_five_seconds_on() {
        let stats = WeaponKind::Minigun.basic().stats();
        let mut trigger = Trigger::default();
        let dt = 1.0 / 240.0;
        let mut times = Vec::new();
        for step in 0..(240 * 11) {
            trigger.tick(dt);
            if trigger.pull(dt, &stats) {
                times.push(step as f32 * dt);
            }
        }
        let first: Vec<f32> = times.iter().copied().filter(|&t| t < 4.0).collect();
        assert_eq!(first.len(), 20, "{times:?}");
        for pair in first.windows(2) {
            assert!(
                (pair[1] - pair[0] - 0.1).abs() <= dt * 1.5,
                "a tenth apart: {first:?}"
            );
        }
        assert!((first[19] - 1.9).abs() < 0.02, "{first:?}");
        let next = times.iter().copied().find(|&t| t >= 4.0).unwrap();
        assert!(
            (next - 5.0).abs() <= dt * 1.5,
            "the next pull at five: {next}"
        );
        assert_eq!(
            times.iter().filter(|&&t| (4.0..9.0).contains(&t)).count(),
            20
        );
    }

    /// Fire `weapon` from `from` at `at` until a shot is rolled a hit —
    /// a miss is aimed wide of the whole line — and fly it out, handing
    /// back what it struck.
    fn fly_a_hit(combat: &mut Combat, sight: &Sight, from: Vec2, at: Vec2) -> Vec<Hit> {
        for _ in 0..50 {
            combat.fire(from, at, WeaponKind::RailLance.basic(), false, false);
            for _ in 0..240 {
                combat.step(1.0 / 60.0, sight, &[]);
            }
            let hits = combat.take_hits();
            if !hits.is_empty() {
                return hits;
            }
        }
        panic!("fifty lance slugs and not one rolled a hit");
    }

    /// Task 115: a rail lance slug goes through three bodies in a line —
    /// all three struck, at one, 0.6 and 0.36 of the damage — and no
    /// fourth; it never strikes one twice and a wall stops it.
    #[test]
    fn a_lance_slug_goes_through_three_bodies_in_a_line_and_stops_at_a_wall() {
        let lance = WeaponKind::RailLance.basic().stats();
        let line = |xs: &[f32]| -> Vec<Option<(Vec2, Weapon)>> {
            xs.iter()
                .map(|&x| Some((middle(x, 5.0), WeaponKind::LaserPistol.basic())))
                .collect()
        };
        {
            let (sight, _) = room_with(&[]);
            let mut combat = Combat::new(3);
            combat.set_targets(line(&[4.0, 6.0, 8.0, 10.0]));
            let hits = fly_a_hit(&mut combat, &sight, middle(1.0, 5.0), middle(4.0, 5.0));
            let who: Vec<usize> = hits.iter().map(|h| h.who).collect();
            assert_eq!(who, vec![0, 1, 2], "three struck, in order, no fourth");
            // All inside the sweet range, so the curve is flat.
            let full = lance.damage;
            for (hit, share) in hits.iter().zip([1.0, 0.6, 0.36]) {
                assert!((hit.damage - full * share).abs() < 1e-3, "{hits:?}");
            }
            assert!(combat.bolts.is_empty(), "spent after the third");
        }
        // A wall between the second and the third: the second is the last.
        {
            let (sight, _) = walled_room();
            let mut combat = Combat::new(3);
            combat.set_targets(line(&[6.0, 8.0, 12.0]));
            let hits = fly_a_hit(&mut combat, &sight, middle(2.0, 5.0), middle(6.0, 5.0));
            let who: Vec<usize> = hits.iter().map(|h| h.who).collect();
            assert_eq!(who, vec![0, 1], "the wall at ten stops it");
        }
        // Every other gun stops at the first body.
        {
            let (sight, _) = room_with(&[]);
            let mut combat = Combat::new(3);
            combat.set_targets(line(&[4.0, 6.0]));
            let mut hits = Vec::new();
            for _ in 0..20 {
                combat.fire(
                    middle(1.0, 5.0),
                    middle(4.0, 5.0),
                    WeaponKind::SniperRifle.basic(),
                    false,
                    false,
                );
                for _ in 0..120 {
                    combat.step(1.0 / 60.0, &sight, &[]);
                }
                hits.extend(combat.take_hits());
            }
            assert!(!hits.is_empty() && hits.iter().all(|h| h.who == 0));
        }
    }

    /// Task 115: a Guardian's shield stops a lance slug from the front —
    /// nothing behind it is struck — and a Guardian hit from the side is
    /// gone through. A body that dodges the slug is no strike: the next
    /// body struck takes the factor for the strikes so far.
    #[test]
    fn a_shield_stops_a_lance_from_the_front_and_a_dodge_is_no_strike() {
        let lance = WeaponKind::RailLance.basic().stats().damage;
        let from = middle(1.0, 5.0);
        let targets = vec![
            Some((middle(4.0, 5.0), WeaponKind::Sweeper.basic())),
            Some((middle(7.0, 5.0), WeaponKind::LaserPistol.basic())),
        ];
        // Facing the shooter: stopped at the plate, nothing struck at all.
        {
            let (sight, _) = room_with(&[]);
            let mut combat = Combat::new(9);
            combat.set_targets(targets.clone());
            combat.set_shields(&[Some(vec2(-1.0, 0.0)), None]);
            for _ in 0..20 {
                combat.fire(
                    from,
                    middle(4.0, 5.0),
                    WeaponKind::RailLance.basic(),
                    false,
                    false,
                );
                for _ in 0..120 {
                    combat.step(1.0 / 60.0, &sight, &[]);
                }
            }
            assert!(combat.take_hits().is_empty(), "never through a shield");
        }
        // Facing across the line: gone through, and the one behind struck.
        {
            let (sight, _) = room_with(&[]);
            let mut combat = Combat::new(9);
            combat.set_targets(targets.clone());
            combat.set_shields(&[Some(vec2(0.0, 1.0)), None]);
            let hits = fly_a_hit(&mut combat, &sight, from, middle(4.0, 5.0));
            let who: Vec<usize> = hits.iter().map(|h| h.who).collect();
            assert_eq!(who, vec![0, 1]);
        }
        // The first body's armour slips it every time: not a strike, so
        // the second takes it whole and the third at 0.6.
        {
            let (sight, _) = room_with(&[]);
            let mut combat = Combat::new(9);
            combat.set_targets(vec![
                Some((middle(4.0, 5.0), WeaponKind::LaserPistol.basic())),
                Some((middle(6.0, 5.0), WeaponKind::LaserPistol.basic())),
                Some((middle(8.0, 5.0), WeaponKind::LaserPistol.basic())),
            ]);
            combat.set_dodge(&[1.0, 0.0, 0.0]);
            let hits = fly_a_hit(&mut combat, &sight, from, middle(4.0, 5.0));
            let got: Vec<(usize, f32)> = hits.iter().map(|h| (h.who, h.damage)).collect();
            assert_eq!(got.len(), 2, "{got:?}");
            assert_eq!((got[0].0, got[1].0), (1, 2));
            assert!((got[0].1 - lance).abs() < 1e-3 && (got[1].1 - lance * 0.6).abs() < 1e-3);
        }
        // And one peeking from cover that dodges it: the same, whenever
        // the peek's half comes up.
        {
            let (sight, _) = room_with(&[]);
            let mut combat = Combat::new(21);
            combat.set_targets(vec![
                Some((middle(4.0, 5.0), WeaponKind::LaserPistol.basic())),
                Some((middle(6.0, 5.0), WeaponKind::LaserPistol.basic())),
                Some((middle(8.0, 5.0), WeaponKind::LaserPistol.basic())),
            ]);
            combat.set_peeking(&[false, true, false]);
            let mut dodged = 0;
            for _ in 0..30 {
                let hits = fly_a_hit(&mut combat, &sight, from, middle(4.0, 5.0));
                let who: Vec<usize> = hits.iter().map(|h| h.who).collect();
                if who == vec![0, 2] {
                    dodged += 1;
                    assert!((hits[1].damage - lance * 0.6).abs() < 1e-3, "{hits:?}");
                } else {
                    assert_eq!(who, vec![0, 1, 2]);
                }
            }
            assert!(dodged > 5, "the peek slipped it {dodged} times of thirty");
        }
    }

    /// Task 115: the same seed, the same strikes.
    #[test]
    fn a_lance_on_one_seed_strikes_the_same_every_time() {
        let run = || {
            let (sight, _) = room_with(&[]);
            let mut combat = Combat::new(77);
            combat.set_targets(
                [4.0, 6.0, 8.0]
                    .iter()
                    .map(|&x| Some((middle(x, 5.0), WeaponKind::LaserPistol.basic())))
                    .collect(),
            );
            let mut all = Vec::new();
            for _ in 0..12 {
                combat.fire(
                    middle(1.0, 5.0),
                    middle(4.0, 5.0),
                    WeaponKind::RailLance.basic(),
                    false,
                    false,
                );
                for _ in 0..90 {
                    combat.step(1.0 / 60.0, &sight, &[]);
                }
                all.extend(combat.take_hits());
            }
            all
        };
        let a = run();
        assert!(!a.is_empty());
        assert_eq!(a, run());
    }

    /// A line of sandbags across the room is no wall — walked over, seen
    /// over — but the tactics stand a body close behind it, and a bolt
    /// coming over it at that body is dodged half the time, like a peek's.
    #[test]
    fn damage_falls_off_with_distance_and_a_body_peeking_or_behind_sandbags_dodges_half() {
        // --- damage_falls_off_with_the_distance_the_bolt_flew ---
        {
            let (sight, _) = room_with(&[]);
            let mut combat = Combat::new(11);
            // A shotgun from two tiles, and one from nine: every hit from
            // close by is the full hundred, every one from far off is less.
            let theirs = middle(10.0, 5.0);
            combat.set_targets(vec![Some((theirs, WeaponKind::LaserPistol.basic()))]);
            for (from, near) in [(middle(8.0, 5.0), true), (middle(1.0, 5.0), false)] {
                let mut hits = Vec::new();
                for _ in 0..40 {
                    combat.fire(from, theirs, WeaponKind::Shotgun.basic(), false, false);
                    for _ in 0..60 {
                        combat.step(0.05, &sight, &[]);
                    }
                    hits.extend(combat.take_hits());
                }
                assert!(!hits.is_empty());
                assert!(hits.iter().all(|h| !h.cut), "a shot is not a cut");
                if near {
                    assert!(
                        hits.iter().all(|h| (h.damage - 60.0).abs() < 1e-3),
                        "{hits:?}"
                    );
                } else {
                    assert!(
                        hits.iter().all(|h| h.damage < 45.0 && h.damage > 36.0),
                        "nine tiles, less the body's edge: {hits:?}"
                    );
                }
            }
        }

        // --- a_body_peeking_from_cover_dodges_half_the_bolts ---
        {
            let (sight, _) = room_with(&[]);
            let mut combat = Combat::new(5);
            let ours = middle(10.0, 5.0);
            let from = middle(8.0, 5.0);
            // Straight at it from two tiles, where the pistol lands four in
            // five: standing in the open near enough all of them land, and
            // peeking about half.
            let landed = |combat: &mut Combat, peeking: bool| {
                let mut own = 0;
                for _ in 0..400 {
                    combat.fire(from, ours, WeaponKind::LaserPistol.basic(), true, false);
                    for _ in 0..40 {
                        combat.step(0.05, &sight, &[Some((ours, peeking, 0.0))]);
                    }
                    own += combat.wounds_taken.len();
                    combat.wounds_taken.clear();
                }
                own
            };
            let open = landed(&mut combat, false);
            let cover = landed(&mut combat, true);
            assert!(open > 310 && open <= 400, "{open} of 400 in the open");
            assert!(cover > 140 && cover < 220, "{cover} of 400 in cover");

            // The same for a target peeking at us.
            combat.set_targets(vec![Some((ours, WeaponKind::LaserPistol.basic()))]);
            combat.set_peeking(&[true]);
            assert!(combat.targets()[0].unwrap().peeking);
            let mut hits = 0;
            for _ in 0..400 {
                combat.fire(from, ours, WeaponKind::LaserPistol.basic(), false, false);
                for _ in 0..40 {
                    combat.step(0.05, &sight, &[]);
                }
                hits += combat.take_hits().len();
            }
            assert!(
                hits > 140 && hits < 220,
                "{hits} of 400 on a peeking target"
            );
            // Named again, nobody is peeking until said.
            combat.set_targets(vec![Some((ours, WeaponKind::LaserPistol.basic()))]);
            assert!(!combat.targets()[0].unwrap().peeking);
        }

        // --- sandbags_are_a_stand_the_tactics_take_and_half_the_bolts_over_them_are_dodged ---
        {
            let interior = Rect::from_min_size(Vec2::ZERO, vec2(20.0 * TILE, 10.0 * TILE));
            // Bags down column 10, the top seven tiles, nothing solid at all.
            let bags = Rect::from_min_size(vec2(10.0 * TILE, 0.0), vec2(TILE, 7.0 * TILE));
            let mut sight = Sight::new(interior, interior, TILE, &[], &[]);
            sight.set_cover(&[bags]);
            let nav = Nav::tiled(interior, &[], BODY_MARGIN, TILE);
            let stats = WeaponKind::LaserPistol.stats();
            let target = middle(14.0, 3.0);
            let from = middle(3.0, 3.0);
            // Walkable: a route from one side to the other runs straight
            // through the bags.
            assert!(nav.can_reach(from, target));
            let plan = |taken: &[Vec2]| {
                Tactics::stand_with_cover(
                    &sight,
                    &nav,
                    from,
                    &[Some(pistol_at(target))],
                    &stats,
                    &[],
                    taken,
                    false,
                    DISTANCE_WORTH,
                )
                .expect("somewhere to shoot from")
            };
            let stand = plan(&[]);
            assert!(stand.cover, "the bags are cover: {:?}", stand.at);
            let (x, y) = sight.tile_of(stand.at);
            // The tile beside the bags or the one behind it: both are within
            // the reach, and the nearer walk wins.
            assert!(
                x == 8 || x == 9,
                "just this side of the bags, at ({x}, {y})"
            );
            // A squadmate already on that tile: the next one picks another,
            // still behind the bags.
            let second = plan(&[stand.at]);
            assert!((second.at - stand.at).len() >= TILE, "not the same tile");
            assert!(
                second.cover,
                "the next tile of the barricade: {:?}",
                second.at
            );
            assert!(matches!(sight.tile_of(second.at).0, 8 | 9));
            assert!(sight.covered(stand.at, target));
            // And the body sees the target from there — no peek needed.
            let (body, _) = views(&sight, stand.at, target);
            assert!(body, "seen over the bags");

            // Bolts over the bags: a body at the stand is dodged about half of
            // them; one standing on the bags' own tile takes them all.
            let landed = |sight: &Sight, at: Vec2| {
                let mut combat = Combat::new(7);
                let shooter = middle(14.0, 3.0);
                let mut own = 0;
                for _ in 0..400 {
                    combat.fire(shooter, at, WeaponKind::LaserPistol.basic(), true, false);
                    for _ in 0..80 {
                        combat.step(0.05, sight, &[Some((at, false, 0.0))]);
                    }
                    own += combat.wounds_taken.len();
                    combat.wounds_taken.clear();
                }
                own
            };
            let behind = landed(&sight, middle(9.0, 3.0));
            let back = landed(&sight, middle(8.0, 3.0));
            let past = landed(&sight, middle(7.0, 3.0));
            let on_top = landed(&sight, middle(10.0, 3.0));
            assert!(on_top > 300, "{on_top} of 400 on the bags");
            assert!(
                behind > on_top / 2 - 40 && behind < on_top / 2 + 40,
                "{behind} of 400 behind them, {on_top} on them"
            );
            assert!(
                back > on_top / 2 - 40 && back < on_top / 2 + 40,
                "{back} of 400 a tile back from them, {on_top} on them"
            );
            // Seven tiles from the shooter, where the pistol lands fewer
            // than on the bags (more so since October 2026 cut its reach),
            // but nothing like half.
            assert!(past > 260, "{past} of 400 two tiles back: past the reach");
        }
    }

    #[test]
    fn an_issued_gear_rolls_every_kind_across_seeds_and_the_same_for_a_seed() {
        let mut seen = [0u32; 5];
        for seed in 0..400u64 {
            let gear = Gear::issued_for(seed);
            let weapon = gear.weapon.expect("always armed");
            // A gun and nothing to wear; a blade and the basic armour,
            // since a melee bot always wears it.
            if weapon.stats().melee {
                assert_eq!(gear.armour_health(), 45.0, "the basic armour on a blade");
                assert!(gear.worn().is_some_and(|w| !w.broken()));
            } else {
                assert_eq!(gear.armour_health(), 0.0, "nothing to wear");
            }
            assert!(gear.charges.iter().all(|&n| n == 0));
            seen[weapon.kind.code() as usize - 1] += 1;
            assert_eq!(
                Gear::issued_for(seed).weapon,
                Some(weapon),
                "a function of the seed"
            );
        }
        assert!(seen.iter().all(|&n| n > 0), "{seen:?}");
        // The pistol is the common one, and the sniper rifle the rare.
        assert!(seen[0] > seen[1] && seen[1] > seen[3], "{seen:?}");
        assert_eq!(Gear::issued().weapon, Some(WeaponKind::LaserPistol.basic()));
    }

    #[test]
    fn a_blade_charges_the_nearest_target_and_a_gun_is_locked_only_by_a_blade() {
        let (sight, nav) = walled_room();
        let blade = WeaponKind::Schword.stats();
        let pistol = WeaponKind::LaserPistol.stats();
        let from = middle(3.0, 2.0);
        let near = middle(6.0, 8.0);
        let far = middle(15.0, 8.0);
        let stand = Tactics::stand(
            &sight,
            &nav,
            from,
            &[Some(pistol_at(far)), Some(pistol_at(near))],
            &blade,
            &[],
        )
        .expect("somewhere beside a target");
        assert!(
            (stand - near).len() <= MELEE_RANGE * TILE,
            "beside the nearer one, not {stand:?}"
        );
        assert!(Tactics::stand(&sight, &nav, from, &[None], &blade, &[]).is_none());

        // A melee is a distance: a blade a tile off locks a gunner, a
        // pistol a tile off does not, and a blade too far off does not.
        let mut combat = Combat::new(3);
        let body = middle(10.0, 8.0);
        let beside = middle(11.0, 8.0);
        combat.set_targets(vec![Some((beside, WeaponKind::Schword.basic()))]);
        assert_eq!(combat.melee_with(&sight, body, &pistol), Some(0));
        combat.set_targets(vec![Some((beside, WeaponKind::LaserPistol.basic()))]);
        assert_eq!(combat.melee_with(&sight, body, &pistol), None);
        assert_eq!(
            combat.melee_with(&sight, body, &blade),
            Some(0),
            "a blade takes on anyone"
        );
        combat.set_targets(vec![Some((middle(13.0, 8.0), WeaponKind::Schword.basic()))]);
        assert_eq!(
            combat.melee_with(&sight, body, &pistol),
            None,
            "out of reach"
        );

        // A blow: a hit on the target in the crew's room, a melee shot in
        // a hostile one, and nothing in the air either way.
        combat.set_targets(vec![Some((beside, WeaponKind::Schword.basic()))]);
        combat.brawl(
            body,
            0,
            WeaponKind::LaserPistol.basic(),
            FIST_DAMAGE,
            false,
            false,
            None,
            FIST_DAMAGE,
        );
        combat.brawl(
            body,
            0,
            WeaponKind::Schword.basic(),
            70.0,
            true,
            true,
            None,
            70.0,
        );
        assert!(combat.bolts.is_empty());
        let hits = combat.take_hits();
        assert_eq!(hits.len(), 1);
        assert_eq!(
            (hits[0].who, hits[0].damage, hits[0].cut),
            (0, FIST_DAMAGE, false)
        );
        let shots = combat.take_shots();
        assert_eq!(shots.len(), 1);
        assert!(shots[0].melee && shots[0].cut && shots[0].damage == 70.0);
        assert_eq!(shots[0].at, beside);
        // A blow the world carried in lands on us, on a part, as a cut.
        let blow = combat.struck(1, 70.0, true);
        assert!(blow.who == 1 && blow.cut && blow.damage == 70.0);
        assert!(combat.wounds_taken.is_empty(), "the game applies it");
    }

    /// The tiers scale the kind's numbers as asked — a quarter of damage
    /// and accuracy at two, the same again with a twentieth of accuracy
    /// and a fifth of range at three, half again of armour a tier — and
    /// a body in tier-three armour dodges bolts in the open where one in
    /// anything less takes every one; a target's dodge is handed across
    /// like its peeking.
    #[test]
    fn a_tier_scales_the_numbers_and_tier_three_armour_dodges() {
        let base = WeaponKind::LaserPistol.stats();
        let two = WeaponKind::LaserPistol.at(Tier::Two).stats();
        let three = WeaponKind::LaserPistol.at(Tier::Three).stats();
        assert_eq!(WeaponKind::LaserPistol.basic().stats(), base);
        assert!((two.damage - base.damage * 1.25).abs() < 1e-5);
        assert!((two.accuracy_far - base.accuracy_far * 1.25).abs() < 1e-5);
        assert_eq!(two.accuracy, 1.0, "0.855 and a quarter is capped");
        assert_eq!(two.range, base.range);
        assert!((three.damage - base.damage * 1.25 * 1.25).abs() < 1e-5);
        assert!((three.accuracy_far - base.accuracy_far * 1.25 * 1.05).abs() < 1e-5);
        assert!((three.range - base.range * 1.2).abs() < 1e-5);
        // The odds never pass one: the sniper's 0.9 at two is capped.
        assert_eq!(WeaponKind::SniperRifle.at(Tier::Two).stats().accuracy, 1.0);
        let blade = WeaponKind::Schword.at(Tier::Three).stats();
        assert!((blade.range - MELEE_RANGE * 1.2).abs() < 1e-5 && blade.melee);

        let helm = Piece::new(1, ArmourKind::Armour, Tier::Two);
        assert_eq!(helm.stats().health, balance::ARMOUR.health * 1.5);
        assert_eq!(
            helm.health,
            helm.stats().health,
            "fresh at the tier's health"
        );
        assert_eq!(
            helm.effective_protection(),
            balance::ARMOUR.protection * 1.5
        );
        assert_eq!(helm.dodge(), 0.0);
        let gold = Piece::new(2, ArmourKind::Armour, Tier::Three);
        assert_eq!(gold.stats().protection, balance::ARMOUR.protection * 2.25);
        assert_eq!(gold.dodge(), balance::TIER_THREE_DODGE);
        let mut gear = Gear::default();
        assert_eq!(gear.dodge(), 0.0);
        gear.armour = Some(gold);
        assert!((gear.dodge() - 0.271).abs() < 1e-6);
        for t in Tier::ALL {
            assert_eq!(Tier::from_code(t.code()), Some(t));
        }
        assert_eq!(Tier::Three.next(), None);
        assert_eq!(Tier::One.next(), Some(Tier::Two));

        // Point blank in the open, a body with no dodge takes every bolt
        // and one in gold armour slips about a tenth.
        let (sight, _) = room_with(&[]);
        let mut combat = Combat::new(5);
        let ours = middle(10.0, 5.0);
        let from = middle(9.0, 5.0);
        let landed = |combat: &mut Combat, dodge: f32| {
            let mut own = 0;
            for _ in 0..400 {
                combat.fire(from, ours, WeaponKind::LaserPistol.basic(), true, false);
                for _ in 0..40 {
                    combat.step(0.05, &sight, &[Some((ours, false, dodge))]);
                }
                own += combat.wounds_taken.len();
                combat.wounds_taken.clear();
            }
            own
        };
        let plain = landed(&mut combat, 0.0);
        let gold = landed(&mut combat, 0.5);
        assert!(plain > 330, "{plain} of 400 with nothing to dodge with");
        assert!(gold > 140 && gold < 220, "{gold} of 400 dodging half");
        // And a target's, handed across after the targets like its peeking.
        combat.set_targets(vec![Some((ours, WeaponKind::LaserPistol.basic()))]);
        assert_eq!(combat.targets()[0].unwrap().dodge, 0.0);
        combat.set_dodge(&[0.5]);
        assert_eq!(combat.targets()[0].unwrap().dodge, 0.5);
        let mut hits = 0;
        for _ in 0..400 {
            combat.fire(from, ours, WeaponKind::LaserPistol.basic(), false, false);
            for _ in 0..40 {
                combat.step(0.05, &sight, &[]);
            }
            hits += combat.take_hits().len();
        }
        assert!(
            hits > 140 && hits < 220,
            "{hits} of 400 on a dodging target"
        );
    }

    /// A tank's Riot Shield (task 155): a hostile bolt meeting the plate
    /// from the front stops there, nothing rolled on him, its damage
    /// handed over as a block, and goes back as his own — head on, back
    /// at the shooter, who is hit by him; at a slant, out at the angle it
    /// came in. From behind the plate is nothing, a spent plate stops
    /// nothing, and a friendly bolt never asks it.
    #[test]
    fn a_riot_shield_stops_a_bolt_from_the_front_and_bounces_it_back() {
        let (sight, _) = room_with(&[]);
        let pistol = WeaponKind::LaserPistol.basic();
        let tank = middle(10.0, 5.0);
        let shooter = middle(4.0, 5.0);
        let west = vec2(-1.0, 0.0);
        let plate = |hp: f32| Plate {
            who: 0,
            facing: west,
            hp,
        };
        let bodies = [Some((tank, false, 0.0))];
        // One bolt loosed straight down a line, signed by target 0.
        let volley = |plates: Vec<Plate>, from: Vec2, dir: Vec2| {
            let mut combat = Combat::new(7);
            combat.set_targets(vec![Some((shooter, pistol))]);
            combat.set_plates(plates);
            combat.loose(from, dir, pistol, true, &Skill::NONE, None);
            combat.bolts[0].shooter = Some(0);
            combat
        };
        // Head on: stopped at the plate, the damage blocked, and back down
        // the same line onto the shooter as the tank's.
        let mut combat = volley(vec![plate(100.0)], shooter, vec2(1.0, 0.0));
        let mut bounced = None;
        for _ in 0..60 {
            combat.step(0.05, &sight, &bodies);
            if bounced.is_none()
                && let Some(b) = combat.bolts.iter().find(|b| !b.hostile)
            {
                bounced = Some(*b);
            }
        }
        assert!(combat.wounds_taken.is_empty(), "nothing reaches the tank");
        let blocks = combat.take_plate_blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, 0);
        assert!(blocks[0].1 > 0.0, "the plate takes the bolt's damage");
        let b = bounced.expect("a bolt bounced off the plate");
        assert_eq!(b.by, Some(0), "the bounce is the tank's");
        assert!(
            (b.pos.x - (tank.x - PLATE_OUT)).abs() < 1.0,
            "off the plate"
        );
        assert!(b.vel.normalize_or_zero().dot(west) > 0.999, "straight back");
        let hits = combat.take_hits();
        assert_eq!(hits.len(), 1, "the shooter is hit by its own bolt");
        assert_eq!((hits[0].who, hits[0].by), (0, Some(0)));
        // At a slant: the angle out is the angle in, mirrored in the face.
        let dir = vec2(3.0, 1.0).normalize_or_zero();
        let from = tank + west * PLATE_OUT - dir * (4.0 * TILE);
        let mut combat = volley(vec![plate(100.0)], from, dir);
        let mut out = None;
        for _ in 0..20 {
            combat.step(0.05, &sight, &bodies);
            if out.is_none()
                && let Some(b) = combat.bolts.iter().find(|b| !b.hostile)
            {
                out = Some(b.vel.normalize_or_zero());
            }
        }
        let out = out.expect("bounced");
        assert!((out.x + dir.x).abs() < 1e-4 && (out.y - dir.y).abs() < 1e-4);
        assert!(combat.wounds_taken.is_empty());
        // From behind the plate is nothing: the bolt lands on him.
        let mut combat = volley(vec![plate(100.0)], middle(16.0, 5.0), west);
        for _ in 0..60 {
            combat.step(0.05, &sight, &bodies);
        }
        assert_eq!(combat.wounds_taken.len(), 1, "a bolt from behind lands");
        assert!(combat.take_plate_blocks().is_empty());
        // A plate with nothing left stops nothing.
        let mut combat = volley(vec![plate(0.0)], shooter, vec2(1.0, 0.0));
        for _ in 0..60 {
            combat.step(0.05, &sight, &bodies);
        }
        assert_eq!(combat.wounds_taken.len(), 1, "a spent plate is no plate");
        // And a friendly bolt flies through it.
        let mut combat = Combat::new(7);
        combat.set_targets(vec![Some((middle(12.0, 5.0), pistol))]);
        combat.set_plates(vec![plate(100.0)]);
        combat.loose(shooter, vec2(1.0, 0.0), pistol, false, &Skill::NONE, None);
        for _ in 0..60 {
            combat.step(0.05, &sight, &[]);
        }
        assert_eq!(combat.take_hits().len(), 1, "our own bolt is not stopped");
    }

    /// A Reflect Barrier (task 155): an enemy's hit on a body sending its
    /// share back lands on the shooter as a hit of that body's; a body
    /// sending none, or a shooter nobody named, sends nothing.
    #[test]
    fn a_reflect_barrier_sends_the_hit_back_on_the_shooter() {
        let (sight, _) = room_with(&[]);
        let pistol = WeaponKind::LaserPistol.basic();
        let ours = middle(10.0, 5.0);
        let theirs = middle(4.0, 5.0);
        let run = |reflect: f32, shooter: Option<usize>| {
            let mut combat = Combat::new(3);
            combat.set_targets(vec![Some((theirs, pistol))]);
            combat.set_reflects(vec![reflect]);
            combat.loose(theirs, vec2(1.0, 0.0), pistol, true, &Skill::NONE, None);
            combat.bolts[0].shooter = shooter;
            for _ in 0..40 {
                combat.step(0.05, &sight, &[Some((ours, false, 0.0))]);
            }
            (std::mem::take(&mut combat.wounds_taken), combat.take_hits())
        };
        let (taken, back) = run(1.0, Some(0));
        assert_eq!(taken.len(), 1, "the body still takes the hit");
        assert_eq!(back.len(), 1);
        assert_eq!((back[0].who, back[0].by), (0, Some(0)));
        assert!((back[0].damage - taken[0].damage).abs() < 1e-5, "all of it");
        let (_, back) = run(0.0, Some(0));
        assert!(back.is_empty(), "no barrier, nothing back");
        let (_, back) = run(1.0, None);
        assert!(back.is_empty(), "nobody to send it to");
    }

    /// A taunt is the only target a shooter within its radius that sees it
    /// picks — shot at in reach, nobody shot at beyond it — and with a
    /// magnet it is charged at too; of two, the most recent (features 77
    /// and 139).
    #[test]
    fn a_taunting_target_is_the_only_one_aimed_at_and_a_magnet_is_charged_at_first() {
        let (sight, nav) = room_with(&[]);
        let from = middle(2.0, 5.0);
        let near = middle(5.0, 5.0);
        let far = middle(12.0, 5.0);
        let mut combat = Combat::new(23);
        let pistol = WeaponKind::LaserPistol.basic();
        combat.set_targets(vec![Some((near, pistol)), Some((far, pistol))]);
        let stats = pistol.stats();
        let aimed = |c: &Combat| c.aim(&sight, from, &stats).map(|(i, _, _)| i);
        let taunt = |radius: f32, order: u32| Taunt {
            radius,
            magnet: false,
            order,
        };
        assert_eq!(aimed(&combat), Some(0), "the nearest without a taunt");
        combat.set_taunting(&[Taunt::NONE, taunt(20.0 * TILE, 1)]);
        assert_eq!(aimed(&combat), Some(1), "the taunt before the nearer");
        // A Juggernaut's reaches at any distance.
        combat.set_taunting(&[Taunt::NONE, taunt(f32::INFINITY, 1)]);
        assert_eq!(aimed(&combat), Some(1), "at any distance");
        // Out of the taunt's own radius, it pulls nobody.
        combat.set_taunting(&[Taunt::NONE, taunt(2.0 * TILE, 1)]);
        assert_eq!(aimed(&combat), Some(0));
        // Of two taunts reaching the shooter, the most recent — the nearer
        // one when it is, the farther when that is.
        combat.set_taunting(&[taunt(20.0 * TILE, 2), taunt(20.0 * TILE, 1)]);
        assert_eq!(aimed(&combat), Some(0), "the more recent, nearer");
        combat.set_taunting(&[taunt(20.0 * TILE, 1), taunt(20.0 * TILE, 2)]);
        assert_eq!(aimed(&combat), Some(1), "the more recent, farther");
        // And beyond the weapon's reach the taunt is still him and nobody
        // else: a shotgun taunted from past its range fires at nobody.
        let shotgun = WeaponKind::Shotgun.basic().stats();
        let beyond = middle(2.0, 5.0) + vec2(shotgun.reach() + 2.0 * TILE, 0.0);
        combat.set_targets(vec![Some((near, pistol)), Some((beyond, pistol))]);
        let short = |c: &Combat| c.aim(&sight, from, &shotgun).map(|(i, _, _)| i);
        assert_eq!(short(&combat), Some(0), "untaunted, the nearer");
        combat.set_taunting(&[Taunt::NONE, taunt(f32::INFINITY, 1)]);
        assert_eq!(short(&combat), None, "taunted, nobody else");
        // Only a shooter that can see the taunter is taunted: one with a
        // wall between shoots the nearer as ever.
        let (walled, _) = walled_room();
        let (shooter, seen, hidden) = (middle(8.0, 2.0), middle(9.0, 3.0), middle(13.0, 2.0));
        combat.set_targets(vec![Some((seen, pistol)), Some((hidden, pistol))]);
        combat.set_taunting(&[Taunt::NONE, taunt(f32::INFINITY, 1)]);
        assert!(walled.sees_from(shooter, hidden).is_none(), "the wall");
        let aimed_behind = combat.aim(&walled, shooter, &stats).map(|(i, _, _)| i);
        assert_eq!(aimed_behind, Some(0), "unseen, the taunt pulls nobody");
        // A blade goes for the nearest until the taunt is a magnet.
        combat.set_targets(vec![Some((near, pistol)), Some((far, pistol))]);
        combat.set_taunting(&[Taunt::NONE, taunt(20.0 * TILE, 1)]);
        let charge =
            |c: &Combat| Tactics::charge(&nav, from, c.targets()).expect("somewhere to charge to");
        let at = charge(&combat);
        assert!((at - near).len() < (at - far).len(), "the nearest blade");
        combat.set_taunting(&[
            Taunt::NONE,
            Taunt {
                magnet: true,
                ..taunt(20.0 * TILE, 1)
            },
        ]);
        let at = charge(&combat);
        assert!((at - far).len() < (at - near).len(), "the magnet's");
    }

    /// Weak Spot (task 124): every bolt rolled on its own off the lent
    /// stream at the shooter's chance, a hit keeping the weapon's flat
    /// damage, a blow rolled the same way and a burst never — and the
    /// fight's own stream untouched, so every other roll of the run comes
    /// out exactly as it did with no chance at all.
    #[test]
    fn weak_spot_rolls_off_its_own_stream_and_every_other_roll_is_as_it_was() {
        let (sight, _) = room_with(&[]);
        let theirs = middle(10.0, 5.0);
        let from = middle(8.0, 5.0);
        let pistol = WeaponKind::LaserPistol.basic();
        let run = |chance: f32| {
            let mut combat = Combat::new(7);
            combat.set_targets(vec![Some((theirs, pistol))]);
            combat.set_crit_chances(vec![chance]);
            combat.lend_crit_rng(Rng::new(99));
            let skill = Skill {
                crit_chance: chance,
                ..Skill::NONE
            };
            let mut hits = Vec::new();
            for _ in 0..3_000 {
                combat.fire_as(from, theirs, pistol, false, false, &skill, Some(0));
                for _ in 0..20 {
                    combat.step(0.05, &sight, &[]);
                }
                hits.extend(combat.take_hits());
            }
            hits
        };
        let without = run(0.0);
        assert!(without.len() > 2_000, "{}", without.len());
        assert!(without.iter().all(|h| !h.crit));
        for chance in [0.10, 0.12, 0.15, 0.20] {
            let with = run(chance);
            // The same hits, on the same parts, for the same damage: the
            // crit's stream is its own.
            assert_eq!(with.len(), without.len(), "{chance}");
            for (a, b) in with.iter().zip(&without) {
                assert_eq!((a.who, a.roll), (b.who, b.roll));
                assert_eq!(a.damage, b.damage);
                // And the flat damage is the weapon's own at the distance.
                assert!((a.flat - pistol.stats().damage_at(2.0)).abs() < 0.5);
            }
            let share = with.iter().filter(|h| h.crit).count() as f32 / with.len() as f32;
            assert!(
                (share - chance).abs() < 0.025,
                "{share} critical at a chance of {chance}"
            );
        }
        // No stream lent, no crit, whatever the chance.
        let mut combat = Combat::new(7);
        combat.set_targets(vec![Some((theirs, pistol))]);
        combat.set_crit_chances(vec![1.0]);
        let skill = Skill {
            crit_chance: 1.0,
            ..Skill::NONE
        };
        combat.fire_as(from, theirs, pistol, false, false, &skill, Some(0));
        for _ in 0..20 {
            combat.step(0.05, &sight, &[]);
        }
        assert!(combat.take_hits().iter().all(|h| !h.crit));
        // A blow is rolled like a bolt: a fist at a chance of one is
        // critical, its flat the fist's own.
        combat.lend_crit_rng(Rng::new(3));
        combat.brawl(
            theirs - vec2(TILE, 0.0),
            0,
            pistol,
            FIST_DAMAGE * 1.5,
            false,
            false,
            Some(0),
            FIST_DAMAGE,
        );
        let blow = combat.take_hits();
        assert_eq!(blow.len(), 1);
        assert!(blow[0].crit && blow[0].flat == FIST_DAMAGE);
        // And a burst never is, whoever threw it.
        let burst = combat.blast(0, 60.0, Some(0));
        assert!(!burst.crit && burst.flat == 0.0);
        // Somebody else's hit is never critical at the soldier's chance.
        combat.fire_as(from, theirs, pistol, false, false, &Skill::NONE, Some(1));
        for _ in 0..20 {
            combat.step(0.05, &sight, &[]);
        }
        assert!(combat.take_hits().iter().all(|h| !h.crit));
    }

    /// Brace (task 124): the miss chance multiplied by one less the share,
    /// near and far, the hit chance never past one, far aim equal to near
    /// with *deadeye* — and with no share the odds are the weapon's to the
    /// bit.
    #[test]
    fn a_miss_cut_takes_its_share_of_the_misses_and_never_passes_one() {
        let pistol = WeaponKind::LaserPistol.basic();
        let base = pistol.stats();
        assert_eq!(Skill::NONE.stats(pistol), base);
        for cut in [0.2f32, 0.3, 0.4, 0.5] {
            let skill = Skill {
                miss_cut: cut,
                ..Skill::NONE
            };
            let s = skill.stats(pistol);
            let near = 1.0 - (1.0 - base.accuracy) * (1.0 - cut);
            let far = 1.0 - (1.0 - base.accuracy_far) * (1.0 - cut);
            assert!((s.accuracy - near).abs() < 1e-6, "{cut}");
            assert!((s.accuracy_far - far).abs() < 1e-6, "{cut}");
        }
        let fourth = Skill {
            miss_cut: 0.5,
            deadeye: true,
            ..Skill::NONE
        }
        .stats(pistol);
        assert_eq!(fourth.accuracy_far, fourth.accuracy);
        // A shot already certain stays certain, and an aim lifted past one
        // is one.
        let lifted = Skill {
            miss_cut: 0.5,
            accuracy: 1.5,
            ..Skill::NONE
        }
        .stats(pistol);
        assert_eq!(lifted.accuracy, 1.0);
        let sniper = WeaponKind::SniperRifle.at(Tier::Two);
        let s = Skill {
            miss_cut: 0.5,
            ..Skill::NONE
        }
        .stats(sniper);
        assert_eq!(s.accuracy, 1.0);
        assert!(s.accuracy_far <= 1.0);
    }

    /// A player's own shot ([`Skill::sure`]): every carried weapon at
    /// every tier at full odds near and far, a shot on the move landing
    /// like one standing, and a shot along the heading never straying —
    /// where the same skill without it misses some.
    #[test]
    fn a_player_s_shot_always_lands_and_a_bot_s_keeps_its_odds() {
        let soldier = Skill {
            accuracy: 0.9,
            ..Skill::NONE
        };
        for kind in WeaponKind::ALL {
            for tier in Tier::ALL.into_iter().filter(|&t| t >= kind.min_tier()) {
                let s = soldier.sure().stats(kind.at(tier));
                assert_eq!(
                    (s.accuracy, s.accuracy_far),
                    (1.0, 1.0),
                    "{kind:?} {tier:?}"
                );
            }
        }
        let (sight, _) = room_with(&[]);
        let theirs = middle(14.0, 5.0);
        let from = middle(2.0, 5.0);
        let pistol = WeaponKind::LaserPistol.basic();
        let landed = |skill: &Skill| {
            let mut combat = Combat::new(7);
            combat.set_targets(vec![Some((theirs, pistol))]);
            let mut hits = 0;
            for _ in 0..200 {
                combat.fire_as(from, theirs, pistol, false, true, skill, Some(0));
                for _ in 0..40 {
                    combat.step(0.05, &sight, &[]);
                }
                hits += combat.take_hits().len();
            }
            hits
        };
        assert_eq!(landed(&soldier.sure()), 200, "every shot on the move");
        assert!(landed(&soldier) < 150, "a bot's walking shots miss");
        let mut combat = Combat::new(7);
        let angle = 0.3;
        for _ in 0..50 {
            combat.fire_along(from, angle, pistol, true, &soldier.sure(), Some(0));
        }
        for bolt in &combat.bolts {
            assert!((bolt.vel.angle() - angle).abs() < 1e-5);
        }
    }
}
