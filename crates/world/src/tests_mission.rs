//! The run's loop (feature 103): world map → travel → mission → back to
//! ship → world map. Travel resolved in one go, the world clock moving
//! only with it, a mission's start and end, the bounty waiting on the
//! site being cleared, death, buyback and the end of a run.

use bims::combat::{Gear, WeaponKind};
use bims::droid::DroidPart;
use bims::health::Part;
use shipdesign::fixture::{COMBAT_CREW, combat_ship, flyer, playtest_ship};
use worldgen::GalaxyType;

use crate::data;
use crate::event::{Refusal, WorldEvent};
use crate::fixture::{REFERENCE_MONEY, crewed_world, simulation_world};
use crate::run::{Departure, Phase, Site};
use crate::world::{Command, ShipState, World};

fn basic(players: u32) -> World {
    simulation_world(flyer(2), REFERENCE_MONEY, players)
}

/// The ship off its site and the map up, whoever is where: the button's
/// end without walking everybody home.
fn to_the_map(world: &mut World) -> Vec<WorldEvent> {
    let events = world.leave_for_probe();
    assert_eq!(world.run.phase, Phase::Map, "the map is up");
    events
}

/// A site in this system the ship is not at, with its quote.
fn another_site_here(world: &World) -> Site {
    let here = world.current_site();
    world
        .sites_at(world.star_id)
        .into_iter()
        .find(|&s| Some(s) != here && world.travel_quote(s).is_ok())
        .expect("the spawn system has somewhere else to go")
}

