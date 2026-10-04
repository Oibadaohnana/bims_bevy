//! A Manufacturer in a friendly room (task 131): one of their people come
//! to attack a site the crew are defending, standing in the residents'
//! room with the site's own people. A child of `game`, so it reaches the
//! room's fields the way `tick_droids` does.
//!
//! **An intruder is derived, never kept**: a Bim with
//! [`Bim::manufacturer`](crate::bim::Bim) in a room whose bodies are not
//! hostile ([`Game::is_intruder`]). At a site of their own the room's
//! bodies are hostile and a Manufacturer is the hostile Bim it always
//! was; here it is an enemy among friends, and it fights **the way the
//! machines fight in a defended town** (feature 94): off the machines'
//! own list ([`crate::combat::Combat::machine_targets`]) — the crew
//! across the seam, then the site's people — a shot at an index below
//! `machine_cross` recorded as a `Shot` for the world to fly in the
//! crew's room, one above it a hostile bolt that flies here and lands on
//! the townsperson. Everything else about it is a Bim's: it walks, it is
//! downed and bleeds out, nobody revives it, doors open for it.
//!
//! What keeps the site's own side off it and it off them is in three
//! places: the world hands it to the site's people as a target and keeps
//! it off the machines' list; the room leaves it out of the bodies a
//! hostile bolt looks for (`tick_combat`); and the alarm's muster and the
//! site's shelter leave it alone.

use super::{Game, PLAN_EVERY, TILE};
use crate::character::{Action, SWING_TIME};
use crate::combat::{
    Blow, COVER_WORTH, Combat, DISTANCE_WORTH, FIST_DAMAGE, MELEE_PERIOD, MELEE_RANGE, Tactics,
    WeaponKind, WeaponStats,
};
use crate::math::{Rect, Vec2};

impl Game {
    /// Whether that Bim is a Manufacturer in a friendly room — an enemy
    /// among a site's own people, come with a wave the crew are holding
    /// off (task 131). False for a machine, for every other Bim, and for
    /// a Manufacturer at a site of theirs, whose room is hostile.
    pub fn is_intruder(&self, who: usize) -> bool {
        !self.hostile_bodies && self.bims.get(who).is_some_and(|b| b.manufacturer)
    }

    /// One step of an intruder's fight, from `tick_combat`'s turn for the
    /// body: its stand planned, then a blow landing, a melee lock, or an
    /// aim and a shot — the machines' rule, a Bim's body.
    pub(super) fn tick_intruder(&mut self, who: usize, dt: f32) {
        let bim = &self.bims[who];
        let up = bim.is_alive() && !bim.character.is_outside() && !bim.character.is_unconscious();
        let weapon = bim.gear.weapon.unwrap_or(WeaponKind::LaserPistol.basic());
        let war = self.combat.machine_targets().iter().any(|t| t.is_some());
        if !up || !war {
            let bim = &mut self.bims[who];
            bim.trigger.hold();
            bim.locked = None;
            bim.blow = None;
            bim.peek = None;
            bim.character.set_lean(None);
            bim.character.set_aim(None);
            bim.character.set_armed(up.then_some(weapon.kind));
            return;
        }
        // Under arms for as long as it stands: it came to fight.
        let bim = &mut self.bims[who];
        bim.character.set_post(None);
        bim.character.set_recruited(true);
        bim.character.set_armed(Some(weapon.kind));
        let stats = weapon.stats();
        self.plan_intruder_stand(who, dt, &stats);
        let from = self.bims[who].character.pos;
        let cross = self.combat.machine_cross();

        // A swing that has been swung lands now, on the target it was
        // aimed at if that one is still within reach.
        if let Some(blow) = self.bims[who].blow.take_if(|b| b.left <= 0.0) {
            if blow.target < cross {
                let at = self
                    .combat
                    .machine_targets()
                    .get(blow.target)
                    .copied()
                    .flatten()
                    .filter(|t| (t.at - from).len() <= MELEE_RANGE * TILE)
                    .map(|t| t.at);
                if let Some(at) = at {
                    self.combat.brawl_at(
                        from,
                        blow.target,
                        at,
                        weapon,
                        blow.damage,
                        blow.cut,
                        true,
                        None,
                        blow.flat,
                    );
                    self.combat.sign_last_shot(who);
                }
            } else {
                self.enemy_strike(from, blow.target - cross, blow.damage, blow.cut);
            }
        }

        // A melee first: locked, it neither aims nor fires.
        let locked = Combat::melee_among(
            self.combat.machine_targets(),
            &self.room.sight,
            from,
            &stats,
        );
        self.bims[who].locked = locked;
        if let Some(enemy) = locked {
            let at = self.combat.machine_targets()[enemy].map(|t| t.at);
            let bim = &mut self.bims[who];
            bim.trigger.hold();
            bim.peek = None;
            bim.character.set_lean(None);
            bim.character.set_aim(None);
            if let Some(at) = at {
                bim.character.face((at - from).angle());
            }
            if bim.melee_timer <= 0.0 {
                bim.melee_timer = MELEE_PERIOD;
                let (damage, cut, swing) = if stats.melee {
                    (stats.damage, true, Action::Swing)
                } else {
                    (FIST_DAMAGE, false, Action::Punch)
                };
                bim.character.antic(swing, SWING_TIME);
                bim.blow = Some(Blow {
                    target: enemy,
                    left: SWING_TIME,
                    damage,
                    cut,
                    flat: damage,
                });
            }
            return;
        }
        let aimed = if stats.melee {
            None
        } else {
            Combat::aim_among(
                self.combat.machine_targets(),
                &self.room.sight,
                from,
                &stats,
            )
        };
        let bim = &mut self.bims[who];
        let Some((mark, eye, at)) = aimed else {
            bim.trigger.hold();
            bim.peek = None;
            bim.character.set_lean(None);
            bim.character.set_aim(None);
            return;
        };
        // On the move it shoots from its own eyes at the walking odds, as
        // anybody does; a peek wants standing against the wall.
        let walking = bim.character.is_walking();
        if walking && eye != from {
            bim.trigger.hold();
            bim.peek = None;
            bim.character.set_lean(None);
            bim.character.set_aim(None);
            return;
        }
        let peek = (eye != from).then_some(eye);
        bim.peek = peek;
        bim.character.set_lean(peek);
        if !walking {
            bim.character.face((at - eye).angle());
        }
        bim.character.set_aim(Some(at));
        if bim.trigger.pull(dt, &stats) {
            let muzzle = self.shot_from(who, eye, at);
            self.reveal(who);
            self.lit_muzzle(muzzle, at, weapon);
            if mark < cross {
                // At the crew, across the seam: recorded here and flown
                // by the world in the crew's room.
                self.combat.shoot(muzzle, at, weapon, walking);
                self.combat.sign_last_shot(who);
            } else {
                // At one of the site's own people: a hostile bolt, here.
                self.combat.fire(muzzle, at, weapon, true, walking);
            }
        }
    }

