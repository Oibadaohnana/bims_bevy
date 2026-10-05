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
//! - **About [`data::FLOOR_SHOPS`] traders**, scattered: one a stretch
//!   of rows of the climb (from [`data::FLOOR_FIRST_SHOP_ROW`]), each on
//!   the place of its row that reaches the place longest stranded without
//!   one, so they spread across the ways and never clump: a trader within
//!   seven rows of nearly every place, a way up that goes for them meets
//!   one every six or seven rows, and going there is a choice of way, as
//!   in Slay the Spire.
//! - **A trader under the Heart**, always (October 2026, the player's):
//!   the row below the Heart is one place, every way up's last stop, and
//!   a trader — what a crew earned on the last rows is spent before the
//!   Heart, never carried into it. One of the [`data::FLOOR_SHOPS`].
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
    /// A trader's place.
    pub shop: bool,
}

/// A floor: its rows, the start first and the Heart last.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Floor {
    pub rows: Vec<Vec<FloorNode>>,
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

    /// Whether the place at `row`, `index` is a trader.
    pub fn is_shop(&self, row: u32, index: usize) -> bool {
        self.node(row, index).is_some_and(|n| n.shop)
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
/// row has, where each is drawn, which go to which, and which are
/// traders.
#[derive(Clone, PartialEq, Debug)]
pub struct Shape {
    pub widths: Vec<u32>,
    pub xs: Vec<Vec<f32>>,
    pub up: Vec<Vec<Vec<u8>>>,
    pub shops: Vec<Vec<bool>>,
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
    // The row under the Heart one place, every way up meeting there: the
    // last trader before the Heart (`scatter_shops`).
    widths[hops as usize - 1] = 1;
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
    let shops = scatter_shops(&widths, &up, &mut rng);
    Shape {
        widths,
        xs,
        up,
        shops,
    }
}

/// The traders, off `rng`: **the row under the Heart** — its one place,
/// every way up's last stop, so a crew can always spend before the Heart
/// (October 2026, the player's) — and the rest scattered: the rows from
/// [`data::FLOOR_FIRST_SHOP_ROW`] to the one three under the Heart cut
/// into [`data::FLOOR_SHOPS`] less one stretches as even as whole rows go, a
/// trader on a row rolled in each — never on the row just after the last
/// one's, where the stretch has another — on the place of that row a
/// place stranded longest below it leads to. So a way up the last
/// traders missed is the one the next is put on, and none clump.
fn scatter_shops(widths: &[u32], up: &[Vec<Vec<u8>>], rng: &mut Rng) -> Vec<Vec<bool>> {
    let mut shops: Vec<Vec<bool>> = widths.iter().map(|&w| vec![false; w as usize]).collect();
    let top = widths.len() as u32 - 1;
    if top >= 2 {
        shops[top as usize - 1][0] = true;
    }
    // The scattered ones end three rows under the Heart, so none is on
    // the row just before the last.
    let last_row = top.saturating_sub(2);
    let first = data::FLOOR_FIRST_SHOP_ROW.clamp(1, last_row.max(1));
    let span = last_row.saturating_sub(first);
    let count = data::FLOOR_SHOPS.saturating_sub(1).min(span);
    if count == 0 {
        return shops;
    }
    // The shop rows, one a stretch, in order.
    let mut rows = Vec::with_capacity(count as usize);
    let mut last: Option<u32> = None;
    for k in 0..count {
        let (lo, hi) = (first + k * span / count, first + (k + 1) * span / count - 1);
        let lo = match last {
            Some(prev) if prev + 1 == lo && lo < hi => lo + 1,
            _ => lo,
        };
        let row = lo + rng.below(hi - lo + 1);
        rows.push(row);
        last = Some(row);
    }
    // Each on the place that rescues the place longest stranded: of the
    // places below that no trader put so far can be reached from, the
    // lowest that can reach it — then the most such, then a roll.
    for row in rows {
        let row = row as usize;
        // Which places below reach a trader already.
        let mut covered: Vec<Vec<bool>> = shops[..row].to_vec();
        for r in (0..row.saturating_sub(1)).rev() {
            for i in 0..covered[r].len() {
                covered[r][i] =
                    covered[r][i] || up[r][i].iter().any(|&j| covered[r + 1][j as usize]);
            }
        }
        let mut best: Option<((u32, u32, u32), usize)> = None;
        for j in 0..widths[row] as usize {
            // The places below that reach this one.
            let mut reach = vec![false; widths[row] as usize];
            reach[j] = true;
            let (mut lowest, mut stranded) = (row as u32, 0u32);
            for r in (0..row).rev() {
                let below: Vec<bool> = up[r]
                    .iter()
                    .map(|edges| edges.iter().any(|&k| reach[k as usize]))
                    .collect();
                for (i, &on) in below.iter().enumerate() {
                    if on && !covered[r][i] {
                        lowest = r as u32;
                        stranded += 1;
                    }
                }
                reach = below;
            }
            let score = (row as u32 - lowest, stranded, rng.below(1_000));
            if best.is_none_or(|(b, _)| score > b) {
                best = Some((score, j));
            }
        }
        if let Some((_, pick)) = best {
            shops[row][pick] = true;
        }
    }
    shops
}

/// The floor of `shape`, a star and a site put on every place by
/// `place(row, index, shop)` — `shop` for a trader's place — in
/// row order and left to right.
pub fn lay(shape: Shape, mut place: impl FnMut(u32, usize, bool) -> (u32, u32)) -> Floor {
    let rows = shape
        .widths
        .iter()
        .enumerate()
        .map(|(row, &w)| {
            (0..w as usize)
                .map(|i| {
                    let shop = shape.shops[row][i];
                    let (star, station) = place(row as u32, i, shop);
                    FloorNode {
                        star,
                        station,
                        x: shape.xs[row][i],
                        up: shape.up[row][i].clone(),
                        shop,
                    }
                })
                .collect()
        })
        .collect();
    Floor { rows }
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
    fn a_floor_is_thirty_two_rows_two_to_four_wide_and_wired_without_a_crossing() {
        for seed in 0..200u64 {
            let shape = shape(mix(seed));
            assert_eq!(shape.widths.len() as u32, data::FLOOR_HOPS + 1);
            assert_eq!(shape.widths[0], 1, "one start");
            assert_eq!(*shape.widths.last().unwrap(), 1, "one Heart");
            for &w in &shape.widths[1..shape.widths.len() - 2] {
                assert!((data::FLOOR_MIN_WAYS..=data::FLOOR_MAX_WAYS).contains(&w));
            }
            assert_eq!(shape.widths[shape.widths.len() - 2], 1, "one last trader");
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
            // place between the start and the last trader.
            for row in 1..shape.widths.len() - 2 {
                assert!(shape.up[row][0].contains(&0));
                let (a, b) = (shape.widths[row], shape.widths[row + 1]);
                assert!(shape.up[row][a as usize - 1].contains(&(b as u8 - 1)));
            }
        }
    }

    /// How many rows on from `(row, i)` the nearest trader a trip up can
    /// reach is (nought on one), `None` for none before the Heart.
    fn rows_to_a_shop(shape: &Shape) -> Vec<Vec<Option<u32>>> {
        let mut ahead: Vec<Vec<Option<u32>>> = shape
            .widths
            .iter()
            .map(|&w| vec![None; w as usize])
            .collect();
        for row in (0..shape.widths.len()).rev() {
            for i in 0..shape.widths[row] as usize {
                ahead[row][i] = if shape.shops[row][i] {
                    Some(0)
                } else {
                    shape.up[row][i]
                        .iter()
                        .filter_map(|&j| ahead[row + 1][j as usize])
                        .min()
                        .map(|d| d + 1)
                };
            }
        }
        ahead
    }

    /// The longest run of rows without a trader on the way up that keeps
    /// it shortest: what a crew going out of its way for them meets.
    fn best_way_s_dry_stretch(shape: &Shape) -> u32 {
        let top = shape.widths.len() - 1;
        // Whether a way up has no run of `n` rows without a trader: the
        // shortest run each place can be reached on, under `n`.
        let under = |n: u32| {
            let mut dry: Vec<Option<u32>> = vec![Some(0)];
            for row in 1..top {
                let mut next = vec![None; shape.widths[row] as usize];
                for (i, d) in dry.iter().enumerate() {
                    let Some(d) = *d else { continue };
                    for &j in &shape.up[row - 1][i] {
                        let d = if shape.shops[row][j as usize] {
                            0
                        } else {
                            d + 1
                        };
                        if d < n {
                            let slot = &mut next[j as usize];
                            *slot = Some(slot.map_or(d, |s: u32| s.min(d)));
                        }
                    }
                }
                dry = next;
            }
            dry.iter().any(Option::is_some)
        };
        (1..=top as u32)
            .find(|&n| under(n + 1))
            .unwrap_or(top as u32)
    }

    /// The most traders one way up can meet.
    fn most_met(shape: &Shape) -> u32 {
        let mut most: Vec<Vec<u32>> = shape.widths.iter().map(|&w| vec![0; w as usize]).collect();
        for row in (0..shape.widths.len() - 1).rev() {
            for i in 0..shape.widths[row] as usize {
                let on = shape.up[row][i].iter().map(|&j| most[row + 1][j as usize]);
                most[row][i] = on.max().unwrap_or(0) + u32::from(shape.shops[row][i]);
            }
        }
        most[0][0]
    }

    #[test]
    fn the_traders_are_scattered_one_a_stretch_and_within_reach() {
        let seeds = 500u64;
        let (mut near, mut places, mut met, mut dry_sum, mut driest) = (0u32, 0u32, 0u32, 0u32, 0);
        for seed in 0..seeds {
            let shape = shape(mix(seed));
            let top = shape.widths.len() - 1;
            let rows: Vec<usize> = (0..=top)
                .filter(|&r| shape.shops[r].iter().any(|&s| s))
                .collect();
            // So many, one a row, none in the first rows or at the Heart,
            // never on two rows running and never far apart — and the last
            // on the row under the Heart, its one place.
            assert_eq!(rows.len() as u32, data::FLOOR_SHOPS, "seed {seed}");
            assert_eq!(*rows.last().unwrap(), top - 1, "seed {seed}");
            assert!(shape.shops[top - 1][0], "seed {seed}");
            for &r in &rows {
                assert_eq!(shape.shops[r].iter().filter(|&&s| s).count(), 1);
                assert!(r as u32 >= data::FLOOR_FIRST_SHOP_ROW && r < top);
            }
            for w in rows.windows(2) {
                assert!(
                    w[1] > w[0] + 1 && w[1] - w[0] <= 6,
                    "seed {seed}: rows {w:?}"
                );
            }
            // From most places below the last of them, a trader a trip up
            // can reach within seven rows.
            let ahead = rows_to_a_shop(&shape);
            for row in 0..*rows.last().unwrap() - 7 {
                for d in &ahead[row] {
                    places += 1;
                    near += u32::from(d.is_some_and(|d| d <= 7));
                }
            }
            // A crew out for them meets one every seven rows or so, and
            // never goes ten without.
            let dry = best_way_s_dry_stretch(&shape);
            assert!(
                dry <= 10,
                "seed {seed}: the best way goes {dry} rows without"
            );
            dry_sum += dry;
            driest = driest.max(dry);
            met += most_met(&shape);
        }
        let near = f64::from(near) / f64::from(places);
        let dry = f64::from(dry_sum) / seeds as f64;
        println!(
            "{:.1}% of places a trader within 7 rows; the best way's longest stretch without {dry:.1} rows (at most {driest}); a way meets at most {:.1}",
            near * 100.0,
            f64::from(met) / seeds as f64
        );
        assert!(near > 0.95, "{near}");
        assert!(dry < 7.0, "{dry}");
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
