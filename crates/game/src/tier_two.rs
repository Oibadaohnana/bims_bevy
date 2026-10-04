//! The tier-two machines' step (task 157): a child of `game`, reached from
//! `Game::tick_droids`.
//!
//! - **The link and the mark** ([`Game::link_and_mark`], at the top of every
//!   machines' step): every machine within `LINK_RADIUS` of a Conductor on
//!   its feet and not stunned is linked — it takes `LINK_TAKEN` of every
//!   hit (`Game::strike_droid`) — and every Conductor's mark is run down
//!   and kept on its body; the one past its warning is what every machine
//!   that can shoot it aims at ([`Game::machine_aim`]), a taunt reaching the
//!   machine coming first.
//! - **The Bomber's bomb** ([`Game::roll_bomb`]): rolled at the nearest body
//!   of the crew's side it sees within `BOMB_TRIGGER`, up to `BOMB_REACH`
//!   along, stopped short where a wall, low cover or deck nobody could
//!   stand on is in the way ([`Game::bomb_stop`]) — the circle is where it
//!   stops, known as it leaves. Recorded as a `Shot` for the world to lay
//!   in the crew's room (`Combat::shoot_bomb`), where it bursts on the
//!   machines as well; laid here as well, unseen, where the machines' list
//!   has bodies of this room's own on it.
//! - **The Lancer's rail** ([`Game::tick_lancer`], its own branch as the
//!   Guardian has): with its mark in sight from its own eye within
//!   `LANCER_REACH`, it plants and charges — `LANCER_TRACK` following the
//!   mark, `LANCER_LOCK` held — and the slug goes along the line, through
//!   every body on it. A stun or its arms shot away put the charge out.
//! - **The Conductor** ([`Game::conduct`]): the mark every `MARK_EVERY`, the
//!   blink away from a body within `BLINK_NEAR` (planted `BLINK_WINDUP`
//!   first), and once, at `STRIKE_AT` of its health, the strike call — four
//!   bombs' circles round its mark.
//! - **Keeping back** ([`Game::keep_back`]): a Bomber, a Lancer and a
//!   Conductor walk off a body that comes within `DroidKind::shy`.
//!
//! Every turn is by the written-out cosines and sines below, never an angle
//! worked out, so two machines agree to the bit; nothing here draws on any
//! stream.

use super::Game;
use crate::balance;
use crate::combat::{Combat, Target, Weapon, WeaponStats};
use crate::cue::{Cue, Cued};
use crate::droid::{Blink, DroidKind, DroidPart, Mark, Rail};
use crate::math::{Vec2, vec2};
use crate::room::TILE;

/// The cosine and sine of thirty degrees, written out: a blink's ways are
/// turned by them.
const COS_30: f32 = 0.866_025_4;
const SIN_30: f32 = 0.5;
/// And of forty-five: the first of a strike's bombs is laid that way off
/// its mark, the rest a quarter turn on from each other.
const COS_45: f32 = 0.707_106_77;

impl Game {
    /// The top of every machines' step (task 157): every machine's link and
    /// each Conductor's tethers, every Conductor's mark run down and kept on
    /// the body it is on, and which body the machines hold to this step
    /// ([`Game::marked`]).
    pub(super) fn link_and_mark(&mut self, dt: f32) {
        let reach = balance::LINK_RADIUS * TILE;
        let conductors: Vec<(usize, Vec2)> = self
            .droids
            .iter()
            .enumerate()
            .filter(|(_, d)| d.kind == DroidKind::Conductor && !d.destroyed && !d.is_stunned())
            .map(|(j, d)| (j, d.pos))
            .collect();
        for i in 0..self.droids.len() {
            let d = &self.droids[i];
            let linked = !d.destroyed
                && !d.kind.is_structure()
                && conductors
                    .iter()
                    .any(|&(j, at)| j != i && (at - d.pos).len() <= reach);
            self.droids[i].rhythm.linked = linked;
        }
        for j in 0..self.droids.len() {
            if self.droids[j].kind != DroidKind::Conductor {
                continue;
            }
            let at = self.droids[j].pos;
            let standing = conductors.iter().any(|&(k, _)| k == j);
            let tethers: Vec<Vec2> = if standing {
                self.droids
                    .iter()
                    .enumerate()
                    .filter(|&(i, d)| i != j && d.rhythm.linked && (d.pos - at).len() <= reach)
                    .map(|(_, d)| d.pos)
                    .collect()
            } else {
                Vec::new()
            };
            self.droids[j].tethers = tethers;
        }
        let targets = self.combat.machine_targets().to_vec();
        let mut marked = None;
        for d in self
            .droids
            .iter_mut()
            .filter(|d| d.kind == DroidKind::Conductor && !d.destroyed)
        {
            let Some(mut m) = d.rhythm.mark else {
                continue;
            };
            m.left -= dt;
            match targets.get(m.target).copied().flatten() {
                Some(t) if m.left > 0.0 => {
                    m.at = t.at;
                    d.rhythm.mark = Some(m);
                    if m.holding() && marked.is_none() {
                        marked = Some(m.target);
                    }
                }
                _ => d.rhythm.mark = None,
            }
        }
        self.marked = marked;
    }

