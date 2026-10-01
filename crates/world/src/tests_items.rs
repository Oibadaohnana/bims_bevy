//! The items (October 2026, `crate::items`, `bims::module`): four slots a
//! player's Bim and none a bot's; bought at a trader at the day's tier and
//! combined a tier up; the *Blink Drive*'s key, its cooldown and its lock
//! after a hit; the *Executioner*'s crit beside Weak Spot; the *Reactor
//! Heart*'s health and regeneration; and the *Override Core*'s rank past
//! the fourth.

use bims::combat::{Item, Tier};
use bims::module::{ITEM_SLOTS, ModuleKind};
use shipdesign::fixture::flyer;

use crate::class::{self, Class};
use crate::event::{Refusal, WorldEvent};
use crate::fixture::crewed_world;
use crate::holdings::{GearSlot, GearSource};
use crate::run::{Phase, Site};
use crate::world::{Command, World};

/// Money enough for anything on a shelf.
const RICH: economy::Money = 10_000_000;

/// A world of one player and a bot (crew member one).
fn basic() -> World {
    crewed_world(flyer(2), RICH, 1, 2)
}

fn refused_with(events: &[WorldEvent], want: Refusal) -> bool {
    events
        .iter()
        .any(|e| matches!(e, WorldEvent::Refused { why, .. } if *why == want))
}

/// `who`'s item slot `index` set outright, through the room.
fn carry(world: &mut World, who: usize, index: usize, item: Option<bims::module::Module>) {
    let mut gear = world.aboard.room.gear(who);
    gear.items[index] = item;
    world.aboard.room.issue(who, gear);
}

/// Straight to a level.
fn level_up(world: &mut World, who: usize, level: u8) {
    let mut events = Vec::new();
    let want = class::LEVEL_XP[level as usize - 1];
    let have = world.progress_of(who as u32).xp;
    world.award(who, want.saturating_sub(have), &mut events);
}

/// A trader a trip can go to from here, open, and the trip taken.
fn at_a_trader(world: &mut World) -> Site {
    world.leave_for_probe();
    let site = world
        .trader_sites()
        .into_iter()
        .find(|&s| world.travel_quote(s).is_ok())
        .expect("a trader near home, as the start makes sure of");
    world.step(&[Command::Propose {
        slot: 0,
        star: site.star,
        station: site.station,
    }]);
    assert_eq!(world.run.phase, Phase::Trade, "arrived at the trader");
    site
}

/// **Four slots a player's Bim, none a bot's**: an item out of the armory
/// goes into the first free slot, a fifth is `ItemsFull`, and a bot is
/// refused one whatever is free.
#[test]
fn a_player_s_bim_carries_four_items_and_a_bot_none() {
    let mut world = basic();
    world.leave_for_probe();
    let blink = Item::Module(ModuleKind::BlinkDrive.at(Tier::One));
    for n in 0..=ITEM_SLOTS {
        let id = world.holdings.put(blink).unwrap();
        let events = world.step(&[Command::Equip {
            slot: 0,
            who: 0,
            from: GearSource::Armory { id },
        }]);
        if n < ITEM_SLOTS {
            assert_eq!(
                world.items_of(0).iter().flatten().count(),
                n + 1,
                "{events:?}"
            );
        } else {
            assert!(refused_with(&events, Refusal::ItemsFull), "{events:?}");
        }
    }
    let id = world.holdings.armory.last().unwrap().id;
    let events = world.step(&[Command::Equip {
        slot: 0,
        who: 1,
        from: GearSource::Armory { id },
    }]);
    assert!(
        refused_with(&events, Refusal::BotsCarryNoItems),
        "{events:?}"
    );
    assert!(world.items_of(1).iter().all(Option::is_none));
    // Named slot: the item there into the armory, and between the Bim's
    // own two slots a swap.
    let heart = Item::Module(ModuleKind::ReactorHeart.at(Tier::One));
    let id = world.holdings.put(heart).unwrap();
    world.step(&[Command::EquipAt {
        slot: 0,
        who: 0,
        from: GearSource::Armory { id },
        at: GearSlot::Item3,
    }]);
    assert_eq!(
        world.items_of(0)[2].map(|m| m.kind),
        Some(ModuleKind::ReactorHeart)
    );
    world.step(&[Command::EquipAt {
        slot: 0,
        who: 0,
        from: GearSource::Worn {
            who: 0,
            slot: GearSlot::Item3,
        },
        at: GearSlot::Item1,
    }]);
    let items = world.items_of(0);
    assert_eq!(items[0].map(|m| m.kind), Some(ModuleKind::ReactorHeart));
    assert_eq!(items[2].map(|m| m.kind), Some(ModuleKind::BlinkDrive));
}

