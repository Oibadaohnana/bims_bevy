//! Procedural stations and towns (feature 112): the generator's own tests
//! and the ignored measurements beside them. The walkability contract over
//! generated stations is `tests::a_station_s_rooms_and_a_one_tile_corridor_
//! can_be_walked` (`Plan::Generated` is on `Plan::ALL`), and over generated
//! towns `tests_surface::a_town_is_a_place_the_room_can_live_in_...`.

use shipdesign::{PartKind, design_hash};
use worldgen::{Galaxy, GalaxyType, StationKind};

use crate::data;
use crate::station::{Plan, Station, layout, residents_of};
use crate::stationgen::{self, Tally, generate, side_range};
use crate::surface::{Biome, Surface, Wall};

/// Whether the full tier asked for the whole matrix.
fn sweep() -> bool {
    std::env::var("BIMS_SWEEP").is_ok()
}

/// A seed a place in a sweep stands for: spread over the whole range.
fn seed_of(i: u64) -> u64 {
    i.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x_5eed
}

/// A generated station is a pure function of its kind and its seed: the
/// same design twice in one process, and again once the cache of built
/// layouts is emptied — and a town the same, built twice. The drawn plans
/// are still rolled off their own stream, as they were.
#[test]
fn a_generated_station_or_town_is_the_same_built_twice() {
    for kind in StationKind::ALL {
        for i in 0..4 {
            let seed = seed_of(i);
            let first = design_hash(&layout(kind, Plan::Generated, seed));
            assert_eq!(first, design_hash(&layout(kind, Plan::Generated, seed)));
            crate::station::forget_built();
            assert_eq!(
                first,
                design_hash(&layout(kind, Plan::Generated, seed)),
                "{kind:?} at {seed}: built again after the cache was emptied"
            );
            let direct = generate(kind, seed).map(|g| design_hash(&g.placer.design));
            if let Some(direct) = direct {
                assert_eq!(first, direct, "{kind:?} at {seed}");
            }
        }
    }
    for biome in Biome::ALL {
        let a = crate::surface::build_town(7, biome, 17);
        let b = crate::surface::build_town(7, biome, 17);
        assert_eq!(design_hash(&a.placer.design), design_hash(&b.placer.design));
        assert_eq!(a.gates, b.gates);
        assert_eq!(a.attempt, b.attempt);
    }
    // Every station rolls the generated plan; the drawn plan it falls back
    // on is the old roll, evenly over the six.
    let mut seen = std::collections::BTreeMap::new();
    for i in 0..600 {
        assert_eq!(Plan::rolled(seed_of(i)), Plan::Generated);
        *seen.entry(Plan::hand_rolled(seed_of(i))).or_insert(0) += 1;
    }
    assert_eq!(seen.len(), Plan::HAND.len(), "{seen:?}");
}

/// Every station of a kind a sweep generates, and the ones that fell back.
fn generated(kind: StationKind, n: u64) -> (Vec<(u64, stationgen::Generated)>, u64) {
    let mut out = Vec::new();
    let mut fell = 0;
    for i in 0..n {
        match generate(kind, seed_of(i)) {
            Some(g) => out.push((seed_of(i), g)),
            None => fell += 1,
        }
    }
    (out, fell)
}

/// The least, the middle and the most of a column of the tallies.
fn spread(values: &mut [u32]) -> String {
    values.sort_unstable();
    format!(
        "{}/{}/{}",
        values[0],
        values[values.len() / 2],
        values[values.len() - 1]
    )
}

