//! Task 113: nothing is stored. The ship's holdings — the pool, the
//! armory and the research keys — and each Bim's loadout: when a loadout
//! may change, who may change what, armour that is never destroyed, a
//! dead player back with everything it wore, a dead bot's kit kept, a key
//! counted the moment it is picked up, and all of it in the checksum.

use bims::combat::{ArmourKind, Item, Piece, Tier, WeaponKind};
use shipdesign::fixture::{flyer, playtest_ship};

use crate::data;
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, crewed_world, simulation_world};
use crate::holdings::{GearSlot, GearSource};
use crate::run::{Phase, Site};
use crate::world::{Command, World};
use crate::world_checksum;

fn refused(events: &[WorldEvent], why: Refusal) -> bool {
    events
        .iter()
        .any(|e| matches!(e, WorldEvent::Refused { why: w, .. } if *w == why))
}

fn any_refusal(events: &[WorldEvent]) -> bool {
    events
        .iter()
        .any(|e| matches!(e, WorldEvent::Refused { .. }))
}

/// A site in this system the ship is not at.
fn another_site_here(world: &World) -> Site {
    let here = world.current_site();
    world
        .sites_at(world.star_id)
        .into_iter()
        .find(|&s| Some(s) != here && world.travel_quote(s).is_ok())
        .expect("the spawn system has somewhere else to go")
}

/// Every player's yes to `site`: the trip, and the mission there begun.
fn travel_to(world: &mut World, site: Site) -> Vec<WorldEvent> {
    let mut events = world.step(&[Command::Propose {
        slot: 0,
        star: site.star,
        station: site.station,
    }]);
    for slot in 1..world.players() {
        events.extend(world.step(&[Command::Accept { slot, yes: true }]));
    }
    assert_eq!(world.run.phase, Phase::Mission, "{events:?}");
    events
}

/// A thing put straight into the armory for a test; its id there.
fn stock(world: &mut World, item: Item) -> u32 {
    world.holdings.put(item).expect("a thing, not a charge")
}

fn helm() -> Item {
    Item::Armour(Piece::new(9_000, ArmourKind::Armour, Tier::One))
}

/// A crew member off the ship onto the station's deck, just inside its
/// door.
fn ashore(world: &mut World, who: usize) {
    let at = world.aboard.ashore.expect("docked, so there is a door");
    world
        .aboard
        .room
        .put_for_probe(who, bims::math::vec2(at.x as f32, at.y as f32));
}

/// **Loadout and armory commands are refused in a mission off the
/// ship** and accepted on the map and the reward screen.
#[test]
fn gear_changes_hands_between_missions_and_never_out_on_the_deck() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 1, 2);
    assert_eq!(world.run.phase, Phase::Mission);
    ashore(&mut world, 0);
    world.step(&[]);
    assert!(!world.inside_ship(0));
    let rifle = stock(&mut world, Item::Weapon(WeaponKind::AutoRifle.basic()));
    let equip = |id| Command::Equip {
        slot: 0,
        who: 0,
        from: GearSource::Armory { id },
    };
    let events = world.step(&[equip(rifle)]);
    assert!(refused(&events, Refusal::GearLocked), "{events:?}");
    let events = world.step(&[Command::Unequip {
        slot: 0,
        who: 0,
        part: GearSlot::Weapon,
    }]);
    assert!(refused(&events, Refusal::GearLocked), "{events:?}");
    assert_eq!(
        world.aboard.room.weapon(0),
        Some(WeaponKind::LaserPistol.basic()),
        "nothing moved"
    );
    assert!(world.holdings.get(rifle).is_some());

    // On the map: the rifle on, the pistol into the armory.
    world.leave_for_probe();
    assert_eq!(world.run.phase, Phase::Map);
    let events = world.step(&[equip(rifle)]);
    assert!(!any_refusal(&events), "{events:?}");
    assert_eq!(
        world.aboard.room.weapon(0),
        Some(WeaponKind::AutoRifle.basic())
    );
    assert!(world.holdings.get(rifle).is_none(), "out of the armory");
    let pistol = Item::Weapon(WeaponKind::LaserPistol.basic());
    assert!(
        world.holdings.armory.iter().any(|s| s.item == pistol),
        "what was in the hand went in"
    );

    // On the reward screen as well: off again, and a bot's slot too.
    world.run.phase = Phase::Reward;
    let events = world.step(&[Command::Unequip {
        slot: 0,
        who: 0,
        part: GearSlot::Weapon,
    }]);
    assert!(!any_refusal(&events), "{events:?}");
    assert_eq!(world.aboard.room.weapon(0), None);
    let helm = stock(&mut world, helm());
    let events = world.step(&[Command::Equip {
        slot: 0,
        who: 1,
        from: GearSource::Armory { id: helm },
    }]);
    assert!(
        !any_refusal(&events),
        "a bot's slot is anybody's: {events:?}"
    );
    assert!(world.aboard.room.worn(1).is_some());
}

