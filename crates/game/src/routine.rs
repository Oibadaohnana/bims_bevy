//! A station's people in peacetime (feature 102): a role, and the round it
//! walks.
//!
//! Nobody on a station eats, sleeps or goes to the heads (the needs went
//! in feature 104), so nothing would move them but the wander.
//! What moves them instead is a **routine**: a role dealt when the site's
//! room is opened — a **guard** walks between the site's ways in, its
//! airlocks or a town's gates; a **trader** stands at the trading desk and
//! now and then steps away from it and back; a **worker** goes between two
//! or three of the site's work fixtures; a **civilian** strolls between
//! rooms and stops in each — and a round of [`Stop`]s worked out once from
//! the room's own fixtures, never written down for a site, so every
//! station the generator makes and every town has one. A role whose
//! fixtures the site has not got falls back on a wander between points of
//! open deck the body can reach — rooms aboard a station, open ground on
//! a planet — and says so ([`Routine::fallback`]).
//!
//! Only the peace is scripted here. On an alert the body is recruited,
//! posted to shelter or given an errand by the fight as before, and the
//! round waits where it stood; `Game::keep_to_routine` picks it up again
//! the moment the body is its own.
//!
//! **Every choice is off a seed**, never off a frame: the stops, their
//! order and how long each is stood at are rolled here once, off the seed
//! the world hands over (the station's own and the body's index), and the
//! walking is the room's fixed step. So two clients on one seed walk the
//! same rounds step for step.

use crate::math::Vec2;
use crate::nav::Nav;
use crate::rng::Rng;
use crate::room::{Room, TILE};

/// What a station's person does with its day (feature 102).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Role {
    /// Walks between the site's ways in: its airlocks, a town's gates.
    Guard,
    /// Stands at the trading desk, and steps away from it now and then.
    Trader,
    /// Goes between two or three of the site's work fixtures.
    Worker,
    /// Strolls between rooms, stopping in each.
    Civilian,
}

impl Role {
    pub const ALL: [Role; 4] = [Role::Guard, Role::Trader, Role::Worker, Role::Civilian];

    /// The role's number, for the app's names and for a checksum.
    pub fn code(self) -> u32 {
        self as u32
    }
}

/// One place on a round: where to stand, and for how many minutes of the
/// room's clock.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stop {
    pub at: Vec2,
    pub minutes: f32,
}

/// A body's round and where it has got to on it. Carried on the
/// [`crate::bim::Bim`], so it goes with the body when the room is built
/// afresh at a dock or an undock (`Game::adopt` shifts it with the body).
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Routine {
    pub role: Role,
    /// Whether the site had nothing the role wants, and the round is the
    /// fallback's wander between reachable open deck instead.
    pub fallback: bool,
    pub stops: Vec<Stop>,
    /// The stop it is walking to, or standing at.
    pub next: usize,
    /// Minutes still to stand at `stops[next]` once it is there; `None`
    /// while it is on its way.
    pub waiting: Option<f32>,
}

impl Routine {
    /// Everything shifted by `by`: the room it is in was laid out again
    /// with its origin somewhere else (`Game::adopt`).
    pub fn shift(&mut self, by: Vec2) {
        for stop in &mut self.stops {
            stop.at = stop.at + by;
        }
    }

    /// The stop it is making for, if it has any.
    pub fn current(&self) -> Option<Stop> {
        self.stops.get(self.next).copied()
    }

    /// On to the next stop, round again after the last.
    pub fn advance(&mut self) {
        if !self.stops.is_empty() {
            self.next = (self.next + 1) % self.stops.len();
        }
        self.waiting = None;
    }
}

/// How close to a stop counts as at it: a tile, since a stop is snapped
/// to a free cell and the walk ends within a step of it.
pub const AT_STOP: f32 = TILE;

/// How far apart two stops of a stroll or a wander have to be, in tiles:
/// far enough that the next one is somewhere else rather than the same
/// room again.
const SPREAD_TILES: f32 = 5.0;

