//! The front (feature 94): how far outside the infection a system is,
//! what that does to a desk's prices, and what it does to the hiring.
//!
//! All three are arithmetic off the day and the hop table the crisis
//! already keeps — nothing is saved and nothing is hashed here — so what
//! these tests hold is the arithmetic and the one door every price goes
//! through.

use economy::market;
use physics::ResourceId;
use shipdesign::fixture::flyer;

use crate::data;
use crate::fixture::{REFERENCE_MONEY, simulation_world};
use crate::mercenary::MERCENARIES_MAX;
use crate::world::{Command, World};

fn basic() -> World {
    simulation_world(flyer(2), REFERENCE_MONEY, 2)
}

/// Put the machines' origin exactly `hops` lane hops from the crew's own
/// star, wind the first day to nought and the clock to day nought: the
/// infection is the origin alone and the crew's system is `hops` out.
/// `false` when this galaxy has no star at that distance.
fn origin_at(world: &mut World, hops: u16) -> bool {
    let table = world.start_star_hops_for_probe();
    let Some(star) = (0..table.len() as u32).find(|&s| table[s as usize] == hops) else {
        return false;
    };
    world.set_crisis_first_day_for_probe(0);
    world.set_droid_origin_for_probe(star);
    world.set_day_for_probe(0);
    assert_eq!(world.crisis_radius(), Some(0));
    assert_eq!(world.hops_from_origin(world.star_id), hops);
    true
}

/// The front is nothing before the first day — which only the probes'
/// dial puts anywhere but day nought (feature 102) — one hop out on the
/// edge of the infection, and it counts down as the infection spreads.
#[test]
fn the_front_is_there_from_day_nought_and_follows_the_day() {
    let mut world = basic();
    let galaxy = world.galaxy();
    let origin = world.droid_origin();
    let out = galaxy.hops_from(origin);

    // A crisis wound ten days off holds nothing before its day, so nowhere
    // is the front — not even the origin's own system.
    world.set_crisis_first_day_for_probe(10);
    world.set_day_for_probe(9);
    assert_eq!(world.crisis_radius(), None);
    assert_eq!(world.front(origin), None);
    for star in 0..galaxy.stars.len() as u32 {
        assert_eq!(world.front(star), None, "star {star} before the first day");
    }

    // Every run's crisis is there from day nought: the origin is theirs —
    // an infested star has no front — and every other star is its own
    // hop count out.
    world.set_crisis_first_day_for_probe(0);
    world.set_day_for_probe(0);
    assert_eq!(world.crisis_radius(), Some(0));
    assert_eq!(world.front(origin), None, "the origin is theirs");
    let one = (0..galaxy.stars.len() as u32)
        .find(|&s| out[s as usize] == 1)
        .expect("a star one hop from the origin");
    assert_eq!(world.front(one), Some(1));

    // Five days on the infection has taken that ring, so the star one hop
    // out is theirs and the star two hops out is the edge.
    world.set_day_for_probe(data::DROID_SPREAD_DAYS);
    assert_eq!(world.crisis_radius(), Some(1));
    assert_eq!(world.front(one), None, "one hop out has fallen");
    for star in 0..galaxy.stars.len().min(60) as u32 {
        let hops = out[star as usize];
        let want = if hops == u16::MAX || hops <= 1 {
            None
        } else {
            Some(hops - 1)
        };
        assert_eq!(world.front(star), want, "star {star}, {hops} hops out");
    }

    // A star no lane reaches is never theirs and so never has a front.
    assert_eq!(world.front(galaxy.stars.len() as u32), None);

    // And the radius is the arithmetic it says it is, day after day.
    for day in 0..40u32 {
        world.set_day_for_probe(day);
        assert_eq!(
            world.crisis_radius(),
            Some((day / data::DROID_SPREAD_DAYS) as u16),
            "day {day} of the crisis"
        );
    }
}

/// The premium: fifteen per cent on the edge of the infection, ten two
/// hops out, five three hops out and nothing beyond — on a weapon, a
/// piece of armour, a medkit or a bandage, and on nothing else.
#[test]
fn a_desk_near_the_front_leans_on_the_guns_the_armour_and_the_medicine() {
    // The two numbers together must stay inside what `economy::market`
    // promises to quote a sound price over.
    assert!(
        data::FRONT_BIAS * i32::from(data::FRONT_HOPS) <= market::MAX_FRONT_BIAS,
        "the front premium is bigger than the market's own bound"
    );

    let mut world = basic();
    let home = world.home;
    // Beyond the front — the crisis has not begun — nothing leans.
    for &resource in ResourceId::ALL.iter() {
        assert_eq!(world.front_bias(home, resource), 0, "{resource:?}");
    }
    assert_eq!(world.front_at(home), None);

    for hops in 1..=4u16 {
        if !origin_at(&mut world, hops) {
            continue;
        }
        let want = if hops <= data::FRONT_HOPS {
            data::FRONT_BIAS * i32::from(data::FRONT_HOPS + 1 - hops)
        } else {
            0
        };
        assert_eq!(
            world.front_at(home),
            (hops <= data::FRONT_HOPS).then_some(hops),
            "{hops} hops out"
        );
        for &resource in ResourceId::ALL.iter() {
            let got = world.front_bias(home, resource);
            if market::war_goods(resource) {
                assert_eq!(got, want, "{resource:?} at {hops} hops");
            } else {
                assert_eq!(got, 0, "{resource:?} is not war goods");
            }
        }
    }
    // The three figures the feature was asked for, written out.
    if origin_at(&mut world, 1) {
        assert_eq!(world.front_bias(home, ResourceId::Medkit), 15);
    }
    if origin_at(&mut world, 2) {
        assert_eq!(world.front_bias(home, ResourceId::Medkit), 10);
    }
    if origin_at(&mut world, 3) {
        assert_eq!(world.front_bias(home, ResourceId::Medkit), 5);
    }
}