/// **Arriving at a site, the crew kit out from the armory aboard**: in a
/// mission a Bim inside the ship may be changed — a thing out of the
/// armory onto it, its own off into the armory, a thing off another Bim
/// aboard — and one out on the deck may not, neither as the one changed
/// nor as the one a thing is taken off. Offers stay between missions.
#[test]
fn a_crew_arriving_at_a_site_kits_out_from_the_armory_aboard() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 2, 3);
    world.step(&[]);
    assert_eq!(world.run.phase, Phase::Mission);
    assert!(
        world.inside_ship(0) && world.inside_ship(2),
        "aboard on arrival"
    );
    let rifle = stock(&mut world, Item::Weapon(WeaponKind::AutoRifle.basic()));
    let events = world.step(&[Command::Equip {
        slot: 0,
        who: 0,
        from: GearSource::Armory { id: rifle },
    }]);
    assert!(!any_refusal(&events), "{events:?}");
    assert_eq!(
        world.aboard.room.weapon(0),
        Some(WeaponKind::AutoRifle.basic())
    );
    assert!(world.may_change_now(0, 0) && world.may_change_now(0, 2));
    // A helm onto the bot aboard, and then the bot ashore.
    let helm = stock(&mut world, helm());
    let events = world.step(&[Command::Equip {
        slot: 0,
        who: 2,
        from: GearSource::Armory { id: helm },
    }]);
    assert!(!any_refusal(&events), "{events:?}");
    ashore(&mut world, 2);
    world.step(&[]);
    assert!(!world.may_change_now(0, 2), "out on the deck");
    let events = world.step(&[Command::Unequip {
        slot: 0,
        who: 2,
        part: GearSlot::Armour,
    }]);
    assert!(refused(&events, Refusal::GearLocked), "{events:?}");
    // Nor is anything taken off the bot ashore onto a Bim aboard.
    let events = world.step(&[Command::Equip {
        slot: 0,
        who: 0,
        from: GearSource::Worn {
            who: 2,
            slot: GearSlot::Armour,
        },
    }]);
    assert!(refused(&events, Refusal::GearLocked), "{events:?}");
    assert!(world.aboard.room.worn(2).is_some());
    // And no offer in a mission, aboard or not.
    let events = world.step(&[Command::Offer {
        slot: 0,
        part: GearSlot::Weapon,
        to: 1,
    }]);
    assert!(refused(&events, Refusal::GearLocked), "{events:?}");
}

/// **Nobody goes into a fight empty-handed**: a bot whose weapon went
/// into the armory aboard keeps its empty hand at peace, and the crew
/// under arms, it has a pistol — the armory's own, and a fresh one when
/// the armory has none.
#[test]
fn a_crewmate_under_arms_with_an_empty_hand_is_given_a_pistol() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 1, 3);
    world.step(&[]);
    assert_eq!(world.run.phase, Phase::Mission);
    let pistol = Item::Weapon(WeaponKind::LaserPistol.basic());
    let pistols = |world: &World| {
        world
            .holdings
            .armory
            .iter()
            .filter(|s| s.item == pistol)
            .count()
    };
    let before = pistols(&world);
    for who in [1, 2] {
        let events = world.step(&[Command::Unequip {
            slot: 0,
            who,
            part: GearSlot::Weapon,
        }]);
        assert!(!any_refusal(&events), "{events:?}");
    }
    // One of the two stowed pistols gone again, so that the armory has
    // one for one bot and none for the other.
    let taken = world
        .holdings
        .armory
        .iter()
        .rev()
        .find(|s| s.item == pistol)
        .map(|s| s.id)
        .expect("stowed");
    world.holdings.take(taken);
    world.step(&[]);
    assert!(!world.aboard.room.is_mustered(), "at peace");
    assert_eq!(world.aboard.room.weapon(1), None, "an empty hand at peace");
    assert_eq!(pistols(&world), before + 1);

    // The player draws its weapon, and the crew are under arms.
    world.aboard.room.recruit_for_probe(0, true);
    world.step(&[]);
    world.step(&[]);
    assert!(world.aboard.room.is_mustered());
    for who in [1, 2] {
        assert_eq!(
            world.aboard.room.weapon(who),
            Some(WeaponKind::LaserPistol.basic()),
            "crewmate {who} armed"
        );
    }
    assert_eq!(pistols(&world), before, "the stowed one drawn, one made");
}

