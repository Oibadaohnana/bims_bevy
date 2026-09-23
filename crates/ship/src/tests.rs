//! The geometry of the game view.
//!
//! **An exception to "the cdylibs' tests are the probes and the harnesses",
//! and a narrow one.** Almost everything in this crate is browser-shaped —
//! a pointer, a ghost, a shape buffer — and is checked by
//! `scratchpad/ship-check.mjs` running the real page against the real wasm.
//! What is below is not: it is the arithmetic that turns a design tile into a
//! place on screen and back, it is pure, and getting it wrong looks like a
//! ship you cannot click on rather than like an error. That is worth a unit
//! test, so the crate is an `rlib` as well as a `cdylib` and
//! `nix flake check` runs this with the rest.

use flight::angle;
use shipdesign::fixture::flyer;
use shipdesign::parts::TILE;
use worldgen::GalaxyType;
use worldgen::math::dvec2;

use crate::game::{Game, ViewMode};
use crate::world_paint;

const CANVAS: (f32, f32) = (960.0, 640.0);

fn game() -> Game {
    let galaxy = worldgen::Galaxy::new(world::data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (star, station) = world::spawn(&galaxy).expect("the default seed has a dock");
    let mut game = Game::start(
        flyer(2),
        40_000,
        2,
        0,
        world::data::DEFAULT_SEED,
        GalaxyType::SpiralTwoArm,
        star,
        station,
        CANVAS.0,
        CANVAS.1,
    )
    .expect("the fixture should open a world");
    // The game opens head up (feature 65); these tests are written north
    // up, the ship turned to its heading, and the ones about head up say so.
    game.head_up = false;
    game
}

/// Where a design tile's middle lands on the canvas, the way the painter puts
/// it there. Written out here rather than called, so the test is comparing the
/// painter's arithmetic against a statement of it rather than against itself.
fn on_canvas(game: &Game, tile: (u32, u32)) -> (f32, f32) {
    let centre = game.world.ship.dynamics.centre_of_mass;
    let middle = dvec2(
        (tile.0 as f64 + 0.5) * TILE as f64,
        (tile.1 as f64 + 0.5) * TILE as f64,
    );
    let d = middle.sub(centre);
    let h = game.ship_turn();
    let (s, c) = (h.sin(), h.cos());
    let (vx, vy) = ((d.x * c - d.y * s) as f32, (d.x * s + d.y * c) as f32);
    let camera = &game.ship_view;
    (
        camera.offset_x() + vx * camera.scale(),
        camera.offset_y() + vy * camera.scale(),
    )
}

/// At rest the ship is drawn exactly as it was laid out: the grid's up is up,
/// and its right is right.
/// Turned a quarter, the nose points at the right-hand edge. That is the whole
/// of what "Forward is the grid's up" means once the ship is moving.
#[test]
fn the_design_is_drawn_as_built_and_turns_with_the_heading() {
    // --- at_a_heading_of_nothing_the_design_is_the_way_it_was_built ---
    {
        let game = game();
        assert_eq!(game.world.ship.heading, 0.0);

        let middle = game.world.ship.design.build_area / 2;
        let here = on_canvas(&game, (middle, middle));
        let above = on_canvas(&game, (middle, middle - 1));
        let right = on_canvas(&game, (middle + 1, middle));

        assert!(above.1 < here.1, "the grid's up should be up the screen");
        assert!((above.0 - here.0).abs() < 1e-3, "and straight up");
        assert!(right.0 > here.0, "the grid's right should be right");
        assert!((right.1 - here.1).abs() < 1e-3, "and straight across");
    }

    // --- at_a_quarter_turn_the_nose_points_to_the_right ---
    {
        let mut game = game();
        game.world.ship.heading = std::f64::consts::FRAC_PI_2;

        let middle = game.world.ship.design.build_area / 2;
        let here = on_canvas(&game, (middle, middle));
        let forward = on_canvas(&game, (middle, middle - 1));

        assert!(forward.0 > here.0, "Forward should be to the right");
        assert!(
            (forward.1 - here.1).abs() < 1e-3,
            "and level with where it started",
        );

        // And the other three quarters, so the sense of the turn is pinned as
        // clockwise rather than merely as "a turn". `here` is recomputed each
        // time: the tile being measured from is not the centre of mass, so it
        // swings round with everything else.
        game.world.ship.heading = std::f64::consts::PI;
        let here = on_canvas(&game, (middle, middle));
        let forward = on_canvas(&game, (middle, middle - 1));
        assert!(
            forward.1 > here.1,
            "half a turn puts the nose down the screen"
        );

        game.world.ship.heading = 3.0 * std::f64::consts::FRAC_PI_2;
        let here = on_canvas(&game, (middle, middle));
        let forward = on_canvas(&game, (middle, middle - 1));
        assert!(forward.0 < here.0, "three quarters puts it to the left");
    }
}

/// A point on the canvas over a known tile has to come back as that tile, at
/// every heading, north up and head up. This is the one that a wrong sign in
/// the inverse turn breaks — and it breaks it silently, as a ship you cannot
/// click on once it has turned.
/// The inverse is the forward turn read backwards and nothing else, which is
/// what stops the two drifting apart.
#[test]
fn a_screen_point_maps_back_to_its_tile_by_the_painters_arithmetic_backwards() {
    // --- a_screen_point_maps_back_to_the_tile_it_is_over ---
    {
        let mut game = game();
        let quarter = std::f64::consts::FRAC_PI_2;
        for head_up in [false, true] {
            game.head_up = head_up;
            for &heading in &[0.0, quarter, std::f64::consts::PI, 3.0 * quarter, 0.7] {
                game.world.ship.heading = heading;
                for tile in [(2u32, 2u32), (9, 3), (17, 17), (10, 10)] {
                    let (x, y) = on_canvas(&game, tile);
                    let back = game.tile_at(x, y);
                    assert_eq!(
                        back,
                        (tile.0 as i32, tile.1 as i32),
                        "head up {head_up}, heading {heading}, tile {tile:?} came back as {back:?}",
                    );
                }
            }
        }
    }

    // --- the_pointer_arithmetic_is_the_painters_arithmetic_backwards ---
    {
        for &heading in &[0.3, 1.9, -2.6] {
            let offset = dvec2(120.0, -75.0);
            let there = angle::rotate_design(offset, heading);
            let back = angle::unrotate_design(there, heading);
            assert!((back.x - offset.x).abs() < 1e-9);
            assert!((back.y - offset.y).abs() < 1e-9);
        }
    }
}

/// Head up, the ship is drawn the way it was laid out whatever its heading,
/// and it is the sky that turns — by the heading undone — so a tile is where
/// it was at rest and a speck has moved.
/// Head up, the map turns round the ship by the heading undone and the marker
/// for the ship stands straight up — and a click still lands on what it looks
/// like it landed on, which is the half that would fail quietly.
#[test]
fn head_up_holds_the_ship_square_and_turns_the_sky_the_map_and_the_pointer() {
    // --- head_up_holds_the_ship_square_and_turns_the_sky ---
    {
        let mut game = game();
        game.head_up = true;
        game.world.ship.heading = 1.1;
        let mut list = crate::draw::DrawList::new();
        world_paint::paint(&game, &mut list);
        let angles = shape_rotations(&list);

        assert!(
            !angles.iter().any(|a| (a - 1.1).abs() < 1e-6),
            "nothing should be turned to the heading when the view is head up",
        );
        assert!(
            angles.iter().any(|a| (a + 1.1).abs() < 1e-6),
            "the sky should be turned by the heading undone",
        );
        // The tiles land exactly where they do at rest — the same picture the
        // designer drew, in the middle of the window.
        for tile in [(2u32, 2u32), (17, 17)] {
            let turned = on_canvas(&game, tile);
            game.world.ship.heading = 0.0;
            let at_rest = on_canvas(&game, tile);
            game.world.ship.heading = 1.1;
            assert!((turned.0 - at_rest.0).abs() < 1e-3 && (turned.1 - at_rest.1).abs() < 1e-3);
        }
        // And a name over a Bim's head follows the same arithmetic, so it stays
        // over the Bim rather than turning off to where the sky went.
        let (cx, cy) = world_paint::crew_on_screen(&game, 0);
        game.world.ship.heading = 0.0;
        let (rx, ry) = world_paint::crew_on_screen(&game, 0);
        assert!((cx - rx).abs() < 1e-3 && (cy - ry).abs() < 1e-3);
    }

    // --- head_up_turns_the_map_and_the_pointer_follows ---
    {
        let mut game = game();
        game.set_mode(ViewMode::Map);
        game.head_up = true;
        game.world.ship.heading = 2.4;

        let mut list = crate::draw::DrawList::new();
        world_paint::paint(&game, &mut list);
        let angles: Vec<f32> = shape_rotations(&list)
            .into_iter()
            .filter(|a| a.abs() > 1e-6)
            .collect();
        assert!(
            !angles.iter().any(|a| (a - 2.4).abs() < 1e-6),
            "the marker should stand straight up rather than to the heading",
        );
        assert!(
            angles.iter().any(|a| (a + 2.4).abs() < 1e-6),
            "the system should be turned by the heading undone",
        );

        // Somewhere out on the map. Where the painter puts it is the offset from
        // the ship with the y flipped and then the camera's turn applied, and a
        // click there has to pick it and a point read there has to be it.
        let here = game.world.ship.position();
        let camera = &game.map_view;
        let (node, at) = game
            .world
            .discovered
            .iter()
            .filter_map(|&n| game.world.system.absolute_position(n).map(|p| (n, p)))
            .find(|(_, p)| p.distance(here) > 0.0)
            .expect("the spawn system is charted, so there is something out there");
        let offset = at.sub(here);
        let (ox, oy) = crate::game::turned(offset.x as f32, -offset.y as f32, -2.4);
        let (sx, sy) = (
            camera.offset_x() + ox * camera.scale(),
            camera.offset_y() + oy * camera.scale(),
        );
        assert_eq!(game.pick(sx, sy, 4.0), Some(node));
        let read = game.point_at(sx, sy);
        let tolerance = 1.0 / camera.scale() as f64;
        assert!(
            read.distance(at) < tolerance,
            "the point under the pointer came back {} away from where it was drawn",
            read.distance(at),
        );
    }
}

/// The starfield and anything drawn because it is out there are **never**
/// turned. Nothing in the shape buffer for them carries a rotation, however
/// the ship is pointing.
/// The sky streams past a ship under way and stands still otherwise. It is
/// a picture clock on the *world's* clock: a step with the ship at rest
/// moves nothing, a frame with no step in it moves nothing however fast
/// the ship is going — a pause holds the stars — and the near layer streams
/// further than the far one, opposite the way the ship is going.
#[test]
fn the_sky_does_not_turn_with_the_ship_and_streams_on_the_clock_under_way() {
    // --- the_sky_and_what_is_alongside_do_not_turn_with_the_ship ---
    {
        let mut game = game();
        let mut list = crate::draw::DrawList::new();

        // Every shape is either where it was at rest — the sky, the station
        // alongside and the void behind them are never turned, and neither
        // are the turns of their own their pictures carry — or turned by
        // exactly the heading on top of what it was at rest: the hull, the
        // fittings (a lamp hung `R270` from the side wall is drawn at its own
        // turn plus the heading), the room aboard's own turned things — a
        // Bim's body, a pot's handle. The same frame at rest is the reference,
        // shape for shape.
        game.world.ship.heading = 1.1;
        world_paint::paint(&game, &mut list);
        let angles = shape_rotations(&list);
        game.world.ship.heading = 0.0;
        let mut at_rest = crate::draw::DrawList::new();
        world_paint::paint(&game, &mut at_rest);
        let fixed = shape_rotations(&at_rest);
        assert_eq!(
            angles.len(),
            fixed.len(),
            "the same frame turned should draw the same shapes"
        );
        let (mut square, mut turned) = (0, 0);
        for (angle, rest) in angles.iter().zip(&fixed) {
            if (angle - rest).abs() < 1e-6 {
                square += 1;
            } else if (angle - rest - 1.1).abs() < 1e-5 {
                turned += 1;
            } else {
                panic!(
                    "a shape at {rest} at rest was drawn at {angle}, which is neither still nor turned with the ship"
                );
            }
        }
        assert!(square > 0, "the starfield should be square to the window");
        assert!(turned > 0, "the ship should be turned to its heading");
    }

    // --- the_sky_streams_on_the_world_s_clock_and_only_under_way ---
    {
        use crate::starfield::Starfield;
        use worldgen::math::dvec2;

        let mut game = game();
        // Docked and still: frames and steps go by and the sky does not move.
        game.stream_sky();
        for _ in 0..10 {
            game.world.step(&[]);
            game.stream_sky();
        }
        assert!(
            game.stars.slid.iter().all(|s| s.x == 0.0 && s.y == 0.0),
            "the sky moved with the ship at rest: {:?}",
            game.stars.slid.map(|s| (s.x, s.y))
        );

        // Under way, to the east, on a clock that moved a minute: the near layer
        // streams west, and further than the far one.
        let mut stars = Starfield::new(1);
        let east = dvec2(3_000.0, 0.0);
        stars.advance(east, 100.0);
        assert!(
            stars.slid.iter().all(|s| s.x == 0.0 && s.y == 0.0),
            "the first frame is a reading, not a stream"
        );
        stars.advance(east, 101.0);
        let near = stars.slid[0];
        let far = stars.slid[2];
        assert!(near.x > 0.0 && near.y == 0.0, "near layer {near:?}");
        // Wrapped to the tile, so "west" reads as a large positive `x`.
        let west_by = |s: worldgen::math::DVec2| crate::starfield::FIELD - s.x;
        assert!(
            west_by(near) > 0.0 && west_by(near) < 100.0,
            "{}",
            west_by(near)
        );
        assert!(
            west_by(far) < west_by(near),
            "the far layer should stream less: {far:?} against {near:?}"
        );
        // The same velocity on a clock that has not moved: a pause.
        let held = stars.slid;
        stars.advance(east, 101.0);
        assert!(
            stars
                .slid
                .iter()
                .zip(held.iter())
                .all(|(a, b)| a.x == b.x && a.y == b.y)
        );
        // Faster is further, and the mapping is monotonic even past the clamp.
        let mut slow = Starfield::new(1);
        slow.advance(dvec2(30.0, 0.0), 0.0);
        slow.advance(dvec2(30.0, 0.0), 1.0);
        assert!(west_by(slow.slid[0]) > 0.0 && west_by(slow.slid[0]) < west_by(near));
        // North on the system's axes is up the screen, so a ship going north
        // has the stars going down: positive `y`, unwrapped.
        let mut north = Starfield::new(1);
        north.advance(dvec2(0.0, 3_000.0), 0.0);
        north.advance(dvec2(0.0, 3_000.0), 1.0);
        assert!(
            north.slid[0].y > 0.0 && north.slid[0].y < 100.0,
            "{:?}",
            (north.slid[0].x, north.slid[0].y)
        );
    }
}

/// The `rot` field of every shape in the buffer.
fn shape_rotations(list: &crate::draw::DrawList) -> Vec<f32> {
    list.shapes()
        .chunks(crate::draw::STRIDE)
        .map(|shape| shape[5])
        .collect()
}

/// The map never turns at all: north is up on it whatever the ship is doing,
/// and the only thing on it that carries a rotation is the marker saying which
/// way the ship is pointing.
/// The camera is centred on the ship, in both views, with or without a pan.
#[test]
fn the_map_is_north_up_and_the_ship_is_in_the_middle_of_both_views() {
    // --- the_map_is_north_up_whatever_the_ship_is_doing ---
    {
        let mut game = game();
        game.set_mode(ViewMode::Map);

        // The pictures of planets and stations carry turns of their own — a gas
        // giant's ring, a belt's rocks — and those are fixed: exactly the same
        // whichever way the ship points. What turns with the ship is the marker
        // for the ship, and nothing else.
        let rotations_at = |game: &mut Game, heading: f64| -> Vec<f32> {
            game.world.ship.heading = heading;
            let mut list = crate::draw::DrawList::new();
            world_paint::paint(game, &mut list);
            shape_rotations(&list)
                .into_iter()
                .filter(|a| a.abs() > 1e-6)
                .collect()
        };
        let at_rest = rotations_at(&mut game, 0.0);
        let turned = rotations_at(&mut game, 2.4);
        let mut moved: Vec<f32> = turned
            .iter()
            .copied()
            .filter(|a| !at_rest.iter().any(|r| (a - r).abs() < 1e-6))
            .collect();
        // The marker is a hull and two fins: three rectangles, at the heading
        // and leaning out either side of it. Nothing else moved.
        moved.sort_by(|a, b| a.total_cmp(b));
        let marker = [
            2.4 - crate::hull::FIN_LEAN,
            2.4,
            2.4 + crate::hull::FIN_LEAN,
        ];
        assert_eq!(
            moved.len(),
            3,
            "only the ship marker should have turned: {moved:?}"
        );
        for (a, b) in moved.iter().zip(marker) {
            assert!(
                (a - b).abs() < 1e-5,
                "the marker turned to {moved:?}, not {marker:?}"
            );
        }
    }

    // --- the_ship_is_in_the_middle_of_both_views ---
    {
        let mut game = game();
        for mode in [ViewMode::Ship, ViewMode::Map] {
            game.set_mode(mode);
            assert!((game.camera().offset_x() - CANVAS.0 / 2.0).abs() < 1e-3);
            assert!((game.camera().offset_y() - CANVAS.1 / 2.0).abs() < 1e-3);
        }

        // And a pan cannot shove it off the edge, however hard it is shoved —
        // once the view is tethered; it opens free, and free goes anywhere.
        game.set_mode(ViewMode::Ship);
        game.set_follow(true);
        game.camera_mut().pan(100_000.0, -100_000.0);
        assert!(game.camera().offset_x() < CANVAS.0);
        assert!(game.camera().offset_y() > 0.0);
    }
}

/// A map click on empty space is a place to fly to; a map click on something
/// is that thing. Both, because the panel offers both and they are different
/// commands.
#[test]
fn a_click_on_the_map_is_a_place_or_a_thing() {
    let mut game = game();
    game.set_mode(ViewMode::Map);

    // The dock the ship is tied to is at the ship's own position, which is the
    // middle of the canvas.
    let picked = game.pick(CANVAS.0 / 2.0, CANVAS.1 / 2.0, 20.0);
    assert!(picked.is_some(), "the station under the ship should pick");

    // Somewhere out in the corner is nothing, and is still somewhere.
    assert!(game.pick(4.0, 4.0, 20.0).is_none());
    let point = game.point_at(4.0, 4.0);
    assert!(point.distance(game.world.ship.position()) > 0.0);
}

/// What is lit follows the plan through every phase: nothing at the dock,
/// the thrusters through the turn, the forward engines through the burn,
/// nothing through the flip, the *same* forward engines through the brake —
/// the flyer has no backward engine, so it turns round to stop — and nothing
/// once it has arrived. And the picture says so: there is exhaust in the
/// buffer exactly when something is lit.
#[test]
fn the_exhaust_follows_the_plan() {
    use flight::{Phase, Target};
    use world::world::Command;

    let mut game = game();
    // Somewhere off to one side, so there is a real turn to make first —
    // and the crew member at the helm, since the ship is flown from there.
    let here = game.world.ship.position();
    game.world.man_the_helm_for_probe(0);
    game.send(Command::Confirm {
        slot: 0,
        target: Target::Point(dvec2(here.x + 30_000.0, here.y + 7_000.0)),
    });

    // The exhaust on its own, rather than the whole frame: the room and the
    // starfield both change from one frame to the next, so a count of
    // everything says nothing about the flame.
    let exhaust = |game: &Game| -> usize {
        let design = &game.world.ship.design;
        let centre = game.world.ship.dynamics.centre_of_mass;
        let mut list = crate::draw::DrawList::default();
        crate::hull::exhaust(
            &mut list,
            design,
            &design.grid(),
            game.firing(),
            (centre.x as f32, centre.y as f32),
            game.frame,
        );
        list.len() / crate::draw::STRIDE
    };
    assert_eq!(game.firing(), crate::hull::Firing::NONE);
    assert_eq!(exhaust(&game), 0, "nothing burns at the dock");

    let mut seen = std::collections::BTreeSet::new();
    let mut under_way = false;
    for _ in 0..400_000 {
        game.step();
        let Some(state) = game.world.trip_state() else {
            // Casting off and pushing off the berth come first, and the
            // engines are cold through both; the trip is over once there
            // has been one.
            assert_eq!(game.firing(), crate::hull::Firing::NONE);
            if under_way {
                break;
            }
            continue;
        };
        under_way = true;
        let firing = game.firing();
        match state.phase {
            Phase::Align => {
                assert!(
                    !firing.forward && !firing.backward,
                    "no engine while aligning"
                );
                assert!(firing.alpha != 0.0, "the thrusters push through a turn");
            }
            Phase::Burn => {
                assert!(
                    firing.forward && !firing.backward,
                    "the forward engines burn"
                );
                assert_eq!(firing.alpha, 0.0);
            }
            Phase::Flip => {
                assert!(!firing.forward && !firing.backward, "a flip is a coast");
            }
            Phase::Brake => {
                assert!(
                    firing.forward && !firing.backward,
                    "the brake after a flip is the forward engines again"
                );
                assert_eq!(firing.alpha, 0.0);
            }
            Phase::Arrived => {}
        }
        if seen.insert(state.phase.code()) {
            // The first frame of each phase: the picture has exhaust in it
            // exactly when something is lit. A flip coasts, so it draws
            // what the dock draws — the ship and nothing behind it.
            let lit = firing.forward || firing.backward || firing.alpha != 0.0;
            let drawn = exhaust(&game);
            assert_eq!(
                drawn > 0,
                lit,
                "phase {:?}: {drawn} shapes of exhaust, lit {lit}",
                state.phase,
            );
        }
    }
    assert!(
        seen.len() >= 4,
        "the trip should have aligned, burnt, flipped and braked: {seen:?}"
    );
    assert!(
        game.world.trip_state().is_none(),
        "the trip should have ended"
    );
    assert_eq!(game.firing(), crate::hull::Firing::NONE);
    assert_eq!(exhaust(&game), 0, "nothing burns once it is there");
}

/// The mated airlocks are a door: shut while nobody is near the passage,
/// open once somebody stands at it, and shut again when they have gone —
/// eased, so the picture never jumps. A picture clock only: the passage is
/// walkable whatever the door looks like.
#[test]
fn the_airlock_opens_for_whoever_comes_to_it_and_shuts_behind_them() {
    let mut game = game();
    assert!(game.mated_airlock().is_some(), "the fixture docks");
    for _ in 0..60 {
        game.tick_airlock();
    }
    assert_eq!(game.airlock_ajar, 0.0, "open with nobody at it");

    // Stand the crew member in the passage: the ship's door face, in the
    // room's units.
    let port = shipdesign::port(&game.world.ship.design).unwrap();
    let (fx, fy) = port.face();
    let offset = game.world.aboard.offset;
    let at = bims::math::vec2((fx + offset.x) as f32, (fy + offset.y) as f32);
    game.world.aboard.room.put_for_probe(0, at);
    let mut opening = Vec::new();
    for _ in 0..40 {
        game.tick_airlock();
        opening.push(game.airlock_ajar);
    }
    assert!(
        opening.windows(2).all(|w| w[1] >= w[0]),
        "it should open, not flicker"
    );
    assert!(
        opening[0] > 0.0 && opening[0] < 1.0,
        "it should ease, not jump"
    );
    assert_eq!(game.airlock_ajar, 1.0, "wide open with somebody at it");

    // And away again: the far end of the ship.
    let far = bims::math::vec2(
        (offset.x + 3.0 * 52.0) as f32,
        (offset.y + 3.0 * 52.0) as f32,
    );
    game.world.aboard.room.put_for_probe(0, far);
    for _ in 0..60 {
        game.tick_airlock();
    }
    assert_eq!(game.airlock_ajar, 0.0, "shut once they have gone");
}

/// The ship view is about the crew member the player steers: wherever they
/// stand — aboard, or across the airlock on the station — they are in the
/// middle of the window, the pan is measured from them and cannot take
/// them off the edge, and a zoom keeps what is under the pointer under it.
#[test]
fn the_camera_follows_the_crew_member_the_player_steers() {
    let mut game = game();
    // The view opens free; this is about it tethered.
    game.set_follow(true);
    let middle = (CANVAS.0 / 2.0, CANVAS.1 / 2.0);
    let pixel = |game: &Game| {
        let (x, y) = world_paint::crew_on_screen(game, 0);
        let cam = &game.ship_view;
        (
            cam.offset_x() + x * cam.scale(),
            cam.offset_y() + y * cam.scale(),
        )
    };

    game.follow_player();
    let at = pixel(&game);
    assert!(
        (at.0 - middle.0).abs() < 1e-3 && (at.1 - middle.1).abs() < 1e-3,
        "{at:?}"
    );

    // Across the airlock: far from the ship's centre, still in the middle.
    let offset = game.world.aboard.offset;
    let side = game.world.ship.design.build_area as f64 * 52.0;
    let far = bims::math::vec2(
        (offset.x + side + 20.0 * 52.0) as f32,
        (offset.y + side / 2.0) as f32,
    );
    let landed = game.world.aboard.room.put_for_probe(0, far);
    assert!(
        (landed - far).len() < 60.0,
        "the probe could not stand there: {landed:?}"
    );
    game.follow_player();
    let at = pixel(&game);
    assert!(
        (at.0 - middle.0).abs() < 1e-3 && (at.1 - middle.1).abs() < 1e-3,
        "{at:?}"
    );
    // And the ship's own centre is off to one side, a long way.
    assert!((game.ship_view.offset_x() - middle.0).abs() > 10.0 * 52.0 * game.ship_view.scale());

    // Zoomed in about a corner of the window, the point under the pointer
    // stays under it; the crew member stays where the pan left them.
    let corner = (100.0, 80.0);
    let before = game.ship_view.to_view(corner.0, corner.1);
    game.ship_view.zoom(corner.0, corner.1, 2.0);
    let after = game.ship_view.to_view(corner.0, corner.1);
    assert!((before.0 - after.0).abs() < 1e-3 && (before.1 - after.1).abs() < 1e-3);

    // Panned as far as it goes, they are still a sliver inside the edge.
    game.ship_view.pan(-100_000.0, -100_000.0);
    game.follow_player();
    let at = pixel(&game);
    assert!(at.0 > 0.0 && at.1 > 0.0, "panned off the edge: {at:?}");

    // Let go, the view stays exactly where it was — nothing on screen
    // moves for the flip of a switch.
    let held = (game.ship_view.offset_x(), game.ship_view.offset_y());
    game.set_follow(false);
    game.follow_player();
    let loose = (game.ship_view.offset_x(), game.ship_view.offset_y());
    assert!(
        (held.0 - loose.0).abs() < 1e-3 && (held.1 - loose.1).abs() < 1e-3,
        "letting go moved the view: {held:?} -> {loose:?}"
    );
    // And the crew member walking off does not drag it after them.
    let back = bims::math::vec2(offset.x as f32 + 100.0, offset.y as f32 + 100.0);
    game.world.aboard.room.put_for_probe(0, back);
    game.follow_player();
    let still = (game.ship_view.offset_x(), game.ship_view.offset_y());
    assert!((still.0 - loose.0).abs() < 1e-3 && (still.1 - loose.1).abs() < 1e-3);
    // A free camera goes wherever it is dragged: a pan that would have
    // been clamped carries the whole way, and the subject is off the edge.
    game.ship_view.pan(-100_000.0, 0.0);
    assert!(
        (game.ship_view.offset_x() - loose.0 + 100_000.0).abs() < 1.0,
        "a free pan was clamped: {} -> {}",
        loose.0,
        game.ship_view.offset_x()
    );
    assert!(
        pixel(&game).0 < 0.0,
        "the subject should be off the edge now"
    );
    // Zooming still keeps what is under the pointer under it.
    let before = game.ship_view.to_view(corner.0, corner.1);
    game.ship_view.zoom(corner.0, corner.1, 0.5);
    let after = game.ship_view.to_view(corner.0, corner.1);
    assert!((before.0 - after.0).abs() < 1e-2 && (before.1 - after.1).abs() < 1e-2);
    // Tethered again, the next frame snaps back to the crew member.
    game.set_follow(true);
    game.follow_player();
    let at = pixel(&game);
    assert!(
        (at.0 - middle.0).abs() < 1e-3 && (at.1 - middle.1).abs() < 1e-3,
        "following again did not bring them back: {at:?}"
    );
}

/// The map let go is a map: a zoom in on a planet lands on the planet,
/// not on the ship, a pan takes it anywhere, and the place it is looking
/// at holds still while the ship flies — its camera is measured from the
/// ship, so that is not for free. Following, it is about the ship again.
#[test]
fn the_map_let_go_holds_a_place_in_the_system_still() {
    let mut game = game();
    game.set_mode(ViewMode::Map);
    let middle = (CANVAS.0 / 2.0, CANVAS.1 / 2.0);
    let pixel_of = |game: &Game, at: worldgen::math::DVec2| {
        let offset = at.sub(game.world.ship.position());
        let cam = &game.map_view;
        (
            cam.offset_x() + offset.x as f32 * cam.scale(),
            cam.offset_y() - offset.y as f32 * cam.scale(),
        )
    };
    let near =
        |a: (f32, f32), b: (f32, f32), tol: f32| (a.0 - b.0).abs() < tol && (a.1 - b.1).abs() < tol;
    // The map opens free, about the ship.
    assert!(!game.follow);
    game.follow_player();
    let ship = game.world.ship.position();
    assert!(near(pixel_of(&game, ship), middle, 1e-2));

    // A place off to one side, zoomed in on about the pointer, twice: it
    // stays under the pointer, and the ship leaves the canvas.
    let corner = (middle.0 + 300.0, middle.1 - 200.0);
    let place = game.point_at(corner.0, corner.1);
    for _ in 0..12 {
        game.zoom(corner.0, corner.1, 4.0);
        game.follow_player();
    }
    assert!(
        near(pixel_of(&game, place), corner, 1.0),
        "{:?}",
        pixel_of(&game, place)
    );
    let at = pixel_of(&game, ship);
    assert!(
        at.0 < 0.0 || at.1 > CANVAS.1,
        "the ship is still on: {at:?}"
    );
    // Panned into the middle, it is the middle, and it stays the middle
    // as the ship flies off: the map is held on the place, not the ship.
    game.pan(middle.0 - corner.0, middle.1 - corner.1);
    game.follow_player();
    assert!(near(pixel_of(&game, place), middle, 1.0));
    let flown = ship.add(worldgen::math::dvec2(5_000_000.0, -3_000_000.0));
    game.world.ship.anchor = game.world.ship.anchor.add(flown.sub(ship));
    game.follow_player();
    assert!(
        near(pixel_of(&game, place), middle, 1.0),
        "the place flew with the ship: {:?}",
        pixel_of(&game, place)
    );
    // The scale is kept across a visit to the ship view.
    let scale = game.map_view.scale();
    game.set_mode(ViewMode::Ship);
    game.follow_player();
    game.set_mode(ViewMode::Map);
    game.follow_player();
    assert_eq!(game.map_view.scale(), scale);
    assert!(near(pixel_of(&game, place), middle, 1.0));

    // Following, the ship is the middle whatever it does; let go again,
    // nothing moves for the flip of the switch, and Select brings the
    // map back to the ship.
    game.set_follow(true);
    game.follow_player();
    assert!(near(
        pixel_of(&game, game.world.ship.position()),
        middle,
        1e-2
    ));
    game.pan(-50.0, 40.0);
    game.follow_player();
    let held = pixel_of(&game, game.world.ship.position());
    game.set_follow(false);
    game.follow_player();
    assert!(near(
        pixel_of(&game, game.world.ship.position()),
        held,
        1e-2
    ));
    game.pan(-100_000.0, 0.0);
    game.follow_player();
    assert!(
        pixel_of(&game, game.world.ship.position()).0 < 0.0,
        "a free pan was clamped"
    );
    game.centre_on_player();
    game.follow_player();
    assert!(near(
        pixel_of(&game, game.world.ship.position()),
        middle,
        1e-2
    ));
}

/// The blueprint in hand is the world's answer about the tile under the
/// pointer, asked once per tile and kept: the same tile again is the same
/// answer without the ship being validated again, a different tile or a
/// turn is asked afresh, and the tool put down forgets it. A site laid out
/// is found under its tiles.
#[test]
fn the_blueprint_asks_the_world_once_a_tile_and_finds_a_site_under_the_pointer() {
    use shipdesign::parts::{PartKind, Rotation};
    use world::world::Command;
    let mut game = game();
    assert!(game.ghost_check().is_none(), "no tool in hand");
    game.set_placing(Some((PartKind::Wall, Rotation::R0)));
    assert!(game.ghost_check().is_none(), "no tile under the pointer");

    // A deck tile the wall may stand on: the flyer's deck, away from the
    // fixtures. Found rather than named, so the fixture may move.
    let design = game.world.ship.design.clone();
    let grid = design.grid();
    let tile = (1..design.build_area as i32 - 1)
        .flat_map(|y| (1..design.build_area as i32 - 1).map(move |x| (x, y)))
        .find(|&t| {
            grid.has_floor(t)
                && game
                    .world
                    .can_place_site(PartKind::Wall, (t.0 as u32, t.1 as u32), Rotation::R0)
                    .is_ok()
        })
        .expect("somewhere on the deck a wall may go");
    game.hover = Some(tile);
    assert_eq!(game.ghost_check(), Some(Ok(())));
    assert!(game.ghost_ok());
    assert_eq!(game.ghost_answer(), Some(Ok(())));
    // Off the hull it is refused, and the refusal is the rules' own.
    game.hover = Some((-1, -1));
    assert!(matches!(
        game.ghost_check(),
        Some(Err(world::SiteRefusal::WontFit(_)))
    ));
    assert!(!game.ghost_ok());
    game.hover = Some(tile);
    assert_eq!(game.ghost_check(), Some(Ok(())));

    // Laid out, and found under the pointer.
    assert!(game.site_at(tile).is_none());
    game.send(Command::PlaceSite {
        slot: 0,
        kind: PartKind::Wall,
        origin: (tile.0 as u32, tile.1 as u32),
        rotation: Rotation::R0,
    });
    game.step();
    let site = game.site_at(tile).expect("the site under the pointer");
    assert_eq!(site.kind, PartKind::Wall);
    // And the same tile is now refused: the site is already there.
    assert!(matches!(
        game.ghost_check(),
        Some(Err(world::SiteRefusal::WontFit(_)))
    ));
    game.set_placing(None);
    assert!(game.ghost_check().is_none());
    assert!(!game.ghost_ok());
}

/// A station's person who has followed the crew through the passage is
/// drawn *over* the ship's deck, not under it. The stations are painted
/// before the ship, the residents' room with them, so a body standing on
/// the ship's tiles was under the hull — a name walking about over an
/// empty tile. The room hands its frame over split at the bodies
/// (`bims::game::Game::shapes_split`) and the painter lifts that half over
/// the ship's own picture.
#[test]
fn a_resident_on_the_ship_s_deck_is_drawn_over_it() {
    use crate::draw::{KIND_ELLIPSE, KIND_RECT, STRIDE};
    use crate::session::Session;

    let mut session = Session::combat(world::data::DEFAULT_SEED, CANVAS.0, CANVAS.1);
    assert!(session.stage_fight_for_probe());
    let game = session.game.as_mut().unwrap();
    // Crew member 1 is at its bunk on the ship; stand the resident near
    // it — a tile or two off, so the two bodies are told apart on screen
    // — on the ship's deck and in the crew's sight.
    let ship = game.world.ship.design.clone();
    let tile = TILE as f32;
    assert!(game.world.aboard.on_ship(1, &ship));
    let near = game.world.aboard.crew_ashore()[1].expect("crew member 1 is on its feet");
    // Where the resident stands, on the ship's own grid: its spot on the
    // joined deck through the station frame, less the ship's shift.
    let on_ship = |game: &Game| {
        let (origin, ex, ey) = game.world.aboard.station_frame.unwrap();
        let p = game.world.residents.as_ref().unwrap().aboard.exposed(0);
        let at = origin
            .add(ex.scale(p.x))
            .add(ey.scale(p.y))
            .sub(game.world.aboard.offset);
        let t = TILE as f64;
        let tile = ((at.x / t).floor() as i32, (at.y / t).floor() as i32);
        ship.grid().get(shipdesign::Layer::Structure, tile) != 0
    };
    let placed = [
        (1.0, 0.0),
        (-1.0, 0.0),
        (0.0, 1.0),
        (0.0, -1.0),
        (2.0, 0.0),
        (0.0, 2.0),
    ]
    .into_iter()
    .any(|(dx, dy): (f64, f64)| {
        let there = near.add(dvec2(dx * tile as f64, dy * tile as f64));
        {
            let residents = game.world.residents.as_mut().unwrap();
            let spot = residents.aboard.to_room(there);
            residents.aboard.room.put_for_probe(0, spot);
        }
        let (x, y) = world_paint::resident_on_screen(game, 0);
        let apart = (0..2).all(|who| {
            let (cx, cy) = world_paint::crew_on_screen(game, who);
            (cx - x).abs().max((cy - y).abs()) > 0.75 * tile
        });
        on_ship(game) && apart
    });
    assert!(placed, "somewhere on the ship's deck a tile from anybody");
    for _ in 0..3 {
        game.world.step(&[]);
    }
    assert!(
        game.world
            .residents
            .as_ref()
            .unwrap()
            .aboard
            .room
            .body_seen(0),
        "the crew see the resident beside them"
    );
    assert!(on_ship(game), "and it has not walked off the ship");
    let list = session.render().to_vec();
    let (x, y) = session
        .resident_on_screen(0)
        .expect("a resident in sight is on screen");

    let shapes: Vec<&[f32]> = list.chunks_exact(STRIDE).collect();
    // The last deck tile under the resident: a tile-sized rectangle the
    // point is inside. The hull's, or the room's — whichever is painted
    // last is what a body under it would be hidden by.
    let deck = shapes
        .iter()
        .rposition(|s| {
            s[0] == KIND_RECT
                && (s[3] - tile).abs() < 1.0
                && (s[4] - tile).abs() < 1.0
                && (s[1] - x).abs() <= tile / 2.0
                && (s[2] - y).abs() <= tile / 2.0
        })
        .expect("the resident stands on a deck tile");
    // The resident's body: its torso, an ellipse over half a tile across,
    // centred within a quarter tile of where its name goes. Not the
    // bolt it fires from there, which is thin rectangles and specks, and
    // not the crew member a tile off.
    let body = shapes
        .iter()
        .rposition(|s| {
            s[0] == KIND_ELLIPSE
                && s[3] > tile / 2.0
                && s[4] > tile / 2.0
                && (s[1] - x).abs() < tile / 4.0
                && (s[2] - y).abs() < tile / 4.0
        })
        .expect("the resident is drawn");
    assert!(
        body > deck,
        "the resident (shape {body}) is under the deck (shape {deck})"
    );
}

/// The host's save, stood up on a guest as the guest's own slot
/// (`Session::restore_as`, feature 67): the same world by its checksum —
/// as read back and six hundred steps on — but this end steers slot 1,
/// and the file says how many players it was saved for off its front
/// (`save::players_of`), for the host to check a load against the room.
#[test]
fn the_host_s_save_read_back_as_a_guest_is_the_same_world_steered_from_its_own_slot() {
    use crate::Session;
    use crate::save::players_of;
    let two = |slot: u32| {
        let mut session = Session::design(
            shipdesign::fixture::AREA,
            100_000,
            2,
            slot,
            world::data::DEFAULT_SEED,
            0,
            ship_session_spawn(),
            crate::Preset::Empty,
            CANVAS.0,
            CANVAS.1,
        );
        session.editor.give(shipdesign::fixture::combat_ship());
        let hash = session.editor.hash();
        assert!(session.accept(0, hash));
        assert!(session.accept(1, hash));
        assert!(session.playing());
        session
    };
    let mut host = two(0);
    let mut guest = two(1);
    for _ in 0..300 {
        host.world_step();
        guest.world_step();
    }
    host.crew_names = vec!["Ada".to_string(), "Bob".to_string()];
    let text = host.save().expect("a world to save");
    assert_eq!(players_of(&text), Some(2), "the players, off the front");
    assert_eq!(players_of("(not a save"), None);
    assert_eq!(players_of("(version:1,local:0"), None);
    let mut back = Session::restore_as(&text, 1, CANVAS.0, CANVAS.1).expect("the text reads back");
    assert_eq!(back.editor.local, 1, "the guest's own slot");
    assert_eq!(back.game.as_ref().unwrap().local, 1);
    assert_eq!(back.editor.players, 2);
    assert_eq!(back.crew_names, host.crew_names);
    let checksum = |s: &Session| s.game.as_ref().unwrap().world.checksum();
    assert_eq!(
        checksum(&back),
        checksum(&host),
        "the host's world, as read back"
    );
    assert_eq!(
        checksum(&back),
        checksum(&guest),
        "which is what the guest had"
    );
    // The file's own slot is what `restore` comes up as, and a slot the
    // crew has not got is the last one.
    assert_eq!(
        Session::restore(&text, CANVAS.0, CANVAS.1)
            .unwrap()
            .editor
            .local,
        0
    );
    assert_eq!(
        Session::restore_as(&text, 7, CANVAS.0, CANVAS.1)
            .unwrap()
            .editor
            .local,
        1
    );
    for _ in 0..600 {
        host.world_step();
        back.world_step();
    }
    assert_eq!(checksum(&back), checksum(&host), "six hundred steps on");
    let room = |s: &Session| {
        let room = &s.game.as_ref().unwrap().world.aboard.room;
        (0..room.crew_count() as usize)
            .map(|who| room.bim_pos(who))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        room(&back),
        room(&host),
        "the crew's places, six hundred steps on"
    );
}

fn ship_session_spawn() -> Option<(u32, u32)> {
    // The dock the wire test docks at, so the two crews stand somewhere.
    crate::session::pick_dock(world::data::DEFAULT_SEED, 0, 0)
}

/// A game written out and read back is the same game: the world's
/// checksum, the room aboard, and the picture both draw; and it stays the
/// same game stepped on — what a save left out would show up there as a
/// crew member walking somewhere else. A file from another build is
/// refused for its version, and something that is not a save for what
/// it is.
#[test]
fn a_game_saved_and_read_back_is_the_same_game() {
    use crate::Session;
    use crate::save::LoadError;
    let mut session = Session::simulate(world::data::DEFAULT_SEED, 0, None, CANVAS.0, CANVAS.1);
    for _ in 0..300 {
        session.world_step();
    }
    session.crew_names = vec!["Ada".to_string()];
    let text = session.save().expect("a world to save");
    let mut back = Session::restore(&text, CANVAS.0, CANVAS.1).expect("the text reads back");
    assert_eq!(back.editor.players, 1);
    assert_eq!(back.crew_names, session.crew_names, "the crew's names");
    assert_eq!(back.seed, session.seed);
    assert_eq!(back.spawn, session.spawn);
    let same = |a: &Session, b: &Session, when: &str| {
        let (a, b) = (a.game.as_ref().unwrap(), b.game.as_ref().unwrap());
        assert_eq!(a.world.steps, b.world.steps, "steps, {when}");
        assert_eq!(a.world.checksum(), b.world.checksum(), "checksum, {when}");
        let room = |g: &Game| {
            let room = &g.world.aboard.room;
            (0..room.crew_count() as usize)
                .map(|who| room.bim_pos(who))
                .collect::<Vec<_>>()
        };
        assert_eq!(room(a), room(b), "the crew's places, {when}");
    };
    same(&session, &back, "as read back");
    assert_eq!(
        session.render().to_vec(),
        back.render().to_vec(),
        "the picture, as read back"
    );
    for _ in 0..600 {
        session.world_step();
        back.world_step();
    }
    same(&session, &back, "six hundred steps on");
    assert_eq!(
        session.render().to_vec(),
        back.render().to_vec(),
        "the picture, six hundred steps on"
    );
    assert!(session.save().is_some());

    let now = format!("version:{}", crate::save::SAVE_VERSION);
    let old = text.replacen(&now, "version:0", 1);
    assert_ne!(old, text, "the version is written first");
    assert_eq!(
        Session::restore(&old, CANVAS.0, CANVAS.1).err(),
        Some(LoadError::Version(0))
    );
    assert!(matches!(
        Session::restore("(not a save", CANVAS.0, CANVAS.1),
        Err(LoadError::Syntax(_))
    ));
    // Nothing to save before there is a world.
    let design = Session::design(
        20,
        10_000,
        1,
        0,
        world::data::DEFAULT_SEED,
        0,
        session.spawn,
        crate::Preset::Playtest,
        CANVAS.0,
        CANVAS.1,
    );
    assert!(design.save().is_none());
}

/// The landed picture as an SVG, for looking at the plain without a
/// window: `cargo test -p ship a_landed_picture -- --ignored --nocapture
/// > target/plain.svg`. Crew member 0 is walked out onto the ground west
/// of the ship first, so the fog lifts round it. The view box is the
/// camera's units about the ship, `BIMS_SVG_REACH` tiles each way
/// (default 70).
#[test]
#[ignore]
fn a_landed_picture_as_svg() {
    use crate::Session;
    let mut session = Session::simulate(world::data::DEFAULT_SEED, 0, None, CANVAS.0, CANVAS.1);
    assert!(session.land_for_probe());
    assert!(session.walk_afield_for_probe());
    session.game.as_mut().unwrap().world.aboard.room.observe();
    let reach: f32 = std::env::var("BIMS_SVG_REACH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(70.0)
        * TILE as f32;
    let shapes: Vec<f32> = session.render().to_vec();
    let stride = crate::draw::STRIDE;
    println!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} {} {} {}\" width=\"1400\" height=\"1400\">",
        -reach,
        -reach,
        2.0 * reach,
        2.0 * reach
    );
    for s in shapes.chunks(stride) {
        let (kind, x, y, w, h, rot, radius, line) =
            (s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]);
        if x.abs() > reach * 1.5 || y.abs() > reach * 1.5 {
            continue;
        }
        let (r, g, b, a) = (
            (s[8] * 255.0).round(),
            (s[9] * 255.0).round(),
            (s[10] * 255.0).round(),
            s[11],
        );
        let paint = if line > 0.0 {
            format!(
                "fill=\"none\" stroke=\"rgb({r},{g},{b})\" stroke-opacity=\"{a}\" stroke-width=\"{line}\""
            )
        } else {
            format!("fill=\"rgb({r},{g},{b})\" fill-opacity=\"{a}\"")
        };
        let turn = rot.to_degrees();
        if kind == crate::draw::KIND_ELLIPSE {
            println!(
                "<ellipse cx=\"0\" cy=\"0\" rx=\"{}\" ry=\"{}\" {paint} transform=\"translate({x} {y}) rotate({turn})\"/>",
                w.abs() / 2.0,
                h.abs() / 2.0
            );
        } else if kind == crate::draw::KIND_TRIANGLE {
            println!(
                "<polygon points=\"{},{} {},{} {},{}\" {paint} transform=\"translate({x} {y}) rotate({turn})\"/>",
                -w / 2.0,
                -h / 2.0,
                w / 2.0,
                -h / 2.0,
                -w / 2.0,
                h / 2.0
            );
        } else {
            println!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{radius}\" {paint} transform=\"translate({x} {y}) rotate({turn})\"/>",
                -w / 2.0,
                -h / 2.0,
                w.abs(),
                h.abs()
            );
        }
    }
    println!("</svg>");
}

