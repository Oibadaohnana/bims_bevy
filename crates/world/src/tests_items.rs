//! The items (October 2026, `crate::items`, `bims::module`): four slots a
//! player's Bim and none a bot's; bought at a trader at the day's tier,
//! upgraded a tier at the traders after and sold back for half; the
//! *Blink Drive*'s key, its cooldown and its lock
//! after a hit; the *Executioner*'s crit beside Weak Spot; the *Reactor
//! Heart*'s health and regeneration; and the *Override Core*'s rank past
//! the fourth.

use bims::combat::{Item, Tier};
use bims::module::{ITEM_SLOTS, Module, ModuleKind};
use shipdesign::fixture::flyer;

use crate::class::{self, Class};
use crate::event::{Refusal, WorldEvent};
use crate::fixture::crewed_world;
use crate::holdings::{GearSlot, GearSource};
use crate::items::ItemOffer;
use crate::run::{Phase, Site};
use crate::trader;
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
/// refused one whatever is free — a bot is nobody's to change
/// (`NotYours`, October 2026).
#[test]
fn a_player_s_bim_carries_four_items_and_a_bot_none() {
    let mut world = basic();
    world.leave_for_probe();
    let blink = Item::Module(ModuleKind::BlinkDrive.at(Tier::One));
    for n in 0..=ITEM_SLOTS {
        let id = world.holdings.put(0, blink).unwrap();
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
    assert!(refused_with(&events, Refusal::NotYours), "{events:?}");
    assert!(world.items_of(1).iter().all(Option::is_none));
    // Named slot: the item there into the armory, and between the Bim's
    // own two slots a swap.
    let heart = Item::Module(ModuleKind::ReactorHeart.at(Tier::One));
    let id = world.holdings.put(0, heart).unwrap();
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

/// **A tier-two or tier-three zone's trader sells as a tier-one zone's**
/// (October 2026, the player's word): with the day past every tier timing,
/// every item is still at its lowest tier and every gun and the armour at
/// tier one (or its kind's lowest) — the Reset Capacitor, made at tier
/// three alone, on the shelf all the same.
#[test]
fn a_trader_past_the_tier_days_sells_what_it_sold_on_the_first_day() {
    let mut world = basic();
    world.set_wave_scaling(crate::droid::WaveScaling {
        tier2_days: 0,
        tier3_days: 0,
        ..crate::droid::WaveScaling::DEFAULT
    });
    assert_eq!(world.zone_tier(), Tier::Three, "the day is past tier three");
    at_a_trader(&mut world);
    for item in world.item_shelf() {
        assert_eq!(item.tier, item.kind.min_tier(), "{item:?}");
    }
    assert!(
        world
            .item_shelf()
            .iter()
            .any(|m| m.kind == ModuleKind::ResetCapacitor)
    );
    let trader = world.trader_here(0).unwrap();
    let shelf: Vec<_> = trader.shelf.iter().flatten().copied().collect();
    assert_eq!(shelf, crate::trader::shelf(|_| Tier::One));
    assert_eq!(
        world.item_offer(0, ModuleKind::Executioner.code()),
        Some(ItemOffer::Buy(ModuleKind::Executioner.at(Tier::One)))
    );
}

/// **A trader sells every item at its lowest tier**, out of the buyer's own
/// money and onto its own Bim — never into the armory, never a bot's —
/// what was paid kept on the item; one of a kind a visit, and a new kind
/// wants a free slot.
#[test]
fn a_trader_sells_items_at_their_lowest_tier_onto_the_buyer_s_own_bim() {
    let mut world = basic();
    at_a_trader(&mut world);
    let tier = Tier::One;
    let shelf = world.item_shelf();
    assert_eq!(shelf.len(), ModuleKind::ALL.len());
    let crit = shelf
        .iter()
        .copied()
        .find(|m| m.kind == ModuleKind::Executioner)
        .unwrap();
    assert_eq!(crit.tier, tier);
    assert_eq!(
        world.item_offer(0, ModuleKind::Executioner.code()),
        Some(ItemOffer::Buy(crit))
    );
    // And every gun and the armour are tier one too (or their kind's
    // lowest, above it).
    let trader = world.trader_here(0).unwrap();
    let shelf: Vec<_> = trader.shelf.iter().flatten().copied().collect();
    assert_eq!(shelf, crate::trader::shelf(|_| tier));
    let price = world.item_price(crit);
    let before = world.wallet(0);
    let events = world.step(&[Command::BuyItem {
        slot: 0,
        kind: ModuleKind::Executioner.code(),
    }]);
    assert!(
        events.iter().any(|e| matches!(
            e,
            WorldEvent::ItemBought {
                to: 0,
                upgrade: false,
                ..
            }
        )),
        "{events:?}"
    );
    assert_eq!(world.wallet(0), before - price);
    assert_eq!(
        world.items_of(0)[0],
        Some(Module {
            paid: price,
            ..crit
        })
    );
    assert!(world.holdings.armory.is_empty(), "never into the armory");
    let events = world.step(&[Command::BuyItem { slot: 0, kind: 99 }]);
    assert!(refused_with(&events, Refusal::NotForSale), "{events:?}");
    // One of a kind a visit: the Executioner is sold out until the next.
    assert!(world.item_sold(0, ModuleKind::Executioner.code()));
    let events = world.step(&[Command::BuyItem {
        slot: 0,
        kind: ModuleKind::Executioner.code(),
    }]);
    assert!(refused_with(&events, Refusal::SoldOut), "{events:?}");
    // A new kind wants a free slot.
    for index in 1..ITEM_SLOTS {
        carry(&mut world, 0, index, Some(ModuleKind::LongBarrel.at(tier)));
    }
    let events = world.step(&[Command::BuyItem {
        slot: 0,
        kind: ModuleKind::ReactorHeart.code(),
    }]);
    assert!(refused_with(&events, Refusal::ItemsFull), "{events:?}");
}

/// **An item carried is offered a tier up at every trader after**, at the
/// next tier's whole price, made in the slot it is in — every slot full
/// or not — what was paid adding up; past tier three nothing.
#[test]
fn an_item_carried_is_upgraded_a_tier_at_every_later_trader() {
    let mut world = basic();
    at_a_trader(&mut world);
    let tier = Tier::One;
    let first = world.item_price(ModuleKind::Executioner.at(tier));
    world.step(&[Command::BuyItem {
        slot: 0,
        kind: ModuleKind::Executioner.code(),
    }]);
    // Moved to the third slot, and every other slot full.
    let held = world.items_of(0)[0].unwrap();
    carry(&mut world, 0, 0, Some(ModuleKind::LongBarrel.at(tier)));
    carry(&mut world, 0, 1, Some(ModuleKind::LongBarrel.at(tier)));
    carry(&mut world, 0, 2, Some(held));
    carry(&mut world, 0, 3, Some(ModuleKind::LongBarrel.at(tier)));
    // The next visit (`Trader::restock` clears what was sold).
    let mut paid = first;
    let mut at = tier;
    while let Some(up) = at.next() {
        for trader in &mut world.run.traders {
            trader.items_sold.clear();
        }
        let offer = world.item_offer(0, ModuleKind::Executioner.code());
        let Some(ItemOffer::Upgrade { at: 2, from, to }) = offer else {
            panic!("an upgrade of the one in the third slot: {offer:?}");
        };
        assert_eq!((from.tier, to.tier), (at, up));
        let price = world.item_price(ModuleKind::Executioner.at(up));
        assert_eq!(world.item_offer_price(offer.unwrap()), Some(price));
        let before = world.wallet(0);
        let events = world.step(&[Command::BuyItem {
            slot: 0,
            kind: ModuleKind::Executioner.code(),
        }]);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, WorldEvent::ItemBought { upgrade: true, .. })),
            "{events:?}"
        );
        assert_eq!(world.wallet(0), before - price);
        paid += price;
        assert_eq!(
            world.items_of(0)[2],
            Some(Module {
                paid,
                ..ModuleKind::Executioner.at(up)
            })
        );
        at = up;
    }
    assert_eq!(at, Tier::Three);
    for trader in &mut world.run.traders {
        trader.items_sold.clear();
    }
    assert!(matches!(
        world.item_offer(0, ModuleKind::Executioner.code()),
        Some(ItemOffer::Top(_))
    ));
    let events = world.step(&[Command::BuyItem {
        slot: 0,
        kind: ModuleKind::Executioner.code(),
    }]);
    assert!(refused_with(&events, Refusal::TopTier), "{events:?}");
    // The Override Core is one tier: carried, it is at its top.
    carry(
        &mut world,
        0,
        0,
        Some(ModuleKind::OverrideCore.at(Tier::One)),
    );
    assert!(matches!(
        world.item_offer(0, ModuleKind::OverrideCore.code()),
        Some(ItemOffer::Top(_))
    ));
}

