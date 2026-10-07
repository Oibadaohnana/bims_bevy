//! Relics (feature 106, rebuilt October 2026): research gone from the
//! game, the tier a site is quoted at, the offer off an elite's clear,
//! choosing one — or none — together, what the crew's relics do to whom
//! and to the run, and the win.

use bims::combat::{Gear, Tier, WeaponKind};
use bims::droid::DroidPart;
use shipdesign::fixture::{COMBAT_CREW, combat_ship, flyer};
use worldgen::GalaxyType;

use crate::checksum::world_checksum;
use crate::class::Class;
use crate::data;
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, crewed_world};
use crate::relic::{self, Relic, RelicChoice, Stat};
use crate::run::Phase;
use crate::world::{Command, World};

/// The `droids` arena, as `tests_mission` makes it: the combat ship's
/// crew docked at the spawn the machines hold, a gun in every hand, one
/// wave of three to clear and reinforcements a minute apart — an elite,
/// since only an elite drops relics (`crate::elite`).
fn held_arena(players: u32) -> (World, u32) {
    let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (star, station) = crate::spawn(&galaxy).unwrap();
    let mut world = World::start_with_crew(
        combat_ship(),
        REFERENCE_MONEY,
        players,
        COMBAT_CREW,
        data::DEFAULT_SEED,
        GalaxyType::SpiralTwoArm,
        star,
        station,
    )
    .unwrap();
    // The machines' fight on day one (the scaling's area 0 is the
    // Manufacturers alone, October 2026).
    world.set_machines_only_for_probe();
    world.arena_dock_for_probe();
    let kinds = WeaponKind::ALL.iter().copied().cycle();
    let crew = world.aboard.room.crew_count() as usize;
    for (who, kind) in kinds.take(crew).enumerate() {
        let gear = world.aboard.room.gear(who);
        world.aboard.room.issue(
            who,
            Gear {
                weapon: Some(kind.basic()),
                ..gear
            },
        );
    }
    world.set_elite_for_probe(station);
    world.infest(station);
    world.set_droid_reinforce_minutes_for_probe(1.0);
    world.set_droid_waves_for_probe(1);
    world.set_droid_wave_for_probe(3);
    (world, station)
}

/// Steps until the wave is standing in the room.
fn open_the_room(world: &mut World) {
    for _ in 0..40 {
        world.step(&[]);
        if world.droids_standing() > 0 {
            return;
        }
    }
    panic!("no machines ever stood up");
}

/// Every machine standing wrecked where it stands.
fn wreck_them_all(world: &mut World) {
    let residents = world.residents.as_mut().unwrap();
    let n = residents.aboard.room.droid_count() as usize;
    for i in 0..n {
        residents
            .aboard
            .room
            .strike_droid(i, DroidPart::Chassis, 1e6);
    }
}

/// The arena's waves wrecked as they stand and the site cleared, the
/// events said on the way.
fn clear(world: &mut World, station: u32) -> Vec<WorldEvent> {
    open_the_room(world);
    let mut events = Vec::new();
    for _ in 0..20_000 {
        if world.droids_standing() > 0 {
            wreck_them_all(world);
        }
        events.extend(world.step(&[]));
        if world.droid_station_cleared(station) {
            break;
        }
    }
    assert!(world.droid_station_cleared(station), "the arena cleared");
    events
}

/// A choice of `options` put to the crew by hand, on the reward screen.
fn put(world: &mut World, options: Vec<Relic>) {
    world.run.relics.choice = Some(RelicChoice::new(options));
    world.run.phase = Phase::Reward;
}

fn propose(slot: u32, relic: Option<Relic>) -> Command {
    Command::ProposeRelic {
        slot,
        relic: relic.map_or(u32::MAX, Relic::code),
    }
}

fn refused(events: &[WorldEvent], why: Refusal) -> bool {
    events
        .iter()
        .any(|e| matches!(e, WorldEvent::Refused { why: w, .. } if *w == why))
}

// --- 2: the tier a site is quoted at ---------------------------------------------

