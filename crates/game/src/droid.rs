//! The infesting race: a machine that is not a Bim (feature 83).
//!
//! A [`Droid`] has no bunk, no memory, no blood, no wounds, no traumas, no
//! gear and no pack. It
//! has a position, a heading, four parts that break, an arm that is part
//! of it, and a route. A droid-held station's room holds them in
//! `Game::droids` beside `bims`, which is empty there; the deck's
//! [`crate::nav::Nav`], [`crate::sight::Sight`] and
//! [`crate::combat::Combat`] serve both.
//!
//! # The body is four parts, and there is no dying
//!
//! Head, Chassis, Arms, Legs — [`DroidPart`] — with their own hit odds
//! ([`crate::balance::DROID_HIT_ODDS`]) and their own healths a kind
//! ([`DroidKind::body`]), every tier above one multiplying all four by
//! `ARMOUR_TIER_STEP` the way a piece of armour's health climbs. Head or
//! Chassis at nothing and the machine is **destroyed at once**: there is
//! no dying state and nothing comes round. Arms at nothing and it fires
//! at [`crate::balance::DROID_ARMS_ACCURACY`] and strikes at
//! [`crate::balance::DROID_ARMS_DAMAGE`]; Legs at nothing and it cannot
//! move but fights where it stands. A hit on a limb already at nothing
//! lands on the Chassis instead, so shooting the legs off a Warden is
//! never a way to make it unkillable.
//!
//! Crew weapons damage these parts directly: a droid wears nothing, so
//! there is no armour step and `strips` does nothing to one.
//!
//! # Nothing is looted
//!
//! A destroyed droid is a wreck: down for everything that asks, worth
//! `XP_ENEMY_DOWN` and `XP_ENEMY_DEAD` together and once, and carrying
//! nothing — the Loot window does not open on one and `take_from_body`
//! refuses it. It lies where it fell until the room closes and blocks
//! nothing.
//!
//! # The drawing
//!
//! Built from the [`DrawList`] primitives and nothing of the Bim's: dark
//! gunmetal and steel with the sensors in the hostile bolt's red. A
//! Husk is low and wide and scuttles on four legs; a Trooper stands with
//! its gun for a forearm; a Warden is the heaviest, shoulder-plated,
//! with the Unmaker as a lance along one side. States: idle, walking,
//! firing, striking, arms gone, legs gone, destroyed.
//!
//! A **wreck is its own drawing**, not the standing machine shrunk: a
//! destroyed one goes to `draw_husk_wreck`, `draw_trooper_wreck` or
//! `draw_warden_wreck` instead, and each is a broken machine rather than
//! a whole one — a Husk's shell split down its length and its legs
//! thrown clear, a Trooper on its back with its head knocked off and its
//! gun arm snapped at the elbow, a Warden broken across with a shoulder
//! plate torn off and the Unmaker in two pieces. Under all three is the
//! same mess ([`Droid::draw_wreck_ground`]): the scorch burnt into the
//! deck, the coolant run out of it — a machine's answer to the blood
//! round a body — and the plate flung clear of it. Where it all lies is
//! [`Droid::scatter`], a hash of where the machine fell rather than a
//! roll, so a wreck lies the same way on every frame and after a load.
//! A fresh one still burns in the rift ([`EMBER_LIFE`]) and spits sparks
//! ([`WRECK_SPARKS`]); an old one is cold.

use crate::balance;
use crate::combat::{HIT_RADIUS, Tier, Weapon, WeaponKind, WeaponStats};
use crate::draw::{Brush, Color, DrawList};
use crate::math::{TAU, Vec2, angle_lerp, clamp, vec2};

// --- the look -----------------------------------------------------------

/// The hull: dark gunmetal, nothing like a coverall.
const HULL: Color = Color::rgb(0.22, 0.24, 0.27);
const HULL_DARK: Color = Color::rgb(0.15, 0.16, 0.18);
/// The plating over it, a shade lighter, and the steel of a limb.
const PLATE: Color = Color::rgb(0.33, 0.36, 0.40);
const STEEL: Color = Color::rgb(0.50, 0.53, 0.57);
/// A joint, and the shade under a plate's edge.
const JOINT: Color = Color::rgb(0.11, 0.12, 0.14);
/// The sensor lights, in the hostile bolt's own red so that a machine
/// reads as an enemy before its shape does.
const SENSOR: Color = crate::combat::HOSTILE_BOLT;
/// A sensor that has gone out.
const SENSOR_OUT: Color = Color::rgb(0.24, 0.13, 0.12);
/// What flies off a struck part, and what a wreck throws for a while.
const SPARK: Color = Color::rgb(1.0, 0.86, 0.48);
/// And how far past white those sparks are drawn: hot metal, so they glow
/// (feature 98) — a little, since a wreck throws them for seconds.
const SPARK_HEAT: f32 = 1.6;
/// The shadow under the machine.
const SHADOW: Color = Color::rgba(0.0, 0.0, 0.0, 0.24);
/// The scorch a wreck burns into the deck it fell on — under everything,
/// wider than the machine, and thin enough that the plating shows
/// through it.
const SCORCH: Color = Color::rgba(0.04, 0.04, 0.05, 0.5);
/// What runs out of a broken machine: dark and oily, never the Bim's
/// red. A wreck's own answer to the blood round a body.
const COOLANT: Color = Color::rgba(0.09, 0.24, 0.20, 0.9);
/// The torn edge of a plate pulled off its hull, and a shard of one.
const TORN: Color = Color::rgb(0.40, 0.42, 0.45);
/// What glows in a fresh rift and dies away over [`EMBER_LIFE`].
const EMBER: Color = Color::rgb(0.96, 0.44, 0.15);
/// A Guardian's shield (feature 100): a translucent plate in the machines'
/// red with a rim past white, so the bloom picks the rim out and nothing
/// else of it; and the same arc laid faintly on the deck.
const SHIELD: Color = SENSOR;
const SHIELD_RIM: Color = Color::rgb(1.4, 0.77, 0.67);
const SHIELD_DECK: Color = Color::rgba(1.0, 0.28, 0.22, 0.16);
/// Half the shield's arc, in radians: the ±60° its cosine says.
const SHIELD_HALF_ARC: f32 = core::f32::consts::FRAC_PI_3;
/// How far out the arc on the deck is laid, as a share of the plate's
/// radius: a little beyond it, so the plate does not hide it.
const SHIELD_DECK_REACH: f32 = 1.25;

// --- the animation ------------------------------------------------------

/// How sharply a droid swings round to the heading it wants. Slower than
/// a Bim's: it is a machine, and it turns like one.
const TURN_RATE: f32 = 4.0;
/// What a Bim marches at, in room units a second, and what each kind's
/// pace is a share of.
const MARCH: f32 = 96.0;
/// How close to a waypoint counts as rounded, and how close to the last
/// one counts as arrived.
const WAYPOINT_RADIUS: f32 = 11.0;
const ARRIVE_RADIUS: f32 = 6.0;
/// How far the body's middle is kept clear of walls and furniture: the
/// Bim's, so a droid navigates the deck the way everything else does.
pub const BODY_MARGIN: f32 = crate::character::BODY_MARGIN;
/// How long a muzzle flash is drawn, and how long a claw stays shut.
const FLASH_TIME: f32 = 0.12;
const SNAP_TIME: f32 = crate::balance::SWING_TIME;
/// How long a spark on a struck part is drawn.
const SPARK_LIFE: f32 = 0.35;
/// How long a fresh wreck throws sparks.
const WRECK_SPARKS: f32 = 3.0;
/// How long the ember inside a fresh wreck's rift glows for. Longer than
/// the sparks: the machine stops spitting before it stops burning.
const EMBER_LIFE: f32 = 9.0;
/// How many shards a wreck throws clear of itself, and how far out they
/// lie as a share of its spread.
const SHARDS: u32 = 7;
/// How many sparks a hit throws, and how far they fly.
const SPARK_COUNT: usize = 4;
const SPARK_THROW: f32 = 9.0;
/// How long the Warden's muzzle ring brightens before a shot.
const CHARGE_TIME: f32 = 0.45;

// --- the Guardian's turn (feature 100) ------------------------------------

/// A Guardian turns in fixed sub-steps, never by an angle worked out: each
/// is a rotation by a written-out cosine and sine, so two platforms turn it
/// alike to the bit. One sub-step is 1.25°, sixty of them a second —
/// [`balance::GUARDIAN_TURN_DEGREES`], 75° a second.
pub const TURN_STEP_COS: f32 = 0.999_762_03;
pub const TURN_STEP_SIN: f32 = 0.021_814_885;
pub const TURN_STEPS_A_SECOND: f32 = 60.0;

/// How near the target its heading has to be before a Guardian starts to
/// wind up: within 15° (a cosine), so the beam leaves the lens along the
/// way the machine faces rather than out of its flank.
pub const WINDUP_COS: f32 = 0.965_925_8;

/// How far ahead of its middle a Guardian's lens is set, in room units:
/// where the beam leaves from.
pub const LENS_AHEAD: f32 = 8.0;

/// Where a Guardian's Sweeper is in its rhythm (feature 100, see
/// `balance::SWEEPER_WINDUP`). A machine of any other kind is always
/// `Ready` and never asked.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Beam {
    /// Nothing wound up, nothing cooling: the next target in sight and in
    /// front of it starts a wind-up.
    #[default]
    Ready,
    /// The lens brightening on a fixed aim: `aim` the unit direction the
    /// beam will be laid along, `at` the point it was fixed on, `mark` the
    /// target it was chosen for (an index into the machines' list), and
    /// `left` seconds to go. The heading is held from here to the end of
    /// the sweep.
    WindUp {
        left: f32,
        aim: Vec2,
        at: Vec2,
        mark: usize,
    },
    /// The beam sweeping, `left` seconds to go, about the aim the wind-up
    /// fixed, from the side `side` (one or minus one) round to the other.
    /// The heading is still held.
    Sweep { left: f32, aim: Vec2, side: f32 },
    /// Resting after a sweep, `left` seconds to go.
    Cooling { left: f32 },
}

impl Beam {
    /// Whether the heading is held where the wind-up fixed it.
    pub fn holds_heading(self) -> bool {
        matches!(self, Beam::WindUp { .. } | Beam::Sweep { .. })
    }
}

/// Which of the four a hit landed on. The discriminants are the codes
/// anything outside this crate reads, written out so a reordering cannot
/// renumber one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DroidPart {
    Head = 0,
    Chassis = 1,
    Arms = 2,
    Legs = 3,
}

