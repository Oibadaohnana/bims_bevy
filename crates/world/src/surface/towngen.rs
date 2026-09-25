//! Generated towns (feature 112): a town's streets, gates, hall and lots
//! drawn from its seed rather than laid on one street template.
//!
//! **Fixed**, the same in every town, since the rest of the world is
//! measured from it: the side ([`data::SURFACE_SIDE`]), the fort's wall on
//! the outermost tiles of the deck, the pad at the middle of the west
//! wall, the yard before it, the watch house south of the pad with the
//! sensor dish and [`GUARD_POST`], and the trading house north of it — the
//! trading hall with the desk, the research room onto the first cross
//! street and the store onto the main street.
//!
//! **Drawn**: the main street's width, east from the pad to the east wall;
//! two or three cross streets north and south across it — the first always
//! beside the trading house and out to the north wall — each running to a
//! wall or to the side street it meets; a side street north of the main
//! street and one south of it, wall to wall, or not; two or three **gates**
//! where a street meets the north, south or east wall; the gathering hall's
//! lot and its place in it; and, along the streets' frontages, the fields
//! or greenhouses first, far from the pad, then the quarters, the
//! bathhouses and the houses nearest the middle — every building placed
//! where it overlaps no street, no other building, no reserved ground and
//! comes no nearer anything than two tiles. The wild goes over the rest,
//! as it always did (`super::wild`).
//!
//! Every street ends at a wall or at another street, so the network has no
//! dead end short of the fort's edge. A candidate is furnished and checked
//! ([`check_trial`] before the wild, [`check_built`] after it) and thrown
//! away if anything fails; `super::layout_town` draws [`ATTEMPTS`] and
//! falls back on the fixed template.

use super::*;

/// How many towns are drawn before the template is fallen back on.
pub(crate) const ATTEMPTS: u32 = 16;

/// The salt a town is drawn with, "TOWNGEN": a stream of its own, so the
/// template's rolls ([`TOWN_SALT`]), the furnisher's and the wild's are
/// what they were.
pub(crate) const TOWNGEN_SALT: u64 = 0x_544f_574e_4745_4e00;

/// The rows the main street starts at: the trading house's store opens
/// onto it.
const MAIN_TOP: u32 = MAIN_Y0;

/// A draw in `lo..=hi`.
fn roll(rng: &mut Rng, lo: u32, hi: u32) -> u32 {
    if hi <= lo {
        return lo;
    }
    lo + rng.below(hi - lo + 1)
}

/// A street, as the tiles it covers, and which way it runs.
#[derive(Clone, Copy)]
struct Street {
    block: Block,
    east_west: bool,
}

/// What may not be built on: a flag a tile.
struct Taken {
    side: u32,
    flags: Vec<bool>,
}

impl Taken {
    fn new(side: u32) -> Taken {
        Taken {
            side,
            flags: vec![false; (side * side) as usize],
        }
    }

    fn mark(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        for y in y0.max(0)..=y1.min(self.side as i32 - 1) {
            for x in x0.max(0)..=x1.min(self.side as i32 - 1) {
                self.flags[(y as u32 * self.side + x as u32) as usize] = true;
            }
        }
    }

    fn free(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
        if x0 < 0 || y0 < 0 || x1 >= self.side as i32 || y1 >= self.side as i32 {
            return false;
        }
        (y0..=y1)
            .all(|y| (x0..=x1).all(|x| !self.flags[(y as u32 * self.side + x as u32) as usize]))
    }
}

/// A row of lots along one side of an east–west street: the row the
/// buildings' street walls stand on and whether the street is south of
/// them.
#[derive(Clone, Copy, Debug)]
struct Edge {
    row: u32,
    street_south: bool,
}