/// **The quote reads the wave's tier at the arrival** (task 147): the run
/// day's, the same at every site but the Machine Heart's fortress.
#[test]
fn a_quote_says_the_tier_the_day_of_arrival_deals() {
    let (mut world, _) = held_arena(1);
    world.set_droid_tier_for_probe(None);
    world.leave_for_probe();
    world.run.phase = Phase::Map;
    for at_day in [1, 12, 30] {
        world.clock_minutes = f64::from(at_day - 1) * time::DAY;
        for (site, quote) in world.travel_quotes() {
            let Ok(quote) = quote else { continue };
            let at = world.clock_minutes + quote.minutes as f64;
            let want = if crate::heart::is_heart(site.station) {
                Tier::Three
            } else {
                world.tier_on(at)
            };
            assert_eq!(quote.tier, want, "day {at_day}");
        }
    }
}

// --- 3: the offer and the vote -------------------------------------------------------

/// **An elite's clear offers three** on the reward screen, after the
/// departure and before the map: three different ones, none the crew
/// hold, and travel waits on the choice.
#[test]
fn an_elite_s_clear_offers_three_and_travel_waits_on_the_vote() {
    let (mut world, station) = held_arena(1);
    world.give_relic_for_probe(Relic::GlassCannon);
    clear(&mut world, station);
    let events = world.leave_for_probe();
    assert_eq!(world.run.phase, Phase::Reward, "the reward screen");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::RelicsOffered { count: 3 }))
    );
    let options = world.relic_choice().unwrap().options.clone();
    assert_eq!(options.len(), data::RELIC_OFFER);
    let mut distinct = options.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), 3, "none twice: {options:?}");
    assert!(!options.contains(&Relic::GlassCannon), "never one held");
    // No travel before the choice.
    let site = world.sites_at(world.star_id)[0];
    let events = world.step(&[Command::Propose {
        slot: 0,
        star: site.star,
        station: site.station,
    }]);
    assert!(refused(&events, Refusal::ChoosingRelic));
    // One player alone: its proposal is the crew's.
    let events = world.step(&[propose(0, Some(options[1]))]);
    assert!(events.contains(&WorldEvent::RelicGiven {
        relic: options[1].code()
    }));
    assert_eq!(world.relics(), &[Relic::GlassCannon, options[1]]);
    assert_eq!(world.run.phase, Phase::Map);
}

/// **The vote is every connected player's yes, and any change clears
/// them all** — the world map's vote over again. A player gone is not
/// waited for, and the relic is the crew's, not the proposer's.
#[test]
fn a_relic_wants_every_connected_yes_and_a_change_clears_them() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 3, 3);
    put(&mut world, vec![Relic::HairTrigger, Relic::Adrenaline]);
    world.apply_now(propose(0, Some(Relic::HairTrigger)));
    world.apply_now(Command::AcceptRelic { slot: 1, yes: true });
    assert!(world.relic_choice().is_some(), "one still to say yes");
    // Player 2 puts another: every yes cleared, player 2's own counted.
    world.apply_now(propose(2, Some(Relic::Adrenaline)));
    let proposal = world.relic_choice().unwrap().proposal.clone().unwrap();
    assert_eq!(proposal.accepted, vec![false, false, true]);
    world.apply_now(Command::AcceptRelic { slot: 0, yes: true });
    assert!(world.relic_choice().is_some());
    // Player 1 leaves: the last yes it owed is not waited for.
    world.apply_now(Command::PlayerGone { slot: 1 });
    assert!(world.relic_choice().is_none());
    assert_eq!(world.relics(), &[Relic::Adrenaline]);
    assert_eq!(world.run.phase, Phase::Map);
    // A relic not on offer, or with nothing on the table, is refused.
    let events = world.apply_now(propose(0, Some(Relic::LoneWolves)));
    assert!(refused(&events, Refusal::NoRelicChoice));
    put(&mut world, vec![Relic::GlassCannon]);
    let events = world.apply_now(propose(0, Some(Relic::LoneWolves)));
    assert!(refused(&events, Refusal::NotOnOffer));
}

