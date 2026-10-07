//! The second set of attack missions (October 2026, the player's: A1 to
//! A5 — "for now go ahead with those also make maps fitting to that"),
//! their states `crate::objective`'s on the site's `Infestation`:
//!
//! - **Fitting the map**: a site whose mission is one of these is laid out
//!   again for it the step the crew arrive ([`World::fit_the_site`],
//!   `Station::fit`, `stationgen::Feature`): the Overseer's office, three
//!   server rooms, a cell with one door, a fuel depot by the port and a
//!   reactor at the far end, two cargo holds — the rooms farthest from the
//!   port. A station that keeps no contract so built (or the probes'
//!   arena) has its spots found on the plain layout instead.
//! - **Kill the Overseer**: one of theirs, his health the area's
//!   ([`data::OVERSEER_HEALTH`]), walking at [`data::OVERSEER_PACE`] of a
//!   body's pace, stands in his office; a wave lands every
//!   [`data::OVERSEER_WAVE_STEPS`] while he lives. Under
//!   [`data::OVERSEER_FLEES_UNDER`] of his health he walks for the airlock
//!   farthest from his office, and out of it he is gone with the site's
//!   bounty. Down, the waves stop and the deck is cleared as any.
//! - **A data heist**: [`data::HEIST_TERMINALS`] terminals, one a server
//!   room, each [`data::HACK_SECONDS`] of hands (the Use key; an
//!   engineer's two a second). Waves on a clock, sooner and bigger with
//!   every terminal taken ([`data::HEIST_WAVE_STEPS`]) until all are.
//! - **A prison break**: [`data::PRISONERS`] of the Republic in the cell,
//!   crew bots unarmed, held there and hidden from the enemy; its door
//!   sealed until [`data::CUT_SECONDS`] of hands cut it open. Out, they are
//!   armed with a pistol each and follow the crew as any bot, and alive
//!   they stay on; never freed, they are gone at the mission's end. The
//!   site's own waves.
//! - **A fuel run**: [`data::FUEL_DRUMS`] drums in the depot, taken up with
//!   the Use key — the carrier holds its fire — and put in within
//!   [`data::REACTOR_REACH_TILES`] of the reactor's core; a carrier hit
//!   goes up with the drum (the drums' own burst). A drum lost is stood
//!   again in the depot after [`data::FUEL_RESTOCK_STEPS`]. Waves on a
//!   clock until [`data::FUEL_NEEDED`] are in and the reactor goes
//!   critical.
//! - **A salvage sweep**: [`data::SALVAGE_CRATES`] crates in the holds,
//!   each taken up bringing a wave at once, each a fifth bigger; carried
//!   aboard the ship, each pays [`data::SALVAGE_PAY_PERCENT`] of an enemy's
//!   money into the takings. The site's own waves besides, and its
//!   bounty paid as each enemy goes down, so the crew may leave when they
//!   like; it is cleared once every crate is home.
//!
//! **A fight won freezes the deck**, so every one of them holds the clear
//! until its objective is done ([`World::objective_holds`]): the Overseer
//! down or out, every terminal taken, the reactor critical, the cell cut
//! open, every crate home.
//!
//! A child of `crate::world`, as `mission.rs` is. **Nothing here draws
//! from a stream.**

use super::*;
use crate::objective::{
    FuelRun, Heist, Load, LoadState, Objective, Overseer, OverseerPhase, Prison, Salvage,
};
use crate::run::Mission;
use crate::stationgen::Feature;
use shipdesign::Layer;

/// The room's deploy code a terminal's hacking goes as: past the weld's
/// and the charge's; never laid.
pub const HACK_CODE: u32 = 102;
/// The room's deploy code a cell door's cutting goes as.
pub const CUT_CODE: u32 = 103;
/// How near a terminal or a cell door the Bim has to stand, in tiles.
pub const USE_REACH: f32 = 1.75;
/// How long the room's errand runs: longer than any hack or cut.
const ERRAND_MINUTES: f32 = 600.0;

/// What the Use key does for that player's Bim at an attack's mission.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Interaction {
    /// The heist's terminal of that index.
    Hack(usize),
    /// The cell's door.
    Cut,
    /// The drum or the crate of that index taken up.
    Take(usize),
    /// The one carried put down.
    Drop,
    /// Bomb disposal's charge of that index defused.
    Defuse(usize),
}

/// What a mark on the map is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MarkKind {
    /// A terminal, taken or not.
    Terminal { taken: bool },
    /// A cell's door, sealed or cut.
    CellDoor { open: bool },
    /// Where the prisoners are held.
    Cell,
    /// A drum, carried or lying.
    Drum { carried: bool },
    /// A crate, carried or lying.
    Crate { carried: bool },
    /// The reactor's core, critical or not.
    Reactor { critical: bool },
    /// The Overseer.
    Overseer { fleeing: bool },
    /// His airlock.
    Escape,
    /// A charge to defuse, or defused.
    Charge { defused: bool },
    /// A vault's door, standing or broken in.
    Gate { breached: bool },
    /// The site's commander, down or up.
    Vip { down: bool },
}

/// One mark: where in the site's own design units, what, and how far its
/// work has got (nought to one).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Mark {
    pub at: DVec2,
    pub kind: MarkKind,
    pub progress: f32,
}

/// An attack's mission of the second set as the app draws and says it:
/// which, its marks, how much is done of how much, and the seconds to the
/// next wave of its clock.
#[derive(Clone, PartialEq, Debug)]
pub struct ObjectiveLook {
    pub mission: Mission,
    pub marks: Vec<Mark>,
    pub done: u32,
    pub total: u32,
    /// The Overseer's phase code, the reactor critical (1) or the cell
    /// open (1); nought otherwise.
    pub phase: u32,
    pub wave_in: Option<f32>,
    /// The seconds the mission's own timer has left: Bomb disposal's, the
    /// doors' hold, the commander's two minutes.
    pub time_left: Option<f32>,
}

/// The map a mission of the second set wants.
pub fn feature_of(mission: Mission) -> Option<Feature> {
    match mission {
        Mission::Overseer => Some(Feature::Office),
        Mission::Heist => Some(Feature::Servers),
        Mission::Prison => Some(Feature::Brig),
        Mission::FuelRun => Some(Feature::FuelRun),
        Mission::Salvage => Some(Feature::Cargo),
        Mission::Doors => Some(Feature::Vault),
        _ => None,
    }
}