/// Where a building `w` by `h` stands along an edge at column `x`, set
/// back `setback` from the street, if the ground is free: its block.
fn site(taken: &Taken, edge: Edge, x: u32, w: u32, h: u32, setback: u32) -> Option<Block> {
    let (x0, x1) = (x as i32, (x + w - 1) as i32);
    let (y0, y1) = if edge.street_south {
        let y1 = edge.row as i32 - setback as i32;
        (y1 - h as i32 + 1, y1)
    } else {
        let y0 = (edge.row + setback) as i32;
        (y0, y0 + h as i32 - 1)
    };
    // The block, and the setback between it and the street.
    let (s0, s1) = if edge.street_south {
        (y0, edge.row as i32)
    } else {
        (edge.row as i32, y1)
    };
    taken
        .free(x0, s0, x1, s1)
        .then(|| Block::new(x0 as u32, y0 as u32, x1 as u32, y1 as u32))
}

/// Mark a building's block and two tiles round it as taken, so the next
/// stands two tiles off.
fn claim(taken: &mut Taken, block: Block) {
    taken.mark(
        block.x0 as i32 - 2,
        block.y0 as i32 - 2,
        block.x1 as i32 + 2,
        block.y1 as i32 + 2,
    );
}

/// A town drawn: its floor, not yet furnished.
pub(crate) fn draw(biome: Biome, population: u32, rng: &mut Rng) -> Result<Floor, &'static str> {
    let side = data::SURFACE_SIDE;
    let last = LAST;
    let mid = side / 2;
    let mut town = Town::default();
    let mut taken = Taken::new(side);

    // --- the streets -------------------------------------------------------
    let main_w = roll(rng, 6, 8);
    let main = Street {
        block: Block::new(FIRST + 1, MAIN_TOP, last - 1, MAIN_TOP + main_w - 1),
        east_west: true,
    };
    let north = (rng.below(4) != 0).then(|| {
        let y = roll(rng, 12, 20);
        Street {
            block: Block::new(FIRST + 1, y, last - 1, y + roll(rng, 6, 7) - 1),
            east_west: true,
        }
    });
    let south = (rng.below(4) != 0).then(|| {
        let y = roll(rng, 66, 74);
        Street {
            block: Block::new(FIRST + 1, y, last - 1, y + roll(rng, 6, 7) - 1),
            east_west: true,
        }
    });

    // The hall, sized to seat everybody: a chair each, two a table, three
    // rows of tables, two columns at the least for the galley.
    let tables = population.div_ceil(2);
    let columns = tables.div_ceil(HALL_ROWS).max(2);
    let (hall_w, hall_h) = (4 * columns + 2, 4 * HALL_ROWS + 4);

    // The cross streets: the first beside the trading house, the rest
    // east of it with a lot between each, the hall's lot wide enough for
    // it.
    let crossings = roll(rng, 2, 3);
    let hall_lot = rng.below(crossings);
    let mut crosses: Vec<(u32, u32)> = vec![(CROSS_A_X0, roll(rng, 6, 8))];
    for i in 1..crossings {
        let (x, w) = crosses[i as usize - 1];
        let gap = if i - 1 == hall_lot {
            hall_w + roll(rng, 4, 9)
        } else {
            roll(rng, 12, 22)
        };
        crosses.push((x + w + gap, roll(rng, 6, 8)));
    }
    let &(lx, lw) = crosses.last().ok_or("crosses")?;
    let east_room = (last - 1).checked_sub(lx + lw - 1).ok_or("east")?;
    let wanted_east = if hall_lot == crossings - 1 {
        hall_w + 4
    } else {
        12
    };
    if east_room < wanted_east {
        return Err("east");
    }
    // Where each runs: to the north wall or the north street, and to the
    // main street, the south street or the south wall — never both ends
    // at the main street. The first runs to the north wall past the
    // research room's door.
    let mut streets: Vec<Street> = vec![main];
    streets.extend(north);
    streets.extend(south);
    let mut reaches: Vec<(bool, bool)> = Vec::new(); // (north wall, south wall)
    for (i, &(x, w)) in crosses.iter().enumerate() {
        let top = match (i, north, rng.below(3)) {
            (0, _, _) | (_, None, 0) | (_, _, 1) => FIRST + 1,
            (_, Some(n), 0) => n.block.y0,
            (_, _, _) => MAIN_TOP,
        };
        let bottom = match (south, rng.below(3)) {
            (_, 0) => last - 1,
            (Some(s), 1) => s.block.y1,
            _ if top == MAIN_TOP => last - 1,
            _ => main.block.y1,
        };
        streets.push(Street {
            block: Block::new(x, top, x + w - 1, bottom),
            east_west: false,
        });
        reaches.push((top == FIRST + 1, bottom == last - 1));
    }

    // --- the gates ---------------------------------------------------------
    // Every place a street meets the north, south or east wall; two or
    // three of them, on as many walls as there are.
    let mut candidates: Vec<Gate> = Vec::new();
    for (&(x, w), &(to_north, to_south)) in crosses.iter().zip(&reaches) {
        if to_north {
            candidates.push(Gate {
                wall: Wall::North,
                from: x,
                width: w,
            });
        }
        if to_south {
            candidates.push(Gate {
                wall: Wall::South,
                from: x,
                width: w,
            });
        }
    }
    for street in streets.iter().filter(|s| s.east_west) {
        candidates.push(Gate {
            wall: Wall::East,
            from: street.block.y0,
            width: street.block.y1 - street.block.y0 + 1,
        });
    }
    let want = roll(rng, 2, 3) as usize;
    if candidates.len() < 2 {
        return Err("gates");
    }
    let mut gates: Vec<Gate> = Vec::new();
    while gates.len() < want.min(candidates.len()) {
        let fresh: Vec<Gate> = candidates
            .iter()
            .copied()
            .filter(|c| !gates.contains(c) && !gates.iter().any(|g| g.wall == c.wall))
            .collect();
        let pool: Vec<Gate> = if fresh.is_empty() {
            candidates
                .iter()
                .copied()
                .filter(|c| !gates.contains(c))
                .collect()
        } else {
            fresh
        };
        gates.push(*rng.pick(&pool).ok_or("gates")?);
    }
    gates.sort_by_key(|g| (g.wall.order(), g.from));

    // The wall round it, on the deck's outermost tiles, less the pad's two
    // and the gates' openings; the piers inside either side of each gate.
    let pad = [(FIRST, mid - 1), (FIRST, mid)];
    let open = |x: u32, y: u32| gates.iter().any(|g| g.opening().contains(&(x, y)));
    for x in FIRST..=last {
        for y in [FIRST, last] {
            if !open(x, y) {
                town.walls.push((x, y));
            }
        }
    }
    for y in FIRST + 1..last {
        if !pad.contains(&(FIRST, y)) {
            town.walls.push((FIRST, y));
        }
        if !open(last, y) {
            town.walls.push((last, y));
        }
    }
    let mut standing_lights: Vec<(u32, u32)> = Vec::new();
    for gate in &gates {
        town.walls.extend(gate.piers());
        standing_lights.extend(gate.lights());
    }

    // --- what may not be built on ------------------------------------------
    taken.mark(0, 0, side as i32 - 1, FIRST as i32 + 2);
    taken.mark(0, last as i32 - 2, side as i32 - 1, side as i32 - 1);
    taken.mark(0, 0, FIRST as i32 + 2, side as i32 - 1);
    taken.mark(last as i32 - 2, 0, side as i32 - 1, side as i32 - 1);
    for street in &streets {
        let b = street.block;
        taken.mark(b.x0 as i32, b.y0 as i32, b.x1 as i32, b.y1 as i32);
    }
    // The yard, the trading house and the watch house with the post and
    // its sandbags.
    taken.mark(
        FIRST as i32 + 1,
        YARD_Y0 as i32 - 1,
        TOWN_X0 as i32 - 1,
        YARD_Y1 as i32 + 1,
    );
    taken.mark(
        LOBBY.x0 as i32 - 1,
        LOBBY.y0 as i32 - 1,
        RESEARCH.x1 as i32 + 1,
        MAIN_TOP as i32,
    );
    taken.mark(
        FIRST as i32 + 1,
        WATCH_Y0 as i32 - 2,
        (WATCH_X0 + WATCH_SIDE) as i32 + 2,
        (WATCH_Y0 + WATCH_SIDE) as i32 + 2,
    );
    for gate in &gates {
        let (x0, y0, x1, y1) = gate.apron();
        taken.mark(x0, y0, x1, y1);
    }

    // --- the fixed buildings -----------------------------------------------
    let watch = Block::new(
        WATCH_X0,
        WATCH_Y0,
        WATCH_X0 + WATCH_SIDE,
        WATCH_Y0 + WATCH_SIDE,
    );
    town.room(LOBBY, ((LOBBY.x0, LOBBY.y0 + 6), Rotation::R0));
    town.room(RESEARCH, ((RESEARCH.x1, RESEARCH.y0 + 6), Rotation::R0));
    town.room(STORE, ((STORE.x0 + 3, STORE.y1), Rotation::R90));
    town.room(watch, ((watch.x0, watch.y0 + 4), Rotation::R0));

    // --- the hall ----------------------------------------------------------
    // On the main street's north side, in its lot, with its door onto the
    // street.
    let lot_x0 = crosses[hall_lot as usize].0 + crosses[hall_lot as usize].1;
    let lot_x1 = crosses
        .get(hall_lot as usize + 1)
        .map_or(last - 1, |&(x, _)| x - 1);
    let slack = (lot_x1 + 1)
        .checked_sub(lot_x0 + hall_w + 2)
        .ok_or("hall lot")?;
    let hall_x = lot_x0 + roll(rng, 0, slack.min(4));
    let hall = Block::new(hall_x, MAIN_TOP - hall_h, hall_x + hall_w - 1, MAIN_TOP - 1);
    if !taken.free(
        hall.x0 as i32,
        hall.y0 as i32,
        hall.x1 as i32,
        hall.y1 as i32,
    ) {
        return Err("hall");
    }
    town.room(hall, ((hall.x0 + hall_w / 2 - 1, hall.y1), Rotation::R90));
    claim(&mut taken, hall);

    // --- the frontages -----------------------------------------------------
    let edges_of = |s: &Street| {
        [
            Edge {
                row: s.block.y0 - 1,
                street_south: true,
            },
            Edge {
                row: s.block.y1 + 1,
                street_south: false,
            },
        ]
    };
    let mut edges: Vec<Edge> = streets
        .iter()
        .filter(|s| s.east_west)
        .flat_map(edges_of)
        .collect();
    // Nearest the middle first, for the houses; the food takes them in the
    // other order.
    edges.sort_by_key(|e| (e.row as i32 - mid as i32).unsigned_abs());

    // Food, far from the pad: fields of four strips, or greenhouses on an
    // arctic world, from the east end of the farthest frontages.
    let strips_wanted = population.div_ceil(2);
    let bays_wanted = population.div_ceil(4).saturating_sub(1);
    let mut far: Vec<Edge> = edges.clone();
    far.reverse();
    'food: for edge in &far {
        let mut x = last - 3;
        while x > TOWN_X0 + 10 {
            let done = match biome {
                Biome::Arctic => town.bays >= bays_wanted,
                _ => town.strips >= strips_wanted,
            };
            if done {
                break 'food;
            }
            let (w, h) = match biome {
                Biome::Arctic => Building::Greenhouse.size(),
                _ => (9, 13),
            };
            let Some(x0) = x.checked_sub(w - 1) else {
                break;
            };
            match site(&taken, *edge, x0, w, h, 0) {
                Some(block) => {
                    match biome {
                        Biome::Arctic => {
                            town.build(Building::Greenhouse, block, edge.street_south, rng)
                        }
                        _ => {
                            for strip in 0..4 {
                                town.extra.push((
                                    PartKind::Field,
                                    (block.x0 + 1, block.y0 + 2 + 3 * strip),
                                    Rotation::R0,
                                ));
                            }
                            town.strips += 4;
                            town.clear.push(block);
                        }
                    }
                    claim(&mut taken, block);
                    x = block.x0.saturating_sub(3 + rng.below(2));
                }
                None => x -= 1,
            }
        }
    }
    let food = match biome {
        Biome::Arctic => town.bays >= bays_wanted,
        _ => town.strips >= strips_wanted,
    };
    if !food {
        return Err("food");
    }

    // The quarters, the heads, the other bathhouses and the houses, along
    // the frontages from the west, nearest the middle first.
    let mut quarters: Option<Block> = None;
    let mut heads: Option<Block> = None;
    let mut baths = population.div_ceil(12).saturating_sub(1);
    let bunks_wanted = population + 2;
    for edge in &edges {
        let mut x = FIRST + 3;
        while x + 8 < last - 2 {
            let want = if quarters.is_none() {
                Building::House {
                    columns: 2,
                    bunks: 2,
                }
            } else if heads.is_none() || baths > 0 {
                Building::Bath
            } else if town.bunks < bunks_wanted {
                town.pick_house(population, rng)
            } else {
                break;
            };
            let setback = if want == Building::Bath || quarters.is_none() {
                0
            } else {
                rng.below(2)
            };
            // The house wanted, or the next smaller that fits here.
            let mut kind = want;
            let placed = loop {
                let (w, h) = kind.size();
                if let Some(block) = site(&taken, *edge, x, w, h, setback) {
                    break Some((kind, block));
                }
                match kind.smaller() {
                    Some(smaller) if quarters.is_some() => kind = smaller,
                    _ => break None,
                }
            };
            let Some((kind, block)) = placed else {
                x += 1;
                continue;
            };
            let door_y = if edge.street_south {
                block.y1
            } else {
                block.y0
            };
            if quarters.is_none() {
                town.room(block, ((block.x0 + 2, door_y), Rotation::R90));
                town.bunks += 4;
                quarters = Some(block);
            } else if heads.is_none() {
                town.room(block, ((block.x0 + 1, door_y), Rotation::R90));
                heads = Some(block);
            } else {
                town.build(kind, block, edge.street_south, rng);
                if kind == Building::Bath {
                    baths -= 1;
                }
            }
            claim(&mut taken, block);
            x = block.x1 + 3 + rng.below(3);
        }
        if quarters.is_some() && heads.is_some() && baths == 0 && town.bunks >= bunks_wanted {
            break;
        }
    }
    let (quarters, heads) = (quarters.ok_or("quarters")?, heads.ok_or("heads")?);
    if baths > 0 || town.bunks < bunks_wanted {
        return Err("houses");
    }

    // --- the streets, lit and kept clear -----------------------------------
    // A standing light every fourteen to eighteen tiles down each street,
    // two in from alternate edges, none near a door, a gate or the pad.
    let doors = town.doors.clone();
    let near_door = |x: u32, y: u32| {
        doors.iter().any(|&((dx, dy), _)| {
            (dx as i32 - x as i32).abs() <= 3 && (dy as i32 - y as i32).abs() <= 3
        })
    };
    let near_gate = |x: u32, y: u32| {
        gates.iter().any(|g| {
            let (x0, y0, x1, y1) = g.apron();
            (x as i32) >= x0 - 1
                && (x as i32) <= x1 + 1
                && (y as i32) >= y0 - 1
                && (y as i32) <= y1 + 1
        })
    };
    for street in &streets {
        let b = street.block;
        let (from, to) = if street.east_west {
            (b.x0.max(14), b.x1.saturating_sub(4))
        } else {
            (b.y0 + 4, b.y1.saturating_sub(4))
        };
        let mut at = from + roll(rng, 0, 6);
        let mut high = rng.below(2) == 0;
        while at <= to {
            let tile = match (street.east_west, high) {
                (true, true) => (at, b.y0 + 2),
                (true, false) => (at, b.y1 - 2),
                (false, true) => (b.x0 + 2, at),
                (false, false) => (b.x1 - 2, at),
            };
            // Not where a street crosses, which is open ground both ways.
            let crossing = streets.iter().any(|o| {
                !std::ptr::eq(o, street)
                    && tile.0 + 1 >= o.block.x0
                    && tile.0 <= o.block.x1 + 1
                    && tile.1 + 1 >= o.block.y0
                    && tile.1 <= o.block.y1 + 1
            });
            if !crossing && !near_door(tile.0, tile.1) && !near_gate(tile.0, tile.1) {
                standing_lights.push(tile);
            }
            at += roll(rng, 14, 18);
            high = !high;
        }
    }
    standing_lights.push((4, YARD_Y0 + 5));
    standing_lights.push((4, YARD_Y1 + 1));
    for street in &streets {
        let mut b = street.block;
        // Through the gate a street runs out by, into the wall's row.
        for gate in &gates {
            let meets = match gate.wall {
                Wall::North => !street.east_west && gate.from == b.x0 && b.y0 == FIRST + 1,
                Wall::South => !street.east_west && gate.from == b.x0 && b.y1 == last - 1,
                Wall::East => street.east_west && gate.from == b.y0,
            };
            if meets {
                match gate.wall {
                    Wall::North => b.y0 = FIRST,
                    Wall::South => b.y1 = last,
                    Wall::East => b.x1 = last,
                }
            }
        }
        town.clear.push(b);
    }
    town.clear
        .push(Block::new(FIRST + 1, YARD_Y0, TOWN_X0 - 1, YARD_Y1));
    // And every gate's apron — its piers, its lights and the street's
    // mouth — so the lights are on ground joined to the street and the
    // wild can wall none of it off.
    for gate in &gates {
        let (x0, y0, x1, y1) = gate.apron();
        town.clear
            .push(Block::new(x0 as u32, y0 as u32, x1 as u32, y1 as u32));
    }

    Ok(Floor {
        hull: vec![Block::new(FIRST, FIRST, last, last)],
        airlocks: vec![((FIRST, mid - 1), Rotation::R0)],
        array: (watch.x0 + 4, watch.y0),
        lobby: LOBBY.inner(),
        walls: town.walls,
        doors: town.doors,
        cover: vec![
            (GUARD_POST.0 - 1, GUARD_POST.1 - 2),
            (GUARD_POST.0 - 1, GUARD_POST.1 + 3),
        ],
        mess: hall,
        quarters,
        heads,
        research: RESEARCH,
        lab: None,
        rec: None,
        stores: vec![STORE],
        bunk_columns: 2,
        lit: town.lit,
        hall: (hall.x0 + hall_w / 2, MAIN_TOP + 3),
        open: true,
        standing_lights,
        extra: town.extra,
        mess_columns: columns,
        wild: Some(biome),
        clear: town.clear,
        gates,
    })
}

