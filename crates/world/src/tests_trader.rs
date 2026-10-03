//! The trader (task 114): a visit on the map with no room and no mission,
//! every gun on the shelf every visit, no relic for sale, nothing combined and a
//! thing sold for half, and a trader closed while its system is the
//! machines'.

use bims::combat::Item;
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

/// **The same seed is the same shelf**: two worlds that arrive at one
/// trader meet one trader — its shelf every kind of the lists, at the
/// player's tiers.
#[test]
fn the_same_seed_gives_the_same_shelf() {
    let mut a = basic(1);
    let mut b = basic(1);
    let site = at_a_trader(&mut a);
    let there = on_the_map(&mut b);
    assert_eq!(there, site);
    travel_to(&mut b, there);
    let (ta, tb) = (a.trader_here(0).unwrap(), b.trader_here(0).unwrap());
    assert_eq!(ta, tb);
    let rolled: Vec<_> = a.shelf_for(0).into_iter().map(Some).collect();
    assert_eq!(ta.shelf, rolled);
    assert_eq!(world_checksum(&a), world_checksum(&b));
}

/// **A bought thing is gone for the visit, and a revisit is a new shelf**
/// (October 2026): bought, its slot is empty and sold out; the crew leave,
/// fight somewhere else, come back, and meet the same trader with its
/// shelf put up again for the visit — the kind bought now a tier past it
/// (`World::shelf_tier`).
#[test]
fn a_bought_thing_is_gone_for_the_visit_and_a_revisit_rolls_the_shelf_again() {
    let mut world = basic(1);
    let site = at_a_trader(&mut world);
    let item = world.trader_here(0).unwrap().shelf[0].unwrap();
    let price = world.shelf_price(item);
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
    assert_eq!(again.site, left.site, "the same trader");
    let rolled: Vec<_> = world.shelf_for(0).into_iter().map(Some).collect();
    assert_eq!(again.shelf, rolled, "the shelf put up for this visit");
    let up = again.shelf[0].unwrap();
    assert_eq!(up.resource, item.resource, "the kind bought is back");
    assert_eq!(up.tier, item.tier.next().unwrap(), "a tier up");
}

/// **Every gun but the pistol, and only the kind bought goes a tier up**
/// (October 2026): a shelf lists every gun but the laser pistol and the
/// armour; the armour and the first gun bought, every later shelf of that
/// player sells those two kinds a tier up and every other gun at tier
/// one; another player's shelves are as they were.
#[test]
fn every_gun_is_on_the_shelf_and_only_the_kind_bought_goes_a_tier_up() {
    use bims::combat::{Tier, WeaponKind};
    let mut world = basic(2);
    let site = at_a_trader(&mut world);
    let day = world.shop_tier();
    assert_eq!(day, Tier::One, "the start's day");
    let shelf: Vec<_> = world
        .trader_here(0)
        .unwrap()
        .shelf
        .iter()
        .map(|i| i.unwrap())
        .collect();
    assert_eq!(shelf, trader::shelf(|_| day), "every kind, at the day's");
    let guns: Vec<_> = shelf.iter().filter_map(|i| i.weapon()).collect();
    assert_eq!(guns.len(), WeaponKind::ALL.len() - 1, "every gun but one");
    assert!(guns.iter().all(|g| g.kind != WeaponKind::LaserPistol));
    let armour = shelf.iter().position(|i| i.armour().is_some()).unwrap();

    // The armour, then the first gun.
    for index in [armour as u32, 0] {
        let events = world.step(&[Command::BuyShelf {
            slot: 0,
            index,
            to: None,
        }]);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, WorldEvent::ShelfBought { .. })),
            "{events:?}"
        );
    }
    let bought = [shelf[0].resource, shelf[armour].resource];
    for item in &shelf {
        let want = if bought.contains(&item.resource) {
            Tier::Two
        } else {
            item.tier
        };
        assert_eq!(
            world.shelf_tier(0, item.resource).max(item.tier),
            want,
            "{item:?}"
        );
    }
    assert_eq!(world.shelf_for(1), shelf, "the other player's");
    let left = world.trader_here(0).unwrap().shelf[1].unwrap();
    assert_eq!(left, shelf[1], "the other guns as they were");

    // Somewhere else, and back: only the kinds bought are a tier up.
    let elsewhere = world
        .destinations()
        .into_iter()
        .find(|&s| s != site && !world.is_trader(s) && world.travel_quote(s).is_ok())
        .expect("somewhere else to go");
    travel_to(&mut world, elsewhere);
    world.leave_for_probe();
    world.run.phase = Phase::Map;
    world.run.relics.choice = None;
    travel_to(&mut world, site);
    assert_eq!(world.run.phase, Phase::Trade);
    assert_eq!(world.shop_tier(), day, "the day's tier has not moved");
    let again = world.trader_here(0).unwrap();
    for (now, was) in again.shelf.iter().map(|i| i.unwrap()).zip(&shelf) {
        assert_eq!(now.resource, was.resource);
        let up = bought.contains(&was.resource);
        assert_eq!(now.tier, if up { Tier::Two } else { was.tier }, "{now:?}");
    }
    let theirs = world.trader_here(1).unwrap();
    assert!(
        theirs
            .shelf
            .iter()
            .zip(&shelf)
            .all(|(i, was)| i.unwrap() == *was)
    );

    // Tier three bought leaves the kind at three.
    for best in &mut world.run.shelf_bought[0] {
        best.tier = Tier::Three;
    }
    for resource in bought {
        assert_eq!(world.shelf_tier(0, resource), Tier::Three);
    }
}

