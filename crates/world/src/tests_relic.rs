//! Relics and unlocks (feature 106): research gone from the game, tier two
//! on time and distance, the relics' offers and caches, choosing one
//! together, what every relic does, and the win.

use bims::combat::{Gear, Tier, WeaponKind};
use bims::droid::DroidPart;
use shipdesign::fixture::{COMBAT_CREW, combat_ship, flyer};
use worldgen::GalaxyType;

use crate::class::{Charge, Class};
use crate::data;
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, crewed_world};
use crate::relic::{self, Relic, RelicChoice, Source};
use crate::run::Phase;
use crate::checksum::world_checksum;
use crate::world::{Command, World};

/// The `droids` arena, as `tests_mission` makes it: the combat ship's
/// crew docked at the spawn the machines hold, a gun in every hand, one
/// wave of three to clear and reinforcements a minute apart.
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

/// The arena's one wave wrecked and the site cleared, the events said on
/// the way.
fn clear(world: &mut World, station: u32) -> Vec<WorldEvent> {
    open_the_room(world);
    wreck_them_all(world);
    let mut events = Vec::new();
    for _ in 0..200 {
        events.extend(world.step(&[]));
        if world.droid_station_cleared(station) {
            break;
        }
    }
    assert!(world.droid_station_cleared(station), "the arena cleared");
    events
}

/// A choice of `options` put to the crew by hand, off `source`.
fn put(world: &mut World, source: Source, options: Vec<Relic>) {
    world.run.relics.choice = Some(RelicChoice {
        source,
        tier: 1,
        options,
        proposal: None,
    });
}

fn propose(slot: u32, relic: Option<Relic>, to: u32) -> Command {
    Command::ProposeRelic {
        slot,
        relic: relic.map_or(u32::MAX, Relic::code),
        to,
    }
}

fn refused(events: &[WorldEvent], why: Refusal) -> bool {
    events
        .iter()
        .any(|e| matches!(e, WorldEvent::Refused { why: w, .. } if *w == why))
}

// --- 1: research is gone -------------------------------------------------------

/// **No station carries a research key**: nothing of the crew's — the
/// hold, a pack — holds one at the start, nor after the crew have walked
/// every desk of the spawn system; there is no key to take and no tree
/// to spend one on. (The workbench's side is
/// `tests::a_workbench_upgrade_wants_no_research`.)
#[test]
fn no_station_carries_a_research_key() {
    use physics::ResourceId;
    let (world, _) = held_arena(1);
    for key in [ResourceId::ResearchKey, ResourceId::ResearchKeyTwo] {
        assert_eq!(world.ship.design.carrying(key), 0, "{key:?} in the hold");
        for who in 0..world.aboard.crew_count() as usize {
            let wanted = bims::combat::Item::Stack(key as u32);
            assert_eq!(world.aboard.room.gear(who).units_of(wanted), 0);
            for tier in [1, 2] {
                let keyed = bims::combat::Item::Key(tier);
                assert!(
                    !world.aboard.room.gear(who).pack.iter().flatten().any(|i| *i == keyed),
                    "a key in a pack"
                );
            }
        }
    }
}

// --- 2: tier two on time and distance ------------------------------------------