/// Whether a design tile is deck with nothing standing on it.
fn free(design: &ShipDesign, x: i32, y: i32) -> bool {
    let grid = design.grid();
    grid.get(Layer::Floor, (x, y)) != 0 && grid.get(Layer::Object, (x, y)) == 0
}

/// Whether a design tile and its eight neighbours are all free deck.
fn open(design: &ShipDesign, x: i32, y: i32) -> bool {
    (-1..=1).all(|dx| (-1..=1).all(|dy| free(design, x + dx, y + dy)))
}

/// A tile's middle in design units.
fn middle(tile: (u32, u32)) -> DVec2 {
    let t = shipdesign::TILE as f64;
    dvec2((tile.0 as f64 + 0.5) * t, (tile.1 as f64 + 0.5) * t)
}

/// Up to `n` open tiles of `room` (inner tiles, inclusive; the whole
/// design without one), spread out: the first the farthest from `from`
/// (or the nearest, with `near`), each next the farthest from `from` and
/// every one chosen. Ties to the lower row, then column.
pub(super) fn spots(
    design: &ShipDesign,
    room: Option<[u32; 4]>,
    n: usize,
    from: DVec2,
    near: bool,
) -> Vec<(u32, u32)> {
    let side = design.build_area;
    let [x0, y0, x1, y1] = room.unwrap_or([0, 0, side.saturating_sub(1), side.saturating_sub(1)]);
    let mut tiles: Vec<(u32, u32)> = Vec::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            if open(design, x as i32, y as i32) {
                tiles.push((x, y));
            }
        }
    }
    // A small room may have no tile with eight free round it: then any
    // free tile of it.
    if tiles.is_empty() && room.is_some() {
        for y in y0..=y1 {
            for x in x0..=x1 {
                if free(design, x as i32, y as i32) {
                    tiles.push((x, y));
                }
            }
        }
    }
    let mut chosen: Vec<(u32, u32)> = Vec::new();
    while chosen.len() < n {
        let score = |t: (u32, u32)| {
            let p = middle(t);
            let d = p.sub(from).length();
            let base = if near && chosen.is_empty() { -d } else { d };
            chosen
                .iter()
                .fold(base, |s, &c| s.min(p.sub(middle(c)).length()))
        };
        let best = tiles
            .iter()
            .copied()
            .filter(|t| !chosen.contains(t))
            .max_by(|&a, &b| {
                score(a)
                    .total_cmp(&score(b))
                    .then((b.1, b.0).cmp(&(a.1, a.0)))
            });
        let Some(t) = best else {
            break;
        };
        chosen.push(t);
    }
    chosen
}

/// The open tile of `room` nearest its middle.
fn room_middle(design: &ShipDesign, room: [u32; 4]) -> Option<(u32, u32)> {
    let m = dvec2(
        ((room[0] + room[2]) as f64 / 2.0 + 0.5) * shipdesign::TILE as f64,
        ((room[1] + room[3]) as f64 / 2.0 + 0.5) * shipdesign::TILE as f64,
    );
    spots(design, Some(room), 1, m, true).first().copied()
}

impl World {
    /// The second set's objective at the site alongside, with its id.
    fn objective_here(&self) -> Option<(u32, Objective)> {
        let id = self.ship.state.alongside()?;
        let o = self.infestation(id)?.objective.clone()?;
        Some((id, o))
    }

    fn set_objective(&mut self, id: u32, o: Objective) {
        if let Some(it) = self.infestation_mut(id) {
            it.objective = Some(o);
        }
    }

    /// Whether the site `id`'s waves come on its objective's clock (the
    /// Overseer, a heist, a fuel run) rather than its own count.
    pub(super) fn objective_on_a_clock(&self, id: u32) -> bool {
        matches!(
            self.infestation(id).and_then(|it| it.objective.as_ref()),
            Some(Objective::Overseer(_) | Objective::Heist(_) | Objective::FuelRun(_))
        )
    }

    /// Whether the site `id`'s objective keeps it from being cleared: the
    /// Overseer standing, a terminal left, the reactor not critical.
    pub(super) fn objective_holds(&self, id: u32) -> bool {
        match self.infestation(id).and_then(|it| it.objective.as_ref()) {
            Some(Objective::Overseer(o)) => {
                matches!(o.phase, OverseerPhase::Office | OverseerPhase::Fleeing)
            }
            Some(Objective::Heist(h)) => h.taken.iter().any(|&t| !t),
            Some(Objective::FuelRun(f)) => !f.critical,
            // The cell cut open: a fight won freezes the deck, and nobody
            // cuts a door on one.
            Some(Objective::Prison(p)) => !p.open,
            // Every crate home or gone up: the crew may leave before, and
            // are paid as they go (`World::pays_as_it_goes`).
            Some(Objective::Salvage(s)) => s
                .crates
                .iter()
                .any(|c| matches!(c.state, LoadState::Lying | LoadState::Carried)),
            _ => false,
        }
    }

    /// Whether the site alongside pays its bounty as each enemy goes down
    /// rather than at the clear: a salvage sweep, which the crew may leave
    /// whenever they like.
    pub(super) fn pays_as_it_goes(&self) -> bool {
        matches!(self.objective_here(), Some((_, Objective::Salvage(_))))
    }

    // --- the map ---------------------------------------------------------

    /// The site the crew are arriving at laid out again for its mission,
    /// where it is one of the second set (`Station::fit`): before the
    /// rooms are joined, so the deck the crew walk onto is the new one.
    pub(super) fn fit_the_site(&mut self, id: u32) {
        let Some(feature) = feature_of(self.mission_here(id)) else {
            return;
        };
        if let Some(station) = self.stations.iter_mut().find(|s| s.id == id) {
            station.fit(feature, false);
        }
    }

    /// The probes' (the `overseer`, `heist`, `prison`, `fuelrun` and
    /// `salvage` commands): the station tied to laid out again for the
    /// mission forced there, whatever plan it stands on, and docked at
    /// again from scratch as `arena_dock_for_probe` does. False, nothing
    /// moved, with no such mission there or no layout kept the contract.
    pub fn fit_dock_for_probe(&mut self) -> bool {
        let Some(id) = self.ship.state.station() else {
            return false;
        };
        let Some(feature) = feature_of(self.mission_here(id)) else {
            return false;
        };
        let Some(station) = self.stations.iter_mut().find(|s| s.id == id) else {
            return false;
        };
        if !station.fit(feature, true) {
            return false;
        }
        self.undock_for_probe();
        self.residents = None;
        self.ship.state = ShipState::Docked { station: id };
        self.dock_at(id);
        true
    }