/// **The combat runs' armory holds everything**: every carried weapon and
/// every piece at every tier it is made at, and nothing below one.
#[test]
fn every_thing_there_is_goes_into_the_armory_for_the_combat_runs() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 1, 1);
    let before = world.holdings.armory.len();
    world.stock_every_thing_for_probe();
    let added = &world.holdings.armory[before..];
    // Five kinds at three tiers, the minigun at two and the lance at one;
    // the armour at three.
    assert_eq!(added.len(), 5 * 3 + 2 + 1 + 3);
    for kind in WeaponKind::ALL {
        for tier in Tier::ALL {
            let n = added
                .iter()
                .filter(|s| matches!(s.item, Item::Weapon(w) if w.kind == kind && w.tier == tier))
                .count();
            assert_eq!(n, kind.made_at(tier) as usize, "{kind:?} {tier:?}");
        }
    }
    for kind in ArmourKind::ALL {
        for tier in Tier::ALL {
            let n = added
                .iter()
                .filter(|s| matches!(s.item, Item::Armour(p) if p.kind == kind && p.tier == tier))
                .count();
            assert_eq!(n, 1, "{kind:?} {tier:?}");
        }
    }
}

/// **Nothing comes off another player's Bim without that player
/// accepting**: an equip from its slot, an unequip of it, an equip onto
/// it and an answer by a third party are all refused, and an offer moves
/// only on the recipient's yes — the recipient's old thing into the
/// armory, and the offer withdrawn when either side's slot changes.
#[test]
fn no_player_takes_another_player_s_gear_without_its_yes() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 3, 4);
    world.leave_for_probe();
    let rifle = WeaponKind::AutoRifle.basic();
    let id = stock(&mut world, Item::Weapon(rifle));
    world.step(&[Command::Equip {
        slot: 1,
        who: 1,
        from: GearSource::Armory { id },
    }]);
    assert_eq!(world.aboard.room.weapon(1), Some(rifle));

    // Every way player 0 could reach player 1's rifle, refused.
    let attempts = [
        Command::Equip {
            slot: 0,
            who: 0,
            from: GearSource::Worn {
                who: 1,
                slot: GearSlot::Weapon,
            },
        },
        Command::Equip {
            slot: 0,
            who: 3,
            from: GearSource::Worn {
                who: 1,
                slot: GearSlot::Weapon,
            },
        },
        Command::Unequip {
            slot: 0,
            who: 1,
            part: GearSlot::Weapon,
        },
        Command::AnswerOffer {
            slot: 0,
            from: 1,
            part: GearSlot::Weapon,
            yes: true,
        },
    ];
    for attempt in attempts {
        let events = world.step(&[attempt]);
        assert!(any_refusal(&events), "{attempt:?}: {events:?}");
        assert_eq!(world.aboard.room.weapon(1), Some(rifle), "{attempt:?}");
    }
    // Nor is anything forced onto it.
    let other = stock(&mut world, Item::Weapon(WeaponKind::Shotgun.basic()));
    let events = world.step(&[Command::Equip {
        slot: 0,
        who: 1,
        from: GearSource::Armory { id: other },
    }]);
    assert!(refused(&events, Refusal::NotYours), "{events:?}");
    assert_eq!(world.aboard.room.weapon(1), Some(rifle));

    // Player 1 offers it to player 2; a third party cannot take it, and
    // player 2 declining leaves it where it was.
    let offer = Command::Offer {
        slot: 1,
        part: GearSlot::Weapon,
        to: 2,
    };
    world.step(&[offer]);
    assert_eq!(world.holdings.offers.len(), 1);
    let events = world.step(&[Command::AnswerOffer {
        slot: 0,
        from: 1,
        part: GearSlot::Weapon,
        yes: true,
    }]);
    assert!(refused(&events, Refusal::NoOffer), "{events:?}");
    world.step(&[Command::AnswerOffer {
        slot: 2,
        from: 1,
        part: GearSlot::Weapon,
        yes: false,
    }]);
    assert!(world.holdings.offers.is_empty());
    assert_eq!(world.aboard.room.weapon(1), Some(rifle));

    // Offered again and the offerer's slot changes: withdrawn.
    world.step(&[offer]);
    world.step(&[Command::Unequip {
        slot: 1,
        who: 1,
        part: GearSlot::Weapon,
    }]);
    assert!(world.holdings.offers.is_empty(), "withdrawn with the slot");
    let back = world
        .holdings
        .armory
        .iter()
        .find(|s| s.item == Item::Weapon(rifle))
        .unwrap()
        .id;
    world.step(&[Command::Equip {
        slot: 1,
        who: 1,
        from: GearSource::Armory { id: back },
    }]);

    // Offered and accepted: on player 2, whose pistol goes to the armory.
    world.step(&[offer]);
    let pistols = |world: &World| {
        world
            .holdings
            .armory
            .iter()
            .filter(|s| s.item == Item::Weapon(WeaponKind::LaserPistol.basic()))
            .count()
    };
    let before = pistols(&world);
    let events = world.step(&[Command::AnswerOffer {
        slot: 2,
        from: 1,
        part: GearSlot::Weapon,
        yes: true,
    }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::OfferTaken { from: 1, to: 2, .. })),
        "{events:?}"
    );
    assert_eq!(world.aboard.room.weapon(2), Some(rifle));
    assert_eq!(world.aboard.room.weapon(1), None);
    assert_eq!(pistols(&world), before + 1, "the replaced one stored");

    // And an offer standing is withdrawn when a mission starts.
    world.step(&[Command::Offer {
        slot: 2,
        part: GearSlot::Weapon,
        to: 0,
    }]);
    assert_eq!(world.holdings.offers.len(), 1);
    let site = another_site_here(&world);
    travel_to(&mut world, site);
    assert!(world.holdings.offers.is_empty(), "gone with the mission");
}

