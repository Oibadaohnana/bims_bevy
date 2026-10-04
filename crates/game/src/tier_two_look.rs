//! The tier-two machines' pictures (task 157): the Bomber, the Lancer and
//! the Conductor, their wrecks, and what each shows of what it is about to
//! do. A child of `droid`, so the pictures reach the machine's own timers
//! and its palette.
//!
//! **The Bomber** is squat and wide: a low hull on two stubby legs, a rack
//! of three bombs on its back with their lights blinking, a domed head
//! with a visor slit, a pistol in its right fist and a long throwing arm
//! on the left that swings forward as it rolls one. **The Lancer** is thin
//! and narrow-shouldered on two long legs, a tall head with one scope lens
//! and its right arm a rail out ahead of it, two bars with coils between
//! them that fill with light as it charges. **The Conductor** is broad and
//! crowned: a six-sided chassis with shoulder plates, a ring emitter on
//! its back from which the tethers run, three vanes standing over it and a
//! rifle along its right side.
//!
//! What a machine is about to do is drawn **over the fog**
//! ([`Droid::draw_telegraph`], from `Game::render` after the shots): the
//! Lancer's line from its rail to where the slug will stop, thin while it
//! follows its mark and bright once it locks, and the Conductor's mark on
//! the body it has picked — a ring closing on it through the warning, held
//! and turning while the machines fire at it. A telegraph is seen whether
//! or not the machine is. The Conductor's tethers to the machines it links
//! and its blink are part of its own picture, under the fog. What glows is
//! past white and nothing else (feature 97's bloom); every flicker is the
//! machine's own clock or [`Droid::scatter`], never a roll.

use super::*;

/// The Bomber's bombs on its back, unlit and lit.
const BOMB_CASE: Color = Color::rgb(0.13, 0.14, 0.15);
/// The Conductor's crown: its vanes and the ring on its back.
const CROWN: Color = Color::rgb(0.40, 0.42, 0.47);
/// How long a blink's after-image lasts, in seconds.
pub(super) const BLINK_FADE: f32 = 0.6;

impl Droid {
    /// How far through its charge a Lancer is, nought to one, and whether
    /// it has locked: `None` while it charges nothing.
    fn rail_charge(&self) -> Option<(f32, bool)> {
        match self.rhythm.rail {
            Rail::Charging { left, .. } => {
                let whole = balance::LANCER_TRACK + balance::LANCER_LOCK;
                Some((
                    clamp(1.0 - left / whole, 0.0, 1.0),
                    self.rhythm.rail.locked(),
                ))
            }
            _ => None,
        }
    }

