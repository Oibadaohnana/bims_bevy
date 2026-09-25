//! The trader (task 114): a visit on the map with no room and no mission,
//! the shelf rolled once a trader a run, the relic bought together, two
//! things combined into one, and a trader closed while its system is the
//! machines'.

use bims::combat::{ArmourKind, Gear, Item, Piece, Tier, WeaponKind};
use shipdesign::fixture::flyer;

use crate::checksum::world_checksum;
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{crewed_world, simulation_world};
use crate::holdings::{GearSlot, GearSource};
use crate::run::{Phase, Site};
use crate::trader;
use crate::world::{Command, ShipState, World};

/// Money enough for anything on a shelf.
const RICH: economy::Money = 10_000_000;

fn basic(players: u32) -> World {
    simulation_world(flyer(2), RICH, players)
}

/// Every player's yes to `site`, the first putting it: the trip.
fn travel_to(world: &mut World, site: Site) -> Vec<WorldEvent> {
    let mut events = world.step(&[Command::Propose {
        slot: 0,
        star: site.star,
        station: site.station,
    }]);
    for slot in 1..world.players() {
        events.extend(world.step(&[Command::Accept { slot, yes: true }]));
    }
    events
}

fn refused_with(events: &[WorldEvent], want: Refusal) -> bool {
    events
        .iter()
        .any(|e| matches!(e, WorldEvent::Refused { why, .. } if *why == want))
}

/// A trader a trip can go to from here, open.
fn an_open_trader(world: &World) -> Site {
    world
        .trader_sites()
        .into_iter()
        .find(|&s| world.travel_quote(s).is_ok())
        .expect("a trader near home, as the start makes sure of")
}

/// A world on the map, and a trader it can go to.
fn on_the_map(world: &mut World) -> Site {
    world.leave_for_probe();
    assert_eq!(world.run.phase, Phase::Map);
    an_open_trader(world)
}

/// A world at a trader: the map, then the trip.
fn at_a_trader(world: &mut World) -> Site {
    let site = on_the_map(world);
    travel_to(world, site);
    assert_eq!(world.run.phase, Phase::Trade, "arrived at the trader");
    site
}

/// **A trader is the map**: the trip moves the world clock by its length
/// and nothing more, and on arrival the run is in `Phase::Trade` — no
/// room loaded, no mission begun, and neither clock moving however long
/// the crew stay.
#[test]
fn arriving_at_a_trader_loads_no_room_and_runs_no_mission() {
    let mut world = basic(1);
    let site = on_the_map(&mut world);
    let quote = world.travel_quote(site).unwrap();
    assert!(quote.trader, "the quote says it is a trader");
    let clock = world.clock_minutes;
    let missions = world.run.missions;
    let events = travel_to(&mut world, site);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Travelled { .. })),
        "{events:?}"
    );
    assert_eq!(world.run.phase, Phase::Trade);
    assert!(world.at_trader() && !world.in_mission());
    assert_eq!(world.current_site(), Some(site));
    assert_eq!(world.clock_minutes, clock + quote.minutes as f64);
    assert_eq!(world.run.missions, missions, "no mission begun");
    assert_eq!(world.ship.state, ShipState::Holding, "holding off it");
    assert!(world.residents.is_none(), "no room loaded");
    let trader = world.trader_here().expect("the trader is met");
    assert_eq!(trader.site, site);
    let (steps, clock) = (world.mission_steps(), world.clock_minutes);
    for _ in 0..200 {
        world.step(&[]);
    }
    assert_eq!(world.clock_minutes, clock, "the world clock stands");
    assert_eq!(world.mission_steps(), steps, "the mission clock stands");
    assert!(world.residents.is_none());
}

