//! **A nest hunt** (October 2026, a mission the map shapes): an attack in
//! the tier-three zone whose machines come out of nests grown into the
//! site's walls.
//!
//! - **The nests**: [`data::NESTS`], one more a player past the first, at
//!   most [`data::NESTS_MOST`], on free deck tiles against a wall, the
//!   first beside a fuel drum where the site has one (a shot to set off),
//!   the rest spread across the site from the port outward — each the one
//!   farthest from the port and every nest already chosen ([`nest_spots`]).
//!   Laid when the crew first dock (`World::settle_nests`) as the Machine
//!   Heart's fabricators are, `bims::droid::Droid::structure`s at the front
//!   of the machines' list, so a wave cleared keeps them and nothing about
//!   a wave counts them standing. Each faces away from its wall.
//! - **They build**: every [`data::NEST_BUILD_STEPS`] each standing nest
//!   puts a machine out of its bay (`World::fabricate`, the fabricators'
//!   own), looking for the crew. The site's own first wave stands about it
//!   as at any attack.
//! - **Cleared** with every nest destroyed and every machine down: the
//!   waves' clock clears nothing while a nest stands.
//!
//! The state is the `Infestation`'s (`droid::Nests`), saved, hashed and put
//! back with it; which nests are destroyed is read off the room every step
//! and laid again as wrecks on a room built afresh. A child of
//! `crate::world`, as `mission.rs` is. **Nothing here draws from a stream.**

use super::*;
use crate::droid::Nests;
use crate::run::Mission;
use bims::droid::{Droid, DroidKind};
use shipdesign::Layer;

/// Where a site's `n` nests grow, with the way each faces: see the module
/// note. Free deck tiles with a wall on one side and free deck the other
/// three, the bay's tile before it free too.
pub fn nest_spots(
    design: &ShipDesign,
    from: (f64, f64),
    n: usize,
) -> Vec<((u32, u32), (i32, i32))> {
    let grid = design.grid();
    let t = shipdesign::TILE as f64;
    let free = |x: i32, y: i32| {
        grid.get(Layer::Floor, (x, y)) != 0 && grid.get(Layer::Object, (x, y)) == 0
    };
    let kind_at = |x: i32, y: i32| design.part(grid.get(Layer::Object, (x, y))).map(|p| p.kind);
    let wall =
        |x: i32, y: i32| matches!(kind_at(x, y), Some(PartKind::Wall | PartKind::OutsideWall));
    let mut candidates: Vec<((u32, u32), (i32, i32))> = Vec::new();
    for part in design.parts.iter().filter(|p| p.kind == PartKind::Floor) {
        let (x, y) = (part.origin.0 as i32, part.origin.1 as i32);
        if !free(x, y) {
            continue;
        }
        for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
            // A wall behind, the bay's two tiles before it free.
            if wall(x + dx, y + dy) && free(x - dx, y - dy) && free(x - 2 * dx, y - 2 * dy) {
                candidates.push(((x as u32, y as u32), (-dx, -dy)));
                break;
            }
        }
    }
    let at = |tile: (u32, u32)| ((tile.0 as f64 + 0.5) * t, (tile.1 as f64 + 0.5) * t);
    let dist = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).hypot(a.1 - b.1);
    let mut chosen: Vec<((u32, u32), (i32, i32))> = Vec::new();
    // The first beside a fuel drum, the one of them farthest from the port.
    let by_drum = candidates
        .iter()
        .filter(|((x, y), _)| {
            (-1..=1).any(|dx| {
                (-1..=1)
                    .any(|dy| kind_at(*x as i32 + dx, *y as i32 + dy) == Some(PartKind::FuelTank))
            })
        })
        .max_by(|a, b| {
            dist(at(a.0), from)
                .total_cmp(&dist(at(b.0), from))
                .then(b.0.cmp(&a.0))
        })
        .copied();
    chosen.extend(by_drum);
    while chosen.len() < n {
        let next = candidates
            .iter()
            .filter(|c| !chosen.iter().any(|o| o.0 == c.0))
            .map(|&c| {
                let near = chosen
                    .iter()
                    .map(|o| dist(at(o.0), at(c.0)))
                    .fold(dist(at(c.0), from), f64::min);
                (c, near)
            })
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.0.cmp(&a.0.0)));
        let Some((c, _)) = next else {
            break;
        };
        chosen.push(c);
    }
    chosen
}

impl World {
    /// The nests of the site alongside, with its id.
    fn nests_here(&self) -> Option<(u32, &Nests)> {
        let id = self.ship.state.alongside()?;
        Some((id, self.infestation(id)?.nests.as_ref()?))
    }

    /// How many of the site alongside's nests stand, of how many — `None`
    /// unless its fight is a nest hunt.
    pub fn nests_standing(&self) -> Option<(u32, u32)> {
        let (_, n) = self.nests_here()?;
        let standing = n.down.iter().filter(|&&d| !d).count() as u32;
        Some((standing, n.spots.len() as u32))
    }