/// **Armour is never destroyed**: a piece worn down to nothing stays
/// worn, does nothing for the rest of the mission, and is whole again the
/// moment the ship leaves the site — on the map, the reward screen and at
/// a trader, not only at the next mission's start — and so is one in the
/// armory.
#[test]
fn armour_at_nothing_stays_worn_and_is_whole_once_the_mission_is_left() {
    let mut world = simulation_world(playtest_ship(), data::SIMULATION_MONEY, 1);
    world.leave_for_probe();
    let id = stock(&mut world, helm());
    world.step(&[Command::Equip {
        slot: 0,
        who: 0,
        from: GearSource::Armory { id },
    }]);
    let site = another_site_here(&world);
    travel_to(&mut world, site);
    // Worn down to nothing: still worn, protecting nothing.
    let out = world.aboard.room.wound(0, 100.0);
    assert!(out.piece_broke);
    let events = world.step(&[]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::PieceBroke { who: 0, .. }))
    );
    let broken = world.aboard.room.worn(0).expect("still worn");
    assert!(broken.broken());
    assert_eq!(world.aboard.room.armour_health(0), 0.0);
    for _ in 0..60 {
        world.step(&[]);
    }
    assert!(
        world.aboard.room.worn(0).is_some_and(|p| p.broken()),
        "worn, and broken, for the rest of the mission"
    );
    // A dented piece in the armory as well.
    let mut dented = Piece::new(9_100, ArmourKind::Armour, Tier::Two);
    dented.health = 1.0;
    let in_armory = stock(&mut world, Item::Armour(dented));

    // The mission left: both whole on the map already.
    world.leave_for_probe();
    let helm = world.aboard.room.worn(0).expect("still worn");
    assert_eq!(helm.health, helm.stats().health, "whole on the map");
    assert!(world.holdings.get(in_armory).is_some_and(|s| match s.item {
        Item::Armour(p) => p.health == p.stats().health,
        _ => false,
    }));
    // And at the next mission.
    let site = another_site_here(&world);
    travel_to(&mut world, site);
    let helm = world.aboard.room.worn(0).expect("still worn");
    assert_eq!(helm.health, helm.stats().health, "whole again");
    let Some(Item::Armour(kevlar)) = world.holdings.get(in_armory).map(|s| s.item) else {
        panic!("the kevlar in the armory");
    };
    assert_eq!(
        kevlar.health,
        kevlar.stats().health,
        "whole in the armory too"
    );
}

