//! The floor (October 2026): the run's map, laid out the way Slay the
//! Spire lays one — the crew start at the bottom and climb, a row a hop,
//! to the Machine Heart at the top, and never go down.
//!
//! - **[`data::FLOOR_HOPS`] rows**, the start (row nought, the crew's own
//!   station) below them and the Heart (the machines' origin) the last.
//!   Row `r` is fought on run day `r` ([`row_day`]; the start and the
//!   first row share day one), so how far up the crew are is the day.
//! - **Two to four ways up** ([`data::FLOOR_MIN_WAYS`],
//!   [`data::FLOOR_MAX_WAYS`]): every row between has that many places,
//!   and the trips between two rows never cross — a lattice path from the
//!   leftmost pair to the rightmost, every place on it — so the leftmost
//!   places and the rightmost are two ways up that share nothing, and no
//!   more than four can.
//! - **Four rows of traders**, one rolled in each of
//!   [`data::FLOOR_SHOP_BANDS`]: every place on such a row is a trader, so
//!   every way up meets four.
//! - **A place is a star**, its system's one site (`World::mission_site`):
//!   the galaxy's systems, their missions and their trips are what the
//!   floor is made of; the galaxy chart is gone. The stars are picked
//!   nearer the origin the higher the row, so the climb walks into the
//!   crisis (`World::lay_floor`).
//!
//! Stateless, like the crisis: the shape is a function of the galaxy's
//! seed and the crew's own star, the stars a function of the galaxy, so
//! nothing of it is saved — two clients lay the same floor.

use worldgen::rng::{Rng, mix};

use crate::data;

/// One place on the floor.
#[derive(Clone, PartialEq, Debug)]
pub struct FloorNode {
    /// The star whose system it is.
    pub star: u32,
    /// The site gone to there: the system's one (`World::mission_site`),
    /// the crew's own station at the start, the Heart's fortress at the
    /// top.
    pub station: u32,
    /// Across the floor, nought at the left and one at the right: where
    /// the map draws it, and nothing else.
    pub x: f32,
    /// The places of the next row a trip from here goes to, by index,
    /// left to right.
    pub up: Vec<u8>,
}

/// A floor: its rows, the start first and the Heart last.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Floor {
    pub rows: Vec<Vec<FloorNode>>,
    /// The rows whose every place is a trader, in order.
    pub shop_rows: Vec<u32>,
}

impl Floor {
    /// The Heart's row: the last.
    pub fn heart_row(&self) -> u32 {
        (self.rows.len() as u32).saturating_sub(1)
    }

    /// The place at `row`, `index`.
    pub fn node(&self, row: u32, index: usize) -> Option<&FloorNode> {
        self.rows.get(row as usize)?.get(index)
    }

    /// Where a star is on the floor, `(row, index)`: the lowest row it is
    /// on.
    pub fn find(&self, star: u32) -> Option<(u32, usize)> {
        self.rows.iter().enumerate().find_map(|(row, nodes)| {
            nodes
                .iter()
                .position(|n| n.star == star)
                .map(|i| (row as u32, i))
        })
    }

    /// The places a trip from `(row, index)` goes to, on the row above.
    pub fn next(&self, row: u32, index: usize) -> Vec<(u32, usize)> {
        self.node(row, index)
            .map(|n| n.up.iter().map(|&j| (row + 1, j as usize)).collect())
            .unwrap_or_default()
    }

    /// Whether every place on `row` is a trader.
    pub fn is_shop_row(&self, row: u32) -> bool {
        self.shop_rows.contains(&row)
    }

    /// How many places there are on the whole floor.
    pub fn len(&self) -> usize {
        self.rows.iter().map(Vec::len).sum()
    }

