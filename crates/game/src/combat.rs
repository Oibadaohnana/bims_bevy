//! Arms, armour, and the shots in the air.
//!
//! Every Bim carries a [`Gear`]: three armour slots, a weapon slot with
//! the hand laser everybody is issued, and a nine-cell pack. A Bim under orders
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
//! the game applies it itself. A [`Hit`] carries the **part** it landed
//! on, rolled from the combat stream the instant the bolt reaches the body
//! ([`crate::health::Part::hit_by`]), and the **damage**, worked out then
//! too: a weapon's damage falls off with the distance the bolt has flown
//! ([`WeaponStats::damage_at`]), and the bolt remembers where it was fired
//! from for exactly that.
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
//! weapon would do back, so a sniper rifle hangs back at twenty tiles
//! where the pistol has run out, a shotgun closes to four, an auto rifle
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
//! A piece of armour ([`Piece`]) goes on the part it is cut for
//! ([`ArmourKind::slot`]) and stands between that part and a hit: its
//! **protection** comes off every hit's damage before anything else, and
//! what is left drains the piece's own **health** first — only what the
//! piece cannot take reaches the body and opens a wound (`Game::wound`).
//! At nothing the piece is **broken**: still worn, still drawn, doing
//! nothing. A piece keeps its damage wherever it goes — into the pack, the
//! hold, another Bim — which is why it has an `id` and is carried about
//! as an [`Item`] rather than counted. The world keeps the pieces in the
//! hold; the room keeps the ones on a body (`Gear::pack`, the three
//! slots), because the room's health reads them.
//!
//! # Accuracy and damage are two points and a line
//!
//! A weapon's odds of a hit are `accuracy` out to `sweet` tiles and
//! `accuracy_far` at `range`, a straight line between, and the same for
//! `damage` and `damage_far` ([`WeaponStats::hit_chance`],
//! [`WeaponStats::damage_at`]): a shotgun does everything it can at four
//! tiles and a good deal less at ten, a sniper rifle cannot miss at twenty
//! and can at thirty-five. A shot that is going to hit is aimed at the
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
use crate::health::Part;
use crate::math::{Rect, Vec2};
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

/// How long a pistol's bolt is drawn, in room units, and how thick.
const BOLT_LENGTH: f32 = 24.0;
const BOLT_CORE: f32 = 2.0;
const BOLT_GLOW: f32 = 6.0;
/// The shotgun's pellets: five short ones fanned about the flight.
const PELLETS: usize = 5;
const PELLET_LENGTH: f32 = 14.0;
const PELLET_SPREAD: f32 = 6.0 * (std::f32::consts::PI / 180.0);
/// The rifle's tracer, short and thin, and the sniper's streak, long.
const TRACER_LENGTH: f32 = 26.0;
const STREAK_LENGTH: f32 = 90.0;

/// How long the flash where a bolt lands lasts, in seconds.
const SPARK_LIFE: f32 = 0.18;

/// Friendly fire is blue and an enemy's is red — always, so a glance at
/// the air says who is shooting whom. The other guns' colours are mixed
/// with the side's, so the tint still says whose it is.
pub const FRIENDLY_BOLT: Color = Color::rgb(0.40, 0.72, 1.0);
pub const HOSTILE_BOLT: Color = Color::rgb(1.0, 0.28, 0.22);
const BOLT_CORE_WHITE: Color = Color::rgb(0.92, 0.97, 1.0);
const PELLET: Color = Color::rgb(1.0, 0.62, 0.25);
const TRACER: Color = Color::rgb(1.0, 0.92, 0.35);
const STREAK: Color = Color::rgb(0.80, 0.92, 1.0);

/// What a Bim can shoot with. The discriminants are the codes the app
/// names, written out so a reordering cannot renumber anything; `0` is
/// "nothing in the slot", as everywhere else.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum WeaponKind {
    /// The hand laser everybody starts with.
    LaserPistol = 1,
    /// Close and hard: everything it has inside four tiles, little past ten.
    Shotgun = 2,
    /// Eight light shots to a trigger pull, then a wait to recharge.
    AutoRifle = 3,
    /// Cannot miss at twenty tiles, and still shoots at thirty-five.
    SniperRifle = 4,
    /// A blade with a laser edge: used within reach, and a cut bleeds.
    Schword = 5,
}

impl WeaponKind {
    pub const ALL: [WeaponKind; 5] = [
        WeaponKind::LaserPistol,
        WeaponKind::Shotgun,
        WeaponKind::AutoRifle,
        WeaponKind::SniperRifle,
        WeaponKind::Schword,
    ];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<WeaponKind> {
        WeaponKind::ALL.iter().copied().find(|k| k.code() == code)
    }

    /// What the weapon is in the hold: the `ResourceId` code, the way a
    /// piece of armour has one ([`ArmourKind::resource`]) — a number,
    /// since this crate does not know `physics`.
    pub fn resource(self) -> u32 {
        match self {
            WeaponKind::LaserPistol => 8,
            WeaponKind::Shotgun => 17,
            WeaponKind::AutoRifle => 18,
            WeaponKind::SniperRifle => 19,
            WeaponKind::Schword => 20,
        }
    }

    /// The weapon a resource code is, if it is one.
    pub fn from_resource(code: u32) -> Option<WeaponKind> {
        WeaponKind::ALL
            .iter()
            .copied()
            .find(|k| k.resource() == code)
    }

    /// The weapon's numbers: one table, `crate::balance`, for a native
    /// server to agree with and for tuning. See the module note on the
    /// two-point curves.
    pub fn stats(self) -> WeaponStats {
        match self {
            WeaponKind::LaserPistol => balance::LASER_PISTOL,
            WeaponKind::Shotgun => balance::SHOTGUN,
            WeaponKind::AutoRifle => balance::AUTO_RIFLE,
            WeaponKind::SniperRifle => balance::SNIPER_RIFLE,
            WeaponKind::Schword => balance::SCHWORD,
        }
    }

    /// The kind at tier one: what everybody is issued and what a bench
    /// makes.
    pub fn basic(self) -> Weapon {
        self.at(Tier::One)
    }

    pub fn at(self, tier: Tier) -> Weapon {
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
    fn armour_factor(self) -> f32 {
        match self {
            Tier::One => 1.0,
            Tier::Two => balance::ARMOUR_TIER_STEP,
            Tier::Three => balance::ARMOUR_TIER_STEP * balance::ARMOUR_TIER_STEP,
        }
    }
}

/// A weapon: what it is and how good. What a hand holds, a pack carries,
/// the hold's list keeps, and a shot or a bolt was fired from. A weapon
/// has no wear and no id — two pistols of a tier are the same pistol —
/// which is why it is a value and a piece of armour is an instance.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
            ..base
        }
    }
}