/// How far a trader steps away from its desk, in tiles, at the most.
const STEP_AWAY_TILES: f32 = 4.0;

/// How many stops a wander, a stroll and a worker's round have.
const WANDER_STOPS: usize = 4;
const STROLL_STOPS: (u32, u32) = (3, 4);
const WORK_STOPS: (u32, u32) = (2, 3);

/// How long each kind of stop is stood at, in minutes of the room's
/// clock: the least and the most, rolled between.
const GUARD_MINUTES: (f32, f32) = (10.0, 25.0);
const DESK_MINUTES: (f32, f32) = (60.0, 150.0);
const AWAY_MINUTES: (f32, f32) = (3.0, 8.0);
const WORK_MINUTES: (f32, f32) = (30.0, 70.0);
const STROLL_MINUTES: (f32, f32) = (10.0, 40.0);
const WANDER_MINUTES: (f32, f32) = (5.0, 20.0);

/// What a site has for a round to be made of: where each kind of fixture
/// is stood at, snapped to the deck the body can reach.
#[derive(Clone, Debug, Default)]
pub struct Anchors {
    /// The ways in: inside every airlock, and every gate the world named.
    pub exits: Vec<Vec2>,
    /// The trading desks' stand spots.
    pub desks: Vec<Vec2>,
    /// The work fixtures' stand spots: the benches, the research desks,
    /// the shelves.
    pub work: Vec<Vec2>,
    /// A spot in each place worth walking to: the bunks, the showers, the
    /// research desks, the shelves, the trading desks and the benches —
    /// one a room, near enough, since a site keeps each kind together.
    pub rooms: Vec<Vec2>,
    /// Open deck the body can reach, one spot every few tiles: what the
    /// fallback wanders between. On a planet it is the town's ground.
    pub open: Vec<Vec2>,
}

impl Anchors {
    /// The anchors of `room`, as reached from `from` over `nav`, with
    /// `gates` — a town's openings in its wall, which are not fixtures the
    /// room knows — added to the ways in. A spot the body cannot get to
    /// from where it stands is left out, so a round never names one.
    pub fn of(room: &Room, nav: &Nav, from: Vec2, gates: &[Vec2]) -> Anchors {
        let reach = |at: Vec2| {
            let at = nav.nearest_free(at);
            nav.can_reach(from, at).then_some(at)
        };
        let exits = room
            .airlocks
            .iter()
            .map(|r| r.center())
            .chain(gates.iter().copied())
            .filter_map(reach)
            .collect();
        let desks = room.desks.iter().filter_map(|d| reach(d.1)).collect();
        let work = room
            .benches
            .iter()
            .map(|b| b.at)
            .chain(room.research.iter().map(|d| d.1))
            .chain(room.shelves.iter().map(|s| s.1))
            .filter_map(reach)
            .collect();
        let bunks = room.bunks();
        let rooms = room
            .beds
            .iter()
            .take(bunks)
            .map(|b| b.frame.center())
            .chain(room.showers.iter().map(|s| s.1))
            .chain(room.research.iter().map(|d| d.1))
            .chain(room.shelves.iter().map(|s| s.1))
            .chain(room.desks.iter().map(|d| d.1))
            .chain(room.benches.iter().map(|b| b.at))
            .filter_map(reach)
            .collect();
        let middle = room.interior.center();
        let half = 0.5 * room.interior.width().max(room.interior.height());
        let open = nav
            .free_cells_within(middle, half * 1.5, 3.0 * TILE)
            .into_iter()
            .filter(|&at| nav.can_reach(from, at))
            .collect();
        Anchors {
            exits,
            desks,
            work,
            rooms,
            open,
        }
    }
}