/// **Tier two waits on the world clock** and then follows the distance
/// ramp: nowhere before [`data::ENEMY_TIER2_HOURS`]; after it, never at
/// home, always at [`data::ENEMY_TIER2_SURE_HOPS`] or further, a mix in
/// between — and tier three near the origin whatever the clock says. The
/// map's quote reads the same answer as the wave.
#[test]
fn tier_two_comes_only_after_its_hours_and_follows_the_ramp() {
    let (mut world, _) = held_arena(1);
    world.set_droid_tier_for_probe(None);
    let hops = world.start_star_hops_for_probe();
    let origin_hops = |star: u32| world.hops_from_origin(star);
    let before = f64::from(data::ENEMY_TIER2_HOURS) * time::HOUR - 1.0;
    let after = f64::from(data::ENEMY_TIER2_HOURS) * time::HOUR;
    let far_from_origin =
        |star: u32| origin_hops(star) > data::DROID_TIER_THREE_HOPS && origin_hops(star) != u16::MAX;
    let mut seen_two = 0;
    let mut seen_one = 0;
    for (star, &h) in hops.iter().enumerate() {
        let star = star as u32;
        if h == u16::MAX || !far_from_origin(star) {
            continue;
        }
        for station in [0, 1, 2] {
            assert_eq!(
                world.site_tier(star, Some(station), before),
                Tier::One,
                "nothing at tier two before its hours"
            );
            let tier = world.site_tier(star, Some(station), after);
            if h == 0 {
                assert_eq!(tier, Tier::One, "never at home");
            } else if h >= data::ENEMY_TIER2_SURE_HOPS {
                assert_eq!(tier, Tier::Two, "always this far out");
            } else if tier == Tier::Two {
                seen_two += 1;
            } else {
                seen_one += 1;
            }
        }
    }
    assert!(seen_two > 0 && seen_one > 0, "a mix on the ramp: {seen_two} {seen_one}");
    // Tier three near the origin, before and after.
    let near = (0..hops.len() as u32)
        .find(|&s| origin_hops(s) <= data::DROID_TIER_THREE_HOPS)
        .expect("the origin is a star");
    assert_eq!(world.site_tier(near, Some(0), 0.0), Tier::Three);
    assert_eq!(world.site_tier(near, Some(0), after), Tier::Three);
    // The wave's tier is the site's: the arena at home, before and after.
    assert_eq!(world.droid_tier(), Tier::One);
    world.clock_minutes = after;
    assert_eq!(world.droid_tier(), Tier::One, "home is never tier two");
    // And the quote reads the rule at the arrival.
    world.leave_for_probe();
    world.run.phase = Phase::Map;
    for (site, quote) in world.travel_quotes() {
        let Ok(quote) = quote else { continue };
        let at = world.clock_minutes + quote.minutes as f64;
        assert_eq!(quote.tier, world.site_tier(site.star, Some(site.station), at));
    }
}

// --- 3: offers ------------------------------------------------------------------

/// **A clear offers three relics of the site's tier** on the reward
/// screen, after the departure and before the map: taken out of the pool,
/// so none of them is offered again this run, and travel waits on the
/// choice.
#[test]
fn a_clear_offers_three_of_the_site_s_tier_and_never_the_same_twice() {
    let (mut world, station) = held_arena(1);
    world.set_droid_tier_for_probe(Some(Tier::One));
    let pool = world.relic_pool().to_vec();
    assert_eq!(pool, relic::starting_pool());
    clear(&mut world, station);
    let events = world.leave_for_probe();
    assert_eq!(world.run.phase, Phase::Reward, "the reward screen");
    assert!(events.iter().any(|e| matches!(
        e,
        WorldEvent::RelicsOffered {
            source: 0,
            count: 3
        }
    )));
    let options = world.relic_choice().unwrap().options.clone();
    assert_eq!(options.len(), data::RELIC_OFFER);
    assert!(options.iter().all(|r| r.tier() == 1), "{options:?}");
    assert!(options.iter().all(|r| !world.relic_pool().contains(r)));
    assert_eq!(world.relic_pool().len(), pool.len() - 3);
    // No travel before the choice.
    let site = world.sites_at(world.star_id)[0];
    let events = world.step(&[Command::Propose {
        slot: 0,
        star: site.star,
        station: site.station,
    }]);
    assert!(refused(&events, Refusal::ChoosingRelic));
    // Taking none: the map, and the three gone for good.
    let events = world.step(&[propose(0, None, 0)]);
    assert!(events.contains(&WorldEvent::RelicsDeclined));
    assert_eq!(world.run.phase, Phase::Map);
    assert!(world.relic_choice().is_none());
    assert!(world.relics_of(0).is_empty());
    for r in &options {
        assert!(!world.relic_pool().contains(r));
    }
}

