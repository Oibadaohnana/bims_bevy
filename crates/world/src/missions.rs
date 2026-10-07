//! The missions the map shapes (October 2026): which a fight at a station
//! is ([`crate::run::Mission`]), and the rules of each beyond the waves.
//!
//! **Which** is stateless, like an elite: a roll off the galaxy's seed,
//! the star and the station on a salt of its own — no stream a fight draws
//! from — for a station's fight on the floor from its first row on, at
//! every tier (the player's: "also in tier one those new mission types
//! should appear"), never at the start at home, an elite, the Heart, a
//! trader or a town. A defence is **Seal the breaches** one time in
//! [`BREACHES_ODDS`] and an **Evacuation** one in as many; an attack a
//! **Sabotage** or a **nest hunt** the same. The map never says which:
//! a mission is found on arrival. Off the floor (the tests, the staged commands) every
//! fight is plain unless a probe forces one ([`World::set_mission_for_probe`]).
//!
//! **Seal the breaches**: the site's ways in (`entry.rs`: its airlocks
//! but the port) are breaches the machines come in by — a wave every
//! [`data::BREACH_WAVE_STEPS`] on the clock, stacking, at the next open
//! one in turn, smaller the fewer are open — until the crew have welded
//! every one shut ([`data::BREACH_WELD_SECONDS`] of hands on it, a second
//! a second for each Bim welding it, two for an engineer) and destroyed
//! what is aboard. Nothing burns through a weld there, and the machines
//! make for the breach being welded. A child of `crate::world`, as
//! `mission.rs` is. **Nothing here draws from a stream.**

use super::*;
use crate::run::Mission;

/// The salt of a site's mission roll, "MISSION".
const MISSION_SALT: u64 = 0x_4D49_5353_494F_4E;
/// One station defence in so many is Seal the breaches.
pub const BREACHES_ODDS: u32 = 3;

impl World {
    /// What a fight at `station` of `star`'s system asks beyond its waves,
    /// for a site of `kind` (an elite's or not): see the module note.
    pub fn mission_at(&self, star: u32, station: u32, kind: SiteKind, elite: bool) -> Mission {
        let mapped = surface::surface_body(station).is_none()
            && !heart::is_heart(station)
            && kind != SiteKind::Trader;
        if let Some(forced) = self.mission_forced {
            // A mission only where its kind of fight is: a defence's at a
            // defence, an attack's at an attack.
            let fits = match forced {
                Mission::Plain => true,
                Mission::Breaches | Mission::Evacuation => kind == SiteKind::Defend,
                attack => kind == SiteKind::Attack && attack.is_attack(),
            };
            return if mapped && fits {
                forced
            } else {
                Mission::Plain
            };
        }
        if !mapped || elite {
            return Mission::Plain;
        }
        // On the floor, from its first row on (the start at home plain).
        let Some(row) = self.floor_row_of(star) else {
            return Mission::Plain;
        };
        if row == 0 {
            return Mission::Plain;
        }
        let seed = worldgen::rng::mix(self.galaxy_seed ^ MISSION_SALT)
            ^ worldgen::rng::mix(u64::from(star))
            ^ worldgen::rng::mix(u64::from(station).wrapping_add(0x_5354));
        let mut rng = worldgen::rng::Rng::new(seed);
        match kind {
            SiteKind::Defend => match rng.below(BREACHES_ODDS) {
                0 => Mission::Breaches,
                1 => Mission::Evacuation,
                _ => Mission::Plain,
            },
            // Every station attack is a mission, each as likely (October
            // 2026, the player's: "drop the plain … it is to boring").
            SiteKind::Attack => {
                Mission::ATTACKS[rng.below(Mission::ATTACKS.len() as u32) as usize]
            }
            SiteKind::Trader => Mission::Plain,
        }
    }

    /// The mission of a site of this system.
    pub fn mission_here(&self, station: u32) -> Mission {
        self.mission_at(
            self.star_id,
            station,
            self.site_kind(station),
            self.is_elite_here(station),
        )
    }

    /// Every station fight of `mission`'s kind made it — a defence's at
    /// every defence, an attack's at every attack, the rest plain; `Plain`
    /// every fight plain; `None` the roll again — for the tests, the
    /// staged commands (`BIMS_MISSION`) and the game setup's Dev tab.
    /// Saved, not hashed.
    pub fn set_mission_for_probe(&mut self, mission: Option<Mission>) {
        self.mission_forced = mission;
        // The site's room opened again, for an Evacuation's refugees.
        if let Some(id) = self.ship.state.alongside() {
            self.reopen_residents(id);
        }
    }

    // --- Seal the breaches -------------------------------------------------

    /// Whether the fight at `station` is Seal the breaches, its defence
    /// begun as one.
    pub fn is_breaches(&self, station: u32) -> bool {
        self.defense(station).is_some_and(|d| d.breaches.is_some())
    }

    /// The breaches the site alongside has, and how many of them are still
    /// open — `None` unless the fight there is Seal the breaches.
    pub fn breaches_open(&self) -> Option<(u32, u32)> {
        let station = self.ship.state.alongside()?;
        let breaches = self.defense(station)?.breaches.as_ref()?;
        Some((breaches.open, breaches.total))
    }

    /// The breaches counted again: how many of the site's ways in are not
    /// welded, onto its defence. Said every step of the fight.
    pub(super) fn count_the_breaches(&mut self, station: u32) {
        let Some(site) = self.station(station).cloned() else {
            return;
        };
        let ways = Self::ways_in(&site);
        let open = ways
            .iter()
            .filter(|w| !self.run.welded.contains(&w.index))
            .count() as u32;
        if let Some(b) = self.defense_mut(station).and_then(|d| d.breaches.as_mut()) {
            b.total = ways.len() as u32;
            b.open = open;
        }
    }

    /// A wave landing at a Seal the breaches fight, of `full` at full
    /// strength: as much smaller as breaches are welded — its share of
    /// the open ones, rounded up, one at the least.
    pub(super) fn breach_wave_size(&self, station: u32, full: u32) -> u32 {
        let Some(b) = self.defense(station).and_then(|d| d.breaches.as_ref()) else {
            return full;
        };
        if b.total == 0 {
            return full;
        }
        (full * b.open).div_ceil(b.total).max(1)
    }

    /// Where the machines make for at a Seal the breaches fight: the weld
    /// being worked, when one is — every machine and every one of their
    /// people at a spot round it — and nothing otherwise. In the
    /// residents' room's units, by body index.
    pub(super) fn breach_objectives(&self, station: u32) -> Vec<Option<bims::math::Vec2>> {
        let Some(residents) = self.residents.as_ref().filter(|r| r.station == station) else {
            return Vec::new();
        };
        let Some(site) = self.station(station) else {
            return Vec::new();
        };
        let worked = Self::ways_in(site)
            .into_iter()
            .find(|w| self.weld_share(w.index) > 0.0 && !self.run.welded.contains(&w.index));
        let Some(way) = worked else {
            return Vec::new();
        };
        let room = &residents.aboard.room;
        let middle = residents
            .aboard
            .to_room(way.at.sub(way.outward.scale(3.0 * shipdesign::TILE as f64)));
        let bodies = room.body_count() as usize;
        let bims = room.crew_count() as usize;
        (0..bodies)
            .map(|body| {
                let enemy = body >= bims || room.is_manufacturer(body);
                enemy.then(|| middle + defense::enemy_spot(body) * shipdesign::TILE as f32)
            })
            .collect()
    }
}
