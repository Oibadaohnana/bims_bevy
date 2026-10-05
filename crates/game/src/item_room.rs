//! What three of the items' third step (October 2026) are in the room: a
//! *Smoke Launcher*'s cloud, a *Tether Link*'s line and a *Decoy
//! Projector*'s ghost. A child of `game`, as `intruder.rs` is.
//!
//! **The room keeps nothing of them.** The world keeps every cloud, link
//! and ghost (`world::items::ItemClocks`) and says them here every step
//! before the rooms step, so nothing below is saved or hashed. What the
//! room does with them is two things only: a cloud said with
//! `blocks_sight` is in the way of every line of sight in this room
//! ([`Game::shut_now`] puts its tiles with the shut doors) — the residents'
//! room, where the machines look from — and everything is drawn in the
//! crew's room, which is told the same clouds without it.

use super::{Game, TILE};
use crate::draw::Color;
use crate::math::{Rect, TAU, Vec2, vec2};

/// A smoke cloud, in this room's own units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SmokeCloud {
    pub at: Vec2,
    /// Its radius, in room units.
    pub radius: f32,
    /// Seconds it has hung, for the churn of the picture.
    pub age: f32,
    /// How thick it is drawn, nought to one: thinning as it runs out.
    pub thick: f32,
}

/// A decoy's ghost as it is drawn: whose figure, where, which way it
/// faces, how far its stride has come and how fast it walks.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GhostLook {
    pub owner: usize,
    pub at: Vec2,
    pub heading: f32,
    pub stride: f32,
    pub speed: f32,
    /// How solid it is drawn, nought to one: fading as it runs out.
    pub fade: f32,
}

/// Everything the world said this step.
#[derive(Clone, Debug, Default)]
pub struct ItemLooks {
    smoke: Vec<SmokeCloud>,
    smoke_blocks: bool,
    ghosts: Vec<GhostLook>,
    /// `(carrier, crewmate)`, crew indices.
    tethers: Vec<(usize, usize)>,
    /// Who is under an *Adrenal Injector*'s rush, with its seconds left.
    rushing: Vec<(usize, f32)>,
    /// Which bots a *Targeting Uplink* lifts.
    uplinked: Vec<usize>,
}

/// How solid a ghost is drawn at the most.
pub const GHOST_OPACITY: f32 = 0.42;
/// The smoke's grey.
const SMOKE: Color = Color::rgb(0.58, 0.60, 0.62);
/// The smoke's darker underside.
const SMOKE_DARK: Color = Color::rgb(0.36, 0.38, 0.40);
/// How many particles a cloud churns with.
const PARTICLES: u32 = 40;
/// How many big, slow puffs make its body under them.
const BODY_PUFFS: u32 = 6;
/// How many puffs the canister throws out going off.
const POP_PUFFS: u32 = 12;
/// Seconds the canister's pop takes to spread.
const POP_SECONDS: f32 = 0.6;
/// The smoke's palest particles.
const SMOKE_PALE: Color = Color::rgb(0.78, 0.80, 0.82);
/// An Adrenal Injector's rush, a hot red lit past white.
const RUSH: Color = Color::rgb(1.0, 0.30, 0.22);
/// A Targeting Uplink's ring under a lifted bot.
const UPLINK: Color = Color::rgb(0.42, 0.88, 0.55);
/// A tether's line, the crew's blue lit past white.
const TETHER: Color = Color::rgb(0.45, 0.85, 1.0);

/// A number off a seed and an index, nought to one: where a puff sits.
/// A hash, never a roll — the picture draws nothing off any stream.
fn scatter(seed: u32, i: u32) -> f32 {
    let mut h = seed.wrapping_mul(0x9E37_79B1) ^ i.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 13;
    (h & 0xFFFF) as f32 / 65535.0
}

impl Game {
    /// The smoke clouds this step (October 2026), in this room's units,
    /// and whether they stop a line of sight here — the residents' room,
    /// where the enemy looks from — or are only drawn — the crew's.
    pub fn set_smoke(&mut self, clouds: Vec<SmokeCloud>, blocks_sight: bool) {
        self.item_looks.smoke = clouds;
        self.item_looks.smoke_blocks = blocks_sight;
    }

    /// The decoys' ghosts this step, to draw.
    pub fn set_ghosts(&mut self, ghosts: Vec<GhostLook>) {
        self.item_looks.ghosts = ghosts;
    }

    /// The tether links this step, `(carrier, crewmate)` by crew index,
    /// to draw.
    pub fn set_tethers(&mut self, links: Vec<(usize, usize)>) {
        self.item_looks.tethers = links;
    }