    /// **The Bomber** (task 157).
    pub(super) fn draw_bomber(&self, b: &mut Brush) {
        let step = self.step();
        let legs_gone = self.body.gone(DroidPart::Legs);
        let arms_gone = self.body.gone(DroidPart::Arms);
        let slump = if legs_gone { 0.9 } else { 1.0 };
        // Two stubby legs, set wide.
        for side in [-1.0f32, 1.0] {
            if legs_gone {
                b.rect(vec2(-4.0, 10.0 * side), vec2(10.0, 7.0), 0.0, 2.5, JOINT);
            } else {
                let swing = step * 5.0 * side;
                b.rect(
                    vec2(swing - 1.0, 11.0 * side),
                    vec2(13.0, 8.5),
                    0.0,
                    3.0,
                    HULL_DARK,
                );
                b.rect(
                    vec2(swing + 5.0, 11.5 * side),
                    vec2(7.0, 6.0),
                    0.0,
                    2.0,
                    JOINT,
                );
            }
        }
        // The hull: low and round-shouldered, wider than deep.
        b.rect(
            vec2(-1.0, 0.0),
            vec2(28.0, 36.0) * slump,
            0.0,
            7.0,
            HULL_DARK,
        );
        b.rect(vec2(0.0, 0.0), vec2(23.0, 31.0) * slump, 0.0, 6.0, HULL);
        b.rect(vec2(3.0, 0.0), vec2(7.0, 22.0), 0.0, 2.0, PLATE);
        // The rack on its back: three bombs in a row, each with a light
        // that blinks in its turn — one gone from the rack for a while
        // after it has rolled one.
        b.rect(vec2(-12.0, 0.0), vec2(10.0, 30.0), 0.0, 2.5, JOINT);
        let spent = self.rhythm.bomb_wait > balance::BOMB_COOLDOWN - 2.0;
        for (k, across) in [-9.5f32, 0.0, 9.5].into_iter().enumerate() {
            if spent && k == 1 {
                b.ellipse(vec2(-12.0, across), vec2(5.0, 5.0), 0.0, JOINT);
                continue;
            }
            b.ellipse(vec2(-12.0, across), vec2(10.0, 10.0), 0.0, BOMB_CASE);
            let on = ((self.idle * 2.0 + k as f32 * 0.33).fract()) < 0.35;
            let light = if on && !self.destroyed {
                SENSOR.glowing(1.1)
            } else {
                SENSOR_OUT
            };
            b.ellipse(vec2(-12.0, across), vec2(3.6, 3.6), 0.0, light);
        }
        // The head: a dome forward on the hull with a wide visor slit.
        let turn = self.sway() * 0.05;
        b.ellipse(vec2(10.0, 0.0), vec2(14.0, 16.0), turn, HULL_DARK);
        b.ellipse(vec2(10.5, 0.0), vec2(10.5, 12.5), turn, PLATE);
        b.rect(vec2(14.0, 0.0), vec2(2.4, 10.0), turn, 1.0, self.eye());
        // The pistol in its right fist.
        let droop = if arms_gone { 1.0 } else { 0.0 };
        let shoulder = vec2(3.0, 15.0);
        let out = vec2(21.0 - droop * 8.0, 12.0 + droop * 6.0);
        b.rect(
            (shoulder + out) * 0.5 - vec2(2.0, 0.0),
            vec2(15.0 - droop * 4.0, 6.0),
            droop * 0.7,
            2.0,
            if arms_gone { JOINT } else { STEEL },
        );
        b.rect(out - vec2(1.0, 0.0), vec2(6.0, 4.6), 0.0, 1.2, HULL_DARK);
        if !arms_gone && self.flash > 0.0 {
            let t = self.flash / FLASH_TIME;
            b.ellipse(
                out + vec2(4.0, 0.0),
                vec2(10.0 * t, 7.0 * t),
                0.0,
                SENSOR.alpha(0.55 * t),
            );
            b.ellipse(
                out + vec2(3.5, 0.0),
                vec2(5.0 * t, 4.0 * t),
                0.0,
                SPARK.alpha(0.9 * t),
            );
        }
        // The throwing arm on the left: swung forward the instant it rolls
        // a bomb, held back over the rack otherwise.
        let swing = clamp(self.snap / SNAP_TIME, 0.0, 1.0);
        let reach = if arms_gone { -0.9 } else { -1.1 + swing * 1.6 };
        let root = vec2(-2.0, -15.0);
        let hand = root + Vec2::from_angle(reach) * 15.0;
        b.rect(
            (root + hand) * 0.5,
            vec2(15.0, 5.6),
            reach,
            2.0,
            if arms_gone { JOINT } else { HULL_DARK },
        );
        b.ellipse(
            hand,
            vec2(6.5, 6.5),
            0.0,
            if arms_gone { JOINT } else { STEEL },
        );
    }

    /// **A Bomber's wreck**: the hull split, the rack torn off its back
    /// with the bombs lying dead round it, the head knocked aside.
    pub(super) fn draw_bomber_wreck(&self, b: &mut Brush) {
        self.draw_wreck_ground(b, 26.0);
        for i in 0..3u32 {
            let a = self.scatter(i + 90) * TAU;
            let at = vec2(-14.0, 0.0) + Vec2::from_angle(a) * (12.0 + 10.0 * self.scatter(i + 93));
            b.ellipse(at, vec2(10.0, 10.0), 0.0, BOMB_CASE);
            b.ellipse(at, vec2(3.4, 3.4), 0.0, SENSOR_OUT);
        }
        b.rect(vec2(-16.0, 4.0), vec2(9.0, 26.0), 0.5, 2.0, JOINT);
        for side in [-1.0f32, 1.0] {
            let at = vec2(0.0, 7.0 * side);
            b.rect(at, vec2(26.0, 16.0), 0.14 * side, 6.0, HULL_DARK);
            b.rect(
                at + vec2(0.5, 0.0),
                vec2(21.0, 11.5),
                0.14 * side,
                5.0,
                HULL,
            );
        }
        self.draw_rift(b, vec2(0.0, 0.0), vec2(20.0, 7.0), 0.0);
        let ha = self.scatter(96) * TAU;
        let head = vec2(17.0, -9.0) + Vec2::from_angle(ha) * 4.0;
        b.ellipse(head, vec2(13.0, 15.0), ha, HULL_DARK);
        b.ellipse(head, vec2(9.5, 11.5), ha, PLATE);
        b.rect(
            head + Vec2::from_angle(ha) * 3.5,
            vec2(2.2, 9.0),
            ha,
            1.0,
            self.eye(),
        );
        b.rect(vec2(10.0, 19.0), vec2(14.0, 5.6), -0.7, 2.0, STEEL);
    }

