//! Where the power goes: which parts are on a network with a reactor.
//!
//! A part has a [`PartDef::power`](crate::parts::PartDef::power) figure —
//! made by the reactor, drawn by what consumes — and it only counts if the
//! part is **wired**: a tile of its footprint carries a
//! [`PartKind::PowerConduit`] on the utility layer, and that conduit is on a
//! network that reaches a reactor. The utility layer exists so that a
//! conduit can run through the tile of the thing it feeds, which is why
//! there is no adjacency rule here: under it or not at all.
//!
//! A **network** is one connected run of conduit — four-neighbour, the same
//! flood as the radiation fill and the connectivity check, over a different
//! layer — plus one more join: two conduit tiles under the **same part** are
//! on one network whether or not the run between them is laid. The part is
//! the join. Without that a reactor with conduit under two of its tiles from
//! two runs would be two half-reactors, each making the whole of its output.
//! Only a part with a power figure joins — a table the run passes under
//! is not a wire.
//!
//! What this hands back is read in three places and has to be read the same
//! way in all of them: [`crate::validate`] warns about a consumer on no live
//! network, about a network drawing more than it makes and about engines
//! the reactors cannot feed flat out; the world runs the battery on
//! [`budget`] every step; and `flight::dynamics` asks [`thrust`] how hard
//! the wired engines may push. Nothing here renders, and nothing here
//! decides what happens in a brownout — that is the world's, and
//! `parts::essential` is what it reads.
//!
//! # The engines are on the network too
//!
//! There is no fuel. A main engine is a consumer like the smelter, wired
//! the same way, and it draws
//! [`PartDef::thrust_power`](crate::parts::PartDef::thrust_power) — but only
//! while it burns, so that figure is kept apart from the day-long draw as
//! [`Network::engine_draw`]. What the reactors have over after the
//! day-long draws is what the engines get, and [`thrust`] turns that into a
//! **throttle**: the engines facing one way share it, and push that
//! fraction of their full thrust. An engine on no live network pushes
//! nothing at all. The batteries are not in it: a plan is closed form and
//! a burn that ran off the batteries for a while and then off the reactor
//! would be a rate change halfway through a segment.

use physics::{EngineSpec, Facing};

use crate::design::{PlacedPart, ShipDesign};
use crate::parts::{Layer, PartKind};

/// One connected run of conduit and everything wired to it.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Network {
    /// The conduit tiles, in row order.
    pub tiles: Vec<(u32, u32)>,
    /// Every part **with a power figure** standing on one of them —
    /// reactors, batteries, consumers and engines — by id, ascending. The
    /// frame and the deck under the run, and a table it passes beneath, are
    /// not on the network in any sense that matters.
    pub parts: Vec<u32>,
    /// Units a minute made, over the reactors on it.
    pub supply: f64,
    /// Units a minute drawn all day, over the consumers on it. Positive.
    /// The engines are not in it.
    pub draw: f64,
    /// Units a minute the main engines on it would draw burning flat out,
    /// all of them at once. Positive. What they actually draw is a
    /// fraction of it — see [`thrust`].
    pub engine_draw: f64,
    /// Units held at most, over the batteries on it.
    pub storage: f64,
}

impl Network {
    /// Whether anything on it is powered at all: it has a reactor.
    pub fn live(&self) -> bool {
        self.supply > 0.0
    }

    /// Whether it draws more than it makes. A battery only delays that;
    /// the world's brownout is what it becomes.
    pub fn short(&self) -> bool {
        self.draw > self.supply
    }
}

/// The ship's power as one figure each, summed over the **live** networks.
///
/// Networks are pooled here on purpose. Two reactors on two runs each with
/// its own consumers are, to the crew, one ship with so much power; the
/// per-network question — is *this* run short — is the validator's, and it
/// asks [`networks`] directly.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Budget {
    pub supply: f64,
    pub draw: f64,
    /// The engines' full draw, all of them burning at once. See
    /// [`Network::engine_draw`].
    pub engine_draw: f64,
    pub storage: f64,
}

impl Budget {
    /// What the reactors have over for the engines once the day-long draw
    /// is paid: never less than nothing.
    pub fn spare(&self) -> f64 {
        (self.supply - self.draw).max(0.0)
    }
}