/// On a planet the view is held to the ground that is loaded: zoomed
/// out as far as the wheel goes, the canvas's far corner is still within
/// `bims::terrain::VIEW` tiles of its middle; and nothing holds it
/// docked in space.
#[test]
fn a_landed_view_reaches_no_further_than_the_ground_is_loaded() {
    use crate::Session;
    let mut session = Session::simulate(world::data::DEFAULT_SEED, 0, None, CANVAS.0, CANVAS.1);
    session.fit(CANVAS.0, CANVAS.1);
    session.zoom(CANVAS.0 / 2.0, CANVAS.1 / 2.0, 0.01);
    session.render();
    let scale_in_space = session.game.as_ref().unwrap().ship_view.scale();
    let reach = bims::terrain::VIEW as f32 * TILE as f32;
    let corner = |scale: f32| CANVAS.0.hypot(CANVAS.1) / 2.0 / scale;
    assert!(corner(scale_in_space) > reach, "docked, the view goes wide");
    assert!(session.land_for_probe());
    session.zoom(CANVAS.0 / 2.0, CANVAS.1 / 2.0, 0.01);
    session.render();
    let scale = session.game.as_ref().unwrap().ship_view.scale();
    let edge = |scale: f32| CANVAS.0.min(CANVAS.1) / 2.0 / scale;
    assert!(
        edge(scale) <= reach + 1.0,
        "landed, the nearer edge is {} units out of {reach}",
        edge(scale)
    );
    // And a notch of the wheel past the floor moves nothing.
    let before = session.game.as_ref().unwrap().ship_view.focus();
    let (px, py) = (
        session.game.as_ref().unwrap().ship_view.offset_x(),
        session.game.as_ref().unwrap().ship_view.offset_y(),
    );
    session.zoom(CANVAS.0 * 0.8, CANVAS.1 * 0.3, 0.5);
    let view = &session.game.as_ref().unwrap().ship_view;
    assert_eq!(view.focus(), before);
    assert_eq!(
        (view.offset_x(), view.offset_y()),
        (px, py),
        "the view walked off"
    );
}