/// What a weapon does, in the units the app prints them in.
#[derive(Clone, Copy, PartialEq, Debug)]
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
/// What a Bim can wear. The discriminants are the codes the app names
/// (`ARMOUR_NAMES`), written out like the weapons'; `0` is the empty
/// slot. Each is cut for one part — see [`ArmourKind::slot`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum ArmourKind {
    /// A steel cap: the head.
    BasicHelm = 1,
    /// A plate vest: the body.
    BasicKevlar = 2,
    /// Shin guards over the boots: the legs.
    BasicLegs = 3,
}

impl ArmourKind {
    pub const ALL: [ArmourKind; 3] = [
        ArmourKind::BasicHelm,
        ArmourKind::BasicKevlar,
        ArmourKind::BasicLegs,
    ];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<ArmourKind> {
        ArmourKind::ALL.iter().copied().find(|k| k.code() == code)
    }

    /// The part it goes on. One piece a part, and the three kinds are one
    /// each, so the slot is the kind's and never a choice.
    pub fn slot(self) -> Part {
        match self {
            ArmourKind::BasicHelm => Part::Head,
            ArmourKind::BasicKevlar => Part::Body,
            ArmourKind::BasicLegs => Part::Legs,
        }
    }

    /// What the piece is in the hold: the `ResourceId` code, since a piece
    /// in a container is a resource and one anywhere else is an instance.
    /// A code and not the id — this crate does not know `physics`, and a
    /// number is enough for the world to match the two.
    pub fn resource(self) -> u32 {
        match self {
            ArmourKind::BasicHelm => 14,
            ArmourKind::BasicKevlar => 15,
            ArmourKind::BasicLegs => 16,
        }
    }

    /// The piece's numbers: in `crate::balance`, like the weapons'.
    pub fn stats(self) -> ArmourStats {
        match self {
            ArmourKind::BasicHelm => balance::BASIC_HELM,
            ArmourKind::BasicKevlar => balance::BASIC_KEVLAR,
            ArmourKind::BasicLegs => balance::BASIC_LEGS,
        }
    }
}

/// What a piece of armour does, in health points.
#[derive(Clone, Copy, PartialEq, Debug)]
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

    /// What it adds to the part's health: what it has left, or nothing
    /// once it is broken.
    pub fn bonus(&self) -> f32 {
        if self.broken() { 0.0 } else { self.health }
    }
}

/// One thing in a pack cell: a piece of armour, a weapon, one unit of a
/// resource by its `ResourceId` code (a bandage, a medkit — the world
/// knows what the number is; the room only carries it), or a research
/// key of a tier, which is the one thing that takes more than a cell.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Item {
    Armour(Piece),
    Weapon(Weapon),
    Stack(u32),
    /// A research key of that tier: two cells tall in the pack — it is
    /// kept in the upper one and the cell under it is its tail — and
    /// nothing a body wears or holds. The world knows what it opens.
    Key(u8),
}

impl Item {
    /// How many cells of a pack it covers, rows by columns, unturned: the
    /// footprint the world lays it on the lockers' grid by
    /// (`economy::footprint`), said again here by resource code since this
    /// crate does not know `physics` — a pistol a row of two, a sniper
    /// rifle a row of ten, a vest four by four, a key two tall — and
    /// pinned against that table by the world's tests.
    pub fn footprint(&self) -> (u8, u8) {
        let code = match self {
            Item::Armour(piece) => piece.kind.resource(),
            Item::Weapon(weapon) => weapon.kind.resource(),
            Item::Stack(code) => *code,
            Item::Key(_) => return (2, 1),
        };
        match code {
            // The suit, the vest.
            7 | 9 => (3, 3),
            // The pistol.
            8 => (1, 2),
            // A medkit.
            10 => (2, 2),
            // The helm, the kevlar, the leg guards.
            14 => (2, 4),
            15 => (4, 4),
            16 => (3, 2),
            // The shotgun, the auto rifle, the sniper rifle, the schword.
            17 => (2, 5),
            18 => (1, 7),
            19 => (1, 10),
            20 => (1, 5),
            // A crate of vegetables, a block of tofu.
            3 => (1, 2),
            4 => (4, 4),
            _ => (1, 1),
        }
    }

    /// How many rows it takes unturned — what a desk's slot asks of a key.
    pub fn rows(&self) -> usize {
        self.footprint().0 as usize
    }

    /// Its footprint as laid, turned a quarter or not.
    pub fn laid(&self, turned: bool) -> (usize, usize) {
        let (rows, cols) = self.footprint();
        if turned {
            (cols as usize, rows as usize)
        } else {
            (rows as usize, cols as usize)
        }
    }
}

/// How many cells a Bim's pack has: seven by seven, laid out the way the
/// lockers are — a thing over its footprint, turned if it is turned —
/// and addressed by the cell its top-left corner is in.
pub const PACK_CELLS: usize = PACK_COLS * PACK_ROWS;
/// How many across, which is what a row down is offset by.
pub const PACK_COLS: usize = 7;
pub const PACK_ROWS: usize = 7;

/// How many cells a body shows when it is looted: the pack's nine, then
/// the three worn pieces and the weapon in hand — see [`LootCell`].
pub const LOOT_CELLS: usize = PACK_CELLS + 4;

/// One cell of what a body shows when it is looted, in the order the Loot
/// window lays them out: the nine of its pack, then the head, the body,
/// the legs and the weapon in hand. `code()` is that order — 0..8 the
/// pack, then 9, 10, 11, 12 — which is what a `Command::Loot` names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LootCell {
    Pack(u8),
    Head,
    Body,
    Legs,
    Weapon,
}

impl LootCell {
    pub fn code(self) -> u32 {
        match self {
            LootCell::Pack(cell) => cell as u32,
            LootCell::Head => PACK_CELLS as u32,
            LootCell::Body => PACK_CELLS as u32 + 1,
            LootCell::Legs => PACK_CELLS as u32 + 2,
            LootCell::Weapon => PACK_CELLS as u32 + 3,
        }
    }

    pub fn from_code(code: u32) -> Option<LootCell> {
        match code {
            c if (c as usize) < PACK_CELLS => Some(LootCell::Pack(c as u8)),
            c if c == PACK_CELLS as u32 => Some(LootCell::Head),
            c if c == PACK_CELLS as u32 + 1 => Some(LootCell::Body),
            c if c == PACK_CELLS as u32 + 2 => Some(LootCell::Legs),
            c if c == PACK_CELLS as u32 + 3 => Some(LootCell::Weapon),
            _ => None,
        }
    }
}

/// What one Bim has on it: three armour slots, top to bottom, the weapon
/// in its hand, and the pack on its back — seven by seven cells, indexed
/// row by row, each thing kept in the cell its top-left corner is in and
/// reaching over the rest of its footprint ([`Item::footprint`]), turned
/// a quarter round if `turned` says so for that cell.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Gear {
    pub head: Option<Piece>,
    pub body: Option<Piece>,
    pub legs: Option<Piece>,
    pub weapon: Option<Weapon>,
    pub pack: [Option<Item>; PACK_CELLS],
    /// Which way round the thing kept in each cell lies; `false` where
    /// nothing is kept.
    pub turned: [bool; PACK_CELLS],
}

