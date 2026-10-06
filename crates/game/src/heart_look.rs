//! The Machine Heart's machines (feature 108): what the world tells each of
//! them ([`HeartState`]) and what they look like. A child of `droid`, so the
//! pictures reach the machine's own timers and its palette.
//!
//! **The core** is round and squat: an armoured octagon with four vanes, a
//! great red orb in the middle and two emitter pods on its rim that swing
//! round to whatever they are winding up on. Sealed, a shell of red plates
//! stands round it with a rim past white; exposed, the orb burns brighter;
//! in its overload it flickers white-hot and the vanes glow. **A conduit**
//! is a pylon — a plate, a coil and a crystal on top — with a line of red
//! light running from it to the core it seals, flickering, while the seal
//! stands. **A fabricator** is a squat block with a bay in its front and
//! two arms over the bay, the bay flaring when it has just built. Each has
//! a wreck of its own, drawn over the common mess
//! (`Droid::draw_wreck_ground`). What glows is past white and nothing else,
//! as for the Guardian (feature 97's bloom); every flicker is a hash of the
//! moment (`Droid::scatter`), never a roll.

use super::*;

/// What the world has told a Machine Heart's machine (feature 108), and
/// what its step keeps for it. Nothing here is decided in the room: the
/// world says whether the core is sealed, how many of its emitters may
/// fire and how hard and fast, every step (`world::heart`).
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HeartState {
    /// The core's shell is up: nothing gets through it, and nobody aims at
    /// it (`combat::Target::sealed`).
    pub sealed: bool,
    /// How many of the core's emitters may fire: none while sealed, one
    /// exposed, two in its overload.
    pub emitters: u8,
    /// What the Sweeper's damage is multiplied by, and how many times a
    /// Guardian's speed the beam sweeps at.
    pub damage: f32,
    pub pace: f32,
    /// Whether the core is in its overload: the picture's to read.
    pub overload: bool,
    /// Each emitter's rhythm — the Guardian's [`Beam`] — and how many
    /// sweeps have gone, the side the next starts from alternating by it.
    pub beams: [Beam; HeartState::EMITTERS],
    pub sweeps: u32,
    /// Seconds of a fabricator's bay still flaring after a build.
    pub made: f32,
    /// Where the core a conduit feeds stands, while it is sealed: the line
    /// the picture draws. The room's to keep.
    pub link: Option<Vec2>,
}

impl HeartState {
    /// How many emitters a core has.
    pub const EMITTERS: usize = 2;
    /// How far out from the core's middle an emitter's lens is, in room
    /// units: where its beam leaves from.
    pub const LENS_OUT: f32 = 32.0;
    /// How long a fabricator's bay flares after a build, in seconds.
    pub const MADE_FLASH: f32 = 1.2;
}

impl Default for HeartState {
    fn default() -> HeartState {
        HeartState {
            sealed: false,
            emitters: 0,
            damage: 1.0,
            pace: 1.0,
            overload: false,
            beams: [Beam::Ready; HeartState::EMITTERS],
            sweeps: 0,
            made: 0.0,
            link: None,
        }
    }
}

/// The core's orb, in the machines' red; its vanes' glow in the overload.
const ORB: Color = Color::rgb(0.86, 0.12, 0.10);
/// The conduit's crystal and the light along its link.
const CRYSTAL: Color = Color::rgb(0.95, 0.30, 0.24);
/// How fast the orb beats at rest, and in the overload, in radians a
/// second.
const BEAT: f32 = 2.4;
const BEAT_OVERLOAD: f32 = 9.0;
/// How much larger than its own units the core is drawn: it never walks a
/// corridor, and it is what the run is for.
pub(super) const CORE_SCALE: f32 = 1.35;
/// The shell round a sealed core: how many plates, how far out.
const SHELL_PLATES: u32 = 12;
const SHELL_OUT: f32 = 44.0;
/// How far out a sealed conduit's shell is.
const CONDUIT_SHELL_OUT: f32 = 36.0;

impl Droid {
    /// A Machine Heart's machine of `kind` (feature 108), of **one** health
    /// — the world's number, never the tier's — stood at `at` facing
    /// `facing`. Built where it stands and never moved: a conduit, a
    /// fabricator or the core.
    pub fn structure(
        kind: DroidKind,
        tier: Tier,
        health: f32,
        at: Vec2,
        facing: Vec2,
        seed: u64,
    ) -> Droid {
        let mut d = Droid::new(kind, tier, 0, 0, at, 0.0, seed);
        d.body = DroidBody::solid(health);
        d.with_facing(facing)
    }