/// **A thing sold fetches half of what was paid for it**: an item every
/// upgrade added in, one never bought half its tier's price, a gun out of
/// the armory and a bot's half its shelf price — into the seller's own
/// wallet, the slot left empty; nowhere but at a trader, never a charge.
#[test]
fn a_thing_sold_fetches_half_of_what_was_paid() {
    let mut world = basic();
    world.leave_for_probe();
    let gun = Item::Weapon(bims::combat::WeaponKind::AutoRifle.at(Tier::Two));
    let id = world.holdings.put(0, gun).unwrap();
    let events = world.step(&[Command::Sell {
        slot: 0,
        from: GearSource::Armory { id },
    }]);
    assert!(refused_with(&events, Refusal::NotAtATrader), "{events:?}");
    at_a_trader(&mut world);
    // Bought and upgraded: half of both prices.
    world.step(&[Command::BuyItem {
        slot: 0,
        kind: ModuleKind::Executioner.code(),
    }]);
    for trader in &mut world.run.traders {
        trader.items_sold.clear();
    }
    world.step(&[Command::BuyItem {
        slot: 0,
        kind: ModuleKind::Executioner.code(),
    }]);
    let crit = world.items_of(0)[0].unwrap();
    assert!(crit.paid > 0);
    let worn = GearSource::Worn {
        who: 0,
        slot: GearSlot::Item1,
    };
    assert_eq!(
        world.sellable(0, worn),
        Ok((Item::Module(crit), crit.paid / 2))
    );
    let before = world.wallet(0);
    let events = world.step(&[Command::Sell {
        slot: 0,
        from: worn,
    }]);
    assert!(
        events.iter().any(|e| matches!(
            e,
            WorldEvent::Sold {
                slot: 0,
                who: 0,
                value
            } if *value == crit.paid / 2
        )),
        "{events:?}"
    );
    assert_eq!(world.wallet(0), before + crit.paid / 2);
    assert_eq!(world.items_of(0)[0], None);
    // Sold, the kind is the day's shelf's again.
    assert!(matches!(
        world.item_offer(0, ModuleKind::Executioner.code()),
        Some(ItemOffer::Buy(_))
    ));
    // One never bought: half its tier's price today.
    let heart = ModuleKind::ReactorHeart.at(Tier::Two);
    carry(&mut world, 0, 1, Some(heart));
    assert_eq!(
        world.sell_value(Item::Module(heart)),
        Some(world.item_price(heart) / 2)
    );
    // The gun out of the armory, at half its shelf price.
    let shelf = trader::ShelfItem {
        resource: crate::armour::weapon_resource(bims::combat::WeaponKind::AutoRifle),
        tier: Tier::Two,
    };
    let half = world.shelf_price(shelf) / 2;
    let before = world.wallet(0);
    world.step(&[Command::Sell {
        slot: 0,
        from: GearSource::Armory { id },
    }]);
    assert_eq!(world.wallet(0), before + half);
    assert!(world.holdings.get(id).is_none());
    // A bot's gun is never the player's to sell (October 2026: a bot
    // keeps the kit it came with).
    let bots = GearSource::Worn {
        who: 1,
        slot: GearSlot::Weapon,
    };
    assert_eq!(world.sellable(0, bots), Err(Refusal::NotYours));
    let before = world.wallet(0);
    let events = world.step(&[Command::Sell {
        slot: 0,
        from: bots,
    }]);
    assert_eq!(world.wallet(0), before, "{events:?}");
    assert!(world.worn_on(1, GearSlot::Weapon).is_some());
    // A charge is no thing to sell.
    assert_eq!(world.sell_value(Item::Stack(0)), None);
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
    // A share of the whole bar (October 2026).
    let want = bims::module::HEART_QUIET_REGEN[1] * max / 100.0;
    assert!((mended - want).abs() < 0.5, "mended {mended}, want {want}");
    // Taken off, the bar is a hundred again.
    carry(&mut world, 0, 0, None);
    assert_eq!(world.aboard.room.max_health(0), bims::health::MAX_HEALTH);
}