/// **Taking none is a choice** the crew vote on like any: every relic has
/// a price, so the crew may pass them all by.
#[test]
fn taking_none_is_voted_like_a_relic() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 2, 2);
    put(&mut world, vec![Relic::GlassCannon, Relic::BlackMarket]);
    world.apply_now(propose(1, None));
    assert!(world.relic_choice().is_some(), "player 0 still to say");
    let events = world.apply_now(Command::AcceptRelic { slot: 0, yes: true });
    assert!(events.contains(&WorldEvent::RelicsDeclined));
    assert!(world.relics().is_empty());
    assert_eq!(world.run.phase, Phase::Map);
}

/// **What the crew hold is never offered again**, and with nothing left
/// the map comes straight up.
#[test]
fn a_relic_held_is_never_offered_and_nothing_left_is_no_offer() {
    let (mut world, station) = held_arena(1);
    for r in Relic::ALL.into_iter().skip(1) {
        world.give_relic_for_probe(r);
    }
    for n in 0..20u32 {
        world.run.relics.offers = n;
        assert_eq!(world.draw_relics(3, station), vec![Relic::ALL[0]]);
    }
    world.give_relic_for_probe(Relic::ALL[0]);
    clear(&mut world, station);
    world.leave_for_probe();
    assert!(world.relic_choice().is_none());
    assert_eq!(world.run.phase, Phase::Map, "nothing to choose");
}

/// **A site with nothing to clear offers nothing**: the spawn, peaceful,
/// left for the map.
#[test]
fn a_peaceful_site_offers_no_relic() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 1, 1);
    world.step(&[]);
    world.leave_for_probe();
    assert_eq!(world.run.phase, Phase::Map);
    assert!(world.relic_choice().is_none());
}

/// **A piece broken in the fight is whole the moment the site is
/// cleared**: the crew sheet never shows it broken after the fight, nor
/// does a trader or the reward screen.
#[test]
fn armour_broken_in_the_fight_is_whole_once_the_site_is_cleared() {
    let (mut world, station) = held_arena(1);
    let mut worn = bims::combat::Piece::new(9_200, bims::combat::ArmourKind::Armour, Tier::One);
    worn.health = 0.0;
    let gear = world.aboard.room.gear(0);
    world.aboard.room.issue(
        0,
        Gear {
            armour: Some(worn),
            ..gear
        },
    );
    assert!(world.aboard.room.worn(0).is_some_and(|p| p.broken()));
    clear(&mut world, station);
    let piece = world.aboard.room.worn(0).expect("still worn");
    assert_eq!(piece.health, piece.stats().health, "whole after the fight");
}

// --- 4: what the crew's relics do ------------------------------------------------------

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

/// Player 0's and bot 1's skills without `relic`, and with it.
fn skills_with(relic: Relic) -> [(bims::combat::Skill, bims::combat::Skill); 2] {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 1, 2);
    let without = [world.skill_of(0), world.skill_of(1)];
    world.give_relic_for_probe(relic);
    [
        (without[0], world.skill_of(0)),
        (without[1], world.skill_of(1)),
    ]
}