    // --- laying it --------------------------------------------------------

    /// The objective laid at `id` the step the crew first dock there,
    /// after its first wave, where its mission is one of the second set.
    pub(super) fn settle_objective(&mut self, id: u32) {
        let mission = self.mission_here(id);
        if feature_of(mission).is_none() {
            return;
        }
        if self
            .infestation(id)
            .is_none_or(|it| it.objective.is_some() || it.cleared || it.heart.is_some())
        {
            return;
        }
        let Some(station) = self.station(id).cloned() else {
            return;
        };
        let Some(port) = station.port() else {
            return;
        };
        let from = dvec2(port.centre.0, port.centre.1);
        let design = &station.design;
        let fitted = station
            .fitted
            .clone()
            .filter(|f| Some(f.feature) == feature_of(mission));
        let room = |i: usize| fitted.as_ref().and_then(|f| f.rooms.get(i).copied());
        let now = self.run.mission_steps;
        let objective = match mission {
            Mission::Overseer => {
                let office = room(0)
                    .and_then(|r| room_middle(design, r))
                    .or_else(|| sabotage::charge_spot(design, port.centre));
                let Some(office) = office else {
                    return;
                };
                let Some(airlock) = sabotage::extraction_of(design, office) else {
                    return;
                };
                Objective::Overseer(Overseer {
                    body: None,
                    office,
                    airlock,
                    phase: OverseerPhase::Office,
                    next_wave: now + data::OVERSEER_WAVE_STEPS,
                })
            }
            Mission::Heist => {
                let n = data::HEIST_TERMINALS as usize;
                let mut terminals: Vec<(u32, u32)> = (0..n)
                    .filter_map(|i| room(i).and_then(|r| room_middle(design, r)))
                    .collect();
                if terminals.len() < n {
                    terminals = spots(design, None, n, from, false);
                }
                if terminals.is_empty() {
                    return;
                }
                Objective::Heist(Heist {
                    work: vec![0; terminals.len()],
                    taken: vec![false; terminals.len()],
                    terminals,
                    next_wave: now + data::HEIST_WAVE_STEPS[0],
                })
            }
            Mission::Prison => {
                let cell = room(0)
                    .and_then(|r| room_middle(design, r))
                    .or_else(|| sabotage::charge_spot(design, port.centre));
                let Some(cell) = cell else {
                    return;
                };
                let door = fitted.as_ref().and_then(|f| f.door);
                // Where the cut is worked from: a free tile beside the
                // door's first or second tile, outside the cell.
                let outside = door.zip(room(0)).and_then(|((x, y), [x0, y0, x1, y1])| {
                    let inside = |a: i32, b: i32| {
                        a >= x0 as i32 && a <= x1 as i32 && b >= y0 as i32 && b <= y1 as i32
                    };
                    let (x, y) = (x as i32, y as i32);
                    [(x, y), (x + 1, y), (x, y + 1)]
                        .into_iter()
                        .flat_map(|(a, b)| [(a + 1, b), (a - 1, b), (a, b + 1), (a, b - 1)])
                        .find(|&(a, b)| !inside(a, b) && free(design, a, b) && a >= 0 && b >= 0)
                        .map(|(a, b)| (a as u32, b as u32))
                });
                let prisoners = self.enlist_prisoners(id, cell);
                if prisoners.is_empty() {
                    return;
                }
                Objective::Prison(Prison {
                    door,
                    outside,
                    cell,
                    cut: 0,
                    open: false,
                    prisoners,
                })
            }
            Mission::FuelRun => {
                let n = data::FUEL_DRUMS as usize;
                let mut depot = room(0)
                    .map(|r| spots(design, Some(r), n, from, true))
                    .unwrap_or_default();
                if depot.len() < n {
                    depot = spots(design, None, n, from, true);
                }
                let reactor = match room(1) {
                    Some([x0, y0, x1, y1]) => {
                        // The core's middle: the generator's two by two in
                        // the middle of the room.
                        let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
                        let (cx, cy) = (x0 + (w - 2) / 2, y0 + (h - 2) / 2);
                        let t = shipdesign::TILE as f64;
                        Some(dvec2((cx + 1) as f64 * t, (cy + 1) as f64 * t))
                    }
                    None => sabotage::charge_spot(design, port.centre).map(middle),
                };
                let Some(reactor) = reactor else {
                    return;
                };
                let drums: Vec<Load> = depot
                    .iter()
                    .filter_map(|&t| self.on_deck(middle(t)))
                    .map(Load::lying)
                    .collect();
                let Some(&first) = drums.first().map(|d| &d.at) else {
                    return;
                };
                Objective::FuelRun(FuelRun {
                    drums,
                    depot: first,
                    reactor: (reactor.x.round() as i32, reactor.y.round() as i32),
                    delivered: 0,
                    critical: false,
                    next_wave: now + data::FUEL_WAVE_STEPS,
                    restock_at: None,
                })
            }
            Mission::Salvage => {
                let n = data::SALVAGE_CRATES as usize;
                let mut tiles: Vec<(u32, u32)> = Vec::new();
                for i in 0..2 {
                    if let Some(r) = room(i) {
                        tiles.extend(spots(design, Some(r), n.div_ceil(2), from, false));
                    }
                }
                if tiles.len() < n {
                    tiles = spots(design, None, n, from, false);
                }
                tiles.truncate(n);
                let crates: Vec<Load> = tiles
                    .iter()
                    .filter_map(|&t| self.on_deck(middle(t)))
                    .map(Load::lying)
                    .collect();
                if crates.is_empty() {
                    return;
                }
                Objective::Salvage(Salvage {
                    crates,
                    taken: 0,
                    home: 0,
                })
            }
            _ => return,
        };
        let clock = matches!(
            objective,
            Objective::Overseer(_) | Objective::Heist(_) | Objective::FuelRun(_)
        );
        if let Some(it) = self.infestation_mut(id) {
            it.objective = Some(objective);
            // Its waves are the objective's clock's from here: the site's
            // own count is done with.
            if clock {
                it.waves_left = 0;
                it.next_wave = None;
            }
        }
        self.lay_the_overseer(id);
    }

