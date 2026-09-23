//! The money rework (feature 95): what a crew are worth, what the
//! Republic pays for an enemy, and where the gear is sold.
//!
//! The construction half of it — a site charged its price, one that waits
//! for want of money — is in `tests.rs` beside the other building tests,
//! since it is the same machinery; and the price table's own inequalities
//! are `economy`'s, which is the crate that holds them.

use bims::combat::{ArmourKind, Tier, WeaponKind};
use economy::{TIER_PRICE, trade_price};
use physics::ResourceId;
use shipdesign::PartKind;

use crate::armour::{self, Where};
use crate::data;
use crate::event::WorldEvent;
use crate::fixture::simulation_world;
use crate::world::{Command, World};

fn a_world() -> World {
    let mut world = simulation_world(
        shipdesign::fixture::playtest_ship(),
        data::SIMULATION_MONEY,
        1,
    );
    crate::tests::without_dressings(&mut world);
    world
}

/// **Worth is everything the crew own**, money included (feature 95):
/// every part at its price, the hold at the book with the gear at its
/// tier, every piece and gun the crew carry, and the pool. Nothing a crew
/// own changes what they are worth by moving from one pocket to another.
#[test]
fn worth_is_the_ship_the_hold_the_crew_s_gear_and_the_money() {
    let mut world = a_world();

    // The money is in it: a euro more is a euro more.
    let before = world.worth();
    world.money += 1_000;
    assert_eq!(world.worth(), before + 1_000);
    world.money -= 1_000;

    // Every part is in it: the sum is at least what the parts come to.
    let parts: economy::Money = world
        .ship
        .design
        .parts
        .iter()
        .map(|p| p.kind.def().price)
        .sum();
    assert!(world.worth() >= parts + world.money);

    // A tier-two rifle is worth four tier-one ones: the tier multiplies
    // the book, both in the hold and on a body.
    let rifle = ResourceId::AutoRifle;
    let one = world.worth();
    world.ship.design.cargo[rifle as usize] += 1;
    world.on_ship_changed();
    assert_eq!(world.worth(), one + trade_price(rifle));
    // Put that gun up a tier by hand, the way the workbench would.
    let at = world
        .guns
        .iter()
        .position(|g| g.kind == WeaponKind::AutoRifle && g.tier == Tier::One)
        .expect("the gun that was just bought");
    world.guns[at] = WeaponKind::AutoRifle.at(Tier::Two);
    assert_eq!(
        world.worth(),
        one + trade_price(rifle) * TIER_PRICE[2],
        "a tier-two rifle is four tier-one ones"
    );

    // And a piece on a crew member's back counts like one in the hold.
    let helm = ResourceId::Helm;
    let worn = world.worth();
    let id = world.next_piece;
    world.next_piece += 1;
    world.pieces.push(armour::Piece {
        at: Where::Worn { who: 0 },
        ..armour::Piece::new(id, ArmourKind::BasicHelm, Tier::Three)
    });
    assert_eq!(
        world.worth(),
        worn + trade_price(helm) * TIER_PRICE[3],
        "a tier-three helm on a head is sixteen tier-one ones"
    );
}

/// `start_worth` is `worth()` on the first step, the starting pool
/// included — so unspent money is never counted as growth, which is what
/// it was before the pool went into the sum.
#[test]
fn start_worth_is_worth_when_the_world_opens() {
    // Not `a_world`: emptying the packs of their dressings would take
    // sixty euros of worth out from under the comparison.
    let world = simulation_world(
        shipdesign::fixture::playtest_ship(),
        data::SIMULATION_MONEY,
        1,
    );
    assert_eq!(world.start_worth, world.worth());
    assert!(world.start_worth > world.money, "the ship is in it too");
}

/// Building a part and taking one off each leave the crew **no richer and
/// no poorer**: the price leaves the pool and the part's own price
/// arrives on the ship, and back again.
#[test]
fn building_and_deconstructing_leave_worth_unchanged() {
    let mut world = a_world();
    let before = world.worth();
    let price = PartKind::Wall.def().price;
    let design = world.ship.design.clone();

    // The site's price out of the pool and the part onto the ship, the
    // way `finish_build` does it.
    let next = shipdesign::apply(
        &design,
        &shipdesign::Budget::new(economy::Money::MAX),
        shipdesign::Edit::Place {
            kind: PartKind::Wall,
            origin: (9, 12),
            rotation: shipdesign::Rotation::R0,
        },
    )
    .expect("a wall on open deck");
    world.ship.design = next;
    world.money -= price;
    world.on_ship_changed();
    assert_eq!(world.worth(), before, "a wall bought is a wall owned");

    // And off again, at the full price — `shipdesign::refund_for`.
    let part = world.ship.design.parts.last().unwrap().id;
    let back = shipdesign::refund_for(&world.ship.design, part);
    assert_eq!(back, price, "no loss either way");
    world.ship.design = shipdesign::apply(
        &world.ship.design,
        &shipdesign::Budget::new(economy::Money::MAX),
        shipdesign::Edit::Remove { part_id: part },
    )
    .expect("the wall came off");
    world.money += back;
    world.on_ship_changed();
    assert_eq!(world.worth(), before);
}