/// **A relic reaches everybody it names** — the players, the bots, or
/// both — and moves its boon and its price where they are read.
#[test]
fn a_relic_reaches_whom_it_names() {
    let f = |p: i32| relic::factor(p) as f32;
    // Glass Cannon: everybody hits harder and takes more.
    for (a, b) in skills_with(Relic::GlassCannon) {
        assert!(close(b.damage, a.damage * f(data::GLASS_CANNON_DAMAGE)));
        assert!(close(b.melee, a.melee * f(data::GLASS_CANNON_DAMAGE)));
        assert!(close(
            b.damage_taken,
            a.damage_taken * f(data::GLASS_CANNON_TAKEN)
        ));
        assert!(close(b.walk, a.walk) && close(b.fire_rate, a.fire_rate));
    }
    // Heavy Plating: less taken, slower.
    for (a, b) in skills_with(Relic::HeavyPlating) {
        assert!(close(
            b.damage_taken,
            a.damage_taken * f(-data::HEAVY_PLATING_TAKEN)
        ));
        assert!(close(b.walk, a.walk * f(-data::HEAVY_PLATING_SPEED)));
    }
    // Hair Trigger's fire rate, for everybody.
    for (a, b) in skills_with(Relic::HairTrigger) {
        assert!(close(
            b.fire_rate,
            a.fire_rate * f(data::HAIR_TRIGGER_FIRE_RATE)
        ));
    }
    // Drill Sergeant: the bot's damage up and its share of a hit down,
    // the player's damage down.
    let [(pa, pb), (ba, bb)] = skills_with(Relic::DrillSergeant);
    assert!(close(
        pb.damage,
        pa.damage * f(-data::DRILL_SERGEANT_PLAYER_DAMAGE)
    ));
    assert!(close(pb.damage_taken, pa.damage_taken), "not the player's");
    assert!(close(
        bb.damage,
        ba.damage * f(data::DRILL_SERGEANT_BOT_DAMAGE)
    ));
    assert!(close(
        bb.damage_taken,
        ba.damage_taken * f(-data::DRILL_SERGEANT_BOT_TAKEN)
    ));
    // Lone Wolves the other way round.
    let [(pa, pb), (ba, bb)] = skills_with(Relic::LoneWolves);
    assert!(close(
        pb.damage,
        pa.damage * f(data::LONE_WOLVES_PLAYER_DAMAGE)
    ));
    assert!(close(pb.walk, pa.walk * f(data::LONE_WOLVES_PLAYER_SPEED)));
    assert!(close(
        bb.damage,
        ba.damage * f(-data::LONE_WOLVES_BOT_DAMAGE)
    ));
    assert!(close(bb.walk, ba.walk), "not the bot's pace");
}

/// **The class cooldowns** take the relics' share: longer with *Hair
/// Trigger*, shorter with *Overclocked Cores*.
#[test]
fn the_cooldown_relics_move_the_class_s_cooldowns() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 1, 1);
    world.set_class(0, Class::Tank).unwrap();
    world.set_ranks_for_probe(0, [1, 0, 0, 1]);
    let plain = world.reflect_cooldown(0);
    world.give_relic_for_probe(Relic::HairTrigger);
    let longer = world.reflect_cooldown(0);
    assert!((longer - plain * relic::factor(data::HAIR_TRIGGER_COOLDOWNS)).abs() < 1e-9);
    world.give_relic_for_probe(Relic::OverclockedCores);
    let both = world.reflect_cooldown(0);
    let want =
        plain * relic::factor(data::HAIR_TRIGGER_COOLDOWNS - data::OVERCLOCKED_CORES_COOLDOWNS);
    assert!((both - want).abs() < 1e-9, "{both} against {want}");
}

/// **The run's own numbers**: the bounty, the experience, the waves and
/// the trader's prices move by the crew's relics, summed.
#[test]
fn the_run_s_own_numbers_take_the_relics_share() {
    let (mut world, _) = held_arena(1);
    world.set_droid_wave_for_probe(3);
    let bounty = world.bounty_here(1_000);
    let xp = world.xp_per_down();
    world.give_relic_for_probe(Relic::BountyContract);
    assert_eq!(
        world.bounty_here(1_000),
        bounty * (100 + data::BOUNTY_CONTRACT_BOUNTY as u64) / 100
    );
    world.give_relic_for_probe(Relic::SalvageBurn);
    let percent = 100 + data::BOUNTY_CONTRACT_BOUNTY - data::SALVAGE_BURN_BOUNTY;
    assert_eq!(world.bounty_here(1_000), bounty * percent as u64 / 100);
    world.give_relic_for_probe(Relic::BlackMarket);
    assert_eq!(
        world.trader_price_by_relics(1_000),
        1_000 * (100 - data::BLACK_MARKET_PRICES as u64) / 100
    );
    world.give_relic_for_probe(Relic::HuntersPact);
    assert_eq!(
        world.xp_per_down(),
        xp * (100 + data::HUNTERS_PACT_EXPERIENCE as u32) / 100
    );
    // A wave forced by the dial is as many as it names; the formula's
    // takes the share, rounded up.
    assert_eq!(world.droid_wave_size(), 3);
    let mut plain = crewed_world(flyer(2), REFERENCE_MONEY, 1, 3);
    let n = plain.droid_wave_size();
    plain.give_relic_for_probe(Relic::HuntersPact);
    let want = (f64::from(n) * relic::factor(data::HUNTERS_PACT_WAVES)).ceil() as u32;
    assert_eq!(plain.droid_wave_size(), want);
    assert!(want > n, "more machines");
}

