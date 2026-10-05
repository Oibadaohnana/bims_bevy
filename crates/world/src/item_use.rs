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
            // A cloud at the pointer, or as far towards it as the launcher
            // reaches (October 2026).
            ModuleKind::SmokeLauncher => {
                let (radius, seconds) = item.smoke().unwrap_or((0.0, 0.0));
                let tile = shipdesign::TILE as f32;
                let from = self.aboard.room.body_pos(slot as usize);
                let to = bims::math::vec2(at.0 as f32, at.1 as f32);
                let reach = bims::module::SMOKE_RANGE_TILES * tile;
                let off = to - from;
                let at = if off.len() > reach {
                    from + off * (reach / off.len())
                } else {
                    to
                };
                self.run.items.smoke.push(crate::items::Smoke {
                    x: at.x,
                    y: at.y,
                    radius: radius * tile,
                    from: now,
                    until: now + Self::item_minutes(f64::from(seconds)),
                });
                events.push(WorldEvent::ItemUsed {
                    who: slot,
                    kind: item.kind.code(),
                });
            }
            // The crewmate at the pointer linked, within reach.
            ModuleKind::TetherLink => {
                let mate = self.tether_pick(slot, at)?;
                let tethers = &mut self.run.items.tethers;
                tethers.retain(|t| t.carrier != slot);
                tethers.push(crate::items::Tether {
                    carrier: slot,
                    mate,
                    share: item.tether_share(),
                    until: now + Self::item_minutes(f64::from(bims::module::TETHER_SECONDS)),
                });
                events.push(WorldEvent::ItemUsed {
                    who: slot,
                    kind: item.kind.code(),
                });
            }
            // A ghost of the carrier off on its walk to the pointer, cut
            // at the projector's reach.
            ModuleKind::DecoyProjector => {
                let to = bims::math::vec2(at.0 as f32, at.1 as f32);
                let Some(route) = self.aboard.room.ghost_route(slot as usize, to) else {
                    return Err(Refusal::NoWayThere);
                };
                let from = self.aboard.room.body_pos(slot as usize);
                let path = cut_walk(
                    from,
                    &route,
                    bims::module::DECOY_RANGE_TILES * shipdesign::TILE as f32,
                );
                let pace = bims::balance::MARCH_SPEED
                    * self.skill_of(slot).walk
                    * bims::module::DECOY_PACE;
                let heading = path
                    .first()
                    .map_or(0.0, |&(x, y)| (y - from.y).atan2(x - from.x));
                let ghosts = &mut self.run.items.ghosts;
                ghosts.retain(|g| g.owner != slot);
                ghosts.push(crate::items::Ghost {
                    owner: slot,
                    x: from.x,
                    y: from.y,
                    path,
                    pace,
                    heading,
                    stride: 0.0,
                    from: now,
                    until: now + Self::item_minutes(f64::from(bims::module::DECOY_SECONDS)),
                });
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

    /// The per cent more of every enemy's experience `who`'s best
    /// *Training Log* gives it (October 2026): two do not add.
    pub fn item_xp_percent(&self, who: u32) -> u32 {
        if who >= self.players() || who >= self.aboard.crew_count() {
            return 0;
        }
        self.aboard
            .room
            .gear(who as usize)
            .modules()
            .map(|m| m.xp_percent())
            .max()
            .unwrap_or(0)
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
        if who >= self.aboard.crew_count() {
            return;
        }
        // A crewmate a *Tether Link* holds takes its share less of every
        // hit (October 2026); the carrier takes it (`settle_tethers`).
        let share = self.tether_on(who).map_or(0.0, |t| t.share);
        if share > 0.0 {
            skill.damage_taken *= 1.0 - share;
        }
        // A bot near a *Targeting Uplink*'s carrier does more damage.
        if who >= self.players() {
            let lift = self.uplink_reaching(who);
            if lift != 0 {
                skill.damage *= crate::relic::factor(lift) as f32;
            }
            return;
        }
        let gear = self.aboard.room.gear(who as usize);
        let rate = gear.item_fire_rate_percent();
        if rate != 0 {
            skill.fire_rate *= crate::relic::factor(rate) as f32;
        }
        // An *Overcharger*: the weapon's own damage up. A crit is a
        // multiple of the flat damage, so `crit_extra` lifts that too.
        let damage = gear.item_damage_percent();
        if damage != 0 {
            skill.damage *= crate::relic::factor(damage) as f32;
        }
        skill.range += gear.item_range_tiles();
        if self.shell_left(who) > 0.0 {
            skill.damage_taken *= bims::module::SHELL_DAMAGE_TAKEN;
            skill.unstrippable = true;
        }
        // An *Adrenal Injector*'s rush: fire rate and pace.
        let rush = self.adrenal_percent_now(who);
        if rush != 0 {
            skill.fire_rate *= crate::relic::factor(rush) as f32;
            skill.walk *= crate::relic::factor(rush) as f32;
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
    pub(crate) fn settle_items(
        &mut self,
        hits_before: &[u32],
        tethered: &[(u32, f32)],
        events: &mut Vec<WorldEvent>,
    ) {
        if !self.any_items() {
            return;
        }
        self.settle_tethers(tethered);
        self.inject_adrenaline(events);
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

    // --- step three (October 2026) ---------------------------------------------

    /// The *Tether Link* holding crewmate `who`, the strongest of them.
    fn tether_on(&self, who: u32) -> Option<crate::items::Tether> {
        let now = self.mission_minutes();
        self.run
            .items
            .tethers
            .iter()
            .filter(|t| t.mate == who && t.until > now)
            .copied()
            .max_by(|a, b| a.share.total_cmp(&b.share))
    }

    /// Whom player `slot`'s *Tether Link* would hold at room point `at`:
    /// the crewmate under it, else the nearest one standing within a
    /// tile and a half of it — never the carrier, nobody downed — and
    /// within [`bims::module::TETHER_RANGE_TILES`] of the carrier.
    fn tether_pick(&self, slot: u32, at: (i32, i32)) -> Result<u32, Refusal> {
        let room = &self.aboard.room;
        let p = bims::math::vec2(at.0 as f32, at.1 as f32);
        let tile = shipdesign::TILE as f32;
        let up = |w: usize| w != slot as usize && room.is_alive(w) && !room.is_downed(w);
        let mate = room
            .crew_at(p.x, p.y)
            .filter(|&w| up(w) && w < self.aboard.crew_count() as usize)
            .or_else(|| {
                (0..self.aboard.crew_count() as usize)
                    .filter(|&w| up(w))
                    .map(|w| (w, (room.body_pos(w) - p).len()))
                    .filter(|&(_, far)| far <= 1.5 * tile)
                    .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
                    .map(|(w, _)| w)
            })
            .ok_or(Refusal::NotACrewmate)?;
        let far = (room.body_pos(mate) - room.body_pos(slot as usize)).len();
        if far > bims::module::TETHER_RANGE_TILES * tile {
            return Err(Refusal::OutOfItemRange);
        }
        Ok(mate as u32)
    }

    /// The per cent a *Targeting Uplink* puts on bot `who`'s damage: the
    /// best of every player on its feet carrying one within
    /// [`bims::module::UPLINK_TILES`] of it — two do not add.
    fn uplink_reaching(&self, who: u32) -> i32 {
        let room = &self.aboard.room;
        let at = room.body_pos(who as usize);
        let reach = bims::module::UPLINK_TILES * shipdesign::TILE as f32;
        (0..self.players().min(self.aboard.crew_count()))
            .filter(|&p| room.is_alive(p as usize) && !room.is_downed(p as usize))
            .filter(|&p| (room.body_pos(p as usize) - at).len() <= reach)
            .map(|p| {
                room.gear(p as usize)
                    .modules()
                    .map(|m| m.uplink_percent())
                    .max()
                    .unwrap_or(0)
            })
            .max()
            .unwrap_or(0)
    }

    /// What `who`'s *Overcharger*s multiply its weapon's damage by: what
    /// `crit_extra` lifts the flat damage a crit is a multiple of by.
    pub fn item_damage_factor(&self, who: u32) -> f32 {
        if who >= self.players() || who >= self.aboard.crew_count() {
            return 1.0;
        }
        let percent = self.aboard.room.gear(who as usize).item_damage_percent();
        crate::relic::factor(percent) as f32
    }

    /// The per cent an *Adrenal Injector*'s rush puts on `who`'s fire rate
    /// and pace right now: nought with none running.
    fn adrenal_percent_now(&self, who: u32) -> i32 {
        let now = self.mission_minutes();
        if !self
            .run
            .items
            .adrenal_until
            .iter()
            .any(|&(w, until)| w == who && until > now)
        {
            return 0;
        }
        self.items_of(who)
            .iter()
            .flatten()
            .filter_map(|m| m.adrenal())
            .map(|(percent, _)| percent)
            .max()
            .unwrap_or(0)
    }

    /// Seconds `who`'s *Adrenal Injector*'s rush has left; nought with
    /// none running.
    pub fn adrenal_left(&self, who: u32) -> f64 {
        let now = self.mission_minutes();
        self.run
            .items
            .adrenal_until
            .iter()
            .filter(|&&(w, until)| w == who && until > now)
            .map(|&(_, until)| (until - now) / time::MINUTES_PER_SECOND)
            .fold(0.0, f64::max)
    }

    /// Every player's Bim on its feet under [`bims::module::ADRENAL_BELOW`]
    /// of its bar with an *Adrenal Injector* off its cooldown: the rush on
    /// for its seconds and the cooldown started, said as the item used.
    fn inject_adrenaline(&mut self, events: &mut Vec<WorldEvent>) {
        let now = self.mission_minutes();
        let players = self.players().min(self.aboard.crew_count());
        for who in 0..players {
            let room = &self.aboard.room;
            if !room.is_alive(who as usize) || room.is_downed(who as usize) {
                continue;
            }
            if self.health_share(who) >= bims::module::ADRENAL_BELOW {
                continue;
            }
            let found = self
                .items_of(who)
                .iter()
                .enumerate()
                .find_map(|(i, m)| m.and_then(|m| m.adrenal().map(|r| (i, r))));
            let Some((index, (_, seconds))) = found else {
                continue;
            };
            if self.item_cooldown_left(who, index) > 0.0 {
                continue;
            }
            let rushes = &mut self.run.items.adrenal_until;
            rushes.retain(|&(w, _)| w != who);
            rushes.push((who, now + Self::item_minutes(f64::from(seconds))));
            let clocks = &mut self.run.items.ready_at;
            clocks.retain(|&(w, i, _)| !(w == who && i as usize == index));
            clocks.push((
                who,
                index as u32,
                now + Self::item_minutes(f64::from(bims::module::ADRENAL_COOLDOWN_SECONDS)),
            ));
            events.push(WorldEvent::ItemUsed {
                who,
                kind: ModuleKind::AdrenalInjector.code(),
            });
        }
    }

    /// What every *Tether Link* took this step: each crewmate's hit
    /// points before the rooms stepped against now, the loss being what
    /// got through its share less, and the carrier drained the rest —
    /// past its armour, as the hits on the crewmate were already through
    /// theirs. A link whose carrier or crewmate is down or dead lets go.
    fn settle_tethers(&mut self, tethered: &[(u32, f32)]) {
        let now = self.mission_minutes();
        for &(mate, before) in tethered {
            let Some(tether) = self.tether_on(mate) else {
                continue;
            };
            let room = &self.aboard.room;
            let after = if room.is_alive(mate as usize) {
                room.health(mate as usize)
            } else {
                0.0
            };
            let loss = before - after;
            let carrier = tether.carrier as usize;
            if loss > 0.0
                && tether.share < 1.0
                && room.is_alive(carrier)
                && !room.is_downed(carrier)
            {
                let taken = loss * tether.share / (1.0 - tether.share);
                self.aboard.room.drain(carrier, taken);
            }
        }
        let room = &self.aboard.room;
        let up = |w: u32| room.is_alive(w as usize) && !room.is_downed(w as usize);
        self.run
            .items
            .tethers
            .retain(|t| t.until > now && up(t.carrier) && up(t.mate));
    }

    /// The items' stage before the rooms step (October 2026): the clouds
    /// run out let go, every ghost walked on its step and let go when its
    /// time is up, and the rooms told — the residents' room the clouds
    /// that stop its people's sight, the crew's the clouds, the links and
    /// the ghosts to draw. Hands back each tethered crewmate's hit points
    /// now, for [`World::settle_items`] to weigh the step's hits by.
    pub(crate) fn hand_the_rooms_the_items(&mut self) -> Vec<(u32, f32)> {
        let now = self.mission_minutes();
        let items = &mut self.run.items;
        items.smoke.retain(|c| c.until > now);
        items.ghosts.retain(|g| g.until > now);
        items.tethers.retain(|t| t.until > now);
        let seconds = (data::STEP_MINUTES / time::MINUTES_PER_SECOND) as f32;
        for ghost in &mut items.ghosts {
            walk_ghost(ghost, seconds);
        }
        let second = time::MINUTES_PER_SECOND;
        let clouds = |c: &crate::items::Smoke, at: bims::math::Vec2| bims::game::SmokeCloud {
            at,
            radius: c.radius,
            age: ((now - c.from) / second) as f32,
            // Thinning over the last two seconds, as its going out is heard.
            thick: (((c.until - now) / second) as f32 / 2.0).clamp(0.0, 1.0),
        };
        let mine: Vec<bims::game::SmokeCloud> = items
            .smoke
            .iter()
            .map(|c| clouds(c, bims::math::vec2(c.x, c.y)))
            .collect();
        let ghosts: Vec<bims::game::GhostLook> = items
            .ghosts
            .iter()
            .map(|g| bims::game::GhostLook {
                owner: g.owner as usize,
                at: bims::math::vec2(g.x, g.y),
                heading: g.heading,
                stride: g.stride,
                speed: if g.path.is_empty() { 0.0 } else { g.pace },
                fade: (((g.until - now) / second) as f32).clamp(0.0, 1.0)
                    * (((now - g.from) / second) as f32 * 4.0).clamp(0.0, 1.0),
            })
            .collect();
        let links: Vec<(usize, usize)> = items
            .tethers
            .iter()
            .map(|t| (t.carrier as usize, t.mate as usize))
            .collect();
        let tethered: Vec<(u32, f32)> = items
            .tethers
            .iter()
            .map(|t| t.mate)
            .map(|mate| (mate, self.aboard.room.health(mate as usize)))
            .collect();
        // The residents' room's clouds, in its own units, where the joined
        // deck can say where they are.
        let theirs: Vec<bims::game::SmokeCloud> = if self.aboard.is_joined() {
            let shift = self.residents.as_ref().map(|r| r.aboard.offset);
            self.run
                .items
                .smoke
                .iter()
                .filter_map(|c| {
                    let p = self.aboard.to_station(dvec2(c.x as f64, c.y as f64))?;
                    let p = p.add(shift?);
                    Some(clouds(c, bims::math::vec2(p.x as f32, p.y as f32)))
                })
                .collect()
        } else {
            Vec::new()
        };
        // Who is under a rush, and which bots an uplink lifts, to draw.
        let rushing: Vec<(usize, f32)> = (0..self.players().min(self.aboard.crew_count()))
            .map(|who| (who as usize, self.adrenal_left(who) as f32))
            .filter(|&(_, left)| left > 0.0)
            .collect();
        let uplinked: Vec<usize> = if self.any_items() {
            (self.players()..self.aboard.crew_count())
                .filter(|&who| {
                    self.aboard.room.is_alive(who as usize) && self.uplink_reaching(who) > 0
                })
                .map(|who| who as usize)
                .collect()
        } else {
            Vec::new()
        };
        self.aboard.room.set_smoke(mine, false);
        self.aboard.room.set_ghosts(ghosts);
        self.aboard.room.set_tethers(links);
        self.aboard.room.set_item_auras(rushing, uplinked);
        if let Some(residents) = &mut self.residents {
            residents.aboard.room.set_smoke(theirs, true);
        }
        tethered
    }

    /// Every smoke cloud hanging, a key of its own and its seconds left:
    /// what the app's sound hisses by and times the going out to.
    pub fn smoke_left(&self) -> Vec<(u64, f64)> {
        let now = self.mission_minutes();
        self.run
            .items
            .smoke
            .iter()
            .map(|c| {
                let key =
                    c.from.to_bits() ^ (u64::from(c.x.to_bits()) << 32) ^ u64::from(c.y.to_bits());
                (key, (c.until - now) / time::MINUTES_PER_SECOND)
            })
            .collect()
    }

    /// Whether crew member `who` stands in a smoke cloud: nobody's target
    /// while it does (`visit`).
    pub(crate) fn in_smoke(&self, who: u32) -> bool {
        if who >= self.aboard.crew_count() || self.run.items.smoke.is_empty() {
            return false;
        }
        let at = self.aboard.room.body_pos(who as usize);
        self.run
            .items
            .smoke
            .iter()
            .any(|c| (bims::math::vec2(c.x, c.y) - at).len() <= c.radius)
    }

    /// The decoys' ghosts as the enemy's targets, in the station's own
    /// units, each with its owner's weapon: what `visit` puts after the
    /// sentries, every one taunting within
    /// [`bims::module::DECOY_TAUNT_TILES`].
    pub(crate) fn ghost_targets(&self) -> Vec<Option<(DVec2, Weapon)>> {
        self.run
            .items
            .ghosts
            .iter()
            .map(|g| {
                let weapon = self
                    .aboard
                    .room
                    .weapon(g.owner as usize)
                    .unwrap_or(WeaponKind::LaserPistol.basic());
                self.aboard
                    .to_station(dvec2(g.x as f64, g.y as f64))
                    .map(|p| (p, weapon))
            })
            .collect()
    }
}

/// A walk cut at `reach` room units from `from`: the waypoints as pairs,
/// the last one where the reach runs out.
fn cut_walk(from: bims::math::Vec2, route: &[bims::math::Vec2], reach: f32) -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    let mut at = from;
    let mut left = reach;
    for &next in route {
        let leg = (next - at).len();
        if leg >= left {
            if leg > 0.0 {
                let p = at + (next - at) * (left / leg);
                out.push((p.x, p.y));
            }
            return out;
        }
        left -= leg;
        out.push((next.x, next.y));
        at = next;
    }
    out
}

/// A ghost walked on for `seconds` at its pace along its path, turned to
/// the way it goes and its stride come on with the ground covered — the
/// body's own count, a tenth of a turn a unit.
fn walk_ghost(ghost: &mut crate::items::Ghost, seconds: f32) {
    let mut left = ghost.pace * seconds;
    while left > 0.0 {
        let Some(&(nx, ny)) = ghost.path.first() else {
            break;
        };
        let (dx, dy) = (nx - ghost.x, ny - ghost.y);
        let leg = (dx * dx + dy * dy).sqrt();
        if leg > 0.0 {
            ghost.heading = dy.atan2(dx);
        }
        let step = leg.min(left);
        if leg > 0.0 {
            ghost.x += dx * step / leg;
            ghost.y += dy * step / leg;
        }
        ghost.stride = (ghost.stride + step * 0.10) % std::f32::consts::TAU;
        left -= step;
        if step >= leg {
            ghost.path.remove(0);
        }
    }
}