/// Every bunk of the ship's wears a name on the deck — whose it is, or
/// that it is nobody's — and the tag lands where the bunk does: over the
/// Bim that starts at it, and turned with the ship the way its name is.
#[test]
fn a_bunk_s_tag_lands_on_the_bunk_and_turns_with_the_ship() {
    let mut game = game();
    let labels = world_paint::bunk_labels(&game);
    // The ship's own: docked, the deck is joined, and the station's
    // bunks are not its to give and wear no tag.
    let room = &game.world.aboard.room;
    let ours = (0..room.bed_count())
        .filter(|&b| room.bed_assignable(b))
        .count();
    assert!(
        ours < room.bed_count(),
        "docked, the station's bunks are there too"
    );
    assert_eq!(labels.len(), ours);
    assert_eq!(ours, 2, "the flyer sleeps two");
    assert_eq!(labels[0].owner, Some(0));
    assert_eq!(labels[1].owner, Some(1));
    // Bim *i* starts at bunk *i*: the tag is within a couple of tiles of
    // where its name goes.
    for who in 0..2u32 {
        let (cx, cy) = world_paint::crew_on_screen(&game, who);
        let l = labels[who as usize];
        let off = ((l.x - cx).powi(2) + (l.y - cy).powi(2)).sqrt();
        assert!(
            off < 2.0 * TILE as f32,
            "bunk {who}'s tag is {off} from its Bim"
        );
    }
    // Given up, it says nobody's.
    assert!(game.world.aboard.room.assign_bed(0, None));
    let labels = world_paint::bunk_labels(&game);
    assert_eq!(labels[0].owner, None);
    // And turned with the ship, not left where the deck was.
    game.world.ship.heading = 1.1;
    let turned = world_paint::bunk_labels(&game);
    assert!(
        (turned[1].x - labels[1].x).abs() > 1.0 || (turned[1].y - labels[1].y).abs() > 1.0,
        "the tag turned with the ship"
    );
    let (cx, cy) = world_paint::crew_on_screen(&game, 1);
    let off = ((turned[1].x - cx).powi(2) + (turned[1].y - cy).powi(2)).sqrt();
    assert!(off < 2.0 * TILE as f32, "and stayed by its Bim: {off}");
}