/// **Every level is ten hit points on a player's bar** (October 2026):
/// 110 at the first level, 260 at the sixteenth, a full bar staying full
/// as it grows and a hurt one keeping its share; a Reactor Heart's on
/// top; a classless bot never more than a hundred.
#[test]
fn every_level_is_ten_hit_points_on_a_player_s_bar() {
    let mut world = basic();
    world.set_class(0, Class::Soldier).unwrap();
    world.step(&[]);
    let room = |w: &World| (w.aboard.room.health(0), w.aboard.room.max_health(0));
    assert_eq!(room(&world), (110.0, 110.0), "the first level, full");
    assert_eq!(world.aboard.room.max_health(1), bims::health::MAX_HEALTH);
    level_up(&mut world, 0, 16);
    world.step(&[]);
    assert_eq!(room(&world), (260.0, 260.0), "the sixteenth, still full");
    world.aboard.room.wound(0, 130.0);
    let heart = ModuleKind::ReactorHeart.at(Tier::One);
    carry(&mut world, 0, 0, Some(heart));
    let max = 260.0 + bims::module::HEART_HEALTH[0];
    assert_eq!(world.aboard.room.max_health(0), max, "the item on top");
    world.step(&[]);
    assert_eq!(world.aboard.room.max_health(0), max, "kept the next step");
    assert_eq!(world.aboard.room.max_health(1), bims::health::MAX_HEALTH);
}

