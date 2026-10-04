//! The floor's half of the world (October 2026, [`crate::floor`]): the
//! floor laid on the galaxy's stars, where the crew are on it, where a
//! trip may go from there, and what the map marks every place with.
//!
//! A child of `crate::world`, like `mission.rs`, so it reaches what the
//! other `impl World` blocks do. The trip itself is the run's as ever
//! (`World::travel_quote`, `World::travel`): on the floor the quote
//! refuses every place but those the crew's place leads to
//! ([`Refusal::TooFar`]), asks nothing about lanes or jammers, and puts
//! the clock on to the row's day.

use std::sync::Arc;

use super::*;
use crate::floor::{self, Floor};
use crate::run::Site;

/// One place on the floor as the map draws it: where it is, the site,
/// and what it will be on its row's day.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FloorMark {
    pub row: u32,
    pub index: u32,
    pub site: Site,
    /// An attack, a defence or a trader, on the row's day.
    pub kind: SiteKind,
    /// The system's elite: a Guardian and relics.
    pub elite: bool,
    /// Its fight over: an attack cleared, a defence held.
    pub cleared: bool,
    /// A defence that is an **Area defend** (October 2026): a town's,
    /// holding its FOB.
    pub area: bool,
    /// The Machine Heart's fortress.
    pub heart: bool,
    /// The tier the row is marked ([`World::floor_tier`]).
    pub tier: Tier,
}

/// A star the floor could put a place on: its one site and what decides
/// which row it suits.
struct Candidate {
    star: u32,
    station: u32,
    /// Lanes from the machines' origin.
    hops: u32,
    /// Whether its site is a trader.
    trader: bool,
    /// The day the machines take it ([`World::infested_on`]).
    turns: u32,
}

impl World {
    /// The floor, while its switch is on.
    pub fn floor(&self) -> Option<&Floor> {
        self.floor.as_deref().filter(|_| self.run.floor)
    }

    /// Switch the floor on or off ([`Run::floor`]), laying it or dropping
    /// it. Every session of the app switches it on.
    pub fn set_floor(&mut self, on: bool) {
        self.run.floor = on;
        let galaxy = self.galaxy();
        self.settle_floor(&galaxy);
    }

    /// The floor laid again, or dropped, as the switch says: at the
    /// switch and at every load ([`World::settle_crisis`]), off `galaxy`
    /// already generated.
    pub(super) fn settle_floor(&mut self, galaxy: &Galaxy) {
        self.floor = self.run.floor.then(|| Arc::new(self.lay_floor(galaxy)));
    }

    /// The floor of this galaxy: its shape off the galaxy's seed and the
    /// crew's own star ([`floor::shape`]), the start the crew's own
    /// station, the Heart the machines' origin, and every place between a
    /// star of its own whose system's one site suits the row — a trader on
    /// a trader's place (one still trading on that row's day where there is
    /// one), a fight anywhere else — the star nearest the row's distance
    /// from the origin: the crew's own distance at the bottom, nought at
    /// the top, so the climb walks into the crisis. Ties go by a roll.
    fn lay_floor(&self, galaxy: &Galaxy) -> Floor {
        let seed = floor::seed(self.galaxy_seed, self.home_star);
        let mut candidates: Vec<Candidate> = Vec::new();
        for star in 0..galaxy.stars.len() as u32 {
            if star == self.home_star || star == self.droid_origin {
                continue;
            }
            let Some(mut system) = galaxy.system(star) else {
                continue;
            };
            self.trim_system(star, &mut system);
            let Some(station) = self.mission_site(star, &system) else {
                continue;
            };
            if heart::is_heart(station) {
                continue;
            }
            candidates.push(Candidate {
                star,
                station,
                hops: u32::from(self.hops_from_origin(star)),
                trader: self.trader_of(star, &system.stations) == Some(station),
                turns: self.infested_on(star),
            });
        }
        let unreached = u32::from(u16::MAX);
        let far = candidates
            .iter()
            .map(|c| c.hops)
            .filter(|&h| h != unreached)
            .max()
            .unwrap_or(1);
        let home = match u32::from(self.hops_from_origin(self.home_star)) {
            h if h == unreached => far + 1,
            h => h,
        };
        let hops = data::FLOOR_HOPS;
        let mut used = vec![false; candidates.len()];
        floor::lay(floor::shape(seed), |row, index, shop| {
            if row == 0 {
                return (self.home_star, self.home);
            }
            if row >= hops {
                return (self.droid_origin, heart::heart_id(self.droid_origin));
            }
            // How far from the origin, in lanes times the hops, so the
            // sums stay whole.
            let want = home * (hops - row);
            let best = |used: &[bool], ok: &dyn Fn(&Candidate) -> bool| -> Option<usize> {
                candidates
                    .iter()
                    .enumerate()
                    .filter(|(k, c)| !used[*k] && ok(c))
                    .min_by_key(|(_, c)| {
                        let off = (c.hops.min(far + 1) * hops).abs_diff(want);
                        let tie = worldgen::rng::mix(
                            seed ^ (u64::from(row) << 40)
                                ^ ((index as u64) << 32)
                                ^ u64::from(c.star),
                        );
                        (off, tie)
                    })
                    .map(|(k, _)| k)
            };
            let open = |c: &Candidate| c.trader && c.turns >= row;
            let trader = |c: &Candidate| c.trader;
            let fight = |c: &Candidate| !c.trader;
            let pick = if shop {
                best(&used, &open).or_else(|| best(&used, &trader))
            } else {
                None
            }
            .or_else(|| best(&used, &fight))
            .or_else(|| best(&used, &|_| true));
            match pick {
                Some(k) => {
                    used[k] = true;
                    (candidates[k].star, candidates[k].station)
                }
                // A galaxy with no star left over: a star twice, the
                // nearest of all.
                None => best(&vec![false; candidates.len()], &|_| true)
                    .map(|k| (candidates[k].star, candidates[k].station))
                    .unwrap_or((self.home_star, self.home)),
            }
        })
    }

