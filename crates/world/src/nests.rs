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
//!   a wave counts them standing. Each faces away from its wall, at the
//!   area's [`data::NEST_HEALTH`] (the relics' and the ascension's enemy
//!   health on it) and the zone's tier.
//! - **They build the mission's budget, a slice a round** (October 2026,
//!   [`crate::run::Budget`]; the player's: "each build is a slice of the
//!   wave … and the nests trickle once the total is spent"). The site's
//!   garrison is the budget's first landing; then every
//!   [`data::NEST_BUILD_STEPS`] each standing, unstunned nest puts `k`
//!   enemies out of its bay — `k = ⌈wave ÷ (`[`data::NEST_SLICE_ROUNDS`]` ×
//!   nests laid)⌉`, a round of every nest a third of a wave — the round
//!   taken off the budget in nest order until it is spent
//!   ([`World::nest_build`]). Spent, the nests **trickle**: one a player a
//!   round between them, every body marked unpaid, so a nest left standing
//!   pays nothing more. A nest builds no Bombers or Lancers: the
//!   garrison's are the hunt's. What comes out is what the area calls for
//!   (the player's: "the nest should spawn the appropriate enemy type not
//!   just tier 3"): the day's share of machines over everything the nests
//!   have built ([`World::machines_of`], so the Manufacturers' people in
//!   area 0, the machines coming in through the tier-one area), the
//!   machines at the day's tiers ([`World::machine_tiers`]) by turns a
//!   Trooper, a Husk, a Trooper, a Warden, the people armed at the day's
//!   share — several out of one bay standing a tile apart, and all of them
//!   looking for the crew. Until then each nest built one enemy a round
//!   for ever, never priced: a nest left standing was a farm.
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