    /// **The Lancer** (task 157).
    pub(super) fn draw_lancer(&self, b: &mut Brush) {
        let step = self.step();
        let legs_gone = self.body.gone(DroidPart::Legs);
        let arms_gone = self.body.gone(DroidPart::Arms);
        // Two long thin legs with a long stride.
        for side in [-1.0f32, 1.0] {
            if legs_gone {
                b.rect(vec2(-3.0, 6.0 * side), vec2(9.0, 4.0), 0.0, 1.8, JOINT);
            } else {
                let swing = step * 9.0 * side;
                b.rect(
                    vec2(swing, 6.5 * side),
                    vec2(14.0, 4.6),
                    0.0,
                    2.0,
                    HULL_DARK,
                );
                b.rect(
                    vec2(swing + 7.0, 6.5 * side),
                    vec2(6.0, 4.0),
                    0.0,
                    1.5,
                    JOINT,
                );
            }
        }
        // The chassis: narrow, a spine down its back.
        b.rect(Vec2::ZERO, vec2(16.0, 24.0), 0.0, 3.0, HULL_DARK);
        b.rect(vec2(0.5, 0.0), vec2(12.5, 19.5), 0.0, 2.5, HULL);
        b.rect(vec2(-5.0, 0.0), vec2(4.0, 18.0), 0.0, 1.0, PLATE);
        // The head: tall and narrow, with one scope lens that burns while
        // it charges.
        let turn = self.sway() * 0.05;
        let charge = self.rail_charge();
        b.rect(vec2(7.0, 0.0), vec2(9.0, 12.0), turn, 1.5, HULL_DARK);
        b.rect(vec2(7.5, 0.0), vec2(6.0, 9.0), turn, 1.0, PLATE);
        let lens = match charge {
            Some((t, locked)) if !self.destroyed => {
                SENSOR.glowing(1.0 + 0.6 * t + if locked { 0.4 } else { 0.0 })
            }
            _ => self.eye(),
        };
        b.ellipse(vec2(10.5, 0.0), vec2(5.0, 5.0), 0.0, JOINT);
        b.ellipse(vec2(10.5, 0.0), vec2(3.4, 3.4), 0.0, lens);
        // The rail: two bars out ahead along the right side, coils across
        // them, filling with light from the root as it charges.
        let droop = if arms_gone { 1.0 } else { 0.0 };
        let root = vec2(2.0, 9.0);
        let tip = vec2(34.0 - droop * 12.0, 7.0 + droop * 8.0);
        let along = (tip - root).angle();
        let mid = (root + tip) * 0.5;
        let length = (tip - root).len();
        let bar = if arms_gone { JOINT } else { STEEL };
        b.rect(mid, vec2(length, 6.0), along, 1.5, HULL_DARK);
        for across in [-1.0f32, 1.0] {
            let off = Vec2::from_angle(along).perp() * (2.2 * across);
            b.rect(mid + off, vec2(length, 1.8), along, 0.8, bar);
        }
        for k in 0..4 {
            let at = root + (tip - root) * (0.25 + 0.2 * k as f32);
            let lit = charge.map_or(0.0, |(t, _)| clamp(t * 4.0 - k as f32, 0.0, 1.0));
            let coil = if lit > 0.0 {
                SENSOR.glowing(0.8 + 0.7 * lit)
            } else {
                JOINT
            };
            b.rect(at, vec2(2.0, 7.0), along, 0.6, coil);
        }
        if self.flash > 0.0 && !arms_gone {
            let t = self.flash / FLASH_TIME;
            b.ellipse(
                tip + vec2(4.0, 0.0),
                vec2(16.0 * t, 9.0 * t),
                0.0,
                SENSOR.alpha(0.6 * t),
            );
            b.ellipse(
                tip + vec2(3.0, 0.0),
                vec2(7.0 * t, 5.0 * t),
                0.0,
                SPARK.glowing(1.4).alpha(t),
            );
        }
        // The off arm, a thin brace.
        b.rect(
            vec2(3.0, -9.0),
            vec2(10.0, 3.6),
            0.3 + droop * 0.5,
            1.5,
            HULL_DARK,
        );
    }