    /// Whether it has no place at all.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// The run day a row is fought on: its number, the start's being the
/// first row's, day one.
pub fn row_day(row: u32) -> u32 {
    row.max(1)
}

/// A floor's shape before any star is put on it: how many places each
/// row has, where each is drawn, which go to which, and the traders' rows.
#[derive(Clone, PartialEq, Debug)]
pub struct Shape {
    pub widths: Vec<u32>,
    pub xs: Vec<Vec<f32>>,
    pub up: Vec<Vec<Vec<u8>>>,
    pub shop_rows: Vec<u32>,
}

/// The stream a floor's shape is rolled off: the galaxy's seed and the
/// crew's own star, a salt of its own.
pub fn seed(galaxy_seed: u64, home_star: u32) -> u64 {
    mix(galaxy_seed ^ 0x_464C_4F4F_5253) ^ mix(u64::from(home_star))
}

/// The shape of a floor off `seed`: [`data::FLOOR_HOPS`] rows above the
/// start, two to four places a row between, the trips between two rows a
/// lattice path that never crosses itself.
pub fn shape(seed: u64) -> Shape {
    let hops = data::FLOOR_HOPS.max(2);
    let (least, most) = (
        data::FLOOR_MIN_WAYS.max(1),
        data::FLOOR_MAX_WAYS.max(data::FLOOR_MIN_WAYS.max(1)),
    );
    let mut rng = Rng::new(seed);
    // How many places each row has: the start and the Heart one, the
    // rows between a walk that steps a place at a time.
    let mut widths = vec![1u32; hops as usize + 1];
    let mut width = least + rng.below(most - least + 1);
    for w in widths.iter_mut().take(hops as usize).skip(1) {
        *w = width;
        width = match rng.below(10) {
            0..=1 => width.saturating_sub(1).max(least),
            2..=3 => (width + 1).min(most),
            _ => width,
        };
    }
    // Where each is drawn: evenly across, a little off true.
    let xs: Vec<Vec<f32>> = widths
        .iter()
        .map(|&w| {
            (0..w)
                .map(|i| {
                    if w == 1 {
                        return 0.5;
                    }
                    let off = (rng.below(1_000) as f32 / 1_000.0 - 0.5) * 0.35;
                    (i as f32 + 0.5 + off) / w as f32
                })
                .collect()
        })
        .collect();
    // Which go to which: a lattice path from the leftmost pair to the
    // rightmost, keeping level where it can and wandering now and then,
    // so every place has a way in and a way on and no two trips cross.
    let mut up: Vec<Vec<Vec<u8>>> = Vec::with_capacity(widths.len());
    for row in 0..widths.len() {
        let a = widths[row];
        let Some(&b) = widths.get(row + 1) else {
            up.push(vec![Vec::new(); a as usize]);
            break;
        };
        let mut edges = vec![Vec::new(); a as usize];
        let (mut i, mut j) = (0u32, 0u32);
        edges[0].push(0);
        while i + 1 < a || j + 1 < b {
            let step = if i + 1 == a {
                1
            } else if j + 1 == b {
                0
            } else if rng.below(10) < 3 {
                rng.below(3)
            } else {
                // Level: the one behind its share moves.
                let (on_i, on_j) = ((i + 1) * b, (j + 1) * a);
                match on_i.cmp(&on_j) {
                    std::cmp::Ordering::Less => 0,
                    std::cmp::Ordering::Greater => 1,
                    std::cmp::Ordering::Equal => 2,
                }
            };
            match step {
                0 => i += 1,
                1 => j += 1,
                _ => {
                    i += 1;
                    j += 1;
                }
            }
            edges[i as usize].push(j as u8);
        }
        up.push(edges);
    }
    // The traders' rows: one in each band, inside the floor.
    let shop_rows = data::FLOOR_SHOP_BANDS
        .iter()
        .map(|&(lo, hi)| {
            let (lo, hi) = (lo.clamp(1, hops - 1), hi.clamp(1, hops - 1));
            lo + rng.below(hi.saturating_sub(lo) + 1)
        })
        .collect();
    Shape {
        widths,
        xs,
        up,
        shop_rows,
    }
}

/// The floor of `shape`, a star and a site put on every place by
/// `place(row, index, shop)` — `shop` for a place on a traders' row — in
/// row order and left to right.
pub fn lay(shape: Shape, mut place: impl FnMut(u32, usize, bool) -> (u32, u32)) -> Floor {
    let rows = shape
        .widths
        .iter()
        .enumerate()
        .map(|(row, &w)| {
            let shop = shape.shop_rows.contains(&(row as u32));
            (0..w as usize)
                .map(|i| {
                    let (star, station) = place(row as u32, i, shop);
                    FloorNode {
                        star,
                        station,
                        x: shape.xs[row][i],
                        up: shape.up[row][i].clone(),
                    }
                })
                .collect()
        })
        .collect();
    Floor {
        rows,
        shop_rows: shape.shop_rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every place on a row reached from the row below, and every place
    /// below the Heart going somewhere.
    fn wired(shape: &Shape) -> bool {
        (0..shape.widths.len() - 1).all(|row| {
            let reached: Vec<u8> = shape.up[row].iter().flatten().copied().collect();
            (0..shape.widths[row + 1] as u8).all(|j| reached.contains(&j))
                && shape.up[row].iter().all(|e| !e.is_empty())
        })
    }

    #[test]
    fn a_floor_is_fifty_rows_two_to_four_wide_and_wired_without_a_crossing() {
        for seed in 0..200u64 {
            let shape = shape(mix(seed));
            assert_eq!(shape.widths.len() as u32, data::FLOOR_HOPS + 1);
            assert_eq!(shape.widths[0], 1, "one start");
            assert_eq!(*shape.widths.last().unwrap(), 1, "one Heart");
            for &w in &shape.widths[1..shape.widths.len() - 1] {
                assert!((data::FLOOR_MIN_WAYS..=data::FLOOR_MAX_WAYS).contains(&w));
            }
            assert!(wired(&shape), "seed {seed}");
            // No two trips between a pair of rows cross: going left to
            // right, where they land never goes back.
            for row in 0..shape.widths.len() - 1 {
                let mut last = 0u8;
                for (i, edges) in shape.up[row].iter().enumerate() {
                    for w in edges.windows(2) {
                        assert!(w[0] < w[1]);
                    }
                    if i > 0 {
                        assert!(edges[0] >= last, "seed {seed} row {row} crosses");
                    }
                    last = *edges.last().unwrap();
                }
            }
            // The leftmost and the rightmost are two ways that share no
            // place between the start and the Heart.
            for row in 1..shape.widths.len() - 2 {
                assert!(shape.up[row][0].contains(&0));
                let (a, b) = (shape.widths[row], shape.widths[row + 1]);
                assert!(shape.up[row][a as usize - 1].contains(&(b as u8 - 1)));
            }
            assert_eq!(shape.shop_rows.len(), data::FLOOR_SHOP_BANDS.len());
            for (&row, &(lo, hi)) in shape.shop_rows.iter().zip(&data::FLOOR_SHOP_BANDS) {
                assert!((lo..=hi).contains(&row));
            }
        }
    }

    #[test]
    fn the_same_seed_is_the_same_floor() {
        assert_eq!(shape(seed(7, 3)), shape(seed(7, 3)));
        assert_ne!(shape(seed(7, 3)), shape(seed(8, 3)));
    }

    #[test]
    fn the_rows_are_the_days() {
        assert_eq!(row_day(0), 1);
        assert_eq!(row_day(1), 1);
        assert_eq!(row_day(37), 37);
        assert_eq!(row_day(data::FLOOR_HOPS), data::FLOOR_HOPS);
    }
}