/// **Every heal is a share of the bar** (October 2026, the player's word:
/// "a heart heals 0.5% of maxhp/s"): a tier-one Reactor Heart puts back
/// half a per cent of the whole bar a second, so a bar of 285 (the
/// sixteenth level's and the Heart's own) mends 1.425 a second where a
/// hundred mends half a point.
#[test]
fn a_reactor_heart_heals_half_a_per_cent_of_the_bar_a_second() {
    let mut world = basic();
    world.set_class(0, Class::Soldier).unwrap();
    level_up(&mut world, 0, 16);
    world.step(&[]);
    let heart = ModuleKind::ReactorHeart.at(Tier::One);
    carry(&mut world, 0, 0, Some(heart));
    world.step(&[]);
    let max = world.aboard.room.max_health(0);
    assert_eq!(max, 260.0 + bims::module::HEART_HEALTH[0]);
    world.aboard.room.wound(0, 100.0);
    world.step(&[]);
    // The plain rate or the quiet one, whichever the wound left it at:
    // both are shares of the bar.
    let rate = world.item_regen_now(0);
    assert!(
        rate == bims::module::HEART_REGEN[0] || rate == bims::module::HEART_QUIET_REGEN[0],
        "rate {rate}"
    );
    let hurt = world.aboard.room.health(0);
    // A second at 1×: sixty steps, none of them a hit.
    for _ in 0..60 {
        world.step(&[]);
    }
    let mended = world.aboard.room.health(0) - hurt;
    let want = max * rate / 100.0;
    assert!((mended - want).abs() < 0.05, "mended {mended}, want {want}");
}

/// **The four levels past the sixteenth** (October 2026): ten hit points
/// each as every level is, five per cent more weapon damage each, and no
/// skill point — sixteen buy every rank.
#[test]
fn the_levels_past_sixteen_are_hit_points_and_weapon_damage() {
    let mut world = basic();
    world.set_class(0, Class::Soldier).unwrap();
    level_up(&mut world, 0, 16);
    world.step(&[]);
    assert_eq!(world.skill_of(0).damage, 1.0, "nothing more at sixteen");
    assert_eq!(world.points_of(0), 16);
    for level in 17..=class::LEVELS {
        level_up(&mut world, 0, level);
        world.step(&[]);
        assert_eq!(world.level_of(0), level);
        let more = 0.05 * (level - 16) as f32;
        assert!(
            (world.skill_of(0).damage - (1.0 + more)).abs() < 1e-5,
            "level {level}: {}",
            world.skill_of(0).damage
        );
        assert_eq!(world.points_of(0), 16, "level {level}: no point");
        assert_eq!(
            world.aboard.room.max_health(0),
            bims::health::MAX_HEALTH + 10.0 * level as f32
        );
    }
    assert_eq!(world.aboard.room.max_health(0), 300.0, "the twentieth");
    assert_eq!(world.skill_of(1).damage, 1.0, "a bot never levels");
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
        world.bastion_radius(0),
        class::BASTION_RADIUS[4],
        "the fifth row read"
    );
    carry(&mut world, 0, 0, None);
    assert_eq!(world.rank_of(0, class::SLOT_R), 4);
}

// --- step two (October 2026) ---------------------------------------------------