impl DroidPart {
    pub const ALL: [DroidPart; 4] = [
        DroidPart::Head,
        DroidPart::Chassis,
        DroidPart::Arms,
        DroidPart::Legs,
    ];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<DroidPart> {
        DroidPart::ALL.iter().copied().find(|p| p.code() == code)
    }

    /// The part a roll of 0 to 1 lands on, by
    /// [`balance::DROID_HIT_ODDS`]. The same shape as
    /// `health::Part::hit_by`, and read off the same draw — a
    /// `combat::Hit` carries its roll so that which kind of body a
    /// target turns out to be does not move the combat stream.
    pub fn hit_by(roll: f32) -> DroidPart {
        let mut edge = 0.0;
        for (i, part) in DroidPart::ALL.iter().enumerate() {
            edge += balance::DROID_HIT_ODDS[i];
            if roll < edge {
                return *part;
            }
        }
        DroidPart::Chassis
    }
}

/// The four machines. Codes written out, never renumbered.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DroidKind {
    /// Low, wide and quick, and it has only its claws.
    Husk = 1,
    /// Upright, with a gun for a forearm. It advances in the open.
    Trooper = 2,
    /// The heaviest, and the one that takes cover: the Unmaker's carrier.
    Warden = 3,
    /// The largest (feature 100): a heavy walker behind a frontal shield
    /// that stops everything from the front, with a beam that sweeps. It
    /// is beaten by getting round it. Only ever at tier three.
    Guardian = 4,
}

impl DroidKind {
    pub const ALL: [DroidKind; 4] = [
        DroidKind::Husk,
        DroidKind::Trooper,
        DroidKind::Warden,
        DroidKind::Guardian,
    ];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<DroidKind> {
        DroidKind::ALL.iter().copied().find(|k| k.code() == code)
    }

    /// What each of the four parts has at tier one, in
    /// [`DroidPart::ALL`] order.
    pub fn body(self) -> [f32; 4] {
        match self {
            DroidKind::Husk => balance::HUSK_BODY,
            DroidKind::Trooper => balance::TROOPER_BODY,
            DroidKind::Warden => balance::WARDEN_BODY,
            DroidKind::Guardian => balance::GUARDIAN_BODY,
        }
    }

    /// What it walks at, as a share of a Bim's marching pace.
    pub fn pace(self) -> f32 {
        match self {
            DroidKind::Husk => balance::HUSK_PACE,
            DroidKind::Trooper => balance::TROOPER_PACE,
            DroidKind::Warden => balance::WARDEN_PACE,
            DroidKind::Guardian => balance::GUARDIAN_PACE,
        }
    }

    /// How wide it is drawn from the middle out, in room units. Nothing
    /// is over thirty: a machine that would not fit a corridor is a
    /// machine that would be drawn through the walls.
    pub fn half_width(self) -> f32 {
        match self {
            DroidKind::Husk => 15.0,
            DroidKind::Trooper => 17.0,
            DroidKind::Warden => 26.0,
            DroidKind::Guardian => 28.0,
        }
    }

    /// Whether it takes cover and peeks when it picks a stand. A Trooper
    /// does not: it advances in the open and fires on the move. Nor does a
    /// Guardian: its shield is its cover, and it advances behind it.
    pub fn takes_cover(self) -> bool {
        matches!(self, DroidKind::Warden)
    }

    /// The arm the machine was built with. A Trooper's is dealt by its
    /// place in the wave — see [`DroidKind::trooper_arm`].
    pub fn arm(self, index: usize) -> WeaponKind {
        match self {
            DroidKind::Husk => WeaponKind::Claw,
            DroidKind::Trooper => DroidKind::trooper_arm(index),
            DroidKind::Warden => WeaponKind::Unmaker,
            DroidKind::Guardian => WeaponKind::Sweeper,
        }
    }

    /// Troopers alternate pistol and auto rifle by their place in the
    /// wave: the stats are a Bim's guns', the arm is not.
    pub fn trooper_arm(index: usize) -> WeaponKind {
        if index % 2 == 0 {
            WeaponKind::LaserPistol
        } else {
            WeaponKind::AutoRifle
        }
    }
}

/// One machine's four parts, each with what it has and what it had.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DroidBody {
    health: [f32; 4],
    max: [f32; 4],
}

impl DroidBody {
    /// A whole body of that kind at that tier: every part multiplied by
    /// `ARMOUR_TIER_STEP` for each tier above one.
    pub fn new(kind: DroidKind, tier: Tier) -> DroidBody {
        let step = match tier {
            Tier::One => 1.0,
            Tier::Two => balance::ARMOUR_TIER_STEP,
            Tier::Three => balance::ARMOUR_TIER_STEP * balance::ARMOUR_TIER_STEP,
        };
        let mut max = kind.body();
        for h in &mut max {
            *h *= step;
        }
        DroidBody { health: max, max }
    }

    pub fn health(&self, part: DroidPart) -> f32 {
        self.health[part.code() as usize]
    }

    pub fn max(&self, part: DroidPart) -> f32 {
        self.max[part.code() as usize]
    }

    /// What is left of a part as a share of what it had, 0 to 1.
    pub fn share(&self, part: DroidPart) -> f32 {
        let max = self.max(part);
        if max <= 0.0 {
            0.0
        } else {
            clamp(self.health(part) / max, 0.0, 1.0)
        }
    }

    pub fn gone(&self, part: DroidPart) -> bool {
        self.health(part) <= 0.0
    }

    /// Whether the machine is finished: head or chassis at nothing.
    /// There is no dying state — it stops the instant either goes.
    pub fn destroyed(&self) -> bool {
        self.gone(DroidPart::Head) || self.gone(DroidPart::Chassis)
    }

    /// Take `damage` off `part`, and answer which part actually took it:
    /// **a hit on a limb already at nothing lands on the Chassis**, so
    /// nothing can be made unkillable by shooting its legs off first.
    /// The head and the chassis themselves take what they are given.
    pub fn take(&mut self, part: DroidPart, damage: f32) -> DroidPart {
        let part = match part {
            DroidPart::Arms | DroidPart::Legs if self.gone(part) => DroidPart::Chassis,
            other => other,
        };
        let i = part.code() as usize;
        self.health[i] = (self.health[i] - damage.max(0.0)).max(0.0);
        part
    }
}

/// A spark thrown off a struck part, or off a fresh wreck: where on the
/// body, which way, and how long it has been lit.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct Spark {
    /// In the machine's own frame, so it rides the body round.
    at: Vec2,
    dir: Vec2,
    age: f32,
}

/// One machine of the infesting race. Not a Bim, and not a
/// [`crate::character::Character`]: everything it is is here.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Droid {
    pub kind: DroidKind,
    pub tier: Tier,
    /// Which wave it arrived with, so the world can say when a wave is
    /// spent and draw its ship while any of it is alive.
    pub wave: u32,
    /// Where it stands, in room units, and which way it faces — the
    /// angle the picture is drawn at. A Guardian's is read off
    /// [`Droid::facing`] for the picture alone.
    pub pos: Vec2,
    pub heading: f32,
    /// Which way it faces as a **unit vector**: what a Guardian's shield
    /// and its turn are worked in (feature 100), since a vector is turned
    /// and compared by plain arithmetic. Kept for every kind, and read for
    /// the Guardian alone.
    #[cfg_attr(feature = "serde", serde(default = "unit_x"))]
    facing: Vec2,
    /// The part of a turn's sub-step owed to the next step: a Guardian
    /// turns [`TURN_STEPS_A_SECOND`] sub-steps a second, whatever the
    /// step, and what a step's time does not make a whole one of waits.
    #[cfg_attr(feature = "serde", serde(default))]
    turn_left: f32,
    /// Where a Guardian's Sweeper is in its rhythm; `Ready` for every
    /// other kind.
    #[cfg_attr(feature = "serde", serde(default))]
    pub beam: Beam,
    /// How many sweeps it has let go: the side the next starts from
    /// alternates by it.
    #[cfg_attr(feature = "serde", serde(default))]
    pub sweeps: u32,
    /// Where it is steering, the way a Bim's intent works.
    intent: f32,
    /// The arm it was built with, as a [`Weapon`] so every curve, every
    /// tactic and every tooltip reads it the way it reads a Bim's — but
    /// never an `Item`, never in a hold, never in a hand.
    pub weapon: Weapon,
    pub body: DroidBody,
    /// The route it is walking, nearest waypoint first; empty when it
    /// stands.
    route: Vec<Vec2>,
    /// How long until it plans a stand again, and until it tries the
    /// doors again.
    pub plan_wait: f32,
    pub breach_wait: f32,
    /// Which door it is heaving at, if any — the room's index.
    pub smashing: Option<usize>,
    /// Whether it has lost sight of everything and is walking to where
    /// something was last seen. Same rule as a hostile Bim's hunt: a
    /// hunter holds or closes and never gives ground.
    pub hunting: bool,
    /// The trigger of the arm, kept between steps like a Bim's.
    pub trigger: crate::combat::Trigger,
    /// Seconds until the next blow may be started, and the blow on its
    /// way. A claw is a melee weapon and locks the way a blade does.
    pub melee_timer: f32,
    pub blow: Option<crate::combat::Blow>,
    /// Which target has it locked in a melee, if any.
    pub locked: Option<usize>,
    /// The peek it is leaning out to while it aims from cover; `None`
    /// for one firing from its own eyes. Only a Warden ever has one.
    pub peek: Option<Vec2>,
    /// Destroyed: a wreck where it fell. Nothing walks, nothing fires,
    /// nothing may be taken off it.
    pub destroyed: bool,
    /// Held still for a **picture** (`World::stage_droids_for_probe`):
    /// it plans nothing, walks nowhere and fires at nobody, so a run
    /// of frames down to a screenshot leaves it where it was put.
    /// Nothing in the game sets it.
    pub posing: bool,
    /// And held at the **instant of firing or striking** with it: the
    /// muzzle lit, the claws shut halfway, the Warden's ring charged.
    /// A flash is a tenth of a second and a screenshot would not catch
    /// one otherwise.
    pub lit: bool,
    /// How long it has been one, for the sparks that die away.
    wreck_age: f32,
    /// The walk phase, the idle phase, and the timers the picture reads.
    stride: f32,
    idle: f32,
    speed: f32,
    flash: f32,
    snap: f32,
    /// How long the trigger has been held down with a target in sight:
    /// what brightens a Warden's muzzle ring before the shot.
    charge: f32,
    sparks: Vec<Spark>,
    /// Its own little stream, for the sparks alone — nothing it rolls
    /// may move the room's or the fight's.
    rng: crate::rng::Rng,
}