/// The four ways out of a tile.
const SIDES: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

/// Why a drawn town was thrown away.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TownFail {
    FewChairs,
    FewBunks,
    FewFood,
    Unreached,
    DoorShut,
    GateShut,
    /// The wild grown up beside something a Bim has to stand at or pass.
    Overgrown,
}

/// A town furnished without its wild, checked the stations' way
/// (`crate::stationgen::reach`): every doorway and every gate's opening
/// within reach of a body two tiles wide from the pad, and every use spot
/// and the post within reach from there. The wild keeps off every
/// one of those, the streets and the ground before a door, so what it
/// adds cannot cut a route this found. And the counts: a chair and a bed
/// each, two beds over, and the food.
pub(crate) fn check_trial(
    placer: &Placer,
    floor: &Floor,
    population: u32,
    biome: Biome,
) -> Result<(), TownFail> {
    let design = &placer.design;
    if design.count(PartKind::Chair) < population {
        return Err(TownFail::FewChairs);
    }
    if design.count(PartKind::Bunk) < population + 2 {
        return Err(TownFail::FewBunks);
    }
    let food = match biome {
        Biome::Arctic => design.count(PartKind::HydroBay) >= population.div_ceil(4),
        _ => design.count(PartKind::Field) >= population.div_ceil(2),
    };
    if !food {
        return Err(TownFail::FewFood);
    }
    let side = design.build_area as i32;
    let at = |x: i32, y: i32| (y * side + x) as usize;
    let inside = |x: i32, y: i32| x >= 0 && y >= 0 && x < side && y < side;
    let mid = data::SURFACE_SIDE as i32 / 2;
    let reach =
        crate::stationgen::reach(placer, (FIRST as i32 + 1, mid - 1)).ok_or(TownFail::Unreached)?;
    for part in &design.parts {
        for (sx, sy) in part.use_spots() {
            if !inside(sx, sy) || !reach.any[at(sx, sy)] {
                return Err(TownFail::Unreached);
            }
        }
        if part.kind == PartKind::Door {
            for (x, y) in part.tiles() {
                if !reach.wide[at(x as i32, y as i32)] {
                    return Err(TownFail::DoorShut);
                }
            }
        }
    }
    if !reach.any[at(GUARD_POST.0 as i32, GUARD_POST.1 as i32)] {
        return Err(TownFail::Unreached);
    }
    for gate in &floor.gates {
        for (x, y) in gate.opening() {
            if !reach.wide[at(x as i32, y as i32)] {
                return Err(TownFail::GateShut);
            }
        }
    }
    Ok(())
}

