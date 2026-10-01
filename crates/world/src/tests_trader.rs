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
    let trader = world.trader_here(0).expect("the trader is met");
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
/// every kind and tier from the lists, and its relic drawn by the day's
/// odds and **left in the pool** until bought (task 117), but out of every
/// other draw while it is on the table.
#[test]
fn the_same_seed_gives_the_same_shelf_and_the_same_relic() {
    let mut a = basic(1);
    let mut b = basic(1);
    let site = at_a_trader(&mut a);
    let there = on_the_map(&mut b);
    assert_eq!(there, site);
    travel_to(&mut b, there);
    let (ta, tb) = (a.trader_here(0).unwrap(), b.trader_here(0).unwrap());
    assert_eq!(ta, tb);
    let rolled: Vec<_> = trader::roll_shelf(
        a.galaxy_seed,
        site.star,
        site.station,
        0,
        a.clock_minutes.to_bits(),
        a.shop_tier(),
    )
    .into_iter()
    .map(Some)
    .collect();
    assert_eq!(ta.shelf, rolled);
    let relic = ta
        .relic
        .expect("a relic at a trader met with the pool full");
    assert!(a.relic_pool().contains(&relic), "in the pool until bought");
    assert_eq!(world_checksum(&a), world_checksum(&b));
    for n in 0..30u32 {
        a.run.relics.offers = n;
        let drawn = a.draw_relics(3, site.station);
        assert!(!drawn.contains(&relic), "on the table, out of other draws");
    }
}

/// **A trader closed puts its relic back in the running**: the relic comes
/// off the table at the next draw after its system falls, and may be drawn
/// from then on. It never left the pool.
#[test]
fn a_closed_trader_s_relic_goes_back_in_the_running() {
    let mut world = basic(1);
    let site = at_a_trader(&mut world);
    let relic = world.trader_here(0).unwrap().relic.expect("a relic");
    // The trader's system falls: the crisis on it, nothing liberated.
    world.set_crisis_first_day_for_probe(0);
    world.set_droid_origin_for_probe(site.star);
    assert!(world.trader_closed_on(site.star, world.days_gone()));
    world.set_relic_pool(vec![relic]);
    let drawn = world.draw_relics(1, site.station);
    assert_eq!(drawn, vec![relic], "back in the running");
    let met = world.traders_met().iter().find(|t| t.site == site).unwrap();
    assert_eq!(met.relic, None, "off the table");
}

/// **A bought thing is gone for the visit, and a revisit is a new shelf**
/// (October 2026): bought, its slot is empty and sold out; the crew leave,
/// fight somewhere else, come back, and meet the same trader — the same
/// relic — with its shelf rolled again for the visit, one gun and one
/// piece at the day's tier.
#[test]
fn a_bought_thing_is_gone_for_the_visit_and_a_revisit_rolls_the_shelf_again() {
    let mut world = basic(1);
    let site = at_a_trader(&mut world);
    let item = world.trader_here(0).unwrap().shelf[0].unwrap();
    let price = world.shelf_price(0, item);
    let money = world.wallet(0);
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
    assert_eq!(world.wallet(0), money - price, "paid out of its own wallet");
    assert_eq!(world.holdings.armory.len(), armory + 1, "into the armory");
    let bought = world.holdings.armory.last().unwrap();
    assert_eq!(bought.tier(), item.tier.code());
    let left = world.trader_here(0).unwrap().clone();
    assert_eq!(left.shelf[0], None);
    let events = world.step(&[Command::BuyShelf {
        slot: 0,
        index: 0,
        to: None,
    }]);
    assert!(refused_with(&events, Refusal::SoldOut), "{events:?}");

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
    let again = world.trader_here(0).unwrap().clone();
    assert_eq!(
        (again.site, again.relic),
        (left.site, left.relic),
        "the same trader"
    );
    let rolled: Vec<_> = trader::roll_shelf(
        world.galaxy_seed,
        site.star,
        site.station,
        0,
        world.clock_minutes.to_bits(),
        world.shop_tier(),
    )
    .into_iter()
    .map(Some)
    .collect();
    assert_eq!(again.shelf, rolled, "the shelf rolled for this visit");
}

