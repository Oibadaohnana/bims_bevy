//! Procedural stations (feature 112): every station but the spawn is a
//! building generated from its seed rather than one of six drawn plans.
//!
//! # What is generated, and what is not
//!
//! The generator draws a [`Floor`] — the hull as a union of blocks, the
//! rooms and what each is for, the walls and their doors, the airlocks, the
//! array, the cover — and hands it to [`furnish_placer`], the one furnisher
//! every plan goes through, unchanged. So everything downstream of the
//! floor (the room, docking, trade, the machines' waves, the painter) sees
//! a station built the way the drawn plans are built.
//!
//! # The shape: a ladder
//!
//! A station is a **ladder** of corridors: two or three long corridors, the
//! **rails**, and two to four short ones across them, the **rungs**, every
//! rung crossing every rail. The rectangles between two rails and two rungs
//! are **cells**, cut into rooms that share their walls — or left as void,
//! a courtyard with the hull's skin round it — and the rails' outer sides
//! carry **bands** of rooms against the skin, each room as deep as it
//! rolled, so the silhouette steps. A rung may run on through a band to the
//! skin as a docking **arm**, and the rails either stop at a closing rung or
//! run on to the skin. The ladder lies east–west (**end on**: the reactor
//! room opens into the first rung) or north–south (**side on**: the
//! reactor room opens into the first rail), and every width, gap, depth and
//! count is a roll, so two docks of one kind are two buildings. Two rails
//! and two rungs round a cell are a **loop** — a crew can go round the cell
//! either way — and every ladder has one.
//!
//! # Wings, and a way in anywhere
//!
//! The ladder is the **core**, and two times in three a compact one, so
//! the size the kind allows is left for **wings** grown off the ends it
//! left against the skin (`grow_wings`): a boom run on out past an arm or
//! a rail, turning once one time in two, rooms along it where they fit,
//! and a module across its far end — so a station is a core with things
//! sticking out of it, never quite the same shape twice. Every block is
//! tried against everything drawn and shortened or left out when it
//! does not fit. The **airlocks** after the port go in any straight run
//! of skin with deck two deep inside — a corridor's or a room's — so the
//! machines, which come aboard by every airlock but the port in turn
//! (`droid::arrival_airlock_at`), land in stores, quarters and halls in
//! different parts of the station.
//!
//! # The contract, and how it is kept
//!
//! A candidate is **furnished and then checked** ([`check`]) against the
//! walkability contract (`crates/world/CLAUDE.md`) with a flood that is
//! stricter than the room's navigation: a body is a two-by-two window of
//! free tiles, moved a tile at a time, and a tile counts as reached only if
//! a reached window holds it or it sits beside a tile one does. Every use
//! spot and every open tile of deck must be reached from inside the port.
//! Doors are chosen after a trial furnishing with the walls closed, where
//! the tiles inside and outside the doorway two deep are free of whatever
//! the furnisher stood there. A candidate that fails anything is thrown
//! away and the next drawn, [`ATTEMPTS`] of them; when all fail the
//! station is the drawn plan its seed rolled before (`Plan::hand_rolled`).
//!
//! # Deterministic
//!
//! A pure function of the kind and the seed: integers only, every draw off
//! `Rng::new(seed ^ GEN_SALT)` branched by the attempt — a stream of its own,
//! so the plan roll, the furnisher's dressing and every other stream are
//! what they were — and nothing iterated out of a hash.

use shipdesign::{Layer, PartKind, Rotation};
use worldgen::StationKind;
use worldgen::rng::Rng;

use crate::station::{Block, Floor, Placer, enclose, furnish_placer, residents_of};

/// The salt the generator draws with, "STAGEN": its own stream off the
/// seed.
pub const GEN_SALT: u64 = 0x_5354_4147_454e_0000;

/// How many candidates are furnished and checked before the drawn plan is
/// fallen back on.
pub const ATTEMPTS: u32 = 24;

/// How many drawings an attempt makes before one is the size its kind
/// wants and has a room for every role: drawing is cheap, furnishing is
/// not.
pub const SKETCHES: u32 = 12;

/// The reactor room's east wall, where the rest of the station begins: the
/// drawn plans' `REACTOR_ROOM`, eight tiles of deck in from the port.
pub const LOBBY_EAST: u32 = 10;

/// How many tiles of deck run straight in from the port on its two rows,
/// at least: `World::stage_droid_fight_for_probe` stands a machine eight in
/// and the crew come ashore two and a half in.
pub const MIN_RUN: u32 = 9;

/// The least deck inside each room, `(across, down)`, read off `furnish`'s
/// own offsets. The reactor room: the desk at `x0 + 1`, the reactor at
/// `x0 + 5` two wide, life support at `x0 + 6` and the batteries up the
/// south wall, with the port's two rows clear between.
pub const LOBBY_MIN: (u32, u32) = (8, 11);
/// The mess: the galley along the north wall to `x0 + 5` and a table at
/// `y0 + 3` with its chairs under it and a row behind.
pub const MESS_MIN: (u32, u32) = (8, 6);
/// The quarters: two columns of bunks two deep — four, the most a
/// station's residents and two mercenaries need — with the gangway past
/// them; [`bunks_in`] is what a room of any size holds.
pub const QUARTERS_MIN: (u32, u32) = (7, 7);
/// The heads: a toilet, a basin and a shower along the north wall from
/// `x1 - 3`, worked from the row below.
pub const HEADS_MIN: (u32, u32) = (5, 4);
/// The research room: the desk at `x0 + 1`, and a run of trays six wide
/// from `y0 + 3` where the lab has not taken them all.
pub const RESEARCH_MIN: (u32, u32) = (7, 6);
/// The laboratory: runs of trays six wide from `y0 + 2`, the broom locker
/// in the corner.
pub const LAB_MIN: (u32, u32) = (7, 5);
/// The rec room: a table and its chairs from `y0 + 1`, the picture at
/// `x0 + 4`.
pub const REC_MIN: (u32, u32) = (6, 5);
/// A store: shelves along the north wall from `x0 + 2`.
pub const STORE_MIN: (u32, u32) = (5, 4);

/// How many columns of bunks a room's quarters lay, at most.
const BUNK_COLUMNS: u32 = 3;

/// How many bunks `furnish` lays in quarters `w` by `h` inside: a column
/// every three tiles from the west wall while two tiles of gangway are
/// left past it, a bunk every three rows while a row is left under it.
pub fn bunks_in(w: u32, h: u32) -> u32 {
    if w < 4 || h < 4 {
        return 0;
    }
    ((w - 4) / 3 + 1).min(BUNK_COLUMNS) * ((h - 4) / 3 + 1)
}

/// The largest build area a kind is generated at, and the smallest: a
/// relay never as big as an orbital. The largest is the drawn spine's on
/// an orbital, sixty-eight, less two.
pub fn side_range(kind: StationKind) -> (u32, u32) {
    match kind {
        StationKind::Relay => (32, 46),
        StationKind::MiningOutpost | StationKind::Derelict => (38, 54),
        StationKind::Refinery => (42, 58),
        StationKind::Orbital => (50, 66),
    }
}

/// A draw in `lo..=hi`.
fn roll(rng: &mut Rng, lo: i32, hi: i32) -> i32 {
    if hi <= lo {
        return lo;
    }
    lo + rng.below((hi - lo + 1) as u32) as i32
}

/// `true` one time in `n`.
fn one_in(rng: &mut Rng, n: u32) -> bool {
    rng.below(n) == 0
}

/// A block in signed tiles, inclusive, while the station is being drawn
/// and before it is moved onto the grid.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Rect {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

impl Rect {
    fn new(x0: i32, y0: i32, x1: i32, y1: i32) -> Rect {
        Rect { x0, y0, x1, y1 }
    }

    fn shifted(self, dx: i32, dy: i32) -> Rect {
        Rect::new(self.x0 + dx, self.y0 + dy, self.x1 + dx, self.y1 + dy)
    }

    fn block(self) -> Block {
        Block::new(
            self.x0 as u32,
            self.y0 as u32,
            self.x1 as u32,
            self.y1 as u32,
        )
    }

    /// Whether the two share a tile, rings included.
    fn overlaps(self, o: Rect) -> bool {
        self.x0 <= o.x1 && o.x0 <= self.x1 && self.y0 <= o.y1 && o.y0 <= self.y1
    }

    /// Whether this one's deck — inside its ring — reaches any tile of
    /// `o`, ring included.
    fn inside_hits(self, o: Rect) -> bool {
        self.x0 + 1 <= o.x1 && o.x0 <= self.x1 - 1 && self.y0 + 1 <= o.y1 && o.y0 <= self.y1 - 1
    }

