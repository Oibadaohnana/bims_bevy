//! One galaxy, generated the same way every time, and what it comes out at.
//!
//! Here rather than in the tests for the reason `shipdesign::fixture` and
//! `world::fixture` are: **two targets have to agree about it.** The lobby
//! generates the galaxy in wasm to draw the star map, and the native server
//! that will one day be authoritative generates it for x86; the only way to
//! find out whether they agree is to run a fixed seed on both and compare each
//! against the same written-down number.
//!
//! The native end is `tests.rs`; the wasm end is `lobby_galaxy_checksum_hi`
//! and `_lo` in `crates/lobby`, which `scratchpad/builder-check.mjs` reads
//! and compares against the constants below — parsed out of this file, so the
//! harness and the test are looking at one copy of the number. If one
//! target's arithmetic ever drifts from the other's, exactly one of those two
//! fails.

use crate::galaxy::{Galaxy, GalaxyType};

/// The seed every pinned number below was generated from. Chosen to have both
/// halves non-zero, so a lobby that dropped the high word — the boundary
/// carries a seed as two `u32`s — would come out at a different galaxy.
pub const REFERENCE_SEED: u64 = 0x_4c4f_4242_0000_0007;

/// What [`Galaxy::checksum`] comes out at for [`REFERENCE_SEED`] under each
/// galaxy type, indexed by `GalaxyType as usize`.
///
/// Pinned rather than computed: a test comparing two computed values would
/// pass happily while both were wrong. Update these only when the generator
/// is meant to change — which is a [`crate::GENERATOR_VERSION`] bump, and
/// the test that reads them says so.
///
/// Task 115 moved them without a bump: two resources (the minigun and the
/// rail lance) grew every station's price lean by two entries, drawn last
/// on the lean's own branch, so no draw before them and no layout moved —
/// only the two new leans are hashed. They were `0x_205c_fd37_8f5d_9849`,
/// `0x_9f94_57ee_6f14_4ebc`, `0x_f4fd_3fa6_9a5b_9dc1` and
/// `0x_1f52_0bb7_ab2f_a1f9`.
///
/// Task 116 moved them the same way, for the same reason: the arc greaves
/// and the Reflective plate are two more resources, two more leans drawn
/// last. They were `0x_b948_a8b1_6811_6788`, `0x_41ad_6e95_f9dc_ec35`,
/// `0x_dd43_0b91_9ebb_0e20` and `0x_04ac_f49a_46b2_4ab8`.
pub const REFERENCE_CHECKSUMS: [u64; 4] = [
    0x_0493_9a6f_0971_1fdd,
    0x_9179_a4bd_4ee4_eff0,
    0x_3ea7_1366_ed45_8715,
    0x_87ea_7d6a_7635_010d,
];

/// The reference galaxy of one type.
pub fn reference(galaxy_type: GalaxyType) -> Galaxy {
    Galaxy::new(REFERENCE_SEED, galaxy_type)
}