    /// **A Lancer's wreck**: folded over, the rail snapped in two with
    /// its coils dark, the head off.
    pub(super) fn draw_lancer_wreck(&self, b: &mut Brush) {
        self.draw_wreck_ground(b, 22.0);
        b.rect(vec2(0.0, 0.0), vec2(15.0, 23.0), 0.2, 3.0, HULL_DARK);
        b.rect(vec2(0.5, 0.0), vec2(11.5, 18.0), 0.2, 2.5, HULL);
        self.draw_rift(b, vec2(0.0, 1.0), vec2(9.0, 12.0), 0.2);
        for (k, at) in [vec2(12.0, 14.0), vec2(26.0, 2.0)].into_iter().enumerate() {
            let a = 0.3 + self.scatter(100 + k as u32) * 1.2 - k as f32 * 0.9;
            b.rect(at, vec2(16.0, 5.0), a, 1.5, HULL_DARK);
            b.rect(at, vec2(15.0, 1.6), a, 0.8, STEEL);
        }
        let ha = self.scatter(104) * TAU;
        let head = vec2(-16.0, -10.0) + Vec2::from_angle(ha) * 4.0;
        b.rect(head, vec2(9.0, 12.0), ha, 1.5, HULL_DARK);
        b.ellipse(head, vec2(3.4, 3.4), 0.0, SENSOR_OUT);
        for side in [-1.0f32, 1.0] {
            b.rect(
                vec2(-14.0, 9.0 * side),
                vec2(13.0, 4.2),
                0.4 * side,
                1.8,
                JOINT,
            );
        }
    }

    /// **The Conductor** (task 157).
    pub(super) fn draw_conductor(&self, b: &mut Brush) {
        let step = self.step();
        let legs_gone = self.body.gone(DroidPart::Legs);
        let arms_gone = self.body.gone(DroidPart::Arms);
        let blinking = self.rhythm.blink.is_some();
        // Two heavy legs.
        for side in [-1.0f32, 1.0] {
            if legs_gone {
                b.rect(vec2(-5.0, 11.0 * side), vec2(12.0, 7.0), 0.0, 2.5, JOINT);
            } else {
                let swing = step * 5.0 * side;
                b.rect(
                    vec2(swing - 2.0, 12.0 * side),
                    vec2(14.0, 8.5),
                    0.0,
                    3.0,
                    HULL_DARK,
                );
                b.rect(
                    vec2(swing + 5.5, 12.5 * side),
                    vec2(9.0, 6.0),
                    0.0,
                    2.0,
                    JOINT,
                );
            }
        }
        // The chassis: six-sided — a rect with its corners cut by two more
        // laid across it — with shoulder plates.
        b.rect(Vec2::ZERO, vec2(30.0, 38.0), 0.0, 5.0, HULL_DARK);
        b.rect(vec2(0.5, 0.0), vec2(25.0, 33.0), 0.0, 4.0, HULL);
        for side in [-1.0f32, 1.0] {
            b.rect(
                vec2(3.0, 19.0 * side),
                vec2(18.0, 9.0),
                -0.15 * side,
                3.0,
                PLATE,
            );
        }
        // The ring emitter on its back, and the three vanes over it: the
        // ring pulses, and burns while it marks or blinks.
        let pulse = 0.5 + 0.5 * (self.idle * 3.0).sin();
        let marking = self.rhythm.mark.is_some_and(|m| !m.holding());
        let ring = if self.destroyed {
            SENSOR_OUT
        } else if blinking || marking {
            SENSOR.glowing(1.5 + 0.3 * pulse)
        } else {
            SENSOR_OUT.mix(SENSOR, 0.45 + 0.4 * pulse)
        };
        b.ellipse(vec2(-9.0, 0.0), vec2(16.0, 16.0), 0.0, JOINT);
        b.ellipse(vec2(-9.0, 0.0), vec2(12.0, 12.0), 0.0, ring);
        b.ellipse(vec2(-9.0, 0.0), vec2(6.0, 6.0), 0.0, HULL_DARK);
        for across in [-9.0f32, 0.0, 9.0] {
            b.rect(vec2(-15.0, across), vec2(12.0, 3.0), 0.0, 1.0, CROWN);
        }
        // The head: a wide band with a lit sensor strip.
        let turn = self.sway() * 0.04;
        b.rect(vec2(10.0, 0.0), vec2(10.0, 18.0), turn, 2.0, JOINT);
        b.rect(vec2(11.5, 0.0), vec2(3.5, 14.0), turn, 1.0, self.eye());
        // The rifle along its right side.
        let droop = if arms_gone { 1.0 } else { 0.0 };
        let root = vec2(-2.0, 16.0);
        let muzzle = vec2(30.0 - droop * 10.0, 14.0 + droop * 7.0);
        let along = (muzzle - root).angle();
        b.rect(
            (root + muzzle) * 0.5,
            vec2((muzzle - root).len(), 6.0),
            along,
            2.0,
            if arms_gone { JOINT } else { STEEL },
        );
        b.rect(
            muzzle - Vec2::from_angle(along) * 3.0,
            vec2(6.0, 7.0),
            along,
            1.5,
            HULL_DARK,
        );
        if self.flash > 0.0 && !arms_gone {
            let t = self.flash / FLASH_TIME;
            b.ellipse(
                muzzle + vec2(4.0, 0.0),
                vec2(12.0 * t, 8.0 * t),
                0.0,
                SENSOR.alpha(0.55 * t),
            );
            b.ellipse(
                muzzle + vec2(3.0, 0.0),
                vec2(6.0 * t, 4.5 * t),
                0.0,
                SPARK.alpha(0.9 * t),
            );
        }
        b.rect(
            vec2(6.0, -17.0),
            vec2(12.0, 5.6),
            0.2 + droop * 0.5,
            2.0,
            if arms_gone { JOINT } else { HULL_DARK },
        );
    }