/// **A tier with nothing left falls to the next lower**: a tier-three site
/// with only tier-one relics in the pool offers tier one; a tier-one site
/// with only tier three left offers nothing, and the map comes straight
/// up.
#[test]
fn an_exhausted_tier_falls_back_to_the_lower_and_nothing_at_all_is_no_offer() {
    let (mut world, station) = held_arena(1);
    world.set_droid_tier_for_probe(Some(Tier::Three));
    world.set_relic_pool(vec![Relic::FocusingLens, Relic::ServoBraces]);
    clear(&mut world, station);
    world.leave_for_probe();
    let options = world.relic_choice().expect("an offer").options.clone();
    assert_eq!(options.len(), 2, "as many as there are");
    assert!(options.iter().all(|r| r.tier() == 1));

    let (mut world, station) = held_arena(1);
    world.set_droid_tier_for_probe(Some(Tier::One));
    world.set_relic_pool(vec![Relic::KillRelay]);
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

// --- 4: caches -------------------------------------------------------------------

/// The arena with a cache on its desk and crew member 0 beside it.
fn at_the_cache(world: &mut World, station: u32) {
    world
        .infested
        .iter_mut()
        .find(|it| it.station == station)
        .unwrap()
        .cache = true;
    world.step(&[]);
    let spot = world.cache_spot().expect("a desk on the arena's deck");
    world.aboard.room.put_for_probe(0, spot);
    world.step(&[]);
}

/// **A cache's relic is kept on clear**: opened, chosen and pending, then
/// the site cleared and the relic the Bim's for good.
#[test]
fn a_cache_s_relic_is_kept_when_the_site_is_cleared() {
    let (mut world, station) = held_arena(1);
    world.set_droid_tier_for_probe(Some(Tier::One));
    at_the_cache(&mut world, station);
    assert!(world.cache_here());
    let events = world.step(&[Command::OpenCache { slot: 0, who: 0 }]);
    assert!(events.contains(&WorldEvent::CacheOpened { who: 0 }), "{events:?}");
    assert!(!world.cache_here(), "the cache is gone off the desk");
    let choice = world.relic_choice().expect("one relic on offer").clone();
    assert_eq!(choice.source, Source::Cache);
    assert_eq!(choice.options.len(), 1);
    let relic = choice.options[0];
    let events = world.step(&[propose(0, Some(relic), 0)]);
    assert!(events.contains(&WorldEvent::RelicPending {
        slot: 0,
        relic: relic.code()
    }));
    assert!(world.relics_of(0).is_empty(), "pending, not held");
    let events = clear(&mut world, station);
    assert!(events.contains(&WorldEvent::RelicGiven {
        slot: 0,
        relic: relic.code()
    }));
    assert_eq!(world.relics_of(0), &[relic]);
    assert!(world.pending_relics().is_empty());
}

/// **A cache's relic is lost when the site is left uncleared**, and the
/// site put back — the cache on its desk again.
#[test]
fn a_cache_s_relic_is_lost_and_the_cache_put_back_when_the_site_is_left() {
    let (mut world, station) = held_arena(1);
    at_the_cache(&mut world, station);
    world.step(&[Command::OpenCache { slot: 0, who: 0 }]);
    let relic = world.relic_choice().unwrap().options[0];
    world.step(&[propose(0, Some(relic), 0)]);
    assert_eq!(world.pending_relics(), &[(0, relic)]);
    let events = world.leave_for_probe();
    assert!(events.contains(&WorldEvent::RelicLost {
        slot: 0,
        relic: relic.code()
    }));
    assert!(world.relics_of(0).is_empty());
    assert!(world.pending_relics().is_empty());
    assert_eq!(world.run.phase, Phase::Map, "no reward for a site not cleared");
    assert!(
        world.infestation(station).is_some_and(|it| it.cache),
        "the cache is back with the rest of the site"
    );
}

/// **A cache is rolled once a site**, off the galaxy's seed: the same on
/// every machine, and about as often as its odds say.
#[test]
fn a_cache_is_rolled_off_the_seed_at_its_odds() {
    let mut caches = 0;
    for station in 0..1_000u32 {
        let a = relic::cache_rolled(7, 3, station);
        assert_eq!(a, relic::cache_rolled(7, 3, station));
        caches += u32::from(a);
    }
    let odds = data::RELIC_CACHE_CHANCE * 10;
    assert!(caches.abs_diff(odds) < 60, "{caches} of a thousand");
}

// --- 5: choosing together ----------------------------------------------------------

/// **A relic choice wants every connected player's yes, and any change
/// clears them all** — the world map's vote over again. A player gone
/// is not waited for.
#[test]
fn a_relic_choice_wants_every_connected_yes_and_a_change_clears_them() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 3, 3);
    put(
        &mut world,
        Source::Reward,
        vec![Relic::FocusingLens, Relic::ServoBraces],
    );
    world.run.phase = Phase::Reward;
    world.apply_now(propose(0, Some(Relic::FocusingLens), 1));
    world.apply_now(Command::AcceptRelic { slot: 1, yes: true });
    assert!(world.relics_of(1).is_empty(), "one still to say yes");
    // Player 2 puts another: every yes cleared, player 2's own counted.
    world.apply_now(propose(2, Some(Relic::ServoBraces), 1));
    let proposal = world.relic_choice().unwrap().proposal.clone().unwrap();
    assert_eq!(proposal.accepted, vec![false, false, true]);
    world.apply_now(Command::AcceptRelic { slot: 0, yes: true });
    assert!(world.relics_of(1).is_empty());
    // Player 1 leaves: the last yes it owed is not waited for.
    world.apply_now(Command::PlayerGone { slot: 1 });
    world.apply_now(Command::AcceptRelic { slot: 0, yes: true });
    assert_eq!(world.relics_of(1), &[Relic::ServoBraces]);
    assert_eq!(world.run.phase, Phase::Map);
    // A relic not on offer, or with nothing on the table, is refused.
    let events = world.apply_now(propose(0, Some(Relic::KillRelay), 0));
    assert!(refused(&events, Refusal::NoRelicChoice));
    put(&mut world, Source::Reward, vec![Relic::FocusingLens]);
    let events = world.apply_now(propose(0, Some(Relic::KillRelay), 0));
    assert!(refused(&events, Refusal::NotOnOffer));
}