/// A site in a system one lane away.
fn a_site_one_hop_off(world: &World) -> Site {
    world
        .destinations()
        .into_iter()
        .find(|s| s.star != world.star_id && world.travel_quote(*s).is_ok())
        .expect("some star next door has somewhere to go")
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

fn travelled(events: &[WorldEvent]) -> bool {
    events
        .iter()
        .any(|e| matches!(e, WorldEvent::Travelled { .. }))
}

/// The `droids` probe's world, made by hand as `tests_droid` makes it:
/// the combat ship's crew docked at the arena the machines hold, a gun in
/// every hand, a wave a step away.
fn held_arena() -> (World, u32) {
    let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (star, station) = crate::spawn(&galaxy).unwrap();
    let mut world = World::start_with_crew(
        combat_ship(),
        REFERENCE_MONEY,
        1,
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
    (world, station)
}

fn open_the_room(world: &mut World) -> u32 {
    for _ in 0..40 {
        world.step(&[]);
        if world.droids_standing() > 0 {
            return world.droids_standing();
        }
    }
    panic!("no machines ever stood up");
}

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

// --- travel -------------------------------------------------------------------

/// **A trip costs the charge and the leg**, and the same on two worlds:
/// `physics::travel_days` of the distance in the system at the ship's
/// own accelerations — the forward engines pushing and the harder way
/// braking — with the hyperdrive's twenty minutes on top for a jump, the
/// leg then from where the jump lands. The clock is put on by the whole
/// minutes, rounded up.
#[test]
fn a_trip_costs_the_charge_and_the_leg_and_the_same_everywhere() {
    let world = basic(1);
    let other = basic(1);
    let dynamics = &world.ship.dynamics;
    let (push, brake) = (
        dynamics.a_forward,
        dynamics.a_forward.max(dynamics.a_backward),
    );
    let at = |w: &World, site: Site| -> worldgen::math::DVec2 {
        match crate::surface::surface_body(site.station) {
            Some(body) => w.system.absolute_position(worldgen::Node::Body(body)),
            None => w
                .system
                .absolute_position(worldgen::Node::Station(site.station)),
        }
        .unwrap()
    };
    // In the system.
    let here = world.current_site().expect("docked at the spawn");
    let site = another_site_here(&world);
    let quote = world.travel_quote(site).unwrap();
    let leg =
        physics::travel_days(at(&world, here).distance(at(&world, site)), push, brake).unwrap();
    assert!(!quote.jump);
    assert_eq!(quote.days, leg, "the leg and nothing else");
    assert_eq!(quote.minutes, (leg * time::DAY).ceil() as u64);
    assert_eq!(other.travel_quote(site), Ok(quote), "and the same twice");
    // One hop away.
    let far = a_site_one_hop_off(&world);
    let quote = world.travel_quote(far).unwrap();
    let system = world.galaxy().system(far.star).unwrap();
    let from = crate::jump::landing_point(&system);
    let to = match crate::surface::surface_body(far.station) {
        Some(body) => system.absolute_position(worldgen::Node::Body(body)),
        None => system.absolute_position(worldgen::Node::Station(far.station)),
    }
    .unwrap();
    let leg = physics::travel_days(from.distance(to), push, brake).unwrap();
    let charge = time::days(data::JUMP_CHARGE_MINUTES);
    assert!(quote.jump);
    assert_eq!(quote.days, charge + leg, "the charge and the leg");
    assert_eq!(other.travel_quote(far), Ok(quote));
    // And the trip is exactly that on the clock.
    let mut world = world;
    to_the_map(&mut world);
    let before = world.clock_minutes;
    assert!(travelled(&travel_to(&mut world, far)));
    assert_eq!(world.clock_minutes, before + quote.minutes as f64);
    assert_eq!(world.star_id, far.star, "in the other system");
    assert_eq!(
        world.ship.state.alongside(),
        Some(far.station),
        "tied up there"
    );
    assert!(world.in_mission(), "and a mission begun");
    assert_eq!(world.mission_steps(), 0);
}

/// A trip is one hop at most, and only between missions; a place that is
/// not there is refused.
#[test]
fn a_trip_is_one_hop_at_most_and_chosen_between_missions() {
    let mut world = basic(1);
    let site = another_site_here(&world);
    // During a mission the map is read-only.
    let events = world.step(&[Command::Propose {
        slot: 0,
        star: site.star,
        station: site.station,
    }]);
    assert!(events.iter().any(|e| matches!(
        e,
        WorldEvent::Refused {
            why: Refusal::MidMission,
            ..
        }
    )));
    // Two hops is too far.
    let galaxy = world.galaxy();
    let near = galaxy.lanes(world.star_id).to_vec();
    let two = near
        .iter()
        .flat_map(|&s| galaxy.lanes(s).to_vec())
        .find(|s| *s != world.star_id && !near.contains(s))
        .expect("somewhere two hops off");
    let far = world
        .galaxy()
        .system(two)
        .and_then(|s| s.stations.first().map(|b| b.id))
        .map(|station| Site { star: two, station });
    if let Some(far) = far {
        assert_eq!(world.travel_quote(far), Err(Refusal::TooFar));
    }
    assert_eq!(
        world.travel_quote(Site {
            star: world.star_id,
            station: 0xdead_beef
        }),
        Err(Refusal::NoSuchPlace)
    );
}

/// **The world clock stands still in a mission and on the map.** Only
/// travel moves it: a mission's steps move the mission clock, and the
/// map's move nothing but the step count a command is stamped with.
#[test]
fn the_world_clock_stands_still_in_a_mission_and_on_the_map() {
    let mut world = basic(1);
    let clock = world.clock_minutes;
    let day = world.day();
    for _ in 0..600 {
        world.step(&[]);
    }
    assert_eq!(world.clock_minutes, clock, "a mission moves no day");
    assert_eq!(world.day(), day);
    assert_eq!(world.mission_steps(), 600, "and the mission clock ran");
    to_the_map(&mut world);
    let steps = world.steps;
    for _ in 0..600 {
        world.step(&[]);
    }
    assert_eq!(world.clock_minutes, clock, "nor does the map");
    assert_eq!(world.steps, steps + 600, "the steps are counted");
    assert_eq!(world.day(), day);
}

/// **Days skipped by travel spread the crisis exactly as the same days
/// stepped would have.** Two worlds on one seed, the machines' origin put
/// so this system turns during the trip: one travels, the other holds in
/// open space and has its clock put on a step's worth at a time, stepping
/// between — what a clock running with the step would have done. They
/// agree about every star, and both find this system's stations the
/// machines'.
#[test]
fn days_skipped_by_travel_spread_the_crisis_as_the_same_days_stepped() {
    let mut a = basic(1);
    let mut b = basic(1);
    // The origin one hop off, so this system turns on day
    // `DROID_SPREAD_DAYS` — and the clock a few minutes short of it.
    for world in [&mut a, &mut b] {
        let hops = world.start_star_hops_for_probe();
        let origin = (0..hops.len() as u32)
            .find(|&s| hops[s as usize] == 1)
            .expect("a star one hop off");
        world.set_crisis_first_day_for_probe(0);
        world.set_droid_origin_for_probe(origin);
    }
    let turns = a.infested_on(a.star_id);
    assert_eq!(turns, data::DROID_SPREAD_DAYS);
    let site = another_site_here(&a);
    let minutes = a.travel_quote(site).unwrap().minutes;
    // Short of the day by a little under the trip.
    let start = f64::from(turns) * time::DAY - (minutes as f64 - 1.0).max(1.0);
    a.clock_minutes = start;
    b.clock_minutes = start;
    assert!(!a.infested(a.star_id));
    let minutes = a.travel_quote(site).unwrap().minutes;
    to_the_map(&mut a);
    assert!(travelled(&travel_to(&mut a, site)));
    // The other steps it, off its berth in the open, the clock put on
    // by hand a step at a time.
    b.undock_for_probe();
    let steps = (minutes as f64 / data::STEP_MINUTES).round() as u64;
    for _ in 0..steps {
        b.clock_minutes += data::STEP_MINUTES;
        b.step(&[]);
    }
    assert_eq!(a.days_gone(), b.days_gone(), "the same day");
    assert!(a.infested(a.star_id), "the system turned on the way");
    assert_eq!(a.infested_stars(), b.infested_stars(), "the same stars");
    for star in 0..a.galaxy().stars.len() as u32 {
        assert_eq!(a.front(star), b.front(star), "the same front at {star}");
    }
    let held = |w: &World| w.infested.iter().map(|it| it.station).collect::<Vec<u32>>();
    assert_eq!(held(&a), held(&b), "the same stations the machines'");
    assert!(!held(&a).is_empty());
}

// --- a mission -----------------------------------------------------------------

/// **A mission starts whole.** Every crew member's health is reset at
/// the start of a mission: every part full, the blood back, every wound
/// and trauma gone.
#[test]
fn health_is_made_whole_at_a_mission_s_start() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 1, 2);
    world.aboard.room.strike(0, Part::Body, 30.0, false);
    world.aboard.room.strike(1, Part::Legs, 40.0, false);
    world.aboard.room.set_blood_for_probe(1, 0.7);
    world.step(&[]);
    assert!(
        world.aboard.room.health(0) < bims::health::Part::ALL.iter().map(|p| p.max()).sum::<f32>()
    );
    to_the_map(&mut world);
    let site = another_site_here(&world);
    assert!(travelled(&travel_to(&mut world, site)));
    let full: f32 = Part::ALL.iter().map(|p| p.max()).sum();
    for who in 0..2 {
        assert_eq!(world.aboard.room.health(who), full, "{who} whole");
        assert_eq!(
            world.aboard.room.blood(who),
            bims::health::MAX_BLOOD,
            "{who}'s blood back"
        );
    }
}

