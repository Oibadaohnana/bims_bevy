//! What a planet's town has to be true of: `crate::surface`, the town
//! generator and the wild round it, the roll a surface makes, the field's
//! pace, the daylight over a landed town, and the placer `furnish` builds
//! through. The station tests next door in `tests.rs` walk every station
//! plan; `Plan::Surface` is not on `Plan::ALL`, so the town is walked here,
//! in every biome at three sizes.

use shipdesign::fixture::flyer;
use shipdesign::validate::walkable;
use shipdesign::{PartKind, ShipDesign, design_hash, has_errors, validate};
use worldgen::{BodyKind, GalaxyType, StationKind};

use crate::data;
use crate::fixture::{REFERENCE_MONEY, simulation_world};
use crate::station::{Plan, build_placer, layout, layout_surface, side_of};
use crate::surface::{Biome, GATE_WIDTH, GATE_X0, GUARD_POST, SURFACE_KIND, Surface, surface_id};
use crate::world::World;

fn basic() -> World {
    simulation_world(flyer(2), REFERENCE_MONEY, 2)
}

/// The middle of a tile, in the room's units.
fn middle((x, y): (u32, u32)) -> bims::math::Vec2 {
    let tile = shipdesign::TILE as f32;
    bims::math::vec2((x as f32 + 0.5) * tile, (y as f32 + 0.5) * tile)
}

/// The populations a town is walked at: the smallest, one in the middle
/// that rounds awkwardly everywhere, and the biggest.
const POPULATIONS: [u32; 3] = [5, 17, 30];
const SEEDS: [u64; 2] = [1, 0x_5749_4e44_4f57_0001];

/// How many wild parts a town has round it.
fn wild_count(design: &ShipDesign) -> u32 {
    [
        PartKind::Tree,
        PartKind::Shrub,
        PartKind::Boulder,
        PartKind::Water,
    ]
    .iter()
    .map(|&kind| design.count(kind))
    .sum()
}

