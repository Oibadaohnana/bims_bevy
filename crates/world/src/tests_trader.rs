//! Every site is an attack, a defence or a trader (feature 111, on hold):
//! how many sites the candidate trader rule would make traders. The
//! feature wants the share between 5 and 40 per cent before it builds on
//! the rule; this rule gives 55–58, so it waits on a decision.

use worldgen::{Galaxy, GalaxyType};

use crate::data;
use crate::run::trades_gear;
use crate::surface::Surface;

/// How many of a galaxy's sites are traders, over ten galaxy seeds — the
/// measurement the rule waits on. Every station the generator rolled and
/// every settlement; the derived jammers and the fortress are not the
/// generator's and are not counted. The first column is the rule
/// ([`trades_gear`]); the others are the same roll read other ways, for
/// choosing a rule that lands inside the band:
///
/// - **both**: a desk dealing in guns *and* armour;
/// - **orbital either / both**: the same, with a planet's town never a
///   trader (the towns are the defences);
/// - and the counts of stations and towns, since the towns are most of
///   the sites and roll their trades as an orbital does.
#[test]
#[ignore]
fn trader_share_over_ten_seeds() {
    println!(
        "seed                  sites (stations, towns)  either  both  orbital-either  orbital-both"
    );
    let mut rows: Vec<[f64; 4]> = Vec::new();
    for n in 0..10u64 {
        let seed = data::DEFAULT_SEED.wrapping_add(n.wrapping_mul(0x9e37_79b9));
        let galaxy = Galaxy::new(seed, GalaxyType::SpiralTwoArm);
        let (mut stations, mut towns) = (0u32, 0u32);
        // either, both, orbital either, orbital both
        let mut count = [0u32; 4];
        for system in galaxy.every_system() {
            for s in &system.stations {
                stations += 1;
                let either = trades_gear(s.kind, false, s.stock);
                let both = either && s.stock.weapon_trade() && s.stock.armour_trade();
                count[0] += either as u32;
                count[1] += both as u32;
                count[2] += either as u32;
                count[3] += both as u32;
            }
            for s in Surface::all_of(&system, seed) {
                towns += 1;
                let either = trades_gear(crate::surface::SURFACE_KIND, true, s.stock);
                let both = either && s.stock.weapon_trade() && s.stock.armour_trade();
                count[0] += either as u32;
                count[1] += both as u32;
            }
        }
        let sites = (stations + towns).max(1) as f64;
        let row = count.map(|c| 100.0 * c as f64 / sites);
        println!(
            "{seed:>20}  {:>5} ({stations}, {towns})  {:>5.1}%  {:>4.1}%  {:>13.1}%  {:>11.1}%",
            stations + towns,
            row[0],
            row[1],
            row[2],
            row[3]
        );
        rows.push(row);
    }
    for (i, name) in ["either", "both", "orbital either", "orbital both"]
        .iter()
        .enumerate()
    {
        let mut shares: Vec<f64> = rows.iter().map(|r| r[i]).collect();
        shares.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "{name:>15}: min {:.1}%  median {:.1}%  max {:.1}%",
            shares[0],
            (shares[4] + shares[5]) / 2.0,
            shares[9]
        );
    }
}
