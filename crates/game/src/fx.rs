//! The fight's passing lights (feature 98): what flares for an instant
//! and fades — a muzzle's glow, a bolt's flash where it lands and the
//! scorch it leaves, the sniper's beam hanging in the air, a blade's cut,
//! the flash on the part a hit struck, a machine bursting apart.
//!
//! # A picture, and nothing the fight reads
//!
//! Everything here is **drawing only**. The fight spawns an effect where
//! something happens — `Combat::fire_as`'s shooter, `Combat::step`'s
//! bolt landing, `Game::strike` on a body, `Game::strike_droid` on a
//! machine — and nothing ever reads one back: no rule, no roll, no
//! checksum, no save. [`Fx`] is `serde(skip)` on the [`crate::combat::
//! Combat`] that holds it, so a load starts with none, and every peer of
//! a game with company spawns the same ones off the same steps and ages
//! them on its own clock.
//!
//! # Real seconds, and only for a host that asks
//!
//! The simulation's `dt` is the clock's: at 24x a step is twenty-four
//! times what the screen shows, and a flash aged by it would be gone
//! inside one frame. So an effect is aged by [`Fx::age`] with the
//! **host's** frame time — real seconds at every speed, frozen while the
//! game is paused — and a host that never calls it (a test, a probe, a
//! server) records **none**: a room nobody ages is drawn exactly as it
//! was before this module, with the fight's own sim-time spark and hit
//! flash, which is also what keeps two copies of a saved game drawing the
//! same picture in the round-trip tests.
//!
//! # Visual randomness is a hash
//!
//! Which way a spark flies or a plate tumbles is [`scatter`], a hash of
//! a spawn counter and the index — never the combat stream, which the
//! lockstep depends on, and never the room's.
//!
//! # Caps
//!
//! Every list is capped, the oldest dropped first: [`FLARE_CAP`] lights
//! in the air, [`SCORCH_CAP`] scorch marks on the deck, [`STRUCK_CAP`]
//! part flashes and [`BURST_CAP`] machines bursting. At 48x a fight
//! spawns more in a frame than it can show, and the newest are the ones
//! worth seeing.

use crate::combat::{FRIENDLY_BOLT, HOSTILE_BOLT, Tier, Weapon, WeaponKind};
use crate::draw::{Color, DrawList};
use crate::math::{TAU, Vec2, clamp, vec2};

// --- the common language ------------------------------------------------------

/// The white every laser's core is pulled towards: the side's colour
/// [`CORE_TINT`] of the way from it, so the core reads white-hot and the
/// halo round it says whose it is.
pub const CORE_WHITE: Color = Color::rgb(0.92, 0.97, 1.0);
/// How much of the side's colour is left in a core.
pub const CORE_TINT: f32 = 0.4;
/// How far past white a core is drawn, before its tier adds to it — the
/// part the bloom picks up (feature 97: a channel past one is emissive).
pub const CORE_HEAT: f32 = 2.4;
/// What each tier above the first adds to a shot's width, as a share of
/// it, and to its heat: a better gun is a little thicker and a little
/// brighter, never another colour.
pub const TIER_WIDTH: f32 = 0.15;
pub const TIER_HEAT: f32 = 0.35;

// --- the muzzle ----------------------------------------------------------------

/// How long a muzzle glows, in real seconds, and how big the glow is,
/// per gun: the pistol's a blink, the shotgun's a wide cough with a cone
/// of rays, the auto rifle's the smallest (it fires eight), the sniper's
/// the biggest with a spike along the barrel.
pub const MUZZLE_PISTOL: (f32, f32) = (0.08, 5.0);
pub const MUZZLE_SHOTGUN: (f32, f32) = (0.11, 8.0);
pub const MUZZLE_AUTO: (f32, f32) = (0.06, 4.0);
pub const MUZZLE_SNIPER: (f32, f32) = (0.14, 9.0);
/// How far the shotgun's rays and the sniper's spike reach, in room units.
pub const MUZZLE_RAY: f32 = 12.0;
pub const MUZZLE_SPIKE: f32 = 16.0;

// --- where a bolt lands ------------------------------------------------------