/// A town is a place the room can live in, in every biome at every size:
/// the port in its west edge with nothing beyond, a wall on every other
/// tile of the edge but the two gates, the designer's rules
/// finding nothing wrong with it for the people who live there, one
/// trading desk, one research desk, a bunk each and two to spare for
/// mercenaries, a chair each in the hall, a bathhouse for every twelve,
/// fields for every two or bays for every four, standing lights on the
/// ground, no skin at all, at least forty wild parts, the guard's post
/// clear, nothing wild where a Bim has to stand, and the same seed
/// building the same town. And every one of its use spots, the post and
/// every walkable tile of ground can be walked from the pad — the
/// walkability contract, `a_station_s_rooms_can_all_be_walked_from_its_door`
/// again, by `Nav::can_reach` since a route per tile over nine thousand
/// tiles is minutes. The biggest temperate town is what `layout` of the
/// surface plan builds.
#[test]
fn a_town_is_a_place_the_room_can_live_in_and_can_be_walked_in_every_biome() {
    let tile = shipdesign::TILE as f32;
    for biome in Biome::ALL {
        for population in POPULATIONS {
            for seed in SEEDS {
                let name = format!("{biome:?} town of {population} at seed {seed}");
                let design = layout_surface(seed, biome, population);
                assert_eq!(design.build_area, data::SURFACE_SIDE);
                assert_eq!(
                    design_hash(&design),
                    design_hash(&layout_surface(seed, biome, population)),
                    "{name}: the same seed builds the same town"
                );
                let port = shipdesign::port(&design).expect("the town has a pad");
                assert_eq!(port.outward, (-1, 0), "{name}: the pad is at the west edge");
                let issues = validate(&design, population);
                assert!(
                    !has_errors(&issues),
                    "{name}: {:?}",
                    issues.iter().map(|i| i.code).collect::<Vec<_>>()
                );
                let count = |kind| design.count(kind);
                assert_eq!(count(PartKind::ResearchDesk), 1, "{name}");
                assert_eq!(count(PartKind::TradingDesk), 1, "{name}");
                assert!(
                    count(PartKind::Bunk) >= population + 2,
                    "{name}: {} bunks",
                    count(PartKind::Bunk)
                );
                assert!(
                    count(PartKind::Chair) >= population,
                    "{name}: {} chairs",
                    count(PartKind::Chair)
                );
                assert!(
                    count(PartKind::Toilet) >= population.div_ceil(12),
                    "{name}: {} bathhouses",
                    count(PartKind::Toilet)
                );
                assert_eq!(count(PartKind::OutsideWall), 0, "{name}: open ground");
                assert_eq!(count(PartKind::Sandbags), 2, "{name}");
                assert!(
                    count(PartKind::StandingLight) >= 8,
                    "{name}: the ground is lit"
                );
                assert!(
                    count(PartKind::WallLight) >= 40,
                    "{name}: the houses are lit"
                );
                match biome {
                    Biome::Arctic => {
                        assert_eq!(count(PartKind::Field), 0, "{name}: nothing grows outside");
                        assert!(
                            count(PartKind::HydroBay) >= population.div_ceil(4),
                            "{name}: {} bays under glass",
                            count(PartKind::HydroBay)
                        );
                    }
                    _ => assert!(
                        count(PartKind::Field) >= population.div_ceil(2),
                        "{name}: {} strips of field",
                        count(PartKind::Field)
                    ),
                }
                assert!(
                    wild_count(&design) >= 40,
                    "{name}: {} wild",
                    wild_count(&design)
                );
                let grid = design.grid();
                let post = (GUARD_POST.0 as i32, GUARD_POST.1 as i32);
                assert!(walkable(&design, &grid, post), "{name}: the post is clear");

                // The fort: a wall on every outermost tile of the deck
                // but the pad's two and the two gates', the gates open
                // where the west cross street meets the north wall and
                // the south, six tiles wide and walkable.
                let (first, last) = (1i32, data::SURFACE_SIDE as i32 - 2);
                let kind_at = |tile: (i32, i32)| {
                    let id = grid.get(shipdesign::Layer::Object, tile);
                    design.part(id).map(|p| p.kind)
                };
                let is_gate = |x: i32| (GATE_X0 as i32..(GATE_X0 + GATE_WIDTH) as i32).contains(&x);
                let mid = data::SURFACE_SIDE as i32 / 2;
                for i in first..=last {
                    for tile in [(i, first), (i, last), (first, i), (last, i)] {
                        let (x, y) = tile;
                        if (y == first || y == last) && is_gate(x) {
                            assert!(
                                walkable(&design, &grid, tile),
                                "{name}: the gate at {tile:?} is open"
                            );
                        } else if x == first && (y == mid - 1 || y == mid) {
                            assert_eq!(
                                kind_at(tile),
                                Some(PartKind::Airlock),
                                "{name}: the pad at {tile:?}"
                            );
                        } else {
                            assert_eq!(
                                kind_at(tile),
                                Some(PartKind::Wall),
                                "{name}: the wall at {tile:?}"
                            );
                        }
                    }
                }

                // Nothing wild where a Bim has to stand or pass: on or
                // beside a use spot, the post, a standing light, a door's
                // tiles or the two beyond either face.
                let is_wild = |tile: (i32, i32)| {
                    let id = grid.get(shipdesign::Layer::Object, tile);
                    id != 0
                        && design.part(id).is_some_and(|p| {
                            matches!(
                                p.kind,
                                PartKind::Tree
                                    | PartKind::Shrub
                                    | PartKind::Boulder
                                    | PartKind::Water
                            )
                        })
                };
                let wild_near = |tile: (i32, i32), what: &str| {
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            assert!(
                                !is_wild((tile.0 + dx, tile.1 + dy)),
                                "{name}: wild at {:?} beside {what} at {tile:?}",
                                (tile.0 + dx, tile.1 + dy)
                            );
                        }
                    }
                };
                for part in &design.parts {
                    for spot in part.use_spots() {
                        wild_near(spot, "a use spot");
                    }
                    match part.kind {
                        PartKind::StandingLight => {
                            wild_near((part.origin.0 as i32, part.origin.1 as i32), "a light");
                        }
                        PartKind::Door => {
                            let across = matches!(
                                part.rotation,
                                shipdesign::Rotation::R0 | shipdesign::Rotation::R180
                            );
                            for (x, y) in part.tiles() {
                                let (x, y) = (x as i32, y as i32);
                                let (a, b) = if across {
                                    ((x - 1, y), (x + 1, y))
                                } else {
                                    ((x, y - 1), (x, y + 1))
                                };
                                wild_near(a, "a door");
                                wild_near(b, "a door");
                            }
                        }
                        _ => {}
                    }
                }
                wild_near(post, "the post");

                // The contract: from the deck inside the port, a route to
                // every use spot, the post and every walkable tile.
                let room = bims::room::Room::from_layout(bims::aboard::layout_of(&design));
                let nav = bims::nav::Nav::tiled(
                    room.interior,
                    &room.solids(),
                    bims::character::BODY_MARGIN,
                    tile,
                );
                let inside = (
                    (port.centre.0 / tile as f64) as u32 + 1,
                    (port.centre.1 / tile as f64) as u32,
                );
                let from = nav.nearest_free(middle(inside));
                let mut cut_off = Vec::new();
                for part in &design.parts {
                    for spot in part.use_spots() {
                        let spot = (spot.0 as u32, spot.1 as u32);
                        if !nav.can_reach(from, middle(spot)) {
                            cut_off.push((part.kind, spot));
                        }
                    }
                }
                assert!(
                    cut_off.is_empty(),
                    "{name}: no route from the pad to {cut_off:?}"
                );
                assert!(
                    nav.can_reach(from, middle(GUARD_POST)),
                    "{name}: no route from the pad to the guard's post"
                );
                let mut pockets = Vec::new();
                for y in 0..design.build_area as i32 {
                    for x in 0..design.build_area as i32 {
                        if walkable(&design, &grid, (x, y))
                            && !nav.can_reach(from, middle((x as u32, y as u32)))
                        {
                            pockets.push((x, y));
                        }
                    }
                }
                assert!(
                    pockets.is_empty(),
                    "{name}: ground nobody can get to at {pockets:?}"
                );
            }
        }
    }
    // The surface plan through `layout` is the biggest temperate town.
    let seed = SEEDS[1];
    assert_eq!(
        design_hash(&layout(SURFACE_KIND, Plan::Surface, seed)),
        design_hash(&layout_surface(
            seed,
            Biome::Temperate,
            data::SURFACE_POPULATION.1
        ))
    );
    assert_eq!(
        Plan::Surface.residents(SURFACE_KIND),
        data::SURFACE_POPULATION.1
    );
}