/// Every quote goes through one function, and the front premium is on
/// top of the station's own rolled lean: the desk's own arithmetic, what
/// `World::quote` answers and what a sale actually pays all agree.
#[test]
fn every_quote_path_agrees_and_a_sale_pays_the_front_price() {
    let mut world = basic();
    world.set_shipyard_enabled(true);
    let home = world.home;
    let desk = world.station(home).unwrap().market().unwrap();

    // Away from the front, the one door answers exactly what the desk's
    // own kind and lean do.
    for &resource in ResourceId::ALL.iter() {
        assert_eq!(
            world.quote(home, resource),
            Some(market::quote(desk.kind, desk.bias.of(resource), resource)),
            "{resource:?} away from the front"
        );
    }
    let quiet = world.quote(home, ResourceId::Medkit).unwrap();

    // One hop out, the premium is added to the roll — and the roll is
    // still in there, since the sum is the one that is quoted.
    assert!(origin_at(&mut world, 1), "a star one hop from the crew");
    let front = world.quote(home, ResourceId::Medkit).unwrap();
    assert_eq!(
        front,
        market::quote(
            desk.kind,
            desk.bias.of(ResourceId::Medkit) + 15,
            ResourceId::Medkit
        )
    );
    assert!(front.bid > quiet.bid, "{front:?} over {quiet:?}");
    assert!(front.ask > quiet.ask, "{front:?} over {quiet:?}");
    // And a resource that is not war goods is untouched by all of it.
    assert_eq!(
        world.quote(home, ResourceId::Vegetable),
        Some(market::quote(
            desk.kind,
            desk.bias.of(ResourceId::Vegetable),
            ResourceId::Vegetable
        ))
    );

    // What a sale pays is that bid, to the euro. Nobody stocks a medkit
    // on the shelf the crew can *buy* it off everywhere, but a desk buys
    // what is brought to it, and that is the direction the front is felt
    // in: a crew selling its kit near the machines is paid over the odds.
    world.ship.design.cargo[ResourceId::Medkit as usize] += 4;
    world.on_ship_changed();
    assert!(world.man_the_desk_for_probe(0), "a desk to trade at");
    let money = world.money;
    world.step(&[Command::Sell {
        slot: 0,
        resource: ResourceId::Medkit,
        units: 4,
    }]);
    assert_eq!(world.money, money + 4 * front.bid);
    assert!(
        world.money > money + 4 * quiet.bid,
        "the front price is over the quiet one"
    );

    // And a buy of something the shelf stocks goes through the same door.
    // A shelf first, since the flyer carries nowhere to put it.
    world.ship.design = shipdesign::apply(
        &world.ship.design,
        &shipdesign::Budget::new(10_000_000),
        shipdesign::Edit::Place {
            kind: shipdesign::parts::PartKind::Shelf,
            origin: (7, 9),
            rotation: shipdesign::Rotation::R0,
        },
    )
    .unwrap();
    world.on_ship_changed();
    let stocked = ResourceId::ALL
        .iter()
        .copied()
        .find(|&r| world.station(home).unwrap().stock.sells(r))
        .expect("the spawn stocks something");
    let quote = world.quote(home, stocked).unwrap();
    let money = world.money;
    assert!(world.man_the_desk_for_probe(0));
    let events = world.step(&[Command::Buy {
        slot: 0,
        resource: stocked,
        units: 1,
        tier: 1,
    }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, crate::event::WorldEvent::Traded { units: 1, .. })),
        "the buy went through: {events:?}"
    );
    assert_eq!(world.money, money - quote.ask, "{stocked:?} at the ask");
}

/// A station inside the front has one more hand for hire, and never more
/// than the most a station ever has.
#[test]
fn one_more_mercenary_inside_the_front() {
    let mut world = basic();
    let quiet: Vec<u32> = world
        .stations
        .iter()
        .map(|s| world.mercenaries_of(s))
        .collect();
    assert!(origin_at(&mut world, 2), "a star two hops from the crew");
    for (station, was) in world.stations.iter().zip(&quiet) {
        let now = world.mercenaries_of(station);
        // A derelict and an enemy's station have none either way.
        if *was == 0 && now == 0 {
            continue;
        }
        assert_eq!(
            now,
            (was + 1).min(MERCENARIES_MAX),
            "station {}",
            station.id
        );
        assert!(now <= MERCENARIES_MAX);
    }
    // At least one station in the system actually gained one, or this
    // test is watching nothing happen.
    let gained = world
        .stations
        .iter()
        .zip(&quiet)
        .any(|(s, was)| world.mercenaries_of(s) > *was);
    assert!(gained, "no station found one more hand");

    // Four hops out is outside the front and changes nothing.
    let mut world = basic();
    let quiet: Vec<u32> = world
        .stations
        .iter()
        .map(|s| world.mercenaries_of(s))
        .collect();
    if origin_at(&mut world, data::FRONT_HOPS + 1) {
        for (station, was) in world.stations.iter().zip(&quiet) {
            assert_eq!(world.mercenaries_of(station), *was, "beyond the front");
        }
    }
}