#[test]
fn the_hair_a_player_chose_is_on_its_crew_member_when_the_world_opens() {
    use crate::Session;
    use bims::character::{Hair, Look, Shade};
    let spawn = Session::simulate(world::data::DEFAULT_SEED, 0, None, CANVAS.0, CANVAS.1).spawn;
    let mut session = Session::design(
        shipdesign::fixture::AREA,
        100_000,
        2,
        0,
        world::data::DEFAULT_SEED,
        0,
        spawn,
        crate::Preset::Playtest,
        CANVAS.0,
        CANVAS.1,
    );
    // Only the first player has said; the second keeps the look its slot
    // deals, and so does the third — who is nobody's, whatever the list
    // says.
    session.crew_hair = vec![(Hair::Mohawk, Shade::Red)];
    // Nothing before the world opens.
    session.dress_crew();
    let hash = session.editor.hash();
    assert!(session.accept(0, hash));
    assert!(session.accept(1, hash));
    let room = &session
        .game
        .as_ref()
        .expect("the world opened")
        .world
        .aboard
        .room;
    assert_eq!(
        room.look(0),
        Look::of(0).with_hair(Hair::Mohawk, Shade::Red)
    );
    assert_eq!(room.look(1), Look::of(1));
    // Said late, for the second: put on at once, and a third slot is
    // nobody's — a bot keeps what it was dealt.
    session.crew_hair = vec![
        (Hair::Mohawk, Shade::Red),
        (Hair::Bald, Shade::Grey),
        (Hair::Curly, Shade::Blond),
    ];
    session.dress_crew();
    let room = &session.game.as_ref().unwrap().world.aboard.room;
    assert_eq!(room.look(1), Look::of(1).with_hair(Hair::Bald, Shade::Grey));
    if room.crew_count() > 2 {
        assert_eq!(room.look(2), Look::of(2));
    }
    // And a look is drawing only: the checksum is the same whatever the hair.
    let before = session.game.as_ref().unwrap().world.checksum();
    session.crew_hair[0] = (Hair::Long, Shade::Black);
    session.dress_crew();
    assert_eq!(session.game.as_ref().unwrap().world.checksum(), before);
}

