//! The parts' colours, and the room's fixtures drawn off a design.
//!
//! The ship designer's painter lived here until the designer went in
//! October 2026; the game's painters are `hull`, `fittings` and
//! `world_paint`, and they take their colours from [`PART_COLORS`].

use shipdesign::ShipDesign;
use shipdesign::parts::PartKind;

use crate::draw::{Color, DrawList};

/// One colour per [`PartKind`], indexed by discriminant, in `PARTS` order. (The smelter's line stayed a
/// row too long after feature 95 closed the kinds up, and every colour
/// from the workbench on was the kind before's; task 120 took it out with
/// the drug lab's.)
///
/// Index 0 is the deck and index 15 is the frame; both are drawn as tiles
/// rather than as objects, and both are in the table anyway.
pub static PART_COLORS: [Color; 49] = [
    Color::rgb(0.13, 0.15, 0.18), // Floor
    Color::rgb(0.30, 0.34, 0.40), // Wall
    Color::rgb(0.38, 0.86, 0.95), // Door
    Color::rgb(0.85, 0.42, 0.22), // Engine
    Color::rgb(0.52, 0.57, 0.70), // Bunk
    Color::rgb(0.56, 0.63, 0.70), // ColdStore
    Color::rgb(0.52, 0.57, 0.63), // Worktop
    Color::rgb(0.72, 0.40, 0.28), // Hob
    Color::rgb(0.44, 0.56, 0.62), // Dishwasher
    Color::rgb(0.40, 0.45, 0.52), // Table
    Color::rgb(0.30, 0.36, 0.44), // Chair
    Color::rgb(0.76, 0.80, 0.84), // Toilet
    Color::rgb(0.64, 0.72, 0.78), // Basin
    Color::rgb(0.40, 0.66, 0.30), // HydroBay
    Color::rgb(0.50, 0.42, 0.30), // BroomLocker
    Color::rgb(0.22, 0.24, 0.27), // Structure — the frame, darker than deck
    Color::rgb(0.46, 0.52, 0.58), // OutsideWall — hull, paler than a wall
    Color::rgb(0.38, 0.72, 0.86), // Helm
    Color::rgb(0.34, 0.62, 0.52), // LifeSupport
    Color::rgb(0.58, 0.68, 0.74), // Airlock
    Color::rgb(0.70, 0.74, 0.80), // SensorArray
    Color::rgb(0.48, 0.44, 0.36), // Shelf
    Color::rgb(0.60, 0.70, 0.76), // Shower
    Color::rgb(0.82, 0.52, 0.30), // Thruster — the engine's orange, paler
    Color::rgb(0.90, 0.32, 0.18), // HeavyEngine — the engine's orange, deeper
    Color::rgb(0.30, 0.34, 0.40), // DiagonalWall — the wall's grey
    Color::rgb(0.46, 0.52, 0.58), // DiagonalOutsideWall — the hull's
    Color::rgb(0.56, 0.50, 0.38), // Workbench
    Color::rgb(0.78, 0.80, 0.84), // SuitLocker — suit-white
    Color::rgb(0.42, 0.38, 0.44), // Armoury — gunmetal
    Color::rgb(0.62, 0.48, 0.30), // TradingDesk — a wooden counter
    Color::rgb(0.66, 0.60, 0.42), // Sandbags — hessian
    Color::rgb(0.30, 0.52, 0.62), // ResearchDesk — a console's blue-grey
    Color::rgb(0.62, 0.42, 0.86), // Hyperdrive — a violet, the far end of the exhaust
    Color::rgb(0.74, 0.88, 1.0),  // WallLight — a tube's blue-white
    Color::rgb(0.96, 0.90, 0.72), // StandingLight — lamplight, a shade cooler
    Color::rgb(0.42, 0.66, 0.36), // SmallPlant — leaf
    Color::rgb(0.30, 0.56, 0.30), // BigPlant — leaf, deeper
    Color::rgb(0.72, 0.58, 0.32), // Picture — the frame's brass
    Color::rgb(0.42, 0.30, 0.18), // Field — soil, a furrowed brown
    Color::rgb(0.22, 0.44, 0.24), // Tree — a deep leaf green
    Color::rgb(0.50, 0.62, 0.42), // Shrub — sage
    Color::rgb(0.56, 0.52, 0.46), // Boulder — a warm grey
    Color::rgb(0.26, 0.46, 0.66), // Water — a lake
    Color::rgb(0.62, 0.66, 0.70), // Railing — brushed steel
    Color::rgb(0.55, 0.78, 0.90), // Window — glass
    Color::rgb(0.04, 0.05, 0.07), // Pit — the drop
    Color::rgb(0.58, 0.44, 0.26), // Crate — plywood
    Color::rgb(0.78, 0.30, 0.18), // FuelTank — a red drum
];

/// The room's fixtures, as the room draws them, on a design: the room is
/// laid out from it the way it is when the game starts, and asked for the
/// pictures of the fixtures the design has got — what `PICTURES` pins
/// (`tests_survivors`). Only
/// those, because a layout puts every fixture it has not got on the worktop.
pub fn fixtures(design: &ShipDesign) -> DrawList {
    use bims::room::Fixtures;
    let has = |kind: PartKind| design.parts.iter().any(|p| p.kind == kind);
    let wanted = Fixtures {
        counter: has(PartKind::Worktop),
        stove: has(PartKind::Hob),
        fridge: has(PartKind::ColdStore),
        dishwasher: has(PartKind::Dishwasher),
        table: has(PartKind::Table),
        chairs: has(PartKind::Chair),
        locker: has(PartKind::BroomLocker),
        beds: has(PartKind::Bunk),
        bay: has(PartKind::HydroBay),
        toilet: has(PartKind::Toilet),
        basin: has(PartKind::Basin),
        doors: has(PartKind::Door),
    };
    let mut list = DrawList::default();
    if wanted == Fixtures::default() {
        return list;
    }
    let room = bims::room::Room::from_layout(bims::aboard::layout_of(design));
    let mut picture = bims::draw::DrawList::new();
    room.draw_fixtures(&mut picture, wanted);
    list.append(picture.data());
    list
}
