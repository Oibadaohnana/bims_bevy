//! Blood on the deck: where a body bled, tile by tile, and how that is
//! drawn.
//!
//! The deck is scored tile by tile. A tile starts at [`BASELINE`] and goes
//! down as blood lands on it, never past [`FOULED`]. A bleeding body drips
//! (`Bim::tick_drips`), a cut or a burst throws it over the tiles round the
//! body ([`Blood::splash`]), and boots carry what is down from one tile to
//! the next ([`Blood::track`]): a quarter of the crossings move a quarter of
//! the tile, so a trail thins fast and dies out rather than working its way
//! across the ship. Nothing takes it up again.
//!
//! All three draw on the room's own stream, which every fight draws from:
//! a drop's scatter, a crossing's roll and a splash's picks are what they
//! always were, in the same order and decided by the same scores, since a
//! draw taken away or moved would re-roll every fight after the first drop.

use crate::draw::{Color, DrawList};
use crate::math::{Rect, Vec2, vec2};
use crate::rng::Rng;

/// How big one tile of deck is. The same grid the floor is drawn on, so a
/// stain lines up with the seams under it rather than floating between them.
pub const TILE: f32 = 52.0;

/// What a clean tile scores, and the floor under the worst of it.
pub const BASELINE: f32 = 10.0;
pub const FOULED: f32 = -100.0;

/// What one drop of blood takes off the tile it lands on: a body standing
/// still with a wound open takes the tile under it to the bottom of the
/// scale in a couple of drops, which is what a pool of blood is.
pub const BLOOD_COST: f32 = 60.0;
/// How many tiles a cut splashes blood over, the one under the body
/// included: three to five.
pub const SPLASH_TILES: (u32, u32) = (3, 5);

/// How far below clean a tile has to be before a boot carries any of it
/// on.
const WORTH_CARRYING: f32 = 1.0;

/// Walking it about. A boot crossing a bloody tile has [`SPREAD_CHANCE`]
/// of taking [`SPREAD_SHARE`] of what is on it onto the tile it steps to.
///
/// The blood *moves*: the tile behind loses exactly what the tile ahead
/// gains, so a trail thins as it lengthens — a quarter, then a sixteenth —
/// and falls under [`WORTH_CARRYING`] on its own after three or four steps.
const SPREAD_CHANCE: f32 = 0.25;
const SPREAD_SHARE: f32 = 0.25;

/// A dark red for the wash and the blobs alike, so a pool beside a body
/// reads as what it is.
const BLOOD: Color = Color::rgb(0.45, 0.04, 0.05);
/// The alpha the worst tile reaches. Short of opaque: the deck seams
/// should still show through, or the tile reads as a hole in the floor.
const STAIN_ALPHA: f32 = 0.72;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Blood {
    cols: usize,
    rows: usize,
    origin: Vec2,
    /// One score per tile, [`FOULED`] to [`BASELINE`].
    tiles: Vec<f32>,
}

impl Blood {
    pub fn new(interior: Rect) -> Blood {
        let cols = (interior.width() / TILE).ceil() as usize + 1;
        let rows = (interior.height() / TILE).ceil() as usize + 1;
        Blood {
            cols,
            rows,
            origin: interior.min,
            tiles: vec![BASELINE; cols * rows],
        }
    }

    /// The same deck over a different interior — one that grew a tile when
    /// deck was laid, or shrank — with every stain kept where it lies. Tiles
    /// carry across by position, so a pool by the door stays by the door;
    /// what the new interior does not reach is gone, and what it reaches
    /// afresh is clean.
    pub fn resized(&self, interior: Rect) -> Blood {
        let mut next = Blood::new(interior);
        for r in 0..next.rows {
            for c in 0..next.cols {
                let at = next.centre(c as i32, r as i32);
                let (oc, or) = self.cell(at);
                if let (Some(from), Some(to)) = (self.index(oc, or), next.index(c as i32, r as i32))
                {
                    next.tiles[to] = self.tiles[from];
                }
            }
        }
        next
    }

    fn cell(&self, at: Vec2) -> (i32, i32) {
        (
            ((at.x - self.origin.x) / TILE).floor() as i32,
            ((at.y - self.origin.y) / TILE).floor() as i32,
        )
    }

    fn index(&self, c: i32, r: i32) -> Option<usize> {
        if c < 0 || r < 0 || c as usize >= self.cols || r as usize >= self.rows {
            return None;
        }
        Some(r as usize * self.cols + c as usize)
    }

    /// The middle of tile `(c, r)`.
    fn centre(&self, c: i32, r: i32) -> Vec2 {
        self.origin + vec2((c as f32 + 0.5) * TILE, (r as f32 + 0.5) * TILE)
    }

    /// What the tile under `at` scores. For the probes and the tests.
    #[allow(dead_code)]
    pub fn at(&self, at: Vec2) -> f32 {
        let (c, r) = self.cell(at);
        self.index(c, r).map_or(BASELINE, |i| self.tiles[i])
    }