/// A save keeps which tier of key each station's desk still holds: the
/// spawn's tier one, an enemy's tier two, and a desk taken bare — and a
/// tier-two key in the crew's own desk comes back as one.
#[test]
fn save_round_trip_keeps_key_tiers() {
    use crate::Session;
    use physics::ResourceId;
    use shipdesign::research::Node;
    let mut session = Session::simulate(world::data::DEFAULT_SEED, 0, None, CANVAS.0, CANVAS.1);
    let world = &mut session.game.as_mut().unwrap().world;
    let home = world.home;
    let keys: Vec<u8> = world.station_keys.clone();
    assert_eq!(world.station_key(home), 1);
    assert!(keys.contains(&2), "an enemy's desk in the spawn system");
    assert!(keys.contains(&0), "and a derelict's, bare");
    // A tier-one key taken from the spawn, and a tier-two key in the desk.
    let at = world.stations.iter().position(|s| s.id == home).unwrap();
    world.station_keys[at] = 0;
    world.ship.design.cargo[ResourceId::ResearchKeyTwo as usize] = 1;
    world.on_ship_changed();
    assert_eq!(world.keys_in_desk(2), 1);
    let keys: Vec<u8> = world.station_keys.clone();
    let checksum = world.checksum();

    let text = session.save().expect("a world to save");
    let back = Session::restore(&text, CANVAS.0, CANVAS.1).expect("the text reads back");
    let world = &back.game.as_ref().unwrap().world;
    assert_eq!(world.station_keys, keys);
    assert_eq!(world.station_key(home), 0);
    assert_eq!(world.keys_in_desk(2), 1);
    assert_eq!(world.keys_in_desk(1), 0);
    assert_eq!(world.checksum(), checksum);
    assert!(!world.research.is_done(Node::Upgrades));
    assert_eq!(world.research.done.len(), shipdesign::research::NODES);
}