// By hand: an array past thirty-two has no `Default` of its own.
impl Default for Gear {
    fn default() -> Gear {
        Gear {
            head: None,
            body: None,
            legs: None,
            weapon: None,
            pack: [None; PACK_CELLS],
            turned: [false; PACK_CELLS],
        }
    }
}

impl Gear {
    /// What everybody is issued: nothing to wear, a hand laser, and an
    /// empty pack.
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

    /// Put a fresh basic helm, kevlar and leg guards on, numbered from
    /// `first_id`, over whatever was worn — what every melee bot gets,
    /// enemy or hired. The ids are the room's own until the world takes a
    /// piece over (a loot renumbers it).
    pub fn basic_armour(&mut self, first_id: u32) {
        self.head = Some(Piece::new(first_id, ArmourKind::BasicHelm, Tier::One));
        self.body = Some(Piece::new(first_id + 1, ArmourKind::BasicKevlar, Tier::One));
        self.legs = Some(Piece::new(first_id + 2, ArmourKind::BasicLegs, Tier::One));
    }

    /// What a mercenary carries: better arms than a resident's, rolled off
    /// `seed` against [`MERCENARY_ODDS`] — a third the pistol, the rest
    /// the heavier guns and a few the schword — and each piece of armour
    /// with the odds in [`MERCENARY_ARMOUR_ODDS`], fresh, with ids off the
    /// same stream (`piece_ids` and up, since the room's pieces are only
    /// its own until the world takes one over) — every piece, for one
    /// carrying the schword, since a melee bot always wears the basic
    /// armour. A function of the seed alone, like [`Gear::issued_for`],
    /// so the world derives it.
    pub fn hired_for(seed: u64, piece_ids: u32) -> Gear {
        let mut rng = Rng::new(seed ^ 0x_4D45_5243);
        let weapon = roll_weapon(&mut rng, &MERCENARY_ODDS).basic();
        let blade = weapon.stats().melee;
        let mut next = piece_ids;
        let mut piece = |rng: &mut Rng, kind: ArmourKind, odds: f32| {
            // Rolled either way, so the stream is the same whatever the
            // weapon was.
            let worn = rng.chance(odds) || blade;
            worn.then(|| {
                next += 1;
                Piece::new(next - 1, kind, Tier::One)
            })
        };
        let [helm, kevlar, legs] = MERCENARY_ARMOUR_ODDS;
        Gear {
            head: piece(&mut rng, ArmourKind::BasicHelm, helm),
            body: piece(&mut rng, ArmourKind::BasicKevlar, kevlar),
            legs: piece(&mut rng, ArmourKind::BasicLegs, legs),
            weapon: Some(weapon),
            ..Gear::default()
        }
    }

    /// The piece on a part, if any — broken or not.
    pub fn worn(&self, part: Part) -> Option<Piece> {
        *self.slot(part)
    }

    pub fn worn_mut(&mut self, part: Part) -> &mut Option<Piece> {
        match part {
            Part::Head => &mut self.head,
            Part::Body => &mut self.body,
            Part::Legs => &mut self.legs,
        }
    }

    fn slot(&self, part: Part) -> &Option<Piece> {
        match part {
            Part::Head => &self.head,
            Part::Body => &self.body,
            Part::Legs => &self.legs,
        }
    }

    /// The odds a bolt reaching this body is dodged for its armour: the
    /// worn pieces' [`Piece::dodge`]s combined, `1 − Π(1 − d)`, so three
    /// whole tier-three pieces are a little over a quarter. Nought for
    /// anything below tier three, which is every body today.
    pub fn dodge(&self) -> f32 {
        let missed: f32 = Part::ALL
            .iter()
            .map(|&part| 1.0 - self.worn(part).map_or(0.0, |p| p.dodge()))
            .product();
        1.0 - missed
    }

    /// What the armour adds to one part's health: the piece's health
    /// left, nothing when there is none or it is broken.
    pub fn part_bonus(&self, part: Part) -> f32 {
        self.worn(part).map_or(0.0, |p| p.bonus())
    }

    /// What the armour adds all told — the unbroken worn pieces summed —
    /// which is what the panel's blue bar is.
    pub fn armour_health(&self) -> f32 {
        Part::ALL.iter().map(|&p| self.part_bonus(p)).sum()
    }

    /// Whether a cell has something in it — its own, or a cell of a
    /// thing kept in another cell that reaches over it.
    pub fn occupied(&self, cell: usize) -> bool {
        if cell >= PACK_CELLS {
            return true;
        }
        self.pack[cell].is_some() || self.head_of(cell) != cell
    }

    /// The cell a thing over `cell` is kept in: `cell` itself, or the
    /// top-left cell of the thing that reaches over it.
    pub fn head_of(&self, cell: usize) -> usize {
        if cell >= PACK_CELLS || self.pack[cell].is_some() {
            return cell;
        }
        (0..PACK_CELLS)
            .find(|&head| {
                self.pack[head].is_some_and(|item| covers(head, item.laid(self.turned[head]), cell))
            })
            .unwrap_or(cell)
    }

    /// Whether `item` would lie with its top-left corner in `cell`, turned
    /// or not: every cell of it on the grid — a footprint does not wrap
    /// round the edge onto the next row — and free, bar the cells of the
    /// thing kept in `ignoring`, which is the one being moved.
    pub fn fits_turned(
        &self,
        cell: usize,
        item: Item,
        turned: bool,
        ignoring: Option<usize>,
    ) -> bool {
        let (rows, cols) = item.laid(turned);
        let (x, y) = (cell % PACK_COLS, cell / PACK_COLS);
        if x + cols > PACK_COLS || y + rows > PACK_ROWS {
            return false;
        }
        for r in y..y + rows {
            for c in x..x + cols {
                let at = r * PACK_COLS + c;
                let head = self.head_of(at);
                if (self.pack[at].is_some() || head != at) && Some(head) != ignoring {
                    return false;
                }
            }
        }
        true
    }

    /// Whether `item` would go into `cell` as it is, unturned.
    pub fn fits(&self, cell: usize, item: Item) -> bool {
        self.fits_turned(cell, item, false, None)
    }

    /// The way round `item` would lie at `cell`, if either: unturned first.
    pub fn fit_at(&self, cell: usize, item: Item) -> Option<bool> {
        [false, true]
            .into_iter()
            .find(|&turned| self.fits_turned(cell, item, turned, None))
    }

    /// The first place `item` fits — row by row from the top left, the
    /// whole pack unturned before any of it turned, the way the lockers
    /// are laid — and which way round.
    pub fn first_fit(&self, item: Item) -> Option<(usize, bool)> {
        for turned in [false, true] {
            if turned && item.footprint().0 == item.footprint().1 {
                break;
            }
            if let Some(cell) = (0..PACK_CELLS).find(|&c| self.fits_turned(c, item, turned, None)) {
                return Some((cell, turned));
            }
        }
        None
    }

