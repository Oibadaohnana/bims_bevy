//! The game view: the session the app owns, its cameras and painters.
//!
//! The app hands a [`Session`] a canvas size and the numbers the lobby
//! chose, then each frame asks it to rebuild a shape buffer and paints
//! that. The format is the room's twelve floats a shape, so one replay loop
//! paints every picture — and this crate imports the room only to run it
//! *aboard*, through `crates/world`. The ship designer that once opened a
//! run here went in October 2026.
//!
//! Every rule is next door in `shipdesign` and `world`, which render
//! nothing and know nothing about a pointer.
//!
//! # The boundary
//!
//! **No strings.** The parts and everything the world says are numbers,
//! and `crates/app/src/names.rs` is where the words live. A native server
//! will one day run `shipdesign` and `world` and it has no words to say;
//! keeping the words out of here is what keeps that true.

pub mod camera;
pub mod draw;
pub mod fittings;
pub mod fork;
pub mod game;
pub mod hull;
pub mod paint;
pub mod save;
pub mod session;
pub mod sprays;
pub mod world_paint;

pub use session::{NONE, Session};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_render;
#[cfg(test)]
mod tests_survivors;