    /// Whether a block drawn here sits beside `o` as a building's blocks
    /// may: sharing nothing but a ring line (a wall between two rooms, a
    /// room's wall the corridor's edge), or not touching at all — but
    /// never touching only corner to corner across a diagonal, which is
    /// a hull meeting itself at a point.
    fn beside(self, o: Rect) -> bool {
        if self.overlaps(o) {
            return !self.inside_hits(o) && !o.inside_hits(self);
        }
        let gap_x = (self.x0 - o.x1 - 1).max(o.x0 - self.x1 - 1);
        let gap_y = (self.y0 - o.y1 - 1).max(o.y0 - self.y1 - 1);
        !(gap_x == 0 && gap_y == 0)
    }

    /// The block continuing this corridor `len` tiles on past its far
    /// end towards `d`, as wide as it: the two overlap by the end's last
    /// row of deck and its ring, so the two are one run of deck.
    fn extended(self, d: (i32, i32), len: i32) -> Rect {
        match d {
            (1, _) => Rect::new(self.x1 - 1, self.y0, self.x1 + len, self.y1),
            (-1, _) => Rect::new(self.x0 - len, self.y0, self.x0 + 1, self.y1),
            (_, 1) => Rect::new(self.x0, self.y1 - 1, self.x1, self.y1 + len),
            _ => Rect::new(self.x0, self.y0 - len, self.x1, self.y0 + 1),
        }
    }

    /// A corridor `width` wide turning off this one's far end (it runs
    /// towards `d`) towards `t`, `len` tiles past this one's side: flush
    /// with the end, so the corner is one square of deck.
    fn turned(self, d: (i32, i32), t: (i32, i32), width: i32, len: i32) -> Rect {
        let across = width + 1;
        let (y0, y1, x0, x1) = match d {
            (_, 1) => (self.y1 - across, self.y1, 0, 0),
            (_, -1) => (self.y0, self.y0 + across, 0, 0),
            (1, _) => (0, 0, self.x1 - across, self.x1),
            _ => (0, 0, self.x0, self.x0 + across),
        };
        match t {
            (1, _) => Rect::new(self.x0, y0, self.x1 + len, y1),
            (-1, _) => Rect::new(self.x0 - len, y0, self.x1, y1),
            (_, 1) => Rect::new(x0, self.y0, x1, self.y1 + len),
            _ => Rect::new(x0, self.y0 - len, x1, self.y1),
        }
    }
}

/// What a room is for: the roles `furnish` lays fixtures in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Role {
    Quarters,
    Mess,
    Research,
    Lab,
    Rec,
    Heads,
    Store,
}

impl Role {
    fn min(self) -> (u32, u32) {
        match self {
            Role::Quarters => QUARTERS_MIN,
            Role::Mess => MESS_MIN,
            Role::Research => RESEARCH_MIN,
            Role::Lab => LAB_MIN,
            Role::Rec => REC_MIN,
            Role::Heads => HEADS_MIN,
            Role::Store => STORE_MIN,
        }
    }
}

/// A station drawn but not yet furnished: the blocks and the port's row.
struct Sketch {
    lobby: Rect,
    /// The port's upper row; its two tiles are `py` and `py + 1` in the
    /// west skin, and the reactor room's door is on the same two rows.
    py: i32,
    corridors: Vec<Rect>,
    rooms: Vec<Rect>,
    /// Whether the ladder lies north–south (the reactor room opening into
    /// the first rail) rather than east–west.
    side_on: bool,
}

/// The ladder, in its own frame: `u` along the rails, `v` across them.
struct Ladder {
    rails: Vec<(i32, i32)>,
    rungs: Vec<(i32, i32)>,
    /// The rails' extent along `u`, walls included.
    u0: i32,
    u1: i32,
}

/// Walls across `lo..=hi` — `lo` and `hi` themselves among them — cutting
/// it into rooms `min..=max` inside, sharing their walls. One room however
/// wide when no cut fits; `None` when not even one does.
fn split(lo: i32, hi: i32, min: i32, max: i32, rng: &mut Rng) -> Option<Vec<i32>> {
    let n = hi - lo - 1;
    if n < min {
        return None;
    }
    let kmin = (n + 1 + max) / (max + 1);
    let kmax = (n + 1) / (min + 1);
    if kmax < 1 || kmin > kmax {
        return Some(vec![lo, hi]);
    }
    let k = roll(rng, kmin, kmax);
    let mut widths = vec![min; k as usize];
    let mut spare = n - (k * min + (k - 1));
    let mut guard = 0;
    while spare > 0 && guard < 10_000 {
        guard += 1;
        let i = rng.below(k as u32) as usize;
        if widths[i] < max {
            widths[i] += 1;
            spare -= 1;
        }
    }
    if spare > 0 {
        widths[0] += spare;
    }
    let mut walls = vec![lo];
    let mut at = lo;
    for w in widths {
        at += w + 1;
        walls.push(at);
    }
    Some(walls)
}

/// The dimensions a kind rolls within.
struct Dials {
    rails: (i32, i32),
    rail_width: (i32, i32),
    rung_width: (i32, i32),
    rungs: (i32, i32),
    cell_across: (i32, i32),
    cell_along: (i32, i32),
}

fn dials(kind: StationKind) -> Dials {
    match kind {
        StationKind::Relay => Dials {
            rails: (2, 2),
            rail_width: (2, 3),
            rung_width: (2, 3),
            rungs: (2, 2),
            cell_across: (7, 10),
            cell_along: (14, 20),
        },
        StationKind::MiningOutpost | StationKind::Derelict => Dials {
            rails: (2, 3),
            rail_width: (2, 4),
            rung_width: (2, 3),
            rungs: (2, 3),
            cell_across: (7, 10),
            cell_along: (8, 15),
        },
        StationKind::Refinery => Dials {
            rails: (2, 3),
            rail_width: (2, 4),
            rung_width: (2, 3),
            rungs: (2, 3),
            cell_across: (6, 13),
            cell_along: (7, 18),
        },
        StationKind::Orbital => Dials {
            rails: (2, 3),
            rail_width: (2, 5),
            rung_width: (2, 4),
            rungs: (2, 4),
            cell_across: (6, 14),
            cell_along: (7, 18),
        },
    }
}