    /// The first empty pack cell — for a one-cell item.
    pub fn free_cell(&self) -> Option<usize> {
        (0..PACK_CELLS).find(|&c| !self.occupied(c))
    }

    /// The first cell `item` would go into, the whole of it.
    pub fn free_cell_for(&self, item: Item) -> Option<usize> {
        self.first_fit(item).map(|(cell, _)| cell)
    }

    /// Lay `item` with its corner in `cell`, the way round it fits —
    /// unturned if it can. `false`, and nothing changed, when it would
    /// not lie there.
    pub fn put(&mut self, cell: usize, item: Item) -> bool {
        let Some(turned) = self.fit_at(cell, item) else {
            return false;
        };
        self.pack[cell] = Some(item);
        self.turned[cell] = turned;
        true
    }

    /// Take the thing kept in `cell`, or reaching over it, out of the pack.
    pub fn take_out(&mut self, cell: usize) -> Option<Item> {
        let head = self.head_of(cell);
        let item = self.pack.get_mut(head)?.take()?;
        self.turned[head] = false;
        Some(item)
    }

    /// Move the thing kept in `cell` so its corner is in `to`, turned or
    /// not — a drag across the pack. `false`, and nothing moved, when it
    /// would not lie there or there is nothing in `cell`.
    pub fn rearrange(&mut self, cell: usize, to: usize, turned: bool) -> bool {
        let head = self.head_of(cell);
        let Some(item) = self.pack.get(head).copied().flatten() else {
            return false;
        };
        if to >= PACK_CELLS || !self.fits_turned(to, item, turned, Some(head)) {
            return false;
        }
        self.pack[head] = None;
        self.turned[head] = false;
        self.pack[to] = Some(item);
        self.turned[to] = turned;
        true
    }
}

/// Whether a thing kept in `head`, covering `laid` (rows, columns), reaches
/// over `cell`.
fn covers(head: usize, (rows, cols): (usize, usize), cell: usize) -> bool {
    let (hx, hy) = (head % PACK_COLS, head / PACK_COLS);
    let (x, y) = (cell % PACK_COLS, cell / PACK_COLS);
    x >= hx && x < hx + cols && y >= hy && y < hy + rows
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
/// The odds a mercenary wears a helm, a kevlar and leg guards.
pub const MERCENARY_ARMOUR_ODDS: [f32; 3] = [0.5, 0.6, 0.4];

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
}

/// One shot in the air.
#[derive(Clone, Copy, Debug)]
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
    /// The one body it has already slipped past — a peek that dodged it —
    /// so a bolt crossing a body over several steps is rolled for once.
    dodged: Option<usize>,
}

/// A bolt that reached a body, or a blow that landed: whose — an index
/// into the targets for a friendly one, into this room's own crew for a
/// hostile one — where on it, how hard, and whether it is a cut, which
/// bleeds three times what a shot does.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Hit {
    pub who: usize,
    pub part: Part,
    pub damage: f32,
    pub cut: bool,
}

/// A blow on its way: a swing or a jab that has started, landing on the
/// target with that index — `damage`, a cut or not — when `left` seconds
/// have run out, which is the animation's length
/// (`crate::character::SWING_TIME`). It lands only if the target is still
/// within reach then ([`Combat::within_reach`]): a body that stepped back
/// during the swing is missed. `Game::tick_combat` keeps one on the Bim.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Blow {
    pub target: usize,
    pub left: f32,
    pub damage: f32,
    pub cut: bool,
}

/// A shot an enemy took, for the world to carry into the crew's room and
/// fire there. Where from — the eye it was aimed from, which for a peek
/// is beside the body — where at, and with what. A **melee** one is a
/// blow, not a shot: nothing flies, and the world delivers it straight to
/// the body (`Game::enemy_strike`) with the damage and whether it cuts
/// carried here, since a fist's damage is not the gun's in the hand.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Shot {
    pub from: Vec2,
    pub at: Vec2,
    pub weapon: Weapon,
    pub melee: bool,
    pub damage: f32,
    pub cut: bool,
    /// Whether the shooter was walking as it fired: half the odds.
    pub moving: bool,
}

/// Where a bolt ended, briefly lit.
#[derive(Clone, Copy, Debug)]
struct Spark {
    pos: Vec2,
    age: f32,
    hostile: bool,
}

/// The fight, as the room keeps it: the enemies the world named, the bolts
/// flying, and the hits that landed since the world last asked.
pub struct Combat {
    /// Where the enemies stand and what they carry, index for index with
    /// whoever the world says they are; `None` for one that is down.
    /// Empty in peace.
    targets: Vec<Option<Target>>,
    pub bolts: Vec<Bolt>,
    sparks: Vec<Spark>,
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
}