    /// Where a core's emitter points: at what it is winding up on or
    /// sweeping, and otherwise slowly round, the two opposite each other.
    pub(super) fn emitter_dir(&self, e: usize) -> Vec2 {
        match self.heart.beams[e] {
            Beam::WindUp { aim, .. } | Beam::Sweep { aim, .. } => aim,
            _ => {
                let base = self.idle * 0.35 + e as f32 * core::f32::consts::PI;
                Vec2::from_angle(base)
            }
        }
    }

    /// How far through a wind-up an emitter is, nought to one.
    fn emitter_wind(&self, e: usize) -> Option<(f32, Vec2, Vec2)> {
        match self.heart.beams[e] {
            Beam::WindUp { left, aim, at, .. } => Some((
                clamp(1.0 - left / balance::SWEEPER_WINDUP, 0.0, 1.0),
                aim,
                at,
            )),
            _ => None,
        }
    }

    /// The core's beat, nought to one: slow at rest, fast in the overload.
    fn beat(&self) -> f32 {
        let rate = if self.heart.overload {
            BEAT_OVERLOAD
        } else {
            BEAT
        };
        0.5 + 0.5 * (self.idle * rate).sin()
    }

    /// The core standing (drawn in its own frame, unturned by its facing:
    /// it is round and faces every way).
    pub(super) fn draw_core(&self, b: &mut Brush) {
        let beat = self.beat();
        // The armoured housing: an octagon of two squares, and the vanes.
        b.rect(Vec2::ZERO, vec2(56.0, 56.0), 0.0, 8.0, HULL_DARK);
        b.rect(
            Vec2::ZERO,
            vec2(56.0, 56.0),
            core::f32::consts::FRAC_PI_4,
            8.0,
            HULL_DARK,
        );
        b.rect(Vec2::ZERO, vec2(48.0, 48.0), 0.0, 7.0, HULL);
        b.rect(
            Vec2::ZERO,
            vec2(48.0, 48.0),
            core::f32::consts::FRAC_PI_4,
            7.0,
            HULL,
        );
        let vane = if self.heart.overload {
            ORB.mix(SPARK, beat).glowing(1.0 + 0.6 * beat)
        } else {
            PLATE
        };
        for k in 0..4 {
            let a = k as f32 * core::f32::consts::FRAC_PI_2 + core::f32::consts::FRAC_PI_4;
            let at = Vec2::from_angle(a) * 20.0;
            b.rect(at, vec2(12.0, 5.0), a, 2.0, vane);
            b.rect(at * 1.15, vec2(5.0, 3.0), a, 1.0, JOINT);
        }
        // The orb: a deep socket, the ring round it and the light itself.
        b.ellipse(Vec2::ZERO, vec2(30.0, 30.0), 0.0, JOINT);
        b.ellipse(Vec2::ZERO, vec2(25.0, 25.0), 0.0, STEEL);
        let (glass, heart) = if self.heart.sealed {
            (
                SENSOR_OUT.mix(ORB, 0.55 + 0.35 * beat),
                ORB.mix(SPARK, 0.2 * beat),
            )
        } else if self.heart.overload {
            let flick = self.scatter(500 + (self.idle * 20.0) as u32);
            (
                ORB.mix(SPARK, 0.4 + 0.4 * flick).glowing(1.4 + 1.2 * beat),
                SPARK.glowing(2.0 + 1.2 * flick),
            )
        } else {
            (
                ORB.glowing(1.05 + 0.5 * beat),
                ORB.mix(SPARK, 0.5).glowing(1.4 + 0.8 * beat),
            )
        };
        b.ellipse(Vec2::ZERO, vec2(21.0, 21.0), 0.0, glass);
        b.ellipse(
            Vec2::ZERO,
            vec2(9.0 + 3.0 * beat, 9.0 + 3.0 * beat),
            0.0,
            heart,
        );
    }

