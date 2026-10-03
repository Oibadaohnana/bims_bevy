//! The Machine Heart in the room (feature 108): the core, its conduits and
//! its fabricators as the deck steps them. A child of `game`, so it reaches
//! the room's fields the way `tick_droids` does.
//!
//! **The room decides nothing about the fight's phases.** Whether the core
//! is sealed, how many beams it sweeps, how hard they hit and how fast
//! they turn is the world's (`world::heart`), said on each machine's
//! [`HeartState`] every step; what the room does is fire what it has been
//! told it may, at whom it sees. A conduit and a fabricator stand and are
//! shot at, and that is all — a fabricator's machines are built by the
//! world, which knows when one is due.
//!
//! **The core's beams are the Guardian's Sweeper** — the same wind-up, the
//! same arc, the same `Shot` across the seam and the same `Sweep` laid
//! where its targets stand — with the damage multiplied and the sweep
//! sped up by what the world says. It has no heading to turn: it is round,
//! and each of its two emitters looks straight at what it picks.

use super::Game;
use crate::balance;
use crate::droid::{Beam, DroidKind, HeartState};
use crate::math::Vec2;

impl Game {
    /// What the world tells a Machine Heart's machine, by **droid** index
    /// (feature 108): the core sealed or not, its emitters, its damage and
    /// its pace; a fabricator's build flaring. `None` past the end.
    pub fn heart_state_mut(&mut self, i: usize) -> Option<&mut HeartState> {
        self.droids.get_mut(i).map(|d| &mut d.heart)
    }

    /// Every machine of a wave off the deck, the Machine Heart's own kept
    /// where they stand, wrecks and all (feature 108): a fresh wave is a
    /// fresh deck, but a conduit shot down stays down. The Heart's are
    /// laid first, so they keep their places at the front of the list and
    /// every body index before them; how many were kept is the answer.
    pub fn clear_wave_droids(&mut self) -> usize {
        self.droids.retain(|d| d.kind.is_structure());
        self.droids.len()
    }

    /// One step of a Machine Heart's machine. A conduit keeps its link to
    /// the core for the picture; a fabricator's last build fades; the
    /// core runs each of its emitters it has been given.
    pub(super) fn tick_structure(&mut self, i: usize, dt: f32, war: bool) {
        let core = self
            .droids
            .iter()
            .find(|d| d.kind == DroidKind::Core && !d.destroyed)
            .map(|d| (d.pos, d.heart.sealed));
        {
            let d = &mut self.droids[i];
            d.trigger.hold();
            d.locked = None;
            d.blow = None;
            d.peek = None;
            d.heart.made = (d.heart.made - dt).max(0.0);
            if d.kind == DroidKind::Conduit {
                // The line the picture draws to the core it feeds, while
                // there is a sealed one to feed.
                d.heart.link = core.filter(|&(_, sealed)| sealed).map(|(at, _)| at);
            }
            if d.kind != DroidKind::Core {
                return;
            }
        }
        for emitter in 0..HeartState::EMITTERS {
            self.tick_emitter(i, emitter, dt, war);
        }
    }

