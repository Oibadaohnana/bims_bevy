//! Bims — the room.
//!
//! The Bims' simulation: the bodies on a deck, their errands, their fight,
//! the machines they fight, and the drawing of all of it as a flat list of
//! shapes. Nothing in here knows what a window is. `crates/world` owns a
//! [`game::Game`] aboard the designed ship and on every station's deck,
//! steps it, and the app turns [`game::Game::shapes`] into pixels. The
//! panels, the menus and every word on screen are the app's; what crosses
//! out of here is numbers and shapes, and that is deliberate: a native
//! server will one day run this same room and it has no words to say
//! either.

// Public, because the ship game runs this room aboard the designed ship —
// `world` reaches `aboard`, `game`, `room` and `math` — and the app drives
// it through `game` and names what it is told through `room`'s codes.
pub mod aboard;
pub mod balance;
pub mod bim;
pub mod blood;
pub mod character;
pub mod clock;
pub mod combat;
pub mod cue;
pub mod door;
pub mod draw;
pub mod droid;
pub mod fixtures;
pub mod fx;
pub mod game;
pub mod health;
pub mod math;
pub mod memory;
pub mod nav;
pub mod order;
pub mod rng;
pub mod room;
pub mod routine;
pub mod sight;
pub mod task;
pub mod terrain;
pub mod work;

/// The shared `time` crate, pulled into the crate root so every module reaches
/// it as `crate::time`. That spelling is deliberate: the native probes in
/// `scratchpad/` declare the crate's modules by `#[path]` and link nothing at
/// all, so they stand a plain `mod time;` over the same file in its place.
/// Written as `time::` it would resolve only through the extern prelude, and
/// every probe would need a `--extern` and a build step to go with it.
pub(crate) use ::time;
