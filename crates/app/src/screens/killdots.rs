//! A machine down, flown into the count (October 2026): where a machine
//! of the wave falls, a red dot leaves it trailing sparks and flies into
//! the big red count of enemies standing at the top of the screen, which
//! goes down by one as it lands — brightening and shaking a little — a
//! second after the machine fell. Picture only, like the payout's coins:
//! the world's count has already gone down, the top frame shows it plus
//! the dots still in the air.

use bevy_egui::egui;
use world::{World, WorldEvent};

/// How long a dot is in the air, from the machine to the count.
pub const FLIGHT_SECONDS: f64 = 1.0;
/// How long the count stays lit and shaking after a dot lands.
const PULSE_SECONDS: f64 = 0.3;
/// How far the count shakes at its most, in points.
const SHAKE: f32 = 2.0;

/// The dots in the air and what the count has to know of them.
#[derive(Default)]
pub struct KillDots {
    dots: Vec<Dot>,
    /// The machines said down this frame, by body, not yet flying.
    fresh: Vec<u32>,
    /// The machines standing last frame (`World::droids_standing`).
    standing: u32,
    /// When the last dot landed, on egui's clock.
    landed: Option<f64>,
    /// Where the count stood on the screen this frame, if it did.
    target: Option<egui::Rect>,
}

struct Dot {
    who: u32,
    born: f64,
    /// Where it left from on the screen, fixed the first frame it is
    /// painted; `None` until then.
    from: Option<egui::Pos2>,
    /// Its own swing, so two dots off one burst do not fly as one.
    seed: f32,
}

impl KillDots {
    /// One of the frame's events: a machine of a wave down (not one of
    /// the Heart's own, which the count never held).
    pub fn note(&mut self, event: &WorldEvent) {
        if let WorldEvent::DroidDown { who, kind, .. } = *event
            && !bims::droid::DroidKind::from_code(kind).is_some_and(|k| k.is_structure())
        {
            self.fresh.push(who);
        }
    }

    /// Once a frame, after the events are read: the machines said down
    /// set off, as many as the count really went down by — an event said
    /// twice (a guest's world rolled back) never puts one back on it —
    /// and the dots that have arrived are taken off. Out of a mission,
    /// nothing flies.
    pub fn follow(&mut self, world: &World, now: f64) {
        let standing = world.droids_standing();
        if !world.in_mission() {
            *self = KillDots {
                standing,
                ..KillDots::default()
            };
            return;
        }
        let dropped = self.standing.saturating_sub(standing) as usize;
        for (k, who) in self.fresh.drain(..).take(dropped).enumerate() {
            self.dots.push(Dot {
                who,
                born: now,
                from: None,
                seed: (who as f32 * 2.399 + k as f32 * 1.713).sin(),
            });
        }
        self.standing = standing;
        let landed = self
            .dots
            .iter()
            .map(|d| d.born + FLIGHT_SECONDS)
            .filter(|&at| at <= now)
            .fold(None, |last: Option<f64>, at| {
                Some(last.map_or(at, |l| l.max(at)))
            });
        if landed.is_some() {
            self.landed = landed;
        }
        self.dots.retain(|d| now < d.born + FLIGHT_SECONDS);
    }

    /// How many are still in the air: the count shows them as standing.
    pub fn pending(&self) -> u32 {
        self.dots.len() as u32
    }

    /// How lit the count is at `now` — one the moment a dot lands, fading
    /// to nought — and where it is shaken to.
    pub fn pulse(&self, now: f64) -> (f32, egui::Vec2) {
        let Some(landed) = self.landed else {
            return (0.0, egui::Vec2::ZERO);
        };
        let since = now - landed;
        if !(0.0..PULSE_SECONDS).contains(&since) {
            return (0.0, egui::Vec2::ZERO);
        }
        let glow = (1.0 - since / PULSE_SECONDS) as f32;
        let t = since as f32;
        let shake = egui::vec2((t * 95.0).sin(), (t * 71.0 + 1.3).sin() * 0.6) * SHAKE * glow;
        (glow, shake)
    }

    /// Where the count stood this frame, for the dots to fly into; `None`
    /// when it was not up.
    pub fn aim_at(&mut self, count: Option<egui::Rect>) {
        self.target = count;
    }