/// Draw a ladder, its rooms and its reactor room: the blocks, before a
/// role, a door or an airlock is decided. `None` when what was drawn is
/// too big for the kind.
fn sketch(kind: StationKind, rng: &mut Rng) -> Option<Sketch> {
    let d = dials(kind);
    // A relay is small enough that a void where a room could be is often
    // the room a role wanted: every cell and band is built.
    let full = kind == StationKind::Relay;
    let side_on = rng.below(5) < 2;
    // Two times in three a **compact** core — the fewest rails and rungs
    // the kind has, the cells short — so the size the kind allows is
    // left to the wings, and the station is a core with things growing
    // off it rather than one block.
    let compact = rng.below(3) < 2;
    let (rails_n, rungs_n) = if compact {
        (d.rails.0, d.rungs.0)
    } else {
        (
            roll(rng, d.rails.0, d.rails.1),
            roll(rng, d.rungs.0, d.rungs.1),
        )
    };
    let cell_along = if compact {
        (
            d.cell_along.0,
            d.cell_along.0 + (d.cell_along.1 - d.cell_along.0) / 2,
        )
    } else {
        d.cell_along
    };

    // Across: the rails and the cells between them, from v = 0.
    let mut rails = Vec::new();
    let mut v = 0;
    for j in 0..rails_n {
        let w = roll(rng, d.rail_width.0, d.rail_width.1);
        rails.push((v, v + w + 1));
        v += w + 1;
        if j + 1 < rails_n {
            v += roll(rng, d.cell_across.0, d.cell_across.1) + 1;
        }
    }

    // Along: the rungs, with an open or a closed end each side. End on,
    // the first rung is at u = 0 against the reactor room, closed.
    let mut rungs = Vec::new();
    let open_start = side_on && one_in(rng, 2);
    let mut u = if open_start { roll(rng, 6, 11) + 1 } else { 0 };
    for i in 0..rungs_n {
        let w = roll(rng, d.rung_width.0, d.rung_width.1);
        rungs.push((u, u + w + 1));
        u += w + 1;
        if i + 1 < rungs_n {
            u += roll(rng, cell_along.0, cell_along.1) + 1;
        }
    }
    let open_end = one_in(rng, 2);
    let u1 = if open_end {
        u + roll(rng, 6, 12) + 1
    } else {
        u
    };
    let ladder = Ladder {
        rails,
        rungs,
        u0: 0,
        u1,
    };
    let first = ladder.rails[0];
    let last = *ladder.rails.last()?;

    // The corridors, and the ends of them a wing may grow from: a rail
    // run on to the skin, an arm.
    let mut along: Vec<(i32, i32, i32, i32)> = Vec::new(); // (u0, v0, u1, v1)
    let mut stubs: Vec<((i32, i32, i32, i32), (i32, i32))> = Vec::new(); // (block, (du, dv))
    for &(a, b) in &ladder.rails {
        along.push((ladder.u0, a, ladder.u1, b));
        if open_end {
            stubs.push(((ladder.u0, a, ladder.u1, b), (1, 0)));
        }
        if open_start {
            stubs.push(((ladder.u0, a, ladder.u1, b), (-1, 0)));
        }
    }
    for &(a, b) in &ladder.rungs {
        along.push((a, first.0, b, last.1));
    }

    // Arms: a rung run on through a band to the skin, decided before the
    // bands so the bands are cut round them. End on, a band either side;
    // side on, only away from the reactor room.
    let sides: &[bool] = if side_on { &[true] } else { &[true, false] };
    let arm_odds = if compact || kind == StationKind::Orbital {
        2
    } else {
        3
    };
    let mut arms: Vec<(usize, bool)> = Vec::new();
    for (i, _) in ladder.rungs.iter().enumerate() {
        for &high in sides {
            if one_in(rng, arm_odds) {
                arms.push((i, high));
            }
        }
    }

    // The rooms, in the ladder's frame: (u0, v0, u1, v1).
    let mut rooms: Vec<(i32, i32, i32, i32)> = Vec::new();

    // The cells between rails, between cuts. The cuts are the rungs and
    // the open ends; a cell between two rungs may be left void.
    let mut spans: Vec<(i32, i32, bool)> = Vec::new(); // (u0, u1, is an end)
    if ladder.rungs[0].0 > ladder.u0 {
        spans.push((ladder.u0, ladder.rungs[0].0, true));
    }
    for pair in ladder.rungs.windows(2) {
        spans.push((pair[0].1, pair[1].0, false));
    }
    let last_rung = *ladder.rungs.last()?;
    if last_rung.1 < ladder.u1 {
        spans.push((last_rung.1, ladder.u1, true));
    }
    for pair in ladder.rails.windows(2) {
        let (va, vb) = (pair[0].1, pair[1].0);
        for &(ua, ub, end) in &spans {
            if !end && !full && one_in(rng, 5) {
                continue;
            }
            let Some(walls) = split(ua, ub, 7, 12, rng) else {
                continue;
            };
            for strip in walls.windows(2) {
                let deep = vb - va - 1;
                if deep >= 13 && one_in(rng, 2) {
                    let cut = roll(rng, va + 7, vb - 7);
                    rooms.push((strip[0], va, strip[1], cut));
                    rooms.push((strip[0], cut, strip[1], vb));
                } else {
                    rooms.push((strip[0], va, strip[1], vb));
                }
            }
        }
    }

    // The bands: rooms against the skin along each rail's outer side, cut
    // round the arms, each as deep as it rolled; some left out.
    let mut arm_blocks: Vec<((i32, i32, i32, i32), (i32, i32))> = Vec::new();
    for &high in sides {
        if !full && one_in(rng, 6) {
            // No band this side: the rail's outer wall is the skin, save
            // for the arms.
            for &(i, h) in &arms {
                if h == high {
                    let (a, b) = ladder.rungs[i];
                    let reach = roll(rng, 4, 8);
                    arm_blocks.push(if high {
                        ((a, last.1, b, last.1 + reach), (0, 1))
                    } else {
                        ((a, first.0 - reach, b, first.0), (0, -1))
                    });
                }
            }
            continue;
        }
        let mut cuts: Vec<(i32, i32)> = arms
            .iter()
            .filter(|&&(_, h)| h == high)
            .map(|&(i, _)| ladder.rungs[i])
            .collect();
        cuts.sort();
        let mut segments = Vec::new();
        let mut start = ladder.u0;
        for &(a, b) in &cuts {
            segments.push((start, a));
            start = b;
        }
        segments.push((start, ladder.u1));
        let mut depths: Vec<(i32, i32, i32)> = Vec::new(); // (u0, u1, depth)
        for (sa, sb) in segments {
            let Some(walls) = split(sa, sb, 7, 12, rng) else {
                continue;
            };
            for strip in walls.windows(2) {
                if !full && one_in(rng, 6) {
                    continue;
                }
                let deep = roll(rng, 6, 9);
                depths.push((strip[0], strip[1], deep));
                rooms.push(if high {
                    (strip[0], last.1, strip[1], last.1 + deep + 1)
                } else {
                    (strip[0], first.0 - deep - 1, strip[1], first.0)
                });
            }
        }
        for &(a, b) in &cuts {
            let beside = depths
                .iter()
                .filter(|&&(ra, rb, _)| rb == a || ra == b)
                .map(|&(_, _, deep)| deep + 1)
                .max()
                .unwrap_or(4);
            let reach = beside + roll(rng, 1, 4);
            arm_blocks.push(if high {
                ((a, last.1, b, last.1 + reach), (0, 1))
            } else {
                ((a, first.0 - reach, b, first.0), (0, -1))
            });
        }
    }
    along.extend(arm_blocks.iter().map(|&(block, _)| block));
    stubs.extend(arm_blocks);

    // Onto the grid: end on, x = LOBBY_EAST + u and y = v; side on, x =
    // LOBBY_EAST + v and y = u.
    let east = LOBBY_EAST as i32;
    let place = |(u0, v0, u1, v1): (i32, i32, i32, i32)| {
        if side_on {
            Rect::new(east + v0, u0, east + v1, u1)
        } else {
            Rect::new(east + u0, v0, east + u1, v1)
        }
    };
    let mut corridors: Vec<Rect> = along.into_iter().map(place).collect();
    let mut rooms: Vec<Rect> = rooms.into_iter().map(place).collect();
    let stubs: Vec<(Rect, (i32, i32))> = stubs
        .into_iter()
        .map(|(block, (du, dv))| (place(block), if side_on { (dv, du) } else { (du, dv) }))
        .collect();

    // The reactor room, its door onto the first rung (end on) or the first
    // rail (side on), on the port's two rows.
    let entry = corridors[if side_on { 0 } else { ladder.rails.len() }];
    let (lo, hi) = (entry.y0 + 1, entry.y1 - 2);
    if hi < lo {
        return None;
    }
    let py = roll(rng, lo, hi);
    let tall = if one_in(rng, 2) { 13 } else { 15 };
    let above = roll(rng, 5, tall - 6);
    let lobby = Rect::new(1, py - above, east, py - above + tall - 1);

    // The wings, grown off the ends the ladder left against the skin.
    grow_wings(kind, &mut corridors, &mut rooms, lobby, &stubs, rng);

    // Moved down so the topmost tile is row one.
    let top = corridors
        .iter()
        .chain(&rooms)
        .chain([&lobby])
        .map(|r| r.y0)
        .min()?;
    let dy = 1 - top;
    let corridors: Vec<Rect> = corridors.into_iter().map(|r| r.shifted(0, dy)).collect();
    let rooms: Vec<Rect> = rooms.into_iter().map(|r| r.shifted(0, dy)).collect();
    Some(Sketch {
        lobby: lobby.shifted(0, dy),
        py: py + dy,
        corridors,
        rooms,
        side_on,
    })
}

/// Where the station may still grow: the box round everything drawn,
/// and the kind's largest side it must stay inside once the topmost
/// tile is moved to row one.
struct Bounds {
    most: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

impl Bounds {
    fn of(kind: StationKind, blocks: &[Rect]) -> Bounds {
        Bounds {
            most: side_range(kind).1 as i32,
            y0: blocks.iter().map(|r| r.y0).min().unwrap_or(0),
            x1: blocks.iter().map(|r| r.x1).max().unwrap_or(0),
            y1: blocks.iter().map(|r| r.y1).max().unwrap_or(0),
        }
    }

    /// Whether `r` keeps the build area within the kind's largest, and
    /// never west of the port's column.
    fn holds(&self, r: Rect) -> bool {
        let (y0, x1, y1) = (self.y0.min(r.y0), self.x1.max(r.x1), self.y1.max(r.y1));
        r.x0 >= 1 && x1.max(y1 - y0 + 1) + 2 <= self.most
    }

