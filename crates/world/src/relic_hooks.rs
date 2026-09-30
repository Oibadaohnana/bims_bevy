//! What task 118's relics do in a fight (`crate::relic`, the rows): a
//! crew hit landing on a machine — *Marksman's Habit*, *Servo Cutter*,
//! *Crippler's Mark*, *Total Teardown*, *Blind Spot*, *Crossfire*,
//! *Spotter* — the auras (*Field Radio*, *Cover Formation*), the reach
//! standing still (*Wide Angle Optics*), the timed effects (*Sprint
//! Coil*, *Tether Field*, *Signal Scrambler*) and how long an ability's
//! last (*Strong Will*), the healing (*Pressure Seal*, *Clot Booster*,
//! *Quick Wrap*), a player falling low or a crewmate down (*Lifeline*,
//! *Rally Point*), the pay (*Hazard Pay*, *Parts Broker*, *Squad Morale*)
//! and *War Chest*.
//!
//! The Lifeline patch heals hit points and never touches the blood.
//!
//! A child of `crate::world`, as `relics.rs` is. **Nothing here draws
//! from a stream** and nothing here runs for a crew holding no relic, so a
//! run without them plays exactly as it did.

use super::*;
use crate::relic::{self, Action, Buff, Relic, Rule, Situation, Stat, Trigger};
use bims::droid::DroidPart;
use bims::math::Vec2;

/// A machine destroyed, as `visit` saw it: who hit it last (a crew index),
/// what the Republic pays for it, and how it was when it went — missing a
/// limb, and the last hit from the side or behind.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct MachineKill {
    pub by: Option<usize>,
    pub bounty: Money,
    pub crippled: bool,
    pub flanked: bool,
}

impl World {
    // --- reading ---------------------------------------------------------------

    /// Whether any player's Bim holds a relic at all: nothing below does
    /// anything for a crew that holds none.
    pub(super) fn any_relics(&self) -> bool {
        (0..self.players()).any(|s| !self.relics_of(s).is_empty())
    }

    /// Minutes of the mission clock in `seconds` of it.
    fn relic_minutes(seconds: f64) -> f64 {
        seconds * time::MINUTES_PER_SECOND
    }

    /// Whether `relic`'s timed effect is on crew member `who` now.
    pub fn relic_buff_on(&self, who: u32, relic: Relic) -> bool {
        let now = self.mission_minutes();
        self.run
            .relics
            .buffs
            .iter()
            .any(|b| b.who == who && b.relic == relic && b.until > now)
    }

    /// `relic`'s timed effect put on crew member `who` for `seconds` —
    /// longer if it is on already, never shorter.
    pub(super) fn put_relic_buff(&mut self, who: u32, relic: Relic, seconds: f64) {
        let until = self.mission_minutes() + Self::relic_minutes(seconds);
        let now = self.mission_minutes();
        let buffs = &mut self.run.relics.buffs;
        buffs.retain(|b| b.until > now);
        match buffs.iter_mut().find(|b| b.who == who && b.relic == relic) {
            Some(b) => b.until = b.until.max(until),
            None => buffs.push(Buff { who, relic, until }),
        }
    }

    /// Whether a relic with a cooldown may fire again for `slot`.
    pub(super) fn relic_ready(&self, slot: u32, relic: Relic) -> bool {
        let now = self.mission_minutes();
        !self
            .run
            .relics
            .ready_at
            .iter()
            .any(|&(s, r, at)| s == slot && r == relic && at > now)
    }

    /// A relic with a cooldown fired: not again for `seconds`.
    pub(super) fn relic_cools(&mut self, slot: u32, relic: Relic, seconds: f64) {
        let at = self.mission_minutes() + Self::relic_minutes(seconds);
        let ready = &mut self.run.relics.ready_at;
        ready.retain(|&(s, r, _)| !(s == slot && r == relic));
        ready.push((slot, relic, at));
    }

    /// The percentage a timed effect moves its stat by, off its relic's
    /// own row: *Sprint Coil*'s pace, *Tether Field*'s share of a hit.
    fn buff_percent(relic: Relic) -> i32 {
        let hooks: &[relic::Hook] = match &relic.def().effect {
            relic::Effect::On(hook) => core::slice::from_ref(hook),
            relic::Effect::OnEach(list) => list,
            _ => &[],
        };
        hooks
            .iter()
            .find_map(|h| match h.action {
                Action::Sprint { percent, .. } => Some(percent),
                Action::Tether { percent, .. } => Some(-percent),
                _ => None,
            })
            .unwrap_or(0)
    }