    /// The core's emitters, over it: a pod on the rim a side, swung round
    /// to its aim, its lens lit by where it is in its rhythm. In world
    /// units, since each points its own way.
    pub(super) fn draw_core_emitters(&self, list: &mut DrawList) {
        for e in 0..HeartState::EMITTERS {
            let dir = self.emitter_dir(e);
            let at = self.pos + dir * HeartState::LENS_OUT;
            let a = dir.angle();
            let mut b = list.brush(at, a, 1.0);
            b.rect(vec2(-3.0, 0.0), vec2(14.0, 12.0), 0.0, 3.0, JOINT);
            b.rect(vec2(-2.0, 0.0), vec2(10.0, 9.0), 0.0, 2.5, STEEL);
            let lit = e < self.heart.emitters as usize;
            let lens = match (self.heart.beams[e], self.emitter_wind(e)) {
                (Beam::Sweep { .. }, _) => SPARK.glowing(2.6),
                (_, Some((t, _, _))) => SENSOR.mix(SPARK, t).glowing(0.9 + 1.6 * t),
                _ if lit => SENSOR,
                _ => SENSOR_OUT,
            };
            b.ellipse(vec2(3.0, 0.0), vec2(7.0, 7.0), 0.0, lens);
        }
    }

    /// A ring of red plates `r` out round a sealed machine of the Heart's,
    /// a translucent band with a rim past white, turning slowly.
    fn draw_shell(&self, list: &mut DrawList, r: f32) {
        let span = TAU / SHELL_PLATES as f32;
        let turn = self.idle * 0.2;
        let beat = self.beat();
        for i in 0..SHELL_PLATES {
            let a0 = turn + span * i as f32;
            let a1 = a0 + span * 0.86;
            let p = self.pos + Vec2::from_angle(a0) * r;
            let q = self.pos + Vec2::from_angle(a1) * r;
            list.line(p, q, 7.0, SHIELD.alpha(0.16 + 0.08 * beat));
            let p = self.pos + Vec2::from_angle(a0) * (r + 3.0);
            let q = self.pos + Vec2::from_angle(a1) * (r + 3.0);
            list.line(p, q, 1.3, SHIELD_RIM.alpha(0.7 + 0.2 * beat));
        }
    }

    /// A sealed conduit's shell (October 2026): up while the wave another
    /// link's fall sent still stands, just past where a bolt is stopped
    /// (`balance::GUARDIAN_SHIELD_RADIUS`).
    pub(super) fn draw_conduit_over(&self, list: &mut DrawList) {
        if self.heart.sealed && !self.destroyed {
            self.draw_shell(list, CONDUIT_SHELL_OUT);
        }
    }

    /// A sealed core's shell (feature 108): a ring of red plates just
    /// round its housing (a bolt is stopped a little inside it, at
    /// `balance::GUARDIAN_SHIELD_RADIUS`),
    /// a translucent band with a rim past white, turning slowly; and the
    /// wind-up's targeting lines once it is exposed. On a wreck, the orb's
    /// smoke.
    pub(super) fn draw_core_over(&self, list: &mut DrawList) {
        if self.destroyed {
            self.draw_smoke(list);
            return;
        }
        if self.heart.sealed {
            self.draw_shell(list, SHELL_OUT);
        }
        self.draw_core_emitters(list);
        for e in 0..HeartState::EMITTERS {
            if let Some((t, dir, at)) = self.emitter_wind(e) {
                let lens = self.pos + dir * HeartState::LENS_OUT;
                let flick =
                    0.55 + 0.45 * self.scatter(300 + e as u32 * 7 + (self.idle * 24.0) as u32);
                list.line(
                    lens,
                    at,
                    0.9 + 0.6 * t,
                    SENSOR
                        .glowing(1.1 + 0.6 * t)
                        .alpha(flick * (0.35 + 0.65 * t)),
                );
            }
        }
    }

    /// The core's wreck: the housing split in four, the orb gone dark and
    /// cracked, the pods torn off.
    pub(super) fn draw_core_wreck(&self, b: &mut Brush) {
        self.draw_wreck_ground(b, 40.0);
        for k in 0..4u32 {
            let a = k as f32 * core::f32::consts::FRAC_PI_2
                + core::f32::consts::FRAC_PI_4
                + (self.scatter(600 + k) - 0.5) * 0.4;
            let at = Vec2::from_angle(a) * (10.0 + 5.0 * self.scatter(610 + k));
            b.rect(at, vec2(26.0, 24.0), a, 6.0, HULL_DARK);
            b.rect(at * 1.05, vec2(20.0, 18.0), a, 5.0, HULL);
        }
        self.draw_rift(b, Vec2::ZERO, vec2(26.0, 26.0), 0.0);
        b.ellipse(
            Vec2::ZERO,
            vec2(16.0, 16.0),
            0.0,
            SENSOR_OUT.mix(JOINT, 0.5),
        );
        for i in 0..5u32 {
            let a = self.scatter(620 + i) * TAU;
            let len = 5.0 + 4.0 * self.scatter(630 + i);
            b.rect(
                Vec2::from_angle(a) * (len * 0.5),
                vec2(len, 1.0),
                a,
                0.3,
                JOINT,
            );
        }
        for e in 0..2u32 {
            let a = self.scatter(640 + e) * TAU;
            let at = Vec2::from_angle(a) * (36.0 + 8.0 * self.scatter(650 + e));
            b.rect(at, vec2(12.0, 10.0), a, 3.0, JOINT);
            b.ellipse(at, vec2(6.0, 6.0), 0.0, SENSOR_OUT);
        }
    }