    /// Who is under an Adrenal Injector's rush (with its seconds left)
    /// and which bots a Targeting Uplink lifts, this step, to draw.
    pub fn set_item_auras(&mut self, rushing: Vec<(usize, f32)>, uplinked: Vec<usize>) {
        self.item_looks.rushing = rushing;
        self.item_looks.uplinked = uplinked;
    }

    /// Every tile a cloud said with `blocks_sight` covers — its middle
    /// within the radius — a hair inside its edges so it marks that tile
    /// and no neighbour: what [`Game::shut_now`] adds to the shut doors.
    pub(super) fn smoke_in_the_way(&self) -> Vec<Rect> {
        if !self.item_looks.smoke_blocks {
            return Vec::new();
        }
        let mut out = Vec::new();
        for cloud in &self.item_looks.smoke {
            let reach = (cloud.radius / TILE).ceil() as i32 + 1;
            let (cx, cy) = (
                (cloud.at.x / TILE).floor() as i32,
                (cloud.at.y / TILE).floor() as i32,
            );
            for ty in cy - reach..=cy + reach {
                for tx in cx - reach..=cx + reach {
                    let middle = vec2((tx as f32 + 0.5) * TILE, (ty as f32 + 0.5) * TILE);
                    if (middle - cloud.at).len() <= cloud.radius {
                        let min = vec2(tx as f32 * TILE + 1.0, ty as f32 * TILE + 1.0);
                        out.push(Rect::from_min_size(min, vec2(TILE - 2.0, TILE - 2.0)));
                    }
                }
            }
        }
        out
    }

    /// A walk for a ghost of crew member `who` to `to`, on the body's own
    /// grid — the deck's, or its window's on a plain: the waypoints from
    /// where it stands, `None` where there is no way there.
    pub fn ghost_route(&self, who: usize, to: Vec2) -> Option<Vec<Vec2>> {
        let from = self.bims.get(who)?.character.pos;
        let nav = self.nav_for(who);
        let goal = nav.nearest_free(to);
        if !nav.can_reach(nav.nearest_free(from), goal) {
            return None;
        }
        let route = nav.path(from, goal);
        (!route.is_empty()).then_some(route)
    }

    /// The clouds, the links and the ghosts, drawn over the bodies: what
    /// `render` asks after them. Nothing for a cloud said to block sight
    /// — that room is not the one drawn.
    pub(super) fn draw_item_looks(&mut self) {
        if !self.item_looks.smoke_blocks {
            for (n, cloud) in self.item_looks.smoke.clone().iter().enumerate() {
                self.draw_smoke(cloud, n as u32);
            }
        }
        // The rush: a hot ring breathing round the body and sparks of it
        // thrown off, faster as it runs out.
        for &(who, left) in &self.item_looks.rushing.clone() {
            let Some(bim) = self.bims.get(who) else {
                continue;
            };
            let at = bim.character.drawn_at();
            let beat = 0.5 + 0.5 * (left * 11.0).sin();
            self.list
                .ring(at, 52.0 + 8.0 * beat, 3.0, RUSH.alpha(0.35 + 0.35 * beat));
            self.list
                .ring(at, 40.0, 1.5, RUSH.glowing(1.3).alpha(0.5 * beat));
            for i in 0..6u32 {
                let a = scatter(who as u32 ^ 0x5157, i) * TAU + left * 2.5;
                let rise = (left * 1.7 + scatter(who as u32, i + 9)).fract();
                let p = at + vec2(a.cos(), a.sin()) * (20.0 + 18.0 * rise);
                self.list.circle(
                    p,
                    4.0 * (1.0 - rise),
                    RUSH.glowing(1.5).alpha(0.8 * (1.0 - rise)),
                );
            }
        }
        // A lifted bot: a thin green ring at its feet and two pips on it.
        for &who in &self.item_looks.uplinked.clone() {
            let Some(bim) = self.bims.get(who) else {
                continue;
            };
            let at = bim.character.drawn_at();
            self.list.ring(at, 46.0, 1.5, UPLINK.alpha(0.55));
            for k in [0.0, std::f32::consts::PI] {
                let a = bim.character.heading + k;
                self.list
                    .circle(at + vec2(a.cos(), a.sin()) * 23.0, 4.0, UPLINK.alpha(0.85));
            }
        }
        for &(carrier, mate) in &self.item_looks.tethers {
            let (Some(a), Some(b)) = (self.bims.get(carrier), self.bims.get(mate)) else {
                continue;
            };
            let (a, b) = (a.character.drawn_at(), b.character.drawn_at());
            self.list.line(a, b, 6.0, TETHER.alpha(0.18));
            self.list.line(a, b, 2.0, TETHER.glowing(1.4).alpha(0.85));
            self.list.ring(b, 34.0, 2.0, TETHER.alpha(0.6));
        }
        for ghost in self.item_looks.ghosts.clone() {
            let Some(bim) = self.bims.get(ghost.owner) else {
                continue;
            };
            let figure = bim
                .character
                .ghost(ghost.at, ghost.heading, ghost.stride, ghost.speed);
            let from = self.list.len();
            figure.draw(&mut self.list, self.viewer);
            self.list.fade_from(from, GHOST_OPACITY * ghost.fade);
            // A faint ring under it, the taunt's reach said small.
            self.list
                .ring(ghost.at, 56.0, 2.0, TETHER.alpha(0.35 * ghost.fade));
        }
    }

