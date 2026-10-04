//! What a player's Bim's **items** do in a mission (October 2026,
//! `crate::items`, `bims::module`): the *Blink Drive*'s key, the
//! *Executioner*'s crit beside the soldier's Weak Spot, and the *Reactor
//! Heart*'s regeneration. The *Override Core* is read where the ultimate's
//! rank is (`World::rank_of`), and the Heart's health by the room off the
//! loadout (`Gear::max_health`).
//!
//! A child of `crate::world`, as `relics.rs` is. **Nothing here draws
//! from a stream**, and nothing here runs for a crew carrying no item, so
//! a run without them plays as it did.

use super::*;
use bims::module::{ITEM_SLOTS, Module, ModuleKind};

impl World {
    // --- reading -------------------------------------------------------------

    /// The six item slots of crew member `who`, empty past the crew.
    pub fn items_of(&self, who: u32) -> [Option<Module>; ITEM_SLOTS] {
        if who < self.aboard.crew_count() {
            self.aboard.room.gear(who as usize).items
        } else {
            [None; ITEM_SLOTS]
        }
    }

    /// Whether `who` carries an item of `kind`.
    pub fn carries_item(&self, who: u32, kind: ModuleKind) -> bool {
        self.items_of(who).iter().flatten().any(|m| m.kind == kind)
    }

    /// Whether any player's Bim carries an item at all: nothing below does
    /// anything for a crew that carries none.
    fn any_items(&self) -> bool {
        (0..self.players().min(self.aboard.crew_count()))
            .any(|who| self.items_of(who).iter().any(Option::is_some))
    }

    /// What `who`'s weapon hits may be critical by: the soldier's Weak
    /// Spot and every *Executioner* carried, each rolling on its own as
    /// Dota's crits do — the chance that any comes up, and the biggest of
    /// their multiples ([`bims::module::combine_crits`]). `None` for a
    /// body with neither.
    pub fn crit_of(&self, who: u32) -> Option<(f32, f32)> {
        let weak_spot = if self.is_soldier(who) {
            let rank = self.rank_of(who, class::SLOT_C);
            class::by_rank(class::WEAK_SPOT_CHANCE, rank)
                .zip(class::by_rank(class::WEAK_SPOT_DAMAGE, rank))
        } else {
            None
        };
        let items = self.items_of(who);
        bims::module::combine_crits(
            weak_spot
                .into_iter()
                .chain(items.iter().flatten().filter_map(|m| m.crit())),
        )
    }

    // --- the Blink Drive -----------------------------------------------------

    /// Minutes of the mission clock in `seconds` of it.
    fn item_minutes(seconds: f64) -> f64 {
        seconds * time::MINUTES_PER_SECOND
    }

    /// Seconds before `who`'s item in slot `index` may be used again;
    /// nought when it is ready.
    pub fn item_cooldown_left(&self, who: u32, index: usize) -> f64 {
        let now = self.mission_minutes();
        self.run
            .items
            .ready_at
            .iter()
            .filter(|&&(w, i, at)| w == who && i as usize == index && at > now)
            .map(|&(_, _, at)| (at - now) / time::MINUTES_PER_SECOND)
            .fold(0.0, f64::max)
    }

    /// The whole of that item's cooldown, in seconds: what the box's sweep
    /// is a share of. Nought for an item with none.
    pub fn item_cooldown(&self, who: u32, index: usize) -> f64 {
        self.items_of(who)
            .get(index)
            .copied()
            .flatten()
            .map_or(0.0, |m| f64::from(m.cooldown()))
    }

    /// Seconds before a hit taken stops locking `who`'s *Blink Drive*;
    /// nought when it is free.
    pub fn blink_locked_left(&self, who: u32) -> f64 {
        let Some(hurt) = self.run.items.hurt_at.get(who as usize).copied().flatten() else {
            return 0.0;
        };
        let free = hurt + Self::item_minutes(f64::from(bims::module::BLINK_HIT_LOCK_SECONDS));
        ((free - self.mission_minutes()) / time::MINUTES_PER_SECOND).max(0.0)
    }