/// **The bounty waits on the site being cleared, and is paid once.** A
/// machine destroyed at a held station is money pending; the last of the
/// last wave clears it and pays it into the pool; nothing is paid twice.
#[test]
fn the_bounty_is_paid_only_on_clear_and_only_once() {
    let (mut world, station) = held_arena();
    // Two waves: the first's bounty waits on the second.
    world.set_droid_waves_for_probe(2);
    world.set_droid_wave_for_probe(3);
    open_the_room(&mut world);
    let money = world.money;
    wreck_them_all(&mut world);
    world.step(&[]);
    let pending = world.run.pending_bounty;
    let tier = world.droid_tier().code();
    assert_eq!(
        pending,
        3 * crate::world::bounty_for(tier),
        "three owed for"
    );
    assert_eq!(world.money, money, "and not paid yet");
    assert!(!world.droid_station_cleared(station));
    // The second wave lands and is destroyed: the station is cleared, and
    // everything owed is paid, once.
    let mut paid = 0;
    let mut wrecked = false;
    for _ in 0..400 {
        if !wrecked && world.droids_standing() > 0 {
            wreck_them_all(&mut world);
            wrecked = true;
        }
        for e in world.step(&[]) {
            if let WorldEvent::Bounty { amount } = e {
                paid += amount;
            }
        }
        if wrecked && world.droid_station_cleared(station) && world.run.pending_bounty == 0 {
            break;
        }
    }
    assert!(
        world.droid_station_cleared(station),
        "the station is cleared"
    );
    assert_eq!(paid, 2 * pending, "both waves paid, once each");
    assert_eq!(world.money, money + paid);
    assert_eq!(world.run.pending_bounty, 0);
    for _ in 0..60 {
        world.step(&[]);
    }
    assert_eq!(world.money, money + paid, "and never again");
}