    /// One cloud as particles: a soft body of a few big puffs breathing,
    /// a churn of smaller ones drifting out from the middle and fading as
    /// they go, born again where they began — so the cloud rolls — and,
    /// the moment the canister goes off, a ring of puffs thrown out to
    /// its edge. Going out (its `thick` under one) the particles thin
    /// and spread. Every place and phase is a hash of the cloud, its
    /// index and its age: nothing drawn off any stream.
    fn draw_smoke(&mut self, cloud: &SmokeCloud, n: u32) {
        let seed = (cloud.at.x as i32 as u32).wrapping_mul(31) ^ (cloud.at.y as i32 as u32) ^ n;
        let r = cloud.radius;
        let age = cloud.age;
        let thick = cloud.thick.clamp(0.0, 1.0);
        // Spreading out from the canister over the pop.
        let grow = {
            let t = (age / POP_SECONDS).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        let spread = (0.35 + 0.65 * grow) * (1.0 + 0.35 * (1.0 - thick));
        for i in 0..BODY_PUFFS {
            let a = scatter(seed, i + 200) * TAU + age * 0.05;
            let off = vec2(a.cos(), a.sin()) * r * 0.35 * scatter(seed, i + 220) * spread;
            let size = r * (1.0 + 0.25 * scatter(seed, i + 240)) * spread;
            let swell = 1.0 + 0.05 * (age * 0.7 + i as f32 * 1.3).sin();
            self.list.circle(
                cloud.at + off,
                size * swell,
                SMOKE_DARK.alpha(0.40 * thick * grow),
            );
        }
        for i in 0..PARTICLES {
            // Going out, a particle at a time drops away.
            if scatter(seed, i + 400) > thick + 0.2 {
                continue;
            }
            let life = 2.0 + 1.4 * scatter(seed, i + 300);
            let p = (age / life + scatter(seed, i + 320)).fract();
            let turn = if scatter(seed, i + 340) < 0.5 {
                -1.0
            } else {
                1.0
            };
            let a = scatter(seed, i) * TAU + turn * 0.7 * p;
            let from = r * 0.45 * scatter(seed, i + 360).sqrt();
            let far = (from + r * 0.55 * p) * spread;
            let at = cloud.at + vec2(a.cos(), a.sin()) * far;
            let size = r * (0.50 + 0.65 * p) * (0.5 + 0.5 * grow);
            let fade = (p * std::f32::consts::PI).sin();
            let shade = scatter(seed, i + 380);
            let colour = if shade < 0.3 {
                SMOKE_DARK
            } else if shade < 0.8 {
                SMOKE
            } else {
                SMOKE_PALE
            };
            self.list
                .circle(at, size, colour.alpha(0.55 * fade * thick * grow.max(0.3)));
        }
        // The pop: puffs thrown to the edge and a flash at the canister.
        if age < POP_SECONDS {
            let t = age / POP_SECONDS;
            for i in 0..POP_PUFFS {
                let a = (i as f32 + scatter(seed, i + 500)) / POP_PUFFS as f32 * TAU;
                let at = cloud.at + vec2(a.cos(), a.sin()) * r * 1.05 * t.sqrt();
                self.list.circle(
                    at,
                    r * (0.40 + 0.45 * t),
                    SMOKE_PALE.alpha(0.65 * (1.0 - t)),
                );
            }
            if age < 0.15 {
                let f = 1.0 - age / 0.15;
                self.list.circle(
                    cloud.at,
                    r * 0.45 * (1.0 + age * 4.0),
                    SMOKE_PALE.glowing(1.6).alpha(0.7 * f),
                );
            }
        }
    }
}