    /// A design point of the site on the crew's deck, in whole units.
    fn on_deck(&self, p: DVec2) -> Option<(i32, i32)> {
        let q = self.aboard.from_station(p)?;
        Some((q.x.round() as i32, q.y.round() as i32))
    }

    /// A design tile's middle on the crew's deck.
    pub(super) fn tile_on_deck(&self, tile: (u32, u32)) -> Option<bims::math::Vec2> {
        let q = self.aboard.from_station(middle(tile))?;
        Some(bims::math::vec2(q.x as f32, q.y as f32))
    }

    /// A design point of the site in the residents' room.
    fn in_their_room(&self, p: DVec2) -> Option<bims::math::Vec2> {
        Some(self.residents.as_ref()?.aboard.to_room(p))
    }

    /// The Overseer stood in his office in the residents' room, where the
    /// mission is his and he is not yet laid — his health the area's, the
    /// relics' and the ascension's enemy health on it.
    fn lay_the_overseer(&mut self, id: u32) {
        let Some(Objective::Overseer(o)) = self.infestation(id).and_then(|it| it.objective.clone())
        else {
            return;
        };
        if o.body.is_some() || self.residents.as_ref().is_none_or(|r| r.station != id) {
            return;
        }
        let Some(at) = self.in_their_room(middle(o.office)) else {
            return;
        };
        let seed = self.garrison_seed(id, 0) ^ 0x_4F56_4552;
        self.stand_people(1, &[at], seed);
        let health = data::OVERSEER_HEALTH[self.area_now().0.min(3)]
            * self.enemy_health_factor().unwrap_or(1.0);
        let Some(residents) = self.residents.as_mut() else {
            return;
        };
        let room = &mut residents.aboard.room;
        // The last of the site's people enlisted is he.
        let who = (0..room.crew_count() as usize)
            .rev()
            .find(|&w| room.is_manufacturer(w))
            .unwrap_or(0);
        room.set_level_health(who, 0.0);
        let base = room.max_health(who);
        room.set_level_health(who, (health - base).max(0.0));
        if let Some(Objective::Overseer(o)) = self
            .infestation_mut(id)
            .and_then(|it| it.objective.as_mut())
        {
            o.body = Some(who as u32);
        }
    }

    /// The prisoners stood in the cell on the crew's deck: classless crew
    /// bots in the Republic's coverall, unarmed, held there by a post.
    /// Their crew indices.
    fn enlist_prisoners(&mut self, id: u32, cell: (u32, u32)) -> Vec<u32> {
        let Some(at) = self.tile_on_deck(cell) else {
            return Vec::new();
        };
        let tile = shipdesign::TILE as f32;
        let mut spots = self.aboard.room.free_tiles_near(at, 2.5 * tile);
        if spots.is_empty() {
            spots.push(at);
        }
        let mut out = Vec::new();
        for k in 0..data::PRISONERS as usize {
            let spot = spots[k % spots.len()];
            let seed = worldgen::rng::mix(self.galaxy_seed ^ u64::from(id) ^ (k as u64 + 1) << 40);
            let gear = bims::combat::Gear {
                weapon: None,
                ..Default::default()
            };
            let who = self.aboard.room.enlist_reinforcement(spot, gear, seed) as u32;
            self.crew_down.push(false);
            self.crew_locked.push(false);
            self.aboard.room.post_at(who as usize, spot);
            out.push(who);
        }
        self.size_for_the_reinforcements();
        out
    }

    /// Whether crew member `who` is a prisoner still held: hidden from
    /// the enemy, counted as nobody's bot.
    pub(crate) fn is_captive(&self, who: u32) -> bool {
        matches!(
            self.objective_here(),
            Some((_, Objective::Prison(p))) if !p.open && p.prisoners.contains(&who)
        )
    }

    /// A crew member dropped (`drop_crew_member`): the prisoners' indices
    /// past it moved down, itself off the list.
    pub(super) fn prisoners_after_drop(&mut self, gone: u32) {
        for it in &mut self.infested {
            if let Some(Objective::Prison(p)) = it.objective.as_mut() {
                p.prisoners.retain(|&w| w != gone);
                for w in &mut p.prisoners {
                    if *w > gone {
                        *w -= 1;
                    }
                }
            }
        }
    }

    /// A mission's end: the prisoners never freed gone with the cell,
    /// highest index first.
    pub(super) fn leave_the_prisoners(&mut self) {
        let Some((_, Objective::Prison(p))) = self.objective_here() else {
            return;
        };
        if p.open {
            return;
        }
        let mut gone = p.prisoners.clone();
        gone.sort_unstable();
        for who in gone.into_iter().rev() {
            self.drop_crew_member(who);
        }
    }

    // --- the Use key -------------------------------------------------------