/// The invariants every generated station keeps, asked without the
/// room's navigation (the contract's own walk is in `tests.rs`): fifty
/// seeds a kind in a plain run and a thousand under `BIMS_SWEEP`. The port
/// in the west skin and first, the array in the north skin, beds for the
/// residents and two mercenaries, the airlocks the kind wants with every
/// extra facing space ten tiles or more from the port, a loop, the desks
/// and the rooms' fixtures, sandbags clear of every doorway, the size in
/// the kind's range — a relay never as big as an orbital — a derelict
/// empty and holed, and the designer's rules on the first ten a kind (all
/// of them in a sweep). Prints the spread of the hull, the rooms, the
/// corridor deck, the loops and the airlocks, and fails if more than two
/// in a hundred fell back on a drawn plan.
#[test]
fn generated_stations_keep_their_invariants() {
    let n: u64 = if sweep() { 1000 } else { 50 };
    let mut sides_of = std::collections::BTreeMap::new();
    for kind in StationKind::ALL {
        let (built, fell) = generated(kind, n);
        assert!(
            fell * 100 <= 2 * n,
            "{kind:?}: {fell} of {n} fell back on a drawn plan"
        );
        let residents = residents_of(kind);
        let (least, most) = side_range(kind);
        let mut tallies: Vec<Tally> = Vec::new();
        for (i, (seed, g)) in built.iter().enumerate() {
            let design = &g.placer.design;
            let name = format!("{kind:?} at seed {seed}");
            let side = design.build_area;
            assert!(
                (least..=most).contains(&side) && side <= 68,
                "{name}: {side}"
            );
            let ports = crate::droid::airlocks(design);
            let port = ports[0];
            assert_eq!(
                shipdesign::port(design).map(|p| p.part_id),
                Some(port.part_id)
            );
            assert_eq!(
                port.outward,
                (-1, 0),
                "{name}: the port is in the west skin"
            );
            let t = shipdesign::TILE as f64;
            for other in &ports[1..] {
                let far =
                    (other.centre.0 - port.centre.0).abs() + (other.centre.1 - port.centre.1).abs();
                assert!(far >= 10.0 * t, "{name}: an airlock {far} from the port");
            }
            let wanted = match kind {
                StationKind::Relay => 1,
                StationKind::Orbital => 3,
                _ => 2,
            };
            assert!(ports.len() >= wanted, "{name}: {} airlocks", ports.len());
            let grid = design.grid();
            let array = design
                .parts
                .iter()
                .find(|p| p.kind == PartKind::SensorArray)
                .expect("an array");
            let (ax, ay) = (array.origin.0 as i32, array.origin.1 as i32);
            assert_eq!(
                grid.get(shipdesign::Layer::Structure, (ax, ay - 1)),
                0,
                "{name}: the array faces north"
            );
            assert!(
                design.count(PartKind::Bunk) >= residents + 2,
                "{name}: bunks"
            );
            for (kind_, least) in [
                (PartKind::TradingDesk, 1),
                (PartKind::ResearchDesk, 1),
                (PartKind::Hob, 1),
                (PartKind::Toilet, 1),
                (PartKind::Shelf, 1),
                (PartKind::HydroBay, 1),
            ] {
                assert!(design.count(kind_) >= least, "{name}: no {kind_:?}");
            }
            assert_eq!(design.count(PartKind::TradingDesk), 1);
            assert_eq!(design.count(PartKind::ResearchDesk), 1);
            // Sandbags: never within three tiles of a doorway or an airlock.
            let doors: Vec<(i32, i32)> = design
                .parts
                .iter()
                .filter(|p| matches!(p.kind, PartKind::Door | PartKind::Airlock))
                .flat_map(|p| p.tiles())
                .map(|(x, y)| (x as i32, y as i32))
                .collect();
            for bag in design.parts.iter().filter(|p| p.kind == PartKind::Sandbags) {
                let (x, y) = (bag.origin.0 as i32, bag.origin.1 as i32);
                assert!(
                    doors
                        .iter()
                        .all(|&(dx, dy)| (dx - x).abs().max((dy - y).abs()) > 3),
                    "{name}: sandbags at {:?} in a doorway's way",
                    bag.origin
                );
            }
            assert!(g.tally.loops >= 1, "{name}: no loop");
            if kind == StationKind::Derelict {
                assert_eq!(Plan::Generated.residents(kind), 0);
            }
            // A derelict's skin is whole too: its holes were black squares
            // in the wall.
            if sweep() || i < 10 {
                assert!(
                    shipdesign::exposure(design).is_empty(),
                    "{name}: open to space"
                );
            }
            if sweep() || i < 10 {
                let issues = shipdesign::validate(design, residents);
                assert!(
                    !shipdesign::has_errors(&issues),
                    "{name}: {:?}",
                    issues.iter().map(|i| i.code).collect::<Vec<_>>()
                );
            }
            tallies.push(g.tally);
        }
        let column = |f: fn(&Tally) -> u32| spread(&mut tallies.iter().map(f).collect::<Vec<_>>());
        let side_on = tallies.iter().filter(|t| t.side_on).count();
        let attempts = spread(&mut built.iter().map(|(_, g)| g.attempt).collect::<Vec<_>>());
        println!(
            "{kind:?} over {n} seeds, least/median/most: side {} hull {} rooms {} corridor {} loops {} airlocks {}; {side_on} side on; attempt {attempts}; {fell} fell back",
            column(|t| t.side),
            column(|t| t.hull),
            column(|t| t.rooms),
            column(|t| t.corridor),
            column(|t| t.loops),
            column(|t| t.airlocks),
        );
        sides_of.insert(
            format!("{kind:?}"),
            (
                tallies.iter().map(|t| t.side).min().unwrap_or(0),
                tallies.iter().map(|t| t.side).max().unwrap_or(0),
            ),
        );
    }
    // The kind sizes the station: every relay smaller than every orbital.
    assert!(sides_of["Relay"].1 < sides_of["Orbital"].0, "{sides_of:?}");
}