    fn take(&mut self, r: Rect) {
        self.y0 = self.y0.min(r.y0);
        self.x1 = self.x1.max(r.x1);
        self.y1 = self.y1.max(r.y1);
    }
}

/// Whether `r` may be drawn among `blocks`, beside every one of them as
/// [`Rect::beside`] asks — bar `parent`, which a corridor grown off it
/// overlaps.
fn fits(r: Rect, blocks: &[Rect], parent: Option<Rect>) -> bool {
    blocks.iter().all(|&o| Some(o) == parent || r.beside(o))
}

/// Grow **wings** off the ends the ladder left against the skin — an
/// arm's, a rail's run on to it — so the silhouette is a core with things
/// sticking out of it rather than a block: a corridor run on out past the
/// end (a boom), sometimes turning once (a dog-leg), rooms along either
/// side where they fit, and one time in two a **module** across its far
/// end, wider than the corridor, which the rooms along it may meet. Every
/// block is tried against everything drawn ([`fits`]) and against the
/// kind's largest size, shortened or made shallower when it does not fit,
/// and left out when nothing does — so a wing is as long as the station
/// has room for, and a station with no room left has plain arms.
fn grow_wings(
    kind: StationKind,
    corridors: &mut Vec<Rect>,
    rooms: &mut Vec<Rect>,
    lobby: Rect,
    stubs: &[(Rect, (i32, i32))],
    rng: &mut Rng,
) {
    let d = dials(kind);
    let mut drawn: Vec<Rect> = corridors.iter().chain(rooms.iter()).copied().collect();
    drawn.push(lobby);
    let mut bounds = Bounds::of(kind, &drawn);
    let skip = match kind {
        StationKind::Relay => 3,
        _ => 5,
    };
    for &(stub, dir) in stubs {
        // Some ends are left as the ladder drew them.
        if rng.below(skip) < 2 {
            continue;
        }
        // The corridor: on out past the end, and one time in two a turn.
        let mut segments: Vec<(Rect, (i32, i32), Rect)> = Vec::new(); // (block, way, parent)
        let (mut parent, mut way) = (stub, dir);
        let turns = u32::from(one_in(rng, 2));
        for s in 0..=turns {
            let len = roll(rng, 5, 13);
            let width = roll(rng, d.rung_width.0, d.rung_width.1.min(3));
            let next = if s == 0 {
                way
            } else if one_in(rng, 2) {
                (way.1, way.0)
            } else {
                (-way.1, -way.0)
            };
            let mut placed = None;
            let mut l = len;
            while l >= 4 {
                let r = if s == 0 {
                    parent.extended(way, l)
                } else {
                    parent.turned(way, next, width, l)
                };
                if bounds.holds(r) && fits(r, &drawn, Some(parent)) {
                    placed = Some(r);
                    break;
                }
                l -= 2;
            }
            let Some(r) = placed else {
                break;
            };
            bounds.take(r);
            drawn.push(r);
            corridors.push(r);
            segments.push((r, next, parent));
            parent = r;
            way = next;
        }
        if segments.is_empty() {
            continue;
        }
        let mut along = 0;
        // Rooms along either side of each segment, from where it leaves
        // its parent to its end.
        for &(r, way, from) in &segments {
            let along_x = way.0 != 0;
            let (lo, hi) = match way {
                (1, _) => (from.x1, r.x1),
                (-1, _) => (r.x0, from.x0),
                (_, 1) => (from.y1, r.y1),
                _ => (r.y0, from.y0),
            };
            for high in [false, true] {
                if one_in(rng, 4) {
                    continue;
                }
                let Some(walls) = split(lo, hi, 7, 12, rng) else {
                    continue;
                };
                for strip in walls.windows(2) {
                    let mut deep = roll(rng, 5, 9);
                    while deep >= 4 {
                        let (a, b) = (strip[0], strip[1]);
                        let block = match (along_x, high) {
                            (true, true) => Rect::new(a, r.y1, b, r.y1 + deep + 1),
                            (true, false) => Rect::new(a, r.y0 - deep - 1, b, r.y0),
                            (false, true) => Rect::new(r.x1, a, r.x1 + deep + 1, b),
                            (false, false) => Rect::new(r.x0 - deep - 1, a, r.x0, b),
                        };
                        if bounds.holds(block) && fits(block, &drawn, None) {
                            bounds.take(block);
                            drawn.push(block);
                            rooms.push(block);
                            along += 1;
                            break;
                        }
                        deep -= 1;
                    }
                }
            }
        }
        // A module across the far end, one time in two — always where
        // no room fitted along it, so a boom leads somewhere.
        let &(end, way, _) = segments.last().expect("a segment");
        if along > 0 && one_in(rng, 2) {
            continue;
        }
        let (e0, e1) = (roll(rng, 2, 5), roll(rng, 2, 5));
        let mut deep = roll(rng, 6, 10);
        while deep >= 5 {
            let block = match way {
                (1, _) => Rect::new(end.x1, end.y0 - e0, end.x1 + deep + 1, end.y1 + e1),
                (-1, _) => Rect::new(end.x0 - deep - 1, end.y0 - e0, end.x0, end.y1 + e1),
                (_, 1) => Rect::new(end.x0 - e0, end.y1, end.x1 + e1, end.y1 + deep + 1),
                _ => Rect::new(end.x0 - e0, end.y0 - deep - 1, end.x1 + e1, end.y0),
            };
            if bounds.holds(block) && fits(block, &drawn, None) {
                bounds.take(block);
                drawn.push(block);
                rooms.push(block);
                break;
            }
            deep -= 1;
        }
    }
}

/// The build area a sketch wants: the farthest tile plus the margin, or
/// why it is outside the kind's range.
fn side_for(kind: StationKind, sketch: &Sketch) -> Result<u32, Fail> {
    let far = sketch
        .corridors
        .iter()
        .chain(&sketch.rooms)
        .chain([&sketch.lobby])
        .map(|r| r.x1.max(r.y1))
        .max()
        .ok_or(Fail::Sketch)?;
    let side = (far + 2) as u32;
    let (least, most) = side_range(kind);
    match side {
        s if s < least => Err(Fail::TooSmall),
        s if s > most => Err(Fail::TooBig),
        s => Ok(s),
    }
}

/// The roles dealt to a sketch's rooms, in its room order: the demanding
/// first, each to the smallest room it fits, the lab and the rec room where
/// a room is left for them, and every room left over a store.
fn deal(kind: StationKind, rooms: &[Block], rng: &mut Rng) -> Option<Vec<Role>> {
    let residents = residents_of(kind);
    let size = |b: &Block| (b.x1 - b.x0 - 1, b.y1 - b.y0 - 1);
    let fits = |role: Role, b: &Block| {
        let (w, h) = size(b);
        let (mw, mh) = role.min();
        w >= mw && h >= mh && (role != Role::Quarters || bunks_in(w, h) >= residents + 2)
    };
    let mut roles: Vec<Option<Role>> = vec![None; rooms.len()];
    let mut want = vec![Role::Quarters, Role::Mess, Role::Research];
    if one_in(rng, 3) {
        want.push(Role::Rec);
        want.push(Role::Lab);
    } else {
        want.push(Role::Lab);
        want.push(Role::Rec);
    }
    want.extend([Role::Heads, Role::Store]);
    for role in want {
        let pick = (0..rooms.len())
            .filter(|&i| roles[i].is_none() && fits(role, &rooms[i]))
            .min_by_key(|&i| {
                let (w, h) = size(&rooms[i]);
                (w * h, i)
            });
        match pick {
            Some(i) => roles[i] = Some(role),
            None if matches!(role, Role::Lab | Role::Rec) => {}
            None => return None,
        }
    }
    Some(
        roles
            .into_iter()
            .map(|r| r.unwrap_or(Role::Store))
            .collect(),
    )
}

/// A station's tiles as a raster the drawing is checked on: which are
/// hull, which are corridor deck.
struct Raster {
    side: u32,
    hull: Vec<bool>,
    corridor: Vec<bool>,
    /// The deck inside the rooms (not the reactor room's).
    room: Vec<bool>,
}

impl Raster {
    fn new(side: u32, sketch: &Sketch) -> Raster {
        let n = (side * side) as usize;
        let mut hull = vec![false; n];
        let mut corridor = vec![false; n];
        let mut room = vec![false; n];
        let paint = |grid: &mut Vec<bool>, r: Rect| {
            for y in r.y0.max(0)..=r.y1.min(side as i32 - 1) {
                for x in r.x0.max(0)..=r.x1.min(side as i32 - 1) {
                    grid[(y as u32 * side + x as u32) as usize] = true;
                }
            }
        };
        for &r in sketch
            .corridors
            .iter()
            .chain(&sketch.rooms)
            .chain([&sketch.lobby])
        {
            paint(&mut hull, r);
        }
        for &r in &sketch.corridors {
            paint(
                &mut corridor,
                Rect::new(r.x0 + 1, r.y0 + 1, r.x1 - 1, r.y1 - 1),
            );
        }
        for &r in &sketch.rooms {
            paint(&mut room, Rect::new(r.x0 + 1, r.y0 + 1, r.x1 - 1, r.y1 - 1));
        }
        Raster {
            side,
            hull,
            corridor,
            room,
        }
    }