/// The round a body of this role walks on a site with these anchors:
/// the role's own where the site has what it wants, the fallback's wander
/// where it has not, and nothing only where the body can reach no deck at
/// all. `seed` decides every choice.
pub fn plan(role: Role, anchors: &Anchors, seed: u64) -> Routine {
    let mut rng = Rng::new(seed);
    let own = match role {
        Role::Guard => guard(anchors, &mut rng),
        Role::Trader => trader(anchors, &mut rng),
        Role::Worker => worker(anchors, &mut rng),
        Role::Civilian => stroll(anchors, &mut rng),
    };
    let (fallback, stops) = match own {
        Some(stops) => (false, stops),
        None => (true, wander(anchors, &mut rng)),
    };
    Routine {
        role,
        fallback,
        stops,
        next: 0,
        waiting: None,
    }
}

fn minutes(rng: &mut Rng, (least, most): (f32, f32)) -> f32 {
    rng.range(least, most)
}

/// A guard's round: every way in, in the order the site lists them, and
/// a while at each. Two at the least — a patrol between one door and
/// itself is a post.
fn guard(anchors: &Anchors, rng: &mut Rng) -> Option<Vec<Stop>> {
    let exits = spread(&anchors.exits, 2.0 * TILE);
    if exits.len() < 2 {
        return None;
    }
    Some(
        exits
            .into_iter()
            .map(|at| Stop {
                at,
                minutes: minutes(rng, GUARD_MINUTES),
            })
            .collect(),
    )
}

/// A trader's round: the desk, a long stand at it, and a step away to a
/// spot a few tiles off and back — the desk rolled among the site's, the
/// spot among the open deck near it.
fn trader(anchors: &Anchors, rng: &mut Rng) -> Option<Vec<Stop>> {
    if anchors.desks.is_empty() {
        return None;
    }
    let desk = anchors.desks[rng.below(anchors.desks.len() as u32) as usize];
    let near: Vec<Vec2> = anchors
        .open
        .iter()
        .copied()
        .filter(|&at| {
            let d = (at - desk).len();
            d >= TILE && d <= STEP_AWAY_TILES * TILE
        })
        .collect();
    let mut stops = vec![Stop {
        at: desk,
        minutes: minutes(rng, DESK_MINUTES),
    }];
    if !near.is_empty() {
        let away = near[rng.below(near.len() as u32) as usize];
        stops.push(Stop {
            at: away,
            minutes: minutes(rng, AWAY_MINUTES),
        });
    }
    Some(stops)
}

/// A worker's round: two or three of the site's work fixtures, rolled.
fn worker(anchors: &Anchors, rng: &mut Rng) -> Option<Vec<Stop>> {
    let work = spread(&anchors.work, TILE);
    if work.len() < 2 {
        return None;
    }
    let want = WORK_STOPS.0 + rng.below(WORK_STOPS.1 - WORK_STOPS.0 + 1);
    Some(
        pick(&work, want as usize, rng)
            .into_iter()
            .map(|at| Stop {
                at,
                minutes: minutes(rng, WORK_MINUTES),
            })
            .collect(),
    )
}

/// A civilian's stroll: three or four rooms, each a few tiles from the
/// last one picked.
fn stroll(anchors: &Anchors, rng: &mut Rng) -> Option<Vec<Stop>> {
    let rooms = spread(&anchors.rooms, SPREAD_TILES * TILE);
    if rooms.len() < 2 {
        return None;
    }
    let want = STROLL_STOPS.0 + rng.below(STROLL_STOPS.1 - STROLL_STOPS.0 + 1);
    Some(
        pick(&rooms, want as usize, rng)
            .into_iter()
            .map(|at| Stop {
                at,
                minutes: minutes(rng, STROLL_MINUTES),
            })
            .collect(),
    )
}

/// The fallback: a few spots of open deck the body can reach, well apart.
/// Empty only for a body that can reach no deck at all.
fn wander(anchors: &Anchors, rng: &mut Rng) -> Vec<Stop> {
    let open = spread(&anchors.open, SPREAD_TILES * TILE);
    let open = if open.len() >= 2 {
        open
    } else {
        anchors.open.clone()
    };
    pick(&open, WANDER_STOPS, rng)
        .into_iter()
        .map(|at| Stop {
            at,
            minutes: minutes(rng, WANDER_MINUTES),
        })
        .collect()
}