    /// Seconds before `who`'s *Reactor Heart* regenerates at its quiet
    /// rate — [`bims::module::HEART_QUIET_SECONDS`] after the last hit
    /// taken; nought once it does. What the app greys the item's box by.
    pub fn quiet_regen_left(&self, who: u32) -> f64 {
        let Some(hurt) = self.run.items.hurt_at.get(who as usize).copied().flatten() else {
            return 0.0;
        };
        let quiet = hurt + Self::item_minutes(f64::from(bims::module::HEART_QUIET_SECONDS));
        ((quiet - self.mission_minutes()) / time::MINUTES_PER_SECOND).max(0.0)
    }

    /// Whether player `slot` may use the item in its slot `index` now, and
    /// the item if so: a mission under way, its own Bim fit to act, an
    /// active item there, not cooling down and — a blink — not locked by a
    /// hit. What the app greys the box by and [`Command::UseItem`] asks.
    pub fn can_use_item(&self, slot: u32, index: u32) -> Result<Module, Refusal> {
        if slot >= self.players() {
            return Err(Refusal::NotAPlayer);
        }
        let Some(item) = self.items_of(slot).get(index as usize).copied().flatten() else {
            return Err(Refusal::NoSuchItem);
        };
        if !item.kind.active() {
            return Err(Refusal::NoSuchItem);
        }
        if !self.in_mission() || !self.fit_to_act(slot) {
            return Err(Refusal::OutOfReach);
        }
        if self.item_cooldown_left(slot, index as usize) > 0.0 {
            return Err(Refusal::CoolingDown);
        }
        if item.is_blink() && self.blink_locked_left(slot) > 0.0 {
            return Err(Refusal::BlinkLocked);
        }
        Ok(item)
    }

    /// [`Command::UseItem`]: the item in player `slot`'s item slot `index`
    /// used at the crew's room point `at` (room units). A *Blink Drive*
    /// puts the Bim at [`bims::game::Game::blink_spot`] — the point, or as
    /// far towards it as the drive reaches on ground the crew see — and
    /// starts its cooldown; `NowhereToBlink` where there is no such
    /// ground.
    pub(super) fn use_item(
        &mut self,
        slot: u32,
        index: u32,
        at: (i32, i32),
        events: &mut Vec<WorldEvent>,
    ) -> Result<(), Refusal> {
        let item = self.can_use_item(slot, index)?;
        if item.is_blink() {
            let to = bims::math::vec2(at.0 as f32, at.1 as f32);
            let reach = item.blink_range() * bims::room::TILE;
            let Some(spot) = self.aboard.room.blink_spot(slot as usize, to, reach) else {
                return Err(Refusal::NowhereToBlink);
            };
            self.aboard.room.blink(slot as usize, spot);
            // A blink walks away from a throw the Bim was walking out to
            // make, as the keys do.
            self.throws.retain(|p| p.who != slot);
            events.push(WorldEvent::Blinked { who: slot });
        }
        let now = self.mission_minutes();
        match item.kind {
            ModuleKind::BlinkDrive => {}
            // Every crewmate on its feet within reach healed, the holder
            // too, through the medic's aura as every heal is.
            ModuleKind::FieldMender => {
                for who in self.crew_within(slot, bims::module::MENDER_TILES) {
                    if !self.aboard.room.is_downed(who as usize) {
                        self.heal_crew(who, item.mend());
                    }
                }
                events.push(WorldEvent::ItemUsed {
                    who: slot,
                    kind: item.kind.code(),
                });
            }
            // Every class cooldown ready and every charge in hand, and the
            // holder's other items' cooldowns with them, Dota's Refresher.
            ModuleKind::ResetCapacitor => {
                self.cooldowns_less(slot, 1.0e9);
                for charge in Charge::ALL {
                    let full = self.charges(slot, charge);
                    if full > self.charges_of(slot, charge) {
                        self.set_charges_held(slot, charge, full);
                    }
                }
                self.run.items.ready_at.retain(|&(w, _, _)| w != slot);
                events.push(WorldEvent::ItemUsed {
                    who: slot,
                    kind: item.kind.code(),
                });
            }
            ModuleKind::AblativeShell => {
                let until = now + Self::item_minutes(f64::from(item.shell_seconds()));
                let shells = &mut self.run.items.shell_until;
                shells.retain(|&(w, _)| w != slot);
                shells.push((slot, until));
                events.push(WorldEvent::ItemUsed {
                    who: slot,
                    kind: item.kind.code(),
                });
            }
            _ => return Err(Refusal::NoSuchItem),
        }
        let until = now + Self::item_minutes(f64::from(item.cooldown()));
        let clocks = &mut self.run.items.ready_at;
        clocks.retain(|&(w, i, _)| !(w == slot && i == index));
        clocks.push((slot, index, until));
        Ok(())
    }