/// A jammer brought down stays down through a save and a load, and the
/// derived station the machines put in a system with no orbit of its own
/// is **not in the file**: it is rolled again off the star's own stream
/// when the world is read back (feature 93, `World::settle_jammer`, which
/// `settle_crisis` calls and `Game::resume` calls in turn).
#[test]
fn save_round_trip_keeps_a_jammer_down_and_rolls_the_derived_one_again() {
    use crate::Session;
    let mut session = Session::simulate(world::data::DEFAULT_SEED, 0, None, CANVAS.0, CANVAS.1);
    let world = &mut session.game.as_mut().unwrap().world;
    // The machines began here, so this system is theirs from day nought.
    let here = world.star_id;
    world.set_droid_origin_for_probe(here);
    world.set_crisis_first_day_for_probe(0);
    assert!(world.infested(here));
    assert!(world.jammed(), "and its jammer stands");

    // The station it stands on, cleared: the last machine of the last
    // wave destroyed, which is what the crisis step records.
    let station = world.jammer_station().expect("infested");
    world.infest(station);
    world
        .infested
        .iter_mut()
        .find(|it| it.station == station)
        .unwrap()
        .cleared = true;
    assert!(!world.jammed());
    let checksum = world.checksum();

    let text = session.save().expect("a world to save");
    assert!(
        !text.contains(&format!("{}", world::jammer_id(here))),
        "a derived jammer is never written to the file"
    );
    let back = Session::restore(&text, CANVAS.0, CANVAS.1).expect("the text reads back");
    let world = &back.game.as_ref().unwrap().world;
    assert_eq!(world.droid_origin(), here);
    assert!(world.infested(here), "the hop table is worked out again");
    assert_eq!(world.jammer_station(), Some(station));
    assert!(!world.jammed(), "and it is still down");
    assert_eq!(world.checksum(), checksum);
}