/// **A bot is never a recipient**: a proposal for a Bim no player steers
/// is refused, and a bot holds nothing whatever a probe tries.
#[test]
fn a_bot_can_never_be_given_a_relic() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 1, 3);
    put(&mut world, Source::Reward, vec![Relic::FocusingLens]);
    let events = world.apply_now(propose(0, Some(Relic::FocusingLens), 1));
    assert!(refused(&events, Refusal::NotAPlayer));
    let events = world.apply_now(propose(0, Some(Relic::FocusingLens), 2));
    assert!(refused(&events, Refusal::NotAPlayer));
    world.give_relic_for_probe(2, Relic::FocusingLens);
    assert!(world.relics_of(1).is_empty() && world.relics_of(2).is_empty());
    assert_eq!(
        world.skill_of(1).damage,
        bims::combat::Skill::NONE.damage,
        "and a bot's skill is its own"
    );
}

/// **A dead player keeps its relics** through its death and its buyback,
/// as it keeps its level.
#[test]
fn a_dead_player_keeps_its_relics_through_buyback() {
    let mut world = crewed_world(flyer(2), data::BUYBACK_COST, 2, 2);
    world.give_relic_for_probe(1, Relic::FieldPlating);
    world.aboard.room.kill_for_probe(1);
    world.step(&[]);
    assert!(world.run.is_out(1));
    assert_eq!(world.relics_of(1), &[Relic::FieldPlating], "dead, and kept");
    world.leave_for_probe();
    let here = world.current_site();
    let site = world
        .sites_at(world.star_id)
        .into_iter()
        .find(|&s| Some(s) != here && world.travel_quote(s).is_ok())
        .unwrap();
    let mut events = world.step(&[Command::Propose {
        slot: 0,
        star: site.star,
        station: site.station,
    }]);
    events.extend(world.step(&[Command::Accept { slot: 1, yes: true }]));
    assert!(events.contains(&WorldEvent::BoughtBack { who: 1 }), "{events:?}");
    assert_eq!(world.relics_of(1), &[Relic::FieldPlating], "back, and kept");
}