    /// Whether crew members `a` and `b` stand within `tiles` of each other
    /// on the crew's deck, both alive and neither outside.
    fn within_tiles(&self, a: u32, b: u32, tiles: f32) -> bool {
        if !self.on_the_deck(a) || !self.on_the_deck(b) {
            return false;
        }
        let room = &self.aboard.room;
        let gap = room.bim_pos(a as usize) - room.bim_pos(b as usize);
        gap.len() <= tiles * shipdesign::TILE as f32
    }

    /// The percentage the players' auras put on crew member `who`'s
    /// `stat`: every holder fit to act within its tiles — the holder's own
    /// too for an aura that says so (*Cover Formation*, task 142).
    fn aura_percent(&self, who: u32, stat: Stat) -> i32 {
        (0..self.players())
            .filter(|&p| self.fit_to_act(p))
            .flat_map(|p| {
                relic::auras(self.relics_of(p), stat)
                    .filter(move |&(_, tiles, own)| {
                        if p == who {
                            own
                        } else {
                            self.within_tiles(p, who, tiles)
                        }
                    })
                    .map(|(percent, _, _)| percent)
            })
            .sum()
    }

    /// *War Chest*'s share of `slot`'s damage now, in per cent: its step
    /// for every thousand in the player's own wallet, to its cap.
    pub fn war_chest_percent(&self, slot: u32) -> i32 {
        let Some((per, cap)) = relic::rule_of(self.relics_of(slot), |r| match r {
            Rule::WarChest {
                percent_per_thousand,
                cap,
            } => Some((percent_per_thousand, cap)),
            _ => None,
        }) else {
            return 0;
        };
        let thousands = (self.wallet(slot) / 1_000).min(i32::MAX as Money) as i32;
        thousands.saturating_mul(per).min(cap)
    }

    /// The tiles *Wide Angle Optics* adds to crew member `who`'s range
    /// while it stands still (task 142); nought without it.
    pub fn still_range_of(&self, who: u32) -> f32 {
        relic::rule_of(self.relics_of(who), |r| match r {
            Rule::StillRange { tiles } => Some(tiles),
            _ => None,
        })
        .unwrap_or(0.0)
    }

    /// What crew member `who`'s ability effects' length is multiplied by:
    /// one, or more with *Strong Will* (task 142). Read by every ability
    /// with a length — a Rampage, an EMP's stun, a Cloak, a Taunt, a
    /// Juggernaut, a Rally and a Battle Cry.
    pub fn ability_length_factor(&self, who: u32) -> f64 {
        relic::rule_of(self.relics_of(who), |r| match r {
            Rule::LongerAbilities { percent } => Some(relic::factor(percent)),
            _ => None,
        })
        .unwrap_or(1.0)
    }

    // --- the skill ----------------------------------------------------------------

    /// What task 118's relics do to a crew member's skill
    /// (`World::skill_of`, after `lift_by_relics`): *War Chest*'s damage,
    /// *Sprint Coil*'s pace, *Tether Field*'s and *Cover Formation*'s share
    /// of a hit, and *Field Radio*'s odds. Nothing for a crew holding no
    /// relic.
    pub(super) fn lift_by_relic_hooks(&self, who: u32, skill: &mut bims::combat::Skill) {
        if !self.any_relics() {
            return;
        }
        let chest = self.war_chest_percent(who);
        if chest != 0 {
            let f = relic::factor(chest) as f32;
            skill.damage *= f;
            skill.melee *= f;
        }
        if self.relic_buff_on(who, Relic::SprintCoil) {
            skill.walk *= relic::factor(Self::buff_percent(Relic::SprintCoil)) as f32;
        }
        let mut taken = self.aura_percent(who, Stat::DamageTaken);
        if self.relic_buff_on(who, Relic::TetherField) {
            taken += Self::buff_percent(Relic::TetherField);
        }
        taken += self.relic_percent(who, Stat::DamageTaken);
        if taken != 0 {
            skill.damage_taken *= relic::factor(taken) as f32;
        }
        let aim = self.aura_percent(who, Stat::Accuracy);
        if aim != 0 {
            skill.accuracy *= relic::factor(aim) as f32;
        }
        skill.still_range += self.still_range_of(who);
    }