impl Droid {
    /// One machine of that kind and tier, stood at `at` facing `heading`,
    /// with the arm its kind and place in the wave give it. `seed` is the
    /// stream its sparks are drawn from and nothing else.
    pub fn new(
        kind: DroidKind,
        tier: Tier,
        index: usize,
        wave: u32,
        at: Vec2,
        heading: f32,
        seed: u64,
    ) -> Droid {
        Droid {
            kind,
            tier,
            wave,
            pos: at,
            heading,
            facing: Vec2::from_angle(heading),
            turn_left: 0.0,
            beam: Beam::Ready,
            sweeps: 0,
            intent: heading,
            weapon: kind.arm(index).at(tier),
            body: DroidBody::new(kind, tier),
            route: Vec::new(),
            plan_wait: 0.0,
            breach_wait: 0.0,
            smashing: None,
            hunting: false,
            trigger: crate::combat::Trigger::default(),
            melee_timer: 0.0,
            blow: None,
            locked: None,
            peek: None,
            destroyed: false,
            posing: false,
            lit: false,
            wreck_age: 0.0,
            stride: 0.0,
            idle: 0.0,
            speed: 0.0,
            flash: 0.0,
            snap: 0.0,
            charge: 0.0,
            sparks: Vec::new(),
            rng: crate::rng::Rng::new(seed ^ 0xD0_1D),
        }
    }

    /// Where a shot at it is aimed: the peek it leans out to while it
    /// aims from one, else where it stands. The same rule as a Bim's
    /// [`crate::game::Game::exposed_at`].
    pub fn exposed_at(&self) -> Vec2 {
        self.peek.unwrap_or(self.pos)
    }

    /// How hard a hit has to land to count — a machine is as big a mark
    /// as a body.
    pub fn hit_radius(&self) -> f32 {
        HIT_RADIUS
    }

    /// Whether it can move at all: legs at nothing and it stands and
    /// fights where it is.
    pub fn can_move(&self) -> bool {
        !self.destroyed && !self.body.gone(DroidPart::Legs)
    }

    /// The arm's numbers with the machine's own state in them: arms at
    /// nothing and a gun's odds are halved and a claw's damage is,
    /// [`balance::DROID_ARMS_ACCURACY`] and
    /// [`balance::DROID_ARMS_DAMAGE`]. The Unmaker counts as a gun, so
    /// its odds fall and its damage and its strip do not.
    pub fn stats(&self) -> WeaponStats {
        let base = self.weapon.stats();
        if !self.body.gone(DroidPart::Arms) {
            return base;
        }
        // A claw, and the Guardian's beam, which rolls no odds: the damage
        // is what falls (feature 100).
        if base.melee || self.weapon.kind == WeaponKind::Sweeper {
            WeaponStats {
                damage: base.damage * balance::DROID_ARMS_DAMAGE,
                damage_far: base.damage_far * balance::DROID_ARMS_DAMAGE,
                ..base
            }
        } else {
            WeaponStats {
                accuracy: base.accuracy * balance::DROID_ARMS_ACCURACY,
                accuracy_far: base.accuracy_far * balance::DROID_ARMS_ACCURACY,
                ..base
            }
        }
    }

    /// A hit on it: the part rolled by the caller off the combat
    /// stream's own draw, the damage taken off it, and the machine
    /// destroyed at once if the head or the chassis goes. Answers the
    /// part that actually took it — a limb already gone passes it to the
    /// chassis — so a caller that wants to say what was struck says the
    /// truth. A wreck takes nothing.
    pub fn strike(&mut self, part: DroidPart, damage: f32) -> Option<DroidPart> {
        if self.destroyed {
            return None;
        }
        let struck = self.body.take(part, damage);
        self.spark_on(struck);
        if self.body.destroyed() {
            self.destroy();
        } else if self.body.gone(DroidPart::Legs) {
            // It stops where it stands the instant the legs go, rather
            // than sliding on to the end of the route it had.
            self.route.clear();
            self.speed = 0.0;
        }
        Some(struck)
    }

    /// Finished: the route dropped, the arm silent, the sensor out.
    pub fn destroy(&mut self) {
        if self.destroyed {
            return;
        }
        self.destroyed = true;
        self.wreck_age = 0.0;
        self.route.clear();
        self.speed = 0.0;
        self.blow = None;
        self.locked = None;
        self.peek = None;
        self.smashing = None;
        self.trigger.hold();
        // A wreck throws a good deal more than a hit does, and they die
        // away over `WRECK_SPARKS`.
        for _ in 0..SPARK_COUNT * 2 {
            self.throw_spark(Vec2::ZERO);
        }
    }

    /// Where on the body a struck part is, in the machine's own frame —
    /// what a spark is thrown from.
    fn part_at(&self, part: DroidPart) -> Vec2 {
        let w = self.kind.half_width();
        match part {
            DroidPart::Head => vec2(w * 0.45, 0.0),
            DroidPart::Chassis => Vec2::ZERO,
            DroidPart::Arms => vec2(w * 0.25, w * 0.55),
            DroidPart::Legs => vec2(-w * 0.35, 0.0),
        }
    }

    /// Where a hit on `part` shows and how big (feature 98's flash on the
    /// part struck), in room units: the same place its sparks fly from,
    /// turned with the machine. Drawing only.
    pub fn part_mark(&self, part: DroidPart) -> (Vec2, f32) {
        let w = self.kind.half_width();
        let radius = match part {
            DroidPart::Head => w * 0.35,
            DroidPart::Chassis => w * 0.6,
            DroidPart::Arms | DroidPart::Legs => w * 0.32,
        };
        (self.pos + self.part_at(part).rotate(self.heading), radius)
    }

    /// Where the arm it is built with ends, in room units: the muzzle its
    /// own drawing puts at the end of a Trooper's forearm or a Warden's
    /// lance, arms gone or not — where a shot's glow is lit (feature 98).
    /// The shot itself is traced from the eye, as it always was; this is
    /// the picture's.
    pub fn muzzle(&self) -> Vec2 {
        let droop = if self.body.gone(DroidPart::Arms) {
            1.0
        } else {
            0.0
        };
        let local = match self.kind {
            DroidKind::Trooper => {
                let shoulder = vec2(3.0, 11.0);
                shoulder + vec2(13.0 - droop * 9.0, 2.0 + droop * 7.0) + vec2(6.0, 0.0)
            }
            DroidKind::Warden => vec2(30.0 - droop * 10.0, 14.0 + droop * 8.0),
            DroidKind::Husk => vec2(self.kind.half_width(), 0.0),
            // The lens in the middle of the chassis, a little forward: the
            // beam comes out of it (feature 100).
            DroidKind::Guardian => vec2(LENS_AHEAD, 0.0),
        };
        self.pos + local.rotate(self.heading)
    }

    fn spark_on(&mut self, part: DroidPart) {
        let at = self.part_at(part);
        for _ in 0..SPARK_COUNT {
            self.throw_spark(at);
        }
    }

    fn throw_spark(&mut self, at: Vec2) {
        let angle = self.rng.unit() * TAU;
        let throw = SPARK_THROW * (0.4 + 0.6 * self.rng.unit());
        self.sparks.push(Spark {
            at,
            dir: Vec2::from_angle(angle) * throw,
            age: 0.0,
        });
    }

    /// The route it is walking, for a caller that wants to know where it
    /// is bound: the last waypoint, or where it stands.
    pub fn destination(&self) -> Vec2 {
        self.route.last().copied().unwrap_or(self.pos)
    }

    pub fn is_walking(&self) -> bool {
        !self.route.is_empty()
    }

    /// Walk this route from here — the nav's, waypoints in order. A
    /// machine that cannot move takes none.
    pub fn follow_path(&mut self, route: Vec<Vec2>) {
        if !self.can_move() {
            return;
        }
        self.route = route;
    }

    pub fn halt(&mut self) {
        self.route.clear();
    }

    /// Face this way.
    pub fn face(&mut self, angle: f32) {
        self.intent = angle;
    }

    /// Which way it faces, a unit vector: what a Guardian's shield and its
    /// turn are worked in (feature 100).
    pub fn facing(&self) -> Vec2 {
        self.facing
    }

    /// Stood facing `dir` from the start — the way a wave is built facing
    /// in from its airlock or its gate, said as the vector the world has
    /// rather than as an angle.
    pub fn with_facing(mut self, dir: Vec2) -> Droid {
        self.face_for_probe(dir);
        self
    }

    /// Faced `dir` at once, the turn rate never asked: for a machine being
    /// stood somewhere (a wave built, a test, a picture). Nothing in a
    /// fight turns one so.
    pub fn face_for_probe(&mut self, dir: Vec2) {
        let dir = dir.normalize_or_zero();
        if dir != Vec2::ZERO {
            self.facing = dir;
            self.heading = dir.angle();
            self.intent = self.heading;
        }
    }

    /// Whether it is the Guardian, which faces and turns by its own rule.
    pub fn is_guardian(&self) -> bool {
        self.kind == DroidKind::Guardian
    }

    /// The way its shield faces, for a standing Guardian — what the world
    /// hands the crew's room with the target (`Combat::set_shields`) — and
    /// `None` for a wreck or any other kind. **The shield cannot be
    /// broken**: nothing but the machine being destroyed takes it away.
    pub fn shield(&self) -> Option<Vec2> {
        (self.is_guardian() && !self.destroyed).then_some(self.facing)
    }