    /// **A Conductor's wreck**: the ring torn off its back and lying
    /// cracked, the vanes scattered, the chassis split.
    pub(super) fn draw_conductor_wreck(&self, b: &mut Brush) {
        self.draw_wreck_ground(b, 32.0);
        for k in 0..3u32 {
            let a = self.scatter(110 + k) * TAU;
            let at = vec2(-12.0, 0.0) + Vec2::from_angle(a) * (16.0 + 8.0 * self.scatter(113 + k));
            b.rect(at, vec2(12.0, 3.0), a, 1.0, CROWN);
        }
        for side in [-1.0f32, 1.0] {
            let at = vec2(-1.0, 8.0 * side);
            b.rect(at, vec2(29.0, 18.0), 0.12 * side, 5.0, HULL_DARK);
            b.rect(
                at + vec2(0.5, 0.0),
                vec2(24.0, 13.0),
                0.12 * side,
                4.0,
                HULL,
            );
        }
        self.draw_rift(b, vec2(0.0, 0.0), vec2(26.0, 8.0), 0.0);
        let ra = self.scatter(116) * TAU;
        let ring = vec2(-26.0, 6.0) + Vec2::from_angle(ra) * 5.0;
        b.ellipse(ring, vec2(16.0, 15.0), ra, JOINT);
        b.ellipse(ring, vec2(11.0, 10.0), ra, SENSOR_OUT);
        b.ellipse(ring, vec2(5.0, 5.0), ra, HULL_DARK);
        b.rect(vec2(14.0, 18.0), vec2(26.0, 6.0), -0.4, 2.0, STEEL);
    }