/// The flash where a bolt stops, in real seconds, and how many little
/// rays it throws back the way it came.
pub const IMPACT_LIFE: f32 = 0.15;
pub const IMPACT_RAYS: u32 = 3;
/// The scorch a bolt leaves where a wall or a lamp stopped it: how long
/// it lies, in real seconds, how long of that it is still hot (a warm
/// tint, not a glow), how big it is across and along, and how near a
/// fresh one has to land to an old one to be the same mark again.
pub const SCORCH_LIFE: f32 = 4.0;
pub const SCORCH_HOT: f32 = 0.35;
pub const SCORCH_SIZE: (f32, f32) = (13.0, 7.0);
pub const SCORCH_MERGE: f32 = 6.0;
/// Dark and a little brown: soot, not a hole.
pub const SCORCH: Color = Color::rgba(0.035, 0.03, 0.025, 0.55);
pub const SCORCH_WARM: Color = Color::rgba(1.0, 0.52, 0.22, 0.5);
/// The sniper's beam, hanging in the air after the bolt has landed.
pub const BEAM_LINGER: f32 = 0.25;
/// The flash on the part a hit struck: how long, and its colour — the
/// room's old hit flash, a warm white, on the part now and not the whole
/// body.
pub const STRUCK_LIFE: f32 = 0.22;
pub const STRUCK: Color = Color::rgb(1.0, 0.95, 0.85);

// --- the blade ---------------------------------------------------------------

/// A schword's cut: how long it glows, in real seconds, how wide an arc
/// it sweeps round the swinger and how far out.
pub const CUT_LIFE: f32 = 0.16;
pub const CUT_ARC: f32 = 100.0 * (core::f32::consts::PI / 180.0);
pub const CUT_REACH: f32 = 46.0;

// --- a machine bursting ------------------------------------------------------

/// A droid's head or chassis at nothing (feature 98): a flash, sparks
/// thrown clear — hot, so they glow — and plates flung off it that land
/// and fade — cold, so they do not. Nothing of a Bim going down, which
/// is a body falling and blood.
pub const BURST_FLASH: f32 = 0.22;
pub const BURST_SPARK_LIFE: f32 = 0.65;
pub const BURST_DEBRIS_FLIGHT: f32 = 0.35;
pub const BURST_LIFE: f32 = 1.6;
pub const BURST_SPARKS: u32 = 14;
pub const BURST_DEBRIS: u32 = 7;
pub const BURST_HOT: Color = Color::rgb(1.0, 0.72, 0.38);
pub const BURST_SPARK: Color = Color::rgb(1.0, 0.82, 0.48);
pub const BURST_SPARK_HEAT: f32 = 2.0;
pub const DEBRIS_PLATE: Color = Color::rgb(0.33, 0.36, 0.40);
pub const DEBRIS_DARK: Color = Color::rgb(0.15, 0.16, 0.18);

/// The most of its first frame a fresh effect is aged by, in real
/// seconds: a sixtieth, one frame at the pace the game is drawn at. See
/// [`Fx::age`].
pub const FIRST_FRAME: f32 = 1.0 / 60.0;

// --- the caps ----------------------------------------------------------------

pub const FLARE_CAP: usize = 160;
pub const SCORCH_CAP: usize = 40;
pub const STRUCK_CAP: usize = 32;
pub const BURST_CAP: usize = 8;

/// How long a Guardian's shield flares where a bolt or a blow stopped
/// on it (feature 100), and how far round the plate the flare runs either
/// side of the spot, in radians.
pub const SHIELD_FLARE_LIFE: f32 = 0.35;
pub const SHIELD_FLARE_SPAN: f32 = 0.8;

/// A side's colour: blue for the crew's fire, red for the enemy's —
/// always, whatever the weapon.
pub fn side(hostile: bool) -> Color {
    if hostile { HOSTILE_BOLT } else { FRIENDLY_BOLT }
}

/// A side's core, `heat` times as bright: past white, so it blooms.
pub fn hot(hostile: bool, heat: f32) -> Color {
    side(hostile).mix(CORE_WHITE, 1.0 - CORE_TINT).glowing(heat)
}

/// What a tier does to a shot's look: its width multiplied, its heat
/// added to.
pub fn tier_look(tier: Tier) -> (f32, f32) {
    let above = (tier.code() - 1) as f32;
    (1.0 + TIER_WIDTH * above, TIER_HEAT * above)
}

