//! An enemy down, flown into the count (October 2026): wherever one of
//! the enemies the count holds falls — a machine of the wave destroyed, a
//! Manufacturer downed, on the screen or off it — a red dot leaves it
//! trailing sparks and flies into the big red count of enemies standing
//! at the top of the screen, which goes down by one as it lands —
//! brightening and shaking a little — a second after the enemy fell.
//! Picture only, like the payout's coins: the world's count has already
//! gone down, the top frame shows it plus the dots still in the air.

use bevy_egui::egui;
use world::World;

/// How long a dot is in the air, from the enemy to the count.
pub const FLIGHT_SECONDS: f64 = 1.0;
/// How long the count stays lit and shaking after a dot lands.
const PULSE_SECONDS: f64 = 0.3;
/// How far the count shakes at its most, in points.
const SHAKE: f32 = 2.0;

/// The dots in the air and what the count has to know of them.
#[derive(Default)]
pub struct KillDots {
    dots: Vec<Dot>,
    /// The enemies standing last frame, and whose room they stood in.
    standing: Vec<Foe>,
    station: Option<u32>,
    /// When the last dot landed, on egui's clock.
    landed: Option<f64>,
    /// Where the count stood on the screen this frame, if it did.
    target: Option<egui::Rect>,
}

/// One enemy the count holds (`World::droids_standing`), by what stays
/// put when a wave grows the residents' room: a Manufacturer by its Bim,
/// a machine by its place among the machines — whose body index moves on
/// whenever a Bim is added before them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Foe {
    Person(u32),
    Machine(u32),
}

struct Dot {
    foe: Foe,
    /// Its body in the residents' room when it fell, for where it lies.
    who: u32,
    born: f64,
    /// Where it left from on the screen, fixed the first frame it is
    /// painted; `None` until then.
    from: Option<egui::Pos2>,
    /// Its own swing, so two dots off one burst do not fly as one.
    seed: f32,
}

/// The enemies standing in the residents' room, as `droids_standing`
/// counts them: the machines not destroyed bar the Heart's own
/// structures, and the Manufacturers alive and on their feet.
fn foes_standing(world: &World) -> Vec<Foe> {
    let Some(residents) = &world.residents else {
        return Vec::new();
    };
    let room = &residents.aboard.room;
    let people = (0..room.crew_count())
        .filter(|&who| {
            let who = who as usize;
            room.is_manufacturer(who) && room.is_alive(who) && !room.is_downed(who)
        })
        .map(Foe::Person);
    let machines = room
        .droids()
        .iter()
        .enumerate()
        .filter(|(_, d)| !d.destroyed && !d.kind.is_structure())
        .map(|(i, _)| Foe::Machine(i as u32));
    people.chain(machines).collect()
}

impl KillDots {
    /// Once a frame, after the world has stepped: every enemy standing
    /// last frame and fallen since — still in the room, so not a room
    /// laid out anew — sets off from its body; one standing again (a
    /// guest's world rolled back) takes its dot back, so the count never
    /// holds it twice; and the dots that have arrived are taken off. Out
    /// of a mission, or in another site's room, nothing flies.
    pub fn follow(&mut self, world: &World, now: f64) {
        let station = world.residents.as_ref().map(|r| r.station);
        let standing = foes_standing(world);
        if !world.in_mission() || station != self.station {
            *self = KillDots {
                standing,
                station,
                ..KillDots::default()
            };
            return;
        }
        if let Some(residents) = &world.residents {
            let room = &residents.aboard.room;
            let bims = room.crew_count();
            for (k, &foe) in self
                .standing
                .iter()
                .filter(|foe| !standing.contains(foe))
                .enumerate()
            {
                let who = match foe {
                    Foe::Person(who) if who < bims => who,
                    Foe::Machine(i) if i < room.droid_count() => bims + i,
                    _ => continue,
                };
                self.dots.push(Dot {
                    foe,
                    who,
                    born: now,
                    from: None,
                    seed: (who as f32 * 2.399 + k as f32 * 1.713).sin(),
                });
            }
        }
        self.dots.retain(|d| !standing.contains(&d.foe));
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