/// A surface rolls its biome — arctic on an ice world, desert or
/// temperate on a rocky planet, both turning up over a galaxy — and its
/// population, within `SURFACE_POPULATION`, spanning the range over a
/// galaxy; the same galaxy rolls the same; and its station carries the
/// roll as its residents.
#[test]
fn a_landable_body_rolls_a_biome_and_a_population() {
    let world = basic();
    assert!(!world.surfaces.is_empty());
    for surface in &world.surfaces {
        assert_eq!(
            surface.biome == Biome::Arctic,
            surface.kind == BodyKind::IceWorld
        );
        let (least, most) = data::SURFACE_POPULATION;
        assert!((least..=most).contains(&surface.population));
    }
    let again = Surface::all_of(&world.system, world.galaxy_seed);
    for (a, b) in world.surfaces.iter().zip(&again) {
        assert_eq!((a.biome, a.population), (b.biome, b.population));
    }
    let surface = &world.surfaces[0];
    let station = world.station(surface.id).expect("a station");
    assert_eq!(station.population, surface.population);
    assert_eq!(station.residents(), surface.population);
    assert_eq!(station.id, surface_id(surface.body));

    // Over the reference galaxy: both dry biomes, and the whole range.
    let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (mut desert, mut temperate, mut arctic) = (0u32, 0u32, 0u32);
    let (mut least, mut most) = (u32::MAX, 0u32);
    for star in 0..(galaxy.stars.len() as u32).min(120) {
        let Some(system) = galaxy.system(star) else {
            continue;
        };
        for surface in Surface::all_of(&system, data::DEFAULT_SEED) {
            match surface.biome {
                Biome::Desert => desert += 1,
                Biome::Temperate => temperate += 1,
                Biome::Arctic => arctic += 1,
            }
            least = least.min(surface.population);
            most = most.max(surface.population);
        }
    }
    assert!(
        desert > 5 && temperate > 5 && arctic > 5,
        "{desert} {temperate} {arctic}"
    );
    let share = desert as f64 / (desert + temperate) as f64;
    assert!(
        (0.35..=0.65).contains(&share),
        "{share} of rocky planets are desert"
    );
    assert!(least < 12 && most > 24, "populations {least}..{most}");
}