/// Whether a part has a power figure of any kind, and so joins a network.
fn electrical(part: &PlacedPart) -> bool {
    let def = part.kind.def();
    def.supplies() || def.draws() || def.stores() || def.pushes()
}

/// Every network of the design, in order of their first tile in row order.
pub fn networks(design: &ShipDesign) -> Vec<Network> {
    let grid = design.grid();
    let side = grid.side();
    let at = |(x, y): (u32, u32)| (y as usize) * (side as usize) + x as usize;
    let conduit = |tile: (i32, i32)| {
        let id = grid.get(Layer::Utility, tile);
        id != 0
            && design
                .part(id)
                .is_some_and(|p| p.kind == PartKind::PowerConduit)
    };

    // Union-find over the grid: every conduit tile joins its four
    // neighbours, and then every part joins the conduit tiles under it.
    let mut parent: Vec<usize> = (0..(side as usize) * (side as usize)).collect();
    fn root(parent: &mut [usize], i: usize) -> usize {
        let mut i = i;
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    fn unite(parent: &mut [usize], a: usize, b: usize) {
        let (a, b) = (root(parent, a), root(parent, b));
        if a != b {
            // The lower index wins, so a network is named by its first tile.
            let (lo, hi) = if a < b { (a, b) } else { (b, a) };
            parent[hi] = lo;
        }
    }

    for y in 0..side {
        for x in 0..side {
            let tile = (x as i32, y as i32);
            if !conduit(tile) {
                continue;
            }
            for next in [(tile.0 + 1, tile.1), (tile.0, tile.1 + 1)] {
                if conduit(next) {
                    unite(&mut parent, at((x, y)), at((next.0 as u32, next.1 as u32)));
                }
            }
        }
    }
    for part in design.parts.iter().filter(|p| electrical(p)) {
        let wired: Vec<(u32, u32)> = part
            .tiles()
            .into_iter()
            .filter(|&(x, y)| conduit((x as i32, y as i32)))
            .collect();
        for pair in wired.windows(2) {
            unite(&mut parent, at(pair[0]), at(pair[1]));
        }
    }

    // Gather. Networks come out in the order of their root, which is their
    // first tile in row order; the tiles within each are in row order too.
    let mut roots: Vec<usize> = Vec::new();
    let mut nets: Vec<Network> = Vec::new();
    for y in 0..side {
        for x in 0..side {
            if !conduit((x as i32, y as i32)) {
                continue;
            }
            let r = root(&mut parent, at((x, y)));
            let i = match roots.iter().position(|&seen| seen == r) {
                Some(i) => i,
                None => {
                    roots.push(r);
                    nets.push(Network {
                        tiles: Vec::new(),
                        parts: Vec::new(),
                        supply: 0.0,
                        draw: 0.0,
                        engine_draw: 0.0,
                        storage: 0.0,
                    });
                    nets.len() - 1
                }
            };
            nets[i].tiles.push((x, y));
        }
    }
    for part in design.parts.iter().filter(|p| electrical(p)) {
        let Some(tile) = part
            .tiles()
            .into_iter()
            .find(|&(x, y)| conduit((x as i32, y as i32)))
        else {
            continue;
        };
        let r = root(&mut parent, at(tile));
        let i = roots
            .iter()
            .position(|&seen| seen == r)
            .expect("a wired tile is on a network");
        let net = &mut nets[i];
        net.parts.push(part.id);
        let def = part.kind.def();
        if def.supplies() {
            net.supply += def.power;
        } else if def.draws() {
            net.draw -= def.power;
        }
        net.engine_draw += def.thrust_power;
        net.storage += def.charge;
    }
    for net in &mut nets {
        net.parts.sort_unstable();
    }
    nets
}

/// Every part on a live network, by id, ascending. The one list the
/// questions below share, and what the world keeps a copy of between
/// changes to the ship (`World::on_ship_changed`): it is a union-find
/// over the grid, and `is_powered` asked of every lamp every step would
/// be that once a lamp.
pub fn powered_parts(design: &ShipDesign) -> Vec<u32> {
    live_parts(design)
}

fn live_parts(design: &ShipDesign) -> Vec<u32> {
    let mut out: Vec<u32> = networks(design)
        .into_iter()
        .filter(|net| net.live())
        .flat_map(|net| net.parts)
        .collect();
    out.sort_unstable();
    out
}

/// Whether `part_id` is on a live network. True for anything standing
/// over a live run whether or not it draws — the question is about the
/// wiring, not the part.
pub fn is_powered(design: &ShipDesign, part_id: u32) -> bool {
    live_parts(design).binary_search(&part_id).is_ok()
}

/// Every consumer not on a live network, by id, ascending — the engines
/// among them, since an engine with no reactor behind it pushes nothing.
/// What the validator points at.
pub fn unpowered(design: &ShipDesign) -> Vec<u32> {
    let live = live_parts(design);
    let mut out: Vec<u32> = design
        .parts
        .iter()
        .filter(|p| p.kind.def().draws() || p.kind.def().pushes())
        .filter(|p| live.binary_search(&p.id).is_err())
        .map(|p| p.id)
        .collect();
    out.sort_unstable();
    out
}

/// The ship's power over its live networks. See [`Budget`].
pub fn budget(design: &ShipDesign) -> Budget {
    let mut out = Budget {
        supply: 0.0,
        draw: 0.0,
        engine_draw: 0.0,
        storage: 0.0,
    };
    for net in networks(design).iter().filter(|n| n.live()) {
        out.supply += net.supply;
        out.draw += net.draw;
        out.engine_draw += net.engine_draw;
        out.storage += net.storage;
    }
    out
}

/// How hard the ship can push, and what that costs it: the reactor's half
/// of a trip.
///
/// Worked out per **facing**, because only one set of engines burns at a
/// time — the forward set on the way out, the forward set again after a
/// flip or the backward set without one — so the spare is not split
/// between them. Each set's throttle is what the spare covers of its full
/// draw, capped at one, and its power is what it then draws a minute. An
/// engine on no live network is not in any set.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Thrust {
    /// Fraction of full thrust the wired engines facing forward can be
    /// fed, `0.0` to `1.0`. One with nothing to throttle.
    pub forward_throttle: f64,
    pub backward_throttle: f64,
    /// Units a minute the forward set draws while it burns, throttled.
    pub forward_power: f64,
    pub backward_power: f64,
    /// The wired engines with the push the reactors can actually feed —
    /// each one's thrust times its facing's throttle — for
    /// `flight::dynamics` to accelerate by in place of the table's raw
    /// figures. An engine on no live network is left out; one facing
    /// sideways is in, at full thrust, and pushes nothing the autopilot
    /// flies, as before.
    pub engines: Vec<EngineSpec>,
}