/// **Bought onto a Bim**: the thing on its own slot, what was there into
/// the armory; another player's Bim is refused, and so is a buy the pool
/// cannot pay for, and nothing moves.
#[test]
fn a_thing_goes_onto_a_bim_or_the_armory_and_the_pool_must_pay() {
    let mut world = basic(2);
    at_a_trader(&mut world);
    let shelf = world.trader_here(0).unwrap().shelf.clone();
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

    // Not with its wallet short.
    world.wallets[0] = world.shelf_price(0, item) - 1;
    let events = world.step(&[Command::BuyShelf {
        slot: 0,
        index,
        to: Some(0),
    }]);
    assert!(refused_with(&events, Refusal::Unaffordable), "{events:?}");
    assert!(world.trader_here(0).unwrap().shelf[index as usize].is_some());

    // Onto its own Bim, the old gun into the armory.
    world.wallets[0] = RICH;
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

/// **Two players, one slot, two shelves**: every player has a trader of
/// its own, so the same slot is two things, each bought off its buyer's
/// own shelf with its own money in the same step — neither sold out by
/// the other, neither paying for the other. A shelf of its own is rolled
/// its own, so the two shelves are not the same shelf.
#[test]
fn two_players_buy_the_same_slot_each_off_its_own_shelf() {
    let mut world = basic(2);
    at_a_trader(&mut world);
    let (w0, w1) = (world.wallet(0), world.wallet(1));
    let mine = world.trader_here(0).unwrap().shelf[1].unwrap();
    let theirs = world.trader_here(1).unwrap().shelf[1].unwrap();
    assert_ne!(
        world.trader_here(0).unwrap().shelf,
        world.trader_here(1).unwrap().shelf,
        "a shelf rolled a player"
    );
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
    for slot in [0, 1] {
        assert!(
            events.contains(&WorldEvent::ShelfBought {
                slot,
                index: 1,
                to: u32::MAX
            }),
            "{events:?}"
        );
    }
    assert_eq!(world.wallet(0), w0 - world.shelf_price(0, mine));
    assert_eq!(world.wallet(1), w1 - world.shelf_price(1, theirs));
    assert!(world.trader_here(0).unwrap().shelf[1].is_none());
    assert!(world.trader_here(1).unwrap().shelf[1].is_none());
}

/// **Every player buys its own trader's relic with its own money**: each
/// player's trader has a relic of its own, none two alike, bought outright
/// for the player's own Bim — no vote — out of its own wallet, and gone
/// off that trader and out of the pool; the other's trader and money are
/// untouched. Another's relic is not on offer, and too poor is refused.
#[test]
fn every_player_buys_its_own_trader_s_relic_with_its_own_money() {
    let mut world = crewed_world(flyer(2), RICH, 2, 3);
    at_a_trader(&mut world);
    let mine = world.trader_here(0).unwrap().relic.unwrap();
    let theirs = world.trader_here(1).unwrap().relic.unwrap();
    assert_ne!(mine, theirs, "no two players are offered one relic");
    let price = world.trader_relic_price(0, mine);
    let (w0, w1) = (world.wallet(0), world.wallet(1));

    let buy = |relic: crate::relic::Relic, to| Command::ProposeRelic {
        slot: 0,
        relic: relic.code(),
        to,
    };
    let events = world.step(&[buy(theirs, 0)]);
    assert!(refused_with(&events, Refusal::NotOnOffer), "{events:?}");

    // Too poor: refused, and the relic stays.
    world.wallets[0] = price - 1;
    let events = world.step(&[buy(mine, 0)]);
    assert!(refused_with(&events, Refusal::Unaffordable), "{events:?}");
    assert_eq!(world.trader_here(0).unwrap().relic, Some(mine));
    assert!(world.relics_of(0).is_empty());

    // With the money: bought at once, for its own Bim whoever is named.
    world.wallets[0] = w0;
    let events = world.step(&[buy(mine, 1)]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::RelicBought { slot: 0, .. })),
        "{events:?}"
    );
    assert_eq!(world.wallet(0), w0 - price);
    assert_eq!(world.wallet(1), w1, "the other's money untouched");
    assert_eq!(world.relics_of(0), &[mine]);
    assert!(world.relics_of(1).is_empty());
    assert!(
        !world.relic_pool().contains(&mine),
        "bought, out of the pool"
    );
    assert_eq!(world.trader_here(0).unwrap().relic, None);
    assert_eq!(world.trader_here(1).unwrap().relic, Some(theirs));
    // Again is sold out, and costs nothing.
    let events = world.step(&[buy(mine, 0)]);
    assert!(refused_with(&events, Refusal::SoldOut), "{events:?}");
    assert_eq!(world.wallet(0), w0 - price);
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
    let helm = |id, t| Item::Armour(Piece::new(id, ArmourKind::Armour, t));
    for who in [0usize, 1] {
        let gear = world.aboard.room.gear(who);
        world.aboard.room.issue(
            who,
            Gear {
                armour: Some(match helm(900 + who as u32, Tier::One) {
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
        slot: GearSlot::Armour,
    };
    let events = world.step(&[combine(theirs, armory(spare))]);
    assert!(refused_with(&events, Refusal::NotYours), "{events:?}");

    // Off its own head: the result on its head, the spare gone.
    let mine = GearSource::Worn {
        who: 0,
        slot: GearSlot::Armour,
    };
    world.step(&[combine(armory(spare), mine)]);
    let Some(Item::Armour(on)) = world.worn_on(0, GearSlot::Armour) else {
        panic!("the armour on");
    };
    assert_eq!((on.kind, on.tier), (ArmourKind::Armour, Tier::Two));
    assert_eq!(
        on.health,
        Piece::new(0, ArmourKind::Armour, Tier::Two).health
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
    // The trader a jump away: a jump is a day, and a trip within the
    // system no time at all (the map rework), so only a jump arrives a day on.
    // A trader of this system is looked at from the next system over.
    if site.star == world.star_id {
        let away = world
            .destinations()
            .into_iter()
            .find(|&s| s.star != world.star_id && !world.is_trader(s))
            .filter(|&s| world.travel_quote(s).is_ok())
            .expect("a site a hyperlane off");
        travel_to(&mut world, away);
        world.leave_for_probe();
    }
    assert_ne!(site.star, world.star_id, "a jump away");
    world.set_droid_origin_for_probe(site.star);
    // A day after now: the jump's day.
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
        index: 1,
        to: None,
    }]);
    b.step(&[]);
    assert_ne!(world_checksum(&a), world_checksum(&b));
}

/// **A trader in one system in ten** (`data::TRADER_SYSTEM_CHANCE`), two
/// galaxies over: never two in one system, and the stars the galaxy chart
/// marks (`World::trader_stars`) are exactly the systems with one.
/// Since task 135 a trader is never the station a system offers, so a
/// system with one station has none: the share is about one in twenty
/// (one of the two galaxies here reads 3.3 per cent).
#[test]
fn a_trader_in_one_system_in_ten_and_the_chart_marks_them() {
    use worldgen::GalaxyType;
    for n in 0..2u64 {
        let seed = crate::data::DEFAULT_SEED.wrapping_add(n.wrapping_mul(0x9e37_79b9));
        let galaxy = worldgen::Galaxy::new(seed, GalaxyType::SpiralTwoArm);
        let (star, station) = crate::spawn(&galaxy).expect("a dock");
        let world = World::start(
            flyer(2),
            RICH,
            1,
            seed,
            GalaxyType::SpiralTwoArm,
            star,
            station,
        )
        .expect("a world");
        let mut with = Vec::new();
        for system in galaxy.every_system() {
            let traders = system
                .stations
                .iter()
                .filter(|s| world.is_trader_station(system.star_id, &system.stations, s))
                .count();
            assert!(
                traders <= 1,
                "star {} has {traders} traders",
                system.star_id
            );
            if traders == 1 {
                with.push(system.star_id);
            }
        }
        assert_eq!(world.trader_stars(&galaxy), with);
        let share = with.len() as f64 / galaxy.stars.len() as f64;
        assert!(
            (0.03..0.14).contains(&share),
            "seed {seed}: {share} of systems have a trader"
        );
    }
}

// --- every site one kind (task 111) ----------------------------------------

/// **Every site is exactly one of attack, defence and trader** (task
/// 111), over a few galaxies: a quote's kind is a trader where the site is
/// one, an attack only where an enemy has it on arrival, a defence
/// otherwise — and this system's sites say the same of themselves, a
/// defence threatened unless its fight is over.
#[test]
fn every_site_is_exactly_one_kind_over_several_seeds() {
    use crate::run::SiteKind;
    let mut seen = [0u32; 3];
    for n in 0..3u64 {
        let seed = crate::data::DEFAULT_SEED.wrapping_add(n.wrapping_mul(0x9e37_79b9));
        let galaxy = worldgen::Galaxy::new(seed, worldgen::GalaxyType::SpiralTwoArm);
        let (star, station) = crate::spawn(&galaxy).expect("a dock");
        let world = World::start(
            flyer(2),
            RICH,
            1,
            seed,
            worldgen::GalaxyType::SpiralTwoArm,
            star,
            station,
        )
        .expect("a world");
        for (site, quote) in world.travel_quotes() {
            let Ok(q) = quote else { continue };
            seen[q.kind.code() as usize] += 1;
            assert_eq!(q.kind == SiteKind::Trader, q.trader, "{site:?}");
            assert_eq!(
                q.kind == SiteKind::Trader,
                world.is_trader(site),
                "{site:?}"
            );
            if q.kind == SiteKind::Defend {
                assert!(
                    !q.infested && !q.manufacturers && q.heart.is_none(),
                    "{site:?}"
                );
                assert_eq!(q.threatened, !q.cleared, "{site:?}");
            } else {
                assert!(!q.threatened, "{site:?} threatened and not a defence");
            }
            if q.infested || q.manufacturers || q.heart.is_some() {
                assert_eq!(q.kind, SiteKind::Attack, "{site:?}");
            }
            if site.star == world.star_id && !q.infested {
                assert_eq!(world.site_kind(site.station), q.kind, "{site:?}");
                assert_eq!(
                    world.site_threatened(site.station),
                    q.threatened,
                    "{site:?}"
                );
            }
        }
        // And the spawn itself, where the crew are: never a trader, and a
        // defence from the first day.
        assert_eq!(world.site_kind(station), SiteKind::Defend);
        assert!(world.site_threatened(station));
    }
    assert!(seen[SiteKind::Defend.code() as usize] > 0, "{seen:?}");
    assert!(seen[SiteKind::Trader.code() as usize] > 0, "{seen:?}");
}

/// **A trader is never the machines'** (task 111): not by the crisis's
/// flip, not by `infest` itself, and never the system's jammer — nor
/// threatened, since nobody comes for one.
#[test]
fn a_trader_is_never_infested_and_never_the_jammer() {
    use crate::run::SiteKind;
    let mut world = basic(1);
    let site = world
        .trader_sites()
        .into_iter()
        .next()
        .expect("a trader near home");
    world.undock_for_probe();
    if site.star != world.star_id {
        let mut events = Vec::new();
        assert!(world.jump(site.star, &mut events), "the ship never jumped");
    }
    assert_eq!(world.site_kind(site.station), SiteKind::Trader);
    assert!(!world.site_threatened(site.station));
    world.infest(site.station);
    assert!(!world.is_droid_held(site.station), "infest took a trader");
    // The whole system the machines', as the crisis's flip has it.
    world.infest_here_for_probe();
    assert!(!world.is_droid_held(site.station), "the flip took a trader");
    assert_eq!(world.site_kind(site.station), SiteKind::Trader);
    assert_ne!(world.jammer_station(), Some(site.station), "a trader jams");
    assert!(!world.site_threatened(site.station));
}