/// A field is a bay at half pace with nothing to plug in: the same tray
/// planted in both and grown the same minutes is half as far along in
/// the field, and cutting the power stops the bay and not the field.
#[test]
fn a_field_grows_at_half_pace_and_ignores_the_plug() {
    use bims::hydro::{Bay, Crop, FIELD_PACE, Job};
    use bims::math::{Rect, vec2};
    let frame = Rect::from_min_size(vec2(100.0, 100.0), vec2(312.0, 52.0));
    let mut bay = Bay::at(frame, vec2(0.0, -1.0));
    let mut field = Bay::field(frame, vec2(0.0, -1.0));
    assert!(field.is_field() && !bay.is_field());
    for b in [&mut bay, &mut field] {
        b.force(Some(Crop::Veg));
        assert!(matches!(
            b.wants_work(0, 0, 0),
            Some(Job::Plant(0, Crop::Veg))
        ));
        b.work(Job::Plant(0, Crop::Veg));
        b.update(1.0, 240.0, 0, 0, 0, (0, 0, 0));
    }
    let (in_bay, in_field) = (bay.growth_at(0), field.growth_at(0));
    assert!(in_bay > 0.0);
    assert!(
        (in_field - in_bay * FIELD_PACE).abs() < 1e-4,
        "{in_field} against {in_bay}"
    );
    // The plug.
    bay.set_powered(false);
    field.set_powered(false);
    assert!(!bay.powered() && field.powered());
    for b in [&mut bay, &mut field] {
        b.update(1.0, 240.0, 0, 0, 0, (0, 0, 0));
    }
    assert_eq!(bay.growth_at(0), in_bay, "the bay stopped");
    assert!(field.growth_at(0) > in_field, "the field went on");
}