    /// One of a core's emitters (feature 108): the Guardian's rhythm — a
    /// wind-up on a target fixed, a sweep, a rest — without the turn,
    /// since a core faces every way at once. An emitter past the number
    /// the world allows lets a sweep it has in the air run out and starts
    /// no other; so does every emitter with nothing to fight.
    fn tick_emitter(&mut self, i: usize, emitter: usize, dt: f32, war: bool) {
        let (allowed, pace) = {
            let h = &self.droids[i].heart;
            (emitter < h.emitters as usize, h.pace.max(0.01))
        };
        let beam = self.droids[i].heart.beams[emitter];
        let next = match beam {
            Beam::Cooling { left } => {
                if left - dt <= 0.0 {
                    Beam::Ready
                } else {
                    Beam::Cooling { left: left - dt }
                }
            }
            // A sweep in the air runs to its end whatever happens, at the
            // pace it was let go at.
            Beam::Sweep { left, aim, side } => {
                if left - dt * pace > 0.0 {
                    Beam::Sweep {
                        left: left - dt * pace,
                        aim,
                        side,
                    }
                } else {
                    Beam::Cooling {
                        left: balance::SWEEPER_COOLDOWN,
                    }
                }
            }
            Beam::WindUp {
                left,
                aim,
                at,
                mark,
            } => {
                if !war || !allowed {
                    Beam::Ready
                } else if left - dt > 0.0 {
                    Beam::WindUp {
                        left: left - dt,
                        aim,
                        at,
                        mark,
                    }
                } else {
                    self.let_the_core_go(i, emitter, aim);
                    return;
                }
            }
            Beam::Ready => {
                if !war || !allowed {
                    Beam::Ready
                } else {
                    match self.core_pick(i, emitter) {
                        Some((mark, at)) => {
                            let from = self.droids[i].pos;
                            let aim = (at - from).normalize_or_zero();
                            if aim == Vec2::ZERO {
                                Beam::Ready
                            } else {
                                Beam::WindUp {
                                    left: balance::SWEEPER_WINDUP,
                                    aim,
                                    at,
                                    mark,
                                }
                            }
                        }
                        None => Beam::Ready,
                    }
                }
            }
        };
        self.droids[i].heart.beams[emitter] = next;
    }

    /// Who an emitter of the core winds up on: the **nearest body it can
    /// see** from where the core stands, within the beam's reach — a
    /// player's Bim or a bot alike, whatever the machines' list holds —
    /// and, for the second emitter, somebody the first is not already on
    /// when there is anybody else to pick. The index into the machines'
    /// list, and where that body stands.
    fn core_pick(&self, i: usize, emitter: usize) -> Option<(usize, Vec2)> {
        let d = &self.droids[i];
        let from = d.pos;
        let reach = d.stats().reach();
        let taken: Vec<usize> = (0..HeartState::EMITTERS)
            .filter(|&e| e != emitter)
            .filter_map(|e| match d.heart.beams[e] {
                Beam::WindUp { mark, .. } => Some(mark),
                _ => None,
            })
            .collect();
        let mut seen: Vec<(f32, usize, Vec2)> = self
            .combat
            .machine_targets()
            .iter()
            .enumerate()
            .filter_map(|(mark, t)| {
                let t = (*t)?;
                if t.stale {
                    return None;
                }
                let d = (t.at - from).len();
                if d > reach {
                    return None;
                }
                self.room
                    .sight
                    .sees_from(from, t.at)
                    .map(|_| (d, mark, t.at))
            })
            .collect();
        // Nearest first, and the lower index among equals, so every client
        // picks alike.
        seen.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        seen.iter()
            .find(|(_, mark, _)| !taken.contains(mark))
            .or_else(|| seen.first())
            .map(|&(_, mark, at)| (mark, at))
    }

    /// A core's emitter lets its beam go (feature 108): out of the core's
    /// rim the way it was aimed, swept from alternate sides as a
    /// Guardian's is, at the Sweeper's damage times the world's factor and
    /// at the world's pace — [`Game::lay_beam`], the Guardian's own.
    fn let_the_core_go(&mut self, i: usize, emitter: usize, aim: Vec2) {
        let d = &mut self.droids[i];
        let side = if d.heart.sweeps % 2 == 0 { 1.0 } else { -1.0 };
        d.heart.sweeps = d.heart.sweeps.wrapping_add(1);
        d.fired();
        let stats = d.stats();
        let weapon = d.weapon;
        let from = d.pos + aim * HeartState::LENS_OUT;
        let damage = stats.damage * d.heart.damage.max(0.0);
        let pace = d.heart.pace.max(0.01);
        d.heart.beams[emitter] = Beam::Sweep {
            left: balance::SWEEPER_SWEEP,
            aim,
            side,
        };
        self.reveal(self.bims.len() + i);
        self.lay_beam(from, aim, side, weapon, stats.reach(), damage, pace);
        self.combat.sign_last_shot(self.bims.len() + i);
    }
}