    /// How many tiles have blood on them worth carrying off on a boot.
    pub fn stained_tiles(&self) -> u32 {
        self.tiles
            .iter()
            .filter(|&&t| t < BASELINE - WORTH_CARRYING)
            .count() as u32
    }

    /// A drop's worth of blood on the tile under `at`, never past the worst
    /// there is.
    pub fn drop_at(&mut self, at: Vec2) {
        let (c, r) = self.cell(at);
        if let Some(i) = self.index(c, r) {
            self.tiles[i] = (self.tiles[i] - BLOOD_COST).max(FOULED);
        }
    }

    /// Walk blood from the tile a Bim was on onto the tile it has stepped
    /// to.
    ///
    /// Handed where a body was at the top of the frame and where it is now.
    /// Nothing happens at all — and nothing is drawn from `rng` — unless that
    /// step crossed a tile boundary *and* the tile behind had blood on it
    /// worth carrying.
    pub fn track(&mut self, from: Vec2, to: Vec2, rng: &mut Rng) -> bool {
        let (fc, fr) = self.cell(from);
        let (tc, tr) = self.cell(to);
        if (fc, fr) == (tc, tr) {
            return false;
        }
        let (Some(behind), Some(ahead)) = (self.index(fc, fr), self.index(tc, tr)) else {
            return false;
        };
        let dirt = BASELINE - self.tiles[behind];
        if dirt < WORTH_CARRYING || !rng.chance(SPREAD_CHANCE) {
            return false;
        }
        let moved = dirt * SPREAD_SHARE;
        self.tiles[behind] += moved;
        self.tiles[ahead] = (self.tiles[ahead] - moved).max(FOULED);
        true
    }

    /// A cut opens: blood over [`SPLASH_TILES`] of the nine round `at`, the
    /// tile under the body always among them, a drop's worth each. A shot
    /// wound only drips (`Bim::tick_drips`); a blade or a burst throws it
    /// about. `can_get_to` keeps it off the deck no body can reach — under
    /// the lip of a counter, the corner past a bunk. The rolls are the
    /// caller's stream.
    pub fn splash(&mut self, at: Vec2, rng: &mut Rng, can_get_to: impl Fn(Vec2) -> bool) {
        self.drop_at(at);
        let (c0, r0) = self.cell(at);
        let mut choices = [vec2(0.0, 0.0); 8];
        let mut found = 0;
        for dr in -1..=1 {
            for dc in -1..=1 {
                if (dc, dr) == (0, 0) || self.index(c0 + dc, r0 + dr).is_none() {
                    continue;
                }
                let tile = self.centre(c0 + dc, r0 + dr);
                if !can_get_to(tile) {
                    continue;
                }
                choices[found] = tile;
                found += 1;
            }
        }
        let (lo, hi) = SPLASH_TILES;
        let mut want = (lo + rng.below(hi - lo + 1)).saturating_sub(1) as usize;
        // Each of the rest picked out of what is left, so no tile is hit
        // twice and the count is the count.
        while want > 0 && found > 0 {
            let i = rng.below(found as u32) as usize;
            self.drop_at(choices[i]);
            choices[i] = choices[found - 1];
            found -= 1;
            want -= 1;
        }
    }

    // --- drawing ----------------------------------------------------------

    /// Under everything: the blood is on the deck, so the Bim walks over it.
    pub fn draw(&self, list: &mut DrawList) {
        for r in 0..self.rows as i32 {
            for c in 0..self.cols as i32 {
                let Some(i) = self.index(c, r) else { continue };
                let score = self.tiles[i];
                if score >= BASELINE {
                    continue;
                }
                // Nothing to full, from the baseline down to the worst — and
                // then square-rooted, so the faint end of the range is still
                // a shade somebody can see against the deck.
                let deep = ((BASELINE - score) / (BASELINE - FOULED))
                    .clamp(0.0, 1.0)
                    .sqrt();
                let at = self.centre(c, r);
                list.rect(
                    at,
                    vec2(TILE, TILE),
                    0.0,
                    6.0,
                    BLOOD.alpha(STAIN_ALPHA * deep * 0.55),
                );
                // A few blobs on top, placed off the tile's own coordinates so
                // they stay put between frames without anything being stored.
                let blobs = (deep * 5.0).ceil() as i32;
                for b in 0..blobs {
                    let h = (c * 73 + r * 149 + b * 31) as f32;
                    let off = vec2((h * 0.37).sin(), (h * 0.71).cos()) * (TILE * 0.30);
                    let size = TILE * (0.12 + 0.10 * (h * 0.53).sin().abs());
                    list.ellipse(
                        at + off,
                        vec2(size, size * 0.8),
                        h,
                        BLOOD.alpha(STAIN_ALPHA * deep),
                    );
                }
            }
        }
    }
}