    fn at(&self, grid: &[bool], x: i32, y: i32) -> bool {
        x >= 0
            && y >= 0
            && (x as u32) < self.side
            && (y as u32) < self.side
            && grid[(y as u32 * self.side + x as u32) as usize]
    }

    fn hull(&self, x: i32, y: i32) -> bool {
        self.at(&self.hull, x, y)
    }

    fn corridor(&self, x: i32, y: i32) -> bool {
        self.at(&self.corridor, x, y)
    }

    fn room(&self, x: i32, y: i32) -> bool {
        self.at(&self.room, x, y)
    }

    /// A hull tile with any of its eight neighbours outside: the skin.
    fn skin(&self, x: i32, y: i32) -> bool {
        self.hull(x, y)
            && (-1..=1)
                .any(|dx| (-1..=1).any(|dy| (dx, dy) != (0, 0) && !self.hull(x + dx, y + dy)))
    }
}

/// The four ways out of a tile, as the unit steps.
const SIDES: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

/// Where the airlocks go, the port first. A site is a pair of skin tiles
/// in a straight run of skin with space beyond both and deck two deep
/// inside both — a corridor's, or **a room's**, so the machines may come
/// aboard into a store or somebody's quarters as well as into a hall —
/// with nothing the trial furnishing stood on those four tiles, and not
/// on the array or beside it. As many as the kind wants and the hull has,
/// none nearer another than ten tiles, each drawn among the few sites
/// farthest from every one chosen, a room's and a corridor's in turn
/// where both are left, so a station's ways in are spread round it and
/// lead into different parts of it.
fn airlocks(
    kind: StationKind,
    raster: &Raster,
    sketch: &Sketch,
    trial: &Placer,
    array: (u32, u32),
    rng: &mut Rng,
) -> Vec<((u32, u32), Rotation)> {
    let port = ((1u32, sketch.py as u32), Rotation::R0);
    let (arx, ary) = (array.0 as i32, array.1 as i32);
    let clear = |x: i32, y: i32| !trial.blocked((x, y));
    // (the upper or western tile, the turn, whether it opens into a room)
    let mut sites: Vec<((i32, i32), Rotation, bool)> = Vec::new();
    let side = raster.side as i32;
    for y in 0..side {
        for x in 0..side {
            for (dx, dy) in SIDES {
                // The pair runs across the way out: down a column for east
                // and west, along a row for north and south.
                let (ax, ay, rotation) = if dx != 0 {
                    (0, 1, Rotation::R0)
                } else {
                    (1, 0, Rotation::R90)
                };
                let pair = [(x, y), (x + ax, y + ay)];
                let skin = pair
                    .iter()
                    .all(|&(px, py)| raster.skin(px, py) && !raster.hull(px + dx, py + dy))
                    && raster.skin(x - ax, y - ay)
                    && !raster.hull(x - ax + dx, y - ay + dy)
                    && raster.skin(x + 2 * ax, y + 2 * ay)
                    && !raster.hull(x + 2 * ax + dx, y + 2 * ay + dy);
                if !skin {
                    continue;
                }
                let inward = |deck: &dyn Fn(i32, i32) -> bool| {
                    pair.iter().all(|&(px, py)| {
                        (1..=2).all(|k| {
                            deck(px - k * dx, py - k * dy) && clear(px - k * dx, py - k * dy)
                        })
                    })
                };
                let into_room = inward(&|x, y| raster.room(x, y));
                let into_corridor = inward(&|x, y| raster.corridor(x, y));
                // Never on the array or beside it, never on the west
                // skin by the port.
                let by_array = (-1..=2).any(|k| (x + k * ax, y + k * ay) == (arx, ary));
                if (into_room || into_corridor) && !by_array && !(dx == -1 && x <= 1) {
                    sites.push(((x, y), rotation, into_room));
                }
            }
        }
    }
    let wanted = match kind {
        StationKind::Relay => 1 + rng.below(2) as usize,
        StationKind::Orbital => 3 + rng.below(2) as usize,
        _ => 2 + rng.below(2) as usize,
    };
    let mut chosen: Vec<(i32, i32)> = vec![(1, sketch.py)];
    let mut out = vec![port];
    let mut room_first = one_in(rng, 2);
    let gap = |(x, y): (i32, i32), chosen: &[(i32, i32)]| {
        chosen
            .iter()
            .map(|&(cx, cy)| (x - cx).abs() + (y - cy).abs())
            .min()
            .unwrap_or(i32::MAX)
    };
    while out.len() < 1 + wanted {
        let mut open: Vec<&((i32, i32), Rotation, bool)> = sites
            .iter()
            .filter(|&&(at, _, _)| gap(at, &chosen) >= 10)
            .collect();
        if open.is_empty() {
            break;
        }
        if open.iter().any(|&&(_, _, room)| room == room_first) {
            open.retain(|&&(_, _, room)| room == room_first);
        }
        // Farthest from every way in already chosen first, and a draw
        // among the first few.
        open.sort_by_key(|&&(at, _, _)| (-gap(at, &chosen), at.1, at.0));
        open.truncate(6);
        let Some(&&(at, rotation, _)) = rng.pick(&open) else {
            break;
        };
        chosen.push(at);
        out.push(((at.0 as u32, at.1 as u32), rotation));
        room_first = !room_first;
    }
    out
}

/// The sensor array's tile: a tile of the north skin in a straight run,
/// clear of the port.
fn array(raster: &Raster, port: (u32, u32), rng: &mut Rng) -> Option<(u32, u32)> {
    let side = raster.side as i32;
    let (px, py) = (port.0 as i32, port.1 as i32);
    let taken = |x: i32, y: i32| x == px && (y == py || y == py + 1);
    let mut sites = Vec::new();
    for y in 0..side {
        for x in 0..side {
            let north = |x: i32| raster.skin(x, y) && !raster.hull(x, y - 1) && !taken(x, y);
            if north(x) && north(x - 1) && north(x + 1) {
                sites.push((x as u32, y as u32));
            }
        }
    }
    rng.pick(&sites).copied()
}

/// The floor a sketch, its roles and its doors make: every room walled
/// with its doorways left open, the lobby with its door onto the entry.
fn floor_of(
    sketch: &Sketch,
    roles: &[Role],
    doors: &[((u32, u32), Rotation)],
    locks: &[((u32, u32), Rotation)],
    array: (u32, u32),
    cover: &[(u32, u32)],
) -> Floor {
    let lobby = sketch.lobby.block();
    let rooms: Vec<Block> = sketch.rooms.iter().map(|r| r.block()).collect();
    let mut walls = Vec::new();
    let mut all_doors = Vec::new();
    let lobby_door = ((lobby.x1, sketch.py as u32), Rotation::R0);
    enclose(lobby, &[lobby_door], &mut walls, &mut all_doors);
    for room in &rooms {
        let own: Vec<((u32, u32), Rotation)> = doors
            .iter()
            .copied()
            .filter(|&((x, y), r)| {
                let on_ring = |x: u32, y: u32| {
                    (x == room.x0 || x == room.x1 || y == room.y0 || y == room.y1)
                        && x >= room.x0
                        && x <= room.x1
                        && y >= room.y0
                        && y <= room.y1
                };
                let second = if r == Rotation::R0 {
                    (x, y + 1)
                } else {
                    (x + 1, y)
                };
                on_ring(x, y) && on_ring(second.0, second.1)
            })
            .collect();
        enclose(*room, &own, &mut walls, &mut Vec::new());
    }
    for &door in doors {
        if !all_doors.contains(&door) {
            all_doors.push(door);
        }
    }
    // No wall where any door stands, whoever's ring put it there.
    let door_tiles: Vec<(u32, u32)> = all_doors
        .iter()
        .flat_map(|&((x, y), r)| {
            [
                (x, y),
                if r == Rotation::R0 {
                    (x, y + 1)
                } else {
                    (x + 1, y)
                },
            ]
        })
        .collect();
    walls.retain(|t| !door_tiles.contains(t));
    walls.dedup();
    let pick = |role: Role| roles.iter().position(|&r| r == role).map(|i| rooms[i]);
    let stores: Vec<Block> = roles
        .iter()
        .zip(&rooms)
        .filter(|(r, _)| **r == Role::Store)
        .map(|(_, b)| *b)
        .collect();
    // The big plant in the reactor room, two rows over the port's.
    let hall = (lobby.x0 + 4, sketch.py as u32 - 2);
    let mut lit = vec![lobby];
    lit.extend(sketch.corridors.iter().map(|r| r.block()));
    lit.extend(rooms.iter().copied());
    Floor {
        hull: std::iter::once(lobby)
            .chain(sketch.corridors.iter().map(|r| r.block()))
            .chain(rooms.iter().copied())
            .collect(),
        airlocks: locks.to_vec(),
        array,
        lobby: lobby.inner(),
        walls,
        doors: all_doors,
        cover: cover.to_vec(),
        mess: pick(Role::Mess).unwrap_or(lobby),
        quarters: pick(Role::Quarters).unwrap_or(lobby),
        heads: pick(Role::Heads).unwrap_or(lobby),
        research: pick(Role::Research).unwrap_or(lobby),
        lab: pick(Role::Lab),
        rec: pick(Role::Rec),
        stores,
        bunk_columns: BUNK_COLUMNS,
        lit,
        hall,
        open: false,
        standing_lights: Vec::new(),
        extra: Vec::new(),
        mess_columns: 1,
        wild: None,
        clear: Vec::new(),
        gates: Vec::new(),
    }
}

/// A doorway two tiles along a room's wall, and the tiles two deep either
/// side of it: every place in a room's walls a door could go with corridor
/// deck outside it and nothing the trial furnishing stood inside it.
fn door_sites(room: Block, raster: &Raster, trial: &Placer) -> Vec<((u32, u32), Rotation)> {
    let mut out = Vec::new();
    let (x0, y0, x1, y1) = (
        room.x0 as i32,
        room.y0 as i32,
        room.x1 as i32,
        room.y1 as i32,
    );
    let inner = |x: i32, y: i32| x > x0 && x < x1 && y > y0 && y < y1;
    let clear_in = |x: i32, y: i32| inner(x, y) && !trial.blocked((x, y));
    let clear_out = |x: i32, y: i32| raster.corridor(x, y) && !trial.blocked((x, y));
    // Along the north and south walls: (x, wall row, the way out).
    for (wall, out_dy) in [(y0, -1), (y1, 1)] {
        for x in x0 + 1..x1 - 1 {
            let ok = [x, x + 1].iter().all(|&tx| {
                clear_out(tx, wall + out_dy)
                    && clear_out(tx, wall + 2 * out_dy)
                    && clear_in(tx, wall - out_dy)
                    && clear_in(tx, wall - 2 * out_dy)
            });
            if ok {
                out.push(((x as u32, wall as u32), Rotation::R90));
            }
        }
    }
    for (wall, out_dx) in [(x0, -1), (x1, 1)] {
        for y in y0 + 1..y1 - 1 {
            let ok = [y, y + 1].iter().all(|&ty| {
                clear_out(wall + out_dx, ty)
                    && clear_out(wall + 2 * out_dx, ty)
                    && clear_in(wall - out_dx, ty)
                    && clear_in(wall - 2 * out_dx, ty)
            });
            if ok {
                out.push(((wall as u32, y as u32), Rotation::R0));
            }
        }
    }
    out
}

/// Every tile a door or an airlock keeps clear: its own two and the two
/// beyond each face.
fn doorway_keep(doors: &[((u32, u32), Rotation)]) -> Vec<(i32, i32)> {
    let mut keep = Vec::new();
    for &((x, y), r) in doors {
        let (x, y) = (x as i32, y as i32);
        let tiles = if r == Rotation::R0 {
            [(x, y), (x, y + 1)]
        } else {
            [(x, y), (x + 1, y)]
        };
        for (tx, ty) in tiles {
            for d in -2..=2 {
                keep.push(if r == Rotation::R0 {
                    (tx + d, ty)
                } else {
                    (tx, ty + d)
                });
            }
        }
    }
    keep
}

/// Sandbags across the corridors three wide or more, a line from one wall
/// leaving two tiles past its end, clear of every doorway and airlock by
/// three tiles and of the corridor's ends: none, one or two a corridor.
fn cover(
    sketch: &Sketch,
    raster: &Raster,
    doors: &[((u32, u32), Rotation)],
    locks: &[((u32, u32), Rotation)],
    rng: &mut Rng,
) -> Vec<(u32, u32)> {
    let mut keep = doorway_keep(doors);
    keep.extend(doorway_keep(locks));
    let near_keep = |x: i32, y: i32| {
        keep.iter()
            .any(|&(kx, ky)| (kx - x).abs() <= 3 && (ky - y).abs() <= 3)
    };
    let mut out: Vec<(u32, u32)> = Vec::new();
    for c in &sketch.corridors {
        let (across, along) = (c.y1 - c.y0 - 1, c.x1 - c.x0 - 1);
        // The long way is the way the corridor runs.
        let runs_x = along >= across;
        let width = if runs_x { across } else { along };
        let length = if runs_x { along } else { across };
        if width < 3 || length < 14 {
            continue;
        }
        let bags = rng.below(3);
        let mut from_low = one_in(rng, 2);
        let mut placed: Vec<i32> = Vec::new();
        for _ in 0..bags {
            let lo = if runs_x { c.x0 } else { c.y0 } + 4;
            let hi = if runs_x { c.x1 } else { c.y1 } - 4;
            let at = roll(rng, lo, hi);
            if placed.iter().any(|&p| (p - at).abs() < 8) {
                continue;
            }
            let line: Vec<(i32, i32)> = (0..width - 2)
                .map(|i| {
                    let off = if from_low { 1 + i } else { width - i };
                    if runs_x {
                        (at, c.y0 + off)
                    } else {
                        (c.x0 + off, at)
                    }
                })
                .collect();
            // Straight across this corridor alone: both walls of it at
            // this point, so the line is not in a crossing.
            let ends = if runs_x {
                [(at, c.y0), (at, c.y1)]
            } else {
                [(c.x0, at), (c.x1, at)]
            };
            let across_only = ends.iter().all(|&(x, y)| !raster.corridor(x, y))
                && line.iter().all(|&(x, y)| raster.corridor(x, y));
            if !across_only || line.iter().any(|&(x, y)| near_keep(x, y)) {
                continue;
            }
            out.extend(line.iter().map(|&(x, y)| (x as u32, y as u32)));
            placed.push(at);
            from_low = !from_low;
        }
    }
    out
}

/// What a generated station came to, for the variety the tests print and
/// the hull the determinism is pinned by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tally {
    /// Tiles across the build area.
    pub side: u32,
    /// Tiles of hull.
    pub hull: u32,
    /// Rooms, the reactor room among them.
    pub rooms: u32,
    /// Tiles of corridor deck.
    pub corridor: u32,
    /// Loops a crew can go round: pieces of wall with deck all round them.
    pub loops: u32,
    pub airlocks: u32,
    /// Which way the ladder lies: `true` north–south.
    pub side_on: bool,
    /// A hash of the hull, the walls and the doors: two stations with the
    /// same are one building.
    pub signature: u64,
}