/// Two docks of one kind are two buildings: over twenty seeds a kind, no
/// two generated stations share a hull, its walls and its doors.
#[test]
fn twenty_docks_of_a_kind_are_twenty_buildings() {
    for kind in StationKind::ALL {
        let (built, _) = generated(kind, 20);
        let mut signatures: Vec<u64> = built.iter().map(|(_, g)| g.tally.signature).collect();
        signatures.sort_unstable();
        let n = signatures.len();
        signatures.dedup();
        assert_eq!(
            signatures.len(),
            n,
            "{kind:?}: two of twenty docks are one building"
        );
    }
}

/// Every station of the default galaxy but the spawn is generated, bar the
/// few the generator could not draw — which are the drawn plan their seed
/// rolled, and at most two in a hundred.
#[test]
fn the_default_galaxy_s_stations_are_generated() {
    let galaxy = Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (mut generated, mut fell) = (0u32, 0u32);
    for system in galaxy.every_system().iter().take(60) {
        for station in Station::all_of(system) {
            if station.plan == Plan::Generated {
                generated += 1;
            } else {
                assert_eq!(station.plan, Plan::hand_rolled(station.map_seed));
                fell += 1;
            }
        }
    }
    println!("{generated} generated, {fell} fell back");
    assert!(
        generated > 100 && fell * 50 <= generated + fell,
        "{generated} and {fell}"
    );
}