#[cfg(test)]
mod tests {
    use crate::combat::Tier;
    use crate::draw::DrawList;
    use crate::droid::{Droid, DroidKind, DroidPart};
    use crate::math::vec2;

    fn built(kind: DroidKind) -> Droid {
        Droid::structure(kind, Tier::Three, 100.0, vec2(0.0, 0.0), vec2(1.0, 0.0), 7)
    }

    /// **A Heart's machine is one health**: a hit on any part is taken off
    /// the Chassis, and only the Chassis at nothing destroys it.
    #[test]
    fn a_heart_machine_is_one_health_whatever_part_is_hit() {
        for kind in DroidKind::HEART {
            let mut d = built(kind);
            for part in DroidPart::ALL {
                assert_eq!(d.strike(part, 20.0), Some(DroidPart::Chassis));
            }
            assert_eq!(d.body.health(DroidPart::Chassis), 20.0);
            assert!(!d.destroyed, "{kind:?} stands on its last twenty");
            d.strike(DroidPart::Head, 20.0);
            assert!(d.destroyed, "{kind:?} is down at nothing");
        }
    }

    /// **A sealed core takes nothing**, whatever strikes it — a grenade's
    /// burst asks no shield — and its shield is the shell all round.
    #[test]
    fn a_sealed_core_takes_nothing_and_is_shelled_all_round() {
        let mut core = built(DroidKind::Core);
        core.heart.sealed = true;
        assert_eq!(core.strike(DroidPart::Chassis, 1e9), None);
        assert_eq!(core.body.health(DroidPart::Chassis), 100.0);
        assert_eq!(core.shield(), Some(crate::math::Vec2::ZERO));
        assert!(crate::combat::shield_stops(
            crate::math::Vec2::ZERO,
            vec2(0.0, -1.0)
        ));
        core.heart.sealed = false;
        assert_eq!(core.shield(), None, "a core has no plate of its own");
        assert!(core.strike(DroidPart::Chassis, 1e9).is_some());
        assert!(core.destroyed);
    }

    /// **A Stun Shot stuns the Heart's machines too** (October 2026): each
    /// of the three takes it, a core's sweep in the air goes to its
    /// cooldown, and a sealed core stays sealed. A wreck takes none.
    #[test]
    fn a_heart_machine_is_stunned_and_a_core_s_beams_cool() {
        for kind in DroidKind::HEART {
            let mut d = built(kind);
            assert!(d.stun(3.0, false), "{kind:?}");
            assert!(d.is_stunned());
        }
        let mut core = built(DroidKind::Core);
        core.heart.sealed = true;
        core.heart.beams[0] = crate::droid::Beam::Sweep {
            left: 0.5,
            aim: vec2(1.0, 0.0),
            side: 1.0,
        };
        assert!(core.stun(3.0, false));
        assert!(matches!(
            core.heart.beams[0],
            crate::droid::Beam::Cooling { .. }
        ));
        assert_eq!(core.shield(), Some(crate::math::Vec2::ZERO), "sealed");
        let mut wreck = built(DroidKind::Conduit);
        wreck.strike(DroidPart::Chassis, 1e9);
        assert!(!wreck.stun(3.0, false));
    }

    /// Each of the three is drawn standing and as a wreck, and the two are
    /// different pictures.
    #[test]
    fn each_heart_machine_has_a_picture_and_a_wreck() {
        for kind in DroidKind::HEART {
            let mut d = built(kind);
            d.heart.sealed = kind == DroidKind::Core;
            d.heart.link = (kind == DroidKind::Conduit).then_some(vec2(200.0, 0.0));
            let mut standing = DrawList::new();
            d.draw(&mut standing);
            d.destroy();
            let mut wreck = DrawList::new();
            d.draw(&mut wreck);
            assert!(standing.len() > 0 && wreck.len() > 0, "{kind:?}");
            assert_ne!(standing.data(), wreck.data(), "{kind:?}");
        }
    }
}