    /// A shield's front against every crew member's shots, for the room
    /// (`Game::set_shield_fronts`): the Guardian's own for everybody since
    /// *Wide Angle Optics* stopped narrowing it (task 142), which an empty
    /// list says.
    pub(super) fn hand_the_room_the_shield_fronts(&mut self) {
        self.aboard.room.set_shield_fronts(Vec::new());
    }

    /// Which of the crew no machine aims at now: *Signal Scrambler*'s
    /// cloak. Empty for a crew holding no relic.
    pub(crate) fn unseen_by_machines(&self) -> Vec<bool> {
        if !self.any_relics() {
            return Vec::new();
        }
        (0..self.aboard.crew_count())
            .map(|who| self.relic_buff_on(who, Relic::SignalScrambler))
            .collect()
    }

    // --- the healing ------------------------------------------------------------

    /// The health the relics put back this step, after the downs are
    /// settled: *Pressure Seal* all the time a holder is alive, *Clot
    /// Booster* for its seconds after it went down — which a downed body
    /// takes only once it is revived (task 120).
    pub(crate) fn relics_mend(&mut self) {
        if !self.any_relics() {
            return;
        }
        let now = self.mission_minutes();
        let seconds = (data::STEP_MINUTES / time::MINUTES_PER_SECOND) as f32;
        for who in 0..self.players().min(self.aboard.crew_count()) {
            let at = who as usize;
            if !self.aboard.room.is_alive(at) {
                continue;
            }
            let held = self.relics_of(who);
            let mut rate = relic::rule_of(held, |r| match r {
                Rule::Regen { hp_per_second } => Some(hp_per_second),
                _ => None,
            })
            .unwrap_or(0.0);
            if let Some((hp_per_second, lasts)) = relic::rule_of(held, |r| match r {
                Rule::MendWhileDown {
                    hp_per_second,
                    seconds,
                } => Some((hp_per_second, seconds)),
                _ => None,
            }) && !self.aboard.room.is_down(at)
                && let Some(since) = self.run.relics.downed_at.get(at).copied().flatten()
                && now - since < Self::relic_minutes(lasts)
            {
                rate += hp_per_second;
            }
            if rate > 0.0 {
                // Times the Healing Aura where it stands (task 130).
                self.heal_crew(at as u32, rate * seconds);
            }
        }
    }

    /// Every revive the room finished this step, done by a player:
    /// `Trigger::Revived` on the crewmate brought round (*Quick Wrap*,
    /// *Tether Field*, task 120).
    pub(super) fn relics_on_a_revive(
        &mut self,
        helper: usize,
        patient: usize,
        events: &mut Vec<WorldEvent>,
    ) {
        if (helper as u32) >= self.players() {
            return;
        }
        let (by, on) = (helper as u32, Some(patient as u32));
        self.relic_trigger_on(by, Trigger::Revived, on, events);
    }

    // --- a hit on a machine ---------------------------------------------------------