/// **A trader sells every item at the day's tier**, out of the buyer's own
/// money and onto its own Bim; a bot is refused one; two of a tier
/// combine into one of the next.
#[test]
fn a_trader_sells_items_at_the_day_s_tier_and_two_combine() {
    let mut world = basic();
    at_a_trader(&mut world);
    let tier = world.shop_tier();
    let shelf = world.item_shelf();
    assert_eq!(shelf.len(), ModuleKind::ALL.len());
    let crit = shelf
        .iter()
        .copied()
        .find(|m| m.kind == ModuleKind::Executioner)
        .unwrap();
    assert_eq!(crit.tier, tier);
    // And the gun and the piece are the day's tier too, one each.
    let trader = world.trader_here(0).unwrap();
    assert_eq!(trader.shelf.len(), 2);
    assert!(trader.shelf.iter().flatten().all(|i| i.tier == tier));
    let price = world.item_price(0, crit);
    let before = world.wallet(0);
    let events = world.step(&[Command::BuyItem {
        slot: 0,
        kind: ModuleKind::Executioner.code(),
        to: Some(0),
    }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::ItemBought { .. })),
        "{events:?}"
    );
    assert_eq!(world.wallet(0), before - price);
    assert_eq!(world.items_of(0)[0], Some(crit));
    let events = world.step(&[Command::BuyItem {
        slot: 0,
        kind: ModuleKind::ReactorHeart.code(),
        to: Some(1),
    }]);
    assert!(
        refused_with(&events, Refusal::BotsCarryNoItems),
        "{events:?}"
    );
    let events = world.step(&[Command::BuyItem {
        slot: 0,
        kind: 99,
        to: None,
    }]);
    assert!(refused_with(&events, Refusal::NotForSale), "{events:?}");
    // One of a kind a visit: a second Executioner is sold out until the
    // next, and so is a second blink.
    assert!(world.item_sold(0, ModuleKind::Executioner.code()));
    let events = world.step(&[Command::BuyItem {
        slot: 0,
        kind: ModuleKind::Executioner.code(),
        to: None,
    }]);
    assert!(refused_with(&events, Refusal::SoldOut), "{events:?}");
    world.step(&[Command::BuyItem {
        slot: 0,
        kind: ModuleKind::BlinkDrive.code(),
        to: None,
    }]);
    let events = world.step(&[Command::BuyItem {
        slot: 0,
        kind: ModuleKind::BlinkDrive.code(),
        to: None,
    }]);
    assert!(refused_with(&events, Refusal::SoldOut), "{events:?}");
    // A second blink of the tier from another visit, and the two combined
    // a tier up.
    world
        .holdings
        .put(Item::Module(ModuleKind::BlinkDrive.at(tier)));
    let ids: Vec<u32> = world
        .holdings
        .armory
        .iter()
        .filter(|s| matches!(s.item, Item::Module(m) if m.kind == ModuleKind::BlinkDrive))
        .map(|s| s.id)
        .collect();
    assert_eq!(ids.len(), 2);
    let events = world.step(&[Command::Combine {
        slot: 0,
        a: GearSource::Armory { id: ids[0] },
        b: GearSource::Armory { id: ids[1] },
    }]);
    match tier.next() {
        Some(next) => assert!(
            world
                .holdings
                .armory
                .iter()
                .any(|s| s.item == Item::Module(ModuleKind::BlinkDrive.at(next))),
            "{events:?}"
        ),
        None => assert!(refused_with(&events, Refusal::TopTier), "{events:?}"),
    }
}

/// **A Reactor Heart raises the bar and puts hit points back**: a full
/// bar stays full with it put on, and a Bim nobody hits mends at the quiet
/// rate.
#[test]
fn a_reactor_heart_raises_the_bar_and_mends() {
    let mut world = basic();
    assert_eq!(world.aboard.room.max_health(0), bims::health::MAX_HEALTH);
    let heart = ModuleKind::ReactorHeart.at(Tier::Two);
    carry(&mut world, 0, 0, Some(heart));
    let max = bims::health::MAX_HEALTH + bims::module::HEART_HEALTH[1];
    assert_eq!(world.aboard.room.max_health(0), max);
    assert_eq!(world.aboard.room.health(0), max, "full stays full");
    world.aboard.room.wound(0, 60.0);
    let hurt = world.aboard.room.health(0);
    assert!(hurt < max);
    // A second of steps: the quiet rate, nothing having hit it.
    for _ in 0..60 {
        world.step(&[]);
    }
    let mended = world.aboard.room.health(0) - hurt;
    let want = bims::module::HEART_QUIET_REGEN[1];
    assert!((mended - want).abs() < 0.5, "mended {mended}, want {want}");
    // Taken off, the bar is a hundred again.
    carry(&mut world, 0, 0, None);
    assert_eq!(world.aboard.room.max_health(0), bims::health::MAX_HEALTH);
}