/// **The same seed is the same shelf and the same relic**: two worlds
/// that arrive at one trader meet one trader — its shelf the roll's,
/// every kind and tier from the lists, and its relic out of the pool at
/// no more than the site's tier.
#[test]
fn the_same_seed_gives_the_same_shelf_and_the_same_relic() {
    let mut a = basic(1);
    let mut b = basic(1);
    let site = at_a_trader(&mut a);
    let there = on_the_map(&mut b);
    assert_eq!(there, site);
    travel_to(&mut b, there);
    let (ta, tb) = (a.trader_here().unwrap(), b.trader_here().unwrap());
    assert_eq!(ta, tb);
    let rolled: Vec<_> = trader::roll_shelf(a.galaxy_seed, site.star, site.station)
        .into_iter()
        .map(Some)
        .collect();
    assert_eq!(ta.shelf, rolled);
    let relic = ta
        .relic
        .expect("a relic at a trader met with the pool full");
    assert!(!a.relic_pool().contains(&relic), "out of the pool for good");
    assert_eq!(world_checksum(&a), world_checksum(&b));
}

/// **A bought thing never comes back**: bought, its slot is empty; the
/// crew leave, fight somewhere else, come back, and meet the same shelf
/// with the same hole in it and the same relic — no restock, no reroll —
/// and the slot is still sold out.
#[test]
fn a_bought_thing_never_reappears_and_a_revisit_is_the_same_trader() {
    let mut world = basic(1);
    let site = at_a_trader(&mut world);
    let item = world.trader_here().unwrap().shelf[0].unwrap();
    let price = world.shelf_price(item);
    let money = world.money;
    let armory = world.holdings.armory.len();
    let events = world.step(&[Command::BuyShelf {
        slot: 0,
        index: 0,
        to: None,
    }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::ShelfBought { index: 0, .. })),
        "{events:?}"
    );
    assert_eq!(world.money, money - price, "paid out of the pool");
    assert_eq!(world.holdings.armory.len(), armory + 1, "into the armory");
    let bought = world.holdings.armory.last().unwrap();
    assert_eq!(bought.tier(), item.tier.code());
    let left = world.trader_here().unwrap().clone();
    assert_eq!(left.shelf[0], None);

    // Somewhere else, and back.
    let elsewhere = world
        .destinations()
        .into_iter()
        .find(|&s| s != site && !world.is_trader(s) && world.travel_quote(s).is_ok())
        .expect("somewhere else to go");
    travel_to(&mut world, elsewhere);
    assert_eq!(world.run.phase, Phase::Mission);
    world.leave_for_probe();
    assert!(matches!(world.run.phase, Phase::Map | Phase::Reward));
    world.run.phase = Phase::Map;
    world.run.relics.choice = None;
    travel_to(&mut world, site);
    assert_eq!(world.run.phase, Phase::Trade);
    assert_eq!(world.trader_here(), Some(&left), "the same trader");
    let events = world.step(&[Command::BuyShelf {
        slot: 0,
        index: 0,
        to: None,
    }]);
    assert!(refused_with(&events, Refusal::SoldOut), "{events:?}");
}

/// **Bought onto a Bim**: the thing on its own slot, what was there into
/// the armory; another player's Bim is refused, and so is a buy the pool
/// cannot pay for, and nothing moves.
#[test]
fn a_thing_goes_onto_a_bim_or_the_armory_and_the_pool_must_pay() {
    let mut world = basic(2);
    at_a_trader(&mut world);
    let shelf = world.trader_here().unwrap().shelf.clone();
    let index = shelf
        .iter()
        .position(|i| i.is_some_and(|i| i.is_weapon()))
        .unwrap() as u32;
    let item = shelf[index as usize].unwrap();
    let before = world.worn_on(0, GearSlot::Weapon);
    assert!(before.is_some(), "player one starts with a gun");

    // Not another player's Bim.
    let events = world.step(&[Command::BuyShelf {
        slot: 0,
        index,
        to: Some(1),
    }]);
    assert!(refused_with(&events, Refusal::NotYours), "{events:?}");

    // Not with the pool short.
    world.money = world.shelf_price(item) - 1;
    let events = world.step(&[Command::BuyShelf {
        slot: 0,
        index,
        to: Some(0),
    }]);
    assert!(refused_with(&events, Refusal::Unaffordable), "{events:?}");
    assert!(world.trader_here().unwrap().shelf[index as usize].is_some());

    // Onto its own Bim, the old gun into the armory.
    world.money = RICH;
    let armory = world.holdings.armory.len();
    world.step(&[Command::BuyShelf {
        slot: 0,
        index,
        to: Some(0),
    }]);
    assert_eq!(
        world.worn_on(0, GearSlot::Weapon),
        item.weapon().map(Item::Weapon)
    );
    assert_eq!(world.holdings.armory.len(), armory + 1);
    assert_eq!(world.holdings.armory.last().map(|s| s.item), before);
}