/// `spots` thinned so no two are closer than `apart`, keeping the first
/// of any bunch in the order given — which is the site's own order, so
/// the answer is the same on every client.
fn spread(spots: &[Vec2], apart: f32) -> Vec<Vec2> {
    let mut kept: Vec<Vec2> = Vec::new();
    for &at in spots {
        if kept.iter().all(|&k| (k - at).len() >= apart) {
            kept.push(at);
        }
    }
    kept
}

/// Up to `n` of `spots`, rolled without putting any back, in the order
/// they were rolled.
fn pick(spots: &[Vec2], n: usize, rng: &mut Rng) -> Vec<Vec2> {
    let mut left = spots.to_vec();
    let mut out = Vec::new();
    while out.len() < n && !left.is_empty() {
        let i = rng.below(left.len() as u32) as usize;
        out.push(left.swap_remove(i));
    }
    out
}

/// The role the `n`th of a site's own people is dealt, off the site's
/// seed: the first a trader where the site trades, a town's guard its
/// guard, and the rest rolled — a guard in five, a worker in three, a
/// civilian otherwise. What the world asks when it opens a site's room;
/// here so the rule is the room's, beside what the roles do.
pub fn deal(n: u32, seed: u64, trades: bool, guard_first: bool) -> Role {
    match (n, guard_first, trades) {
        (0, true, _) => return Role::Guard,
        (0, false, true) | (1, true, true) => return Role::Trader,
        _ => {}
    }
    let mut rng =
        Rng::new(seed ^ (0x_524f_4c45 ^ u64::from(n)).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let roll = rng.below(15);
    if roll < 3 {
        Role::Guard
    } else if roll < 8 {
        Role::Worker
    } else {
        Role::Civilian
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::vec2;

    fn anchors_with(exits: usize, desks: usize, work: usize, rooms: usize) -> Anchors {
        let row = |n: usize, y: f32| -> Vec<Vec2> {
            (0..n)
                .map(|i| vec2(i as f32 * 8.0 * TILE, y * TILE))
                .collect()
        };
        Anchors {
            exits: row(exits, 0.0),
            desks: row(desks, 10.0),
            work: row(work, 20.0),
            rooms: row(rooms, 30.0),
            open: (0..12)
                .map(|i| {
                    vec2(
                        (i % 4) as f32 * 3.0 * TILE,
                        40.0 * TILE + (i / 4) as f32 * 3.0 * TILE,
                    )
                })
                .collect(),
        }
    }

    /// Every role walks its own round where the site has what it wants.
    #[test]
    fn every_role_has_a_round_of_its_own_on_a_site_that_has_its_fixtures() {
        let anchors = anchors_with(3, 1, 4, 5);
        for role in Role::ALL {
            let routine = plan(role, &anchors, 7);
            assert!(!routine.fallback, "{role:?} fell back");
            assert!(routine.stops.len() >= 2 || role == Role::Trader);
            assert!(routine.stops.iter().all(|s| s.minutes > 0.0));
        }
    }

    /// And the fallback's wander where it has not.
    #[test]
    fn a_role_without_its_fixtures_wanders_the_open_deck() {
        let anchors = anchors_with(1, 0, 1, 1);
        for role in Role::ALL {
            let routine = plan(role, &anchors, 7);
            assert!(routine.fallback, "{role:?} did not fall back");
            assert!(routine.stops.len() >= 2);
            assert!(routine.stops.iter().all(|s| anchors.open.contains(&s.at)));
        }
    }

    /// The same seed is the same round, stop for stop.
    #[test]
    fn a_round_is_its_seed() {
        let anchors = anchors_with(3, 2, 5, 6);
        for role in Role::ALL {
            assert_eq!(plan(role, &anchors, 99), plan(role, &anchors, 99));
        }
        assert_eq!(deal(4, 11, true, false), deal(4, 11, true, false));
        assert_eq!(deal(0, 11, true, false), Role::Trader);
        assert_eq!(deal(0, 11, true, true), Role::Guard);
        assert_eq!(deal(1, 11, true, true), Role::Trader);
    }
}