/// **The three relics in item form, and the Long Barrel**: a Coolant
/// Loop shortens the class's cooldowns, a Steady Grip lifts the fire rate,
/// a Long Barrel the range, and a Pressure Seal raises the bar (it
/// mended all the time until October 2026) and mends nothing.
#[test]
fn the_passives_lift_the_skill_cut_the_cooldowns_and_raise_the_bar() {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Soldier), Ok(()));
    level_up(&mut world, 0, 16);
    world.set_ranks_for_probe(0, [1, 0, 0, 1]);
    // A step, so the bar is the level's before anybody is hurt.
    world.step(&[]);
    let charge = crate::class::Charge::Grenade;
    let (cooldown, rampage) = (world.charge_cooldown(0, charge), world.rampage_cooldown(0));
    let skill = world.skill_of(0);
    carry(
        &mut world,
        0,
        0,
        Some(ModuleKind::CoolantLoop.at(Tier::Three)),
    );
    carry(
        &mut world,
        0,
        1,
        Some(ModuleKind::SteadyGrip.at(Tier::Three)),
    );
    carry(
        &mut world,
        0,
        2,
        Some(ModuleKind::LongBarrel.at(Tier::Three)),
    );
    let cut = 1.0 - f64::from(bims::module::COOLANT_LOOP_PERCENT[2]) / 100.0;
    assert!((world.charge_cooldown(0, charge) - cooldown * cut).abs() < 1e-6);
    assert!((world.rampage_cooldown(0) - rampage * cut).abs() < 1e-6);
    let lifted = world.skill_of(0);
    let rate = 1.0 + bims::module::STEADY_GRIP_PERCENT[2] as f32 / 100.0;
    assert!((lifted.fire_rate - skill.fire_rate * rate).abs() < 1e-5);
    assert_eq!(
        lifted.range,
        skill.range + bims::module::LONG_BARREL_TILES[2]
    );
    // A Pressure Seal: its health on the bar, and no regeneration.
    let bar = world.aboard.room.max_health(0);
    carry(
        &mut world,
        0,
        3,
        Some(ModuleKind::PressureSeal.at(Tier::Three)),
    );
    world.step(&[]);
    let raised = world.aboard.room.max_health(0);
    let want = bar + bims::module::PRESSURE_SEAL_HEALTH[2];
    assert!((raised - want).abs() < 1e-3, "bar {raised}, want {want}");
    assert_eq!(world.item_regen_now(0), 0.0);
    world.aboard.room.wound(0, 50.0);
    let hurt = world.aboard.room.health(0);
    for _ in 0..60 {
        world.step(&[]);
    }
    assert_eq!(world.aboard.room.health(0), hurt, "it mends nothing");
}

/// **The three new actives**: a Field Mender heals the crew round its
/// holder, a Reset Capacitor makes a Rampage ready again, and an Ablative
/// Shell takes damage down and stops the stripping while it lasts.
#[test]
fn the_mender_heals_the_reset_readies_and_the_shell_shields() {
    let mut world = basic();
    assert_eq!(world.set_class(0, Class::Soldier), Ok(()));
    level_up(&mut world, 0, 16);
    world.set_ranks_for_probe(0, [0, 0, 0, 1]);
    // A step, so the bar is the level's before anybody is hurt.
    world.step(&[]);
    carry(
        &mut world,
        0,
        0,
        Some(ModuleKind::FieldMender.at(Tier::One)),
    );
    carry(
        &mut world,
        0,
        1,
        Some(ModuleKind::ResetCapacitor.at(Tier::Three)),
    );
    carry(
        &mut world,
        0,
        2,
        Some(ModuleKind::AblativeShell.at(Tier::One)),
    );
    // The bot beside its player, both hurt.
    let at = world.aboard.room.bim_pos(0);
    world
        .aboard
        .room
        .put_for_probe(1, at + bims::math::vec2(bims::room::TILE, 0.0));
    world.aboard.room.wound(0, 50.0);
    world.aboard.room.wound(1, 50.0);
    let before = [world.aboard.room.health(0), world.aboard.room.health(1)];
    let use_item = |world: &mut World, item: u32| {
        world.step(&[Command::UseItem {
            slot: 0,
            item,
            x: 0,
            y: 0,
        }])
    };
    let events = use_item(&mut world, 0);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::ItemUsed { who: 0, .. })),
        "{events:?}"
    );
    for who in 0..2 {
        // A share of each one's bar (October 2026), never past it.
        let max = world.aboard.room.max_health(who);
        let heal = (bims::module::MENDER_HEAL[0] * max / 100.0).min(max - before[who]);
        let healed = world.aboard.room.health(who) - before[who];
        assert!((healed - heal).abs() < 1.0, "{who} healed {healed}");
    }
    let events = use_item(&mut world, 0);
    assert!(refused_with(&events, Refusal::CoolingDown), "{events:?}");
    // A Rampage gone on, then ready again under the Reset — and the
    // Mender's cooldown with it.
    world.step(&[Command::Rampage { slot: 0 }]);
    assert!(world.rampage_cooldown_left(0) > 0.0);
    use_item(&mut world, 1);
    assert_eq!(world.rampage_cooldown_left(0), 0.0);
    assert_eq!(world.item_cooldown_left(0, 0), 0.0);
    assert!(world.item_cooldown_left(0, 1) > 0.0);
    // The shell.
    let skill = world.skill_of(0);
    assert!(!skill.unstrippable);
    use_item(&mut world, 2);
    let shelled = world.skill_of(0);
    assert!(shelled.unstrippable);
    assert!(
        (shelled.damage_taken - skill.damage_taken * bims::module::SHELL_DAMAGE_TAKEN).abs() < 1e-5
    );
    let steps = (bims::module::SHELL_SECONDS[0] * 60.0) as usize + 2;
    for _ in 0..steps {
        world.step(&[]);
    }
    assert!(!world.skill_of(0).unstrippable, "worn off");
    // A passive item's key does nothing.
    carry(&mut world, 0, 3, Some(ModuleKind::SteadyGrip.at(Tier::One)));
    let events = use_item(&mut world, 3);
    assert!(refused_with(&events, Refusal::NoSuchItem), "{events:?}");
}