/// **Two players, one thing**: the first command to want it has it, and
/// the second is refused `SoldOut` — in one step or two.
#[test]
fn two_players_buying_the_same_thing_the_first_has_it() {
    let mut world = basic(2);
    at_a_trader(&mut world);
    let money = world.money;
    let item = world.trader_here().unwrap().shelf[1].unwrap();
    let events = world.step(&[
        Command::BuyShelf {
            slot: 1,
            index: 1,
            to: None,
        },
        Command::BuyShelf {
            slot: 0,
            index: 1,
            to: None,
        },
    ]);
    assert!(
        events.contains(&WorldEvent::ShelfBought {
            slot: 1,
            index: 1,
            to: u32::MAX
        }),
        "{events:?}"
    );
    assert!(
        events.contains(&WorldEvent::Refused {
            slot: 0,
            why: Refusal::SoldOut
        }),
        "{events:?}"
    );
    assert_eq!(world.money, money - world.shelf_price(item), "paid once");
}

/// **The relic is bought together**: put for a player's Bim, it waits for
/// every connected player's yes; a new proposal clears every yes; carried,
/// the pool pays its price once and the Bim holds it, and it is gone off
/// the trader. A bot is refused, and so is a carried vote the pool cannot
/// pay for.
#[test]
fn the_relic_wants_everybody_s_yes_and_is_paid_for_once() {
    let mut world = crewed_world(flyer(2), RICH, 2, 3);
    at_a_trader(&mut world);
    let relic = world.trader_here().unwrap().relic.unwrap();
    let price = trader::relic_price(relic);
    let money = world.money;

    // A bot never holds a relic.
    let events = world.step(&[Command::ProposeRelic {
        slot: 0,
        relic: relic.code(),
        to: 2,
    }]);
    assert!(refused_with(&events, Refusal::NotAPlayer), "{events:?}");

    // Player one's proposal, then player two's: the first yes goes.
    world.step(&[Command::ProposeRelic {
        slot: 0,
        relic: relic.code(),
        to: 0,
    }]);
    world.step(&[Command::ProposeRelic {
        slot: 1,
        relic: relic.code(),
        to: 1,
    }]);
    assert_eq!(world.money, money, "nothing carried yet");
    assert_eq!(world.trade_relic().unwrap().accepted, vec![false, true]);
    assert_eq!(world.trader_here().unwrap().relic, Some(relic));

    // Too poor when it carries: refused, the relic stays, the vote goes.
    world.money = price - 1;
    let events = world.step(&[Command::AcceptRelic { slot: 0, yes: true }]);
    assert!(refused_with(&events, Refusal::Unaffordable), "{events:?}");
    assert_eq!(world.trader_here().unwrap().relic, Some(relic));
    assert!(world.trade_relic().is_none());
    assert!(world.relics_of(1).is_empty());

    // With the money, and both yeses: bought, once.
    world.money = money;
    world.step(&[Command::ProposeRelic {
        slot: 1,
        relic: relic.code(),
        to: 1,
    }]);
    let events = world.step(&[Command::AcceptRelic { slot: 0, yes: true }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::RelicBought { slot: 1, .. })),
        "{events:?}"
    );
    assert_eq!(world.money, money - price);
    assert_eq!(world.relics_of(1), &[relic]);
    assert_eq!(world.trader_here().unwrap().relic, None);
    // A yes after is a yes to nothing, and costs nothing.
    let events = world.step(&[Command::AcceptRelic { slot: 1, yes: true }]);
    assert!(refused_with(&events, Refusal::NoRelicChoice), "{events:?}");
    assert_eq!(world.money, money - price);
}