/// **Black Market's price**: every machine laid has
/// [`data::BLACK_MARKET_ENEMY_HEALTH`] per cent more hit points — every
/// part and the one health, whole — and the elite keeps its own waves,
/// cleared after its one as it is without the relic.
#[test]
fn black_market_lays_every_machine_with_more_health() {
    let laid = |relic: bool| -> (World, u32, Vec<(bims::droid::DroidKind, f32, f32)>) {
        let (mut world, station) = held_arena(2);
        world.set_droid_tier_for_probe(Some(Tier::Three));
        if relic {
            world.give_relic_for_probe(Relic::BlackMarket);
        }
        open_the_room(&mut world);
        let room = &world.residents.as_ref().unwrap().aboard.room;
        let bodies = (0..room.droid_count() as usize)
            .filter_map(|i| room.droid(i))
            .map(|d| (d.kind, d.body.life_max(), d.body.max(DroidPart::Chassis)))
            .collect();
        (world, station, bodies)
    };
    let (_, _, plain) = laid(false);
    let (mut world, station, tough) = laid(true);
    assert!(!plain.is_empty());
    assert_eq!(plain.len(), tough.len(), "as many machines");
    let factor = relic::factor(data::BLACK_MARKET_ENEMY_HEALTH) as f32;
    for (p, t) in plain.iter().zip(&tough) {
        assert_eq!(p.0, t.0, "the same kinds");
        assert!((t.1 - p.1 * factor).abs() < 0.01, "{p:?} -> {t:?}");
        assert!((t.2 - p.2 * factor).abs() < 0.01, "{p:?} -> {t:?}");
    }
    let room = &world.residents.as_ref().unwrap().aboard.room;
    for i in 0..room.droid_count() as usize {
        let body = room.droid(i).unwrap().body;
        assert_eq!(body.life(), body.life_max(), "laid whole");
    }
    let it = world.infestation(station).unwrap();
    assert_eq!((it.wave, it.waves_left), (1, 0), "no wave of its own");
    clear(&mut world, station);
}

/// **The damage to a machine**: a crew hit on one lands at the relics'
/// share — *Salvage Burn*'s more, *Bounty Contract*'s less.
#[test]
fn a_hit_on_a_machine_takes_the_relics_share() {
    let landed = |held: &[Relic]| -> f32 {
        let (mut world, _) = held_arena(1);
        for &r in held {
            world.give_relic_for_probe(r);
        }
        open_the_room(&mut world);
        let residents = world.residents.as_ref().unwrap();
        let bims = residents.aboard.room.crew_count() as usize;
        let before = residents.aboard.room.droid(0).unwrap().body.life();
        let hit = bims::combat::Hit {
            who: bims,
            damage: 10.0,
            cut: false,
            by: Some(0),
            blast: false,
            roll: 0.5,
            strips: 0.0,
            flat: 0.0,
            crit: false,
        };
        assert!(world.land_on_enemies(vec![hit]).is_empty());
        let residents = world.residents.as_ref().unwrap();
        before - residents.aboard.room.droid(0).unwrap().body.life()
    };
    let plain = landed(&[]);
    assert!(plain > 0.0);
    let more = landed(&[Relic::SalvageBurn]);
    assert!((more - plain * relic::factor(data::SALVAGE_BURN_DAMAGE) as f32).abs() < 1e-3);
    let less = landed(&[Relic::BountyContract]);
    assert!((less - plain * relic::factor(-data::BOUNTY_CONTRACT_DAMAGE) as f32).abs() < 1e-3);
}

