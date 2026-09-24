//! The screens, one module each, in the order a player meets them: the
//! builder (the menu, the setup screen and the lobby), the designer, the
//! game, and — on its own — the station builder.

pub mod builder;
pub mod designer;
pub mod game;
pub mod station;
pub mod worldmap;

/// A random `u64`, so each run wanders somewhere new: the setup screen's
/// galaxy seed, a lobby's roll and the `test` commands' galaxy and dock.
pub fn rand_seed() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0),
    );
    h.finish()
}