    /// Turn a Guardian towards `want` (any length) by as many of its
    /// fixed sub-steps as this step's time buys: never more than
    /// [`balance::GUARDIAN_TURN_DEGREES`] a second, and never past the
    /// way it wants — within a sub-step of it, it faces it exactly. No
    /// angle is worked out: the sense of the turn is a cross product and
    /// the turn a rotation by [`TURN_STEP_COS`] and [`TURN_STEP_SIN`].
    /// Held while the Sweeper is wound up (`Beam::holds_heading`), and
    /// nothing for a wreck or a zero `want`.
    pub fn turn_toward(&mut self, want: Vec2, dt: f32) {
        let want = want.normalize_or_zero();
        if self.destroyed || want == Vec2::ZERO || self.beam.holds_heading() {
            self.turn_left = 0.0;
            return;
        }
        self.turn_left += dt * TURN_STEPS_A_SECOND;
        while self.turn_left >= 1.0 {
            self.turn_left -= 1.0;
            if self.facing.dot(want) >= TURN_STEP_COS {
                // Within a sub-step: it faces it, and has nothing owed.
                self.facing = want;
                self.turn_left = 0.0;
                break;
            }
            // Clockwise on screen for a positive cross product, and — for
            // a want dead behind, where it is nought — clockwise too.
            let sin = if self.facing.perp_dot(want) >= 0.0 {
                TURN_STEP_SIN
            } else {
                -TURN_STEP_SIN
            };
            self.facing = self
                .facing
                .rotate_by(TURN_STEP_COS, sin)
                .normalize_or_zero();
        }
        // The picture's angle, read off the vector: drawing only.
        self.heading = self.facing.angle();
        self.intent = self.heading;
    }

    /// One step of the machine's own motion and its picture: the route
    /// walked at its pace, the heading swung round to where it is going
    /// or to what it was told to face, and every timer aged. Whether it
    /// fires, strikes, plans or breaches is the room's
    /// (`Game::tick_droids`) — this is the body alone.
    pub fn walk(&mut self, dt: f32) {
        self.idle += dt;
        for s in &mut self.sparks {
            s.age += dt;
        }
        self.sparks.retain(|s| s.age < SPARK_LIFE);
        if self.lit {
            // Held at the instant, so a screenshot catches a flash that
            // is otherwise a tenth of a second long.
            self.flash = FLASH_TIME;
            self.snap = SNAP_TIME * 0.5;
            self.charge = CHARGE_TIME;
        } else {
            self.flash = (self.flash - dt).max(0.0);
            self.snap = (self.snap - dt).max(0.0);
        }
        if self.destroyed {
            self.wreck_age += dt;
            // A fresh wreck spits for a few seconds and then lies still.
            if self.wreck_age < WRECK_SPARKS && self.rng.chance(clamp(dt * 3.0, 0.0, 1.0)) {
                self.throw_spark(Vec2::ZERO);
            }
            self.speed = 0.0;
            return;
        }
        // A Guardian's heading is its own (feature 100): turned by
        // `turn_toward` in whole sub-steps, never eased here.
        let eases = !self.is_guardian();
        if !self.can_move() {
            self.route.clear();
            self.speed = 0.0;
            if eases {
                self.heading =
                    angle_lerp(self.heading, self.intent, clamp(dt * TURN_RATE, 0.0, 1.0));
            }
            return;
        }
        let pace = MARCH * self.kind.pace();
        if let Some(&to) = self.route.first() {
            let away = to - self.pos;
            let span = away.len();
            let last = self.route.len() == 1;
            let reached = if last { ARRIVE_RADIUS } else { WAYPOINT_RADIUS };
            if span <= reached {
                self.route.remove(0);
                self.speed = 0.0;
            } else {
                let dir = away * (1.0 / span.max(1e-6));
                let stride = (pace * dt).min(span);
                self.pos = self.pos + dir * stride;
                self.speed = pace;
                self.intent = dir.angle();
            }
        } else {
            self.speed = 0.0;
        }
        // The stride runs with the walking, so the legs step at the pace
        // the machine actually moves at.
        self.stride += dt * (2.0 + 6.0 * (self.speed / MARCH));
        if eases {
            self.heading = angle_lerp(self.heading, self.intent, clamp(dt * TURN_RATE, 0.0, 1.0));
        }
    }

    /// The way it is walking, towards the next waypoint of its route, or
    /// nothing while it stands: what a Guardian with nothing in sight
    /// turns to face.
    pub fn walking_toward(&self) -> Option<Vec2> {
        self.route
            .first()
            .map(|&to| (to - self.pos).normalize_or_zero())
            .filter(|d| *d != Vec2::ZERO)
    }

    /// The trigger was pulled: the muzzle flashes.
    pub fn fired(&mut self) {
        self.flash = FLASH_TIME;
        self.charge = 0.0;
    }

    /// A blow was started: the claws snap shut.
    pub fn struck_out(&mut self) {
        self.snap = SNAP_TIME;
    }

    /// Held on a target without firing yet: what brightens a Warden's
    /// muzzle ring. Said every step it has something in its sights.
    pub fn charging(&mut self, dt: f32, on: bool) {
        // A machine held at the instant keeps its ring lit.
        if self.lit {
            return;
        }
        self.charge = if on {
            (self.charge + dt).min(CHARGE_TIME)
        } else {
            0.0
        };
    }

    // --- the picture ----------------------------------------------------

    /// Draw it as it stands. Nothing of the Bim's: no body, no head, no
    /// hair, no clothes, no armour layers, no held item.
    pub fn draw(&self, list: &mut DrawList) {
        let scale = if self.destroyed { 0.88 } else { 1.0 };
        // A soft shadow (feature 98), and a wreck casts less of one, being
        // flatter and smaller.
        let w = self.kind.half_width();
        list.soft_ellipse(
            self.pos + vec2(0.0, w * 0.22),
            vec2(w * 1.7, w * 1.9) * scale,
            self.heading,
            SHADOW,
        );
        // A Guardian's shield marked faintly on the deck under it (feature
        // 100), so the arc it stops things in is read before a shot is.
        if self.is_guardian() && !self.destroyed {
            self.draw_shield_on_deck(list);
        }
        // The sparks, over the machine: the struck part's, and a fresh
        // wreck's. Put through the brush's frame first, so they ride the
        // body's turn, and drawn after the machine so they sit on top of
        // it — the painter cannot be borrowed twice at once.
        let mut sparks: Vec<(Vec2, f32, f32)> = Vec::with_capacity(self.sparks.len());
        {
            let mut b = list.brush(self.pos, self.heading, scale);
            match (self.kind, self.destroyed) {
                (DroidKind::Husk, false) => self.draw_husk(&mut b),
                (DroidKind::Trooper, false) => self.draw_trooper(&mut b),
                (DroidKind::Warden, false) => self.draw_warden(&mut b),
                (DroidKind::Guardian, false) => self.draw_guardian(&mut b),
                (DroidKind::Husk, true) => self.draw_husk_wreck(&mut b),
                (DroidKind::Trooper, true) => self.draw_trooper_wreck(&mut b),
                (DroidKind::Warden, true) => self.draw_warden_wreck(&mut b),
                (DroidKind::Guardian, true) => self.draw_guardian_wreck(&mut b),
            }
            for s in &self.sparks {
                let t = 1.0 - s.age / SPARK_LIFE;
                sparks.push((b.to_world(s.at + s.dir * (1.0 - t)), t, 0.0));
            }
        }
        // And the shield's plate, standing out in front of it.
        if self.is_guardian() && !self.destroyed {
            self.draw_shield_plate(list);
        }
        for (at, t, _) in sparks {
            list.circle(at, 2.0 + 3.0 * t, SPARK.glowing(SPARK_HEAT).alpha(0.9 * t));
        }
    }

    /// The sensor's colour: out on a wreck, and dimmed when the head is
    /// most of the way gone.
    fn eye(&self) -> Color {
        if self.destroyed {
            return SENSOR_OUT;
        }
        let lit = 0.35 + 0.65 * self.body.share(DroidPart::Head);
        SENSOR_OUT.mix(SENSOR, lit)
    }

    /// The step of a leg, -1 to 1: nought when it is not walking and
    /// nought when the legs are gone, so a crippled machine's legs do
    /// not paddle on the spot.
    fn step(&self) -> f32 {
        if self.destroyed || !self.can_move() {
            return 0.0;
        }
        self.stride.sin() * clamp(self.speed / MARCH, 0.0, 1.0)
    }

    /// The idle sway a standing machine has: a good deal less than a
    /// Bim's breathing, since it does not breathe.
    fn sway(&self) -> f32 {
        if self.destroyed {
            0.0
        } else {
            (self.idle * 1.3).sin() * 0.5
        }
    }

    /// **The Husk**: low and wide, crab-like. A flat hull with four short
    /// legs that scuttle and two forward claws that snap shut on a
    /// strike. Legs gone and it lies flat on its hull.
    fn draw_husk(&self, b: &mut Brush) {
        let step = self.step();
        let flat = !self.can_move();
        let legs_gone = self.body.gone(DroidPart::Legs);
        let arms_gone = self.body.gone(DroidPart::Arms);
        // Four short legs, two a side, splayed out and under the hull.
        // A scuttle is the fore pair and the aft pair out of phase.
        if !legs_gone {
            for (i, (ahead, side)) in [(7.0f32, 1.0f32), (-6.0, 1.0), (7.0, -1.0), (-6.0, -1.0)]
                .into_iter()
                .enumerate()
            {
                let phase = if (i / 2) % 2 == 0 { step } else { -step };
                let swing = phase * 3.5;
                let knee = vec2(ahead + swing, 10.0 * side);
                let foot = vec2(ahead + swing * 1.8, 16.5 * side);
                b.rect(knee, vec2(8.0, 4.4), 0.5 * side, 2.0, STEEL);
                b.rect(foot, vec2(7.0, 3.6), 1.0 * side, 1.8, JOINT);
            }
        } else {
            // Dragged under it: the legs are there, folded and useless.
            for side in [-1.0f32, 1.0] {
                b.rect(vec2(0.0, 9.0 * side), vec2(13.0, 3.4), 0.0, 1.6, JOINT);
            }
        }
        // The hull: a flat shell, wider than it is long.
        let lift = if flat { 0.92 } else { 1.0 };
        b.ellipse(Vec2::ZERO, vec2(26.0, 30.0) * lift, 0.0, HULL_DARK);
        b.ellipse(vec2(1.0, 0.0), vec2(21.0, 24.5) * lift, 0.0, HULL);
        // A ridge down the back, and the plating either side of it.
        b.rect(vec2(-1.0, 0.0), vec2(17.0, 6.5), 0.0, 2.5, PLATE);
        for side in [-1.0f32, 1.0] {
            b.rect(vec2(0.0, 8.0 * side), vec2(14.0, 5.0), 0.0, 2.0, HULL_DARK);
        }
        // Two sensor lights up front, low on the shell.
        for side in [-1.0f32, 1.0] {
            b.ellipse(vec2(9.5, 4.5 * side), vec2(4.2, 3.2), 0.0, self.eye());
        }
        // The claws, one either side of the front. They open in the idle
        // and snap shut through a strike; with the arms gone they hang
        // and drag.
        let shut = if self.snap > 0.0 {
            1.0 - self.snap / SNAP_TIME
        } else {
            0.0
        };
        for side in [-1.0f32, 1.0] {
            let droop = if arms_gone { 0.55 } else { 0.0 };
            let root = vec2(10.0, 9.0 * side);
            let out = vec2(18.0 - droop * 5.0, (12.0 + droop * 4.0) * side);
            b.rect(
                (root + out) * 0.5,
                vec2(11.0, 4.6),
                (0.35 + droop) * side,
                2.0,
                STEEL,
            );
            // The two fingers: open apart, closed together.
            let gape = (1.0 - shut) * 0.45 + droop * 0.5;
            for finger in [-1.0f32, 1.0] {
                b.rect(
                    out + vec2(4.0, 2.2 * finger * side),
                    vec2(9.0, 2.8),
                    (finger * gape + droop) * side,
                    1.4,
                    if arms_gone { JOINT } else { STEEL },
                );
            }
        }
    }