    /// A conduit standing: a square plate, a coil round the column and the
    /// crystal on top, pulsing — brighter while it seals a core.
    pub(super) fn draw_conduit(&self, b: &mut Brush) {
        let beat = 0.5 + 0.5 * (self.idle * 3.1 + self.scatter(700) * TAU).sin();
        b.rect(Vec2::ZERO, vec2(28.0, 28.0), 0.0, 4.0, HULL_DARK);
        b.rect(Vec2::ZERO, vec2(23.0, 23.0), 0.0, 3.5, HULL);
        for k in 0..4 {
            let a = k as f32 * core::f32::consts::FRAC_PI_2 + core::f32::consts::FRAC_PI_4;
            b.ellipse(Vec2::from_angle(a) * 13.0, vec2(4.0, 4.0), 0.0, JOINT);
        }
        b.ellipse(Vec2::ZERO, vec2(18.0, 18.0), 0.0, STEEL);
        b.ellipse(Vec2::ZERO, vec2(14.0, 14.0), 0.0, JOINT);
        // The coil: a ring of short bars round the column.
        for k in 0..8 {
            let a = k as f32 * TAU / 8.0 + self.idle * 0.8;
            b.rect(Vec2::from_angle(a) * 8.0, vec2(3.0, 1.6), a, 0.5, PLATE);
        }
        let feeding = self.heart.link.is_some();
        let crystal = if feeding {
            CRYSTAL.glowing(1.1 + 0.7 * beat)
        } else {
            SENSOR_OUT.mix(CRYSTAL, 0.4 + 0.3 * beat)
        };
        b.rect(
            Vec2::ZERO,
            vec2(8.0, 8.0),
            core::f32::consts::FRAC_PI_4,
            1.5,
            crystal,
        );
        b.ellipse(
            Vec2::ZERO,
            vec2(3.0, 3.0),
            0.0,
            SPARK.alpha(0.6 + 0.4 * beat),
        );
    }

    /// The line of light from a conduit to the core it seals, drawn under
    /// the machines: a faint band and a thin core past white that crawls
    /// and flickers along it.
    pub(super) fn draw_conduit_link(&self, list: &mut DrawList) {
        let Some(to) = self.heart.link.filter(|_| !self.destroyed) else {
            return;
        };
        let flick = 0.6 + 0.4 * self.scatter(720 + (self.idle * 16.0) as u32);
        list.line(self.pos, to, 5.0, SHIELD_DECK.alpha(0.18 * flick));
        list.line(self.pos, to, 1.1, CRYSTAL.glowing(1.2).alpha(0.35 * flick));
        // A spark of light running along it, from the conduit to the core.
        let run = (self.idle * 0.6 + self.scatter(730)).fract();
        list.circle(self.pos.lerp(to, run), 4.0, CRYSTAL.glowing(1.6).alpha(0.7));
    }

    /// A conduit's wreck: the plate cracked, the column fallen and the
    /// crystal in shards.
    pub(super) fn draw_conduit_wreck(&self, b: &mut Brush) {
        self.draw_wreck_ground(b, 20.0);
        b.rect(Vec2::ZERO, vec2(26.0, 26.0), 0.12, 4.0, HULL_DARK);
        b.rect(vec2(-3.0, 2.0), vec2(20.0, 18.0), 0.2, 3.0, HULL);
        self.draw_rift(b, Vec2::ZERO, vec2(14.0, 6.0), 0.5);
        let a = self.scatter(740) * TAU;
        b.rect(Vec2::from_angle(a) * 14.0, vec2(22.0, 7.0), a, 2.5, STEEL);
        for i in 0..4u32 {
            let a = self.scatter(750 + i) * TAU;
            let at = Vec2::from_angle(a) * (8.0 + 12.0 * self.scatter(760 + i));
            b.rect(
                at,
                vec2(4.0, 2.4),
                a * 2.0,
                0.6,
                SENSOR_OUT.mix(CRYSTAL, 0.3),
            );
        }
    }