/// One stroke of laser light from `tail` to `head`: a faint halo `glow`
/// wide in the side's colour, a band of it round the core, and the core
/// `core` wide past white — the part that blooms. `alpha` fades all three.
#[allow(clippy::too_many_arguments)]
pub fn laser(
    list: &mut DrawList,
    tail: Vec2,
    head: Vec2,
    glow: f32,
    core: f32,
    hostile: bool,
    heat: f32,
    alpha: f32,
) {
    let colour = side(hostile);
    list.line(tail, head, glow, colour.alpha(0.26 * alpha));
    list.line(tail, head, core + 1.4, colour.alpha(0.8 * alpha));
    list.line(tail, head, core, hot(hostile, heat).alpha(alpha));
}

/// A stable number in `[0, 1)` for the `i`th piece of effect number
/// `seq`. A hash, never a roll: see the module note.
pub fn scatter(seq: u32, i: u32) -> f32 {
    let mut h = seq.wrapping_mul(0x9E37_79B9) ^ i.wrapping_mul(0x85EB_CA6B) ^ 0x5bd1_e995;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// How much of a flash is left, one to nought, `through` its life: bright
/// for longer than a straight line and gone quickly at the end, so a light
/// that lives four frames is seen at full strength for two of them.
fn flash(through: f32) -> f32 {
    (1.0 - clamp(through, 0.0, 1.0)).sqrt()
}

/// Out fast and settling: how far along a throw is, nought to one.
fn ease_out(t: f32) -> f32 {
    let t = clamp(t, 0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t)
}

/// The muzzle's life and size for a gun; nought for anything that does
/// not fire out of a barrel of its own — a blade, a claw, and the
/// Unmaker, whose ring at the muzzle is its own flash (`crate::droid`).
fn muzzle_look(kind: WeaponKind) -> (f32, f32) {
    match kind {
        WeaponKind::LaserPistol => MUZZLE_PISTOL,
        WeaponKind::Shotgun => MUZZLE_SHOTGUN,
        WeaponKind::AutoRifle => MUZZLE_AUTO,
        WeaponKind::SniperRifle => MUZZLE_SNIPER,
        WeaponKind::Schword | WeaponKind::Claw | WeaponKind::Unmaker | WeaponKind::Sweeper => {
            (0.0, 0.0)
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Light {
    /// Where a shot left: the barrel's direction and the gun.
    Muzzle { dir: Vec2, kind: WeaponKind },
    /// Where a bolt stopped, and the way it was flying.
    Impact { dir: Vec2 },
    /// The sniper's beam, from the muzzle to where it ended.
    Beam { from: Vec2 },
    /// A blade's cut, round the swinger, facing the way it swung.
    Cut { facing: f32 },
    /// A bolt or a blow stopped at a Guardian's shield (feature 100):
    /// the plate flaring round the spot, centred on the machine.
    Shield { centre: Vec2 },
}

#[derive(Clone, Copy, Debug)]
struct Flare {
    at: Vec2,
    light: Light,
    hostile: bool,
    width: f32,
    heat: f32,
    age: f32,
    life: f32,
    seq: u32,
}

#[derive(Clone, Copy, Debug)]
struct Scorch {
    at: Vec2,
    rot: f32,
    age: f32,
}

#[derive(Clone, Copy, Debug)]
struct Struck {
    /// Which of the room's bodies — its Bims, then its machines, the
    /// way `Game::body_seen` counts them — and the part's code: a
    /// `health::Part`'s for a Bim, a `droid::DroidPart`'s for a machine.
    body: usize,
    part: u32,
    age: f32,
}

#[derive(Clone, Copy, Debug)]
struct Burst {
    at: Vec2,
    /// Half the machine's width: what the throw is measured in.
    size: f32,
    age: f32,
    seq: u32,
}

/// The passing lights of one room. See the module note.
#[derive(Default)]
pub struct Fx {
    /// Whether a host is ageing them: nothing is recorded until one does.
    on: bool,
    flares: Vec<Flare>,
    scorches: Vec<Scorch>,
    struck: Vec<Struck>,
    bursts: Vec<Burst>,
    /// Counts every effect spawned, for [`scatter`].
    seq: u32,
}

/// Push onto a capped list, the oldest going first.
fn capped<T>(list: &mut Vec<T>, cap: usize, item: T) {
    if list.len() >= cap {
        list.remove(0);
    }
    list.push(item);
}

impl Fx {
    /// Whether a host is ageing the effects, which is when there are any
    /// — and when the fight's own sim-time spark and hit flash stand
    /// aside for them.
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// Age everything by `dt` **real** seconds — the host's frame, nought
    /// while it is paused — and switch recording on.
    ///
    /// Something spawned during the steps of this frame is aged by no more
    /// than [`FIRST_FRAME`] of it: it happened somewhere inside the frame
    /// and not at its start, and a long frame — the first of a run, a
    /// hitch — would otherwise age a four-frame flash out before it was
    /// ever drawn.
    pub fn age(&mut self, dt: f32) {
        self.on = true;
        if dt <= 0.0 {
            return;
        }
        let step = |age: &mut f32| *age += if *age == 0.0 { dt.min(FIRST_FRAME) } else { dt };
        for f in &mut self.flares {
            step(&mut f.age);
        }
        self.flares.retain(|f| f.age < f.life);
        for s in &mut self.scorches {
            step(&mut s.age);
        }
        self.scorches.retain(|s| s.age < SCORCH_LIFE);
        for s in &mut self.struck {
            step(&mut s.age);
        }
        self.struck.retain(|s| s.age < STRUCK_LIFE);
        for b in &mut self.bursts {
            step(&mut b.age);
        }
        self.bursts.retain(|b| b.age < BURST_LIFE);
    }

    /// How many are alive, every list together: for the tests.
    pub fn count(&self) -> usize {
        self.flares.len() + self.scorches.len() + self.struck.len() + self.bursts.len()
    }

    fn next(&mut self) -> u32 {
        self.seq = self.seq.wrapping_add(1);
        self.seq
    }

    fn flare(&mut self, at: Vec2, light: Light, hostile: bool, weapon: Weapon, life: f32) {
        if !self.on || life <= 0.0 {
            return;
        }
        let (width, heat) = tier_look(weapon.tier);
        let seq = self.next();
        capped(
            &mut self.flares,
            FLARE_CAP,
            Flare {
                at,
                light,
                hostile,
                width,
                heat,
                age: 0.0,
                life,
                seq,
            },
        );
    }

    /// A shot left a muzzle at `at`, along `dir`.
    pub fn muzzle(&mut self, at: Vec2, dir: Vec2, weapon: Weapon, hostile: bool) {
        let (life, _) = muzzle_look(weapon.kind);
        let dir = dir.normalize_or_zero();
        self.flare(
            at,
            Light::Muzzle {
                dir,
                kind: weapon.kind,
            },
            hostile,
            weapon,
            life,
        );
    }

    /// A bolt stopped at `at`, flying along `dir`: a flash, and — where
    /// it was a wall or a lamp that stopped it rather than a body — a
    /// scorch. A sniper's leaves its beam hanging from `from` besides.
    pub fn landed(
        &mut self,
        from: Vec2,
        at: Vec2,
        dir: Vec2,
        weapon: Weapon,
        hostile: bool,
        on_body: bool,
    ) {
        if !self.on {
            return;
        }
        let dir = dir.normalize_or_zero();
        self.flare(at, Light::Impact { dir }, hostile, weapon, IMPACT_LIFE);
        if weapon.kind == WeaponKind::SniperRifle {
            self.spent(from, at, weapon, hostile);
        }
        if on_body {
            return;
        }
        // A fresh bolt into the same spot is the same mark, hot again.
        if let Some(old) = self
            .scorches
            .iter_mut()
            .find(|s| (s.at - at).len() < SCORCH_MERGE)
        {
            old.age = 0.0;
            return;
        }
        let seq = self.next();
        let rot = dir.angle() + TAU * 0.25 + (scatter(seq, 0) - 0.5) * 0.6;
        capped(&mut self.scorches, SCORCH_CAP, Scorch { at, rot, age: 0.0 });
    }

    /// A bolt that flew out its range and faded: only the sniper's beam
    /// is left of it, hanging from the muzzle to where it gave out.
    pub fn spent(&mut self, from: Vec2, at: Vec2, weapon: Weapon, hostile: bool) {
        if weapon.kind == WeaponKind::SniperRifle {
            self.flare(at, Light::Beam { from }, hostile, weapon, BEAM_LINGER);
        }
    }

    /// A blade's blow landed: the cut glows round the swinger at `from`,
    /// facing `toward`.
    pub fn cut(&mut self, from: Vec2, toward: Vec2, weapon: Weapon, hostile: bool) {
        let facing = (toward - from).angle();
        self.flare(from, Light::Cut { facing }, hostile, weapon, CUT_LIFE);
    }

    /// A Guardian's beam burning where a wall stopped it (feature 100): a
    /// scorch at `at`, laid across the beam's way `dir`, and nothing lit —
    /// the beam's own end is drawn with the beam. A sweep lays one a
    /// sub-step, so the scorches run along the wall as a streak.
    pub fn burn(&mut self, at: Vec2, dir: Vec2) {
        if !self.on {
            return;
        }
        let rot = dir.angle() + TAU * 0.25;
        capped(&mut self.scorches, SCORCH_CAP, Scorch { at, rot, age: 0.0 });
    }

    /// A bolt or a blow stopped on a Guardian's shield (feature 100): the
    /// plate round `centre` flares at `at`, where it was struck. Always the
    /// machines' red: the shield is theirs, whoever's bolt it stopped.
    pub fn shield(&mut self, centre: Vec2, at: Vec2) {
        self.flare(
            at,
            Light::Shield { centre },
            true,
            WeaponKind::Sweeper.basic(),
            SHIELD_FLARE_LIFE,
        );
    }

    /// A hit struck that part of that body: a flash on the part, drawn
    /// with the body wherever it has got to (`Game::render`).
    pub fn struck(&mut self, body: usize, part: u32) {
        if !self.on {
            return;
        }
        capped(
            &mut self.struck,
            STRUCK_CAP,
            Struck {
                body,
                part,
                age: 0.0,
            },
        );
    }

    /// A machine `size` across the middle burst apart at `at`.
    pub fn burst(&mut self, at: Vec2, size: f32) {
        if !self.on {
            return;
        }
        let seq = self.next();
        capped(
            &mut self.bursts,
            BURST_CAP,
            Burst {
                at,
                size,
                age: 0.0,
                seq,
            },
        );
    }

    /// The flashes on one body's parts, as `(part code, how much is left
    /// of it, one to nought)`.
    pub fn struck_on(&self, body: usize) -> impl Iterator<Item = (u32, f32)> + '_ {
        self.struck
            .iter()
            .filter(move |s| s.body == body)
            .map(|s| (s.part, flash(s.age / STRUCK_LIFE)))
    }

    /// What lies on the deck: the scorches. Under the bodies, over the
    /// deck and the mess.
    pub fn draw_ground(&self, list: &mut DrawList) {
        for s in &self.scorches {
            let t = s.age / SCORCH_LIFE;
            // Full for the first half of its life, then fading out.
            let fade = clamp(2.0 - 2.0 * t, 0.0, 1.0);
            let (along, across) = SCORCH_SIZE;
            list.ellipse(
                s.at,
                vec2(along, across) * 1.35,
                s.rot,
                SCORCH.alpha(SCORCH.a * 0.45 * fade),
            );
            list.ellipse(
                s.at,
                vec2(along, across),
                s.rot,
                SCORCH.alpha(SCORCH.a * fade),
            );
            if s.age < SCORCH_HOT {
                let h = 1.0 - s.age / SCORCH_HOT;
                list.ellipse(
                    s.at,
                    vec2(along, across) * 0.55,
                    s.rot,
                    SCORCH_WARM.alpha(SCORCH_WARM.a * h),
                );
            }
        }
    }

    /// The plates a bursting machine throws, with the bodies: flung out,
    /// landing, and fading where they lie. Cold metal — nothing here
    /// glows.
    pub fn draw_debris(&self, list: &mut DrawList) {
        for b in &self.bursts {
            let flight = ease_out(b.age / BURST_DEBRIS_FLIGHT);
            let fade = clamp((BURST_LIFE - b.age) / (BURST_LIFE * 0.5), 0.0, 1.0);
            for i in 0..BURST_DEBRIS {
                let angle = scatter(b.seq, i * 4) * TAU;
                let out = b.size * (0.9 + 1.1 * scatter(b.seq, i * 4 + 1));
                let at = b.at + Vec2::from_angle(angle) * (out * flight);
                let spin = (scatter(b.seq, i * 4 + 2) - 0.5) * 7.0;
                let size = vec2(
                    4.0 + 5.0 * scatter(b.seq, i * 4 + 3),
                    3.0 + 2.0 * scatter(b.seq, i * 4 + 2),
                );
                let colour = if i % 2 == 0 {
                    DEBRIS_PLATE
                } else {
                    DEBRIS_DARK
                };
                list.rect(at, size, angle + spin * flight, 0.8, colour.alpha(fade));
            }
        }
    }

    /// Everything in the air, over the fog with the bolts: the muzzles,
    /// the flashes, the beams, the cuts, and a bursting machine's flash
    /// and sparks.
    pub fn draw_air(&self, list: &mut DrawList) {
        for f in &self.flares {
            let t = flash(f.age / f.life);
            match f.light {
                Light::Muzzle { dir, kind } => draw_muzzle(list, f, dir, kind, t),
                Light::Impact { dir } => draw_impact(list, f, dir, t),
                Light::Beam { from } => {
                    // Thinning and fading from the whole beam to nothing.
                    let w = f.width * (0.4 + 0.6 * t);
                    laser(
                        list,
                        from,
                        f.at,
                        6.0 * w,
                        1.6 * w,
                        f.hostile,
                        1.6 + f.heat,
                        t * t,
                    );
                }
                Light::Cut { facing } => draw_cut(list, f, facing, t),
                Light::Shield { centre } => draw_shield_flare(list, f, centre, t),
            }
        }
        for b in &self.bursts {
            // The flash: a hot swell, gone in a fifth of a second.
            if b.age < BURST_FLASH {
                let u = b.age / BURST_FLASH;
                list.circle(
                    b.at,
                    b.size * (1.6 + 2.2 * u),
                    BURST_HOT.alpha(0.45 * (1.0 - u)),
                );
                list.circle(
                    b.at,
                    b.size * (0.9 + 0.6 * u),
                    BURST_HOT.glowing(1.8).alpha(1.0 - u),
                );
            }
            // The sparks: thrown clear and slowing, each a short streak
            // along its own flight.
            if b.age < BURST_SPARK_LIFE {
                let u = b.age / BURST_SPARK_LIFE;
                let reach = ease_out(u);
                for i in 0..BURST_SPARKS {
                    let dir = Vec2::from_angle(scatter(b.seq, 100 + i * 2) * TAU);
                    let out = b.size * (1.6 + 2.4 * scatter(b.seq, 101 + i * 2));
                    let head = b.at + dir * (out * reach);
                    let tail = head - dir * (2.0 + 7.0 * (1.0 - u));
                    list.line(
                        tail,
                        head,
                        1.6,
                        BURST_SPARK.glowing(BURST_SPARK_HEAT).alpha(1.0 - u),
                    );
                }
            }
        }
    }
}

fn draw_muzzle(list: &mut DrawList, f: &Flare, dir: Vec2, kind: WeaponKind, t: f32) {
    let (_, size) = muzzle_look(kind);
    let size = size * f.width;
    let colour = side(f.hostile);
    list.circle(f.at, size * 2.6 * (0.7 + 0.3 * t), colour.alpha(0.35 * t));
    list.circle(
        f.at,
        size * (0.5 + 0.5 * t),
        hot(f.hostile, 2.0 + f.heat).alpha(t),
    );
    match kind {
        WeaponKind::Shotgun => {
            // A cone of short rays: the spread leaving the bore.
            for i in 0..3 {
                let turn = (i as f32 - 1.0) * 0.35;
                let d = dir.rotate(turn);
                laser(
                    list,
                    f.at,
                    f.at + d * (MUZZLE_RAY * f.width * (0.6 + 0.4 * t)),
                    3.0,
                    1.1,
                    f.hostile,
                    1.6 + f.heat,
                    t,
                );
            }
        }
        WeaponKind::SniperRifle => {
            laser(
                list,
                f.at,
                f.at + dir * (MUZZLE_SPIKE * f.width),
                4.0,
                1.4,
                f.hostile,
                2.0 + f.heat,
                t,
            );
        }
        _ => {}
    }
}

fn draw_impact(list: &mut DrawList, f: &Flare, dir: Vec2, t: f32) {
    let colour = side(f.hostile);
    list.circle(
        f.at,
        (10.0 + 14.0 * (1.0 - t)) * f.width,
        colour.alpha(0.5 * t),
    );
    list.circle(
        f.at,
        6.0 * t * f.width,
        hot(f.hostile, 2.2 + f.heat).alpha(t),
    );
    // A few rays thrown back the way the bolt came, fanned.
    let back = dir * -1.0;
    for i in 0..IMPACT_RAYS {
        let turn = (scatter(f.seq, i) - 0.5) * 2.2;
        let d = back.rotate(turn);
        let reach = 4.0 + 9.0 * (1.0 - t) * (0.6 + 0.4 * scatter(f.seq, i + 7));
        list.line(
            f.at + d * (reach * 0.4),
            f.at + d * reach,
            1.2,
            hot(f.hostile, 1.6 + f.heat).alpha(t),
        );
    }
}

fn draw_cut(list: &mut DrawList, f: &Flare, facing: f32, t: f32) {
    const STEPS: usize = 6;
    let reach = CUT_REACH * (1.0 + 0.1 * (1.0 - t));
    let mut prev = f.at + Vec2::from_angle(facing - CUT_ARC * 0.5) * reach;
    for i in 1..=STEPS {
        let a = facing - CUT_ARC * 0.5 + CUT_ARC * (i as f32 / STEPS as f32);
        let next = f.at + Vec2::from_angle(a) * reach;
        // Brightest in the middle of the arc, where the blade met.
        let mid = 1.0 - ((i as f32 - 0.5) / STEPS as f32 - 0.5).abs() * 1.2;
        laser(
            list,
            prev,
            next,
            7.0 * f.width,
            2.0 * f.width * (0.5 + 0.5 * t),
            f.hostile,
            2.0 + f.heat,
            t * mid,
        );
        prev = next;
    }
}

/// A Guardian's shield flaring where it was struck (feature 100): the
/// plate itself lit up over a run of its arc either side of the spot —
/// thicker and redder there, its rim past white at the spot and dying off
/// along the plate — so the bloom makes the glow and nothing is drawn
/// round it by hand.
fn draw_shield_flare(list: &mut DrawList, f: &Flare, centre: Vec2, t: f32) {
    const STEPS: usize = 6;
    let out = f.at - centre;
    let radius = out.len().max(1.0);
    let mid = out.angle();
    let colour = side(f.hostile);
    let span = SHIELD_FLARE_SPAN * (0.6 + 0.4 * (1.0 - t));
    let at = |a: f32, r: f32| centre + Vec2::from_angle(a) * r;
    for i in 0..STEPS {
        let a0 = mid - span + 2.0 * span * (i as f32 / STEPS as f32);
        let a1 = mid - span + 2.0 * span * ((i + 1) as f32 / STEPS as f32);
        // Brightest where it struck, dying off along the plate.
        let near = (1.0 - ((i as f32 + 0.5) / STEPS as f32 - 0.5).abs() * 1.8).max(0.0);
        list.line(
            at(a0, radius),
            at(a1, radius),
            10.0,
            colour.alpha(0.6 * t * near),
        );
        list.line(
            at(a0, radius + 2.5),
            at(a1, radius + 2.5),
            2.0,
            hot(f.hostile, 1.4 + 1.2 * near).alpha(t * near),
        );
    }
}

/// The flash on a struck part: a warm white wash over it and a ring
/// round it, centred at `at`, `radius` across — drawn by the room over
/// the body it belongs to. Not past white: a flash on a body is not the
/// shot's light, and the bloom stays the shots'.
pub fn draw_struck(list: &mut DrawList, at: Vec2, radius: f32, t: f32) {
    list.circle(
        at,
        radius * 2.0 * (1.0 + 0.5 * (1.0 - t)),
        STRUCK.alpha(0.5 * t),
    );
    list.ring(
        at,
        radius * 2.0 * (1.2 + 0.6 * (1.0 - t)),
        1.5,
        STRUCK.alpha(0.8 * t),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pistol() -> Weapon {
        WeaponKind::LaserPistol.basic()
    }

    /// Nothing is recorded for a room nobody ages — a test, a probe, a
    /// server — and once one is, a flash lives its real seconds whatever
    /// the steps between, and is gone after them.
    #[test]
    fn nothing_is_recorded_until_a_host_ages_it_and_a_flash_lives_real_seconds() {
        let mut fx = Fx::default();
        fx.muzzle(Vec2::ZERO, vec2(1.0, 0.0), pistol(), false);
        fx.landed(
            Vec2::ZERO,
            vec2(50.0, 0.0),
            vec2(1.0, 0.0),
            pistol(),
            false,
            false,
        );
        fx.struck(0, 1);
        fx.burst(Vec2::ZERO, 12.0);
        assert_eq!(fx.count(), 0, "off until aged");
        assert!(!fx.is_on());

        fx.age(0.0);
        assert!(fx.is_on());
        fx.landed(
            Vec2::ZERO,
            vec2(50.0, 0.0),
            vec2(1.0, 0.0),
            pistol(),
            false,
            false,
        );
        // A flash and a scorch.
        assert_eq!(fx.count(), 2);
        // Paused: nothing ages.
        for _ in 0..100 {
            fx.age(0.0);
        }
        assert_eq!(fx.count(), 2, "a pause holds every effect where it is");
        // A long frame — a hitch — ages a fresh one by a frame and no
        // more, so it is drawn at least once.
        fx.age(0.5);
        assert_eq!(fx.count(), 2, "a hitch does not age a fresh flash out");
        fx.age(IMPACT_LIFE);
        assert_eq!(fx.count(), 1, "the flash is gone, the scorch lies");
        fx.age(SCORCH_LIFE);
        assert_eq!(fx.count(), 0, "and the scorch after it");
    }

    /// Every list is capped and the oldest goes first; a bolt into the
    /// same spot is the same scorch again, not another.
    #[test]
    fn the_lists_are_capped_and_a_scorch_in_the_same_spot_is_one_mark() {
        let mut fx = Fx::default();
        fx.age(0.0);
        for i in 0..(SCORCH_CAP * 3) {
            let at = vec2(i as f32 * 20.0, 0.0);
            fx.landed(at, at, vec2(1.0, 0.0), pistol(), false, false);
        }
        assert_eq!(fx.scorches.len(), SCORCH_CAP);
        assert_eq!(fx.flares.len(), FLARE_CAP.min(SCORCH_CAP * 3));
        // The newest survived.
        let last = vec2((SCORCH_CAP * 3 - 1) as f32 * 20.0, 0.0);
        assert!(fx.scorches.iter().any(|s| (s.at - last).len() < 1e-3));

        let mut fx = Fx::default();
        fx.age(0.0);
        for _ in 0..10 {
            fx.landed(
                Vec2::ZERO,
                vec2(2.0, 1.0),
                vec2(1.0, 0.0),
                pistol(),
                false,
                false,
            );
        }
        assert_eq!(fx.scorches.len(), 1, "one mark, hit ten times");
        for _ in 0..(STRUCK_CAP * 2) {
            fx.struck(3, 0);
        }
        assert_eq!(fx.struck.len(), STRUCK_CAP);
        for _ in 0..(BURST_CAP * 2) {
            fx.burst(Vec2::ZERO, 12.0);
        }
        assert_eq!(fx.bursts.len(), BURST_CAP);
    }

    /// Only a shot's light is past white. The scorch, the debris and the
    /// flash on a struck body are not — the bloom is the shots' — and
    /// every core is, in the side's colour: blue's core is bluest, red's
    /// reddest.
    #[test]
    fn a_core_is_past_white_in_its_side_s_colour_and_the_marks_are_not() {
        let past = |c: Color| c.r > 1.0 || c.g > 1.0 || c.b > 1.0;
        for hostile in [false, true] {
            let c = hot(hostile, CORE_HEAT);
            assert!(past(c));
            if hostile {
                assert!(c.r > c.b, "red's core leans red");
            } else {
                assert!(c.b > c.r, "blue's core leans blue");
            }
        }
        for c in [SCORCH, SCORCH_WARM, STRUCK, DEBRIS_PLATE, DEBRIS_DARK] {
            assert!(!past(c));
        }
        assert!(past(BURST_SPARK.glowing(BURST_SPARK_HEAT)));
        // And a tier draws thicker and hotter, never another colour.
        let (w1, h1) = tier_look(Tier::One);
        let (w3, h3) = tier_look(Tier::Three);
        assert_eq!((w1, h1), (1.0, 0.0));
        assert!(w3 > w1 && h3 > h1);
    }

    /// Scatter is a hash: the same number for the same piece every time,
    /// and spread over the unit.
    #[test]
    fn scatter_is_a_stable_hash_over_the_unit() {
        let mut sum = 0.0;
        for i in 0..1000 {
            let v = scatter(7, i);
            assert!((0.0..1.0).contains(&v));
            assert_eq!(v, scatter(7, i));
            sum += v;
        }
        let mean = sum / 1000.0;
        assert!((0.4..0.6).contains(&mean), "mean {mean}");
    }
}