    /// What a Conductor draws under itself (task 157): a faint tether to
    /// every machine it links, a pulse running out along each; while it
    /// plants for a blink, a ring closing where it will stand and a line to
    /// it; and for a moment after it has blinked, its after-image fading at
    /// the spot it left.
    pub(super) fn draw_conductor_under(&self, list: &mut DrawList) {
        if self.destroyed {
            return;
        }
        for (k, &to) in self.tethers.iter().enumerate() {
            list.line(self.pos, to, 1.6, SENSOR.alpha(0.22));
            let run = (self.idle * 0.9 + k as f32 * 0.37).fract();
            list.circle(
                self.pos + (to - self.pos) * run,
                6.0,
                SENSOR.glowing(1.2).alpha(0.7),
            );
            list.ring(to, 40.0, 1.5, SENSOR.alpha(0.3));
        }
        if let Some(blink) = self.rhythm.blink {
            let t = clamp(1.0 - blink.left / balance::BLINK_WINDUP, 0.0, 1.0);
            list.line(self.pos, blink.to, 2.0, SENSOR.alpha(0.25 + 0.4 * t));
            list.ring(
                blink.to,
                70.0 * (1.0 - 0.6 * t),
                2.5,
                SENSOR.glowing(1.3).alpha(0.4 + 0.5 * t),
            );
            list.ring(
                self.pos,
                56.0 * (1.0 + t),
                2.0,
                SENSOR.glowing(1.2).alpha(0.6 * (1.0 - t)),
            );
        }
        if let Some((from, age)) = self.rhythm.blinked {
            let t = clamp(age / BLINK_FADE, 0.0, 1.0);
            list.circle(from, 40.0 * (1.0 + t), SENSOR.alpha(0.25 * (1.0 - t)));
            list.ring(
                from,
                50.0 * (1.0 + 1.5 * t),
                2.0,
                SENSOR.glowing(1.3).alpha(0.8 * (1.0 - t)),
            );
            list.line(
                from,
                self.pos,
                2.5 * (1.0 - t),
                SENSOR.glowing(1.2).alpha(0.5 * (1.0 - t)),
            );
        }
    }

    /// What a machine is about to do, drawn **over the fog** whether or not
    /// it is seen (task 157): a Lancer's line and a Conductor's mark. Nothing
    /// for any other kind, or a wreck.
    pub fn draw_telegraph(&self, list: &mut DrawList) {
        if self.destroyed || self.posing && !self.lit {
            return;
        }
        // Held lit for a picture (the probes' rack), a Lancer is charged and
        // locked six tiles ahead and a Conductor holds a mark four ahead.
        let staged = self.lit && matches!(self.rhythm.rail, Rail::Ready | Rail::Cooling { .. });
        let staged_end = self.pos + self.front() * (6.0 * TILE);
        let rail = match self.rhythm.rail {
            Rail::Charging { end, .. } => Some(end),
            _ if staged && self.kind == DroidKind::Lancer => Some(staged_end),
            _ => None,
        };
        let mark = self.rhythm.mark.or_else(|| {
            (self.lit && self.kind == DroidKind::Conductor).then(|| Mark {
                target: 0,
                at: self.pos + self.front() * (4.0 * TILE),
                left: balance::MARK_HOLD,
            })
        });
        if let Some(end) = rail {
            let from = self.muzzle();
            let (t, locked) = self.rail_charge().unwrap_or((1.0, true));
            if locked {
                let flick = 0.5 + 0.5 * (self.idle * 40.0).sin();
                list.line(from, end, 7.0, HOSTILE_GLOW.alpha(0.35));
                list.line(
                    from,
                    end,
                    2.6,
                    SENSOR.glowing(1.4 + 0.4 * flick).alpha(0.95),
                );
                list.circle(end, 12.0, SENSOR.glowing(1.6));
            } else {
                list.line(from, end, 1.4 + 1.2 * t, SENSOR.alpha(0.30 + 0.35 * t));
                list.circle(end, 7.0, SENSOR.alpha(0.6));
            }
        }
        if let Some(mark) = mark {
            let at = mark.at;
            if mark.holding() {
                // Held: a steady ring and four brackets turning slowly.
                let spin = self.idle * 1.5;
                list.ring(at, 64.0, 3.0, SENSOR.glowing(1.3).alpha(0.9));
                for k in 0..4 {
                    let a = spin + k as f32 * core::f32::consts::FRAC_PI_2;
                    let d = Vec2::from_angle(a);
                    list.line(at + d * 38.0, at + d * 50.0, 3.0, SENSOR.glowing(1.5));
                }
            } else {
                // The warning: a ring closing on the body, quicker as it
                // comes.
                let w = clamp(
                    (mark.left - balance::MARK_HOLD) / balance::MARK_WARNING,
                    0.0,
                    1.0,
                );
                let blink = (self.idle * 14.0).sin() > 0.0;
                list.ring(
                    at,
                    64.0 + 120.0 * w,
                    2.5,
                    SENSOR.alpha(if blink { 0.9 } else { 0.5 }),
                );
                list.circle(at, 10.0, SENSOR.glowing(1.2).alpha(0.8));
            }
        }
    }
}

/// The soft red round a locked rail's line.
const HOSTILE_GLOW: Color = Color::rgb(1.0, 0.30, 0.24);