    /// **The Trooper**: upright. A boxy chassis wider than deep, a small
    /// square sensor head with one red slit, the gun built into the right
    /// forearm — the barrel *is* the arm — a stub left arm, and two legs
    /// that step.
    fn draw_trooper(&self, b: &mut Brush) {
        let step = self.step();
        let legs_gone = self.body.gone(DroidPart::Legs);
        let arms_gone = self.body.gone(DroidPart::Arms);
        let slump = if legs_gone { 0.88 } else { 1.0 };
        // Two legs, under the chassis: one forward as the other trails.
        if !legs_gone {
            for side in [-1.0f32, 1.0] {
                let swing = step * 6.0 * side;
                b.rect(
                    vec2(swing, 7.5 * side),
                    vec2(11.0, 6.0),
                    0.0,
                    2.0,
                    HULL_DARK,
                );
                b.rect(
                    vec2(swing + 5.5, 7.5 * side),
                    vec2(7.5, 4.6),
                    0.0,
                    1.6,
                    JOINT,
                );
            }
        } else {
            // Down on the stumps: the legs folded under, no step.
            for side in [-1.0f32, 1.0] {
                b.rect(vec2(-3.0, 7.0 * side), vec2(9.0, 5.0), 0.0, 2.0, JOINT);
            }
        }
        // The chassis: boxy and wider than deep.
        b.rect(Vec2::ZERO, vec2(22.0, 30.0) * slump, 0.0, 3.0, HULL_DARK);
        b.rect(vec2(0.5, 0.0), vec2(18.0, 25.5) * slump, 0.0, 2.5, HULL);
        // A plate across the chest, and the vent slots under it.
        b.rect(vec2(5.0, 0.0), vec2(6.0, 20.0), 0.0, 1.5, PLATE);
        for i in 0..3 {
            b.rect(
                vec2(-4.0, (i as f32 - 1.0) * 6.0),
                vec2(7.0, 2.0),
                0.0,
                0.8,
                JOINT,
            );
        }
        // The head: a small square with one red slit across it, set
        // forward on the chassis and swaying a little while it stands.
        let turn = self.sway() * 0.06;
        let head = vec2(9.0, 0.0);
        b.rect(head, vec2(11.0, 11.0), turn, 1.5, HULL_DARK);
        b.rect(head, vec2(8.5, 8.5), turn, 1.0, PLATE);
        b.rect(head + vec2(3.2, 0.0), vec2(1.8, 7.0), turn, 0.6, self.eye());
        // The right arm *is* the gun: a forearm that ends in a muzzle.
        // With the arms gone it hangs, and the barrel points at the deck.
        let droop = if arms_gone { 1.0 } else { 0.0 };
        let gun_side = 1.0f32;
        let shoulder = vec2(3.0, 11.0 * gun_side);
        let out = shoulder + vec2(13.0 - droop * 9.0, (2.0 + droop * 7.0) * gun_side);
        b.rect(
            (shoulder + out) * 0.5,
            vec2(16.0 - droop * 5.0, 6.4),
            (droop * 0.7) * gun_side,
            2.0,
            if arms_gone { JOINT } else { STEEL },
        );
        b.rect(out + vec2(2.0, 0.0), vec2(5.0, 5.0), 0.0, 1.2, HULL_DARK);
        if !arms_gone {
            // The muzzle, and the flash the instant it fires.
            b.ellipse(out + vec2(4.0, 0.0), vec2(3.4, 3.4), 0.0, JOINT);
            if self.flash > 0.0 {
                let t = self.flash / FLASH_TIME;
                b.ellipse(
                    out + vec2(7.0, 0.0),
                    vec2(13.0 * t, 9.0 * t),
                    0.0,
                    SENSOR.alpha(0.55 * t),
                );
                b.ellipse(
                    out + vec2(6.0, 0.0),
                    vec2(6.5 * t, 5.0 * t),
                    0.0,
                    SPARK.alpha(0.9 * t),
                );
            }
        }
        // The stub left arm: short, and nothing on the end of it.
        let stub = vec2(2.0, -11.0);
        b.rect(
            stub + vec2(4.0 - droop * 2.0, -2.0),
            vec2(9.0, 5.4),
            -0.3 - droop * 0.6,
            2.0,
            if arms_gone { JOINT } else { HULL_DARK },
        );
    }

    /// **The Warden**: the heaviest. A broad chassis with shoulder plates,
    /// the head recessed between them behind a sensor band, and the
    /// Unmaker as a long twin-rail lance along one side with a ring at
    /// the muzzle that brightens before each shot.
    fn draw_warden(&self, b: &mut Brush) {
        let step = self.step();
        let legs_gone = self.body.gone(DroidPart::Legs);
        let arms_gone = self.body.gone(DroidPart::Arms);
        let slump = if legs_gone { 0.9 } else { 1.0 };
        // Two heavy legs.
        if !legs_gone {
            for side in [-1.0f32, 1.0] {
                let swing = step * 5.0 * side;
                b.rect(
                    vec2(swing - 2.0, 12.0 * side),
                    vec2(15.0, 9.0),
                    0.0,
                    3.0,
                    HULL_DARK,
                );
                b.rect(
                    vec2(swing + 6.0, 12.5 * side),
                    vec2(11.0, 6.5),
                    0.0,
                    2.0,
                    JOINT,
                );
            }
        } else {
            for side in [-1.0f32, 1.0] {
                b.rect(vec2(-6.0, 11.0 * side), vec2(12.0, 7.0), 0.0, 2.5, JOINT);
            }
        }
        // The chassis: broad and deep, with a heavy skirt aft.
        b.rect(
            vec2(-2.0, 0.0),
            vec2(34.0, 44.0) * slump,
            0.0,
            4.0,
            HULL_DARK,
        );
        b.rect(vec2(-1.0, 0.0), vec2(29.0, 38.0) * slump, 0.0, 3.0, HULL);
        b.rect(vec2(-13.0, 0.0), vec2(9.0, 32.0), 0.0, 3.0, HULL_DARK);
        // The shoulder plates, either side and standing proud of the hull.
        for side in [-1.0f32, 1.0] {
            b.rect(
                vec2(4.0, 21.0 * side),
                vec2(22.0, 11.0),
                -0.12 * side,
                3.0,
                PLATE,
            );
            b.rect(
                vec2(4.0, 21.0 * side),
                vec2(18.0, 7.0),
                -0.12 * side,
                2.0,
                HULL_DARK,
            );
        }
        // The head, recessed between them behind a sensor band: a dark
        // well with a wide lit strip across it.
        let turn = self.sway() * 0.04;
        let head = vec2(11.0, 0.0);
        b.rect(head, vec2(13.0, 22.0), turn, 2.0, JOINT);
        b.rect(
            head + vec2(1.5, 0.0),
            vec2(4.0, 17.0),
            turn,
            1.0,
            self.eye(),
        );
        // The Unmaker: a twin-rail lance along the right side, running
        // well forward of the machine, with a ring at the muzzle.
        let droop = if arms_gone { 1.0 } else { 0.0 };
        let side = 1.0f32;
        let along = -0.06 * side + droop * 0.55 * side;
        let root = vec2(-4.0, 17.0 * side);
        let muzzle = vec2(30.0 - droop * 10.0, (14.0 + droop * 8.0) * side);
        let mid = (root + muzzle) * 0.5;
        let rail = if arms_gone { JOINT } else { STEEL };
        b.rect(mid, vec2(42.0 - droop * 9.0, 7.0), along, 2.0, HULL_DARK);
        for rail_side in [-1.0f32, 1.0] {
            b.rect(
                mid + vec2(0.0, 2.6 * rail_side),
                vec2(40.0 - droop * 9.0, 2.2),
                along,
                1.0,
                rail,
            );
        }
        if !arms_gone {
            // The ring at the muzzle, brightening as the charge builds,
            // and a flash the instant it lets go.
            let charge = clamp(self.charge / CHARGE_TIME, 0.0, 1.0);
            b.ellipse(muzzle, vec2(11.0, 11.0), 0.0, JOINT);
            b.ellipse(
                muzzle,
                vec2(9.0, 9.0),
                0.0,
                SENSOR_OUT.mix(SENSOR, 0.25 + 0.75 * charge),
            );
            b.ellipse(muzzle, vec2(5.2, 5.2), 0.0, HULL_DARK);
            if self.flash > 0.0 {
                let t = self.flash / FLASH_TIME;
                b.ellipse(
                    muzzle + vec2(6.0 * side, 0.0),
                    vec2(18.0 * t, 13.0 * t),
                    0.0,
                    SENSOR.alpha(0.6 * t),
                );
                b.ellipse(
                    muzzle + vec2(5.0 * side, 0.0),
                    vec2(8.0 * t, 6.5 * t),
                    0.0,
                    SPARK.alpha(0.95 * t),
                );
            }
        }
        // The off-side arm: a short brace, nothing on the end of it.
        b.rect(
            vec2(6.0, -18.0),
            vec2(14.0, 6.5),
            0.18 + droop * 0.5,
            2.0,
            if arms_gone { JOINT } else { HULL_DARK },
        );
    }