/// **A dead player's Bim respawns at the mission's end with its whole
/// loadout**, the pool charged the buyback — and a pool that holds less
/// goes to nought and no further, the respawn not waiting for it.
#[test]
fn a_dead_player_is_back_at_the_mission_s_end_with_everything_it_wore() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 2, 2);
    world.leave_for_probe();
    let rifle = WeaponKind::SniperRifle.at(Tier::Two);
    let id = stock(&mut world, Item::Weapon(rifle));
    world.step(&[Command::Equip {
        slot: 1,
        who: 1,
        from: GearSource::Armory { id },
    }]);
    let helm = stock(&mut world, helm());
    world.step(&[Command::Equip {
        slot: 1,
        who: 1,
        from: GearSource::Armory { id: helm },
    }]);
    world.award(1, 400, &mut Vec::new());
    let level = world.level_of(1);
    let site = another_site_here(&world);
    travel_to(&mut world, site);

    // Dead, and out for the rest of the mission; its gear on the body.
    world.aboard.room.kill_for_probe(1);
    world.step(&[]);
    assert!(world.run.is_out(1));
    assert!(!world.lost, "one player still standing");
    for _ in 0..30 {
        world.step(&[]);
    }
    assert!(!world.aboard.room.is_alive(1), "out while the mission runs");

    // The mission ends: back, with the sniper and the helm, its own
    // wallet charged the buyback, since it holds it.
    world.money = 0;
    world.wallets = vec![0, data::BUYBACK_COST + 7];
    let events = world.leave_for_probe();
    assert!(
        events.iter().any(|e| matches!(
            e,
            WorldEvent::Respawned {
                who: 1,
                paid: data::BUYBACK_COST
            }
        )),
        "{events:?}"
    );
    assert!(world.aboard.room.is_alive(1));
    assert!(!world.run.is_out(1));
    assert_eq!(world.wallets, vec![0, 7], "its own money, nobody else's");
    assert_eq!(world.aboard.room.weapon(1), Some(rifle), "its gun kept");
    assert!(world.aboard.room.worn(1).is_some(), "its helm kept");
    assert_eq!(world.level_of(1), level, "the level kept");

    // Again, with less than the buyback between the two of them: all of
    // it, and nought, not below.
    let site = another_site_here(&world);
    travel_to(&mut world, site);
    world.aboard.room.kill_for_probe(1);
    world.step(&[]);
    world.money = 0;
    world.wallets = vec![600, 400];
    let events = world.leave_for_probe();
    assert!(
        events.iter().any(|e| matches!(
            e,
            WorldEvent::Respawned {
                who: 1,
                paid: 1_000
            }
        )),
        "{events:?}"
    );
    assert_eq!(world.wallets, vec![0, 0], "never below nought");
    assert!(world.aboard.room.is_alive(1), "back without the money");
    assert_eq!(world.aboard.room.weapon(1), Some(rifle));
}

/// **A bot that dies, or is left behind, is gone — and its loadout is in
/// the armory** after the mission, the penalty paid for it.
#[test]
fn a_dead_or_left_behind_bot_s_loadout_is_in_the_armory() {
    // Dead.
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 1, 2);
    world.leave_for_probe();
    let rifle = WeaponKind::AutoRifle.at(Tier::Three);
    let id = stock(&mut world, Item::Weapon(rifle));
    world.step(&[Command::Equip {
        slot: 0,
        who: 1,
        from: GearSource::Armory { id },
    }]);
    let helm = stock(&mut world, helm());
    world.step(&[Command::Equip {
        slot: 0,
        who: 1,
        from: GearSource::Armory { id: helm },
    }]);
    let site = another_site_here(&world);
    travel_to(&mut world, site);
    world.aboard.room.kill_for_probe(1);
    let events = world.step(&[]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::BotLost { who: 1, .. }))
    );
    let crew = world.aboard.crew_count();
    world.leave_for_probe();
    assert_eq!(world.aboard.crew_count(), crew - 1, "gone");
    let armory = &world.holdings.armory;
    assert!(
        armory.iter().any(|s| s.item == Item::Weapon(rifle)),
        "its rifle"
    );
    assert!(
        armory
            .iter()
            .any(|s| matches!(s.item, Item::Armour(p) if p.kind == ArmourKind::Armour)),
        "its helm"
    );

    // Left behind: the ship goes with it outside, and its kit comes home.
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 1, 2);
    world.leave_for_probe();
    let id = stock(&mut world, Item::Weapon(rifle));
    world.step(&[Command::Equip {
        slot: 0,
        who: 1,
        from: GearSource::Armory { id },
    }]);
    let site = another_site_here(&world);
    travel_to(&mut world, site);
    if let Some(at) = world.aboard.ashore {
        world
            .aboard
            .room
            .put_for_probe(1, bims::math::vec2(at.x as f32, at.y as f32));
    }
    assert!(!world.inside_ship(1), "outside the ship");
    let events = world.leave_for_probe();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::LeftBehind { who: 1 })),
        "{events:?}"
    );
    assert_eq!(world.aboard.crew_count(), 1, "gone");
    assert!(
        world
            .holdings
            .armory
            .iter()
            .any(|s| s.item == Item::Weapon(rifle)),
        "its rifle is home"
    );
}

