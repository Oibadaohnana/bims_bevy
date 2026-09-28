//! The shape buffer is the same however its two jobs are run (task 122).
//!
//! [`Session::render`] builds the stations' own pictures beside the crew's
//! room drawing itself, through whatever `fork::Join` the host said. Two
//! sessions built alike are stepped alike and drawn frame for frame, one
//! with the jobs run in turn ([`fork::serial`]) and one with the stations'
//! pictures on a thread of their own ([`fork::scoped`]), and the two
//! buffers — every float, bit for bit — and their cut at the fog have to
//! be one. The decks: the simulation, docked at its spawn with the
//! station's people about; the fight, the ship at the droids' arena; a
//! town on a planet, whose picture is the ground's; the system map, which
//! builds nothing; and the yard, which has no rooms at all. `PICTURES` in
//! `tests_survivors` is the other half: that the serial buffer is the one
//! the painter always drew.

use crate::fork::{self, Join};
use crate::game::ViewMode;
use crate::session::{galaxy_type, pick_ground};
use crate::{Preset, Session, world_paint};

const W: f32 = 1400.0;
const H: f32 = 900.0;

/// Draw both sessions `frames` times, `steps` world steps apart, each with
/// its own join, and compare every frame's buffer and fog cut. `built` says
/// whether any frame had a station's picture to build beside the room, so
/// a deck that never did cannot pass for having been tested.
fn alike(name: &str, mut a: Session, mut b: Session, frames: u32, steps: u32) -> bool {
    let joins: [Join; 2] = [fork::serial, fork::scoped];
    let mut built = false;
    for frame in 0..frames {
        for _ in 0..steps {
            a.world_step();
            b.world_step();
        }
        built |= a
            .game
            .as_ref()
            .is_some_and(|g| !world_paint::stations_to_build(g).is_empty());
        let one: Vec<u32> = a
            .render_with(joins[0])
            .iter()
            .map(|f| f.to_bits())
            .collect();
        let two: Vec<u32> = b
            .render_with(joins[1])
            .iter()
            .map(|f| f.to_bits())
            .collect();
        assert!(!one.is_empty(), "{name}: frame {frame} drew nothing");
        assert!(
            one == two,
            "{name}: frame {frame} differs ({} floats against {})",
            one.len(),
            two.len()
        );
        assert_eq!(
            a.fog_split().0.len(),
            b.fog_split().0.len(),
            "{name}: frame {frame} is cut at the fog elsewhere"
        );
    }
    built
}

#[test]
fn the_shape_buffer_is_the_same_however_its_two_jobs_are_run() {
    let seed = world::data::DEFAULT_SEED;

    let simulation = || Session::simulate(seed, 0, None, W, H);
    assert!(
        alike("simulation", simulation(), simulation(), 20, 30),
        "the simulation's station is drawn whole"
    );

    let droids = || Session::droids(seed, None, 1.0, None, 3, W, H);
    assert!(
        alike("droids", droids(), droids(), 20, 40),
        "the arena is drawn whole"
    );

    let town = || {
        let spawn = pick_ground(12_345, 0, 678);
        let mut session = Session::simulate_on(
            shipdesign::fixture::combat_ship(),
            1,
            12_345,
            0,
            spawn,
            W,
            H,
        );
        session.land_for_probe();
        session
    };
    assert!(
        alike("town", town(), town(), 10, 30),
        "the town is drawn whole"
    );

    let map = || {
        let mut session = simulation();
        session
            .game
            .as_mut()
            .expect("a game")
            .set_mode(ViewMode::Map);
        session
    };
    assert!(
        !alike("map", map(), map(), 5, 10),
        "the map builds no station's picture"
    );

    let yard = || {
        let spawn = world::spawn(&worldgen::Galaxy::new(seed, galaxy_type(0)));
        Session::design(
            shipdesign::fixture::AREA,
            100_000,
            1,
            0,
            seed,
            0,
            spawn,
            Preset::Playtest,
            W,
            H,
        )
    };
    assert!(!alike("yard", yard(), yard(), 3, 0), "the yard has no game");
}

/// The same frame drawn from nothing: [`world_paint::paint`], which keeps
/// no picture from one frame to the next.
fn from_nothing(session: &Session) -> Vec<u32> {
    let mut list = crate::draw::DrawList::default();
    world_paint::paint(session.game.as_ref().expect("a game"), &mut list);
    list.shapes().iter().map(|f| f.to_bits()).collect()
}

fn bits(shapes: &[f32]) -> Vec<u32> {
    shapes.iter().map(|f| f.to_bits()).collect()
}

/// A station's picture kept from frame to frame (`world_paint::KeptStation`)
/// is the picture the painter draws from nothing, bit for bit: every frame
/// of a while at the simulation's dock, the airlock swinging as the crew
/// come and go; a frame with nothing changed builds nothing and uses what
/// it kept; and the frame after a part of the station is taken away draws
/// the station without it, not the picture kept from before.
#[test]
fn a_kept_station_picture_is_the_one_drawn_from_nothing() {
    let mut session = Session::simulate(world::data::DEFAULT_SEED, 0, None, W, H);
    for frame in 0..40 {
        for _ in 0..15 {
            session.world_step();
        }
        let kept = bits(session.render_with(fork::serial));
        assert!(kept == from_nothing(&session), "frame {frame} differs");
    }

    // Nothing moves between these frames, so after the airlock has come to
    // rest the kept picture is used again as it is: the same buffer.
    for _ in 0..200 {
        session.render_with(fork::serial);
    }
    let kept = |s: &Session| {
        let game = s.game.as_ref().expect("a game");
        assert_eq!(game.kept_stations.len(), 1, "one station drawn whole");
        game.kept_stations[0].shapes().as_ptr()
    };
    let before = kept(&session);
    let still = bits(session.render_with(fork::serial));
    assert_eq!(kept(&session), before, "nothing changed, nothing built");
    assert!(still == from_nothing(&session));

    // A floor tile of the station taken away: the next frame is the station
    // without it, as the painter draws it from nothing.
    {
        let game = session.game.as_mut().expect("a game");
        let docked = game.world.ship.state.alongside().expect("docked");
        let station = game
            .world
            .stations
            .iter_mut()
            .find(|s| s.id == docked)
            .expect("the dock is a station");
        let floor = station
            .design
            .parts
            .iter()
            .position(|p| p.layer() == shipdesign::parts::Layer::Floor)
            .expect("a floor tile");
        station.design.parts.remove(floor);
    }
    let changed = bits(session.render_with(fork::serial));
    assert_ne!(kept(&session), before, "the picture was built again");
    assert!(changed != still, "the tile is gone from the picture");
    assert!(
        changed == from_nothing(&session),
        "and drawn as from nothing"
    );
}