    /// **The Guardian** (feature 100): the largest, a walker. Heavy legs,
    /// a broad chassis, and the lens in the middle of it.
    fn draw_guardian(&self, b: &mut Brush) {
        let step = self.step();
        let legs_gone = self.body.gone(DroidPart::Legs);
        if !legs_gone {
            for side in [-1.0f32, 1.0] {
                let swing = step * 6.0 * side;
                b.rect(
                    vec2(swing - 4.0, 16.0 * side),
                    vec2(18.0, 11.0),
                    0.0,
                    3.0,
                    HULL_DARK,
                );
            }
        }
        b.rect(Vec2::ZERO, vec2(36.0, 50.0), 0.0, 5.0, HULL_DARK);
        b.rect(vec2(1.0, 0.0), vec2(30.0, 43.0), 0.0, 4.0, HULL);
        b.ellipse(vec2(LENS_AHEAD, 0.0), vec2(13.0, 13.0), 0.0, JOINT);
        b.ellipse(vec2(LENS_AHEAD, 0.0), vec2(9.0, 9.0), 0.0, self.eye());
    }

    /// A Guardian's wreck: the chassis down, the lens dark.
    fn draw_guardian_wreck(&self, b: &mut Brush) {
        self.draw_wreck_ground(b, 36.0);
        // Both legs off and thrown clear.
        for i in 0..2u32 {
            let a = self.scatter(i + 90) * TAU;
            let at = vec2(-14.0, 0.0) + Vec2::from_angle(a) * (18.0 + 8.0 * self.scatter(i + 92));
            b.rect(at, vec2(18.0, 11.0), a, 3.0, HULL_DARK);
            b.rect(
                at + Vec2::from_angle(a) * 10.0,
                vec2(12.0, 7.0),
                a + 0.3,
                2.0,
                JOINT,
            );
        }
        // The largest machine throws the most: a second ring of plate
        // further out than the common mess reaches.
        for i in 0..10u32 {
            let k = i * 3 + 100;
            let a = self.scatter(k) * TAU;
            let away = 44.0 + 16.0 * self.scatter(k + 1);
            let size = 4.0 + 5.0 * self.scatter(k + 2);
            b.rect(
                Vec2::from_angle(a) * away,
                vec2(size * 2.2, size),
                a * 1.7,
                size * 0.3,
                if i % 2 == 0 { TORN } else { JOINT },
            );
        }
        // The shield's emitters, torn off the front and lying dark.
        for i in 0..3u32 {
            let a = self.scatter(i + 94) * 1.2 - 0.6;
            let at = Vec2::from_angle(a) * (30.0 + 6.0 * self.scatter(i + 97));
            b.rect(at, vec2(8.0, 4.0), a + 1.2, 1.5, TORN);
        }
        b.rect(vec2(-2.0, 0.0), vec2(36.0, 48.0), 0.12, 5.0, HULL_DARK);
        b.rect(vec2(-1.0, 0.0), vec2(29.0, 40.0), 0.12, 4.0, HULL);
        self.draw_rift(b, vec2(-2.0, 2.0), vec2(14.0, 30.0), 0.1);
        b.ellipse(vec2(LENS_AHEAD, 0.0), vec2(13.0, 13.0), 0.0, JOINT);
        b.ellipse(vec2(LENS_AHEAD, 0.0), vec2(9.0, 9.0), 0.0, self.eye());
    }

    /// A point on the shield's arc, `a` radians off the facing and `r` out
    /// from the middle: drawing only.
    fn on_arc(&self, a: f32, r: f32) -> Vec2 {
        self.pos + self.facing.rotate(a) * r
    }

    /// The arc the shield stops things in, faint on the deck under the
    /// machine (feature 100, section 7): the ±60° wedge's two edges and
    /// its rim, so a crew member can see where to get round to.
    fn draw_shield_on_deck(&self, list: &mut DrawList) {
        const STEPS: usize = 10;
        let half = SHIELD_HALF_ARC;
        let r = balance::GUARDIAN_SHIELD_RADIUS * SHIELD_DECK_REACH;
        let mut prev = self.on_arc(-half, r);
        for i in 1..=STEPS {
            let next = self.on_arc(-half + 2.0 * half * i as f32 / STEPS as f32, r);
            list.line(prev, next, 1.5, SHIELD_DECK);
            prev = next;
        }
        for side in [-1.0f32, 1.0] {
            list.line(
                self.on_arc(side * half, balance::GUARDIAN_SHIELD_RADIUS * 0.8),
                self.on_arc(side * half, r),
                1.2,
                SHIELD_DECK,
            );
        }
    }

    /// The shield's plate: short overlapping segments round the front at
    /// [`balance::GUARDIAN_SHIELD_RADIUS`].
    fn draw_shield_plate(&self, list: &mut DrawList) {
        const STEPS: usize = 12;
        let half = SHIELD_HALF_ARC;
        let r = balance::GUARDIAN_SHIELD_RADIUS;
        let mut prev = self.on_arc(-half, r);
        for i in 1..=STEPS {
            let next = self.on_arc(-half + 2.0 * half * i as f32 / STEPS as f32, r);
            list.line(prev, next, 5.0, SHIELD.alpha(0.35));
            list.line(prev, next, 1.6, SHIELD_RIM);
            prev = next;
        }
    }

    // --- the wrecks -----------------------------------------------------

    /// A stable number in `[0, 1)` for the `i`th thing scattered round
    /// this wreck. Worked out afresh from where the machine lies rather
    /// than rolled: a picture may not touch the sparks' own stream, and a
    /// wreck has to lie the same way on every frame — and the same way
    /// again after a save and a load.
    fn scatter(&self, i: u32) -> f32 {
        let mut h = 0x9E37_79B9_7F4A_7C15u64;
        for n in [
            self.pos.x.to_bits() as u64,
            self.pos.y.to_bits() as u64,
            self.wave as u64,
            self.kind.code() as u64,
            i as u64,
        ] {
            h ^= n.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
            h = h.rotate_left(29).wrapping_mul(0xC4CE_B9FE_1A85_EC53);
        }
        (h >> 40) as f32 / (1u64 << 24) as f32
    }

    /// How brightly a wreck is still burning inside: full the instant it
    /// went and out by [`EMBER_LIFE`], so a fresh kill reads differently
    /// from one that has been lying there since the last wave.
    fn ember(&self) -> f32 {
        1.0 - clamp(self.wreck_age / EMBER_LIFE, 0.0, 1.0)
    }

    /// What lies on the deck under every wreck, drawn before the hulk so
    /// the hulk lies on top of its own mess: the scorch it burnt into the
    /// plating, the coolant run out of it, and the plate torn off it and
    /// flung clear. `spread` is how far its wreckage reaches, in the
    /// machine's own frame.
    fn draw_wreck_ground(&self, b: &mut Brush, spread: f32) {
        // The scorch: three smudges laid off-centre, so what is burnt
        // into the deck is a mark and not a disc.
        for i in 0..3u32 {
            let a = self.scatter(i) * TAU;
            let r = 0.3 + 0.7 * self.scatter(i + 8);
            b.ellipse(
                Vec2::from_angle(a) * (spread * 0.4 * r),
                vec2(spread * (1.7 - 0.5 * r), spread * (1.4 - 0.4 * r)),
                a,
                SCORCH,
            );
        }
        // The coolant, spreading out from under the hull.
        for i in 0..2u32 {
            let a = self.scatter(i + 16) * TAU;
            b.ellipse(
                Vec2::from_angle(a) * (spread * 0.3),
                vec2(spread, spread * 0.66),
                a,
                COOLANT,
            );
        }
        // The shards, lying flat where they came down.
        for i in 0..SHARDS {
            let k = i * 3 + 24;
            let a = self.scatter(k) * TAU;
            let away = spread * (0.8 + 0.8 * self.scatter(k + 1));
            let size = spread * (0.09 + 0.13 * self.scatter(k + 2));
            b.rect(
                Vec2::from_angle(a) * away,
                vec2(size * 2.4, size),
                a * 1.9,
                size * 0.35,
                if i % 3 == 0 { TORN } else { JOINT },
            );
        }
    }

    /// The rift the killing hit tore in a hull: a dark split with
    /// whatever is left of the fire still in it.
    fn draw_rift(&self, b: &mut Brush, at: Vec2, size: Vec2, rot: f32) {
        b.ellipse(at, size, rot, JOINT);
        let e = self.ember();
        if e > 0.0 {
            b.ellipse(at, size * (0.55 * e), rot, EMBER.alpha(0.75 * e));
            b.ellipse(at, size * (0.3 * e), rot, SPARK.alpha(0.6 * e));
        }
    }

    /// **A Husk's wreck**: the shell split down its length and the two
    /// halves tipped apart, the legs snapped off and lying about it, one
    /// claw still on the front hanging open and the other thrown clear.
    fn draw_husk_wreck(&self, b: &mut Brush) {
        self.draw_wreck_ground(b, 26.0);
        // The legs, off the body: three thrown clear and a stump left on
        // the shell.
        for i in 0..3u32 {
            let a = self.scatter(i + 60) * TAU;
            let at = Vec2::from_angle(a) * (20.0 + 12.0 * self.scatter(i + 63));
            b.rect(at, vec2(13.0, 4.2), a * 1.3, 2.0, STEEL);
            b.rect(
                at + Vec2::from_angle(a * 1.3) * 6.0,
                vec2(6.5, 3.4),
                a * 1.3 + 0.8,
                1.6,
                JOINT,
            );
        }
        b.rect(vec2(-7.0, 11.0), vec2(11.0, 4.0), 0.5, 2.0, JOINT);
        // The shell in two halves, each the hull's own shape cut down the
        // middle and tipped the way a cracked casing sits.
        for side in [-1.0f32, 1.0] {
            let at = vec2(0.5 * side, 7.5 * side);
            b.ellipse(at, vec2(25.0, 17.0), 0.16 * side, HULL_DARK);
            b.ellipse(at + vec2(0.5, 0.0), vec2(20.0, 12.5), 0.16 * side, HULL);
            b.rect(
                at + vec2(-1.0, 0.0),
                vec2(15.0, 4.0),
                0.16 * side,
                1.5,
                PLATE,
            );
        }
        // The split itself, burning while it is fresh.
        self.draw_rift(b, vec2(-1.0, 0.0), vec2(22.0, 7.0), 0.0);
        // The sensors, out: dark pits where the two lights were.
        for side in [-1.0f32, 1.0] {
            b.ellipse(vec2(9.0, 8.0 * side), vec2(4.2, 3.2), 0.0, JOINT);
            b.ellipse(vec2(9.0, 8.0 * side), vec2(2.4, 1.8), 0.0, self.eye());
        }
        // The near claw, still on the front and slack, its fingers open
        // and pointing nowhere.
        let arm = 0.75f32;
        let out = vec2(18.0, 19.0);
        b.rect(vec2(13.5, 14.5), vec2(11.0, 4.6), arm, 2.0, JOINT);
        for finger in [-1.0f32, 1.0] {
            let angle = arm + finger * 0.6;
            b.rect(
                out + Vec2::from_angle(angle) * 4.5,
                vec2(9.0, 2.8),
                angle,
                1.4,
                JOINT,
            );
        }
        // And the far one, thrown clear, lying open on the deck.
        let a = self.scatter(66) * TAU;
        let at = vec2(4.0, -24.0) + Vec2::from_angle(a) * 8.0;
        b.rect(at, vec2(11.0, 4.6), a, 2.0, STEEL);
        for finger in [-1.0f32, 1.0] {
            b.rect(
                at + Vec2::from_angle(a) * 7.0,
                vec2(9.0, 2.8),
                a + finger * 0.55,
                1.4,
                JOINT,
            );
        }
    }