    /// A probe's items (`BIMS_ITEMS`): `items` into crew member `who`'s
    /// item slots in order, whatever was there, the rest left; nothing for
    /// a bot, which carries none.
    pub fn give_items_for_probe(&mut self, who: u32, items: &[Module]) {
        if who >= self.players() || who >= self.aboard.crew_count() {
            return;
        }
        let mut gear = self.aboard.room.gear(who as usize);
        for (slot, &item) in gear.items.iter_mut().zip(items) {
            *slot = Some(item);
        }
        self.aboard.room.issue(who as usize, gear);
    }

    // --- step two: the passives and the shell (October 2026) -------------------

    /// The per cent `who`'s *Coolant Loop*s take off its class's
    /// cooldowns: what `World::relic_percent` adds to the relics' for
    /// `Stat::Cooldowns`, so every class cooldown reads it.
    pub fn item_cooldown_cut(&self, who: u32) -> i32 {
        if who >= self.players() || who >= self.aboard.crew_count() {
            return 0;
        }
        self.aboard.room.gear(who as usize).item_cooldown_cut()
    }

    /// Seconds `who`'s *Ablative Shell* has left; nought with none on.
    pub fn shell_left(&self, who: u32) -> f64 {
        let now = self.mission_minutes();
        self.run
            .items
            .shell_until
            .iter()
            .filter(|&&(w, until)| w == who && until > now)
            .map(|&(_, until)| (until - now) / time::MINUTES_PER_SECOND)
            .fold(0.0, f64::max)
    }

    /// What `who`'s items do to its skill (`World::skill_of`, after the
    /// relics): a *Steady Grip*'s fire rate, a *Long Barrel*'s tiles of
    /// range, and while an *Ablative Shell* is on the damage taken down
    /// and nothing stripped. The skill as it was for a Bim carrying none.
    pub(super) fn lift_by_items(&self, who: u32, skill: &mut bims::combat::Skill) {
        if who >= self.players() || who >= self.aboard.crew_count() {
            return;
        }
        let gear = self.aboard.room.gear(who as usize);
        let rate = gear.item_fire_rate_percent();
        if rate != 0 {
            skill.fire_rate *= crate::relic::factor(rate) as f32;
        }
        skill.range += gear.item_range_tiles();
        if self.shell_left(who) > 0.0 {
            skill.damage_taken *= bims::module::SHELL_DAMAGE_TAKEN;
            skill.unstrippable = true;
        }
    }