// --- 6: what every relic does ----------------------------------------------------------

/// Crew member 0's skill without a relic, and with `relic`.
fn skill_with(relic: Relic) -> (bims::combat::Skill, bims::combat::Skill) {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 2, 2);
    let without = world.skill_of(0);
    world.give_relic_for_probe(0, relic);
    (without, world.skill_of(0))
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

#[test]
fn the_stat_relics_move_the_stat_they_name() {
    let f = |p: i32| relic::factor(p) as f32;
    let (a, b) = skill_with(Relic::FocusingLens);
    assert!(close(b.damage, a.damage * f(data::FOCUSING_LENS_DAMAGE_PERCENT)));
    assert!(close(b.accuracy, a.accuracy) && close(b.walk, a.walk));
    let (a, b) = skill_with(Relic::ServoBraces);
    assert!(close(b.walk, a.walk * f(data::SERVO_BRACES_SPEED_PERCENT)));
    let (a, b) = skill_with(Relic::FieldPlating);
    assert!(close(
        b.armour_protection,
        a.armour_protection * f(data::FIELD_PLATING_ARMOUR_PERCENT)
    ));
    let (a, b) = skill_with(Relic::SteadyGrip);
    assert!(close(b.accuracy, a.accuracy * f(data::STEADY_GRIP_ACCURACY_PERCENT)));
    let (a, b) = skill_with(Relic::TraumaKit);
    assert!(close(b.healing, a.healing * f(data::TRAUMA_KIT_HEALING_PERCENT)));
    let (a, b) = skill_with(Relic::OverchargeCell);
    assert_eq!((a.overcharge, b.overcharge), (0, data::OVERCHARGE_CELL_EVERY));
    // Every fifth shot, and only it, is doubled.
    for shots in 0..10 {
        let damage = b.for_shot(shots).damage;
        if (shots + 1) % data::OVERCHARGE_CELL_EVERY == 0 {
            assert!(close(damage, b.damage * f(data::OVERCHARGE_CELL_DAMAGE_PERCENT)));
        } else {
            assert!(close(damage, b.damage));
        }
    }
}

/// **Coolant Loop** shortens the class's cooldowns — a grenade, a taunt, a
/// rally — and never the medicine.
#[test]
fn coolant_loop_shortens_the_class_s_cooldowns_and_not_the_medicine() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 3, 3);
    world.set_class(0, Class::Soldier).unwrap();
    world.set_class(1, Class::Tank).unwrap();
    world.set_class(2, Class::Commander).unwrap();
    let grenade = world.charge_cooldown(0, Charge::Grenade);
    let medkit = world.charge_cooldown(0, Charge::Medkit);
    let taunt = world.taunt_cooldown(1);
    let rally = world.rally_cooldown(2);
    for slot in 0..3 {
        world.give_relic_for_probe(slot, Relic::CoolantLoop);
    }
    let f = relic::factor(-data::COOLANT_LOOP_COOLDOWN_PERCENT);
    assert!((world.charge_cooldown(0, Charge::Grenade) - grenade * f).abs() < 1e-9);
    assert_eq!(world.charge_cooldown(0, Charge::Medkit), medkit);
    assert!((world.taunt_cooldown(1) - taunt * f).abs() < 1e-9);
    assert!((world.rally_cooldown(2) - rally * f).abs() < 1e-9);
}