    /// **A Trooper's wreck**: over on its back with the chest blown open,
    /// the head knocked off and lying ahead of it, the gun arm snapped at
    /// the elbow and thrown beside it, and the legs sheared off at the
    /// hips.
    fn draw_trooper_wreck(&self, b: &mut Brush) {
        self.draw_wreck_ground(b, 24.0);
        // A leg lying behind it, and the stump the other one left.
        let a = self.scatter(70) * TAU;
        let leg = vec2(-24.0, 0.0) + Vec2::from_angle(a) * 12.0;
        b.rect(leg, vec2(12.0, 6.0), a, 2.0, HULL_DARK);
        b.rect(
            leg + Vec2::from_angle(a) * 8.0,
            vec2(8.0, 4.6),
            a + 0.4,
            1.6,
            JOINT,
        );
        b.rect(vec2(-9.0, -8.0), vec2(9.0, 5.0), -0.4, 2.0, JOINT);
        // The chassis, down and knocked out of square.
        b.rect(vec2(-1.0, 0.0), vec2(23.0, 29.0), 0.09, 3.0, HULL_DARK);
        b.rect(vec2(-0.5, 0.0), vec2(18.0, 24.0), 0.09, 2.5, HULL);
        // The chest plate, torn half off and hanging over the side.
        b.rect(vec2(3.0, 11.0), vec2(6.0, 15.0), 0.5, 1.5, TORN);
        // The vents under it, split open.
        for i in 0..3 {
            b.rect(
                vec2(-5.0, (i as f32 - 1.0) * 6.0),
                vec2(8.0, 2.4),
                0.09,
                0.8,
                JOINT,
            );
        }
        // What went through it.
        self.draw_rift(b, vec2(1.0, -2.0), vec2(15.0, 12.0), 0.35);
        // The head, off the shoulders and rolled onto a corner, its slit
        // dark; and the empty socket it came off.
        let ha = 0.5 + self.scatter(74) * 1.6;
        let head = vec2(20.0, -6.0) + Vec2::from_angle(self.scatter(75) * TAU) * 5.0;
        b.ellipse(vec2(9.0, 1.0), vec2(7.0, 7.0), 0.0, JOINT);
        b.rect(head, vec2(11.0, 11.0), ha, 1.5, HULL_DARK);
        b.rect(head, vec2(8.0, 8.0), ha, 1.0, PLATE);
        b.rect(
            head + Vec2::from_angle(ha) * 3.0,
            vec2(1.8, 7.0),
            ha,
            0.6,
            self.eye(),
        );
        // The gun arm: the stump still on the shoulder, the barrel lying
        // beside the body with the muzzle dark.
        b.rect(vec2(5.0, 12.0), vec2(8.0, 6.0), 0.3, 2.0, JOINT);
        let ga = -0.5 - self.scatter(76) * 0.8;
        let gun = vec2(9.0, 24.0);
        b.rect(gun, vec2(17.0, 6.0), ga, 2.0, STEEL);
        b.rect(
            gun + Vec2::from_angle(ga) * 9.0,
            vec2(5.0, 5.0),
            ga,
            1.2,
            HULL_DARK,
        );
        b.ellipse(
            gun + Vec2::from_angle(ga) * 11.0,
            vec2(3.4, 3.4),
            0.0,
            JOINT,
        );
        // The stub arm, still on the other shoulder and bent back.
        b.rect(vec2(4.0, -12.0), vec2(9.0, 5.4), -0.9, 2.0, JOINT);
    }

    /// **A Warden's wreck**: the heaviest of them, and it breaks like it.
    /// The chassis is split across and the skirt has come away aft, one
    /// shoulder plate is buckled where it sits and the other is torn
    /// clean off, and the Unmaker is broken in two with the muzzle ring
    /// out at the far end of the wreckage.
    fn draw_warden_wreck(&self, b: &mut Brush) {
        self.draw_wreck_ground(b, 34.0);
        // Both legs off, and heavy enough to have gone a way.
        for i in 0..2u32 {
            let a = self.scatter(i + 80) * TAU;
            let at = vec2(-16.0, 0.0) + Vec2::from_angle(a) * (14.0 + 8.0 * self.scatter(i + 82));
            b.rect(at, vec2(16.0, 9.0), a, 3.0, HULL_DARK);
            b.rect(
                at + Vec2::from_angle(a) * 10.0,
                vec2(11.0, 6.5),
                a + 0.3,
                2.0,
                JOINT,
            );
        }
        // The skirt, come away aft of the hull it was bolted to.
        b.rect(vec2(-24.0, 4.0), vec2(9.0, 30.0), 0.22, 3.0, HULL_DARK);
        // The chassis in two: the fore part square on the deck and the
        // aft part pulled back and tipped, with the break between them.
        b.rect(vec2(6.0, 0.0), vec2(20.0, 42.0), 0.0, 4.0, HULL_DARK);
        b.rect(vec2(6.0, 0.0), vec2(15.0, 36.0), 0.0, 3.0, HULL);
        b.rect(vec2(-11.0, 2.0), vec2(16.0, 40.0), 0.13, 4.0, HULL_DARK);
        b.rect(vec2(-11.0, 2.0), vec2(11.0, 33.0), 0.13, 3.0, HULL);
        self.draw_rift(b, vec2(-2.0, 1.0), vec2(11.0, 34.0), 0.06);
        // The shoulder plates: the near one buckled over the hull, the
        // far one off it altogether.
        b.rect(vec2(3.0, 21.0), vec2(22.0, 11.0), -0.34, 3.0, PLATE);
        b.rect(vec2(3.0, 21.0), vec2(18.0, 7.0), -0.34, 2.0, HULL_DARK);
        let pa = self.scatter(86) * TAU;
        let plate = vec2(8.0, -30.0) + Vec2::from_angle(pa) * 7.0;
        b.rect(plate, vec2(22.0, 11.0), pa, 3.0, TORN);
        b.rect(plate, vec2(17.0, 6.0), pa, 2.0, HULL_DARK);
        // The head, dark in its well.
        let head = vec2(15.0, 0.0);
        b.rect(head, vec2(13.0, 22.0), 0.0, 2.0, JOINT);
        b.rect(head + vec2(1.5, 0.0), vec2(4.0, 17.0), 0.0, 1.0, self.eye());
        // The Unmaker, broken: the root still on the hull with its rail
        // bent, and the muzzle half lying well forward of it.
        b.rect(vec2(5.0, 18.0), vec2(16.0, 7.0), 0.2, 2.0, HULL_DARK);
        b.rect(vec2(5.0, 18.0), vec2(14.0, 2.2), 0.28, 1.0, STEEL);
        let ma = 0.7 + self.scatter(88) * 0.9;
        let muzzle = vec2(30.0, 26.0);
        b.rect(muzzle, vec2(30.0, 7.0), ma, 2.0, HULL_DARK);
        for rail_side in [-1.0f32, 1.0] {
            b.rect(
                muzzle + Vec2::from_angle(ma + TAU / 4.0) * (2.6 * rail_side),
                vec2(28.0, 2.2),
                ma,
                1.0,
                STEEL,
            );
        }
        let ring = muzzle + Vec2::from_angle(ma) * 15.0;
        b.ellipse(ring, vec2(11.0, 11.0), 0.0, JOINT);
        b.ellipse(ring, vec2(9.0, 9.0), 0.0, SENSOR_OUT);
        b.ellipse(ring, vec2(5.2, 5.2), 0.0, HULL_DARK);
    }
}

/// Facing along `+x`: what a machine saved before feature 100 read back as.
#[cfg(feature = "serde")]
fn unit_x() -> Vec2 {
    vec2(1.0, 0.0)
}

impl core::fmt::Debug for Droid {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Droid({:?} t{} at {:.0},{:.0}{})",
            self.kind,
            self.tier.code(),
            self.pos.x,
            self.pos.y,
            if self.destroyed { ", wreck" } else { "" }
        )
    }
}

/// How many of each kind a wave of `n` is made of: Wardens `n / 6`,
/// Husks `n / 3`, Troopers the rest. Integers only, and the Troopers
/// take whatever the two divisions leave — so a wave is always exactly
/// `n` machines. Answered as (Husks, Troopers, Wardens).
pub fn mix_of(n: u32) -> (u32, u32, u32) {
    let wardens = n / 6;
    let husks = n / 3;
    (husks, n.saturating_sub(husks + wardens), wardens)
}

/// How many Guardians a wave of `n` at `tier` has (feature 100): **none
/// below tier three**, and at tier three `n / 8` — at least one once the
/// wave is four or more. They are taken out of the Troopers' share
/// ([`mix_of`]), which is always at least half the wave and so always has
/// them to give.
pub fn guardians_of(n: u32, tier: Tier) -> u32 {
    if tier != Tier::Three || n < 4 {
        return 0;
    }
    (n / 8).max(1)
}