    /// What a machine at `from` aims at with `stats` (task 157): the body a
    /// Conductor's mark holds the machines to, while it can shoot it — but
    /// not where a tank's taunt reaches it, which comes first — else
    /// [`Combat::aim_among`] over its targets as ever.
    pub(super) fn machine_aim(
        &self,
        from: Vec2,
        stats: &WeaponStats,
    ) -> Option<(usize, Vec2, Vec2)> {
        let targets = self.combat.machine_targets();
        if let Some(m) = self.marked
            && m < targets.len()
            && !targets
                .iter()
                .flatten()
                .any(|t| t.taunting > 0.0 && (t.at - from).len() <= t.taunting)
        {
            let only: Vec<Option<Target>> = targets
                .iter()
                .enumerate()
                .map(|(k, t)| if k == m { *t } else { None })
                .collect();
            if let Some(aim) = Combat::aim_among(&only, &self.room.sight, from, stats) {
                return Some(aim);
            }
        }
        Combat::aim_among(targets, &self.room.sight, from, stats)
    }

    /// The nearest of the machines' targets up, seen (not a belief), not a
    /// sealed core, within `reach` of `from` and with a clear line to it:
    /// its index and where it stands.
    fn nearest_seen(&self, from: Vec2, reach: f32) -> Option<(usize, Vec2)> {
        self.combat
            .machine_targets()
            .iter()
            .enumerate()
            .filter_map(|(k, t)| t.map(|t| (k, t)))
            .filter(|(_, t)| !t.stale && !t.sealed() && (t.at - from).len() <= reach)
            .filter(|(_, t)| self.line_clear(from, t.at))
            .min_by(|a, b| (a.1.at - from).len().total_cmp(&(b.1.at - from).len()))
            .map(|(k, t)| (k, t.at))
    }

    /// A Bomber's bomb (task 157): its wait run down, and with a body of the
    /// crew's side seen within [`balance::BOMB_TRIGGER`] tiles, its arms on
    /// and the wait out, one rolled at it — [`balance::BOMB_REACH`] tiles at
    /// most, stopped short by what is in the way — and the wait set to
    /// [`balance::BOMB_COOLDOWN`]. With nobody seen the wait is held at
    /// [`balance::BOMB_FIRST`] at least, so the first bomb comes a moment
    /// after it has somebody. The damage at the centre is
    /// [`balance::BOMB_DAMAGE`] times its tier's damage factor.
    pub(super) fn roll_bomb(&mut self, i: usize, dt: f32) {
        let from = self.droids[i].pos;
        let seen = self.nearest_seen(from, balance::BOMB_TRIGGER * TILE);
        let d = &mut self.droids[i];
        d.rhythm.bomb_wait = (d.rhythm.bomb_wait - dt).max(0.0);
        let Some((_, at)) = seen else {
            d.rhythm.bomb_wait = d.rhythm.bomb_wait.max(balance::BOMB_FIRST);
            return;
        };
        if d.rhythm.bomb_wait > 0.0 || d.body.gone(DroidPart::Arms) {
            return;
        }
        d.rhythm.bomb_wait = balance::BOMB_COOLDOWN;
        d.struck_out();
        let damage = balance::BOMB_DAMAGE * d.tier.damage_factor();
        let weapon = d.weapon;
        let span = (at - from).len().min(balance::BOMB_REACH * TILE);
        let want = from + (at - from).normalize_or_zero() * span;
        let stop = self.bomb_stop(from, want);
        self.reveal(self.bims.len() + i);
        self.lay_bomb(from, stop, weapon, damage);
    }

    /// A bomb rolling from `from` to `at` with `damage` at the centre:
    /// recorded for the world to lay in the crew's room, and laid here as
    /// well, unseen, when the machines' list has bodies of this room's own
    /// on it — a town the crew are defending — as a Guardian's grenade is.
    fn lay_bomb(&mut self, from: Vec2, at: Vec2, weapon: Weapon, damage: f32) {
        self.combat.shoot_bomb(from, at, weapon, damage);
        if self.combat.machine_cross() < self.combat.machine_targets().len() {
            self.combat.drop_bomb(from, at, damage, true);
        }
    }