/// **Combining** two of a kind at a tier is one of the next, whole and at
/// once; tier three is refused, and so is a pair that is not one, an input
/// off another player's Bim, and anything anywhere but at a trader. Where
/// one of the two was worn the result is on that Bim.
#[test]
fn combining_makes_one_of_the_next_tier_and_refuses_what_it_should() {
    let mut world = basic(2);
    world.leave_for_probe();
    let rifle = |t| Item::Weapon(WeaponKind::AutoRifle.at(t));
    let a = world.holdings.put(rifle(Tier::One)).unwrap();
    let b = world.holdings.put(rifle(Tier::One)).unwrap();
    let combine = |a, b| Command::Combine { slot: 0, a, b };
    let armory = |id| GearSource::Armory { id };

    // On the map: not at a trader.
    let events = world.step(&[combine(armory(a), armory(b))]);
    assert!(refused_with(&events, Refusal::NotAtATrader), "{events:?}");

    let site = an_open_trader(&world);
    travel_to(&mut world, site);
    assert!(world.at_trader());

    // Two tier-one rifles in the armory: one tier-two rifle in the armory.
    let before = world.holdings.armory.len();
    let events = world.step(&[combine(armory(a), armory(b))]);
    assert!(
        events.iter().any(|e| matches!(
            e,
            WorldEvent::Combined {
                who: u32::MAX,
                tier: 2,
                ..
            }
        )),
        "{events:?}"
    );
    assert_eq!(world.holdings.armory.len(), before - 1);
    assert!(world.holdings.get(a).is_none() && world.holdings.get(b).is_none());
    assert_eq!(world.holdings.armory.last().unwrap().item, rifle(Tier::Two));

    // Tier three is as far as it goes.
    let c = world.holdings.put(rifle(Tier::Three)).unwrap();
    let d = world.holdings.put(rifle(Tier::Three)).unwrap();
    let events = world.step(&[combine(armory(c), armory(d))]);
    assert!(refused_with(&events, Refusal::TopTier), "{events:?}");
    assert!(world.holdings.get(c).is_some() && world.holdings.get(d).is_some());

    // Not a pair.
    let e = world.holdings.put(rifle(Tier::One)).unwrap();
    let events = world.step(&[combine(armory(c), armory(e))]);
    assert!(refused_with(&events, Refusal::NotAPair), "{events:?}");

    // Never off another player's Bim.
    let helm = |id, t| Item::Armour(Piece::new(id, ArmourKind::BasicHelm, t));
    for who in [0usize, 1] {
        let gear = world.aboard.room.gear(who);
        world.aboard.room.issue(
            who,
            Gear {
                head: Some(match helm(900 + who as u32, Tier::One) {
                    Item::Armour(p) => p,
                    _ => unreachable!(),
                }),
                ..gear
            },
        );
    }
    let spare = world.holdings.put(helm(0, Tier::One)).unwrap();
    let theirs = GearSource::Worn {
        who: 1,
        slot: GearSlot::Head,
    };
    let events = world.step(&[combine(theirs, armory(spare))]);
    assert!(refused_with(&events, Refusal::NotYours), "{events:?}");

    // Off its own head: the result on its head, the spare gone.
    let mine = GearSource::Worn {
        who: 0,
        slot: GearSlot::Head,
    };
    world.step(&[combine(armory(spare), mine)]);
    let Some(Item::Armour(on)) = world.worn_on(0, GearSlot::Head) else {
        panic!("a helm on its head");
    };
    assert_eq!((on.kind, on.tier), (ArmourKind::BasicHelm, Tier::Two));
    assert_eq!(
        on.health,
        Piece::new(0, ArmourKind::BasicHelm, Tier::Two).health
    );
    assert!(world.holdings.get(spare).is_none());
}