/// **A Blink Drive puts the Bim where it is aimed**, as far as it reaches,
/// then cools down; and a hit taken locks it for a few seconds.
#[test]
fn a_blink_drive_moves_the_bim_and_cools_down() {
    let mut world = basic();
    let events = world.step(&[Command::UseItem {
        slot: 0,
        item: 0,
        x: 0,
        y: 0,
    }]);
    assert!(refused_with(&events, Refusal::NoSuchItem), "{events:?}");
    carry(&mut world, 0, 0, Some(ModuleKind::BlinkDrive.at(Tier::One)));
    let reach = bims::module::BLINK_RANGE_TILES[0] * bims::room::TILE;
    let from = world.aboard.room.bim_pos(0);
    // Somewhere the drive would put it, looked for round about.
    let (aim, spot) = (0..16)
        .find_map(|n| {
            let a = n as f32 / 16.0 * std::f32::consts::TAU;
            let aim = from + bims::math::vec2(a.cos(), a.sin()) * reach * 2.0;
            world
                .aboard
                .room
                .blink_spot(0, aim, reach)
                .filter(|s| (*s - from).len() > bims::room::TILE * 2.0)
                .map(|s| (aim, s))
        })
        .expect("open deck round the crew member");
    assert!((spot - from).len() <= reach + 1.0, "never past the reach");
    let events = world.step(&[Command::UseItem {
        slot: 0,
        item: 0,
        x: aim.x.round() as i32,
        y: aim.y.round() as i32,
    }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Blinked { who: 0 })),
        "{events:?}"
    );
    let at = world.aboard.room.bim_pos(0);
    assert!(
        (at - from).len() > bims::room::TILE * 2.0,
        "moved: {from:?} to {at:?}"
    );
    assert!(world.item_cooldown_left(0, 0) > 0.0);
    let events = world.step(&[Command::UseItem {
        slot: 0,
        item: 0,
        x: from.x as i32,
        y: from.y as i32,
    }]);
    assert!(refused_with(&events, Refusal::CoolingDown), "{events:?}");
    // The cooldown run out, a hit locks it.
    world.run.items.ready_at.clear();
    world.run.items.hurt_at = vec![Some(world.mission_minutes())];
    assert!(world.blink_locked_left(0) > 0.0);
    let events = world.step(&[Command::UseItem {
        slot: 0,
        item: 0,
        x: from.x as i32,
        y: from.y as i32,
    }]);
    assert!(refused_with(&events, Refusal::BlinkLocked), "{events:?}");
}

/// **An Executioner beside Weak Spot**: each rolls on its own, so the
/// chance is either's and the multiple the bigger.
#[test]
fn an_executioner_and_weak_spot_roll_apart() {
    let mut world = basic();
    assert_eq!(world.crit_of(0), None);
    carry(
        &mut world,
        0,
        0,
        Some(ModuleKind::Executioner.at(Tier::One)),
    );
    let one = (
        bims::module::EXECUTIONER_CHANCE[0],
        bims::module::EXECUTIONER_DAMAGE[0],
    );
    assert_eq!(world.crit_of(0), Some(one));
    assert_eq!(world.skill_of(0).crit_chance, one.0);
    assert_eq!(world.set_class(0, Class::Soldier), Ok(()));
    level_up(&mut world, 0, 16);
    world.set_ranks_for_probe(0, [0, 4, 0, 0]);
    let (chance, damage) = world.crit_of(0).unwrap();
    let want = 1.0 - (1.0 - class::WEAK_SPOT_CHANCE[3]) * (1.0 - one.0);
    assert!((chance - want).abs() < 1e-6);
    assert_eq!(damage, class::WEAK_SPOT_DAMAGE[3].max(one.1));
}

/// **An Override Core plays the ultimate a rank higher**, up to a fifth
/// no point buys, and nothing without a rank bought.
#[test]
fn an_override_core_is_a_rank_past_the_fourth() {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Tank), Ok(()));
    let core = Some(ModuleKind::OverrideCore.at(Tier::One));
    carry(&mut world, 0, 0, core);
    assert_eq!(
        world.rank_of(0, class::SLOT_R),
        0,
        "nothing bought, nothing added"
    );
    level_up(&mut world, 0, 16);
    world.set_ranks_for_probe(0, [1, 1, 1, 2]);
    assert_eq!(world.bought_rank_of(0, class::SLOT_R), 2);
    assert_eq!(world.rank_of(0, class::SLOT_R), 3);
    assert_eq!(world.rank_of(0, class::SLOT_Q), 1, "the ultimate alone");
    world.set_ranks_for_probe(0, [1, 1, 1, 4]);
    assert_eq!(world.rank_of(0, class::SLOT_R), class::OVERRIDE_RANK);
    assert_eq!(
        world.juggernaut_seconds(0),
        class::JUGGERNAUT_SECONDS[4],
        "the fifth row read"
    );
    carry(&mut world, 0, 0, None);
    assert_eq!(world.rank_of(0, class::SLOT_R), 4);
}