/// The kinds of a wave of `n` at `tier`, in the order they are made: the
/// Wardens first, then the Guardians, then the Husks, then the Troopers,
/// so the index a Trooper's arm is dealt by runs over the Troopers alone.
pub fn wave_kinds(n: u32, tier: Tier) -> Vec<DroidKind> {
    let (husks, troopers, wardens) = mix_of(n);
    let guardians = guardians_of(n, tier).min(troopers);
    let mut out = Vec::with_capacity(n as usize);
    out.extend(std::iter::repeat_n(DroidKind::Warden, wardens as usize));
    out.extend(std::iter::repeat_n(DroidKind::Guardian, guardians as usize));
    out.extend(std::iter::repeat_n(DroidKind::Husk, husks as usize));
    out.extend(std::iter::repeat_n(
        DroidKind::Trooper,
        (troopers - guardians) as usize,
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_part_odds_add_to_one_and_every_roll_lands_somewhere() {
        let sum: f32 = balance::DROID_HIT_ODDS.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6, "odds add to {sum}");
        // Every roll in [0, 1) lands on a part, and the shares come out
        // near the table over a sweep.
        let mut seen = [0u32; 4];
        let n = 100_000;
        for i in 0..n {
            let roll = i as f32 / n as f32;
            seen[DroidPart::hit_by(roll).code() as usize] += 1;
        }
        for (i, &count) in seen.iter().enumerate() {
            let share = count as f32 / n as f32;
            assert!(
                (share - balance::DROID_HIT_ODDS[i]).abs() < 1e-3,
                "part {i}: {share} against {}",
                balance::DROID_HIT_ODDS[i]
            );
        }
    }

    #[test]
    fn head_or_chassis_at_nothing_destroys_and_there_is_no_dying() {
        for part in [DroidPart::Head, DroidPart::Chassis] {
            let mut d = Droid::new(DroidKind::Trooper, Tier::One, 0, 0, Vec2::ZERO, 0.0, 7);
            assert!(!d.destroyed);
            d.strike(part, 1e6);
            assert!(d.destroyed, "{part:?} at nothing destroys");
            // A wreck takes nothing more, and is still a wreck.
            assert_eq!(d.strike(DroidPart::Chassis, 1e6), None);
            assert!(d.destroyed);
        }
    }

    #[test]
    fn a_wreck_is_its_own_picture_and_lies_the_same_way_every_frame() {
        for kind in DroidKind::ALL {
            let at = vec2(311.0, 207.0);
            let standing = Droid::new(kind, Tier::One, 0, 1, at, 0.4, 9);
            let mut wreck = Droid::new(kind, Tier::One, 0, 1, at, 0.4, 9);
            wreck.destroy();

            let mut up = DrawList::new();
            standing.draw(&mut up);
            let mut down = DrawList::new();
            wreck.draw(&mut down);
            assert_ne!(
                up.data(),
                down.data(),
                "{kind:?}: a wreck is not the standing machine"
            );
            // The mess under it — the scorch, the coolant and the shards
            // — is shapes of its own, so a wreck is more of a picture
            // than the machine on its feet was and not a smaller one.
            assert!(
                down.len() > up.len(),
                "{kind:?}: {} floats down against {} up",
                down.len(),
                up.len()
            );

            // Drawn again it lies exactly where it lay: the scatter is a
            // hash of where the machine fell, never a roll, so nothing
            // about the picture wanders from frame to frame.
            let mut again = DrawList::new();
            wreck.draw(&mut again);
            assert_eq!(down.data(), again.data(), "{kind:?}: the wreck moved");

            // And it burns while it is fresh and is cold by the time the
            // ember is out.
            assert!((wreck.ember() - 1.0).abs() < 1e-6);
            wreck.wreck_age = EMBER_LIFE;
            assert_eq!(wreck.ember(), 0.0);
        }
    }

    #[test]
    fn a_hit_on_a_limb_already_gone_lands_on_the_chassis() {
        for limb in [DroidPart::Arms, DroidPart::Legs] {
            let mut d = Droid::new(DroidKind::Warden, Tier::One, 0, 0, Vec2::ZERO, 0.0, 1);
            d.strike(limb, d.body.max(limb));
            assert!(d.body.gone(limb));
            let before = d.body.health(DroidPart::Chassis);
            let struck = d.strike(limb, 10.0);
            assert_eq!(struck, Some(DroidPart::Chassis));
            assert_eq!(d.body.health(DroidPart::Chassis), before - 10.0);
        }
    }

    #[test]
    fn arms_gone_halve_a_gun_s_odds_and_a_claw_s_damage() {
        let mut trooper = Droid::new(DroidKind::Trooper, Tier::One, 0, 0, Vec2::ZERO, 0.0, 2);
        let full = trooper.stats();
        trooper.strike(DroidPart::Arms, trooper.body.max(DroidPart::Arms));
        let hurt = trooper.stats();
        assert!((hurt.accuracy - full.accuracy * 0.5).abs() < 1e-6);
        assert_eq!(hurt.damage, full.damage, "a gun's damage does not fall");

        let mut husk = Droid::new(DroidKind::Husk, Tier::One, 0, 0, Vec2::ZERO, 0.0, 3);
        let full = husk.stats();
        husk.strike(DroidPart::Arms, husk.body.max(DroidPart::Arms));
        let hurt = husk.stats();
        assert!((hurt.damage - full.damage * 0.5).abs() < 1e-6);
        assert_eq!(hurt.accuracy, full.accuracy, "a claw never misses");

        // The Unmaker counts as a gun: its odds fall, its strip does not.
        let mut warden = Droid::new(DroidKind::Warden, Tier::One, 0, 0, Vec2::ZERO, 0.0, 4);
        let full = warden.stats();
        warden.strike(DroidPart::Arms, warden.body.max(DroidPart::Arms));
        let hurt = warden.stats();
        assert!((hurt.accuracy - full.accuracy * 0.5).abs() < 1e-6);
        assert_eq!(hurt.strips, full.strips);
    }

    #[test]
    fn legs_gone_stop_it_moving_and_leave_it_firing() {
        let mut d = Droid::new(DroidKind::Trooper, Tier::One, 0, 0, Vec2::ZERO, 0.0, 5);
        d.follow_path(vec![vec2(500.0, 0.0)]);
        d.walk(0.1);
        assert!(d.pos.x > 0.0, "it walked");
        d.strike(DroidPart::Legs, d.body.max(DroidPart::Legs));
        let held = d.pos;
        d.follow_path(vec![vec2(500.0, 0.0)]);
        d.walk(1.0);
        assert_eq!(d.pos, held, "legs gone, it does not move");
        assert!(!d.destroyed, "and it is not destroyed");
        // It still has its arm and its odds.
        assert_eq!(d.stats().accuracy, d.weapon.stats().accuracy);
    }

    #[test]
    fn a_tier_multiplies_every_part() {
        for kind in DroidKind::ALL {
            let one = DroidBody::new(kind, Tier::One);
            let two = DroidBody::new(kind, Tier::Two);
            let three = DroidBody::new(kind, Tier::Three);
            for part in DroidPart::ALL {
                let base = one.max(part);
                assert!((two.max(part) - base * balance::ARMOUR_TIER_STEP).abs() < 1e-3);
                assert!(
                    (three.max(part)
                        - base * balance::ARMOUR_TIER_STEP * balance::ARMOUR_TIER_STEP)
                        .abs()
                        < 1e-3
                );
            }
        }
    }

    #[test]
    fn the_body_table_is_the_one_the_spec_wrote_down() {
        assert_eq!(DroidKind::Husk.body(), [8.0, 40.0, 12.0, 15.0]);
        assert_eq!(DroidKind::Trooper.body(), [10.0, 60.0, 15.0, 20.0]);
        assert_eq!(DroidKind::Warden.body(), [20.0, 120.0, 25.0, 30.0]);
        assert_eq!(DroidKind::Guardian.body(), [16.0, 110.0, 25.0, 30.0]);
        assert_eq!(DroidKind::Guardian.pace(), 0.7);
        assert_eq!(DroidKind::Guardian.arm(0), WeaponKind::Sweeper);
        assert!(
            !DroidKind::Guardian.takes_cover(),
            "its shield is its cover"
        );
    }

    /// Feature 100: Guardians only at tier three, `n / 8` of a wave and
    /// at least one from four, out of the Troopers' share.
    #[test]
    fn guardians_come_only_at_tier_three_an_eighth_of_a_wave_and_one_from_four() {
        for n in 0..40u32 {
            for tier in [Tier::One, Tier::Two] {
                assert_eq!(guardians_of(n, tier), 0);
                assert!(!wave_kinds(n, tier).contains(&DroidKind::Guardian));
                assert_eq!(wave_kinds(n, tier).len(), n as usize);
            }
            let want = if n < 4 { 0 } else { (n / 8).max(1) };
            assert_eq!(guardians_of(n, Tier::Three), want, "a wave of {n}");
            let kinds = wave_kinds(n, Tier::Three);
            let count = |k: DroidKind| kinds.iter().filter(|&&x| x == k).count() as u32;
            let (husks, troopers, wardens) = mix_of(n);
            assert_eq!(count(DroidKind::Guardian), want);
            assert_eq!(count(DroidKind::Husk), husks, "the Husks untouched");
            assert_eq!(count(DroidKind::Warden), wardens, "the Wardens untouched");
            assert_eq!(
                count(DroidKind::Trooper),
                troopers - want,
                "the Troopers give"
            );
        }
        assert_eq!(guardians_of(16, Tier::Three), 2);
    }

    #[test]
    fn a_wave_s_mix_is_wardens_a_sixth_husks_a_third_and_troopers_the_rest() {
        for n in 0..40u32 {
            let (husks, troopers, wardens) = mix_of(n);
            assert_eq!(wardens, n / 6);
            assert_eq!(husks, n / 3);
            assert_eq!(husks + troopers + wardens, n, "a wave of {n} is {n}");
            assert_eq!(wave_kinds(n, Tier::One).len(), n as usize);
            assert_eq!(wave_kinds(n, Tier::Three).len(), n as usize);
        }
        // The shape the spec names: a wave of twelve is two Wardens,
        // four Husks and six Troopers.
        assert_eq!(mix_of(12), (4, 6, 2));
    }

    #[test]
    fn troopers_alternate_pistol_and_auto_rifle_by_their_place() {
        assert_eq!(DroidKind::trooper_arm(0), WeaponKind::LaserPistol);
        assert_eq!(DroidKind::trooper_arm(1), WeaponKind::AutoRifle);
        assert_eq!(DroidKind::trooper_arm(2), WeaponKind::LaserPistol);
        assert_eq!(DroidKind::Husk.arm(3), WeaponKind::Claw);
        assert_eq!(DroidKind::Warden.arm(3), WeaponKind::Unmaker);
    }

    #[test]
    fn no_droid_is_wider_than_thirty() {
        for kind in DroidKind::ALL {
            assert!(kind.half_width() <= 30.0, "{kind:?}");
        }
    }
}
