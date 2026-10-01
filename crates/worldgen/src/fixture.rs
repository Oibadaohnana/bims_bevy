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
///
/// Task 111 moved them without a bump too: a mining outpost is never
/// built (`data::parent_suits` suits it to nothing, and a star promised
/// one is promised nothing), so `pick_kind` offers one kind fewer and the
/// systems that would have had an outpost have something else, or nothing.
/// Which kind is picked is a station share, off the bump list. They were
/// `0x_0493_9a6f_0971_1fdd`, `0x_9179_a4bd_4ee4_eff0`,
/// `0x_3ea7_1366_ed45_8715` and `0x_87ea_7d6a_7635_010d`.
///
/// `GENERATOR_VERSION` 8 moved them with a bump: six hundred stars where
/// there were a thousand, and every system given a station and a planet
/// to set down on where it had none. They were `0x_f667_2369_aaf7_8e9b`,
/// `0x_da56_7ccd_f912_1602`, `0x_6c4b_e819_4f12_e6d3` and
/// `0x_61af_b7d5_7df6_2c6b`.
///
/// Task 120 moved them without a bump: the medicine went out of the game,
/// so no shelf stocks a medkit (a staple no more) or a bandage — the
/// shelves' bits, and no layout or other draw. They were
/// `0x_bbdb_ea99_21ed_2e34`, `0x_2aa6_e423_eb27_2fb3`,
/// `0x_0d14_75e5_b017_85f6` and `0x_1378_9fb5_d958_66db`.
///
/// Task 127 moved them without a bump: the sandbag kit, the sentry kit
/// and the grenade went out of the resources, so every station's price
/// lean is three entries shorter — drawn on the lean's own branch, so no
/// layout and no other draw moved. They were `0x_f726_4e5d_d786_d734`,
/// `0x_cd96_b463_1cf1_1273`, `0x_d2c8_e0d1_e3b1_bd96` and
/// `0x_977e_dc0b_a6c2_6e9b`.
///
/// October 2026 moved them without a bump, the same way: a Bim wears one
/// armour, so the helm, the leg guards, the arc greaves and the Reflective
/// plate went out of the resources and every lean is four entries
/// shorter. They were `0x_393e_f652_82eb_b85b`, `0x_19df_385e_9d22_d91c`,
/// `0x_8009_4c42_c725_ac59` and `0x_3794_c50f_8b35_67d4`.
///
/// October 2026 moved them without a bump: 240 stars where there were six
/// hundred, in a radius of twenty thousand where it was fifty — the star
/// field and its lanes, not the inside of any system. They were
/// `0x_e34f_9e80_fcdd_2871`, `0x_325e_bc41_cdea_a9de`,
/// `0x_4f04_aca7_b928_c89b` and `0x_0d7e_71d9_b169_88b6`.
pub const REFERENCE_CHECKSUMS: [u64; 4] = [
    0x_7401_81a7_4d3b_2ca2,
    0x_3ee7_48a7_707f_d097,
    0x_8fe0_4fe5_7c35_8d5c,
    0x_ee52_7add_492b_72a8,
];

/// The reference galaxy of one type.
pub fn reference(galaxy_type: GalaxyType) -> Galaxy {
    Galaxy::new(REFERENCE_SEED, galaxy_type)
}