/// **A Leech Capacitor** gives back its share of what a hit did, and an
/// **Arc Coil** every fourth hit arcs to the machines round the one
/// struck, the nearest first.
#[test]
fn a_leech_gives_back_and_an_arc_jumps_every_fourth_hit() {
    let (mut world, _) = arena();
    carry(
        &mut world,
        0,
        0,
        Some(ModuleKind::LeechCapacitor.at(Tier::Two)),
    );
    carry(&mut world, 0, 1, Some(ModuleKind::ArcCoil.at(Tier::One)));
    world.aboard.room.wound(0, 50.0);
    let hurt = world.aboard.room.health(0);
    // Three machines a tile apart, a fourth far off.
    let room = &mut world.residents.as_mut().unwrap().aboard.room;
    assert!(room.droid_count() >= 4, "{} machines", room.droid_count());
    let from = room.droid(0).unwrap().pos;
    for (j, dx) in [(1, 1.0), (2, 2.0), (3, 30.0)] {
        room.droid_mut_for_probe(j).unwrap().pos =
            from + bims::math::vec2(dx * bims::room::TILE, 0.0);
    }
    let life = |world: &World, j: usize| {
        world
            .residents
            .as_ref()
            .unwrap()
            .aboard
            .room
            .droid(j)
            .unwrap()
            .body
            .life_share()
    };
    // The first machine, by its body index.
    let bims = world.residents.as_ref().unwrap().aboard.room.crew_count() as usize;
    for n in 1..=4 {
        world.items_on_enemy_hits(&[(Some(0), bims, 10.0)]);
        if n < 4 {
            assert_eq!(life(&world, 1), 1.0, "no arc on hit {n}");
        }
    }
    let healed = world.aboard.room.health(0) - hurt;
    let want = 4.0 * 10.0 * bims::module::LEECH_SHARE[1];
    assert!((healed - want).abs() < 0.01, "healed {healed}, want {want}");
    assert!(
        life(&world, 1) < 1.0 && life(&world, 2) < 1.0,
        "the two near"
    );
    assert_eq!(life(&world, 3), 1.0, "not the one far off");
}

/// The fight's arena, held, its first wave standing, one player and the
/// combat ship's crew.
fn arena() -> (World, u32) {
    use shipdesign::fixture::{COMBAT_CREW, combat_ship};
    let galaxy = worldgen::Galaxy::new(
        crate::data::DEFAULT_SEED,
        worldgen::GalaxyType::SpiralTwoArm,
    );
    let (star, station) = crate::spawn(&galaxy).unwrap();
    let mut world = World::start_with_crew(
        combat_ship(),
        RICH,
        1,
        COMBAT_CREW,
        crate::data::DEFAULT_SEED,
        worldgen::GalaxyType::SpiralTwoArm,
        star,
        station,
    )
    .unwrap();
    world.arena_dock_for_probe();
    world.infest(station);
    world.set_droid_kinds_for_probe(vec![bims::droid::DroidKind::Trooper; 4]);
    for _ in 0..40 {
        world.step(&[]);
        if world.droids_standing() > 0 {
            break;
        }
    }
    (world, station)
}