/// A generated station: its furnishing — the design, with the occupancy
/// it was built through — and what it came to.
pub struct Generated {
    pub(crate) placer: Placer,
    pub tally: Tally,
    /// Which attempt it was, from nought.
    pub attempt: u32,
}

/// The station a kind and a seed generate, or `None` when every attempt
/// failed the contract and the drawn plan is to be used instead.
pub fn generate(kind: StationKind, map_seed: u64) -> Option<Generated> {
    let base = Rng::new(map_seed ^ GEN_SALT);
    for attempt in 0..ATTEMPTS {
        let mut rng = base.branch(attempt as u64);
        if let Ok((placer, tally)) = attempt_once(kind, map_seed, &mut rng) {
            return Some(Generated {
                placer,
                tally,
                attempt,
            });
        }
    }
    None
}

/// Every attempt a kind and a seed make, and how each ended — for the
/// tests that print why candidates are thrown away. Stops at the first
/// that passes, as [`generate`] does.
#[cfg(test)]
pub(crate) fn attempts(kind: StationKind, map_seed: u64) -> Vec<Result<Tally, Fail>> {
    let base = Rng::new(map_seed ^ GEN_SALT);
    let mut out = Vec::new();
    for attempt in 0..ATTEMPTS {
        let mut rng = base.branch(attempt as u64);
        let outcome = attempt_once(kind, map_seed, &mut rng).map(|(_, tally)| tally);
        let done = outcome.is_ok();
        out.push(outcome);
        if done {
            break;
        }
    }
    out
}

