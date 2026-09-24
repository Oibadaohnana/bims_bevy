//! The stars behind the ship.
//!
//! **Cosmetic, and nothing else.** Not the galaxy's stars, not the system's,
//! not anything a player can fly to or point at: three layers of specks
//! behind the ship, so that it does not sit in a black rectangle. Nothing
//! in the simulation has ever heard of them. They streamed past a ship
//! under way until nothing was flown any more (feature 104); a trip is
//! resolved rather than flown, and the sky stands still.
//!
//! **It tiles.** Each layer is a square tile of stars repeated for ever, so
//! the field never runs out however far the view is panned.

use worldgen::math::{DVec2, dvec2};
use worldgen::rng::Rng;

/// One speck.
///
/// Its position is in **screen pixels**, not world units. A backdrop is a
/// backdrop: it covers the canvas whatever the zoom, and a field measured in
/// world units would tile four hundred times over at the far end of the zoom
/// range and once at the near end.
#[derive(Clone, Copy)]
pub struct Speck {
    /// Where it is inside its layer's tile.
    pub at: DVec2,
    pub size: f32,
    pub brightness: f32,
}

/// How many layers, and how many specks each has. Three, because two do not
/// read as depth and four is a lot of rectangles a frame for something
/// nobody is looking at.
const LAYER_SPECKS: [u32; 3] = [90, 130, 170];

/// The side of the square each layer tiles, in screen pixels. Two or three
/// repeats cover an ordinary window, which is few enough that the pattern is
/// not obvious and few enough to draw.
pub const FIELD: f64 = 700.0;

pub struct Starfield {
    pub layers: Vec<Vec<Speck>>,
}

impl Starfield {
    /// Built from the world's own seed, so the same galaxy has the same sky.
    ///
    /// Its own branch of the seed rather than a stream with a `Purpose`:
    /// `worldgen::rng::Purpose` is the list of things the **world** is
    /// generated from, and a decoration has no business being on it.
    pub fn new(seed: u64) -> Starfield {
        let mut rng = Rng::new(seed ^ 0x_5354_4152_4649_454c);
        let layers = LAYER_SPECKS
            .iter()
            .map(|&count| {
                (0..count)
                    .map(|_| Speck {
                        at: dvec2(rng.range(0.0, FIELD), rng.range(0.0, FIELD)),
                        // The near layer is not the bright one: a bright speck
                        // up front reads as a scratch on the screen.
                        size: rng.range(0.8, 2.2) as f32,
                        brightness: rng.range(0.18, 0.75) as f32,
                    })
                    .collect()
            })
            .collect();
        Starfield { layers }
    }
}