impl Combat {
    pub fn new(seed: u64) -> Combat {
        Combat {
            targets: Vec::new(),
            bolts: Vec::new(),
            sparks: Vec::new(),
            hits: Vec::new(),
            wounds_taken: Vec::new(),
            shots: Vec::new(),
            lamp_hits: Vec::new(),
            cues: Vec::new(),
            // A fresh room has been quiet for ever.
            lull: f32::MAX,
            // Its own stream: a fight must not re-roll the room.
            rng: Rng::new(seed ^ 0xC0B_A7),
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
        });
    }

    /// The nearest enemy a shooter standing at `from` can see, within the
    /// weapon's range: which one, the eye it is seen from — the body, or the
    /// peek beside a wall — and where it stands.
    pub fn aim(
        &self,
        sight: &Sight,
        from: Vec2,
        stats: &WeaponStats,
    ) -> Option<(usize, Vec2, Vec2)> {
        let reach = stats.reach();
        let mut best: Option<(f32, usize, Vec2, Vec2)> = None;
        for (i, target) in self.targets.iter().enumerate() {
            let Some(at) = target.filter(|t| !t.stale).map(|t| t.at) else {
                continue;
            };
            let d = (at - from).len();
            if d > reach || best.is_some_and(|b| b.0 <= d) {
                continue;
            }
            if let Some(eye) = sight.sees_from(from, at) {
                best = Some((d, i, eye, at));
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
        let reach = MELEE_RANGE * TILE;
        let mut best: Option<(f32, usize)> = None;
        for (i, target) in self.targets.iter().enumerate() {
            let Some(t) = target.filter(|t| !t.stale) else {
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
        self.targets
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
        let stats = weapon.stats();
        self.lull = 0.0;
        let to = at - from;
        let tiles = to.len() / TILE;
        let hits = self
            .rng
            .chance(stats.hit_chance(tiles) * if moving { WALKING_ACCURACY } else { 1.0 });
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
        self.cues.push(Cued {
            cue: Cue::Shot {
                weapon: weapon.kind,
                hostile,
            },
            at: from,
        });
        self.bolts.push(Bolt {
            pos: from,
            vel: dir * stats.pace(),
            left: stats.reach(),
            weapon,
            fired_from: from,
            hostile,
            dodged: None,
        });
    }

    /// A blow in a melee, from a body at `from` on the target with that
    /// index: `damage` on a part rolled now, a cut or not. In the crew's
    /// room it is a [`Hit`] straight onto the hits, for the world to carry
    /// to the enemy's body; in a hostile room (`as_shot`) it is a melee
    /// [`Shot`] for the world to deliver to the crew member. Nothing
    /// flies either way.
    pub fn brawl(
        &mut self,
        from: Vec2,
        target: usize,
        weapon: Weapon,
        damage: f32,
        cut: bool,
        as_shot: bool,
    ) {
        let Some(at) = self.targets.get(target).copied().flatten().map(|t| t.at) else {
            return;
        };
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
            });
        } else {
            self.hits.push(Hit {
                who: target,
                part: Part::hit_by(self.rng.unit()),
                damage,
                cut,
            });
            self.cues.push(Cued {
                cue: Cue::Blow {
                    cut,
                    on_crew: false,
                },
                at,
            });
        }
    }

    /// One roll off the combat stream, 0 to 1: what the game rolls a
    /// dying state with (`health::Trauma::roll`), so a fight re-rolls
    /// nothing of the room's.
    pub fn roll(&mut self) -> f32 {
        self.rng.unit()
    }

    /// A blow the world delivered to one of this room's own bodies — an
    /// enemy's melee [`Shot`], carried across — as the hit it is, on a
    /// part rolled now from the combat stream. The game applies it.
    pub fn struck(&mut self, who: usize, damage: f32, cut: bool) -> Hit {
        self.lull = 0.0;
        Hit {
            who,
            part: Part::hit_by(self.rng.unit()),
            damage,
            cut,
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
        let rng = &mut self.rng;
        let mut landed: Vec<Hit> = Vec::new();
        let mut taken: Vec<Hit> = Vec::new();
        let mut sparks: Vec<Spark> = Vec::new();
        let mut heard: Vec<Cued> = Vec::new();
        let mut broken: Vec<(usize, f32)> = Vec::new();
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
            for (i, body) in looking_for.iter().enumerate() {
                let Some((body, peeking, dodge)) = *body else {
                    continue;
                };
                if bolt.dodged == Some(i) {
                    continue;
                }
                if let Some(t) = along(from, to, body, HIT_RADIUS)
                    && stop.is_none_or(|(s, _)| t < s)
                {
                    // In cover — leaning back in from a peek, or ducking
                    // behind the sandbags between it and the shooter — the
                    // bolt flies on past, and is not rolled for this body
                    // again.
                    let covered = peeking || sight.covered(body, bolt.fired_from);
                    if covered && rng.chance(DODGE_IN_COVER) {
                        bolt.dodged = Some(i);
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
                }
            }
            match stop {
                Some((t, who)) => {
                    let at = from + flight * t;
                    if let Some(who) = who {
                        let flown = (at - bolt.fired_from).len() / TILE;
                        let hit = Hit {
                            who,
                            part: Part::hit_by(rng.unit()),
                            damage: bolt.weapon.stats().damage_at(flown),
                            cut: false,
                        };
                        if bolt.hostile {
                            taken.push(hit);
                        } else {
                            landed.push(hit);
                        }
                    } else if let Some(lamp) = lamp {
                        // Nothing nearer than the lamp: the lamp took it.
                        let flown = (at - bolt.fired_from).len() / TILE;
                        broken.push((lamp, bolt.weapon.stats().damage_at(flown)));
                    }
                    heard.push(Cued {
                        cue: match who {
                            Some(_) => Cue::Impact {
                                on_crew: bolt.hostile,
                            },
                            None => Cue::Ricochet,
                        },
                        at,
                    });
                    sparks.push(Spark {
                        pos: at,
                        age: 0.0,
                        hostile: bolt.hostile,
                    });
                    false
                }
                None => {
                    bolt.pos = to;
                    bolt.left -= span;
                    if bolt.left <= 0.0 {
                        // Spent: it fades where it got to, without a flash.
                        return false;
                    }
                    true
                }
            }
        });
        self.hits.extend(landed);
        self.wounds_taken.extend(taken);
        self.lamp_hits.extend(broken);
        self.sparks.extend(sparks);
        self.cues.extend(heard);
    }

    /// Whether anything is in the air or lit.
    pub fn quiet(&self) -> bool {
        self.bolts.is_empty() && self.sparks.is_empty()
    }

    /// The bolts and the sparks. Over the fog — a shot is always seen.
    /// Each gun's shot has a look of its own, and an enemy's keeps the red
    /// mixed in so the tint still says whose it is.
    pub fn draw(&self, list: &mut DrawList) {
        for bolt in &self.bolts {
            let dir = bolt.vel.normalize_or_zero();
            let head = bolt.pos;
            let side = if bolt.hostile {
                HOSTILE_BOLT
            } else {
                FRIENDLY_BOLT
            };
            match bolt.weapon.kind {
                WeaponKind::Shotgun => {
                    // A fan of pellets about the flight, the outer ones
                    // trailing a little.
                    let pellet = side.mix(PELLET, 0.7);
                    for i in 0..PELLETS {
                        let f = (i as f32 - (PELLETS as f32 - 1.0) * 0.5)
                            / ((PELLETS - 1) as f32 * 0.5);
                        let d = dir.rotate(f * PELLET_SPREAD);
                        let tip = head - dir * (f.abs() * 6.0) + dir.perp() * (f * 7.0);
                        list.line(tip - d * PELLET_LENGTH, tip, 3.0, pellet.alpha(0.9));
                        list.circle(tip, 3.5, BOLT_CORE_WHITE.alpha(0.8));
                    }
                }
                WeaponKind::AutoRifle => {
                    let tracer = side.mix(TRACER, 0.65);
                    let tail = head - dir * TRACER_LENGTH;
                    list.line(tail, head, 5.0, tracer.alpha(0.35));
                    list.line(tail, head, 2.0, tracer.alpha(0.95));
                    list.circle(head, 3.0, BOLT_CORE_WHITE.alpha(0.9));
                }
                WeaponKind::SniperRifle => {
                    let streak = side.mix(STREAK, 0.6);
                    let tail = head - dir * STREAK_LENGTH;
                    list.line(tail, head, 6.0, streak.alpha(0.18));
                    list.line(
                        tail + dir * (STREAK_LENGTH * 0.3),
                        head,
                        2.0,
                        streak.alpha(0.9),
                    );
                    list.circle(head, 6.0, BOLT_CORE_WHITE.alpha(0.95));
                }
                // The pistol as it has always been; a blade never flies.
                WeaponKind::LaserPistol | WeaponKind::Schword => {
                    let tail = head - dir * BOLT_LENGTH;
                    list.line(tail, head, BOLT_GLOW, side.alpha(0.30));
                    list.line(tail, head, BOLT_CORE + 1.5, side.alpha(0.85));
                    list.line(
                        tail + dir * (BOLT_LENGTH * 0.35),
                        head,
                        BOLT_CORE,
                        BOLT_CORE_WHITE,
                    );
                }
            }
        }
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
}

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
/// What a candidate stand is worth to the enemy, in **tiles of walking**
/// — every term is in that one unit, so the trade-offs read off the
/// numbers. Cover is worth walking [`COVER_WORTH`] tiles for, so cover a
/// few tiles off beats the open anywhere and cover across the station
/// loses to the open here. How the weapon fits the distance against
/// what the target holds ([`Tactics::fit`]) is worth up to [`FIT_WORTH`]
/// either way, and outweighs the plain preference for distance
/// ([`DISTANCE_WORTH`] a tile), which is what is left when the two
/// weapons are a match: keep away, all else equal.
const COVER_WORTH: f32 = 15.0;
const FIT_WORTH: f32 = 30.0;
const DISTANCE_WORTH: f32 = 1.0;
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

/// How far a dying body looks for somewhere to run to, in tiles.
const FLEE_LOOK: f32 = 10.0;

/// Where the tactics say to stand, and whether it is cover: a peek beside
/// a wall, or close behind sandbags.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Stand {
    pub at: Vec2,
    pub cover: bool,
}

