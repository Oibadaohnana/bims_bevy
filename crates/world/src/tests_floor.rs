//! The floor (October 2026, `crate::floor`): the run's map, the start at
//! the bottom and the Machine Heart at the top, a row a hop and a day, two
//! to four ways up, traders scattered over it, the tiers marked by their days —
//! and a trip only up it.

use shipdesign::fixture::flyer;
use worldgen::GalaxyType;

use crate::data;
use crate::droid::WaveScaling;
use crate::event::{Refusal, WorldEvent};
use crate::fixture::REFERENCE_MONEY;
use crate::floor;
use crate::heart;
use crate::run::{Site, SiteKind};
use crate::world::{Command, World};
use bims::combat::Tier;

/// A run opened at `seed`'s spawn with the floor on, the sites quiet so
/// a trip's fight is nobody's subject here.
fn floor_world(seed: u64, galaxy_type: GalaxyType) -> Option<World> {
    let galaxy = worldgen::Galaxy::new(seed, galaxy_type);
    let (star, station) = crate::spawn(&galaxy)?;
    let mut world = World::start(
        flyer(2),
        REFERENCE_MONEY,
        1,
        seed,
        galaxy_type,
        star,
        station,
    )
    .ok()?;
    world.set_quiet_sites_for_probe(true);
    world.set_floor(true);
    Some(world)
}

fn default_world() -> World {
    floor_world(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm).expect("the default seed has a dock")
}

/// The one player's proposal: the trip, carried at once.
fn travel_to(world: &mut World, site: Site) -> Vec<WorldEvent> {
    world.step(&[Command::Propose {
        slot: 0,
        star: site.star,
        station: site.station,
    }])
}

fn travelled(events: &[WorldEvent]) -> bool {
    events
        .iter()
        .any(|e| matches!(e, WorldEvent::Travelled { .. }))
}

#[test]
fn a_run_s_floor_climbs_from_home_to_the_heart_on_stars_of_its_own() {
    let world = default_world();
    let floor = world.floor().expect("the floor is on");
    assert_eq!(floor.rows.len() as u32, data::FLOOR_HOPS + 1);
    let start = &floor.rows[0];
    assert_eq!(start.len(), 1);
    assert_eq!(
        (start[0].star, start[0].station),
        (world.home_star, world.home)
    );
    let top = floor.rows.last().unwrap();
    assert_eq!(top.len(), 1);
    assert_eq!(top[0].star, world.droid_origin());
    assert!(heart::is_heart(top[0].station));
    // Every place between on a star of its own, neither home nor the
    // origin.
    let mut stars: Vec<u32> = floor.rows.iter().flatten().map(|n| n.star).collect();
    let all = stars.len();
    stars.sort_unstable();
    stars.dedup();
    assert_eq!(stars.len(), all, "no star twice");
    // The traders' places traders, the rest none.
    for (row, nodes) in floor.rows.iter().enumerate() {
        for node in nodes {
            let shop = node.shop;
            let site = Site {
                star: node.star,
                station: node.station,
            };
            if row > 0 && row < floor.rows.len() - 1 {
                assert_eq!(world.is_trader(site), shop, "row {row}: {site:?}");
            }
        }
    }
    // The marks say the same on each row's day: a trader still trading.
    let marks = world.floor_marks();
    assert_eq!(marks.len(), floor.len(), "every place marked");
    for mark in &marks {
        if floor.is_shop(mark.row, mark.index as usize) {
            assert_eq!(mark.kind, SiteKind::Trader, "{mark:?}");
        } else if mark.row > 0 {
            assert_ne!(mark.kind, SiteKind::Trader, "{mark:?}");
        }
    }
    assert!(marks.last().unwrap().heart);
    // The row under the Heart one place, a trader still trading on its
    // day: the crew can always spend before the Heart.
    let last = marks
        .iter()
        .filter(|m| m.row == data::FLOOR_HOPS - 1)
        .collect::<Vec<_>>();
    assert_eq!(last.len(), 1, "one place under the Heart");
    assert_eq!(last[0].kind, SiteKind::Trader, "{:?}", last[0]);
}

#[test]
fn two_worlds_on_one_seed_lay_the_same_floor_and_a_load_lays_it_again() {
    let a = default_world();
    let mut b = default_world();
    assert_eq!(a.floor(), b.floor());
    b.set_floor(false);
    assert!(b.floor().is_none());
    assert!(b.floor_next().is_empty());
    b.set_floor(true);
    assert_eq!(a.floor(), b.floor());
    let mut c = a.clone();
    c.settle_crisis();
    assert_eq!(a.floor(), c.floor());
}