    /// Where the crew are on the floor, `(row, index)`: the place of the
    /// star the ship is at — of two with that star, the one whose day is
    /// nearest today's. `None` off the floor (a staged command's world,
    /// opened somewhere else) or with the floor off.
    pub fn floor_at(&self) -> Option<(u32, usize)> {
        let floor = self.floor()?;
        let today = self.run_day();
        floor
            .rows
            .iter()
            .enumerate()
            .flat_map(|(row, nodes)| {
                nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| n.star == self.star_id)
                    .map(move |(i, _)| (row as u32, i))
            })
            .min_by_key(|&(row, _)| floor::row_day(row).abs_diff(today))
    }

    /// The row a star's place is on, nearest today's where it is on two.
    fn floor_row_of(&self, star: u32) -> Option<u32> {
        let floor = self.floor()?;
        let today = self.run_day();
        floor
            .rows
            .iter()
            .enumerate()
            .filter(|(_, nodes)| nodes.iter().any(|n| n.star == star))
            .map(|(row, _)| row as u32)
            .min_by_key(|&row| floor::row_day(row).abs_diff(today))
    }

    /// The places a trip may go from where the crew are, left to right:
    /// those their place leads to on the row above, and — off the floor —
    /// the whole first row. Nothing with the floor off, or at the Heart.
    pub fn floor_next(&self) -> Vec<Site> {
        let Some(floor) = self.floor() else {
            return Vec::new();
        };
        let places = match self.floor_at() {
            Some((row, index)) => floor.next(row, index),
            None => (0..floor.rows.get(1).map_or(0, Vec::len))
                .map(|j| (1, j))
                .collect(),
        };
        places
            .into_iter()
            .filter_map(|(row, j)| floor.node(row, j))
            .map(|n| Site {
                star: n.star,
                station: n.station,
            })
            .collect()
    }

    /// The tier a row of the floor is marked: tier three at the Heart and
    /// from the day of [`droidplan::WaveScaling::tier3_days`], tier two
    /// from [`droidplan::WaveScaling::tier2_days`], tier one before — the
    /// days from which every enemy has reached that tier, so a row marked
    /// tier one may still meet a few machines a tier up.
    pub fn floor_tier(&self, row: u32) -> Tier {
        let heart = self.floor().map_or(data::FLOOR_HOPS, Floor::heart_row);
        if row >= heart {
            return Tier::Three;
        }
        let scaling = self.scaling();
        let day = floor::row_day(row);
        if day >= scaling.tier3_days {
            Tier::Three
        } else if day >= scaling.tier2_days {
            Tier::Two
        } else {
            Tier::One
        }
    }

    /// What a trip to `star`'s place puts the world clock on by: to the
    /// day of its row, never back. `None` for a star not on the floor.
    pub(super) fn floor_minutes(&self, star: u32) -> Option<u64> {
        let row = self.floor_row_of(star)?;
        let days = floor::row_day(row).saturating_sub(self.run_day());
        Some(data::JUMP_MINUTES * u64::from(days))
    }

    /// The tier a trip to `star`'s place says, off its row. `None` for a
    /// star not on the floor.
    pub(super) fn floor_tier_of(&self, star: u32) -> Option<Tier> {
        self.floor_row_of(star).map(|row| self.floor_tier(row))
    }

    /// Every place on the floor as the map marks it, row by row and left
    /// to right: what it will be on its row's day, quoted as a look
    /// (every question about whether the crew may go there left unasked).
    /// Each system generated once, so it is asked when the run moves on,
    /// never a frame.
    pub fn floor_marks(&self) -> Vec<FloorMark> {
        let Some(floor) = self.floor() else {
            return Vec::new();
        };
        let galaxy = self.galaxy();
        let mut marks = Vec::with_capacity(floor.len());
        for (row, nodes) in floor.rows.iter().enumerate() {
            for (index, node) in nodes.iter().enumerate() {
                let site = Site {
                    star: node.star,
                    station: node.station,
                };
                let generated;
                let given = if node.star == self.star_id {
                    None
                } else {
                    let Some(mut system) = galaxy.system(node.star) else {
                        continue;
                    };
                    self.trim_system(node.star, &mut system);
                    generated = system;
                    Some(&generated)
                };
                let Ok(quote) = self.quote_given(Some(&galaxy), site, true, given) else {
                    continue;
                };
                marks.push(FloorMark {
                    row: row as u32,
                    index: index as u32,
                    site,
                    kind: quote.kind,
                    // An Area defend is an elite fight too (October 2026).
                    elite: quote.elite
                        || (quote.kind == SiteKind::Defend && self.is_area_defense(node.station)),
                    cleared: quote.cleared,
                    area: quote.kind == SiteKind::Defend && self.is_area_defense(node.station),
                    heart: heart::is_heart(node.station),
                    tier: self.floor_tier(row as u32),
                });
            }
        }
        marks
    }
}