    /// Every standing nest of the site alongside, its tile's middle in the
    /// site's own design units: where the app marks them through the fog.
    pub fn nests_look(&self) -> Vec<DVec2> {
        let Some((_, n)) = self.nests_here() else {
            return Vec::new();
        };
        let t = shipdesign::TILE as f64;
        n.spots
            .iter()
            .zip(&n.down)
            .filter(|(_, down)| !**down)
            .map(|(&(x, y), _)| dvec2((x as f64 + 0.5) * t, (y as f64 + 0.5) * t))
            .collect()
    }

    /// The nests laid at `id` the step the crew first dock there, where its
    /// mission is a nest hunt.
    pub(super) fn settle_nests(&mut self, id: u32) {
        if self.mission_here(id) != Mission::Nests {
            return;
        }
        if self
            .infestation(id)
            .is_none_or(|it| it.nests.is_some() || it.cleared || it.heart.is_some())
        {
            return;
        }
        let Some(station) = self.station(id) else {
            return;
        };
        let Some(port) = station.port() else {
            return;
        };
        let players = self.players().max(1);
        let n = (data::NESTS + data::NESTS_PER_PLAYER * (players - 1)).min(data::NESTS_MOST);
        let spots = nest_spots(&station.design, port.centre, n as usize);
        if spots.is_empty() {
            return;
        }
        let now = self.run.mission_steps;
        if let Some(it) = self.infestation_mut(id) {
            it.nests = Some(Nests {
                down: vec![false; spots.len()],
                facing: spots.iter().map(|s| s.1).collect(),
                spots: spots.iter().map(|s| s.0).collect(),
                next_build: now + data::NEST_BUILD_STEPS,
                built: 0,
            });
        }
    }

    /// The nests to go on a fresh deck ahead of the wave — `settle_droids`
    /// lays them first, as the Heart's machines, so they keep the front of
    /// the list; one destroyed before is laid a wreck.
    pub(super) fn nest_machines_to_lay(&self, station: &Station) -> Vec<Droid> {
        let Some(nests) = self
            .infestation(station.id)
            .and_then(|it| it.nests.as_ref())
        else {
            return Vec::new();
        };
        let Some(residents) = &self.residents else {
            return Vec::new();
        };
        let t = shipdesign::TILE as f64;
        nests
            .spots
            .iter()
            .zip(&nests.facing)
            .zip(&nests.down)
            .enumerate()
            .map(|(i, ((&(x, y), &(fx, fy)), &down))| {
                let at = residents
                    .aboard
                    .to_room(dvec2((x as f64 + 0.5) * t, (y as f64 + 0.5) * t));
                let mut nest = Droid::structure(
                    DroidKind::Fabricator,
                    Tier::Three,
                    data::NEST_HEALTH,
                    at,
                    bims::math::vec2(fx as f32, fy as f32),
                    station.map_seed ^ 0x_4E55 ^ (i as u64) << 12,
                );
                if down {
                    nest.destroyed = true;
                }
                nest
            })
            .collect()
    }

    /// The nest hunt's step, after the waves: which nests are destroyed
    /// read off the room, and every standing one building on its clock.
    pub(super) fn nests_step(&mut self, events: &mut Vec<WorldEvent>) {
        let Some((id, nests)) = self.nests_here().map(|(id, n)| (id, n.clone())) else {
            return;
        };
        if !self.aboard.is_joined() || self.residents.as_ref().is_none_or(|r| r.station != id) {
            return;
        }
        // The nests are the first machines on the deck, in their order.
        let down: Vec<bool> = {
            let room = &self.residents.as_ref().unwrap().aboard.room;
            (0..nests.spots.len())
                .map(|i| {
                    room.droid(i)
                        .filter(|d| d.kind == DroidKind::Fabricator)
                        .is_none_or(|d| d.destroyed)
                })
                .collect()
        };
        let newly = down
            .iter()
            .zip(&nests.down)
            .filter(|(now, was)| **now && !**was)
            .count();
        for _ in 0..newly {
            let left = down.iter().filter(|&&d| !d).count() as u32;
            events.push(WorldEvent::NestDestroyed { station: id, left });
        }
        let now = self.run.mission_steps;
        let standing = down.iter().any(|&d| !d);
        let due = standing && now >= nests.next_build;
        let built = if due {
            let made = self.fabricate(id, nests.built);
            // What a nest builds goes looking for the crew.
            if let Some(residents) = self.residents.as_mut() {
                let room = &mut residents.aboard.room;
                let n = room.droid_count() as usize;
                for i in n.saturating_sub(made as usize)..n {
                    if let Some(d) = room.droid_mut_for_probe(i) {
                        d.seeking = true;
                    }
                }
            }
            made
        } else {
            0
        };
        if let Some(n) = self.infestation_mut(id).and_then(|it| it.nests.as_mut()) {
            n.down = down;
            if due {
                n.next_build = now + data::NEST_BUILD_STEPS;
                n.built += built;
            }
        }
    }

    /// Whether a nest of the site `id` still stands: its waves clear
    /// nothing while one does.
    pub(super) fn a_nest_stands(&self, id: u32) -> bool {
        self.infestation(id)
            .and_then(|it| it.nests.as_ref())
            .is_some_and(|n| n.down.iter().any(|&d| !d))
    }
}