/// **A trader is closed while its system is the machines'** and not
/// liberated — the crisis passes it by, so it is never theirs to hold —
/// and open again once every site of the system they took is cleared.
#[test]
fn a_trader_in_an_infested_system_is_closed_until_it_is_liberated() {
    let mut world = basic(1);
    let site = on_the_map(&mut world);
    // The machines' origin at the trader's own star, from day nought.
    world.set_droid_origin_for_probe(site.star);
    world.set_crisis_first_day_for_probe(0);
    assert!(world.infested(site.star));
    assert_eq!(world.travel_quote(site), Err(Refusal::TraderClosed));
    let events = travel_to(&mut world, site);
    assert!(refused_with(&events, Refusal::TraderClosed), "{events:?}");
    assert_eq!(world.run.phase, Phase::Map, "nobody went");

    // Liberated: every site of its system the machines took, cleared.
    if site.star == world.star_id {
        world.infest_here_for_probe();
        assert!(!world.is_droid_held(site.station), "never the machines'");
        assert!(!world.liberated(site.star));
        for it in world.infested.iter_mut() {
            it.cleared = true;
        }
    } else {
        let mut it = crate::droid::Infestation::new(0);
        it.cleared = true;
        let memory = crate::memory::SystemMemory {
            star: site.star,
            lamps: Vec::new(),
            discovered: Vec::new(),
            losses: Vec::new(),
            graves: Vec::new(),
            visited: Vec::new(),
            infested: vec![it],
            defenses: Vec::new(),
            held_towns: Vec::new(),
        };
        crate::memory::file_memory(&mut world.memories, memory);
    }
    assert!(world.liberated(site.star));
    let quote = world.travel_quote(site).expect("open again");
    assert!(quote.trader && !quote.infested);
}

/// **Closed on arrival** is the crisis on the arrival day, exactly: a
/// trader whose system turns before the crew could get there is refused,
/// and a site of the same system beside it is quoted infested on arrival;
/// with the day further off it is open.
#[test]
fn closed_on_arrival_is_the_crisis_on_the_arrival_day() {
    let mut world = basic(1);
    let site = on_the_map(&mut world);
    world.set_droid_origin_for_probe(site.star);
    // A day after now: every trip is at least a day (feature 105).
    world.set_crisis_first_day_for_probe(world.days_gone() + 1);
    assert!(!world.infested(site.star), "not theirs today");
    assert_eq!(world.travel_quote(site), Err(Refusal::ClosedOnArrival));
    let beside = world
        .sites_at(site.star)
        .into_iter()
        .filter(|&s| s != site && !world.is_trader(s))
        .find_map(|s| world.travel_quote(s).ok());
    if let Some(quote) = beside {
        assert!(quote.infested, "the system is theirs on arrival");
        assert!(world.trader_closed_on(site.star, quote.arrival_day));
    }
    world.set_crisis_first_day_for_probe(world.days_gone() + 1_000);
    assert!(world.travel_quote(site).is_ok());
}

/// The trader's state is in the checksum: a thing bought on one world and
/// not the other is two worlds.
#[test]
fn what_a_trader_has_left_is_in_the_checksum() {
    let mut a = basic(1);
    let mut b = basic(1);
    at_a_trader(&mut a);
    at_a_trader(&mut b);
    assert_eq!(world_checksum(&a), world_checksum(&b));
    a.step(&[Command::BuyShelf {
        slot: 0,
        index: 2,
        to: None,
    }]);
    b.step(&[]);
    assert_ne!(world_checksum(&a), world_checksum(&b));
}

/// How many of a galaxy's stations are traders, over ten galaxy seeds —
/// what `data::TRADER_SITE_CHANCE` comes to, beside the Manufacturers'
/// and every station the rule leaves out.
#[test]
#[ignore]
fn trader_share_over_ten_seeds() {
    use worldgen::GalaxyType;
    println!("seed                  stations  traders  share");
    for n in 0..10u64 {
        let seed = crate::data::DEFAULT_SEED.wrapping_add(n.wrapping_mul(0x9e37_79b9));
        let galaxy = worldgen::Galaxy::new(seed, GalaxyType::SpiralTwoArm);
        let Some((star, station)) = crate::spawn(&galaxy) else {
            continue;
        };
        let world = World::start(
            flyer(2),
            RICH,
            1,
            seed,
            GalaxyType::SpiralTwoArm,
            star,
            station,
        )
        .unwrap();
        let (mut stations, mut traders) = (0u32, 0u32);
        for system in galaxy.every_system() {
            for s in &system.stations {
                stations += 1;
                traders += world.is_trader_station(system.star_id, &system.stations, s) as u32;
            }
        }
        println!(
            "{seed:>20}  {stations:>8}  {traders:>7}  {:>4.1}%",
            100.0 * traders as f64 / stations.max(1) as f64
        );
    }
}