/// The town with its wild, checked the way the wild's own flood checks
/// it: every free tile reached four ways from the pad, and every gate's
/// opening with it — and nothing wild on or beside a use spot, a standing
/// light, the post, a door's tiles or the tile before either face of one,
/// which the wild keeps off unless the pockets it fills afterwards are
/// round one.
pub(crate) fn check_built(placer: &Placer, floor: &Floor) -> Result<(), TownFail> {
    let wild = |x: i32, y: i32| {
        matches!(
            placer.object_at((x, y)),
            Some(PartKind::Tree | PartKind::Shrub | PartKind::Boulder | PartKind::Water)
        )
    };
    let wild_near = |x: i32, y: i32| (-1..=1).any(|dy| (-1..=1).any(|dx| wild(x + dx, y + dy)));
    for part in &placer.design.parts {
        if part.use_spots().iter().any(|&(x, y)| wild_near(x, y)) {
            return Err(TownFail::Overgrown);
        }
        match part.kind {
            PartKind::StandingLight if wild_near(part.origin.0 as i32, part.origin.1 as i32) => {
                return Err(TownFail::Overgrown);
            }
            PartKind::Door => {
                let across = matches!(part.rotation, Rotation::R0 | Rotation::R180);
                for (x, y) in part.tiles() {
                    let (x, y) = (x as i32, y as i32);
                    let (a, b) = if across {
                        ((x - 1, y), (x + 1, y))
                    } else {
                        ((x, y - 1), (x, y + 1))
                    };
                    if wild_near(a.0, a.1) || wild_near(b.0, b.1) {
                        return Err(TownFail::Overgrown);
                    }
                }
            }
            _ => {}
        }
    }
    if wild_near(GUARD_POST.0 as i32, GUARD_POST.1 as i32) {
        return Err(TownFail::Overgrown);
    }
    let side = placer.design.build_area as i32;
    let at = |x: i32, y: i32| (y * side + x) as usize;
    let inside = |x: i32, y: i32| x >= 0 && y >= 0 && x < side && y < side;
    let free = |x: i32, y: i32| inside(x, y) && !placer.blocked((x, y));
    let mid = data::SURFACE_SIDE as i32 / 2;
    let start = (FIRST as i32 + 1, mid - 1);
    let mut reached = vec![false; (side * side) as usize];
    reached[at(start.0, start.1)] = true;
    let mut stack = vec![start];
    while let Some((x, y)) = stack.pop() {
        for (dx, dy) in SIDES {
            let (nx, ny) = (x + dx, y + dy);
            if free(nx, ny) && !reached[at(nx, ny)] {
                reached[at(nx, ny)] = true;
                stack.push((nx, ny));
            }
        }
    }
    for y in 0..side {
        for x in 0..side {
            if free(x, y) && !reached[at(x, y)] {
                return Err(TownFail::Unreached);
            }
        }
    }
    for gate in &floor.gates {
        if gate
            .opening()
            .iter()
            .any(|&(x, y)| !reached[at(x as i32, y as i32)])
        {
            return Err(TownFail::GateShut);
        }
    }
    Ok(())
}