/// **A site left uncleared is put back as the mission met it.** The
/// machines' station with a wave destroyed and more to come: leaving it
/// throws the bounty away, keeps the experience, and the next visit finds
/// it exactly as the first did.
#[test]
fn a_site_left_uncleared_is_put_back_the_bounty_lost_and_the_experience_kept() {
    let (mut world, station) = held_arena();
    world.set_droid_waves_for_probe(3);
    world.set_droid_wave_for_probe(3);
    // The first step photographs the site; the next lays the wave.
    open_the_room(&mut world);
    let met = world.run.snapshot.clone().expect("the site photographed");
    assert_eq!(met.station, station);
    assert_eq!(
        met.infestation,
        world.infestation(station).cloned().map(|mut it| {
            // The count is settled the step after the photograph.
            it.settled = false;
            it.waves_left = 0;
            it.wave = 0;
            it
        })
    );
    let money = world.money;
    let xp_before = world.progress_of(0).xp;
    wreck_them_all(&mut world);
    world.step(&[]);
    assert!(world.run.pending_bounty > 0);
    let xp = world.progress_of(0).xp;
    let events = to_the_map(&mut world);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::LeftSite { cleared: false, .. }))
    );
    assert_eq!(world.money, money, "the bounty is lost");
    assert_eq!(world.run.pending_bounty, 0);
    assert_eq!(world.progress_of(0).xp, xp, "the experience is kept");
    assert!(xp >= xp_before);
    assert_eq!(
        world.infestation(station).cloned(),
        met.infestation,
        "the station as it was met"
    );
}

/// **A town left under attack falls.** Landed at a town the machines
/// are coming for, the attack under way: leaving it before the last wave
/// is destroyed gives it to the machines, an infested site like any
/// other.
#[test]
fn a_town_left_under_assault_becomes_infested() {
    let mut world = basic(1);
    let hops = world.start_star_hops_for_probe();
    let Some(origin) = (0..hops.len() as u32).find(|&s| hops[s as usize] == 1) else {
        return;
    };
    world.set_crisis_first_day_for_probe(0);
    world.set_droid_origin_for_probe(origin);
    world.set_day_for_probe(0);
    world.set_defense_delay_for_probe(data::STEP_MINUTES * 4.0);
    if !world.land_for_probe() {
        return;
    }
    let town = world.ship.state.alongside().unwrap();
    if !world.town_threatened(town) {
        return;
    }
    for _ in 0..10 {
        world.step(&[]);
    }
    assert!(
        world.defense(town).is_some_and(|d| !d.over()),
        "under attack"
    );
    assert!(!world.site_cleared(town));
    let events = to_the_map(&mut world);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::TownFell { station } if *station == town))
    );
    assert!(world.is_droid_held(town), "the machines have it");
    assert!(world.defense(town).is_some_and(|d| d.lost));
    assert!(!world.town_threatened(town));
}

// --- dying ----------------------------------------------------------------------