    /// A fabricator standing: a squat block, the bay in its front, two
    /// arms folded over the bay, a lamp; the bay flaring and the arms
    /// thrown wide for a moment after it builds.
    pub(super) fn draw_fabricator(&self, b: &mut Brush) {
        let made = clamp(self.heart.made / HeartState::MADE_FLASH, 0.0, 1.0);
        b.rect(vec2(-4.0, 0.0), vec2(40.0, 46.0), 0.0, 6.0, HULL_DARK);
        b.rect(vec2(-5.0, 0.0), vec2(34.0, 40.0), 0.0, 5.0, HULL);
        for i in 0..3 {
            b.rect(
                vec2(-16.0 + i as f32 * 7.0, 0.0),
                vec2(3.0, 34.0),
                0.0,
                1.2,
                HULL_DARK,
            );
        }
        // The bay: a dark mouth in the front, lit while it builds.
        b.rect(vec2(14.0, 0.0), vec2(10.0, 26.0), 0.0, 2.0, JOINT);
        if made > 0.0 {
            b.rect(
                vec2(14.0, 0.0),
                vec2(8.0, 22.0),
                0.0,
                2.0,
                EMBER.mix(SPARK, made).glowing(1.0 + 1.6 * made),
            );
        }
        // The arms over the bay, folded — thrown wide as a build leaves.
        for side in [-1.0f32, 1.0] {
            let spread = 0.25 + 0.9 * made;
            let root = vec2(8.0, 15.0 * side);
            let dir = Vec2::from_angle(-side * spread);
            b.rect(
                root + dir * 6.0,
                vec2(12.0, 4.0),
                -side * spread,
                1.5,
                STEEL,
            );
            b.ellipse(root, vec2(6.0, 6.0), 0.0, JOINT);
            b.ellipse(root + dir * 12.0, vec2(4.0, 4.0), 0.0, PLATE);
        }
        let lamp = if self.heart.emitters > 0 || made > 0.0 {
            SENSOR.glowing(1.05)
        } else {
            SENSOR
        };
        b.ellipse(vec2(-18.0, -16.0), vec2(4.0, 4.0), 0.0, lamp);
    }

    /// A fabricator's wreck: the block caved in and split, the arms off,
    /// the bay dark.
    pub(super) fn draw_fabricator_wreck(&self, b: &mut Brush) {
        self.draw_wreck_ground(b, 30.0);
        b.rect(vec2(-6.0, -8.0), vec2(34.0, 22.0), 0.1, 5.0, HULL_DARK);
        b.rect(vec2(-2.0, 11.0), vec2(30.0, 18.0), -0.18, 5.0, HULL);
        self.draw_rift(b, vec2(-3.0, 1.0), vec2(28.0, 7.0), 0.0);
        b.rect(vec2(13.0, 0.0), vec2(9.0, 24.0), 0.05, 2.0, JOINT);
        for i in 0..2u32 {
            let a = self.scatter(780 + i) * TAU;
            let at = Vec2::from_angle(a) * (26.0 + 8.0 * self.scatter(790 + i));
            b.rect(at, vec2(14.0, 4.0), a, 1.5, STEEL);
        }
    }

    /// Smoke off a fresh wreck of the Heart's, as a Guardian's smokes.
    fn draw_smoke(&self, list: &mut DrawList) {
        if self.wreck_age >= SMOKE_LIFE {
            return;
        }
        let u = self.wreck_age / SMOKE_LIFE;
        for i in 0..SMOKE_PUFFS {
            let born = self.scatter(400 + i) * 0.5;
            let age = (u - born) / (1.0 - born);
            if age <= 0.0 {
                continue;
            }
            let lean = Vec2::from_angle(self.scatter(420 + i) * TAU) * 12.0;
            let at = self.pos + lean * age + vec2(0.0, -52.0 * age);
            let size = 14.0 + 32.0 * age;
            list.circle(at, size, SMOKE.alpha(0.45 * (1.0 - age)));
        }
    }
}