/// **A Targeting Uplink and an Overcharger** (October 2026): a bot within
/// the uplink's reach of its carrier does its share more damage and one
/// beyond it none; an Overcharger lifts its carrier's own damage and the
/// flat damage a crit is a multiple of.
#[test]
fn an_uplink_lifts_a_bot_near_its_carrier_and_an_overcharger_the_crit() {
    let mut world = basic();
    world.step(&[]);
    let tile = bims::room::TILE;
    let at = world.aboard.room.bim_pos(0);
    world
        .aboard
        .room
        .put_for_probe(1, at + bims::math::vec2(tile * 2.0, 0.0));
    let plain = world.skill_of(1).damage;
    carry(
        &mut world,
        0,
        0,
        Some(ModuleKind::TargetingUplink.at(Tier::Three)),
    );
    let lift = 1.0 + bims::module::UPLINK_DAMAGE_PERCENT[2] as f32 / 100.0;
    assert!((world.skill_of(1).damage - plain * lift).abs() < 1e-5);
    world.aboard.room.put_for_probe(
        1,
        at + bims::math::vec2(tile * (bims::module::UPLINK_TILES + 2.0), 0.0),
    );
    assert!(
        (world.skill_of(1).damage - plain).abs() < 1e-5,
        "out of reach"
    );
    let own = world.skill_of(0).damage;
    assert_eq!(world.item_damage_factor(0), 1.0);
    carry(
        &mut world,
        0,
        1,
        Some(ModuleKind::Overcharger.at(Tier::Two)),
    );
    let over = 1.0 + bims::module::OVERCHARGE_PERCENT[1] as f32 / 100.0;
    assert!((world.skill_of(0).damage - own * over).abs() < 1e-5);
    assert!((world.item_damage_factor(0) - over).abs() < 1e-6);
}

/// **An Adrenal Injector** fires on its own under its share of the bar:
/// the rush on, the fire rate and the pace up, said as the item used —
/// and not again until its cooldown is out.
#[test]
fn an_adrenal_injector_rushes_under_a_third_of_the_bar_and_cools_down() {
    let mut world = basic();
    world.step(&[]);
    carry(
        &mut world,
        0,
        2,
        Some(ModuleKind::AdrenalInjector.at(Tier::One)),
    );
    let skill = world.skill_of(0);
    let events = world.step(&[]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::ItemUsed { .. })),
        "nothing at a whole bar"
    );
    let bar = world.aboard.room.max_health(0);
    world.aboard.room.wound(0, bar * 0.8);
    let events = world.step(&[]);
    assert!(
        events.iter().any(|e| matches!(
            e,
            WorldEvent::ItemUsed { who: 0, kind } if *kind == ModuleKind::AdrenalInjector.code()
        )),
        "{events:?}"
    );
    assert!(world.adrenal_left(0) > 0.0);
    assert!(world.item_cooldown_left(0, 2) > 0.0);
    let rushed = world.skill_of(0);
    let up = 1.0 + bims::module::ADRENAL_PERCENT[0] as f32 / 100.0;
    assert!((rushed.fire_rate - skill.fire_rate * up).abs() < 1e-5);
    assert!((rushed.walk - skill.walk * up).abs() < 1e-5);
    let events = world.step(&[]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::ItemUsed { .. })),
        "cooling down"
    );
}

/// **A Tether Link** holds the crewmate at the pointer: it takes its share
/// less of a hit and the carrier the rest, past the carrier's armour; one
/// out of reach is refused, and nobody at the pointer is nobody.
#[test]
fn a_tether_link_takes_its_share_of_the_crewmate_s_hits() {
    let mut world = basic();
    world.step(&[]);
    let tile = bims::room::TILE;
    let at = world.aboard.room.bim_pos(0);
    carry(
        &mut world,
        0,
        0,
        Some(ModuleKind::TetherLink.at(Tier::Three)),
    );
    let nobody = at + bims::math::vec2(0.0, tile * 40.0);
    let events = world.step(&[Command::UseItem {
        slot: 0,
        item: 0,
        x: nobody.x as i32,
        y: nobody.y as i32,
    }]);
    assert!(refused_with(&events, Refusal::NotACrewmate), "{events:?}");
    let far = at + bims::math::vec2(tile * (bims::module::TETHER_RANGE_TILES + 3.0), 0.0);
    world.aboard.room.put_for_probe(1, far);
    let events = world.step(&[Command::UseItem {
        slot: 0,
        item: 0,
        x: far.x as i32,
        y: far.y as i32,
    }]);
    assert!(refused_with(&events, Refusal::OutOfItemRange), "{events:?}");
    let near = at + bims::math::vec2(tile * 3.0, 0.0);
    world.aboard.room.put_for_probe(1, near);
    let plain = world.skill_of(1).damage_taken;
    world.step(&[Command::UseItem {
        slot: 0,
        item: 0,
        x: near.x as i32,
        y: near.y as i32,
    }]);
    let share = bims::module::TETHER_SHARE[2];
    assert_eq!(world.run.items.tethers.len(), 1);
    assert!((world.skill_of(1).damage_taken - plain * (1.0 - share)).abs() < 1e-5);
    // A hit on the crewmate between the hand-over and the settling, as a
    // bolt landing in the room's step is.
    let tethered = world.hand_the_rooms_the_items();
    let (mate, carrier) = (world.aboard.room.health(1), world.aboard.room.health(0));
    world.aboard.room.drain(1, 30.0);
    let lost = mate - world.aboard.room.health(1);
    let mut events = Vec::new();
    world.settle_items(&[0, 0], &tethered, &mut events);
    let taken = carrier - world.aboard.room.health(0);
    let want = lost * share / (1.0 - share);
    assert!((taken - want).abs() < 1e-3, "took {taken}, want {want}");
}