/// **Buyback** at a mission's start: a dead player's Bim bought back when
/// the pool can pay, the longest dead first, with no gear and its level
/// kept; the one the pool cannot pay for stays out and is tried again.
#[test]
fn buyback_goes_longest_dead_first_and_a_player_stays_out_when_the_pool_cannot_pay() {
    let mut world = crewed_world(flyer(2), data::BUYBACK_COST, 3, 3);
    world.award(1, 400, &mut Vec::new());
    let level = world.progress_of(1).level();
    // Slot 2 dies first, then slot 1.
    world.aboard.room.kill_for_probe(2);
    world.step(&[]);
    world.aboard.room.kill_for_probe(1);
    world.step(&[]);
    assert!(world.run.is_out(2) && world.run.is_out(1));
    assert_eq!(world.run.fallen[0].slot, 2, "the longest dead first");
    assert!(!world.lost, "one player still standing");
    to_the_map(&mut world);
    let site = another_site_here(&world);
    let events = travel_to(&mut world, site);
    assert!(travelled(&events));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::BoughtBack { who: 2 }))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::StillOut { who: 1 }))
    );
    assert!(world.aboard.room.is_alive(2), "bought back");
    assert!(!world.aboard.room.is_alive(1), "still out");
    assert!(world.run.is_out(1) && !world.run.is_out(2));
    assert_eq!(world.money, 0, "the pool paid what it held");
    let gear = world.aboard.room.gear(2);
    assert!(gear.weapon.is_none() && gear.head.is_none() && gear.body.is_none());
    assert!(
        gear.pack
            .iter()
            .flatten()
            .all(|i| matches!(i, bims::combat::Item::Stack(_))),
        "nothing in the pack but the charges every body carries"
    );
    // Slot 1 is tried again at the next mission, and comes back with its
    // level.
    world.money = data::BUYBACK_COST;
    to_the_map(&mut world);
    let site = another_site_here(&world);
    let events = travel_to(&mut world, site);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::BoughtBack { who: 1 }))
    );
    assert!(world.aboard.room.is_alive(1));
    assert_eq!(world.progress_of(1).level(), level, "the level kept");
}

/// **A bot's death costs the pool**, never below nought, and it is gone
/// for good when the ship leaves.
#[test]
fn a_bot_s_death_costs_the_penalty_and_never_takes_the_pool_below_nought() {
    let mut world = crewed_world(flyer(2), 3_000, 1, 2);
    world.aboard.room.kill_for_probe(1);
    let events = world.step(&[]);
    assert!(events.iter().any(|e| matches!(
        e,
        WorldEvent::BotLost {
            who: 1,
            paid: 3_000
        }
    )));
    assert_eq!(world.money, 0, "never below nought");
    assert!(!world.lost, "a bot does not end a run");
    let crew = world.aboard.crew_count();
    to_the_map(&mut world);
    assert_eq!(world.aboard.crew_count(), crew - 1, "gone for good");
    // And a second bot with money in the pool costs the whole penalty.
    let mut world = crewed_world(flyer(2), 12_000, 1, 2);
    world.aboard.room.kill_for_probe(1);
    world.step(&[]);
    assert_eq!(world.money, 12_000 - data::BOT_DEATH_PENALTY);
}

/// **The run is over when every player's Bim is dead at once** — not
/// down, not while a bot stands, and not while one player lives.
#[test]
fn the_run_is_lost_only_when_every_player_is_dead() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 2, 3);
    world.aboard.room.kill_for_probe(0);
    world.step(&[]);
    assert!(!world.lost, "one player of two dead");
    world.aboard.room.knock_out_for_probe(1);
    for _ in 0..5 {
        world.step(&[]);
    }
    assert!(world.aboard.room.is_unconscious(1));
    assert!(!world.lost, "the other out cold is not dead");
    world.aboard.room.kill_for_probe(1);
    let events = world.step(&[]);
    assert!(world.aboard.room.is_alive(2), "the bot stands");
    assert!(world.lost, "every player dead");
    assert!(events.iter().any(|e| matches!(e, WorldEvent::CrewLost)));
    // Said once, and kept: the app's end screen reads the flag.
    let events = world.step(&[]);
    assert!(
        !events.iter().any(|e| matches!(e, WorldEvent::CrewLost)),
        "said once"
    );
    assert!(world.lost, "and kept");
    assert_eq!(WorldEvent::CrewLost.code(), 63);
}

// --- ending a mission -----------------------------------------------------------

/// A crew member off the ship onto the station's deck, just inside its
/// door.
fn ashore(world: &mut World, who: usize) {
    let at = world.aboard.ashore.expect("docked, so there is a door");
    world
        .aboard
        .room
        .put_for_probe(who, bims::math::vec2(at.x as f32, at.y as f32));
}