/// Landed, the town's ground is under the sky: a crew member on the pad
/// sees a tile of the main street far beyond any lamp's reach and beyond
/// the ten tiles a body makes out in the dark, and does not once the
/// daylight is taken off the joined deck.
#[test]
fn a_settlement_s_ground_is_lit_by_day() {
    use worldgen::math::dvec2;
    let tile = shipdesign::TILE as f64;
    let mut world = basic();
    assert!(world.land_for_probe(), "somewhere to land");
    let id = world.ship.state.alongside().expect("landed");
    let station = world.station(id).expect("the town").clone();
    assert!(world.aboard.is_joined());
    // The eye: the first crew member, stood on the pad's inside tile.
    let inside = dvec2(
        2.5 * tile,
        (data::SURFACE_SIDE / 2) as f64 * tile + 0.5 * tile,
    );
    let eye = world.aboard.from_station(inside).expect("joined");
    world
        .aboard
        .room
        .put_for_probe(0, bims::math::vec2(eye.x as f32, eye.y as f32));
    // A tile of the main street: walkable, further than any lamp reaches
    // with a tile to spare, further than the dark is seen through, and
    // with a straight clear line from the eye.
    let grid = station.design.grid();
    let lights: Vec<((f64, f64), f64)> = station
        .design
        .parts
        .iter()
        .filter_map(|p| {
            shipdesign::light_tiles(p.kind)
                .map(|reach| ((p.origin.0 as f64 + 0.5, p.origin.1 as f64 + 0.5), reach))
        })
        .collect();
    // A line of sight is stopped by what stops sight — a wall, a tree, a
    // fixture — the room's own rule (`PartDef::blocks_sight`), so a lamp
    // behind a wall lights nothing on the street.
    let opaque = |x: f64, y: f64| {
        let tile = (x.floor() as i32, y.floor() as i32);
        if !grid.has_floor(tile) {
            return true;
        }
        let object = grid.get(shipdesign::Layer::Object, tile);
        object != 0
            && station
                .design
                .part(object)
                .is_some_and(|p| p.kind.def().blocks_sight())
    };
    let clear_line = |(ax, ay): (f64, f64), (bx, by): (f64, f64)| {
        let steps = 400;
        (1..steps).all(|i| {
            let t = i as f64 / steps as f64;
            !opaque(ax + (bx - ax) * t, ay + (by - ay) * t)
        })
    };
    let eye_at = (2.5, (data::SURFACE_SIDE / 2) as f64 + 0.5);
    let mut far = None;
    'search: for x in 20..data::SURFACE_SIDE - 2 {
        for y in data::SURFACE_SIDE / 2 - 4..=data::SURFACE_SIDE / 2 + 3 {
            let at = (x, y);
            let (cx, cy) = (x as f64 + 0.5, y as f64 + 0.5);
            // A standing light shines from its tile's middle, as here; a
            // wall light from a little in towards its wall, so a tile of
            // margin covers that.
            let unlit = lights.iter().all(|&((lx, ly), reach)| {
                let margin = if reach > 8.0 { 0.1 } else { 1.0 };
                ((cx - lx).powi(2) + (cy - ly).powi(2)).sqrt() > reach + margin
                    || !clear_line((lx, ly), (cx, cy))
            });
            let beyond = (cx - 2.5) > bims::sight::DARK_RANGE as f64 + 2.0;
            if unlit
                && beyond
                && walkable(&station.design, &grid, (x as i32, y as i32))
                && clear_line(eye_at, (cx, cy))
            {
                far = Some(at);
                break 'search;
            }
        }
    }
    let far = far.expect("a tile of the main street beyond every lamp");
    let p = world
        .aboard
        .from_station(dvec2(
            (far.0 as f64 + 0.5) * tile,
            (far.1 as f64 + 0.5) * tile,
        ))
        .unwrap();
    // The trace is the painter's, before every picture (`Game::observe`);
    // a step alone does not look.
    assert_eq!(world.aboard.room.fog(), bims::sight::Fog::Crew);
    world
        .aboard
        .room
        .put_for_probe(0, bims::math::vec2(eye.x as f32, eye.y as f32));
    world.aboard.room.observe();
    assert!(
        world.aboard.room.seen_at(p.x as f32, p.y as f32),
        "{far:?} is seen by day"
    );
    world.aboard.room.set_daylight(None);
    world.aboard.room.observe();
    assert!(
        !world.aboard.room.seen_at(p.x as f32, p.y as f32),
        "{far:?} is dark without the sky"
    );
    // The residents' own room is under it too, and stays so unjoined.
    let residents = world.residents.as_ref().expect("the town's people");
    assert_eq!(residents.station, id);
    assert!(residents.aboard.count() >= data::SURFACE_POPULATION.0);
}

