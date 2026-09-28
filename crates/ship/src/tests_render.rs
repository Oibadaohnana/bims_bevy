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