/// **Last Stand** is more damage while another player's Bim is down, and
/// none otherwise.
#[test]
fn last_stand_waits_on_another_player_down() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 2, 2);
    let base = world.skill_of(0).damage;
    world.give_relic_for_probe(0, Relic::LastStand);
    assert!(close(world.skill_of(0).damage, base));
    world.aboard.room.knock_out_for_probe(1);
    world.step(&[]);
    assert!(world.aboard.room.is_down(1));
    let f = relic::factor(data::LAST_STAND_DAMAGE_PERCENT) as f32;
    assert!(close(world.skill_of(0).damage, base * f));
}

/// **Salvage Beacon** is more bounty for its holder's own kills, and
/// **Kill Relay** a second off every class cooldown running on it for
/// each.
#[test]
fn salvage_beacon_and_kill_relay_read_their_holder_s_kills() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 2, 2);
    world.set_class(0, Class::Soldier).unwrap();
    let bounty = crate::world::bounty_for(1);
    let mut events = Vec::new();
    assert_eq!(world.machine_kills(&[(Some(0), bounty)], &mut events), bounty);
    world.give_relic_for_probe(0, Relic::SalvageBeacon);
    let raised = bounty + bounty * data::SALVAGE_BEACON_BOUNTY_PERCENT as u64 / 100;
    assert_eq!(world.machine_kills(&[(Some(0), bounty)], &mut events), raised);
    assert_eq!(
        world.machine_kills(&[(Some(1), bounty), (None, bounty)], &mut events),
        2 * bounty,
        "nobody else's kills"
    );

    world.give_relic_for_probe(0, Relic::KillRelay);
    let now = world.mission_minutes();
    world.charge_timers[0][Charge::Grenade.code() as usize] = Some(now);
    let left = world.charge_cooldown_left(0, Charge::Grenade);
    world.machine_kills(&[(Some(0), bounty)], &mut events);
    let after = world.charge_cooldown_left(0, Charge::Grenade);
    assert!((left - after - data::KILL_RELAY_SECONDS).abs() < 1e-6, "{left} {after}");
    assert!(events.contains(&WorldEvent::RelicFired {
        who: 0,
        relic: Relic::KillRelay.code()
    }));
}

/// Steps a world `seconds` of the mission clock on.
fn run_seconds(world: &mut World, seconds: f64) -> Vec<WorldEvent> {
    let steps = (seconds * time::MINUTES_PER_SECOND / data::STEP_MINUTES).ceil() as u32;
    let mut events = Vec::new();
    for _ in 0..steps {
        events.extend(world.step(&[]));
    }
    events
}

/// **Second Wind** gets its Bim up five seconds after it goes down, at a
/// quarter of its health — once a mission, and again the next.
#[test]
fn second_wind_gets_up_once_a_mission() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 2, 2);
    world.give_relic_for_probe(0, Relic::SecondWind);
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    assert!(world.aboard.room.is_down(0));
    run_seconds(&mut world, data::SECOND_WIND_SECONDS * 0.5);
    assert!(world.aboard.room.is_down(0), "not before its seconds");
    let events = run_seconds(&mut world, data::SECOND_WIND_SECONDS);
    assert!(events.contains(&WorldEvent::RelicFired {
        who: 0,
        relic: Relic::SecondWind.code()
    }));
    assert!(!world.aboard.room.is_down(0), "up again");
    let share = world.health_share(0);
    assert!(share >= data::SECOND_WIND_HEALTH_PERCENT as f32 / 100.0 - 1e-3, "{share}");
    // Down again the same mission: it stays down.
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    run_seconds(&mut world, data::SECOND_WIND_SECONDS * 2.0);
    assert!(world.aboard.room.is_down(0), "once a mission");
    // The next mission, it is ready again.
    world.leave_for_probe();
    let here = world.current_site();
    let site = world
        .sites_at(world.star_id)
        .into_iter()
        .find(|&s| Some(s) != here && world.travel_quote(s).is_ok())
        .unwrap();
    world.step(&[Command::Propose {
        slot: 0,
        star: site.star,
        station: site.station,
    }]);
    world.step(&[Command::Accept { slot: 1, yes: true }]);
    assert!(world.in_mission());
    world.aboard.room.knock_out_for_probe(0);
    world.step(&[]);
    assert!(world.aboard.room.is_down(0));
    run_seconds(&mut world, data::SECOND_WIND_SECONDS * 1.5);
    assert!(!world.aboard.room.is_down(0), "ready again");
}