/// The placer `furnish` builds through answers exactly what a run of
/// `apply`s does — the same refusals, the same ids in the same order —
/// on a hub, a pod and the arena, so every station's hash is what it was
/// before it went in.
#[test]
fn furnish_through_the_placer_is_furnish_through_apply() {
    let seed = 0x_5749_4e44_4f57_0001;
    let builds = [
        (
            StationKind::Orbital,
            Plan::Hub,
            side_of(StationKind::Orbital),
            1,
        ),
        (
            StationKind::Relay,
            Plan::Pod,
            Plan::Pod.side(StationKind::Relay),
            1,
        ),
        (
            StationKind::Orbital,
            Plan::Hub,
            data::ARENA_SIDE,
            data::ARENA_BUNK_COLUMNS,
        ),
    ];
    for (kind, plan, side, columns) in builds {
        let placer = build_placer(kind, plan, side, columns, seed);
        let slow = placer.replay();
        assert_eq!(placer.design, slow, "{kind:?} {plan:?} at {side}");
        assert_eq!(design_hash(&placer.design), design_hash(&slow));
        assert!(placer.attempts.len() > placer.design.parts.len());
    }
}

/// The numbers a town comes out at, for reading: `cargo test -p world
/// town_numbers -- --ignored --nocapture`.
#[test]
#[ignore]
fn town_numbers() {
    for biome in Biome::ALL {
        for population in POPULATIONS {
            let began = std::time::Instant::now();
            let design = layout_surface(SEEDS[1], biome, population);
            let took = began.elapsed();
            let count = |kind| design.count(kind);
            println!(
                "{biome:?} {population}: {} parts, {} bunks, {} chairs, {} fields, {} bays, {} baths, {} houses lit, {} wild ({} trees, {} shrubs, {} boulders, {} water), {} lights, in {took:?}",
                design.parts.len(),
                count(PartKind::Bunk),
                count(PartKind::Chair),
                count(PartKind::Field),
                count(PartKind::HydroBay),
                count(PartKind::Toilet),
                count(PartKind::Door),
                wild_count(&design),
                count(PartKind::Tree),
                count(PartKind::Shrub),
                count(PartKind::Boulder),
                count(PartKind::Water),
                count(PartKind::StandingLight) + count(PartKind::WallLight),
            );
        }
    }
}

/// Where crew member 0 stands, in the room's units.
fn room_pos(world: &World) -> bims::math::Vec2 {
    world.aboard.room.bim_pos(0)
}