    /// The dots in the air at `now`, over everything. `fallen` is where a
    /// body of the station's room lies on the screen, asked once a dot,
    /// the first frame it is painted. Slow off the wreck and fast into
    /// the count, swinging out on a curve of its own, a trail of fading
    /// red behind it and a few sparks shed along the way; and a ring on
    /// the count as one lands.
    pub fn paint(
        &mut self,
        ctx: &egui::Context,
        fallen: impl Fn(u32) -> Option<egui::Pos2>,
        now: f64,
    ) {
        let Some(target) = self.target else {
            return;
        };
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::new("kill-dots"),
        ));
        let end = target.center();
        if let Some(landed) = self.landed {
            let since = ((now - landed) / 0.25) as f32;
            if (0.0..1.0).contains(&since) {
                painter.circle_stroke(
                    end,
                    8.0 + 22.0 * since,
                    egui::Stroke::new(2.0 * (1.0 - since), red((1.0 - since) * 0.8)),
                );
            }
        }
        for dot in &mut self.dots {
            if dot.from.is_none() {
                dot.from = fallen(dot.who);
            }
            let Some(start) = dot.from else {
                continue;
            };
            let s = ((now - dot.born) / FLIGHT_SECONDS) as f32;
            if !(0.0..1.0).contains(&s) {
                continue;
            }
            let at = |s: f32| path(start, end, dot.seed, s);
            // The trail: the dot's own way a little back, thinning out.
            const TRAIL: usize = 24;
            for k in (1..=TRAIL).rev() {
                let back = s - k as f32 * 0.009;
                if back < 0.0 {
                    continue;
                }
                let fade = 1.0 - k as f32 / (TRAIL as f32 + 1.0);
                painter.circle_filled(at(back), 1.0 + 3.5 * fade, red(0.5 * fade));
            }
            // The sparks: shed from points behind it, drifting off its way
            // and dying as they go.
            for k in 0..6 {
                let shed = s - 0.05 - k as f32 * 0.045;
                if shed < 0.0 {
                    continue;
                }
                let age = (s - shed) / 0.3;
                if age >= 1.0 {
                    continue;
                }
                let a = (dot.seed * 7.1 + k as f32 * 2.3).sin();
                let b = (dot.seed * 3.7 + k as f32 * 1.9).cos();
                let drift = egui::vec2(a, b + 0.6) * 18.0 * age;
                painter.circle_filled(at(shed) + drift, 2.2 * (1.0 - age), spark(1.0 - age));
            }
            // The dot: a soft red glow round a bright core.
            let p = at(s);
            let r = 6.0 * (1.0 - 0.3 * s.powi(3));
            painter.circle_filled(p, r * 2.2, red(0.25));
            painter.circle_filled(p, r, egui::Color32::from_rgb(0xff, 0x3a, 0x30));
            painter.circle_filled(p, r * 0.45, egui::Color32::from_rgb(0xff, 0xc8, 0xb8));
        }
        if !self.dots.is_empty() {
            ctx.request_repaint();
        }
    }
}

/// Where a dot is `s` of the way from `start` to `end`: a curve swung out
/// to one side by `seed` and up, eased in so it is quick into the count.
fn path(start: egui::Pos2, end: egui::Pos2, seed: f32, s: f32) -> egui::Pos2 {
    let across = end - start;
    let side = egui::vec2(-across.y, across.x).normalized();
    let control = start + across * 0.35 + side * (seed * 140.0) + egui::vec2(0.0, 60.0);
    let e = s * s * (3.0 - 2.0 * s);
    let e = e * e.sqrt();
    start.lerp(control, e).lerp(control.lerp(end, e), e)
}

/// The dot's red at `alpha`.
fn red(alpha: f32) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(0xff, 0x30, 0x28, (alpha.clamp(0.0, 1.0) * 255.0) as u8)
}

/// A spark's orange-red at `alpha`.
fn spark(alpha: f32) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(0xff, 0x7a, 0x40, (alpha.clamp(0.0, 1.0) * 255.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A dot lands where it was aimed, and leaves where the machine fell.
    #[test]
    fn a_dot_flies_from_the_wreck_into_the_count() {
        let (a, b) = (egui::pos2(100.0, 500.0), egui::pos2(640.0, 30.0));
        for seed in [-1.0, 0.0, 0.7] {
            assert!((path(a, b, seed, 0.0) - a).length() < 0.01);
            assert!((path(a, b, seed, 1.0) - b).length() < 0.01);
        }
    }

    /// The count is lit and shaken only a moment after a dot lands.
    #[test]
    fn the_count_pulses_only_after_a_landing() {
        let mut dots = KillDots::default();
        assert_eq!(dots.pulse(5.0).0, 0.0);
        dots.landed = Some(5.0);
        let (glow, shake) = dots.pulse(5.05);
        assert!(glow > 0.5 && shake.length() <= SHAKE * 1.2);
        assert_eq!(dots.pulse(5.01 + PULSE_SECONDS).0, 0.0);
    }
}