/// **Nanite Mesh** puts hit points back into everybody on its feet, a bot
/// as well as a player — never into a body downed.
#[test]
fn nanite_mesh_mends_everybody_on_their_feet() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 1, 2);
    world.give_relic_for_probe(Relic::NaniteMesh);
    for who in 0..2 {
        world.aboard.room.set_health_for_probe(who, 20.0);
    }
    for _ in 0..120 {
        world.step(&[]);
    }
    for who in 0..2 {
        let gained = world.aboard.room.health(who) - 20.0;
        let want = data::NANITE_MESH_REGEN as f32 * 2.0;
        assert!(
            (gained - want).abs() < 0.5,
            "{who}: {gained} against {want}"
        );
    }
    assert!(
        relic::Relic::NaniteMesh
            .modifiers()
            .iter()
            .any(|m| m.stat == Stat::Regen)
    );
}

// --- 5: the win ------------------------------------------------------------------------

/// **`run_won` is said once**, and `BIMS_WIN`'s dial wins a run on the
/// next site cleared with machines in it.
#[test]
fn a_run_is_won_once_and_the_probe_wins_on_the_next_clear() {
    let (mut world, station) = held_arena(1);
    world.set_win_on_clear(true);
    let events = clear(&mut world, station);
    assert!(events.contains(&WorldEvent::RunWon), "{events:?}");
    assert!(world.is_won());
    let mut again = Vec::new();
    world.run_won(&mut again);
    assert!(again.is_empty(), "said once");
}

// --- 6: one world on every client ------------------------------------------------------

/// **What the relics do comes out the same on two worlds of one seed**:
/// the same fight, the same offer, the same checksum.
#[test]
fn the_relics_are_the_same_on_every_client() {
    let mut worlds: Vec<(World, u32)> = (0..2).map(|_| held_arena(1)).collect();
    for (world, _) in &mut worlds {
        world.set_droid_tier_for_probe(Some(Tier::Two));
        world.give_relic_for_probe(Relic::GlassCannon);
        world.give_relic_for_probe(Relic::DrillSergeant);
        world.give_relic_for_probe(Relic::NaniteMesh);
    }
    let [(a, station), (b, _)] = &mut worlds[..] else {
        unreachable!()
    };
    let station = *station;
    for _ in 0..600 {
        a.step(&[]);
        b.step(&[]);
    }
    assert_eq!(world_checksum(a), world_checksum(b), "the fight alike");
    for w in [&mut *a, &mut *b] {
        wreck_them_all(w);
        for _ in 0..200 {
            w.step(&[]);
            if w.droid_station_cleared(station) {
                break;
            }
        }
        w.leave_for_probe();
    }
    let offer_a = a.relic_choice().map(|c| c.options.clone());
    let offer_b = b.relic_choice().map(|c| c.options.clone());
    assert!(offer_a.is_some(), "an elite offers");
    assert_eq!(offer_a, offer_b, "one offer");
    assert_eq!(world_checksum(a), world_checksum(b));
}

/// **A revive is no relic's**: no relic is paid for in revives, and the
/// revive is the class's alone — ten seconds, a medic's four.
#[test]
fn no_relic_touches_a_revive() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 2, 2);
    for r in Relic::ALL {
        world.give_relic_for_probe(r);
    }
    assert_eq!(world.revive_seconds(0), 10.0);
    world.set_class(1, Class::Medic).unwrap();
    assert_eq!(world.revive_seconds(1), 4.0);
}

// --- October 2026: six more relics -----------------------------------------------

/// ***Point Blank*** and ***Marksman's Creed*** move every crew member's
/// near and far factors, players and bots alike, and nothing else of the
/// skill; held together they add.
#[test]
fn point_blank_and_marksman_s_creed_move_the_near_and_far_damage() {
    let f = |p: i32| relic::factor(p) as f32;
    for (relic, near, far) in [
        (
            Relic::PointBlank,
            f(data::POINT_BLANK_NEAR),
            f(-data::POINT_BLANK_FAR),
        ),
        (
            Relic::MarksmansCreed,
            f(-data::MARKSMANS_CREED_NEAR),
            f(data::MARKSMANS_CREED_FAR),
        ),
    ] {
        for (without, with) in skills_with(relic) {
            assert!(close(without.near_damage, 1.0) && close(without.far_damage, 1.0));
            assert!(close(with.near_damage, near), "{relic:?}");
            assert!(close(with.far_damage, far), "{relic:?}");
            assert!(close(with.damage, without.damage), "{relic:?}");
        }
    }
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 1, 2);
    world.give_relic_for_probe(Relic::PointBlank);
    world.give_relic_for_probe(Relic::MarksmansCreed);
    let both = world.skill_of(0);
    assert!(close(
        both.near_damage,
        f(data::POINT_BLANK_NEAR - data::MARKSMANS_CREED_NEAR)
    ));
    assert!(close(
        both.far_damage,
        f(data::MARKSMANS_CREED_FAR - data::POINT_BLANK_FAR)
    ));
}