/// One candidate, drawn, furnished and checked.
fn attempt_once(kind: StationKind, map_seed: u64, rng: &mut Rng) -> Result<(Placer, Tally), Fail> {
    let (placer, floor, side_on) = candidate(kind, map_seed, rng)?;
    let tally = check(&placer, &floor, kind, side_on)?;
    Ok((placer, tally))
}

/// Attempt `attempt`'s candidate for a kind and a seed, furnished but not
/// checked — for looking at one that failed.
#[cfg(test)]
pub(crate) fn candidate_at(
    kind: StationKind,
    map_seed: u64,
    attempt: u32,
) -> Result<(Placer, Floor, bool), Fail> {
    let mut rng = Rng::new(map_seed ^ GEN_SALT).branch(attempt as u64);
    candidate(kind, map_seed, &mut rng)
}

/// A candidate drawn and furnished, before it is checked.
fn candidate(
    kind: StationKind,
    map_seed: u64,
    rng: &mut Rng,
) -> Result<(Placer, Floor, bool), Fail> {
    // The drawing and the roles are cheap next to a furnishing, so a
    // drawing the wrong size or with no room for a role is drawn again
    // off the same stream, up to `SKETCHES` times, before the attempt is
    // spent.
    let mut why = Fail::Sketch;
    let mut drawn = None;
    for _ in 0..SKETCHES {
        let Some(sketch) = sketch(kind, rng) else {
            continue;
        };
        let side = match side_for(kind, &sketch) {
            Ok(side) => side,
            Err(e) => {
                why = e;
                continue;
            }
        };
        let rooms: Vec<Block> = sketch.rooms.iter().map(|r| r.block()).collect();
        match deal(kind, &rooms, rng) {
            Some(roles) => {
                drawn = Some((sketch, side, rooms, roles));
                break;
            }
            None => why = Fail::Roles,
        }
    }
    let (sketch, side, rooms, roles) = drawn.ok_or(why)?;
    let raster = Raster::new(side, &sketch);
    let port = ((1u32, sketch.py as u32), Rotation::R0);
    let array = array(&raster, port.0, rng).ok_or(Fail::NoArray)?;

    // A trial furnishing with every room shut and the port the one way
    // in: what stands where, so each room's door and every other airlock
    // goes where nothing blocks it.
    let trial_floor = floor_of(&sketch, &roles, &[], &[port], array, &[]);
    let trial = furnish_placer(side, trial_floor, map_seed);
    let locks = airlocks(kind, &raster, &sketch, &trial, array, rng);
    let mut doors = Vec::new();
    for room in &rooms {
        let sites = door_sites(*room, &raster, &trial);
        let first = *rng.pick(&sites).ok_or(Fail::NoDoor)?;
        doors.push(first);
        // Now and then a second, on another wall: a room to go through.
        if one_in(rng, 4) {
            let other: Vec<_> = sites
                .iter()
                .copied()
                .filter(|&((x, y), r)| {
                    let ((fx, fy), fr) = first;
                    r != fr || (r == Rotation::R90 && y != fy) || (r == Rotation::R0 && x != fx)
                })
                .collect();
            if let Some(&second) = rng.pick(&other) {
                doors.push(second);
            }
        }
    }
    // The reactor room's door is kept clear of cover with the rest.
    let mut kept = doors.clone();
    kept.push(((LOBBY_EAST, sketch.py as u32), Rotation::R0));
    let bags = cover(&sketch, &raster, &kept, &locks, rng);
    let floor = floor_of(&sketch, &roles, &doors, &locks, array, &bags);
    let placer = furnish_placer(side, floor.clone(), map_seed);
    Ok((placer, floor, sketch.side_on))
}

/// Why a candidate was thrown away.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fail {
    /// Drawn too small to hold a reactor room's door.
    Sketch,
    /// Drawn bigger than the kind's range of sizes.
    TooBig,
    /// Drawn smaller.
    TooSmall,
    /// Not a room for every role.
    Roles,
    /// A room with nowhere in its walls a door can go.
    NoDoor,
    NoPort,
    PortNotWest,
    NoArray,
    SmallLobby,
    ShortRun,
    DoorBlocked,
    Pinch,
    DiagonalHull,
    CutOff,
    FewBunks,
    FewAirlocks,
    NoLoop,
    Desks,
}

/// The walkability contract and the station's invariants, asked of a
/// furnished floor by a flood stricter than the room's navigation: see the
/// module note. `side_on` is only carried into the tally.
pub(crate) fn check(
    placer: &Placer,
    floor: &Floor,
    kind: StationKind,
    side_on: bool,
) -> Result<Tally, Fail> {
    let design = &placer.design;
    let side = design.build_area as i32;
    let at = |x: i32, y: i32| (y * side + x) as usize;
    let inside = |x: i32, y: i32| x >= 0 && y >= 0 && x < side && y < side;
    let object = |x: i32, y: i32| placer.object_at((x, y));
    let free = |x: i32, y: i32| inside(x, y) && !placer.blocked((x, y));

    // The port: the first airlock, in the west skin.
    let port = shipdesign::port(design).ok_or(Fail::NoPort)?;
    if port.outward != (-1, 0) {
        return Err(Fail::PortNotWest);
    }
    let ((px, py), _) = *floor.airlocks.first().ok_or(Fail::NoPort)?;
    if px != 1 {
        return Err(Fail::PortNotWest);
    }
    let (px, py) = (px as i32, py as i32);

    // The array, in the north skin.
    let (ax, ay) = (floor.array.0 as i32, floor.array.1 as i32);
    if object(ax, ay) != Some(PartKind::SensorArray)
        || placer.get(Layer::Structure, (ax, ay - 1)) != 0
    {
        return Err(Fail::NoArray);
    }

    // The reactor room, as big as the furnisher wants.
    let lobby = floor.lobby;
    if lobby.x1 - lobby.x0 + 1 < LOBBY_MIN.0 || lobby.y1 - lobby.y0 + 1 < LOBBY_MIN.1 {
        return Err(Fail::SmallLobby);
    }

    // A straight run in from the port on both its rows.
    for dx in 1..=MIN_RUN as i32 {
        if !free(px + dx, py) || !free(px + dx, py + 1) {
            return Err(Fail::ShortRun);
        }
    }

    // Every door two deep clear both sides.
    for part in &design.parts {
        if part.kind != PartKind::Door {
            continue;
        }
        let (x, y) = (part.origin.0 as i32, part.origin.1 as i32);
        let upright = matches!(part.rotation, Rotation::R0 | Rotation::R180);
        for i in 0..2 {
            for d in [-2, -1, 1, 2] {
                let (tx, ty) = if upright {
                    (x + d, y + i)
                } else {
                    (x + i, y + d)
                };
                if !free(tx, ty) {
                    return Err(Fail::DoorBlocked);
                }
            }
        }
    }

    // No diagonal pinch between two pieces of structure, and no hull that
    // meets itself only at a corner.
    let structural = |x: i32, y: i32| {
        !free(x, y)
            && matches!(
                object(x, y),
                None | Some(PartKind::Wall | PartKind::OutsideWall | PartKind::SensorArray)
            )
    };
    let hull = |x: i32, y: i32| inside(x, y) && placer.get(Layer::Structure, (x, y)) != 0;
    for y in 0..side - 1 {
        for x in 0..side - 1 {
            let (a, b, c, d) = (
                structural(x, y),
                structural(x + 1, y),
                structural(x, y + 1),
                structural(x + 1, y + 1),
            );
            let (fa, fb, fc, fd) = (
                free(x, y),
                free(x + 1, y),
                free(x, y + 1),
                free(x + 1, y + 1),
            );
            if (a && d && fb && fc) || (b && c && fa && fd) {
                return Err(Fail::Pinch);
            }
            let (ha, hb, hc, hd) = (
                hull(x, y),
                hull(x + 1, y),
                hull(x, y + 1),
                hull(x + 1, y + 1),
            );
            if (ha && hd && !hb && !hc) || (hb && hc && !ha && !hd) {
                return Err(Fail::DiagonalHull);
            }
        }
    }

    // The flood, from the window just inside the port: every doorway and
    // every airlock's two tiles within reach of a body two tiles wide,
    // and every use spot and every open tile of deck within reach from
    // there.
    let n = (side * side) as usize;
    let reach = reach(placer, (px + 1, py)).ok_or(Fail::CutOff)?;
    for part in &design.parts {
        if matches!(part.kind, PartKind::Door | PartKind::Airlock) {
            for (x, y) in part.tiles() {
                if !reach.wide[at(x as i32, y as i32)] {
                    return Err(Fail::CutOff);
                }
            }
        }
        for (sx, sy) in part.use_spots() {
            if !inside(sx, sy) || !reach.any[at(sx, sy)] {
                return Err(Fail::CutOff);
            }
        }
    }
    for y in 0..side {
        for x in 0..side {
            if free(x, y) && !reach.any[at(x, y)] {
                return Err(Fail::CutOff);
            }
        }
    }

    // Beds for everybody who lives here and two mercenaries; the airlocks
    // the kind wants; the two desks.
    if design.count(PartKind::Bunk) < residents_of(kind) + 2 {
        return Err(Fail::FewBunks);
    }
    let locks = design.count(PartKind::Airlock);
    let least = match kind {
        StationKind::Relay => 1,
        StationKind::Orbital => 3,
        _ => 2,
    };
    if locks < least {
        return Err(Fail::FewAirlocks);
    }
    if design.count(PartKind::TradingDesk) != 1 || design.count(PartKind::ResearchDesk) != 1 {
        return Err(Fail::Desks);
    }

    // The loops: pieces of structure (the outside counted as one) that do
    // not touch the outside — deck all the way round each.
    let solid = |x: i32, y: i32| !inside(x, y) || structural(x, y) || !hull(x, y);
    let mut piece = vec![u32::MAX; n];
    let mut pieces = 0u32;
    let mut outside = u32::MAX;
    for y in 0..side {
        for x in 0..side {
            if !solid(x, y) || piece[at(x, y)] != u32::MAX {
                continue;
            }
            let id = pieces;
            pieces += 1;
            let mut touches_edge = false;
            let mut stack = vec![(x, y)];
            piece[at(x, y)] = id;
            while let Some((cx, cy)) = stack.pop() {
                if cx == 0 || cy == 0 || cx == side - 1 || cy == side - 1 {
                    touches_edge = true;
                }
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let (nx, ny) = (cx + dx, cy + dy);
                        if inside(nx, ny) && solid(nx, ny) && piece[at(nx, ny)] == u32::MAX {
                            piece[at(nx, ny)] = id;
                            stack.push((nx, ny));
                        }
                    }
                }
            }
            if touches_edge {
                outside = id;
            }
        }
    }
    let loops = pieces - u32::from(outside != u32::MAX);
    if loops == 0 {
        return Err(Fail::NoLoop);
    }

    let mut hull_tiles = 0u32;
    let mut corridor = 0u32;
    let mut signature = 0x_cbf2_9ce4_8422_2325u64;
    let mut eat = |v: u64| {
        signature ^= v;
        signature = signature.wrapping_mul(0x100_0000_01b3);
    };
    eat(side as u64);
    for y in 0..side {
        for x in 0..side {
            if hull(x, y) {
                hull_tiles += 1;
                eat(at(x, y) as u64);
            }
        }
    }
    for &(x, y) in &floor.walls {
        eat(((x as u64) << 32) | y as u64 | 1 << 62);
    }
    for &((x, y), r) in &floor.doors {
        eat(((x as u64) << 32) | y as u64 | (r as u64) << 60 | 1 << 63);
    }
    // Rooms: the reactor room and every room with a role — which is every
    // room, since a room left over is a store.
    let rooms = 5
        + floor.stores.len() as u32
        + u32::from(floor.lab.is_some())
        + u32::from(floor.rec.is_some());
    let blocks: Vec<Block> = [floor.mess, floor.quarters, floor.heads, floor.research]
        .into_iter()
        .chain(floor.lab)
        .chain(floor.rec)
        .chain(floor.stores.iter().copied())
        .chain([Block::new(
            lobby.x0 - 1,
            lobby.y0 - 1,
            lobby.x1 + 1,
            lobby.y1 + 1,
        )])
        .collect();
    for y in 0..side {
        for x in 0..side {
            let (ux, uy) = (x as u32, y as u32);
            let roomed = blocks
                .iter()
                .any(|b| ux >= b.x0 && ux <= b.x1 && uy >= b.y0 && uy <= b.y1);
            if free(x, y) && !roomed {
                corridor += 1;
            }
        }
    }
    Ok(Tally {
        side: side as u32,
        hull: hull_tiles,
        rooms,
        corridor,
        loops,
        airlocks: locks,
        side_on,
        signature,
    })
}

