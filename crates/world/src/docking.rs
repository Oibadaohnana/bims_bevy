//! A docked ship and its station as **one grid**, so that one room — one
//! deck, one navigation grid — holds both and the crew can walk through
//! the airlocks from one to the other.
//!
//! The ship stays at its own coordinates, shifted by a whole number of
//! tiles so nothing is negative; the station is turned into the ship's
//! frame by however many quarter turns the berth put between them and
//! shifted likewise. Both are laid down again through `shipdesign::apply`,
//! so the joined design is one the rules admit — and the one tile of open
//! space between the two hulls where the collars meet is decked, which is
//! the whole of the passage.
//!
//! The berth is what makes this a matter of whole tiles: two ports are
//! mated face to face, both faces sit on tile seams, and both outward
//! steps are axis-aligned, so the ship's heading at the berth is a quarter
//! turn and the offset between the grids is an integer.
//!
//! # What is lost in the joining
//!
//! The room has one galley, one heads, one bay and one locker, and takes
//! the **first** of each by id — which is the ship's, since the ship is
//! laid down first. The station's galley is furniture to walk round while
//! the ship is docked. Its residents are **not** in the joined room: they
//! keep a room of their own on the station's design, where that galley is
//! the galley — see `World::join_rooms` — so nothing of theirs is lost;
//! what the crew cannot do is use the station's fixtures from this deck.
//!
//! Since September 2026 that room of their own is the same two hulls the
//! **other way round** ([`join_mirror`]): the station at its own
//! coordinates and the ship turned into its frame, so its people can walk
//! through the passage onto the ship after the crew. The ship's fixtures
//! are furniture to them the way the station's are to the crew.

use flight::angle;
use shipdesign::parts::{Rotation, TILE, covered};
use shipdesign::{Budget, Edit, Money, PartKind, ShipDesign, apply, dock};
use worldgen::math::{DVec2, dvec2};

use crate::station::{Berth, Station};

/// The two designs as one. Named for [`join`]'s way round — the ship
/// first, the station turned in; [`join_mirror`] fills the same fields
/// with the roles swapped, the station first and the ship turned in.
#[derive(Clone, Debug)]
pub struct Joined {
    pub design: ShipDesign,
    /// Where the first design's tile (0, 0) sits in the joined grid, in
    /// tiles: the ship's for `join`, the station's for the mirror.
    pub ship_at: (u32, u32),
    /// Where a point of the *turned* design lands in the joined grid: the
    /// image of its origin, in design units, and its two axes as unit
    /// steps in the joined grid. A point `(x, y)` of it is at `origin + x
    /// * ex + y * ey`. The station's for `join`, the ship's for the mirror.
    pub station_origin: DVec2,
    pub station_ex: DVec2,
    pub station_ey: DVec2,
}

impl Joined {
    /// A ship design point, in the joined grid's units.
    pub fn ship_shift(&self) -> DVec2 {
        dvec2(
            self.ship_at.0 as f64 * TILE as f64,
            self.ship_at.1 as f64 * TILE as f64,
        )
    }

    /// A station design point, in the joined grid's units.
    pub fn from_station(&self, p: DVec2) -> DVec2 {
        self.station_origin
            .add(self.station_ex.scale(p.x))
            .add(self.station_ey.scale(p.y))
    }
}

/// Lay the two down as one grid. `None` when either has no port — there is
/// nothing to join by — or the berth is not a quarter turn, which cannot
/// happen for two axis-aligned ports and is refused rather than rounded.
pub fn join(ship: &ShipDesign, com: DVec2, station: &Station, berth: &Berth) -> Option<Joined> {
    let port = dock::port(ship)?;
    station.port()?;
    let ship_anchor = berth.position.sub(angle::rotate_design(com, berth.heading));
    // A station design point into the ship's design frame: through the
    // system and back.
    let to_ship = |p: DVec2| -> DVec2 {
        let system = station.to_system(p);
        angle::unrotate_design(system.sub(ship_anchor), berth.heading)
    };
    join_frames(ship, &station.design, port, &to_ship)
}