    /// What the Use key does for that player's Bim at an attack's mission
    /// of the second set: put down what it carries, else the nearest
    /// terminal or cell door in reach, else take up the nearest drum or
    /// crate. `OutOfReach` unfit or out of a mission, `NothingToUse`
    /// otherwise.
    pub fn can_interact(&self, slot: u32) -> Result<Interaction, Refusal> {
        if !self.fit_to_act(slot) || slot >= self.players() || !self.in_mission() {
            return Err(Refusal::OutOfReach);
        }
        // Bomb disposal's charges (October 2026).
        if let Some(i) = self.can_defuse(slot) {
            return Ok(Interaction::Defuse(i));
        }
        let Some((_, o)) = self.objective_here() else {
            return Err(Refusal::NothingToUse);
        };
        let me = self.aboard.room.bim_pos(slot as usize);
        let tile = shipdesign::TILE as f32;
        let near = |at: bims::math::Vec2, reach: f32| (at - me).len() <= reach * tile;
        match &o {
            Objective::Heist(h) => h
                .terminals
                .iter()
                .enumerate()
                .filter(|&(i, _)| !h.taken[i])
                .filter_map(|(i, &t)| Some((i, self.tile_on_deck(t)?)))
                .filter(|&(_, at)| near(at, USE_REACH))
                .min_by(|a, b| (a.1 - me).len().total_cmp(&(b.1 - me).len()))
                .map(|(i, _)| Interaction::Hack(i))
                .ok_or(Refusal::NothingToUse),
            Objective::Prison(p) if !p.open => {
                let at = self.cut_spot(p).ok_or(Refusal::NothingToUse)?;
                if near(at, USE_REACH) {
                    Ok(Interaction::Cut)
                } else {
                    Err(Refusal::NothingToUse)
                }
            }
            Objective::FuelRun(FuelRun { drums: loads, .. })
            | Objective::Salvage(Salvage { crates: loads, .. }) => {
                if loads
                    .iter()
                    .any(|l| l.state == LoadState::Carried && l.carrier == Some(slot))
                {
                    return Ok(Interaction::Drop);
                }
                loads
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| l.state == LoadState::Lying)
                    .map(|(i, l)| (i, bims::math::vec2(l.at.0 as f32, l.at.1 as f32)))
                    .filter(|&(_, at)| near(at, data::LOAD_REACH_TILES))
                    .min_by(|a, b| (a.1 - me).len().total_cmp(&(b.1 - me).len()))
                    .map(|(i, _)| Interaction::Take(i))
                    .ok_or(Refusal::NothingToUse)
            }
            _ => Err(Refusal::NothingToUse),
        }
    }

    /// Where the hands go to cut a prison open: the corridor's tile
    /// outside its door, or the cell where the station has no door, on
    /// the crew's deck.
    fn cut_spot(&self, p: &Prison) -> Option<bims::math::Vec2> {
        self.tile_on_deck(p.outside.unwrap_or(p.cell))
    }

    /// [`Command::Interact`]: what [`World::can_interact`] says, done.
    pub(super) fn interact(
        &mut self,
        slot: u32,
        events: &mut Vec<WorldEvent>,
    ) -> Result<(), Refusal> {
        let what = self.can_interact(slot)?;
        if let Interaction::Defuse(i) = what {
            return self.defuse(slot, i);
        }
        let Some((id, o)) = self.objective_here() else {
            return Err(Refusal::NothingToUse);
        };
        match (what, o) {
            (Interaction::Hack(i), Objective::Heist(h)) => {
                let at = self
                    .tile_on_deck(h.terminals[i])
                    .ok_or(Refusal::NothingToUse)?;
                if !self
                    .aboard
                    .room
                    .deploy(slot as usize, at, HACK_CODE, ERRAND_MINUTES)
                {
                    return Err(Refusal::NothingToUse);
                }
            }
            (Interaction::Cut, Objective::Prison(p)) => {
                let at = self.cut_spot(&p).ok_or(Refusal::NothingToUse)?;
                if !self
                    .aboard
                    .room
                    .deploy(slot as usize, at, CUT_CODE, ERRAND_MINUTES)
                {
                    return Err(Refusal::NothingToUse);
                }
            }
            (Interaction::Take(i), mut o) => {
                let hits = self.aboard.room.hits_taken(slot as usize);
                let salvage = matches!(o, Objective::Salvage(_));
                let taken = match &mut o {
                    Objective::FuelRun(f) => {
                        let l = &mut f.drums[i];
                        l.state = LoadState::Carried;
                        l.carrier = Some(slot);
                        l.hits = hits;
                        0
                    }
                    Objective::Salvage(s) => {
                        let l = &mut s.crates[i];
                        l.state = LoadState::Carried;
                        l.carrier = Some(slot);
                        l.hits = hits;
                        s.taken += 1;
                        s.taken
                    }
                    _ => return Err(Refusal::NothingToUse),
                };
                self.set_objective(id, o);
                events.push(WorldEvent::Objective {
                    station: id,
                    what: if salvage { 3 } else { 5 },
                    n: taken,
                    who: slot,
                });
                // Every crate taken up brings a wave, a fifth bigger each.
                if salvage {
                    let full = self.landing_wave_size();
                    let n =
                        (full * (100 + data::SALVAGE_WAVE_GROWTH_PERCENT * taken)).div_ceil(100);
                    self.land_objective_wave(id, n, events);
                }
            }
            (Interaction::Drop, mut o) => {
                let at = self.aboard.room.bim_pos(slot as usize);
                if let Objective::FuelRun(FuelRun { drums: loads, .. })
                | Objective::Salvage(Salvage { crates: loads, .. }) = &mut o
                {
                    for l in loads.iter_mut() {
                        if l.state == LoadState::Carried && l.carrier == Some(slot) {
                            l.state = LoadState::Lying;
                            l.carrier = None;
                            l.at = (at.x.round() as i32, at.y.round() as i32);
                        }
                    }
                }
                self.set_objective(id, o);
            }
            _ => return Err(Refusal::NothingToUse),
        }
        Ok(())
    }

    /// Whether crew member `who` has a drum or a crate in its arms: its
    /// hands are full, and it holds its fire.
    pub(crate) fn carries_a_load(&self, who: u32) -> bool {
        match self.objective_here() {
            Some((_, Objective::FuelRun(FuelRun { drums: loads, .. })))
            | Some((_, Objective::Salvage(Salvage { crates: loads, .. }))) => loads
                .iter()
                .any(|l| l.state == LoadState::Carried && l.carrier == Some(who)),
            _ => false,
        }
    }

    /// The hands on the room's errand `code` at `at` this step: who, and
    /// the steps of work they did between them (an engineer's two).
    pub(super) fn hands_on(&self, code: u32, at: bims::math::Vec2) -> (Vec<usize>, u32) {
        let half = shipdesign::TILE as f32 * 0.5;
        let mut hands = Vec::new();
        let mut work = 0;
        for who in 0..self.aboard.room.crew_count() as usize {
            let Some((tile, c, laying)) = self.aboard.room.deploy_work(who) else {
                continue;
            };
            if c != code || (tile - at).len() > half {
                continue;
            }
            hands.push(who);
            if laying {
                work += if self.is_engineer(who as u32) { 2 } else { 1 };
            }
        }
        (hands, work)
    }

    /// A wave of `n` landing at the site's next way in, on the objective's
    /// account: counted on the infestation and said.
    fn land_objective_wave(&mut self, id: u32, n: u32, events: &mut Vec<WorldEvent>) {
        let wave = match self.infestation_mut(id) {
            Some(it) => {
                it.wave += 1;
                it.wave
            }
            None => return,
        };
        self.lay_held_wave(id, n.max(1), wave);
        events.push(WorldEvent::DroidReinforcements { station: id });
    }

    /// A wave due on the objective's clock: lands when `due` is reached
    /// and fewer than a wave's worth stand, `n` of it. Whether it landed.
    fn clock_wave(&mut self, id: u32, due: u64, n: u32, events: &mut Vec<WorldEvent>) -> bool {
        if self.run.mission_steps < due || self.droids_standing() >= n {
            return false;
        }
        self.land_objective_wave(id, n, events);
        true
    }

    // --- the step ------------------------------------------------------------

    /// The second set's step, after the waves: the hands' work, the loads,
    /// the Overseer, the clock's waves.
    pub(super) fn objective_step(&mut self, events: &mut Vec<WorldEvent>) {
        let Some((id, o)) = self.objective_here() else {
            return;
        };
        if !self.aboard.is_joined() || !self.in_mission() {
            return;
        }
        match o {
            Objective::Overseer(o) => self.overseer_step(id, o, events),
            Objective::Heist(h) => self.heist_step(id, h, events),
            Objective::Prison(p) => self.prison_step(id, p, events),
            Objective::FuelRun(f) => self.fuel_step(id, f, events),
            Objective::Salvage(s) => self.salvage_step(id, s, events),
        }
    }

    fn overseer_step(&mut self, id: u32, mut o: Overseer, events: &mut Vec<WorldEvent>) {
        self.lay_the_overseer(id);
        if let Some(Objective::Overseer(now)) =
            self.infestation(id).and_then(|it| it.objective.clone())
        {
            o = now;
        }
        let Some(body) = o.body else {
            return;
        };
        let Some(residents) = self.residents.as_ref() else {
            return;
        };
        let room = &residents.aboard.room;
        let who = body as usize;
        let alive = room.is_alive(who) && !room.is_down(who) && !room.is_gone(who);
        let share = room.health(who) / room.max_health(who).max(1.0);
        let pos = room.body_pos(who);
        let station = self.station(id).cloned();
        let exit = station.as_ref().and_then(|s| {
            let port = *droidplan::airlocks(&s.design).get(o.airlock as usize)?;
            let inside = dvec2(port.centre.0, port.centre.1).sub(
                dvec2(port.outward.0 as f64, port.outward.1 as f64)
                    .scale(1.0 * shipdesign::TILE as f64),
            );
            Some(residents.aboard.to_room(inside))
        });
        let mut said = None;
        match o.phase {
            OverseerPhase::Office | OverseerPhase::Fleeing if !alive => {
                o.phase = OverseerPhase::Down;
                said = Some(12);
            }
            OverseerPhase::Office if share < data::OVERSEER_FLEES_UNDER => {
                o.phase = OverseerPhase::Fleeing;
                said = Some(10);
            }
            OverseerPhase::Fleeing
                if exit.is_some_and(|e| {
                    (pos - e).len() <= data::OVERSEER_OUT_TILES * shipdesign::TILE as f32
                }) =>
            {
                o.phase = OverseerPhase::Escaped;
                said = Some(11);
            }
            _ => {}
        }
        if o.phase == OverseerPhase::Escaped && said.is_some() {
            // Out of the airlock: off the deck, paying nobody — and the
            // site's bounty gone with him.
            if let Some(residents) = self.residents.as_mut() {
                if let Some(flag) = residents.xp_down.get_mut(who) {
                    *flag = true;
                }
                if let Some(flag) = residents.down.get_mut(who) {
                    *flag = true;
                }
                residents.aboard.room.kill_now(who);
                residents.aboard.room.vanish(who);
            }
            self.run.pending_bounty = 0;
        }
        // His pace, said to the room every step: a third of a body's.
        if let Some(residents) = self.residents.as_mut() {
            let room = &mut residents.aboard.room;
            let mut skills = vec![bims::combat::Skill::NONE; room.crew_count() as usize];
            if let Some(s) = skills.get_mut(who) {
                s.walk = data::OVERSEER_PACE;
            }
            room.set_skills(skills);
        }
        // The waves while he stands.
        if matches!(o.phase, OverseerPhase::Office | OverseerPhase::Fleeing) {
            let n = self.landing_wave_size();
            if self.clock_wave(id, o.next_wave, n, events) {
                o.next_wave = self.run.mission_steps + data::OVERSEER_WAVE_STEPS;
            }
        }
        if let Some(what) = said {
            events.push(WorldEvent::Objective {
                station: id,
                what,
                n: 0,
                who: u32::MAX,
            });
        }
        self.set_objective(id, Objective::Overseer(o));
    }

    fn heist_step(&mut self, id: u32, mut h: Heist, events: &mut Vec<WorldEvent>) {
        for i in 0..h.terminals.len() {
            if h.taken[i] {
                continue;
            }
            let Some(at) = self.tile_on_deck(h.terminals[i]) else {
                continue;
            };
            let (hands, work) = self.hands_on(HACK_CODE, at);
            if work == 0 {
                continue;
            }
            h.work[i] += work;
            if h.work[i] >= data::HACK_SECONDS * 60 {
                h.taken[i] = true;
                for who in hands {
                    self.aboard.room.end_deploy(who);
                }
                let left = h.taken.iter().filter(|&&t| !t).count() as u32;
                events.push(WorldEvent::Objective {
                    station: id,
                    what: 1,
                    n: left,
                    who: u32::MAX,
                });
                // The alarm: the next wave sooner.
                let step = data::HEIST_WAVE_STEPS[(h.taken_count() as usize).min(3)];
                h.next_wave = h.next_wave.min(self.run.mission_steps + step);
            }
        }
        if h.taken.iter().any(|&t| !t) {
            let taken = h.taken_count();
            let full = self.landing_wave_size();
            let n = (full * (100 + data::HEIST_WAVE_GROWTH_PERCENT * taken)).div_ceil(100);
            if self.clock_wave(id, h.next_wave, n, events) {
                h.next_wave =
                    self.run.mission_steps + data::HEIST_WAVE_STEPS[(taken as usize).min(3)];
            }
        }
        self.set_objective(id, Objective::Heist(h));
    }

    fn prison_step(&mut self, id: u32, mut p: Prison, events: &mut Vec<WorldEvent>) {
        if !p.open {
            // The cell's door sealed in both rooms, every step: a room
            // built afresh starts every door unsealed.
            self.seal_the_cell(&p, true);
            let Some(at) = self.cut_spot(&p) else {
                return;
            };
            let (hands, work) = self.hands_on(CUT_CODE, at);
            p.cut += work;
            if p.cut >= data::CUT_SECONDS * 60 {
                p.open = true;
                for who in hands {
                    self.aboard.room.end_deploy(who);
                }
                self.seal_the_cell(&p, false);
                // Out: a pistol each, the post lifted — a bot of the crew's.
                for &who in &p.prisoners {
                    let w = who as usize;
                    if !self.aboard.room.is_alive(w) {
                        continue;
                    }
                    let mut gear = self.aboard.room.gear(w);
                    gear.weapon = Some(WeaponKind::LaserPistol.basic());
                    self.aboard.room.issue(w, gear);
                    self.aboard.room.stand_down(w);
                }
                events.push(WorldEvent::Objective {
                    station: id,
                    what: 9,
                    n: p.prisoners.len() as u32,
                    who: u32::MAX,
                });
            }
        }
        self.set_objective(id, Objective::Prison(p));
    }

    /// The cell's door sealed shut, or cut open, in both rooms.
    fn seal_the_cell(&mut self, p: &Prison, sealed: bool) {
        let Some(door) = p.door else {
            return;
        };
        let t = shipdesign::TILE as f64;
        // The middle of the door's first tile is in its opening.
        let in_design = dvec2((door.0 as f64 + 0.5) * t, (door.1 as f64 + 0.5) * t);
        if let Some(q) = self.aboard.from_station(in_design) {
            let at = bims::math::vec2(q.x as f32, q.y as f32);
            if let Some(i) = self.aboard.room.door_index_at(at) {
                self.aboard.room.seal_door(i, sealed);
            }
        }
        if let Some(residents) = self.residents.as_mut() {
            let at = residents.aboard.to_room(in_design);
            if let Some(i) = residents.aboard.room.door_index_at(at) {
                residents.aboard.room.seal_door(i, sealed);
            }
        }
    }

    fn fuel_step(&mut self, id: u32, mut f: FuelRun, events: &mut Vec<WorldEvent>) {
        let tile = shipdesign::TILE as f32;
        let reactor = self
            .aboard
            .from_station(dvec2(f.reactor.0 as f64, f.reactor.1 as f64))
            .map(|q| bims::math::vec2(q.x as f32, q.y as f32));
        let now = self.run.mission_steps;
        for d in f.drums.iter_mut() {
            if d.state != LoadState::Carried {
                continue;
            }
            let Some(who) = d.carrier else {
                d.state = LoadState::Lying;
                continue;
            };
            let w = who as usize;
            let at = self.aboard.room.bim_pos(w);
            d.at = (at.x.round() as i32, at.y.round() as i32);
            // A hit on the carrier goes up with the drum.
            if self.aboard.room.hits_taken(w) > d.hits {
                self.aboard.room.burst_drum(at);
                d.state = LoadState::Lost;
                d.carrier = None;
                f.restock_at.get_or_insert(now + data::FUEL_RESTOCK_STEPS);
                events.push(WorldEvent::Objective {
                    station: id,
                    what: 6,
                    n: 0,
                    who,
                });
                continue;
            }
            if !self.fit_to_act(who) || self.aboard.room.is_down(w) {
                d.state = LoadState::Lying;
                d.carrier = None;
                continue;
            }
            if reactor.is_some_and(|r| (r - at).len() <= data::REACTOR_REACH_TILES * tile) {
                d.state = LoadState::Home;
                d.carrier = None;
                f.delivered += 1;
                events.push(WorldEvent::Objective {
                    station: id,
                    what: 7,
                    n: f.delivered,
                    who,
                });
            }
        }
        if !f.critical && f.delivered >= data::FUEL_NEEDED {
            f.critical = true;
            if let Some(r) = reactor {
                // The core goes up in a ring of the drums' own bursts.
                for (dx, dy) in [(0.0, 0.0), (1.5, 0.0), (-1.5, 0.0), (0.0, 1.5), (0.0, -1.5)] {
                    self.aboard
                        .room
                        .burst_drum(r + bims::math::vec2(dx * tile, dy * tile));
                }
            }
            events.push(WorldEvent::Objective {
                station: id,
                what: 8,
                n: f.delivered,
                who: u32::MAX,
            });
        }
        // A drum lost stood again in the depot.
        if let Some(due) = f.restock_at
            && now >= due
        {
            f.restock_at = None;
            if !f.critical {
                f.drums.push(Load::lying(f.depot));
                events.push(WorldEvent::Objective {
                    station: id,
                    what: 13,
                    n: 0,
                    who: u32::MAX,
                });
            }
        }
        if !f.critical {
            let n = self.landing_wave_size();
            if self.clock_wave(id, f.next_wave, n, events) {
                f.next_wave = self.run.mission_steps + data::FUEL_WAVE_STEPS;
            }
        }
        self.set_objective(id, Objective::FuelRun(f));
    }

    fn salvage_step(&mut self, id: u32, mut s: Salvage, events: &mut Vec<WorldEvent>) {
        let mut paid = 0;
        for c in s.crates.iter_mut() {
            if c.state != LoadState::Carried {
                continue;
            }
            let Some(who) = c.carrier else {
                c.state = LoadState::Lying;
                continue;
            };
            let w = who as usize;
            let at = self.aboard.room.bim_pos(w);
            c.at = (at.x.round() as i32, at.y.round() as i32);
            if !self.fit_to_act(who) || self.aboard.room.is_down(w) {
                c.state = LoadState::Lying;
                c.carrier = None;
                continue;
            }
            if self.on_the_ship(at) {
                c.state = LoadState::Home;
                c.carrier = None;
                s.home += 1;
                paid += 1;
                events.push(WorldEvent::Objective {
                    station: id,
                    what: 4,
                    n: s.home,
                    who,
                });
            }
        }
        if paid > 0 {
            let each = self.enemy_money_on(self.run_day())
                * Money::from(self.players().max(1))
                * data::SALVAGE_PAY_PERCENT
                / 100;
            self.money += each * paid;
        }
        self.set_objective(id, Objective::Salvage(s));
    }

    /// Where the enemies make for at an attack of the second set: the
    /// Overseer to his office, or his airlock once he flees. Nothing for
    /// anybody else.
    pub(super) fn objective_spots(&self) -> Vec<Option<bims::math::Vec2>> {
        let Some((id, Objective::Overseer(o))) = self.objective_here() else {
            return Vec::new();
        };
        let Some(body) = o.body else {
            return Vec::new();
        };
        let Some(residents) = self.residents.as_ref() else {
            return Vec::new();
        };
        let to = match o.phase {
            OverseerPhase::Office => residents.aboard.to_room(middle(o.office)),
            OverseerPhase::Fleeing => {
                let Some(station) = self.station(id) else {
                    return Vec::new();
                };
                let Some(port) = droidplan::airlocks(&station.design)
                    .get(o.airlock as usize)
                    .copied()
                else {
                    return Vec::new();
                };
                residents
                    .aboard
                    .to_room(dvec2(port.centre.0, port.centre.1))
            }
            _ => return Vec::new(),
        };
        let room = &residents.aboard.room;
        (0..room.body_count() as usize)
            .map(|b| (b == body as usize).then_some(to))
            .collect()
    }

    // --- what the app draws ------------------------------------------------

    /// The second set's mission at the site alongside as the app draws it.
    pub fn objective_look(&self) -> Option<ObjectiveLook> {
        let Some((id, o)) = self.objective_here() else {
            return self.guard_look();
        };
        let station = self.station(id)?;
        let from_deck = |at: (i32, i32)| self.aboard.to_station(dvec2(at.0 as f64, at.1 as f64));
        let carried_at = |l: &Load| match (l.state, l.carrier) {
            (LoadState::Carried, Some(w)) => {
                let p = self.aboard.room.bim_pos(w as usize);
                self.aboard.to_station(dvec2(p.x as f64, p.y as f64))
            }
            _ => from_deck(l.at),
        };
        let wave_in = |due: u64| Some(due.saturating_sub(self.run.mission_steps) as f32 / 60.0);
        let mut marks = Vec::new();
        let look = match o {
            Objective::Overseer(o) => {
                if let Some(port) = droidplan::airlocks(&station.design).get(o.airlock as usize) {
                    marks.push(Mark {
                        at: dvec2(port.centre.0, port.centre.1),
                        kind: MarkKind::Escape,
                        progress: 0.0,
                    });
                }
                let alive = matches!(o.phase, OverseerPhase::Office | OverseerPhase::Fleeing);
                if alive && let (Some(body), Some(residents)) = (o.body, self.residents.as_ref()) {
                    let room = &residents.aboard.room;
                    let p = room.body_pos(body as usize);
                    let share =
                        room.health(body as usize) / room.max_health(body as usize).max(1.0);
                    marks.push(Mark {
                        at: residents.aboard.to_design(p),
                        kind: MarkKind::Overseer {
                            fleeing: o.phase == OverseerPhase::Fleeing,
                        },
                        progress: share,
                    });
                }
                ObjectiveLook {
                    mission: Mission::Overseer,
                    marks,
                    done: u32::from(o.phase == OverseerPhase::Down),
                    total: 1,
                    phase: o.phase.code(),
                    wave_in: alive.then(|| wave_in(o.next_wave)).flatten(),
                    time_left: None,
                }
            }
            Objective::Heist(h) => {
                for (i, &t) in h.terminals.iter().enumerate() {
                    marks.push(Mark {
                        at: middle(t),
                        kind: MarkKind::Terminal { taken: h.taken[i] },
                        progress: (h.work[i] as f32 / (data::HACK_SECONDS * 60) as f32).min(1.0),
                    });
                }
                let left = h.taken.iter().any(|&t| !t);
                ObjectiveLook {
                    mission: Mission::Heist,
                    marks,
                    done: h.taken_count(),
                    total: h.terminals.len() as u32,
                    phase: 0,
                    wave_in: left.then(|| wave_in(h.next_wave)).flatten(),
                    time_left: None,
                }
            }
            Objective::Prison(p) => {
                marks.push(Mark {
                    at: middle(p.cell),
                    kind: MarkKind::Cell,
                    progress: 0.0,
                });
                // The door's mark where it is cut from, outside it.
                if let Some(d) = p.outside.or(p.door) {
                    marks.push(Mark {
                        at: middle(d),
                        kind: MarkKind::CellDoor { open: p.open },
                        progress: (p.cut as f32 / (data::CUT_SECONDS * 60) as f32).min(1.0),
                    });
                }
                let alive = p
                    .prisoners
                    .iter()
                    .filter(|&&w| self.aboard.room.is_alive(w as usize))
                    .count() as u32;
                ObjectiveLook {
                    mission: Mission::Prison,
                    marks,
                    done: alive,
                    total: p.prisoners.len() as u32,
                    phase: u32::from(p.open),
                    wave_in: None,
                    time_left: None,
                }
            }
            Objective::FuelRun(f) => {
                marks.push(Mark {
                    at: dvec2(f.reactor.0 as f64, f.reactor.1 as f64),
                    kind: MarkKind::Reactor {
                        critical: f.critical,
                    },
                    progress: f.delivered as f32 / data::FUEL_NEEDED as f32,
                });
                for d in &f.drums {
                    if matches!(d.state, LoadState::Lying | LoadState::Carried)
                        && let Some(at) = carried_at(d)
                    {
                        marks.push(Mark {
                            at,
                            kind: MarkKind::Drum {
                                carried: d.state == LoadState::Carried,
                            },
                            progress: 0.0,
                        });
                    }
                }
                ObjectiveLook {
                    mission: Mission::FuelRun,
                    marks,
                    done: f.delivered,
                    total: data::FUEL_NEEDED,
                    phase: u32::from(f.critical),
                    wave_in: (!f.critical).then(|| wave_in(f.next_wave)).flatten(),
                    time_left: None,
                }
            }
            Objective::Salvage(s) => {
                for c in &s.crates {
                    if matches!(c.state, LoadState::Lying | LoadState::Carried)
                        && let Some(at) = carried_at(c)
                    {
                        marks.push(Mark {
                            at,
                            kind: MarkKind::Crate {
                                carried: c.state == LoadState::Carried,
                            },
                            progress: 0.0,
                        });
                    }
                }
                ObjectiveLook {
                    mission: Mission::Salvage,
                    marks,
                    done: s.home,
                    total: s.crates.len() as u32,
                    phase: 0,
                    wave_in: None,
                    time_left: None,
                }
            }
        };
        Some(look)
    }

    /// A probe's way to the Overseer's fight: him hurt under the share he
    /// flees at, for the tests.
    pub fn hurt_the_overseer_for_probe(&mut self) {
        let Some((_, Objective::Overseer(o))) = self.objective_here() else {
            return;
        };
        let Some(body) = o.body else {
            return;
        };
        if let Some(residents) = self.residents.as_mut() {
            let room = &mut residents.aboard.room;
            let max = room.max_health(body as usize);
            let now = room.health(body as usize);
            let want = max * (data::OVERSEER_FLEES_UNDER - 0.1);
            if now > want {
                room.strike(body as usize, now - want, false);
            }
        }
    }
}