    /// Where a bomb rolled from `from` towards `to` comes to rest: the last
    /// point, a quarter tile at a time, short of a wall or a shut door, a
    /// tile of low cover (not the one it left from), or deck no body could
    /// stand on past the first half tile.
    pub(crate) fn bomb_stop(&self, from: Vec2, to: Vec2) -> Vec2 {
        let sight = &self.room.sight;
        let to = match sight.first_opaque_along(from, to) {
            Some(wall) => {
                let back = (wall - from).normalize_or_zero() * (TILE * 0.25);
                if (wall - from).len() > TILE * 0.25 {
                    wall - back
                } else {
                    from
                }
            }
            None => to,
        };
        let span = (to - from).len();
        let steps = (span / (TILE * 0.25)).ceil().max(1.0) as u32;
        let nav = self.maps.deck();
        let start = sight.tile_of(from);
        let mut last = from;
        for k in 1..=steps {
            let p = from + (to - from) * (k as f32 / steps as f32);
            let (x, y) = sight.tile_of(p);
            if (x, y) != start && sight.cover_at(x, y) {
                break;
            }
            if (p - from).len() > TILE * 0.5 && !nav.is_free(p) {
                break;
            }
            last = p;
        }
        last
    }

    /// One step of a **Lancer** (task 157). Charging, it stands, faces its
    /// line and follows its mark until the last [`balance::LANCER_LOCK`],
    /// then the slug goes along the line to the rail's reach — recorded for
    /// the world to fly in the crew's room, and flown here too where the
    /// machines' list has this room's own bodies on it. Otherwise it plans
    /// a stand like any gunner, and with its mark seen from its own eye
    /// within [`balance::LANCER_REACH`] tiles and the rail ready it plants
    /// and begins a charge. Its arms shot away in a charge begun with them,
    /// or the fight gone, put the charge out ([`balance::LANCER_CANCELLED`]).
    pub(super) fn tick_lancer(&mut self, i: usize, dt: f32, war: bool, stats: &WeaponStats) {
        {
            let d = &mut self.droids[i];
            d.trigger.hold();
            d.locked = None;
            d.blow = None;
            d.peek = None;
            if let Rail::Cooling { left } = d.rhythm.rail {
                d.rhythm.rail = if left - dt <= 0.0 {
                    Rail::Ready
                } else {
                    Rail::Cooling { left: left - dt }
                };
            }
            if let Rail::Charging { armed, .. } = d.rhythm.rail
                && ((armed && d.body.gone(DroidPart::Arms)) || !war)
            {
                d.put_out_charge();
            }
        }
        if let Rail::Charging {
            left,
            aim,
            mark,
            armed,
            ..
        } = self.droids[i].rhythm.rail
        {
            let from = self.droids[i].pos;
            let mut aim = aim;
            if left > balance::LANCER_LOCK
                && let Some(t) = self.combat.machine_targets().get(mark).copied().flatten()
                && !t.stale
            {
                let to = (t.at - from).normalize_or_zero();
                if to != Vec2::ZERO {
                    aim = to;
                }
            }
            let reach = stats.reach();
            let far = from + aim * reach;
            let end = self.room.sight.first_opaque_along(from, far).unwrap_or(far);
            let left = left - dt;
            let d = &mut self.droids[i];
            if d.is_walking() {
                d.halt();
            }
            d.face(aim.angle());
            d.charging(dt, true);
            if left > 0.0 {
                d.rhythm.rail = Rail::Charging {
                    left,
                    aim,
                    end,
                    mark,
                    armed,
                };
                return;
            }
            d.rhythm.rail = Rail::Cooling {
                left: balance::LANCER_COOLDOWN,
            };
            d.fired();
            let weapon = d.weapon;
            let muzzle = d.muzzle();
            self.reveal(self.bims.len() + i);
            self.lit_muzzle(muzzle, far, weapon);
            self.combat.shoot(from, far, weapon, false);
            self.combat.sign_last_shot(self.bims.len() + i);
            if self.combat.machine_cross() < self.combat.machine_targets().len() {
                self.combat.fire(from, far, weapon, true, false);
            }
            return;
        }
        if !war {
            self.droids[i].charging(dt, false);
            return;
        }
        self.plan_droid_stand(i, dt, stats);
        self.breach_droid(i, dt);
        let from = self.droids[i].pos;
        let reach = balance::LANCER_REACH * TILE;
        let sighted = self
            .machine_aim(from, stats)
            .filter(|&(_, eye, at)| eye == from && (at - from).len() <= reach);
        let d = &mut self.droids[i];
        d.charging(dt, false);
        let Some((mark, _, at)) = sighted else {
            return;
        };
        if !d.is_walking() {
            d.face((at - from).angle());
        }
        let aim = (at - from).normalize_or_zero();
        if d.rhythm.rail != Rail::Ready || aim == Vec2::ZERO {
            return;
        }
        d.halt();
        d.rhythm.rail = Rail::Charging {
            left: balance::LANCER_TRACK + balance::LANCER_LOCK,
            aim,
            end: at,
            mark,
            armed: !d.body.gone(DroidPart::Arms),
        };
        self.reveal(self.bims.len() + i);
        self.combat.cues.push(Cued {
            cue: Cue::RailCharge,
            at: from,
        });
    }