/// **The departure check** waits for every player on their feet to have
/// pressed *Back to ship* and to be aboard — not for one who is down —
/// lists everybody alive outside the ship, and goes only on every
/// connected player's yes; a no keeps the ship, the presses standing.
#[test]
fn the_departure_check_waits_for_standing_players_lists_everyone_outside_and_wants_every_yes() {
    let mut world = crewed_world(playtest_ship(), REFERENCE_MONEY, 2, 3);
    world.step(&[]);
    assert!(
        world.inside_ship(0) && world.inside_ship(1),
        "aboard at the start"
    );
    // Player 1 goes ashore and a bot with them.
    ashore(&mut world, 1);
    ashore(&mut world, 2);
    world.step(&[]);
    assert!(!world.inside_ship(1) && !world.inside_ship(2));
    // Player 0 presses: player 1 is standing and has not, so nothing.
    let events = world.step(&[Command::Return { slot: 0 }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Returning { slot: 0 }))
    );
    assert!(world.run.recalled, "the first press sends the bots home");
    // Hold the bot where it is for the test: it would otherwise walk home.
    ashore(&mut world, 2);
    world.step(&[]);
    assert!(world.run.departure.is_none(), "player 1 is still wanted");
    assert!(world.in_mission());
    // Player 1 down outside the ship: not waited for, but listed.
    world.aboard.room.knock_out_for_probe(1);
    for _ in 0..3 {
        ashore(&mut world, 2);
        world.step(&[]);
    }
    assert!(world.aboard.room.is_down(1));
    let Some(Departure::Asking { behind, .. }) = world.run.departure.clone() else {
        panic!("the check should be asking, not {:?}", world.run.departure);
    };
    assert!(behind.contains(&1), "the downed player outside is listed");
    assert!(!behind.contains(&0), "the one aboard is not");
    // A no keeps the ship, the press standing.
    ashore(&mut world, 2);
    world.step(&[Command::LeaveBehind {
        slot: 1,
        yes: false,
    }]);
    assert!(matches!(
        world.run.departure,
        Some(Departure::Declined { .. })
    ));
    assert!(world.run.is_returning(0), "the press stands");
    for _ in 0..3 {
        ashore(&mut world, 2);
        world.step(&[]);
    }
    assert!(world.in_mission(), "declined, the ship stays");
    assert!(matches!(
        world.run.departure,
        Some(Departure::Declined { .. })
    ));
    // Pressed again, it asks again; every yes, and it goes.
    ashore(&mut world, 2);
    world.step(&[Command::Return { slot: 0 }]);
    ashore(&mut world, 2);
    world.step(&[]);
    assert!(matches!(
        world.run.departure,
        Some(Departure::Asking { .. })
    ));
    let behind = world.run.departure.as_ref().unwrap().behind().to_vec();
    ashore(&mut world, 2);
    world.step(&[Command::LeaveBehind { slot: 0, yes: true }]);
    assert!(world.in_mission(), "one yes of two");
    ashore(&mut world, 2);
    let events = world.step(&[Command::LeaveBehind { slot: 1, yes: true }]);
    assert!(!world.in_mission(), "every yes, and the ship went");
    for who in behind {
        assert!(
            events
                .iter()
                .any(|e| matches!(e, WorldEvent::LeftBehind { who: w } if *w == who)),
            "{who} left behind"
        );
    }
    assert!(
        world.run.is_out(1),
        "the player left behind is dead and out"
    );
    assert_eq!(world.ship.state, ShipState::Holding);
}

/// With nobody outside, the last press aboard is the ship leaving.
#[test]
fn with_everybody_aboard_the_last_press_is_the_ship_leaving() {
    let mut world = basic(2);
    world.step(&[Command::Return { slot: 0 }]);
    assert!(world.in_mission(), "one of two");
    world.step(&[Command::Return { slot: 1 }]);
    let events = world.step(&[]);
    assert!(
        !world.in_mission()
            || events
                .iter()
                .any(|e| matches!(e, WorldEvent::LeftSite { .. })),
        "the ship left"
    );
    assert!(!world.in_mission());
}