/// The salt of what the nests build's people, "NEST".
const NEST_SALT: u64 = 0x_4E45_5354;

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
        // The area's health (October 2026), the crew's relics and the
        // ascension on it, and the zone's tier.
        let health =
            data::NEST_HEALTH[self.area_now().0.min(3)] * self.enemy_health_factor().unwrap_or(1.0);
        let tier = self.zone_tier();
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
                    tier,
                    health,
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
            self.nest_build(id, nests.built)
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

    /// Whether the nests of the site alongside have built anything: what
    /// they put out is told where the crew are.
    pub(super) fn nests_standing_built(&self) -> bool {
        self.nests_here().is_some_and(|(_, n)| n.built > 0)
    }

    /// A round of every standing nest's bays at `id`, the `built` built
    /// before counted: see the module note. How many were built.
    pub(super) fn nest_build(&mut self, id: u32, built: u32) -> u32 {
        const KINDS: [DroidKind; 4] = [
            DroidKind::Trooper,
            DroidKind::Husk,
            DroidKind::Trooper,
            DroidKind::Warden,
        ];
        let wave = self.infestation(id).map_or(1, |it| it.wave);
        let laid = self
            .infestation(id)
            .and_then(|it| it.nests.as_ref())
            .map_or(1, |n| n.spots.len() as u32)
            .max(1);
        let Some(residents) = self.residents.as_ref() else {
            return 0;
        };
        let room = &residents.aboard.room;
        // Each bay, a tile and a bit out of a nest standing and not
        // stunned, the way it faces, and its index among the machines.
        let bays: Vec<(bims::math::Vec2, bims::math::Vec2, usize)> = room
            .droids()
            .iter()
            .enumerate()
            .filter(|(_, d)| d.kind == DroidKind::Fabricator && !d.destroyed && !d.is_stunned())
            .map(|(i, d)| {
                (
                    d.pos + d.facing() * (shipdesign::TILE as f32 * 1.2),
                    d.facing(),
                    i,
                )
            })
            .collect();
        let standing = bays.len() as u32;
        if standing == 0 {
            return 0;
        }
        // The round's slice of the mission's budget (October 2026): `k` a
        // nest, a round of every nest laid a third of a wave — or, the
        // budget spent, the trickle, one a player round the nests. With no
        // budget (the probes' forced kinds) one a nest, as before.
        let (k, landing) = match self.run.budget {
            Some(budget) => {
                let k = budget
                    .wave
                    .div_ceil(data::NEST_SLICE_ROUNDS * laid)
                    .max(1);
                (k, self.budget_take_plain(standing * k))
            }
            None => (
                1,
                run::Landing {
                    base: standing,
                    bombers: 0,
                    lancers: 0,
                    unpaid: false,
                },
            ),
        };
        let n = landing.base;
        // Which bay each body comes out of: `k` a nest in nest order until
        // the slice is spent, or the trickle dealt round them.
        let owner = |j: u32| -> usize {
            if landing.unpaid {
                (j % standing) as usize
            } else {
                ((j / k).min(standing - 1)) as usize
            }
        };
        // Where each stands: its bay, and a tile aside or a row out for
        // every one before it out of the same bay — never two on a tile.
        let t = shipdesign::TILE as f32;
        let mut out_of = vec![0u32; bays.len()];
        let mut spots: Vec<(bims::math::Vec2, bims::math::Vec2)> = Vec::with_capacity(n as usize);
        for j in 0..n {
            let (spot, facing, _) = bays[owner(j)];
            let m = out_of[owner(j)];
            out_of[owner(j)] += 1;
            let aside = bims::math::vec2(-facing.y, facing.x);
            let col = [0.0, 1.0, -1.0][(m % 3) as usize];
            let row = (m / 3) as f32;
            spots.push((spot + (facing * row + aside * col) * t, facing));
        }
        // The day's share over everything built so far, so a share under
        // one a round still comes out right over the fight.
        let before = self.machines_of(built, wave);
        let machines = self
            .machines_of(built + n, wave)
            .saturating_sub(before)
            .min(n);
        let people = n - machines;
        let count = |m: u32| {
            let tiers = self.machine_tiers(m);
            Tier::ALL.map(|t| tiers.iter().filter(|&&x| x == t).count())
        };
        let (was, now) = (count(before), count(before + machines));
        let mut tiers: Vec<Tier> = [Tier::Three, Tier::Two, Tier::One]
            .into_iter()
            .flat_map(|t| {
                let i = t.code() as usize - 1;
                std::iter::repeat_n(t, now[i].saturating_sub(was[i]))
            })
            .collect();
        tiers.resize(machines as usize, self.zone_tier());
        let toughen = self.enemy_health_factor();
        // Their people the first of the round, the machines the rest.
        let (theirs, rest) = spots.split_at(people as usize);
        let at: Vec<bims::math::Vec2> = theirs.iter().map(|s| s.0).collect();
        let seed = self.garrison_seed(id, wave) ^ NEST_SALT ^ u64::from(built);
        let bims_before = self
            .residents
            .as_ref()
            .map_or(0, |r| r.aboard.room.crew_count() as usize);
        self.stand_people(people, &at, seed);
        let Some(residents) = self.residents.as_mut() else {
            return 0;
        };
        let room = &mut residents.aboard.room;
        if landing.unpaid {
            for who in bims_before..room.crew_count() as usize {
                room.set_unpaid(who);
            }
        }
        let mut made = Vec::with_capacity(rest.len());
        for (j, (&(spot, facing), &tier)) in rest.iter().zip(&tiers).enumerate() {
            let k = (before as usize) + j;
            let mut droid = Droid::new(
                KINDS[k % KINDS.len()],
                tier,
                built as usize + j,
                wave,
                spot,
                facing.angle(),
                u64::from(id) << 20 ^ (built as u64 + j as u64) ^ 0x_FAB,
            );
            if let Some(factor) = toughen {
                droid.body.toughen(factor);
            }
            droid.plan_wait = 0.0;
            droid.seeking = true;
            droid.unpaid = landing.unpaid;
            made.push(droid);
        }
        for (b, &(_, _, i)) in bays.iter().enumerate() {
            if out_of[b] == 0 {
                continue;
            }
            if let Some(h) = room.heart_state_mut(i) {
                h.made = bims::droid::HeartState::MADE_FLASH;
            }
        }
        room.adopt_droids(made, bims::math::Vec2::ZERO);
        residents.aboard.crew = room.body_count();
        n
    }

    /// Whether a nest of the site `id` still stands: its waves clear
    /// nothing while one does.
    pub(super) fn a_nest_stands(&self, id: u32) -> bool {
        self.infestation(id)
            .and_then(|it| it.nests.as_ref())
            .is_some_and(|n| n.down.iter().any(|&d| !d))
    }
}