    /// Every crew hit this step that landed on one of the residents'
    /// machines, delivered — the part read off the hit's own roll, and
    /// what the relics make of the part and the damage — and the rest, the
    /// hits on the residents' Bims, handed back for `visit` to land. With
    /// no relic held it is `strike_droid(hit_by(roll), damage)` exactly, as
    /// it always was.
    pub(crate) fn land_on_machines(
        &mut self,
        hits: Vec<bims::combat::Hit>,
    ) -> Vec<bims::combat::Hit> {
        let Some(shift) = self.residents.as_ref().map(|r| r.aboard.offset) else {
            return hits;
        };
        let relics = self.any_relics();
        // Where each of the crew stands in the residents' room, on its
        // feet: what a flank and a crossfire are read off.
        let crew_at: Vec<Option<Vec2>> = if relics {
            self.aboard
                .crew_ashore()
                .into_iter()
                .map(|p| {
                    p.map(|p| {
                        let at = p.add(shift);
                        bims::math::vec2(at.x as f32, at.y as f32)
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        let mut rest = Vec::new();
        for hit in hits {
            let Some(residents) = self.residents.as_ref() else {
                break;
            };
            let room = &residents.aboard.room;
            let bims = room.crew_count() as usize;
            let Some(i) = hit.who.checked_sub(bims) else {
                rest.push(hit);
                continue;
            };
            if hit.who >= room.body_count() as usize || !room.is_alive(hit.who) {
                continue;
            }
            let rolled = DroidPart::hit_by(hit.roll);
            let exposed = room.droid(i).is_some_and(|d| d.is_exposed());
            let (part, damage) = if relics {
                self.relic_hit_on_machine(&hit, i, rolled, &crew_at)
            } else if exposed {
                // A top-rank EMP's stun (task 127): the machine takes more
                // from everyone while it lasts — the relics' own sum.
                let factor = relic::factor(class::EMP_EXPOSE_PERCENT) as f32;
                (rolled, hit.damage * factor)
            } else {
                (rolled, hit.damage)
            };
            // Weak Spot's crit (task 124), after every relic's factor.
            let damage = damage + self.crit_extra(&hit);
            let Some(residents) = self.residents.as_mut() else {
                break;
            };
            residents.aboard.room.strike_droid(i, part, damage);
            // And what it did, for the number over the machine.
            self.shown_hits.push((hit.who as u32, damage, hit.crit));
            if let Some(last) = residents.last_hit_by.get_mut(hit.who) {
                *last = hit.by;
            }
        }
        rest
    }

    /// One crew hit on machine `i` (body `hit.who`) through the relics:
    /// the part it lands on and what it does. Notes *Marksman's Habit*'s
    /// machines, *Spotter*'s mark and whether the hit came from the side.
    fn relic_hit_on_machine(
        &mut self,
        hit: &bims::combat::Hit,
        i: usize,
        rolled: DroidPart,
        crew_at: &[Option<Vec2>],
    ) -> (DroidPart, f32) {
        let now = self.mission_minutes();
        let body = hit.who as u32;
        let players = self.players();
        let slot = hit.by.map(|b| b as u32).filter(|&b| b < players);
        let held: Vec<Relic> = slot.map_or(Vec::new(), |s| self.relics_of(s).to_vec());
        let Some(d) = self.residents.as_ref().and_then(|r| r.aboard.room.droid(i)) else {
            return (rolled, hit.damage);
        };
        let (pos, front) = (d.pos, d.front());
        let solid = d.body.is_solid();
        let gone = |p: DroidPart| d.body.gone(p);
        let crippled = !solid && (gone(DroidPart::Arms) || gone(DroidPart::Legs));
        let limb = |p: DroidPart| matches!(p, DroidPart::Arms | DroidPart::Legs);

        // *Marksman's Habit*: its first hit on this machine lands on a
        // limb — one it still has, where it has one — whatever was rolled.
        let mut part = rolled;
        let habit = relic::rule_of(&held, |r| (r == Rule::FirstHitOnLimb).then_some(()));
        if let (Some(s), Some(())) = (slot, habit)
            && !solid
            && !self.run.relics.limb_aimed.contains(&(s, body))
        {
            let first = if hit.roll < 0.5 {
                DroidPart::Arms
            } else {
                DroidPart::Legs
            };
            let other = if first == DroidPart::Arms {
                DroidPart::Legs
            } else {
                DroidPart::Arms
            };
            part = if !gone(first) || gone(other) {
                first
            } else {
                other
            };
            self.run.relics.limb_aimed.push((s, body));
        }
        let on_limb = !solid && limb(part) && !gone(part);
        let tear = !solid && limb(part) && gone(part);

        // From the side or behind: the shooter outside the machine's front
        // arc, the Guardian shield's.
        let shooter = hit.by.and_then(|b| crew_at.get(b).copied().flatten());
        let flanked = match (slot, shooter) {
            (Some(_), Some(at)) => {
                let toward = (at - pos).normalize_or_zero();
                toward != Vec2::ZERO && front.dot(toward) < bims::balance::GUARDIAN_SHIELD_COS
            }
            _ => false,
        };

        // A top-rank EMP's stun (task 127) is on the same sum as the relics.
        let exposed = if d.is_exposed() {
            class::EMP_EXPOSE_PERCENT
        } else {
            0
        };
        let mut percent = match slot {
            Some(s) => relic::stat_percent(
                &held,
                Stat::MachineDamage,
                Situation {
                    other_down: false,
                    on_limb,
                    crippled,
                    flanked,
                },
            )
            .saturating_add(self.crossfire_percent(s, hit.by, pos, crew_at)),
            None => 0,
        };
        // *Crossfire*'s other half: a bot's hit, or another player's, with
        // a holder standing opposite it round the machine.
        if slot.is_none() || hit.by.is_some_and(|b| (b as u32) < players) {
            percent = percent.saturating_add(self.crossfire_partner(hit.by, pos, crew_at));
        }
        // *Spotter*'s mark on this machine, whoever hits it.
        let mark = self
            .run
            .relics
            .spotted
            .iter()
            .filter(|&&(_, b, until)| b == body && until > now)
            .find_map(|&(s, _, _)| {
                relic::rule_of(self.relics_of(s), |r| match r {
                    Rule::Spotter { damage_percent, .. } => Some(damage_percent),
                    _ => None,
                })
            });
        percent = percent.saturating_add(mark.unwrap_or(0));
        percent = percent.saturating_add(exposed);
        let mut damage = hit.damage * relic::factor(percent) as f32;
        // *Total Teardown*: a hit on a limb already gone tears into the
        // chassis for more.
        if tear
            && let Some(more) = relic::rule_of(&held, |r| match r {
                Rule::TearIntoChassis { damage_percent } => Some(damage_percent),
                _ => None,
            })
        {
            damage *= relic::factor(more) as f32;
        }

        // What is noted: this holder's *Spotter* mark, and whether the last
        // hit on the machine came from the side (for *Signal Scrambler*'s
        // kill).
        if let Some(s) = slot
            && let Some(seconds) = relic::rule_of(&held, |r| match r {
                Rule::Spotter { seconds, .. } => Some(seconds),
                _ => None,
            })
        {
            let until = now + Self::relic_minutes(seconds);
            let spotted = &mut self.run.relics.spotted;
            spotted.retain(|&(t, _, _)| t != s);
            spotted.push((s, body, until));
        }
        let flanks = &mut self.run.relics.flanked;
        flanks.retain(|&b| b != body);
        if flanked {
            flanks.push(body);
        }
        (part, damage)
    }

    /// *Crossfire* for the holder shooting: its share when a crewmate on
    /// its feet stands opposite it round the machine at `pos`, both within
    /// the relic's tiles.
    fn crossfire_percent(
        &self,
        slot: u32,
        by: Option<usize>,
        pos: Vec2,
        crew_at: &[Option<Vec2>],
    ) -> i32 {
        let Some((percent, apart_cos, tiles)) = self.crossfire_of(slot) else {
            return 0;
        };
        let Some(me) = by.and_then(|b| crew_at.get(b).copied().flatten()) else {
            return 0;
        };
        let opposite = crew_at.iter().enumerate().any(|(c, at)| {
            Some(c) != by && at.is_some_and(|at| Self::opposite(pos, me, at, apart_cos, tiles))
        });
        if opposite { percent } else { 0 }
    }

    /// *Crossfire* for the crewmate: a player holding it, not the shooter,
    /// standing opposite the shooter round the machine.
    fn crossfire_partner(&self, by: Option<usize>, pos: Vec2, crew_at: &[Option<Vec2>]) -> i32 {
        let Some(shooter) = by.and_then(|b| crew_at.get(b).copied().flatten()) else {
            return 0;
        };
        (0..self.players())
            .filter(|&p| Some(p as usize) != by)
            .find_map(|p| {
                let (percent, apart_cos, tiles) = self.crossfire_of(p)?;
                let at = crew_at.get(p as usize).copied().flatten()?;
                Self::opposite(pos, shooter, at, apart_cos, tiles).then_some(percent)
            })
            .unwrap_or(0)
    }

    fn crossfire_of(&self, slot: u32) -> Option<(i32, f32, f32)> {
        relic::rule_of(self.relics_of(slot), |r| match r {
            Rule::Crossfire {
                damage_percent,
                apart_cos,
                tiles,
            } => Some((damage_percent, apart_cos, tiles)),
            _ => None,
        })
    }

    /// Whether `a` and `b` stand on opposite sides of `pos` — the bearings
    /// further apart than `apart_cos` — both within `tiles` of it.
    fn opposite(pos: Vec2, a: Vec2, b: Vec2, apart_cos: f32, tiles: f32) -> bool {
        let reach = tiles * shipdesign::TILE as f32;
        let (da, db) = (a - pos, b - pos);
        if da.len() > reach || db.len() > reach {
            return false;
        }
        let (ua, ub) = (da.normalize_or_zero(), db.normalize_or_zero());
        ua != Vec2::ZERO && ub != Vec2::ZERO && ua.dot(ub) < apart_cos
    }

    // --- kills and pay -----------------------------------------------------------

    /// What task 118's relics make of one machine destroyed, beside the
    /// bounty `machine_kills` already pays: *Signal Scrambler* on one from
    /// the side, and *Squad Morale* on every one, whoever destroyed it
    /// (task 142; it was the holder's own and the bots').
    pub(super) fn relics_on_a_kill(&mut self, kill: &MachineKill, events: &mut Vec<WorldEvent>) {
        let players = self.players();
        let slot = kill.by.map(|b| b as u32).filter(|&b| b < players);
        if let Some(s) = slot
            && kill.flanked
        {
            self.relic_trigger(s, Trigger::FlankKill, events);
        }
        for p in 0..players {
            self.relic_trigger(p, Trigger::CrewKill, events);
        }
    }

    /// *Hazard Pay*: the site just cleared pays the relic's holder its
    /// money over the players, once a relic held, into their own wallet.
    /// Part of `settle_clear`.
    pub(super) fn relics_pay_the_clear(&mut self, events: &mut Vec<WorldEvent>) {
        for slot in 0..self.players() {
            let Some(money) = relic::rule_of(self.relics_of(slot), |r| match r {
                Rule::SitePay { money } => Some(money),
                _ => None,
            }) else {
                continue;
            };
            // Shared by the players (task 142): each has money of their own.
            let share = money / Money::from(self.players().max(1));
            self.credit(slot, share);
            events.push(WorldEvent::RelicFired {
                who: slot,
                relic: Relic::HazardPay.code(),
            });
        }
    }

    // --- a crewmate down ------------------------------------------------------------

    /// Which of the crew were down — alive and downed — before the rooms
    /// stepped, and each one's share of its health then: what
    /// [`World::settle_relic_downs`] reads a fall and a fall low (*Lifeline*,
    /// task 142) off.
    pub(crate) fn downs_before_the_step(&self) -> Vec<(bool, f32)> {
        let room = &self.aboard.room;
        (0..self.aboard.crew_count())
            .map(|who| {
                let at = who as usize;
                (
                    room.is_alive(at) && room.is_down(at),
                    self.health_share(who),
                )
            })
            .collect()
    }

    /// After `settle_relics`: when each player's Bim last went down (*Clot
    /// Booster*, kept after it is revived), and every player's Bim that
    /// fell under *Lifeline*'s share of its health this step said to every
    /// player (`Trigger::PlayerLow`) — a bot's never.
    pub(crate) fn settle_relic_downs(
        &mut self,
        before: &[(bool, f32)],
        events: &mut Vec<WorldEvent>,
    ) {
        if !self.any_relics() {
            return;
        }
        let now = self.mission_minutes();
        let players = self.players();
        let crew = self.aboard.crew_count();
        for who in 0..players.min(crew) {
            let at = who as usize;
            let down = self.aboard.room.is_alive(at) && self.aboard.room.is_down(at);
            let downed = &mut self.run.relics.downed_at;
            if downed.len() <= at {
                downed.resize(at + 1, None);
            }
            if down && !before.get(at).is_some_and(|b| b.0) {
                downed[at] = Some(now);
            }
        }
        // A player's Bim fallen under the line this step: it was over it
        // (or at it) before the rooms stepped and is under it now.
        let line = data::LIFELINE_BELOW_PERCENT as f32 / 100.0;
        for c in 0..players.min(crew) {
            let was = before.get(c as usize).map_or(0.0, |b| b.1);
            if !self.aboard.room.is_alive(c as usize) || was < line || self.health_share(c) >= line
            {
                continue;
            }
            for p in 0..players {
                self.relic_trigger_on(p, Trigger::PlayerLow, Some(c), events);
            }
        }
    }

    /// *Lifeline* (task 142): holder `who`, alive, and the player's Bim
    /// `other` that fell low within `tiles` of it — or `who` itself — each
    /// given a shield of `hp` hit points for `seconds`. True when one was.
    pub(super) fn relic_shield(
        &mut self,
        who: u32,
        other: Option<u32>,
        tiles: f32,
        hp: f32,
        seconds: f32,
    ) -> bool {
        let Some(other) = other else {
            return false;
        };
        if !self.aboard.room.is_alive(who as usize)
            || (other != who && !self.within_tiles(who, other, tiles))
        {
            return false;
        }
        self.aboard.room.set_shield(who as usize, hp, seconds);
        self.aboard.room.set_shield(other as usize, hp, seconds);
        true
    }

    /// *Rally Point*: every crewmate down within `tiles` of `who` up again
    /// at `health_percent` of its health. True when one was.
    pub(super) fn relic_rally_up(&mut self, who: u32, tiles: f32, health_percent: u32) -> bool {
        let mut any = false;
        for c in 0..self.aboard.crew_count() {
            let at = c as usize;
            if c == who
                || !self.aboard.room.is_alive(at)
                || !self.aboard.room.is_down(at)
                || !self.within_tiles(who, c, tiles)
            {
                continue;
            }
            if self
                .aboard
                .room
                .bring_round(at, health_percent as f32 / 100.0)
            {
                if let Some(down) = self.crew_down.get_mut(at) {
                    *down = false;
                }
                any = true;
            }
        }
        any
    }

    // --- the trader ---------------------------------------------------------------------

    /// *Trade License*: what a trader's `price` comes to for player `slot`
    /// — the best of the players' discounts ([`data::TRADE_LICENSE_PERCENT`]
    /// for everybody while anybody holds it), and the holder's own
    /// [`data::TRADE_LICENSE_HOLDER_PERCENT`] (task 142) — whole euros
    /// rounded down.
    pub fn trader_discount_for(&self, slot: u32, price: Money) -> Money {
        let crew = (0..self.players())
            .map(|s| self.relic_percent(s, Stat::TraderPrices))
            .min()
            .unwrap_or(0)
            .min(0);
        let own = if self.relics_of(slot).contains(&Relic::TradeLicense) {
            -data::TRADE_LICENSE_HOLDER_PERCENT
        } else {
            0
        };
        let percent = crew.min(own);
        if percent == 0 {
            return price;
        }
        price - price * Money::from(percent.unsigned_abs()) / 100
    }

    /// What the trader's relic costs player `slot`: [`crate::trader::relic_price`]
    /// with *Trade License*, then the players' share.
    pub fn trader_relic_price(&self, slot: u32, relic: Relic) -> Money {
        self.trader_share(
            self.trader_discount_for(slot, self.rewards.relic_price_of(relic.tier() as u32)),
        )
    }

    /// Whether the crew may restock the trader they are at (*Restock
    /// Codes*): at a trader, a player holding it, not done this visit.
    pub fn can_restock(&self) -> Result<(), Refusal> {
        if !self.at_trader() {
            return Err(Refusal::NotAtATrader);
        }
        let holds = (0..self.players()).any(|s| {
            relic::rule_of(self.relics_of(s), |r| (r == Rule::Restock).then_some(())).is_some()
        });
        if !holds {
            return Err(Refusal::NoRestock);
        }
        if self.run.relics.restocked {
            return Err(Refusal::Restocked);
        }
        Ok(())
    }
}

impl World {
    /// [`Command::Restock`]: the shelf of the pressing player's own trader
    /// rolled again — its weapons and its armour, never its relic — off the
    /// galaxy's seed, the site and the world clock's minute, so every
    /// machine rolls the same. A slot bought before is a thing again.
    pub(super) fn restock(&mut self, slot: u32, events: &mut Vec<WorldEvent>) {
        if let Err(why) = self.can_restock() {
            events.push(refused(slot, why));
            return;
        }
        let Some(site) = self.trader_here(slot).map(|t| t.site) else {
            events.push(refused(slot, Refusal::NotAtATrader));
            return;
        };
        let shelf = crate::trader::reroll_shelf(
            self.galaxy_seed,
            site.star,
            site.station,
            self.clock_minutes.to_bits() ^ (u64::from(slot) << 56),
        );
        if let Some(trader) = self
            .run
            .traders
            .iter_mut()
            .find(|t| t.site == site && t.owner == slot)
        {
            trader.shelf = shelf.into_iter().map(Some).collect();
        }
        self.run.relics.restocked = true;
        events.push(WorldEvent::Restocked { slot });
    }
}