    /// Where an intruder walks to: a hostile Bim's stand — cover taken,
    /// the hunter's rule with nobody in sight — scored against the
    /// machines' list rather than the room's.
    fn plan_intruder_stand(&mut self, who: usize, dt: f32, stats: &WeaponStats) {
        self.bims[who].plan_wait -= dt;
        if self.bims[who].plan_wait > 0.0 {
            return;
        }
        self.bims[who].plan_wait = PLAN_EVERY;
        let from = self.bims[who].character.pos;
        // An Area defend's ring (October 2026): walked to and held, as
        // the machines do.
        if let Some(spot) = self.objective(who)
            && !self.prey_at_hand(from, stats)
        {
            self.walk_for(who, spot);
            return;
        }
        let nav = self.maps.for_body(false);
        let targets = self.combat.machine_targets().to_vec();
        if !stats.melee && targets.iter().flatten().all(|t| t.stale) {
            self.bims[who].hunting = true;
            let Some(to) = Tactics::charge(nav, from, &targets) else {
                return;
            };
            let going = self.bims[who].character.destination().unwrap_or(from);
            if (to - going).len() <= TILE {
                return;
            }
            let route = nav.path(from, to);
            if !route.is_empty() {
                self.bims[who].character.follow_path(route);
            }
            return;
        }
        let closing = self.bims[who].hunting;
        let doorways: Vec<Rect> = self
            .room
            .doors
            .iter()
            .map(|d| d.rect)
            .chain(self.room.airlocks.iter().copied())
            .collect();
        // Where the rest of its side stand or are walking to: the other
        // intruders and the machines.
        let taken: Vec<Vec2> = self
            .bims
            .iter()
            .enumerate()
            .filter(|&(j, b)| j != who && b.manufacturer && b.is_alive())
            .map(|(_, b)| b.character.destination().unwrap_or(b.character.pos))
            .chain(
                self.droids
                    .iter()
                    .filter(|d| !d.destroyed)
                    .map(|d| d.destination()),
            )
            .collect();
        let Some(stand) = Tactics::stand_scored(
            &self.room.sight,
            nav,
            from,
            &targets,
            stats,
            &doorways,
            &taken,
            closing,
            COVER_WORTH,
            DISTANCE_WORTH,
            crate::combat::Seeing::Line,
        ) else {
            return;
        };
        let has_a_shot =
            !stats.melee && Combat::aim_among(&targets, &self.room.sight, from, stats).is_some();
        if has_a_shot && !stand.cover {
            if self.bims[who].character.is_walking() {
                self.bims[who].character.halt();
            }
            return;
        }
        let going = self.bims[who].character.destination().unwrap_or(from);
        if (stand.at - going).len() <= TILE {
            return;
        }
        let route = nav.path(from, stand.at);
        if !route.is_empty() {
            self.bims[who].character.follow_path(route);
        }
    }
}