/// **A Decoy Projector and a Smoke Launcher** in a held station: the
/// ghost walks at half its owner's pace and goes on the enemy's list
/// after the crew and the sentries; a cloud on the crew member takes it
/// off that list and stands in the enemy's way like a shut door.
#[test]
fn a_decoy_walks_and_is_a_target_and_smoke_hides_and_blinds() {
    let (mut world, _station) = arena();
    carry(
        &mut world,
        0,
        0,
        Some(ModuleKind::DecoyProjector.at(Tier::One)),
    );
    carry(
        &mut world,
        0,
        1,
        Some(ModuleKind::SmokeLauncher.at(Tier::One)),
    );
    let listed = |world: &World| {
        world
            .residents
            .as_ref()
            .map_or(0, |r| r.aboard.room.hostiles_for_probe().len())
    };
    world.step(&[]);
    let before = listed(&world);
    let from = world.aboard.room.bim_pos(0);
    let tile = bims::room::TILE;
    let aim = (0..16)
        .map(|n| {
            let a = n as f32 / 16.0 * std::f32::consts::TAU;
            from + bims::math::vec2(a.cos(), a.sin()) * tile * 5.0
        })
        .find(|&p| world.aboard.room.ghost_route(0, p).is_some())
        .expect("somewhere to walk a ghost to");
    let events = world.step(&[Command::UseItem {
        slot: 0,
        item: 0,
        x: aim.x as i32,
        y: aim.y as i32,
    }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::ItemUsed { who: 0, .. })),
        "{events:?}"
    );
    assert_eq!(world.run.items.ghosts.len(), 1);
    let ghost = world.run.items.ghosts[0].clone();
    let pace = bims::balance::MARCH_SPEED * world.skill_of(0).walk * bims::module::DECOY_PACE;
    assert!((ghost.pace - pace).abs() < 1e-3);
    world.step(&[]);
    assert_eq!(listed(&world), before + 1, "the ghost on the enemy's list");
    let start = (world.run.items.ghosts[0].x, world.run.items.ghosts[0].y);
    for _ in 0..30 {
        world.step(&[]);
    }
    let walked = &world.run.items.ghosts[0];
    let gone = ((walked.x - start.0).powi(2) + (walked.y - start.1).powi(2)).sqrt();
    let half_a_second = pace * 0.5;
    assert!(gone > 0.0 && gone <= half_a_second + 1.0, "walked {gone}");
    // The cloud on the crew member.
    let shut = world
        .residents
        .as_ref()
        .map_or(0, |r| r.aboard.room.doors_shut_for_probe());
    let me = world.aboard.room.bim_pos(0);
    world.step(&[Command::UseItem {
        slot: 0,
        item: 1,
        x: me.x as i32,
        y: me.y as i32,
    }]);
    assert!(world.in_smoke(0));
    world.step(&[]);
    let shut_now = world
        .residents
        .as_ref()
        .map_or(0, |r| r.aboard.room.doors_shut_for_probe());
    assert!(shut_now > shut, "{shut} then {shut_now}");
    let hidden = world
        .residents
        .as_ref()
        .and_then(|r| r.aboard.room.hostiles_for_probe().first().copied())
        .flatten();
    assert!(hidden.is_none(), "nobody's target in the smoke");
}