/// A town the crew held against the machines stays held through a save
/// and a load, and so does an attack still running (feature 94): both
/// are in the file, so a world read back is the same fight at the same
/// point of it and the same checksum.
#[test]
fn save_round_trip_keeps_a_held_town_and_an_attack_under_way() {
    use crate::Session;
    let mut session = Session::simulate(world::data::DEFAULT_SEED, 0, None, CANVAS.0, CANVAS.1);
    // The town threatened — the machines a hop off — and the crew set
    // down at it, so an attack is laid down at the first step.
    assert!(
        session.defense_for_probe(world::data::STEP_MINUTES * 4.0, 1.0, 1),
        "a star one hop off"
    );
    let world = &mut session.game.as_mut().unwrap().world;
    if !world.land_for_probe() {
        return;
    }
    let id = world.ship.state.alongside().expect("landed");
    if !world.town_threatened(id) {
        // The roll put an enemy's town on this planet; nothing to hold.
        return;
    }
    for _ in 0..40 {
        world.step(&[]);
        if world.droids_standing() > 0 {
            break;
        }
    }
    let attacking = world.defense(id).cloned().expect("an attack");
    assert!(attacking.settled);
    let checksum = world.checksum();

    let text = session.save().expect("a world to save");
    let back = Session::restore(&text, CANVAS.0, CANVAS.1).expect("the text reads back");
    let world = &back.game.as_ref().unwrap().world;
    assert_eq!(world.defense(id), Some(&attacking), "the attack moved");
    assert_eq!(world.checksum(), checksum);

    // And a town held: the flag and the front price it carries survive.
    let world = &mut session.game.as_mut().unwrap().world;
    if let Some(residents) = world.residents.as_mut() {
        let room = &mut residents.aboard.room;
        for i in 0..room.droid_count() as usize {
            for _ in 0..60 {
                room.strike_droid(i, bims::droid::DroidPart::Chassis, 100.0);
            }
        }
    }
    for _ in 0..20 {
        world.step(&[]);
        if world.town_held(id) {
            break;
        }
    }
    if !world.town_held(id) {
        return;
    }
    let checksum = world.checksum();
    let text = session.save().expect("a world to save");
    let back = Session::restore(&text, CANVAS.0, CANVAS.1).expect("the text reads back");
    let world = &back.game.as_ref().unwrap().world;
    assert!(world.town_held(id), "a held town was forgotten");
    assert_eq!(world.front_at(id), Some(1), "and it is still on the front");
    assert_eq!(world.checksum(), checksum);
}

/// `tier2_test` and `tier3_test` are the `combat` session with everybody's
/// kit at that tier: every crew member's gun at it, its kind as `combat`
/// dealt it, and a full set of armour at it — pieces of the world's, worn
/// — and every one of the garrison the same. `combat` itself is left at
/// tier one, unarmoured.
#[test]
fn the_tier_tests_are_the_fight_with_everybody_s_kit_at_that_tier() {
    use crate::session::Session;
    use bims::combat::Tier;
    use bims::health::Part;

    let plain = Session::combat(world::data::DEFAULT_SEED, CANVAS.0, CANVAS.1);
    let plain = plain.game.as_ref().unwrap();
    for tier in [Tier::Two, Tier::Three] {
        let session = Session::combat_at_tier(world::data::DEFAULT_SEED, tier, CANVAS.0, CANVAS.1);
        let game = session.game.as_ref().unwrap();
        let world = &game.world;
        let crew = world.aboard.crew_count() as usize;
        assert_eq!(crew, shipdesign::fixture::COMBAT_CREW as usize);
        for who in 0..crew {
            let gear = world.aboard.room.gear(who);
            let dealt = plain.world.aboard.room.gear(who).weapon.unwrap();
            assert_eq!(gear.weapon, Some(dealt.kind.at(tier)), "crew {who}");
            for part in Part::ALL {
                let piece = gear.worn(part).expect("a piece on every part");
                assert_eq!(piece.tier, tier);
                let kept = world
                    .pieces
                    .iter()
                    .find(|p| p.id == piece.id)
                    .expect("the world knows the piece");
                assert_eq!(kept.at, world::armour::Where::Worn { who: who as u32 });
                assert_eq!(kept.tier, tier);
            }
        }
        let residents = world.residents.as_ref().expect("the garrison's room");
        assert_eq!(residents.aboard.count(), world::data::ARENA_GARRISON);
        for who in 0..residents.aboard.count() as usize {
            let gear = residents.aboard.room.gear(who);
            assert_eq!(gear.weapon.map(|w| w.tier), Some(tier), "resident {who}");
            for part in Part::ALL {
                assert_eq!(gear.worn(part).map(|p| p.tier), Some(tier));
            }
        }
        // The garrison's pieces are numbered a head apart, past any
        // mercenary's kit, so no two bodies' collide in the room.
        let ids: std::collections::BTreeSet<u32> = (0..residents.aboard.count() as usize)
            .flat_map(|who| {
                let gear = residents.aboard.room.gear(who);
                Part::ALL
                    .into_iter()
                    .map(move |part| gear.worn(part).unwrap().id)
            })
            .collect();
        assert_eq!(ids.len(), 3 * residents.aboard.count() as usize);
    }
    // `combat` is what it was: tier one in every hand, nothing worn.
    for who in 0..plain.world.aboard.crew_count() as usize {
        let gear = plain.world.aboard.room.gear(who);
        assert_eq!(gear.weapon.map(|w| w.tier), Some(Tier::One));
        assert!(Part::ALL.iter().all(|&p| gear.worn(p).is_none()));
    }
}