/// **A key picked up is counted the moment it is**, and said.
#[test]
fn a_key_picked_up_is_counted_at_once() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 1, 1);
    assert_eq!(world.holdings.keys, 0);
    let mut events = Vec::new();
    world.pick_up_key(&mut events);
    assert_eq!(world.holdings.keys, 1, "counted before any step");
    assert!(events.contains(&WorldEvent::KeyFound { keys: 1 }));
    world.pick_up_key(&mut events);
    assert_eq!(world.holdings.keys, 2);
}

/// **The holdings are in the checksum**: a thing in the armory, a key
/// and an offer each move it.
#[test]
fn the_holdings_change_the_checksum() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 2, 2);
    let twin = crewed_world(flyer(2), REFERENCE_MONEY, 2, 2);
    assert_eq!(world_checksum(&world), world_checksum(&twin));
    stock(&mut world, Item::Weapon(WeaponKind::Shotgun.basic()));
    let stocked = world_checksum(&world);
    assert_ne!(stocked, world_checksum(&twin), "a thing in the armory");
    world.holdings.keys += 1;
    let keyed = world_checksum(&world);
    assert_ne!(keyed, stocked, "a key");
    world.leave_for_probe();
    let before = world_checksum(&world);
    world.holdings.offers.push(crate::holdings::Offer {
        from: 0,
        slot: GearSlot::Weapon,
        to: 1,
    });
    assert_ne!(world_checksum(&world), before, "an offer");
}

/// The design's gear is the armory's from the first step, and none of it
/// is a count in the hold any more.
#[test]
fn the_design_s_gear_is_in_the_armory_and_not_the_hold() {
    use physics::ResourceId;
    let design = playtest_ship();
    let helms = design.carrying(ResourceId::Armour);
    let pistols = design.carrying(ResourceId::Shotgun);
    assert!(helms > 0 && pistols > 0, "the playtest ship carries gear");
    let world = simulation_world(design, data::SIMULATION_MONEY, 1);
    for resource in [ResourceId::Armour, ResourceId::Shotgun] {
        assert_eq!(world.ship.design.carrying(resource), 0, "{resource:?}");
    }
    assert_eq!(world.held(ResourceId::Armour), helms);
    assert_eq!(world.held(ResourceId::Shotgun), pistols);
}

/// **A buyback the fallen cannot pay pools every player's money**: the
/// user's own example — three players, C fallen with 2 000, A with 1 000
/// and B with 10 000 — 13 000 pooled, the 5 000 paid, and of the 8 000
/// left A gets 1/11 and B 10/11 back, C nothing; what the division leaves
/// over goes into the takings. A fallen player holding the buyback pays
/// it alone and keeps the rest.
#[test]
fn a_buyback_the_fallen_cannot_pay_pools_every_player_s_money() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 3, 3);
    world.money = 0;
    world.wallets = vec![1_000, 10_000, 2_000];
    assert_eq!(world.buy_back(2), data::BUYBACK_COST);
    assert_eq!(world.wallets, vec![727, 7_272, 0]);
    assert_eq!(world.money, 1, "the rounding into the takings");
    assert_eq!(
        world.crew_money(),
        1_000 + 10_000 + 2_000 - data::BUYBACK_COST
    );

    world.money = 0;
    world.wallets = vec![1_000, 10_000, 6_000];
    assert_eq!(world.buy_back(2), data::BUYBACK_COST);
    assert_eq!(world.wallets, vec![1_000, 10_000, 1_000], "its own alone");
}