/// What the contract's flood reached, a flag a tile.
pub(crate) struct Reach {
    /// By a body two tiles wide: every tile of a two-by-two window of free
    /// tiles moved a tile at a time from the start — sandbags counted as in
    /// the way, so no line of them can be what cuts a route — which passes
    /// no gap narrower than two and no diagonal pinch. Doorways and
    /// airlocks must be reached this way.
    pub(crate) wide: Vec<bool>,
    /// From there, one tile at a time, four ways, over free tiles: the gap
    /// a tray run leaves at its end, a nook between bunks. The room's
    /// navigation walks a one-tile gap straight or round a corner (the
    /// grid aboard is phased to the tiles; `a_one_tile_corridor_can_be_
    /// walked`) and never a diagonal one, which a four-way step cannot
    /// take either. Use spots and open deck must be reached this way.
    pub(crate) any: Vec<bool>,
}

/// The flood the contract is checked with, from the window at `start`;
/// `None` when `start` is no window.
pub(crate) fn reach(placer: &Placer, start: (i32, i32)) -> Option<Reach> {
    let side = placer.design.build_area as i32;
    let at = |x: i32, y: i32| (y * side + x) as usize;
    let inside = |x: i32, y: i32| x >= 0 && y >= 0 && x < side && y < side;
    let free = |x: i32, y: i32| inside(x, y) && !placer.blocked((x, y));
    let open = |x: i32, y: i32| free(x, y) && placer.object_at((x, y)) != Some(PartKind::Sandbags);
    let window =
        |x: i32, y: i32| open(x, y) && open(x + 1, y) && open(x, y + 1) && open(x + 1, y + 1);
    let n = (side * side) as usize;
    let mut seen = vec![false; n];
    let mut wide = vec![false; n];
    if !window(start.0, start.1) {
        return None;
    }
    let mut stack = vec![start];
    seen[at(start.0, start.1)] = true;
    while let Some((x, y)) = stack.pop() {
        for (tx, ty) in [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)] {
            wide[at(tx, ty)] = true;
        }
        for (dx, dy) in SIDES {
            let (nx, ny) = (x + dx, y + dy);
            if inside(nx, ny) && !seen[at(nx, ny)] && window(nx, ny) {
                seen[at(nx, ny)] = true;
                stack.push((nx, ny));
            }
        }
    }
    let mut any = wide.clone();
    let mut stack: Vec<(i32, i32)> = (0..side)
        .flat_map(|y| (0..side).map(move |x| (x, y)))
        .filter(|&(x, y)| wide[at(x, y)])
        .collect();
    while let Some((x, y)) = stack.pop() {
        for (dx, dy) in SIDES {
            let (nx, ny) = (x + dx, y + dy);
            if free(nx, ny) && !any[at(nx, ny)] {
                any[at(nx, ny)] = true;
                stack.push((nx, ny));
            }
        }
    }
    Some(Reach { wide, any })
}

/// A furnished station as characters, a row a line: a part by the first
/// letter of its kind (`O` the skin, `#` a wall, `D` a door, `A` an
/// airlock, `s` sandbags), `.` open deck, ` ` void. For the tests and for
/// looking at a candidate that failed.
#[cfg(test)]
pub(crate) fn picture(placer: &Placer, from: Option<(i32, i32)>) -> String {
    let side = placer.design.build_area as i32;
    let reach = from.and_then(|start| reach(placer, start));
    let mut out = String::new();
    for y in 0..side {
        out.push_str(&format!("{y:3} "));
        for x in 0..side {
            let c = match placer.object_at((x, y)) {
                Some(PartKind::OutsideWall) => 'O',
                Some(PartKind::Wall) => '#',
                Some(PartKind::Door) => 'D',
                Some(PartKind::Airlock) => 'A',
                Some(PartKind::Sandbags) => 's',
                Some(kind) => format!("{kind:?}").chars().next().unwrap_or('?'),
                None if placer.get(Layer::Floor, (x, y)) != 0 => {
                    let i = (y * side + x) as usize;
                    if reach.as_ref().is_some_and(|r| !r.any[i]) {
                        '!'
                    } else if reach.as_ref().is_some_and(|r| !r.wide[i]) {
                        ','
                    } else {
                        '.'
                    }
                }
                None if placer.get(Layer::Structure, (x, y)) != 0 => 'x',
                None => ' ',
            };
            out.push(c);
        }
        out.push('\n');
    }
    out
}