/// **A destination wants every connected player's yes, and any change
/// clears them.** The one who puts it has said yes; another destination
/// put starts again; a player gone is not waited for.
#[test]
fn a_proposal_wants_every_connected_player_and_any_change_clears_it() {
    let mut world = basic(3);
    to_the_map(&mut world);
    let a = another_site_here(&world);
    let b = a_site_one_hop_off(&world);
    world.step(&[Command::Propose {
        slot: 0,
        star: a.star,
        station: a.station,
    }]);
    world.step(&[Command::Accept { slot: 1, yes: true }]);
    assert!(!world.in_mission(), "two of three");
    assert_eq!(
        world.run.proposal.as_ref().unwrap().accepted,
        vec![true, true, false]
    );
    // Player 2 puts another: every yes but its own is gone.
    world.step(&[Command::Propose {
        slot: 2,
        star: b.star,
        station: b.station,
    }]);
    let p = world.run.proposal.clone().unwrap();
    assert_eq!(p.site, b);
    assert_eq!(p.accepted, vec![false, false, true]);
    world.step(&[Command::Accept { slot: 0, yes: true }]);
    world.step(&[Command::Accept {
        slot: 0,
        yes: false,
    }]);
    world.step(&[Command::Accept { slot: 0, yes: true }]);
    assert_eq!(
        world.run.proposal.as_ref().unwrap().accepted,
        vec![true, false, true],
        "a yes taken back and given again"
    );
    assert!(!world.in_mission(), "two of three");
    // Player 1 leaves the game: the vote no longer waits for it.
    let events = world.step(&[Command::PlayerGone { slot: 1 }]);
    assert!(travelled(&events), "carried without the one gone");
    assert_eq!(world.star_id, b.star);
}

/// The pinned reference scenario does what its note says: a mission at the
/// spawn, then back to the ship, a trip, and a mission somewhere else,
/// the world clock on by the trip and no further.
#[test]
fn the_reference_run_ends_in_a_mission_somewhere_else() {
    let start = crate::fixture::reference_world();
    let world = crate::fixture::reference_run_world();
    assert!(world.in_mission(), "a mission at the far end");
    assert_eq!(world.run.missions, 2, "the second of the run");
    assert_ne!(world.current_site(), start.current_site(), "somewhere else");
    assert!(world.ship.state.alongside().is_some(), "tied up there");
    assert_eq!(
        world.mission_steps(),
        u64::from(crate::fixture::REFERENCE_STEPS) - 1,
        "and the mission clock ran"
    );
}

// --- the measurements -------------------------------------------------------------

/// How long trips are for the default ship, over ten galaxies: every site
/// in the spawn system from the spawn, and every site of every system one
/// hop off. Printed, not asserted — `cargo test -p world -- --ignored
/// --nocapture travel_days_over_ten_galaxies`. The root `CLAUDE.md`
/// carries what it said.
#[test]
#[ignore]
fn travel_days_over_ten_galaxies() {
    let mut here: Vec<f64> = Vec::new();
    let mut hop: Vec<f64> = Vec::new();
    for seed in 1..=10u64 {
        let galaxy = worldgen::Galaxy::new(seed, GalaxyType::SpiralTwoArm);
        let Some((star, station)) = crate::spawn_anywhere(&galaxy, seed) else {
            continue;
        };
        let Ok(world) = World::start(
            playtest_ship(),
            data::START_MONEY_PER_BIM,
            1,
            seed,
            GalaxyType::SpiralTwoArm,
            star,
            station,
        ) else {
            continue;
        };
        for (site, quote) in world.travel_quotes() {
            let Ok(q) = quote else { continue };
            if Some(site) == world.current_site() {
                continue;
            }
            if q.jump {
                hop.push(q.days);
            } else {
                here.push(q.days);
            }
        }
    }
    let show = |name: &str, v: &mut Vec<f64>| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        if v.is_empty() {
            println!("{name}: none");
            return;
        }
        let at = |f: f64| v[((v.len() - 1) as f64 * f).round() as usize];
        println!(
            "{name}: n={} min={:.2} p25={:.2} median={:.2} p75={:.2} max={:.2} days",
            v.len(),
            v[0],
            at(0.25),
            at(0.5),
            at(0.75),
            v[v.len() - 1]
        );
    };
    show("in the system", &mut here);
    show("one hop", &mut hop);
}