/// The classes in the design phase (features 74 and 75): a class chosen
/// leaves the pool alone — every Bim brings the same money, whatever it
/// is — and is what the world opens with, each with its starting kit;
/// and the classes, the progress and a deployable laid all read back
/// from a save the same, checksum and all.
#[test]
fn a_class_chosen_in_the_yard_leaves_the_pool_and_opens_the_world_and_is_saved() {
    use crate::Session;
    use world::deploy::Kit;
    use world::{Class, Command};
    let mut session = Session::design(
        shipdesign::fixture::AREA,
        100_000,
        2,
        0,
        world::data::DEFAULT_SEED,
        0,
        ship_session_spawn(),
        crate::Preset::Empty,
        CANVAS.0,
        CANVAS.1,
    );
    let plain = session.remaining();
    assert_eq!(plain, 200_000);
    assert!(session.set_class(0, Class::Engineer));
    assert_eq!(session.class_of(0), Class::Engineer);
    assert_eq!(session.class_of(1), Class::None);
    assert_eq!(
        session.remaining(),
        plain,
        "a class owns abilities, never money"
    );
    assert!(!session.set_class(0, Class::Engineer), "no change");
    assert!(!session.set_class(5, Class::Engineer), "no such slot");
    assert!(session.set_class(0, Class::Soldier));
    assert_eq!(session.remaining(), plain);
    assert!(session.set_class(1, Class::Engineer));
    session.editor.give(shipdesign::fixture::combat_ship());
    let hash = session.editor.hash();
    assert!(session.accept(0, hash));
    assert!(session.accept(1, hash));
    assert!(session.playing());
    let world = &session.game.as_ref().unwrap().world;
    assert_eq!(world.class_of(0), Class::Soldier);
    assert_eq!(world.class_of(1), Class::Engineer);
    assert_eq!(
        world.aboard.room.weapon(0),
        Some(bims::combat::WeaponKind::AutoRifle.basic()),
        "the soldier's rifle is in its hand"
    );
    assert_eq!(world.grenades_of(0), 2);
    let kits = |session: &Session, who: usize| {
        session
            .game
            .as_ref()
            .unwrap()
            .world
            .aboard
            .room
            .pack(who)
            .iter()
            .filter(|i| **i == Some(bims::combat::Item::Stack(Kit::Sandbag.resource() as u32)))
            .count()
    };
    assert_eq!(
        kits(&session, 1),
        world::deploy::SANDBAG_CHARGES as usize,
        "the kits are in the pack"
    );
    assert_eq!(kits(&session, 0), 0);
    assert!(
        !session.set_class(0, Class::Engineer),
        "playing: the command's job"
    );
    // A kit laid, a level reached, and the whole of it through a save.
    {
        let world = &mut session.game.as_mut().unwrap().world;
        let mut events = Vec::new();
        world.award(1, 100, &mut events);
        assert!(events.iter().any(|e| matches!(
            e,
            world::WorldEvent::LevelUp {
                who: 1,
                level: 2,
                ..
            }
        )));
        let here = world.aboard.room.bim_pos(1);
        let t = TILE as f32;
        let (cx, cy) = ((here.x / t).floor() as i32, (here.y / t).floor() as i32);
        let mut laid = false;
        'outer: for dx in -4..=4 {
            for dy in -4..=4 {
                if world
                    .can_deploy(1, Kit::Sandbag, (cx + dx, cy + dy))
                    .is_ok()
                {
                    world.step(&[Command::Deploy {
                        slot: 1,
                        kit: Kit::Sandbag,
                        x: cx + dx,
                        y: cy + dy,
                    }]);
                    laid = true;
                    break 'outer;
                }
            }
        }
        assert!(laid, "somewhere near the engineer to lay a kit");
    }
    for _ in 0..20_000 {
        session.world_step();
        if !session.game.as_ref().unwrap().world.deployables.is_empty() {
            break;
        }
    }
    assert_eq!(session.game.as_ref().unwrap().world.deployables.len(), 1);
    let text = session.save().expect("a world to save");
    let back = Session::restore(&text, CANVAS.0, CANVAS.1).expect("the text reads back");
    let (a, b) = (
        &session.game.as_ref().unwrap().world,
        &back.game.as_ref().unwrap().world,
    );
    assert_eq!(a.checksum(), b.checksum());
    assert_eq!(b.classes, vec![Class::Soldier, Class::Engineer]);
    assert_eq!(b.progress_of(1).level(), 2);
    assert_eq!(b.deployables, a.deployables);
    assert_eq!(back.class_of(1), Class::Engineer);
    assert_eq!(back.class_of(0), Class::Soldier);
    assert_eq!(b.grenades_of(0), 2, "the grenades read back in the pack");

    // And a medic's beam and charge, the same way (feature 76): the
    // soldier is a medic in a fresh session, linked to the engineer,
    // and the link and the charge read back.
    let mut session = Session::design(
        shipdesign::fixture::AREA,
        100_000,
        2,
        0,
        world::data::DEFAULT_SEED,
        0,
        ship_session_spawn(),
        crate::Preset::Empty,
        CANVAS.0,
        CANVAS.1,
    );
    assert!(session.set_class(0, Class::Medic));
    session.editor.give(shipdesign::fixture::combat_ship());
    let hash = session.editor.hash();
    assert!(session.accept(0, hash));
    assert!(session.accept(1, hash));
    {
        let world = &mut session.game.as_mut().unwrap().world;
        assert_eq!(world.class_of(0), Class::Medic);
        assert_eq!(
            world
                .aboard
                .room
                .pack(0)
                .iter()
                .filter(|i| **i
                    == Some(bims::combat::Item::Stack(
                        physics::ResourceId::Medkit as u32
                    )))
                .count(),
            2,
            "the medkits are in the pack"
        );
        // Crew member 1 beside it, a wound on it, and the beam on.
        let at = world.aboard.room.bim_pos(0) + bims::math::vec2(TILE as f32, 0.0);
        world.aboard.room.put_for_probe(1, at);
        world.aboard.room.wound(1, bims::health::Part::Legs, 2.0);
        world.step(&[]);
        world.step(&[Command::Beam {
            slot: 0,
            patient: Some(1),
        }]);
        assert_eq!(world.patients_of(0), vec![1]);
        for _ in 0..600 {
            world.step(&[]);
        }
        assert!(world.surge_charge(0) > 0.0, "charging");
    }
    let text = session.save().expect("a world to save");
    let back = Session::restore(&text, CANVAS.0, CANVAS.1).expect("the text reads back");
    let (a, b) = (
        &session.game.as_ref().unwrap().world,
        &back.game.as_ref().unwrap().world,
    );
    assert_eq!(a.checksum(), b.checksum());
    assert_eq!(b.class_of(0), Class::Medic);
    assert_eq!(b.patients_of(0), vec![1], "the beam reads back");
    assert_eq!(b.surge_charge(0), a.surge_charge(0));
}

/// Feature 83: a world with the machines in it round-trips through a
/// save whole — which station they hold, which wave is aboard, when the
/// next is due, and every machine where it stood with the damage it had.
/// The checksum, the picture and six hundred steps on, as the round trip
/// above asks of a world with people in it.
#[test]
fn a_droid_held_station_is_saved_and_read_back_whole() {
    use crate::Session;
    let mut session = Session::droids(
        world::data::DEFAULT_SEED,
        Some(bims::combat::Tier::One),
        1.0,
        None,
        3,
        CANVAS.0,
        CANVAS.1,
    );
    // Step until the wave is aboard, then knock a couple about so the
    // save has damage and a wreck in it rather than a fresh rack.
    for _ in 0..60 {
        session.world_step();
    }
    let standing = session
        .game
        .as_ref()
        .map(|g| g.world.droids_standing())
        .unwrap_or(0);
    assert!(standing > 2, "a wave stood up: {standing}");
    {
        let world = &mut session.game.as_mut().unwrap().world;
        let room = &mut world.residents.as_mut().unwrap().aboard.room;
        room.strike_droid(0, bims::droid::DroidPart::Chassis, 1e6);
        room.strike_droid(1, bims::droid::DroidPart::Legs, 5.0);
        room.strike_droid(2, bims::droid::DroidPart::Arms, 1e6);
    }
    session.world_step();

    let text = session.save().expect("a world to save");
    let mut back = Session::restore(&text, CANVAS.0, CANVAS.1).expect("the text reads back");

    let machines = |s: &Session| {
        let world = &s.game.as_ref().unwrap().world;
        let room = &world.residents.as_ref().unwrap().aboard.room;
        (0..room.droid_count() as usize)
            .filter_map(|i| room.droid(i))
            .map(|d| {
                (
                    d.kind.code(),
                    d.weapon.kind.code(),
                    d.destroyed,
                    (d.pos.x * 1000.0) as i64,
                    (d.pos.y * 1000.0) as i64,
                    bims::droid::DroidPart::ALL.map(|p| (d.body.health(p) * 100.0) as i64),
                )
            })
            .collect::<Vec<_>>()
    };
    let waves = |s: &Session| {
        let world = &s.game.as_ref().unwrap().world;
        world.infested.clone()
    };
    let same = |a: &Session, b: &Session, when: &str| {
        let (ga, gb) = (a.game.as_ref().unwrap(), b.game.as_ref().unwrap());
        assert_eq!(ga.world.checksum(), gb.world.checksum(), "checksum, {when}");
        assert_eq!(waves(a), waves(b), "which waves are left, {when}");
        assert_eq!(machines(a), machines(b), "the machines, {when}");
    };
    assert!(!machines(&session).is_empty(), "there are machines to keep");
    assert!(
        machines(&session).iter().any(|m| m.2),
        "and a wreck among them"
    );
    assert!(!waves(&session).is_empty(), "and a station held");
    same(&session, &back, "as read back");
    assert_eq!(
        session.render().to_vec(),
        back.render().to_vec(),
        "the picture, as read back"
    );
    for _ in 0..600 {
        session.world_step();
        back.world_step();
    }
    same(&session, &back, "six hundred steps on");
}

/// Feature 92: the crisis round-trips through a save. The origin and the
/// day the first star turns are in the file; the **hop table** is not —
/// it is derived from the galaxy and the origin, and `Game::resume`
/// works it out again — so a world read back has to report the same
/// infested set as the one it was written from, and the same checksum
/// with it.
#[test]
fn the_crisis_is_saved_and_the_hop_table_is_worked_out_again() {
    use crate::Session;
    let mut session = Session::simulate(world::data::DEFAULT_SEED, 0, None, CANVAS.0, CANVAS.1);
    // A day's spread behind them, so the read-back has stars to agree
    // about rather than an empty set.
    assert!(session.crisis_for_probe(0), "the origin goes two hops off");
    {
        let world = &mut session.game.as_mut().unwrap().world;
        world.set_day_for_probe(world::data::DROID_SPREAD_DAYS * 6);
    }
    session.world_step();

    let taken = |s: &Session| {
        let world = &s.game.as_ref().unwrap().world;
        (
            world.droid_origin(),
            world.crisis_first_day(),
            world.infested_stars(),
        )
    };
    let (origin, first_day, stars) = taken(&session);
    assert!(stars.len() > 1, "the crisis has spread: {}", stars.len());

    let text = session.save().expect("a world to save");
    let mut back = Session::restore(&text, CANVAS.0, CANVAS.1).expect("the text reads back");
    assert_eq!(taken(&back), (origin, first_day, stars.clone()));
    assert_eq!(
        session.game.as_ref().unwrap().world.checksum(),
        back.game.as_ref().unwrap().world.checksum(),
        "two clients at the same day"
    );
    for _ in 0..300 {
        session.world_step();
        back.world_step();
    }
    assert_eq!(taken(&session), taken(&back), "three hundred steps on");
    assert_eq!(
        session.game.as_ref().unwrap().world.checksum(),
        back.game.as_ref().unwrap().world.checksum(),
    );
}