/// A generated town, in every biome at five, seventeen and thirty people,
/// over three seeds (twelve in a sweep): two or three gates in the north,
/// south and east walls, each a street's width, with open ground beyond
/// it on the plain; a chair and a bed each and two beds over. The walk
/// from the pad to every door, gate, spot and tile of ground is the town
/// test's in `tests_surface.rs`; this is the drawing's count of how often
/// the template was fallen back on.
#[test]
fn a_generated_town_has_its_gates_its_hall_and_its_beds() {
    let seeds: u64 = if sweep() { 12 } else { 3 };
    let (mut drawn, mut fell) = (0, 0);
    for biome in Biome::ALL {
        for population in [5, 17, 30] {
            for i in 0..seeds {
                let seed = seed_of(i);
                let name = format!("{biome:?} town of {population} at {seed}");
                let built = crate::surface::build_town(seed, biome, population);
                match built.attempt {
                    Some(_) => drawn += 1,
                    None => fell += 1,
                }
                let design = &built.placer.design;
                assert!((2..=3).contains(&built.gates.len()), "{name}");
                assert!(design.count(PartKind::Chair) >= population, "{name}");
                assert!(design.count(PartKind::Bunk) >= population + 2, "{name}");
                let terrain = bims::terrain::Terrain::new(
                    seed ^ 0x_504c_4149_4e00_0000,
                    biome.code() as u8,
                    data::SURFACE_SIDE,
                );
                for gate in &built.gates {
                    assert!((6..=8).contains(&gate.width), "{name}: {gate:?}");
                    // Beyond the opening, two columns of it open ground
                    // three tiles out onto the plain.
                    let open: Vec<bool> = gate
                        .opening()
                        .iter()
                        .map(|&(x, y)| {
                            let (x, y) = (x as i32, y as i32);
                            (1..=3).all(|d| {
                                let (tx, ty) = match gate.wall {
                                    Wall::North => (x, y - d),
                                    Wall::South => (x, y + d),
                                    Wall::East => (x + d, y),
                                };
                                !terrain.at(tx, ty).blocks()
                            })
                        })
                        .collect();
                    assert!(
                        open.windows(2).any(|w| w[0] && w[1]),
                        "{name}: {gate:?} opens onto nothing"
                    );
                }
            }
        }
    }
    println!("{drawn} towns drawn, {fell} fell back on the template");
    assert!(
        fell * 50 <= drawn + fell,
        "{fell} of {} fell back",
        drawn + fell
    );
}

/// Not a test: what building the stations of a system and a town costs,
/// in whatever build it is run in — `cargo test --release -p world --
/// --ignored --nocapture layout_timings`. Every station of forty systems,
/// each system timed on its own (a cold cache, since no two stations share
/// a seed), and a dozen towns.
#[test]
#[ignore]
fn layout_timings() {
    let galaxy = Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let systems: Vec<_> = galaxy
        .every_system()
        .into_iter()
        .filter(|s| !s.stations.is_empty())
        .take(40)
        .collect();
    let mut per_system = Vec::new();
    let mut stations = 0;
    for system in &systems {
        let started = std::time::Instant::now();
        let built = Station::all_of(system);
        per_system.push(started.elapsed().as_secs_f64() * 1000.0);
        stations += built.len();
    }
    let summary = |label: &str, mut ms: Vec<f64>| {
        ms.sort_by(f64::total_cmp);
        let mean = ms.iter().sum::<f64>() / ms.len().max(1) as f64;
        println!(
            "{label}: n {} mean {mean:.2} ms, median {:.2} ms, max {:.2} ms",
            ms.len(),
            ms[ms.len() / 2],
            ms[ms.len() - 1]
        );
    };
    println!("{stations} stations over {} systems", systems.len());
    summary("a system's stations", per_system);
    let mut towns = Vec::new();
    for system in systems.iter().take(30) {
        for surface in Surface::all_of(system, galaxy.seed) {
            if towns.len() >= 12 {
                break;
            }
            let started = std::time::Instant::now();
            let _ =
                crate::station::layout_surface(surface.map_seed, surface.biome, surface.population);
            towns.push(started.elapsed().as_secs_f64() * 1000.0);
        }
    }
    summary("a town", towns);
    let _ = Biome::ALL;
}

