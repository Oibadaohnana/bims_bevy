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
}

/// How solid a ghost is drawn at the most.
pub const GHOST_OPACITY: f32 = 0.42;
/// The smoke's grey.
const SMOKE: Color = Color::rgb(0.58, 0.60, 0.62);
/// The smoke's darker underside.
const SMOKE_DARK: Color = Color::rgb(0.36, 0.38, 0.40);
/// How many puffs a cloud is drawn as.
const PUFFS: u32 = 14;
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
            for (n, cloud) in self.item_looks.smoke.iter().enumerate() {
                let seed = (cloud.at.x as i32 as u32).wrapping_mul(31)
                    ^ (cloud.at.y as i32 as u32)
                    ^ n as u32;
                for i in 0..PUFFS {
                    let a =
                        scatter(seed, i) * TAU + cloud.age * 0.12 * (scatter(seed, i + 40) - 0.5);
                    let far = scatter(seed, i + 80).sqrt() * cloud.radius * 0.72;
                    let drift = vec2(a.cos(), a.sin()) * far;
                    let size = cloud.radius * (0.55 + 0.35 * scatter(seed, i + 120));
                    let swell = 1.0 + 0.06 * (cloud.age * 0.8 + i as f32).sin();
                    let alpha = 0.62 * cloud.thick;
                    self.list.circle(
                        cloud.at + drift + vec2(0.0, size * 0.10),
                        size * swell,
                        SMOKE_DARK.alpha(alpha * 0.8),
                    );
                    self.list
                        .circle(cloud.at + drift, size * swell * 0.92, SMOKE.alpha(alpha));
                }
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
}