#[test]
fn a_trip_goes_only_up_the_floor_and_a_row_is_a_day() {
    let mut world = default_world();
    assert_eq!(world.floor_at(), Some((0, 0)), "the run opens at the start");
    assert_eq!(world.run_day(), 1);
    world.leave_for_probe();
    let floor = world.floor().unwrap().clone();
    // From the start, the whole first row and nothing else.
    let first: Vec<Site> = floor.rows[1]
        .iter()
        .map(|n| Site {
            star: n.star,
            station: n.station,
        })
        .collect();
    assert_eq!(world.floor_next(), first);
    let second = Site {
        star: floor.rows[2][0].star,
        station: floor.rows[2][0].station,
    };
    assert_eq!(world.travel_quote(second).err(), Some(Refusal::TooFar));
    for site in &first {
        assert!(world.destinations().contains(site));
    }
    // The first row is fought on day one, the start's own.
    let quote = world.travel_quote(first[0]).expect("a trip up");
    assert_eq!(quote.minutes, 0);
    assert!(travelled(&travel_to(&mut world, first[0])));
    assert_eq!(world.floor_at(), Some((1, 0)));
    assert_eq!(world.run_day(), 1);
    // Up a row a day, only where the place leads; never back down, and
    // never sideways.
    for row in 1..4u32 {
        if world.in_mission() {
            world.leave_for_probe();
        }
        let (at, index) = world.floor_at().unwrap();
        assert_eq!(at, row);
        let next = world.floor_next();
        let leads: Vec<Site> = floor
            .next(at, index)
            .into_iter()
            .map(|(r, j)| {
                let n = floor.node(r, j).unwrap();
                Site {
                    star: n.star,
                    station: n.station,
                }
            })
            .collect();
        assert_eq!(next, leads);
        let home = Site {
            star: world.home_star,
            station: world.home,
        };
        assert_eq!(world.travel_quote(home).err(), Some(Refusal::TooFar));
        for (j, n) in floor.rows[at as usize].iter().enumerate() {
            if j != index {
                let beside = Site {
                    star: n.star,
                    station: n.station,
                };
                assert_eq!(world.travel_quote(beside).err(), Some(Refusal::TooFar));
            }
        }
        for (j, n) in floor.rows[at as usize + 1].iter().enumerate() {
            let above = Site {
                star: n.star,
                station: n.station,
            };
            let leads_there = floor.node(at, index).unwrap().up.contains(&(j as u8));
            assert_eq!(world.travel_quote(above).is_ok(), leads_there);
        }
        let quote = world.travel_quote(next[0]).unwrap();
        assert_eq!(quote.minutes, data::JUMP_MINUTES, "a row is a day");
        assert!(travelled(&travel_to(&mut world, next[0])));
        assert_eq!(world.run_day(), row + 1);
    }
}

#[test]
fn the_rows_are_marked_by_the_tier_timings() {
    let mut world = default_world();
    world.set_wave_scaling(WaveScaling {
        tier2_days: 19,
        tier3_days: 29,
        ..WaveScaling::DEFAULT
    });
    let heart = world.floor().unwrap().heart_row();
    let tiers: Vec<Tier> = (0..=heart).map(|row| world.floor_tier(row)).collect();
    let count = |t: Tier| tiers[1..].iter().filter(|&&x| x == t).count();
    assert_eq!(world.floor_tier(18), Tier::One);
    assert_eq!(world.floor_tier(19), Tier::Two);
    assert_eq!(world.floor_tier(28), Tier::Two);
    assert_eq!(world.floor_tier(29), Tier::Three);
    assert_eq!(world.floor_tier(heart), Tier::Three);
    assert_eq!(
        (count(Tier::One), count(Tier::Two), count(Tier::Three)),
        (18, 10, 4)
    );
    // A quote up the floor says its row's tier.
    world.leave_for_probe();
    let first = world.floor_next()[0];
    assert_eq!(world.travel_quote(first).unwrap().tier, Tier::One);
    assert_eq!(floor::row_day(37), 37);
}

/// What the floors of ten seeds over every galaxy type are made of: how
/// many places, how many stars twice, the traders' places that fell to the
/// machines, and the attacks and defences on their days. Run with
/// `--ignored --nocapture`.
#[test]
#[ignore]
fn floors_over_ten_seeds() {
    for galaxy_type in GalaxyType::ALL {
        for n in 0..10u64 {
            let seed = data::DEFAULT_SEED.wrapping_add(n * 0x9E37);
            let Some(world) = floor_world(seed, galaxy_type) else {
                continue;
            };
            let floor = world.floor().unwrap();
            let mut stars: Vec<u32> = floor.rows.iter().flatten().map(|n| n.star).collect();
            let all = stars.len();
            stars.sort_unstable();
            stars.dedup();
            let marks = world.floor_marks();
            let count = |k: SiteKind| marks.iter().filter(|m| m.kind == k).count();
            let fallen = marks
                .iter()
                .filter(|m| floor.is_shop(m.row, m.index as usize) && m.kind != SiteKind::Trader)
                .count();
            let elites = marks.iter().filter(|m| m.elite).count();
            println!(
                "{galaxy_type:?} {n}: {all} places, {} twice, attack {} defend {} trader {} ({fallen} fallen), elite {elites}, home {} hops off",
                all - stars.len(),
                count(SiteKind::Attack),
                count(SiteKind::Defend),
                count(SiteKind::Trader),
                world.hops_from_origin(world.home_star),
            );
        }
    }
}
