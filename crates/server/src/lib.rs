//! The relay as a library: [`hub`], the rooms and who is in them, with no
//! socket in it — what `main.rs` puts on a listener, and what the app's
//! own tests pass two `Session`s' messages through to prove the lockstep
//! holds without opening a port.

pub mod hub;