/// **The laser pistol is never sold**: the one a player's own Bim sets
/// out with, and a bot's, are refused `NotSellable` and stay where they
/// are; a gun bought is sold as ever.
#[test]
fn the_pistols_the_crew_set_out_with_are_never_sold() {
    use bims::combat::WeaponKind;
    let mut world = crewed_world(flyer(2), RICH, 1, 2);
    at_a_trader(&mut world);
    assert!(world.aboard.crew_count() > 1, "a bot aboard");
    for who in [0, 1] {
        let from = GearSource::Worn {
            who,
            slot: GearSlot::Weapon,
        };
        let held = world.worn_on(who, GearSlot::Weapon);
        assert!(
            matches!(held, Some(Item::Weapon(w)) if w.kind == WeaponKind::LaserPistol),
            "{held:?}"
        );
        assert_eq!(world.sellable(0, from), Err(Refusal::NotSellable));
        let money = world.wallet(0);
        let events = world.step(&[Command::Sell { slot: 0, from }]);
        assert!(refused_with(&events, Refusal::NotSellable), "{events:?}");
        assert_eq!(world.worn_on(who, GearSlot::Weapon), held);
        assert_eq!(world.wallet(0), money);
    }
    // A gun bought onto the Bim, the pistol into the armory: the gun is
    // sold, the pistol in the armory is not.
    world.step(&[Command::BuyShelf {
        slot: 0,
        index: 0,
        to: Some(0),
    }]);
    let worn = GearSource::Worn {
        who: 0,
        slot: GearSlot::Weapon,
    };
    assert!(world.sellable(0, worn).is_ok());
    let pistol = world
        .holdings
        .armory
        .iter()
        .find(|s| matches!(s.item, Item::Weapon(w) if w.kind == WeaponKind::LaserPistol))
        .expect("the pistol into the armory");
    assert_eq!(
        world.sellable(0, GearSource::Armory { id: pistol.id }),
        Err(Refusal::NotSellable)
    );
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
    world.wallets[0] = world.shelf_price(item) - 1;
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
/// the other, neither paying for the other.
#[test]
fn two_players_buy_the_same_slot_each_off_its_own_shelf() {
    let mut world = basic(2);
    at_a_trader(&mut world);
    let (w0, w1) = (world.wallet(0), world.wallet(1));
    let mine = world.trader_here(0).unwrap().shelf[1].unwrap();
    let theirs = world.trader_here(1).unwrap().shelf[1].unwrap();
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
    assert_eq!(world.wallet(0), w0 - world.shelf_price(mine));
    assert_eq!(world.wallet(1), w1 - world.shelf_price(theirs));
    assert!(world.trader_here(0).unwrap().shelf[1].is_none());
    assert!(world.trader_here(1).unwrap().shelf[1].is_none());
}

/// **No trader sells a relic** (October 2026): only an elite's fight
/// drops one. A relic proposed at a trader has nothing to be chosen from,
/// and nobody's money moves.
#[test]
fn no_trader_sells_a_relic() {
    let mut world = crewed_world(flyer(2), RICH, 2, 3);
    at_a_trader(&mut world);
    let (w0, w1) = (world.wallet(0), world.wallet(1));
    let events = world.step(&[Command::ProposeRelic {
        slot: 0,
        relic: crate::relic::Relic::GlassCannon.code(),
    }]);
    assert!(refused_with(&events, Refusal::NoRelicChoice), "{events:?}");
    assert!(world.relics().is_empty());
    assert_eq!((world.wallet(0), world.wallet(1)), (w0, w1));
}

/// **Nothing is combined** (October 2026): a gun bought off the shelf
/// with one like it in the armory is a second gun; and a **sale** is the
/// player's own — never off another player's Bim — at half the price.
#[test]
fn a_second_gun_bought_is_a_second_gun_and_another_player_s_kit_is_not_sold() {
    let mut world = basic(2);
    world.leave_for_probe();
    let site = an_open_trader(&world);
    travel_to(&mut world, site);
    assert!(world.at_trader());
    let gun = world.trader_here(0).unwrap().shelf[0].expect("a gun on the shelf");
    let weapon = gun.weapon().expect("the first slot is the gun");
    world.holdings.put(Item::Weapon(weapon));
    world.step(&[Command::BuyShelf {
        slot: 0,
        index: 0,
        to: None,
    }]);
    let guns = world
        .holdings
        .armory
        .iter()
        .filter(|s| s.item == Item::Weapon(weapon))
        .count();
    assert_eq!(guns, 2, "two of the gun, none made a tier up");

    let theirs = GearSource::Worn {
        who: 1,
        slot: GearSlot::Weapon,
    };
    assert!(world.worn_on(1, GearSlot::Weapon).is_some());
    let events = world.step(&[Command::Sell {
        slot: 0,
        from: theirs,
    }]);
    assert!(refused_with(&events, Refusal::NotYours), "{events:?}");
    assert!(world.worn_on(1, GearSlot::Weapon).is_some());

    // One of the two sold out of the armory, for half. (The pistol in the
    // player's own hand is never sold: `the_pistols_the_crew_set_out_with_
    // are_never_sold`.)
    let id = world
        .holdings
        .armory
        .iter()
        .find(|s| s.item == Item::Weapon(weapon))
        .unwrap()
        .id;
    let half = world.shelf_price(trader::ShelfItem {
        resource: crate::armour::weapon_resource(weapon.kind),
        tier: weapon.tier,
    }) / 2;
    let before = world.wallet(0);
    world.step(&[Command::Sell {
        slot: 0,
        from: GearSource::Armory { id },
    }]);
    assert_eq!(world.wallet(0), before + half);
    let guns = world
        .holdings
        .armory
        .iter()
        .filter(|s| s.item == Item::Weapon(weapon))
        .count();
    assert_eq!(guns, 1);
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

/// **A trader every five hops** (`data::TRADER_EVERY_HOPS`): from every
/// star of the galaxy — a trader's own included — another star's trader is
/// at most five lanes away, over every galaxy type and a few seeds, and
/// the made-up ones are the same for the same seed.
#[test]
fn from_every_star_another_trader_is_at_most_five_hops_away() {
    use worldgen::GalaxyType;
    let reach = crate::data::TRADER_EVERY_HOPS;
    for (n, &kind) in GalaxyType::ALL.iter().enumerate() {
        for m in 0..2u64 {
            let seed = crate::data::DEFAULT_SEED
                .wrapping_add((n as u64 * 2 + m).wrapping_mul(0x9e37_79b9));
            let galaxy = worldgen::Galaxy::new(seed, kind);
            let (star, station) = crate::spawn(&galaxy).expect("a dock");
            let world =
                World::start(flyer(2), RICH, 1, seed, kind, star, station).expect("a world");
            let traders = world.trader_stars(&galaxy);
            for from in 0..galaxy.stars.len() as u32 {
                let hops = galaxy.hops_from(from);
                let nearest = traders
                    .iter()
                    .filter(|&&t| t != from)
                    .map(|&t| hops[t as usize])
                    .min()
                    .unwrap_or(u16::MAX);
                assert!(
                    nearest <= reach,
                    "{kind:?} seed {seed}: star {from}'s nearest other trader is {nearest} hops off"
                );
            }
            let again =
                World::start(flyer(2), RICH, 1, seed, kind, star, station).expect("a world");
            assert_eq!(again.trader_stars(&galaxy), traders);
            println!(
                "{kind:?} seed {seed}: {} traders of {} stars",
                traders.len(),
                galaxy.stars.len()
            );
        }
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

/// **Every kind starts at tier one** (October 2026, the player's word):
/// the tank, though it sets out in tier-one armour, and the soldier,
/// though it sets out with a tier-one auto rifle, are offered every kind
/// at tier one like a classless player — nothing counts as bought until
/// it is — and a day past every tier timing lifts no shelf: only buying
/// does.
#[test]
fn every_kind_starts_at_tier_one_whatever_the_class_or_the_day() {
    use crate::class::Class;
    use bims::combat::Tier;
    use physics::ResourceId;
    let mut world = basic(3);
    assert_eq!(world.set_class(0, Class::Tank), Ok(()));
    assert_eq!(world.set_class(1, Class::Soldier), Ok(()));
    // Every tier timing at nought: the day is past tier three.
    world.set_wave_scaling(crate::droid::WaveScaling {
        tier2_days: 0,
        tier3_days: 0,
        ..crate::droid::WaveScaling::DEFAULT
    });
    assert_eq!(world.shop_tier(), Tier::Three, "the day's tier");
    at_a_trader(&mut world);
    let plain = trader::shelf(|_| Tier::One);
    for slot in 0..3 {
        let shelf: Vec<_> = world
            .trader_here(slot)
            .unwrap()
            .shelf
            .iter()
            .map(|i| i.unwrap())
            .collect();
        assert_eq!(shelf, plain, "slot {slot}");
        for resource in [ResourceId::Armour, ResourceId::AutoRifle] {
            assert_eq!(world.shelf_tier(slot, resource), Tier::One);
        }
    }
}

/// **A tier up onto a Bim sells the one it replaces** (October 2026): a
/// gun or the armour bought onto a Bim already carrying the same kind a
/// tier lower sells the old one at once into the buyer's wallet — it never
/// goes into the armory. A thing of another kind still does.
#[test]
fn a_tier_up_bought_onto_a_bim_sells_the_lower_tier_it_replaces() {
    use bims::combat::{Piece, Tier};
    let mut world = basic(1);
    at_a_trader(&mut world);
    let shelf = world.trader_here(0).unwrap().shelf.clone();
    let gun = shelf[0].unwrap();
    let armour_at = shelf
        .iter()
        .position(|i| i.unwrap().armour().is_some())
        .unwrap();
    let armour = shelf[armour_at].unwrap();
    // The Bim carries both kinds at tier one; the shelf sells them at two.
    let mut gear = world.aboard.room.gear(0);
    gear.weapon = Some(gun.weapon().unwrap().kind.at(Tier::One));
    gear.armour = Some(Piece::new(9_999, armour.armour().unwrap(), Tier::One));
    world.aboard.room.issue(0, gear);
    for t in world.run.traders.iter_mut().filter(|t| t.owner == 0) {
        for index in [0, armour_at] {
            if let Some(item) = t.shelf[index].as_mut() {
                item.tier = Tier::Two;
            }
        }
    }
    let armory = world.holdings.armory.len();
    for (index, part) in [(0, GearSlot::Weapon), (armour_at, GearSlot::Armour)] {
        let old = world.worn_on(0, part).unwrap();
        let value = world.sell_value(old).unwrap();
        let price = world.shelf_price(trader::ShelfItem {
            tier: Tier::Two,
            ..shelf[index].unwrap()
        });
        let money = world.wallet(0);
        let events = world.step(&[Command::BuyShelf {
            slot: 0,
            index: index as u32,
            to: Some(0),
        }]);
        assert!(
            events.contains(&WorldEvent::Sold {
                slot: 0,
                who: 0,
                value
            }),
            "{events:?}"
        );
        assert_eq!(world.wallet(0), money - price + value, "sold, not stored");
        assert_eq!(
            world.holdings.armory.len(),
            armory,
            "nothing into the armory"
        );
        assert!(
            matches!(
                world.worn_on(0, part),
                Some(Item::Weapon(w)) if w.tier == Tier::Two
            ) || matches!(
                world.worn_on(0, part),
                Some(Item::Armour(p)) if p.tier == Tier::Two
            )
        );
    }
}

/// **The minigun costs three times its ask** (October 2026, the player's
/// word, when its magazine made it fire ten a second): its shelf price is
/// the ordinary rule over three times the ask, and it sells back for half
/// of that, as anything bought does.
#[test]
fn a_minigun_costs_three_times_its_ask_and_sells_back_for_half_of_that() {
    use bims::combat::{Tier, WeaponKind};
    use physics::ResourceId;
    let mut world = basic(1);
    at_a_trader(&mut world);
    let tier = Tier::Two;
    let ask = world
        .run
        .site
        .and_then(|id| world.quote_at(id, ResourceId::Minigun, tier.code()))
        .map(|q| q.ask)
        .unwrap_or_else(|| {
            economy::trade_price(ResourceId::Minigun)
                .saturating_mul(economy::tier_price(tier.code()))
        });
    let rule = |ask: economy::Money| {
        world.trader_share(world.trader_price_by_relics(world.rewards().shelf_price(ask)))
    };
    let price = world.shelf_price(trader::ShelfItem {
        resource: ResourceId::Minigun,
        tier,
    });
    assert_eq!(price, rule(ask * 3), "three times the ask");
    assert!(price > rule(ask) * 2, "{price} against {}", rule(ask));
    let back = world
        .sell_value(Item::Weapon(WeaponKind::Minigun.at(tier)))
        .unwrap();
    assert_eq!(back, price * crate::data::SELL_BACK_PERCENT / 100);
    // Every other kind keeps its own price.
    let shotgun = trader::ShelfItem {
        resource: ResourceId::Shotgun,
        tier,
    };
    let shotgun_ask = world
        .run
        .site
        .and_then(|id| world.quote_at(id, ResourceId::Shotgun, tier.code()))
        .map(|q| q.ask)
        .unwrap_or_else(|| {
            economy::trade_price(ResourceId::Shotgun)
                .saturating_mul(economy::tier_price(tier.code()))
        });
    assert_eq!(world.shelf_price(shotgun), rule(shotgun_ask));
}