/// ***Giant Slayer***: a hit on a big machine does more, on any other
/// less — a Warden against a Trooper, the same body struck.
#[test]
fn giant_slayer_hits_the_big_harder_and_the_rest_softer() {
    use bims::droid::DroidKind;
    assert!(relic::is_big(DroidKind::Warden) && relic::is_big(DroidKind::Guardian));
    assert!(relic::is_big(DroidKind::Lancer) && relic::is_big(DroidKind::Core));
    assert!(!relic::is_big(DroidKind::Husk) && !relic::is_big(DroidKind::Trooper));
    let landed = |held: &[Relic], kind: DroidKind| -> f32 {
        let (mut world, _) = held_arena(1);
        for &r in held {
            world.give_relic_for_probe(r);
        }
        open_the_room(&mut world);
        let residents = world.residents.as_mut().unwrap();
        let bims = residents.aboard.room.crew_count() as usize;
        residents.aboard.room.droid_mut_for_probe(0).unwrap().kind = kind;
        let before = residents.aboard.room.droid(0).unwrap().body.life();
        let hit = bims::combat::Hit {
            who: bims,
            damage: 10.0,
            cut: false,
            by: Some(0),
            blast: false,
            roll: 0.5,
            strips: 0.0,
            flat: 0.0,
            crit: false,
        };
        assert!(world.land_on_enemies(vec![hit]).is_empty());
        let residents = world.residents.as_ref().unwrap();
        before - residents.aboard.room.droid(0).unwrap().body.life()
    };
    for kind in [DroidKind::Warden, DroidKind::Trooper] {
        let plain = landed(&[], kind);
        assert!(plain > 0.0);
        let with = landed(&[Relic::GiantSlayer], kind);
        let percent = if relic::is_big(kind) {
            data::GIANT_SLAYER_BIG
        } else {
            -data::GIANT_SLAYER_SMALL
        };
        assert!(
            (with - plain * relic::factor(percent) as f32).abs() < 1e-3,
            "{kind:?}: {with} against {plain}"
        );
    }
}

/// ***War Chest***: a site cleared pays every player its per cent of the
/// money in its wallet, capped at a share of a site's pay — once — and
/// nothing without the relic.
#[test]
fn war_chest_pays_on_the_money_kept_at_a_clear() {
    for (held, wallet) in [(false, 2_000), (true, 2_000), (true, 1_000_000)] {
        let (mut world, station) = held_arena(1);
        if held {
            world.give_relic_for_probe(Relic::WarChest);
        }
        world.set_money_for_probe(wallet);
        let before = world.wallet(0);
        let events = clear(&mut world, station);
        let paid: Vec<_> = events
            .iter()
            .filter_map(|e| match *e {
                WorldEvent::WarChest { slot: 0, money } => Some(money),
                _ => None,
            })
            .collect();
        if !held {
            assert!(paid.is_empty());
            continue;
        }
        let cap = world.enemy_money_on(world.run_day()) * data::WAR_CHEST_CAP_ENEMIES;
        let interest = (before * data::WAR_CHEST_INTEREST as u64 / 100).min(cap);
        assert!(interest > 0);
        assert_eq!(paid, vec![interest], "once, at the clear");
        assert_eq!(world.wallet(0), before + interest);
        if wallet > 100_000 {
            assert_eq!(interest, cap, "capped");
        }
    }
}