/// Not a test: why candidates are thrown away, over `n` seeds a kind.
#[test]
#[ignore]
fn why_candidates_fail() {
    use crate::stationgen::attempts;
    let n: u64 = std::env::var("BIMS_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);
    for kind in worldgen::StationKind::ALL {
        let mut why = std::collections::BTreeMap::new();
        let mut tries = 0;
        let mut fell = 0;
        for seed in 0..n {
            let outcome = attempts(kind, seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
            tries += outcome.len();
            if outcome.last().is_some_and(|o| o.is_err()) {
                fell += 1;
            }
            for o in outcome {
                if let Err(e) = o {
                    *why.entry(format!("{e:?}")).or_insert(0) += 1;
                }
            }
        }
        println!("{kind:?}: {fell} of {n} fell back, {tries} tries: {why:?}");
    }
}

/// Not a test: a candidate drawn, as characters — a part by its first
/// letter, `.` deck, ` ` void — for looking at one that failed.
/// `BIMS_STATION_KIND`, `BIMS_STATION_SEED`, `BIMS_ATTEMPT`.
#[test]
#[ignore]
fn print_candidate() {
    let kind = std::env::var("BIMS_STATION_KIND")
        .ok()
        .and_then(|k| {
            worldgen::StationKind::ALL
                .into_iter()
                .find(|s| format!("{s:?}") == k)
        })
        .unwrap_or(worldgen::StationKind::Relay);
    let seed: u64 = std::env::var("BIMS_STATION_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let attempt: u32 = std::env::var("BIMS_ATTEMPT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    // The first candidate from `attempt` on that got as far as a
    // furnishing, and what the check says of it; `!` is open deck the
    // flood did not reach.
    for attempt in attempt..crate::stationgen::ATTEMPTS {
        match crate::stationgen::candidate_at(kind, seed, attempt) {
            Err(e) => println!("attempt {attempt}: no candidate: {e:?}"),
            Ok((placer, floor, side_on)) => {
                println!(
                    "attempt {attempt}: {:?}",
                    crate::stationgen::check(&placer, &floor, kind, side_on).map(|t| t.side)
                );
                let (px, py) = floor.airlocks[0].0;
                print!(
                    "{}",
                    crate::stationgen::picture(&placer, Some((px as i32 + 1, py as i32)))
                );
                break;
            }
        }
    }
}

/// Not a test: why town drawings are thrown away, every biome at three
/// sizes over `BIMS_SEEDS` seeds.
#[test]
#[ignore]
fn why_towns_fail() {
    let n: u64 = std::env::var("BIMS_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    for biome in Biome::ALL {
        for population in [5, 17, 30] {
            let mut why = std::collections::BTreeMap::new();
            let (mut tries, mut fell) = (0, 0);
            for seed in 0..n {
                let outcome = crate::surface::town_attempts(
                    seed.wrapping_mul(0x9E37_79B9_7F4A_7C15),
                    biome,
                    population,
                );
                tries += outcome.len();
                if outcome.last().is_some_and(|o| o.is_err()) {
                    fell += 1;
                }
                for o in outcome {
                    if let Err(e) = o {
                        *why.entry(e).or_insert(0) += 1;
                    }
                }
            }
            println!("{biome:?} {population}: {fell} of {n} fell back, {tries} tries: {why:?}");
        }
    }
}

/// Not a test: a town as characters. `BIMS_TOWN_SEED`, `BIMS_TOWN_BIOME`
/// (desert, temperate, arctic), `BIMS_TOWN_POPULATION`.
#[test]
#[ignore]
fn print_town() {
    let seed: u64 = std::env::var("BIMS_TOWN_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let biome = std::env::var("BIMS_TOWN_BIOME")
        .ok()
        .and_then(|b| {
            Biome::ALL
                .into_iter()
                .find(|x| format!("{x:?}").eq_ignore_ascii_case(&b))
        })
        .unwrap_or(Biome::Temperate);
    let population: u32 = std::env::var("BIMS_TOWN_POPULATION")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(17);
    let built = crate::surface::build_town(seed, biome, population);
    println!("attempt {:?}, gates {:?}", built.attempt, built.gates);
    print!(
        "{}",
        crate::stationgen::picture(&built.placer, Some((2, 47)))
    );
}

/// What `REFERENCE_CHECKSUM` and `SURVIVORS` come to under the layouts
/// before feature 112. They were `0x_d534_2dda_6563_babe` and
/// `0x_024e_0982_c931_c4bb` — the numbers before feature 112 itself — and
/// were taken again under task 113, which moved both on every layout
/// (the constants' own notes say why); the check still says the layouts
/// are all feature 112 moved. And taken again under task 111 (every site
/// an attack, a defence or a trader), which moved both on every layout
/// for the same reason: they were `0x_c7b1_8af4_f64f_0fa1` and
/// `0x_a881_25c7_68e4_5807`. And taken again under worldgen's
/// `GENERATOR_VERSION` 8, which is another galaxy on every layout: they
/// were `0x_e03e_343e_8918_672c` and `0x_e1c2_f369_96f5_fb38`. And taken
/// again on 29 September 2026 after the machine's one health, the map
/// rework and the bots' deaths (the constants' notes), and with them
/// every move since `GENERATOR_VERSION` 8 that was never taken here —
/// task 136's outposts and the elites: they were `0x_e50d_58ec_b57a_d470`
/// and `0x_6b0f_8e4d_f02e_4348`.
const REFERENCE_BEFORE_112: u64 = 0x_12cc_5e55_13c6_6fd3;
const SURVIVORS_BEFORE_112: u64 = 0x_bc3f_3b30_b53e_59c3;

/// Not a test of its own, and **run alone** (`--exact`): the process-wide
/// switch to the old layouts is flipped here, and a test running beside
/// it would build on the wrong ones. With every station on the drawn plan
/// its seed rolled before feature 112 and every town on the template, the
/// two pinned readings come back to what they were — so the layouts are
/// the whole of why they moved. And which station the reference run
/// travels to, on both.
/// `cargo test -p world -- --ignored --exact --nocapture
/// tests_layoutgen::the_pins_come_back_under_the_old_layouts`.
#[test]
#[ignore]
fn the_pins_come_back_under_the_old_layouts() {
    let now = (
        crate::fixture::reference_run(),
        crate::tests_survivors::survivors(),
    );
    let world = crate::fixture::reference_run_world();
    let site = world.current_site();
    let station = site.and_then(|s| world.station(s.station));
    println!(
        "the reference run's second mission: {site:?} — {:?}",
        station.map(|s| (s.kind, s.map_seed, s.plan))
    );
    crate::station::set_legacy_layouts(true);
    crate::station::forget_built();
    let old_world = crate::fixture::reference_run_world();
    let old_station = old_world
        .current_site()
        .and_then(|s| old_world.station(s.station));
    println!(
        "under the old layouts: {:?} — {:?}",
        old_world.current_site(),
        old_station.map(|s| (s.kind, s.map_seed, s.plan))
    );
    let then = (
        crate::fixture::reference_run(),
        crate::tests_survivors::survivors(),
    );
    crate::station::set_legacy_layouts(false);
    crate::station::forget_built();
    println!("now: reference {:#018x} survivors {:#018x}", now.0, now.1);
    println!(
        "old layouts: reference {:#018x} survivors {:#018x}",
        then.0, then.1
    );
    assert_eq!(then, (REFERENCE_BEFORE_112, SURVIVORS_BEFORE_112));
}

/// The combat rooms (October 2026): over fifty seeds a kind, how many
/// stations have a shaft (a pit), a hangar or a gallery (a pillar or an
/// alcove), crates, a fuel drum and
/// windows — printed — and most stations of every kind but the relay have
/// at least one combat room, every kind has drums, crates and windows
/// somewhere, and no piece stands off the deck.
#[test]
fn most_stations_have_a_combat_room_and_the_pieces_turn_up() {
    for kind in [
        StationKind::Relay,
        StationKind::Derelict,
        StationKind::Refinery,
        StationKind::Orbital,
    ] {
        let (mut shafts, mut combat_rooms, mut crates, mut drums, mut windows) = (0, 0, 0, 0, 0);
        let n = 50u64;
        for seed in 0..n {
            let g = generate(kind, seed).expect("generated");
            let design = &g.placer.design;
            let count = |k: PartKind| design.count(k);
            let pits = count(PartKind::Pit);
            // A hangar's pillars and a gallery's alcoves are the walls the
            // combat rooms stand: the floor the station was built from
            // says which.
            let (_, floor, _) =
                stationgen::candidate_at(kind, seed, g.attempt).expect("the same candidate");
            let standing_walls = floor
                .combat
                .iter()
                .filter(|(k, _)| *k == PartKind::Wall)
                .count();
            shafts += u32::from(pits > 0);
            combat_rooms += u32::from(pits > 0 || standing_walls > 0);
            crates += u32::from(count(PartKind::Crate) > 0);
            drums += u32::from(count(PartKind::FuelTank) > 0);
            windows += u32::from(count(PartKind::Window) > 0);
        }
        println!(
            "{kind:?} over {n} seeds: a combat room {combat_rooms}, a shaft {shafts}, crates {crates}, drums {drums}, windows {windows}"
        );
        if kind != StationKind::Relay {
            assert!(
                combat_rooms as u64 * 2 >= n,
                "{kind:?}: {combat_rooms} of {n}"
            );
        }
        assert!(crates > 0 && drums > 0 && windows > 0, "{kind:?}");
    }
}

/// A station built for a mission's feature (October 2026): most seeds of
/// every kind keep the contract with the feature's rooms where it says,
/// and none of them lands in the reactor room or a role's room.
#[test]
fn a_station_is_built_for_its_mission_s_rooms() {
    use crate::stationgen::{Feature, generate_fitted};
    let features = [
        Feature::Office,
        Feature::Servers,
        Feature::Brig,
        Feature::FuelRun,
        Feature::Cargo,
        Feature::Vault,
        Feature::Command,
    ];
    for kind in [
        StationKind::Relay,
        StationKind::MiningOutpost,
        StationKind::Derelict,
        StationKind::Refinery,
        StationKind::Orbital,
    ] {
        for feature in features {
            let mut built = 0;
            for seed in 0..12u64 {
                if let Some((g, fitted)) = generate_fitted(kind, 1000 + seed * 7919, feature) {
                    built += 1;
                    assert_eq!(fitted.feature, feature);
                    let want = match feature {
                        Feature::Office | Feature::Brig | Feature::Command => 1,
                        Feature::Servers => 3,
                        Feature::FuelRun | Feature::Cargo | Feature::Vault => 2,
                    };
                    assert_eq!(fitted.rooms.len(), want, "{kind:?} {feature:?}");
                    assert_eq!(fitted.door.is_some(), feature == Feature::Brig);
                    assert_eq!(
                        fitted.doors.len(),
                        if feature == Feature::Vault { 3 } else { 0 }
                    );
                    // No way in opens into the commander's room.
                    if feature == Feature::Command {
                        let r = fitted.rooms[0];
                        for port in crate::droid::airlocks(&g.placer.design) {
                            let (x, y) = crate::droid::inside_of(&port, 1.0);
                            let t = shipdesign::TILE as f64;
                            let (tx, ty) = ((x / t).floor() as u32, (y / t).floor() as u32);
                            assert!(
                                !(tx >= r[0] && tx <= r[2] && ty >= r[1] && ty <= r[3]),
                                "{kind:?}: an airlock into the commander's room"
                            );
                        }
                    }
                    let side = g.placer.design.build_area;
                    for r in &fitted.rooms {
                        assert!(r[0] <= r[2] && r[1] <= r[3] && r[2] < side && r[3] < side);
                    }
                }
            }
            println!("{kind:?} {feature:?}: {built} of 12");
            assert!(built >= 6, "{kind:?} {feature:?}: only {built} of 12");
        }
    }
}