/// An enemy's hit on crew member 0 that takes `damage` off `part`, as the
/// relics' stage sees it: the count before, the wound and the count up,
/// and the stage run over it — the stage the step runs after the rooms.
fn hit(world: &mut World, part: bims::health::Part, damage: f32) -> Vec<WorldEvent> {
    let before = world.hits_before_the_step();
    world.aboard.room.wound(0, part, damage);
    let hits = world.aboard.room.hits_taken(0);
    world.aboard.room.set_hits_taken(0, hits + 1);
    let mut events = Vec::new();
    world.settle_relics(&before, &mut events);
    events
}

/// **Phase Harness** makes its Bim untouchable for two seconds the first
/// time a hit takes it under a quarter of its health — once a mission.
#[test]
fn phase_harness_fires_once_a_mission_below_a_quarter() {
    use bims::health::Part;
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 1, 1);
    world.give_relic_for_probe(0, Relic::PhaseHarness);
    let fired = |events: &[WorldEvent]| {
        events.contains(&WorldEvent::RelicFired {
            who: 0,
            relic: Relic::PhaseHarness.code(),
        })
    };
    // A hit that leaves it above a quarter: nothing.
    let events = hit(&mut world, Part::Legs, 5.0);
    assert!(!fired(&events));
    assert!(!world.aboard.room.is_surging(0));
    // A hit that takes it under: untouchable.
    world.aboard.room.wound(0, Part::Body, 60.0);
    let events = hit(&mut world, Part::Legs, 20.0);
    assert!(world.health_share(0) < 0.25, "{}", world.health_share(0));
    assert!(fired(&events), "{events:?}");
    assert!(world.aboard.room.is_surging(0));
    // Two seconds of it, and not twice in one mission.
    run_seconds(&mut world, f64::from(data::PHASE_HARNESS_SECONDS) + 0.5);
    assert!(!world.aboard.room.is_surging(0), "two seconds and no more");
    let events = hit(&mut world, Part::Head, 1.0);
    assert!(!fired(&events), "once a mission");
    assert!(!world.aboard.room.is_surging(0));
}

// --- 7: the win ------------------------------------------------------------------------

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

// --- 8: one pool, one world on every client --------------------------------------------

/// **The host's pool is the run's on every client**, and what the relics
/// do comes out the same on two worlds of one seed: the same offer, the
/// same checksum, fight after fight.
#[test]
fn the_host_s_pool_and_the_relics_effects_are_the_same_on_every_client() {
    let host_pool = vec![
        Relic::FocusingLens,
        Relic::SteadyGrip,
        Relic::OverchargeCell,
        Relic::KillRelay,
    ];
    let mut worlds: Vec<(World, u32)> = (0..2).map(|_| held_arena(1)).collect();
    for (world, _) in &mut worlds {
        world.set_droid_tier_for_probe(Some(Tier::Two));
        world.set_relic_pool(host_pool.clone());
        world.give_relic_for_probe(0, Relic::OverchargeCell);
        world.give_relic_for_probe(0, Relic::KillRelay);
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
    assert_eq!(offer_a, offer_b, "one offer");
    for r in offer_a.into_iter().flatten() {
        assert!(host_pool.contains(&r), "only the host's relics: {r:?}");
    }
    assert_eq!(world_checksum(a), world_checksum(b));
}