impl Thrust {
    /// Whether the reactors cannot feed some set of engines flat out.
    pub fn throttled(&self) -> bool {
        self.forward_throttle < 1.0 || self.backward_throttle < 1.0
    }

    /// How many fed engines face `facing`. What the painter lights.
    pub fn count(&self, facing: Facing) -> u32 {
        self.engines.iter().filter(|e| e.facing == facing).count() as u32
    }
}

/// The wired engines and how much of their push the reactors can feed.
/// See [`Thrust`].
pub fn thrust(design: &ShipDesign) -> Thrust {
    let budget = budget(design);
    let live = live_parts(design);
    let wired: Vec<&PlacedPart> = design
        .parts
        .iter()
        .filter(|p| p.kind.def().pushes() && live.binary_search(&p.id).is_ok())
        .collect();
    let set = |facing: Facing| -> (f64, f64) {
        let full: f64 = wired
            .iter()
            .filter(|p| p.rotation.facing() == facing)
            .map(|p| p.kind.def().thrust_power)
            .sum();
        if full <= 0.0 {
            return (1.0, 0.0);
        }
        let throttle = (budget.spare() / full).min(1.0);
        (throttle, full * throttle)
    };
    let (forward_throttle, forward_power) = set(Facing::Forward);
    let (backward_throttle, backward_power) = set(Facing::Backward);
    let engines = wired
        .iter()
        .map(|p| {
            let facing = p.rotation.facing();
            let throttle = match facing {
                Facing::Forward => forward_throttle,
                Facing::Backward => backward_throttle,
                Facing::Left | Facing::Right => 1.0,
            };
            EngineSpec {
                thrust: p.kind.def().thrust * throttle,
                facing,
            }
        })
        .collect();
    Thrust {
        forward_throttle,
        backward_throttle,
        forward_power,
        backward_power,
        engines,
    }
}