    /// A **Conductor**'s own rhythms (task 157), before its gunner's step:
    /// a blink it is planted for run down and taken; then, at war, the
    /// strike call once its health is down to [`balance::STRIKE_AT`], a
    /// mark put on the nearest body it sees once its wait is out, and a
    /// blink begun away from a body within [`balance::BLINK_NEAR`] tiles.
    /// Whether it is planted for a blink this step — it does nothing else.
    pub(super) fn conduct(&mut self, i: usize, dt: f32, war: bool) -> bool {
        {
            let d = &mut self.droids[i];
            d.rhythm.blink_wait = (d.rhythm.blink_wait - dt).max(0.0);
            d.rhythm.mark_wait = (d.rhythm.mark_wait - dt).max(0.0);
        }
        if let Some(blink) = self.droids[i].rhythm.blink {
            let d = &mut self.droids[i];
            d.trigger.hold();
            d.peek = None;
            d.locked = None;
            d.blow = None;
            d.charging(dt, false);
            if d.is_walking() {
                d.halt();
            }
            if blink.left - dt > 0.0 {
                d.rhythm.blink = Some(Blink {
                    left: blink.left - dt,
                    ..blink
                });
                return true;
            }
            let from = d.pos;
            d.pos = blink.to;
            d.was = blink.to;
            d.rhythm.blink = None;
            d.rhythm.blinked = Some((from, 0.0));
            d.rhythm.blink_wait = balance::BLINK_COOLDOWN;
            d.plan_wait = 0.0;
            self.reveal(self.bims.len() + i);
            self.combat.cues.push(Cued {
                cue: Cue::Blink,
                at: from,
            });
            return true;
        }
        if !war {
            return false;
        }
        let from = self.droids[i].pos;
        // The strike call: once, the first time its health is down to the
        // share, on its mark or the nearest body it sees.
        if !self.droids[i].rhythm.called && self.droids[i].body.life_share() <= balance::STRIKE_AT {
            let centre = self.droids[i]
                .rhythm
                .mark
                .map(|m| m.at)
                .or_else(|| self.nearest_seen(from, f32::INFINITY).map(|(_, at)| at));
            if let Some(centre) = centre {
                self.droids[i].rhythm.called = true;
                self.call_strike(i, centre);
            }
        }
        // The mark.
        if self.droids[i].rhythm.mark.is_none() {
            let pick = self.nearest_seen(from, f32::INFINITY);
            let d = &mut self.droids[i];
            match pick {
                None => d.rhythm.mark_wait = d.rhythm.mark_wait.max(balance::MARK_FIRST),
                Some((target, at)) if d.rhythm.mark_wait <= 0.0 => {
                    d.rhythm.mark = Some(Mark {
                        target,
                        at,
                        left: balance::MARK_WARNING + balance::MARK_HOLD,
                    });
                    d.rhythm.mark_wait = balance::MARK_EVERY;
                    self.combat.cues.push(Cued {
                        cue: Cue::Marked,
                        at,
                    });
                }
                Some(_) => {}
            }
        }
        // The blink.
        if self.droids[i].rhythm.blink_wait <= 0.0
            && self.droids[i].can_move()
            && let Some((_, threat)) = self.nearest_seen(from, balance::BLINK_NEAR * TILE)
            && let Some(to) = self.blink_to(i, threat)
        {
            let d = &mut self.droids[i];
            d.halt();
            d.trigger.hold();
            d.rhythm.blink = Some(Blink {
                left: balance::BLINK_WINDUP,
                to,
            });
            return true;
        }
        false
    }