/// Landed, the town stands on a plain (feature 55): ground on every side
/// of the ship, a crew member sent two hundred tiles out walks there —
/// on windows, leg by leg, past the deck's grids — and back to the pad,
/// and what it saw of the plain on the way is kept.
#[test]
fn the_ground_beyond_the_town_is_a_plain_the_crew_walk_out_on_and_back() {
    use bims::terrain::{DECK_MARGIN, EXTENT, VIEW};
    use worldgen::math::dvec2;
    let tile = shipdesign::TILE as f64;
    let mut world = basic();
    assert!(world.land_for_probe(), "somewhere to land");
    let plane = world.aboard.room.plane().expect("a plain under the town");
    let (x0, y0, x1, y1) = plane.terrain().extent();
    assert_eq!((x1 - x0, y1 - y0), (EXTENT, EXTENT));
    // The room's box: the deck and the margin round it, no more.
    let interior = world.aboard.room.interior();
    let side = data::SURFACE_SIDE as f32 * tile as f32;
    assert!(interior.width() < side + 2.0 * DECK_MARGIN as f32 * tile as f32 + 60.0 * tile as f32);
    assert!(interior.height() < side + 2.0 * DECK_MARGIN as f32 * tile as f32 + 2.0 * tile as f32);
    // Ground on every side of the ship: the tiles a step west, north and
    // south of the hull are stood on — reached from the pad.
    let ship = &world.ship.design;
    let mut span: Option<(u32, u32, u32, u32)> = None;
    for part in &ship.parts {
        for (x, y) in part.tiles() {
            span = Some(match span {
                None => (x, y, x, y),
                Some((a, b, c, d)) => (a.min(x), b.min(y), c.max(x), d.max(y)),
            });
        }
    }
    let (sx0, sy0, sx1, sy1) = span.expect("a hull");
    let offset = world.aboard.offset;
    let room_tile = |x: i32, y: i32| {
        let p = dvec2((x as f64 + 0.5) * tile, (y as f64 + 0.5) * tile).add(offset);
        bims::math::vec2(p.x as f32, p.y as f32)
    };
    let mid_y = ((sy0 + sy1) / 2) as i32;
    let mid_x = ((sx0 + sx1) / 2) as i32;
    let west = room_tile(sx0 as i32 - 2, mid_y);
    let north = room_tile(mid_x, sy0 as i32 - 2);
    let south = room_tile(mid_x, sy1 as i32 + 2);
    let from = room_pos(&world);
    for (name, at) in [("west", west), ("north", north), ("south", south)] {
        assert!(
            world.aboard.room.can_reach_for_probe(0, at),
            "the ground {name} of the ship is walked to from the pad"
        );
    }
    // Two hundred tiles north of the town, in the station's frame: past
    // the margin, past one window's reach.
    // The nearest open tile to there, since a spot in a cliff is walked
    // to as near as the ground allows and no nearer.
    let terrain = plane.terrain().clone();
    let (tx, ty) = (0..40)
        .flat_map(|d| (-d..=d).flat_map(move |dx| [(dx, -d), (dx, d), (-d, dx), (d, dx)]))
        .map(|(dx, dy)| ((data::SURFACE_SIDE / 2) as i32 + dx, -200 + dy))
        .find(|&(x, y)| terrain.at(x, y) == bims::terrain::Ground::Open)
        .expect("open ground north of the town");
    let far = world
        .aboard
        .from_station(dvec2((tx as f64 + 0.5) * tile, (ty as f64 + 0.5) * tile))
        .expect("joined");
    let far = bims::math::vec2(far.x as f32, far.y as f32);
    assert!(!interior.contains(far));
    // Nothing to eat or see to on the way: the walk is the whole of it.
    world.aboard.room.set_autonomous(false);
    assert!(
        world.aboard.room.walk_to(0, far),
        "a route out onto the plain"
    );
    let mut arrived = false;
    for i in 0..20_000 {
        world.step(&[]);
        let at = room_pos(&world);
        if (at - far).len() < 1.5 * tile as f32 {
            arrived = true;
            break;
        }
    }
    let at = room_pos(&world);
    assert!(arrived, "got to {at:?}, bound for {far:?}");
    assert!(world.aboard.room.is_afield_for_probe(0));
    // And sees the plain round it, out to the view and no further.
    world.aboard.room.observe();
    let plane = world.aboard.room.plane().expect("the plain");
    assert!(plane.seen_at(far, tile as f32));
    let beyond = far + bims::math::vec2(0.0, -(VIEW as f32 + 2.0) * tile as f32);
    assert!(!plane.seen_at(beyond, tile as f32));
    // One look, from wherever it stands — a canyon or a plain.
    assert!(plane.explored_count() > 50, "{}", plane.explored_count());
    // Back to the pad: the way in is the same walk the other way.
    assert!(world.aboard.room.walk_to(0, from));
    let mut home = false;
    for i in 0..20_000 {
        world.step(&[]);
        let at = room_pos(&world);
        if (at - from).len() < 1.5 * tile as f32 {
            home = true;
            break;
        }
    }
    let at = room_pos(&world);
    assert!(home, "got back to {at:?}, bound for {from:?}");
    assert!(!world.aboard.room.is_afield_for_probe(0));
}