/// **The Republic pays a bounty, once per enemy, at the first down or
/// death, whoever did it** — and nothing at all for a friend, a neutral
/// or one of the crew (feature 95).
#[test]
fn the_republic_pays_once_for_every_enemy_taken_down() {
    let mut world = crate::tests::basic();
    let station = match world.ship.state {
        crate::world::ShipState::Docked { station } => station,
        _ => panic!("a world opens docked"),
    };
    // A friendly dock first: nobody down is worth anything, since the
    // bounty is only ever paid at a hostile one.
    let money = world.money;
    for _ in 0..60 {
        let events = world.step(&[]);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, WorldEvent::Bounty { .. })),
            "a bounty at a friendly dock"
        );
    }
    assert_eq!(world.money, money, "nothing is paid for a friend");

    // Hostile, and one of its people down: paid once, at its gear's tier.
    world.set_hostile(station, true);
    world.step(&[]);
    let money = world.money;
    let residents = world.residents.as_ref().expect("the station's room");
    assert!(residents.aboard.room.crew_count() > 0, "somebody to shoot");
    let tier = {
        let room = &residents.aboard.room;
        let gear = room.gear(0);
        gear.weapon.map(|w| w.tier.code()).unwrap_or(1)
    };
    let want = crate::world::bounty_for(tier);
    assert!(want > 0, "a body carries something");
    if let Some(residents) = &mut world.residents {
        residents.aboard.room.kill_for_probe(0);
    }
    let mut paid = 0;
    for _ in 0..10 {
        for event in world.step(&[]) {
            if let WorldEvent::Bounty { amount } = event {
                paid += amount;
            }
        }
    }
    assert_eq!(paid, want, "one enemy, one bounty, at its tier");
    assert_eq!(world.money, money + want);

    // And it is never paid twice for the same body.
    let money = world.money;
    for _ in 0..120 {
        for event in world.step(&[]) {
            if let WorldEvent::Bounty { amount } = event {
                assert!(amount > 0);
            }
        }
    }
    // Whatever else went down in those two minutes was somebody new; the
    // one already counted is not counted again.
    let residents = world.residents.as_ref().unwrap();
    let down = (0..residents.aboard.room.crew_count() as usize)
        .filter(|&who| !residents.aboard.room.is_alive(who))
        .count();
    assert!(world.money >= money);
    assert!(down >= 1);
}

/// The bounty table: three tiers, each a step up, and nought for no tier.
#[test]
fn the_bounty_is_by_the_enemy_s_gear_tier() {
    assert_eq!(crate::world::bounty_for(1), 500);
    assert_eq!(crate::world::bounty_for(2), 1_500);
    assert_eq!(crate::world::bounty_for(3), 4_500);
    assert_eq!(crate::world::bounty_for(0), 0, "no tier is no bounty");
    assert_eq!(crate::world::bounty_for(9), 0);
    for tier in 1..3 {
        assert!(data::REPUBLIC_BOUNTY[tier + 1] > data::REPUBLIC_BOUNTY[tier]);
    }
}

/// **Gear is sold only where its trade is** (feature 95): a station rolls
/// a weapon trade and an armour trade off its own seed, and a buy of
/// something it does not stock is `NotSoldHere` however much money there
/// is. Every tier of what it does stock is on sale, at the book times
/// `TIER_PRICE`.
#[test]
fn gear_is_sold_where_its_trade_is_and_every_tier_is_priced() {
    let mut world = a_world();
    let station = match world.ship.state {
        crate::world::ShipState::Docked { station } => station,
        _ => panic!("a world opens docked"),
    };
    world.money = 10_000_000;
    assert!(world.man_the_desk_for_probe(0));
    let stock = world.station(station).unwrap().stock;

    // The two flags are read back off the shelf, and each is all of its
    // list or none of it.
    let weapons = [
        ResourceId::Handgun,
        ResourceId::Shotgun,
        ResourceId::AutoRifle,
        ResourceId::SniperRifle,
        ResourceId::Schword,
    ];
    let armour = [ResourceId::Helm, ResourceId::Kevlar, ResourceId::LegGuard];
    for resource in weapons {
        assert_eq!(stock.sells(resource), stock.weapon_trade(), "{resource:?}");
    }
    for resource in armour {
        assert_eq!(stock.sells(resource), stock.armour_trade(), "{resource:?}");
    }

    // A tier's price is the book times the multiplier, on both sides.
    let one = world.quote_at(station, ResourceId::Handgun, 1).unwrap();
    let two = world.quote_at(station, ResourceId::Handgun, 2).unwrap();
    let three = world.quote_at(station, ResourceId::Handgun, 3).unwrap();
    assert_eq!(two.ask, one.ask * TIER_PRICE[2]);
    assert_eq!(three.bid, one.bid * TIER_PRICE[3]);
    assert!(one.bid < one.ask && two.bid < two.ask && three.bid < three.ask);
    // And a resource that comes at no tier ignores the tier entirely.
    let food = world.quote_at(station, ResourceId::Vegetable, 3).unwrap();
    assert_eq!(food, world.quote(station, ResourceId::Vegetable).unwrap());

    // What it does not stock is refused; what it does arrives at the tier
    // it was bought at.
    let refused = |events: &[WorldEvent]| {
        events.iter().any(|e| {
            matches!(
                e,
                WorldEvent::Refused {
                    why: crate::Refusal::NotSoldHere,
                    ..
                }
            )
        })
    };
    if stock.weapon_trade() {
        let money = world.money;
        let events = world.step(&[Command::Buy {
            slot: 0,
            resource: ResourceId::Handgun,
            units: 1,
            tier: 2,
        }]);
        assert!(!refused(&events), "{events:?}");
        assert_eq!(world.money, money - two.ask, "a tier-two price");
        assert!(
            world
                .guns
                .iter()
                .any(|g| g.kind == WeaponKind::LaserPistol && g.tier == Tier::Two),
            "the gun arrived at the tier it was bought at"
        );
    } else {
        let events = world.step(&[Command::Buy {
            slot: 0,
            resource: ResourceId::Handgun,
            units: 1,
            tier: 1,
        }]);
        assert!(refused(&events), "{events:?}");
    }
}