/// The same two as one grid the other way round: the **station** at its
/// own coordinates and the ship turned into the station's frame. What
/// the station's people walk on while the ship is docked, so that they
/// can go aboard it — see `crate::crew::Residents::join`. The `Joined`
/// reads with the roles swapped: `ship_at` is where the *station's*
/// origin sits, and `station_origin`/`_ex`/`_ey` are the *ship's* frame
/// in the grid.
pub fn join_mirror(
    ship: &ShipDesign,
    com: DVec2,
    station: &Station,
    berth: &Berth,
) -> Option<Joined> {
    dock::port(ship)?;
    let port = station.port()?;
    let ship_anchor = berth.position.sub(angle::rotate_design(com, berth.heading));
    // A ship design point into the station's design frame: the inverse of
    // `join`'s, through the system and back.
    let to_station = |p: DVec2| -> DVec2 {
        let system = ship_anchor.add(angle::rotate_design(p, berth.heading));
        station.from_system(system)
    };
    join_frames(&station.design, ship, port, &to_station)
}

/// The one grid out of two designs: `first` at its own coordinates plus a
/// shift, `second` turned into the first's frame by `to_first`, the
/// passage decked beyond `first`'s port, and the first's cargo stocked.
/// `join` and `join_mirror` are the two ways round.
fn join_frames(
    first: &ShipDesign,
    second: &ShipDesign,
    port: dock::Port,
    to_first: &dyn Fn(DVec2) -> DVec2,
) -> Option<Joined> {
    let t = TILE as f64;
    let origin = to_first(DVec2::ZERO);
    let ex = to_first(dvec2(t, 0.0)).sub(origin).scale(1.0 / t);
    let ey = to_first(dvec2(0.0, t)).sub(origin).scale(1.0 / t);
    let axis = |v: DVec2| -> Option<(i32, i32)> {
        let (x, y) = (v.x.round() as i32, v.y.round() as i32);
        ((v.x - x as f64).abs() < 1e-6 && (v.y - y as f64).abs() < 1e-6 && x.abs() + y.abs() == 1)
            .then_some((x, y))
    };
    let (ax, ay) = (axis(ex)?, axis(ey)?);
    // Quarter turns clockwise on a y-down grid: `+x` goes to `+y` on the
    // first, to `-x` on the second, to `-y` on the third.
    let turns = match ax {
        (1, 0) => 0,
        (0, 1) => 1,
        (-1, 0) => 2,
        _ => 3,
    };
    // The origin lands on a tile corner, to rounding.
    let (ox, oy) = ((origin.x / t).round() as i32, (origin.y / t).round() as i32);
    if (origin.x / t - ox as f64).abs() > 1e-6 || (origin.y / t - oy as f64).abs() > 1e-6 {
        return None;
    }
    // A second tile's corner in first tiles. A tile is a unit square whose
    // image is a unit square with a different min corner; the min corner is
    // the least of the four images.
    let corner =
        |x: i32, y: i32| -> (i32, i32) { (ox + x * ax.0 + y * ay.0, oy + x * ax.1 + y * ay.1) };
    let tile = |x: i32, y: i32| -> (i32, i32) {
        let corners = [
            corner(x, y),
            corner(x + 1, y),
            corner(x, y + 1),
            corner(x + 1, y + 1),
        ];
        let mx = corners.iter().map(|c| c.0).min().unwrap();
        let my = corners.iter().map(|c| c.1).min().unwrap();
        (mx, my)
    };

    // The extent of both, in first tiles, and the shift that puts the least
    // corner one tile in from the edge.
    let side = second.build_area as i32;
    let second_corners = [
        tile(0, 0),
        tile(side - 1, 0),
        tile(0, side - 1),
        tile(side - 1, side - 1),
    ];
    let min_x = second_corners.iter().map(|c| c.0).min().unwrap().min(0);
    let min_y = second_corners.iter().map(|c| c.1).min().unwrap().min(0);
    let max_x = second_corners
        .iter()
        .map(|c| c.0)
        .max()
        .unwrap()
        .max(first.build_area as i32 - 1);
    let max_y = second_corners
        .iter()
        .map(|c| c.1)
        .max()
        .unwrap()
        .max(first.build_area as i32 - 1);
    let shift = (1 - min_x, 1 - min_y);
    let area = ((max_x - min_x + 3).max(max_y - min_y + 3)) as u32;

    let budget = Budget::new(Money::MAX);
    let mut design = ShipDesign::new(area);
    let mut put = |kind: PartKind, origin: (i32, i32), rotation: Rotation| {
        if origin.0 < 0 || origin.1 < 0 {
            return;
        }
        if let Ok(next) = apply(
            &design,
            &budget,
            Edit::Place {
                kind,
                origin: (origin.0 as u32, origin.1 as u32),
                rotation,
            },
        ) {
            design = next;
        }
    };

    // The first at its own coordinates plus the shift, so its parts have
    // the lowest ids and its fixtures are the room's.
    for part in &first.parts {
        put(
            part.kind,
            (
                part.origin.0 as i32 + shift.0,
                part.origin.1 as i32 + shift.1,
            ),
            part.rotation,
        );
    }
    // Then the second, turned. A part's tiles are turned one by one and
    // its origin is the least of them; its rotation gains the quarter
    // turns. `covered` is checked against the turned tiles, so a mismatch
    // in the turning arithmetic is a part left out, not a part put down
    // wrong.
    for part in &second.parts {
        let turned: Vec<(i32, i32)> = part
            .tiles()
            .into_iter()
            .map(|(x, y)| tile(x as i32, y as i32))
            .collect();
        let origin = (
            turned.iter().map(|c| c.0).min().unwrap(),
            turned.iter().map(|c| c.1).min().unwrap(),
        );
        let rotation = Rotation::ALL[(part.rotation as usize + turns) % 4];
        let mut expect: Vec<(i32, i32)> = covered(part.kind, rotation)
            .into_iter()
            .map(|(dx, dy)| (origin.0 + dx as i32, origin.1 + dy as i32))
            .collect();
        let mut got = turned.clone();
        expect.sort_unstable();
        got.sort_unstable();
        debug_assert_eq!(expect, got, "{:?} turned {turns} times", part.kind);
        if expect != got {
            continue;
        }
        put(
            part.kind,
            (origin.0 + shift.0, origin.1 + shift.1),
            rotation,
        );
    }
    // The passage: the tile beyond each of the first's airlock tiles, where
    // the two collars meet, decked so a body can cross it.
    let airlock = first.part(port.part_id)?;
    for (x, y) in airlock.tiles() {
        let gap = (
            x as i32 + port.outward.0 + shift.0,
            y as i32 + port.outward.1 + shift.1,
        );
        put(PartKind::Structure, gap, Rotation::R0);
        put(PartKind::Floor, gap, Rotation::R0);
    }
    // And the first's cargo, which is what the room stocks its cold store
    // from.
    for (resource, units) in first.manifest() {
        if let Ok(next) = apply(&design, &budget, Edit::Buy { resource, units }) {
            design = next;
        }
    }

    // Where a second point lands in the joined grid, in design units: the
    // origin's image, shifted, and the axes.
    let station_origin = dvec2((ox + shift.0) as f64 * t, (oy + shift.1) as f64 * t);
    Some(Joined {
        design,
        ship_at: (shift.0 as u32, shift.1 as u32),
        station_origin,
        station_ex: dvec2(ax.0 as f64, ax.1 as f64),
        station_ey: dvec2(ay.0 as f64, ay.1 as f64),
    })
}