/// The enemy's choice of where to stand. See the module note.
pub struct Tactics;

impl Tactics {
    /// Where a body at `from` that is dying should run to: away from the
    /// enemy. The enemy is one place — the average of every target that
    /// is up — and the best cell is the reachable one within
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

    /// Where a body at `from` with a blade should go: the free cell it
    /// can reach nearest the nearest target — a charge. Cover means
    /// nothing to a blade, and distance is the one thing it wants less
    /// of. Every cell of the grid is a candidate, not the tile-spaced
    /// lattice a stand is chosen from: a blade wants to stand *beside*
    /// the target, and the lattice's nearest cell can be half a tile
    /// short of reach. `None` with no target it can get near.
    pub fn charge(nav: &Nav, from: Vec2, targets: &[Option<Target>]) -> Option<Vec2> {
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
        Tactics::stand_with_cover(sight, nav, from, targets, stats, doors, &[]).map(|s| s.at)
    }

    /// [`Tactics::stand`], and whether the stand it picked is cover — the
    /// one thing a bot with a shot from where it is will walk for, since
    /// walking halves its odds ([`WALKING_ACCURACY`]) — keeping off
    /// `taken`: where the others of its side stand or are walking to, no
    /// cell within a tile of one being a stand, so a squad does not pick
    /// the one best tile and pile onto it. A blade's charge is never
    /// cover.
    pub fn stand_with_cover(
        sight: &Sight,
        nav: &Nav,
        from: Vec2,
        targets: &[Option<Target>],
        stats: &WeaponStats,
        doors: &[Rect],
        taken: &[Vec2],
    ) -> Option<Stand> {
        if stats.melee {
            return Tactics::charge(nav, from, targets).map(|at| Stand { at, cover: false });
        }
        let reach = stats.reach();
        let here = nav.nearest_free(from);
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
        candidates.extend(
            lattice
                .into_iter()
                .filter(|&c| (c - here).len() > 1e-3 && !is_taken(c) && nav.can_reach(from, c)),
        );
        let mut best: Option<(f32, Stand)> = None;
        for (i, &c) in candidates.iter().enumerate() {
            let Some((view, cover)) = Tactics::view_from(sight, c, targets, stats) else {
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
    /// from the spot.
    fn view_from(
        sight: &Sight,
        c: Vec2,
        targets: &[Option<Target>],
        stats: &WeaponStats,
    ) -> Option<(f32, bool)> {
        let reach = stats.reach();
        let eyes = sight.eyes_from(c);
        let mut best: Option<(f32, bool)> = None;
        for target in targets.iter().flatten() {
            let d = (target.at - c).len();
            if d > reach {
                continue;
            }
            let tile = sight.tile_of(target.at);
            let mut body_sees = false;
            let mut peek_sees = false;
            for eye in &eyes {
                if !eye.admits(tile) || !sight.clear_line(eye.at, tile) {
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
            let cover = if in_cover { COVER_WORTH } else { 0.0 };
            let tiles = d / TILE;
            let fit = Tactics::fit(stats, &target.weapon.stats(), tiles) * FIT_WORTH;
            let score = cover + fit + tiles * DISTANCE_WORTH;
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

    /// The pack is a grid things lie on over their footprint, kept by
    /// their top-left cell: a key stands two tall, a rifle lies seven
    /// along — or stands, turned, when only that fits — a cell reached
    /// over answers to the thing's corner, and nothing lies off the edge
    /// or over anything else.
    #[test]
    fn a_thing_lies_over_its_footprint_and_turns_to_fit() {
        let mut gear = Gear::issued();
        let key = Item::Key(1);
        let rifle = Item::Weapon(WeaponKind::AutoRifle.basic());
        let bandage = Item::Stack(13);
        assert_eq!(key.rows(), 2);
        assert_eq!(key.footprint(), (2, 1));
        assert_eq!(rifle.footprint(), (1, 7));
        assert_eq!(rifle.laid(true), (7, 1));
        assert_eq!(bandage.footprint(), (1, 1));
        assert!(gear.fits(0, key));
        assert!(
            !gear.fits(6 * PACK_COLS, key),
            "the bottom row has nothing under it"
        );
        assert!(gear.fits(0, rifle), "the whole top row");
        assert!(
            !gear.fits(1, rifle),
            "a footprint does not wrap onto the next row"
        );
        assert_eq!(gear.free_cell_for(key), Some(0));
        assert!(gear.put(0, key));
        assert!(gear.occupied(0));
        assert!(gear.occupied(PACK_COLS), "the cell under it");
        assert!(!gear.occupied(2 * PACK_COLS));
        assert_eq!(gear.head_of(PACK_COLS), 0);
        assert_eq!(gear.head_of(2 * PACK_COLS), 2 * PACK_COLS);
        assert_eq!(gear.free_cell(), Some(1));
        assert!(
            !gear.fits(PACK_COLS, bandage),
            "nothing goes where a thing reaches"
        );
        // The rifle no longer lies along the top row; it lies along the
        // third, and put at the corner it would have to stand — which it
        // does, turned, down the last column.
        assert_eq!(gear.first_fit(rifle), Some((2 * PACK_COLS, false)));
        assert_eq!(gear.fit_at(6, rifle), Some(true));
        assert!(gear.put(6, rifle));
        assert!(gear.turned[6]);
        assert!(gear.occupied(6 + 6 * PACK_COLS), "down to the bottom");
        assert_eq!(gear.head_of(6 + 3 * PACK_COLS), 6);
        // Moved back to lie along a row, unturned; and refused over the key.
        assert!(gear.rearrange(6 + 3 * PACK_COLS, 2 * PACK_COLS, false));
        assert!(!gear.turned[2 * PACK_COLS]);
        assert!(gear.pack[6].is_none() && !gear.occupied(6 + 6 * PACK_COLS));
        assert!(!gear.rearrange(2 * PACK_COLS, 0, false));
        assert!(
            !gear.rearrange(2 * PACK_COLS, PACK_COLS + 1, true),
            "off the bottom"
        );
        assert!(
            gear.rearrange(2 * PACK_COLS, 2 * PACK_COLS, false),
            "where it is"
        );
        assert_eq!(
            gear.take_out(2 * PACK_COLS + 4),
            Some(rifle),
            "by any of its cells"
        );
        assert!(!gear.occupied(2 * PACK_COLS));
        // A square thing is never turned to fit.
        let suit = Item::Stack(7);
        assert_eq!(suit.footprint(), (3, 3));
        assert_eq!(gear.first_fit(suit), Some((1, false)));
        assert_eq!(PACK_COLS * PACK_ROWS, PACK_CELLS);
        assert_eq!(PACK_CELLS, 49);
    }

    #[test]
    fn a_piece_is_cut_for_one_part_and_does_nothing_once_broken() {
        for kind in ArmourKind::ALL {
            assert_eq!(ArmourKind::from_code(kind.code()), Some(kind));
            assert!(kind.stats().health > 0.0 && kind.stats().protection > 0.0);
        }
        assert_eq!(ArmourKind::from_code(0), None);
        assert_eq!(ArmourKind::BasicHelm.slot(), Part::Head);
        assert_eq!(ArmourKind::BasicKevlar.slot(), Part::Body);
        assert_eq!(ArmourKind::BasicLegs.slot(), Part::Legs);

        let mut gear = Gear::issued();
        assert_eq!(gear.armour_health(), 0.0);
        assert_eq!(gear.free_cell(), Some(0));
        let vest = Piece::new(1, ArmourKind::BasicKevlar, Tier::One);
        *gear.worn_mut(Part::Body) = Some(vest);
        assert_eq!(gear.worn(Part::Body), Some(vest));
        assert_eq!(gear.part_bonus(Part::Body), 20.0);
        assert_eq!(gear.armour_health(), 20.0);
        assert_eq!(vest.effective_protection(), 2.0);
        // Worn down to nothing: still worn, worth nothing.
        gear.worn_mut(Part::Body).as_mut().unwrap().health = 0.0;
        let worn = gear.worn(Part::Body).unwrap();
        assert!(worn.broken());
        assert_eq!(worn.effective_protection(), 0.0);
        assert_eq!(gear.part_bonus(Part::Body), 0.0);
        assert_eq!(gear.armour_health(), 0.0);
    }

    #[test]
    fn the_enemy_takes_cover_where_there_is_some_and_the_open_at_range_where_there_is_none() {
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
        // the greatest distance there is — the pistol reaches twenty-two
        // tiles, further than this room goes, so that is its far corner.
        let (sight, nav) = room_with(&[]);
        let target = middle(15.0, 8.5);
        let from = middle(17.0, 9.0);
        let stand =
            Tactics::stand(&sight, &nav, from, &[Some(pistol_at(target))], &stats, &[]).unwrap();
        let (body, _) = views(&sight, stand, target);
        assert!(body, "the open");
        let d = (stand - target).len();
        assert!(d <= stats.reach() && d > 14.0 * TILE, "{d}");

        // Nothing named: nowhere to stand.
        assert!(Tactics::stand(&sight, &nav, from, &[None], &stats, &[]).is_none());
    }

    #[test]
    fn an_enemy_stands_where_its_weapon_beats_the_target_s() {
        // A long hall with nothing in it — nowhere to take cover, so the
        // distance is all the weapon has to play with — the target at one
        // end with a pistol, the enemy starting a few tiles from it.
        let interior = Rect::from_min_size(Vec2::ZERO, vec2(40.0 * TILE, 6.0 * TILE));
        let sight = Sight::new(interior, interior, TILE, &[], &[]);
        let nav = Nav::tiled(interior, &[], BODY_MARGIN, TILE);
        let target = pistol_at(middle(1.0, 3.0));
        let from = middle(5.0, 3.0);
        let tiles_off = |stand: Vec2| (stand - target.at).len() / TILE;

        // A sniper rifle is full out to twenty tiles, where the pistol
        // has long run out: it hangs back there, and not at its own
        // thirty-five, where its curve has fallen off.
        let sniper = WeaponKind::SniperRifle.stats();
        let stand = Tactics::stand(&sight, &nav, from, &[Some(target)], &sniper, &[]).unwrap();
        let d = tiles_off(stand);
        assert!((19.0..=24.0).contains(&d), "the sniper at {d} tiles");

        // A shotgun is at its best inside four tiles and a pistol is not
        // much worse there than anywhere: it closes.
        let shotgun = WeaponKind::Shotgun.stats();
        let stand = Tactics::stand(&sight, &nav, from, &[Some(target)], &shotgun, &[]).unwrap();
        let d = tiles_off(stand);
        assert!(d <= 4.5, "the shotgun at {d} tiles");

        // An auto rifle reaches twenty-six tiles to the pistol's twenty-two:
        // it stands where it can shoot and cannot be shot back at.
        let rifle = WeaponKind::AutoRifle.stats();
        let stand = Tactics::stand(&sight, &nav, from, &[Some(target)], &rifle, &[]).unwrap();
        let d = tiles_off(stand);
        assert!(d > 22.0 && d <= 26.0, "the rifle at {d} tiles");

        // Pistol against pistol is a match, and keeping away is what is
        // left: the far end of its own reach.
        let pistol = WeaponKind::LaserPistol.stats();
        let stand = Tactics::stand(&sight, &nav, from, &[Some(target)], &pistol, &[]).unwrap();
        let d = tiles_off(stand);
        assert!(d > 20.0 && d <= 22.0, "the pistol at {d} tiles");

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
        // Past the pistol's twenty-two tiles the sniper has it all its own
        // way; inside them the pistol answers, a little.
        assert!(Tactics::fit(&sniper, &target.weapon.stats(), 24.0) > 0.7);
        assert!(Tactics::fit(&sniper, &target.weapon.stats(), 20.0) > 0.2);
    }

    #[test]
    fn a_doorway_is_never_a_stand_since_it_opens_for_whoever_comes() {
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
        let naive =
            Tactics::stand(&sight, &nav, from, &[Some(pistol_at(target))], &stats, &[]).unwrap();
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

    #[test]
    fn the_curves_pin_the_numbers_the_guns_were_asked_for() {
        // Every kind is its own code and its own resource, both ways.
        for kind in WeaponKind::ALL {
            assert_eq!(WeaponKind::from_code(kind.code()), Some(kind));
            assert_eq!(WeaponKind::from_resource(kind.resource()), Some(kind));
        }
        assert_eq!(WeaponKind::from_code(0), None);
        assert_eq!(WeaponKind::from_resource(14), None, "a helm is not a gun");
        assert_eq!(WeaponKind::LaserPistol.resource(), 8);

        // The second tuning of September 2026: the pistol's odds a tenth
        // down from the 95% and 65% it had, its damage a fifth up from 6,
        // and ten tiles more range — 72% at ten tiles now, where the app
        // used to print 70%.
        let pistol = WeaponKind::LaserPistol.stats();
        assert_eq!(pistol.range, 22.0);
        assert!((pistol.hit_chance(10.0) - 0.732).abs() < 0.01);
        assert_eq!(pistol.hit_chance(0.0), 0.855);
        assert_eq!(pistol.hit_chance(30.0), 0.585, "no worse past the range");
        assert_eq!(pistol.damage_at(11.0), 7.2);
        assert!((pistol.dps() - 10.8).abs() < 1e-5);

        let shotgun = WeaponKind::Shotgun.stats();
        assert_eq!(shotgun.damage_at(4.0), 60.0);
        assert_eq!(shotgun.damage_at(2.0), 60.0);
        assert!((shotgun.damage_at(7.0) - 48.0).abs() < 1e-3);
        assert_eq!(shotgun.damage_at(10.0), 36.0);
        assert_eq!(shotgun.hit_chance(4.0), 0.81);
        assert!((shotgun.hit_chance(10.0) - 0.54).abs() < 1e-6);

        let sniper = WeaponKind::SniperRifle.stats();
        assert_eq!(sniper.hit_chance(20.0), 0.9);
        assert_eq!(sniper.damage_at(20.0), 54.0);
        assert!((sniper.hit_chance(35.0) - 0.63).abs() < 1e-6);
        assert_eq!(sniper.damage_at(35.0), 30.0);

        // A burst counts in the rate: eight sixes every four seconds, and
        // the rifle reaches twenty-six tiles now, full to eight.
        let rifle = WeaponKind::AutoRifle.stats();
        assert_eq!(rifle.burst, 8);
        assert_eq!((rifle.range, rifle.sweet), (26.0, 8.0));
        assert!((rifle.dps() - 12.0).abs() < 1e-5);
        assert!(
            (rifle.burst as f32 - 1.0) * rifle.burst_gap <= 2.0,
            "eight in two seconds"
        );

        // A blade reaches a tile and a bit, and swings every two seconds.
        let blade = WeaponKind::Schword.stats();
        assert!(blade.melee);
        assert_eq!(blade.reach(), MELEE_RANGE * TILE);
        assert_eq!(blade.damage_at(1.0), 42.0);
        assert!((1.0 / blade.fire_rate - MELEE_PERIOD).abs() < 1e-6);
        assert!(!pistol.melee && !shotgun.melee && !sniper.melee && !rifle.melee);
    }

    #[test]
    fn damage_falls_off_with_the_distance_the_bolt_flew() {
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

    #[test]
    fn a_body_peeking_from_cover_dodges_half_the_bolts() {
        let (sight, _) = room_with(&[]);
        let mut combat = Combat::new(5);
        let ours = middle(10.0, 5.0);
        let from = middle(8.0, 5.0);
        // Straight at it from two tiles, where the pistol lands nine in
        // ten: standing in the open near enough all of them land, and
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
        assert!(open > 330 && open <= 400, "{open} of 400 in the open");
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

    /// A line of sandbags across the room is no wall — walked over, seen
    /// over — but the tactics stand a body close behind it, and a bolt
    /// coming over it at that body is dodged half the time, like a peek's.
    #[test]
    fn sandbags_are_a_stand_the_tactics_take_and_half_the_bolts_over_them_are_dodged() {
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
            )
            .expect("somewhere to shoot from")
        };
        let stand = plan(&[]);
        assert!(stand.cover, "the bags are cover: {:?}", stand.at);
        let (x, y) = sight.tile_of(stand.at);
        assert_eq!(x, 9, "just this side of the bags, at ({x}, {y})");
        // A squadmate already on that tile: the next one picks another,
        // still behind the bags.
        let second = plan(&[stand.at]);
        assert!((second.at - stand.at).len() >= TILE, "not the same tile");
        assert!(
            second.cover,
            "the next tile of the barricade: {:?}",
            second.at
        );
        assert_eq!(sight.tile_of(second.at).0, 9);
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
        let on_top = landed(&sight, middle(10.0, 3.0));
        assert!(on_top > 300, "{on_top} of 400 on the bags");
        assert!(
            behind > on_top / 2 - 40 && behind < on_top / 2 + 40,
            "{behind} of 400 behind them, {on_top} on them"
        );
    }

    #[test]
    fn an_issued_gear_rolls_every_kind_across_seeds_and_the_same_for_a_seed() {
        let mut seen = [0u32; 5];
        for seed in 0..400u64 {
            let gear = Gear::issued_for(seed);
            let weapon = gear.weapon.expect("always armed");
            // A gun and nothing to wear; a blade and the whole basic set,
            // since a melee bot always wears armour.
            if weapon.stats().melee {
                assert_eq!(gear.armour_health(), 45.0, "the basic set on a blade");
                assert!(
                    Part::ALL
                        .iter()
                        .all(|&p| gear.worn(p).is_some_and(|w| !w.broken()))
                );
            } else {
                assert_eq!(gear.armour_health(), 0.0, "nothing to wear");
            }
            assert!(gear.pack.iter().all(|c| c.is_none()));
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
        );
        combat.brawl(body, 0, WeaponKind::Schword.basic(), 70.0, true, true);
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

        let helm = Piece::new(1, ArmourKind::BasicHelm, Tier::Two);
        assert_eq!(helm.stats().health, balance::BASIC_HELM.health * 1.5);
        assert_eq!(
            helm.health,
            helm.stats().health,
            "fresh at the tier's health"
        );
        assert_eq!(
            helm.effective_protection(),
            balance::BASIC_HELM.protection * 1.5
        );
        assert_eq!(helm.dodge(), 0.0);
        let gold = Piece::new(2, ArmourKind::BasicKevlar, Tier::Three);
        assert_eq!(
            gold.stats().protection,
            balance::BASIC_KEVLAR.protection * 2.25
        );
        assert_eq!(gold.dodge(), balance::TIER_THREE_DODGE);
        let mut gear = Gear::default();
        assert_eq!(gear.dodge(), 0.0);
        gear.body = Some(gold);
        assert!((gear.dodge() - 0.10).abs() < 1e-6);
        gear.head = Some(Piece::new(3, ArmourKind::BasicHelm, Tier::Three));
        assert!((gear.dodge() - 0.19).abs() < 1e-6, "combined, not summed");
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
}