    /// Every crew weapon hit that landed on an enemy this step — who
    /// fired it (a crew index), which enemy (a body index of the
    /// residents' room: a machine, or one of the Manufacturers' people)
    /// and what it did — through the items: a *Leech Capacitor*'s share
    /// back as hit points, and an *Arc Coil*'s count, every
    /// [`bims::module::ARC_EVERY`]th hit arcing from the enemy struck to
    /// the nearest others within [`bims::module::ARC_REACH_TILES`] — a
    /// machine standing or one of their people on its feet, the nearest
    /// first, the lower index on a tie, so every copy arcs alike.
    pub(crate) fn items_on_enemy_hits(&mut self, landed: &[(Option<usize>, usize, f32)]) {
        if !self.any_items() {
            return;
        }
        let players = self.players().min(self.aboard.crew_count());
        for &(by, struck, damage) in landed {
            let Some(slot) = by.map(|b| b as u32).filter(|&b| b < players) else {
                continue;
            };
            let gear = self.aboard.room.gear(slot as usize);
            let leech = gear.item_leech();
            if leech > 0.0 && damage > 0.0 {
                // A share of the damage done, in points: the one heal not
                // a share of the bar (`heal_crew`).
                self.aboard.room.heal(slot as usize, damage * leech);
            }
            let Some((targets, arc_damage)) = gear.item_arc() else {
                continue;
            };
            let hits = &mut self.run.items.arc_hits;
            if hits.len() <= slot as usize {
                hits.resize(slot as usize + 1, 0);
            }
            hits[slot as usize] += 1;
            if hits[slot as usize] % bims::module::ARC_EVERY != 0 {
                continue;
            }
            let Some(residents) = self.residents.as_mut() else {
                continue;
            };
            let room = &mut residents.aboard.room;
            if struck >= room.body_count() as usize {
                continue;
            }
            let from = room.body_pos(struck);
            let bims = room.crew_count() as usize;
            let reach = bims::module::ARC_REACH_TILES * bims::room::TILE;
            let enemy = |j: usize| {
                room.is_alive(j) && (j >= bims || (room.is_manufacturer(j) && !room.is_downed(j)))
            };
            let mut near: Vec<(f32, usize)> = (0..room.body_count() as usize)
                .filter(|&j| j != struck && enemy(j))
                .filter_map(|j| {
                    let far = (room.body_pos(j) - from).len();
                    (far <= reach).then_some((far, j))
                })
                .collect();
            near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            let mut shown = Vec::new();
            for &(_, j) in near.iter().take(targets) {
                match j.checked_sub(bims) {
                    Some(i) => {
                        room.strike_droid(i, bims::droid::DroidPart::Chassis, arc_damage);
                    }
                    None => {
                        room.strike(j, arc_damage, false);
                    }
                }
                room.arc_light(struck, j);
                shown.push((j as u32, arc_damage, false, Some(slot)));
            }
            for &(body, ..) in &shown {
                if let Some(last) = residents.last_hit_by.get_mut(body as usize) {
                    *last = Some(slot as usize);
                }
            }
            self.shown_hits.extend(shown);
        }
    }

    // --- the Reactor Heart -----------------------------------------------------

    /// The items' stage, after the rooms have stepped: every player's Bim
    /// hit this step noted (`hits_before` is
    /// [`World::hits_before_the_step`]) — what locks a blink and slows a
    /// Heart's regeneration — then every *Reactor Heart* carried by a Bim
    /// on its feet puts its hit points back: the quiet rate once nothing
    /// has hit it for [`bims::module::HEART_QUIET_SECONDS`], the plain one
    /// before. Through [`World::heal_crew`].
    pub(crate) fn settle_items(&mut self, hits_before: &[u32]) {
        if !self.any_items() {
            return;
        }
        let now = self.mission_minutes();
        let players = self.players().min(self.aboard.crew_count());
        let hurt = &mut self.run.items.hurt_at;
        if hurt.len() < players as usize {
            hurt.resize(players as usize, None);
        }
        for who in 0..players as usize {
            let hit = hits_before
                .get(who)
                .is_some_and(|&before| self.aboard.room.hits_taken(who) > before);
            if hit {
                self.run.items.hurt_at[who] = Some(now);
            }
        }
        let seconds = (data::STEP_MINUTES / time::MINUTES_PER_SECOND) as f32;
        for who in 0..players {
            let at = who as usize;
            if !self.aboard.room.is_alive(at) || self.aboard.room.is_downed(at) {
                continue;
            }
            let rate = self.item_regen_now(who);
            if rate <= 0.0 {
                continue;
            }
            self.heal_crew(who, rate * seconds);
        }
    }

    /// Hit points a second `who`'s items regenerate him by now: the
    /// quiet rate once nothing has hit him for
    /// [`bims::module::HEART_QUIET_SECONDS`], the plain one before —
    /// nought without a *Reactor Heart*. What a
    /// medic's beam adds to its patients' heal from its fourth rank
    /// ([`World::beam_item_rate`]).
    pub fn item_regen_now(&self, who: u32) -> f32 {
        let at = who as usize;
        if at >= self.aboard.crew_count() as usize {
            return 0.0;
        }
        let (plain, calm) = self.aboard.room.gear(at).item_regen();
        if plain <= 0.0 && calm <= 0.0 {
            return 0.0;
        }
        let now = self.mission_minutes();
        let quiet = Self::item_minutes(f64::from(bims::module::HEART_QUIET_SECONDS));
        let unhurt = self
            .run
            .items
            .hurt_at
            .get(at)
            .copied()
            .flatten()
            .is_none_or(|t| now - t >= quiet);
        if unhurt { calm } else { plain }
    }
}