    /// Where Conductor `i` blinks to from a body at `threat`: straight away
    /// from it at [`balance::BLINK_REACH`] tiles, else turned thirty, sixty
    /// or ninety degrees either way, else the same at half the reach — the
    /// first that is free deck it could walk to, with nothing opaque between
    /// and farther than [`balance::BLINK_NEAR`] from the threat.
    fn blink_to(&self, i: usize, threat: Vec2) -> Option<Vec2> {
        let d = &self.droids[i];
        let from = d.pos;
        let away = (from - threat).normalize_or_zero();
        let away = if away == Vec2::ZERO { -d.front() } else { away };
        let nav = self.maps.deck();
        let turns = [
            (1.0, 0.0),
            (COS_30, SIN_30),
            (COS_30, -SIN_30),
            (SIN_30, COS_30),
            (SIN_30, -COS_30),
            (0.0, 1.0),
            (0.0, -1.0),
        ];
        for reach in [balance::BLINK_REACH, balance::BLINK_REACH * 0.5] {
            for (c, s) in turns {
                let to = from + away.rotate_by(c, s) * (reach * TILE);
                if (to - threat).len() <= balance::BLINK_NEAR * TILE
                    || !nav.is_free(to)
                    || !nav.can_reach(from, to)
                    || self.room.sight.first_opaque_along(from, to).is_some()
                {
                    continue;
                }
                return Some(to);
            }
        }
        None
    }

    /// A Conductor's **strike call** (task 157): [`balance::STRIKE_BOMBS`]
    /// bombs' circles laid round `centre`, [`balance::STRIKE_SPREAD`] tiles
    /// out — the first forty-five degrees off it, the rest a quarter turn
    /// on — each on free deck, and each a bomb that only spins up.
    fn call_strike(&mut self, i: usize, centre: Vec2) {
        let d = &self.droids[i];
        let damage = balance::BOMB_DAMAGE * d.tier.damage_factor();
        let weapon = d.weapon;
        let mut dir = vec2(COS_45, COS_45);
        let spots: Vec<Vec2> = (0..balance::STRIKE_BOMBS)
            .map(|_| {
                let at = centre + dir * (balance::STRIKE_SPREAD * TILE);
                dir = dir.rotate_by(0.0, 1.0);
                let nav = self.maps.deck();
                if nav.is_free(at) {
                    at
                } else {
                    nav.nearest_free(at)
                }
            })
            .collect();
        self.reveal(self.bims.len() + i);
        for at in spots {
            self.lay_bomb(at, at, weapon, damage);
        }
    }

    /// A Bomber, a Lancer or a Conductor with a body of the crew's side
    /// seen within `shy` tiles of `from` (task 157): the walk to the free
    /// cell within three tiles that puts it farthest from all of them, half
    /// a tile of walking taken off each tile gained — `None` where nobody is
    /// that near, or nothing within reach is half a tile better.
    pub(super) fn keep_back(
        &self,
        from: Vec2,
        targets: &[Option<Target>],
        shy: f32,
    ) -> Option<Vec<Vec2>> {
        let live: Vec<Vec2> = targets
            .iter()
            .flatten()
            .filter(|t| !t.stale && !t.sealed())
            .map(|t| t.at)
            .collect();
        let nearest = |p: Vec2| {
            live.iter()
                .map(|&t| (t - p).len())
                .fold(f32::INFINITY, f32::min)
        };
        let now = nearest(from);
        if now > shy * TILE {
            return None;
        }
        let nav = self.maps.for_body(false);
        let (best, score) = nav
            .free_cells_within(from, 3.0 * TILE, TILE)
            .into_iter()
            .filter(|&p| nav.can_reach(from, p))
            .map(|p| (p, nearest(p) - 0.5 * (p - from).len()))
            .max_by(|a, b| a.1.total_cmp(&b.1))?;
        if score <= now + TILE * 0.5 {
            return None;
        }
        let route = nav.path(from, best);
        (!route.is_empty()).then_some(route)
    }

    /// A Bomber's bomb or a strike call recorded in the other room, laid in
    /// this one by the world (task 157): rolling from `from` to `at`,
    /// bursting on this room's own bodies and its targets with `damage` at
    /// the centre — see [`Combat::drop_bomb`].
    pub fn enemy_bomb(&mut self, from: Vec2, at: Vec2, damage: f32) {
        self.combat.drop_bomb(from, at, damage, false);
    }

    /// The Bomber's stand (task 157): its pistol's reach and sweet range cut
    /// to [`balance::BOMBER_STAND`] tiles for the stand it picks, so it walks
    /// in to bomb.
    pub(super) fn bomber_stand(stats: &WeaponStats) -> WeaponStats {
        WeaponStats {
            range: stats.range.min(balance::BOMBER_STAND),
            sweet: stats.sweet.min(balance::BOMBER_STAND * 0.5),
            ..*stats
        }
    }
}
