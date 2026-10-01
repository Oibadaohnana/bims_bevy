//! The run's loop (feature 103): world map → travel → mission → back to
//! ship → world map. Travel resolved in one go, the world clock moving
//! only with it, a mission's start and end, the bounty waiting on the
//! site being cleared, death, the respawn and the end of a run.

use bims::combat::{Gear, WeaponKind};
use bims::droid::DroidPart;
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

/// **A jump costs a day and a trip in the system nothing** (the map rework),
/// and the same on two worlds: however far the site lies, a site of this
/// system is nought minutes off and a site a hyperlane away is exactly
/// [`data::JUMP_MINUTES`] — and the clock is put on by exactly that.
#[test]
fn a_jump_costs_a_day_and_a_trip_in_the_system_nothing() {
    let world = basic(1);
    let other = basic(1);
    // In the system.
    let site = another_site_here(&world);
    let quote = world.travel_quote(site).unwrap();
    assert!(!quote.jump);
    assert_eq!(quote.minutes, 0, "nothing within a system");
    assert_eq!(quote.days, 0.0);
    assert_eq!(other.travel_quote(site), Ok(quote), "and the same twice");
    // One hop away.
    let far = a_site_one_hop_off(&world);
    let quote = world.travel_quote(far).unwrap();
    assert!(quote.jump);
    assert_eq!(quote.minutes, data::JUMP_MINUTES, "a day for a jump");
    assert_eq!(quote.minutes, 24 * 60);
    assert_eq!(quote.days, 1.0);
    assert_eq!(other.travel_quote(far), Ok(quote));
    // And the trips are exactly that on the clock.
    let mut world = world;
    to_the_map(&mut world);
    let before = world.clock_minutes;
    assert!(travelled(&travel_to(&mut world, site)));
    assert_eq!(
        world.clock_minutes, before,
        "the clock stands in the system"
    );
    to_the_map(&mut world);
    let far = a_site_one_hop_off(&world);
    let quote = world.travel_quote(far).unwrap();
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

/// A trip is two hops at most (the second map rework), and only between missions; a place that is
/// not there is refused.
#[test]
fn a_trip_is_two_hops_at_most_and_chosen_between_missions() {
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
    // Three hops is too far (the second map rework: two is a trip).
    let hops = world.galaxy().hops_from(world.star_id);
    let three = (0..hops.len() as u32)
        .find(|&s| hops[s as usize] == 3)
        .expect("somewhere three hops off");
    let far = world
        .galaxy()
        .system(three)
        .and_then(|s| s.stations.first().map(|b| b.id))
        .map(|station| Site {
            star: three,
            station,
        });
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

/// **Days skipped by a jump spread the crisis exactly as the same days
/// stepped would have.** Two worlds on one seed, the machines' origin put
/// so the system jumped to turns during the jump's day (the map rework: only a
/// jump moves the clock): one jumps, the other holds in open space and
/// has its clock put on a step's worth at a time, stepping between — what
/// a clock running with the step would have done. They agree about every
/// star, and the system arrived in is the machines'.
#[test]
fn days_skipped_by_travel_spread_the_crisis_as_the_same_days_stepped() {
    let mut a = basic(1);
    let mut b = basic(1);
    let far = a_site_one_hop_off(&a);
    // The origin a hop past the star jumped to, not home, so that star
    // turns on day `DROID_SPREAD_DAYS`.
    let galaxy = a.galaxy();
    let origin = galaxy
        .lanes(far.star)
        .iter()
        .copied()
        .find(|&s| s != a.star_id && s != far.star)
        .expect("a star a hop past the one next door");
    for world in [&mut a, &mut b] {
        world.set_crisis_first_day_for_probe(0);
        world.set_droid_origin_for_probe(origin);
    }
    let turns = a.infested_on(far.star);
    assert_eq!(turns, data::DROID_SPREAD_DAYS);
    // Short of the day by a little under the jump.
    let minutes = data::JUMP_MINUTES;
    let start = f64::from(turns) * time::DAY - (minutes as f64 - 1.0);
    a.clock_minutes = start;
    b.clock_minutes = start;
    assert!(!a.infested(far.star));
    to_the_map(&mut a);
    let far = a_site_one_hop_off(&a);
    assert_eq!(a.travel_quote(far).map(|q| q.minutes), Ok(minutes));
    assert!(travelled(&travel_to(&mut a, far)));
    // The other steps it, off its berth in the open, the clock put on
    // by hand a step at a time.
    b.undock_for_probe();
    let steps = (minutes as f64 / data::STEP_MINUTES).round() as u64;
    for _ in 0..steps {
        b.clock_minutes += data::STEP_MINUTES;
        b.step(&[]);
    }
    assert_eq!(a.days_gone(), b.days_gone(), "the same day");
    assert_eq!(a.star_id, far.star, "in the system jumped to");
    assert!(a.infested(a.star_id), "the system turned on the way");
    assert_eq!(a.infested_stars(), b.infested_stars(), "the same stars");
    for star in 0..a.galaxy().stars.len() as u32 {
        assert_eq!(a.front(star), b.front(star), "the same front at {star}");
    }
    assert!(!a.infested.is_empty(), "its stations the machines'");
}

// --- a mission -----------------------------------------------------------------

/// **A mission starts whole.** Every crew member's bar is filled at the
/// start of a mission (task 120).
#[test]
fn health_is_made_whole_at_a_mission_s_start() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 1, 2);
    world.aboard.room.strike(0, 30.0, false);
    world.aboard.room.strike(1, 40.0, false);
    world.step(&[]);
    assert!(world.aboard.room.health(0) < bims::health::MAX_HEALTH);
    to_the_map(&mut world);
    let site = another_site_here(&world);
    assert!(travelled(&travel_to(&mut world, site)));
    for who in 0..2 {
        assert_eq!(
            world.aboard.room.health(who),
            bims::health::MAX_HEALTH,
            "{who} whole"
        );
    }
}

/// **The slow a downing leaves ends with the mission** (task 120): a crew
/// member downed and revived walks at `DOWNED_PACE` until the ship leaves
/// the site, and at its ordinary pace after — the bar as the mission left
/// it, until the next one fills it.
#[test]
fn the_slow_a_downing_leaves_is_cleared_at_the_mission_s_end() {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 1, 2);
    world.aboard.room.knock_out_for_probe(1);
    world.step(&[]);
    assert!(world.aboard.room.is_downed(1));
    world.aboard.room.bring_round(1, bims::health::REVIVED_TO);
    world.step(&[]);
    assert!(world.aboard.room.was_downed(1), "slowed for the mission");
    let hp = world.aboard.room.health(1);
    to_the_map(&mut world);
    assert!(!world.aboard.room.was_downed(1), "cleared at its end");
    assert_eq!(world.aboard.room.health(1), hp, "the bar as it was");
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
    // Each at its kind's share of the tier's bounty.
    let room = &world.residents.as_ref().unwrap().aboard.room;
    let owed: economy::Money = (0..room.droid_count() as usize)
        .filter_map(|i| room.droid(i))
        .map(|d| {
            crate::world::bounty_share(
                crate::world::bounty_for(tier),
                crate::world::droid_bounty_percent(d.kind),
            )
        })
        .sum();
    assert_eq!(pending, owed, "three owed for");
    assert_eq!(room.droid_count(), 3);
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
    if !world.site_threatened(town) {
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
    assert!(!world.site_threatened(town));
}

// --- dying ----------------------------------------------------------------------

/// **A bot's death costs the pool nothing** — only a player's Bim is paid
/// for — and it is gone for good when the ship leaves.
#[test]
fn a_bot_s_death_costs_nothing_and_it_is_gone_for_good() {
    let mut world = crewed_world(flyer(2), 12_000, 1, 2);
    world.aboard.room.kill_for_probe(1);
    let events = world.step(&[]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::BotLost { who: 1 })),
        "{events:?}"
    );
    assert_eq!(world.crew_money(), 12_000, "the pool untouched");
    assert!(!world.lost, "a bot does not end a run");
    let crew = world.aboard.crew_count();
    to_the_map(&mut world);
    assert_eq!(world.aboard.crew_count(), crew - 1, "gone for good");
    assert_eq!(
        world.crew_money(),
        12_000,
        "and nothing paid when the ship leaves"
    );
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
    assert!(world.aboard.room.is_downed(1));
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

/// *Back to ship* walks the player's own Bim home as it does the bots:
/// the one who pressed it out on the station's deck is aboard a while
/// later without another order, and the ship leaves with it.
#[test]
fn back_to_ship_walks_the_player_s_own_bim_home() {
    let mut world = crewed_world(playtest_ship(), REFERENCE_MONEY, 1, 1);
    world.step(&[]);
    ashore(&mut world, 0);
    world.step(&[]);
    assert!(!world.inside_ship(0), "ashore");
    world.step(&[Command::Return { slot: 0 }]);
    for _ in 0..3000 {
        if !world.in_mission() {
            break;
        }
        world.step(&[]);
    }
    assert!(
        !world.in_mission(),
        "the player walked home and the ship left"
    );
    assert!(world.aboard.room.is_alive(0), "not left behind");
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
    // The player left behind died there, and is back aboard at the
    // mission's end, as every dead player is (task 113).
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Respawned { who: 1, .. })),
        "the player left behind is back"
    );
    assert!(!world.run.is_out(1) && world.aboard.room.is_alive(1));
    assert_eq!(world.ship.state, ShipState::Holding);
}

/// **After a fight won the deck is frozen, and everybody alive comes
/// home** (task 133). The arena cleared with four of the crew outside the
/// ship: one whole, one hurt, and two downed. From the step after, the
/// room is not stepped — nobody moves, a downed body's countdown stands,
/// the mission clock stops — and an order to the crew is refused. The
/// player's *Back to ship* is the ship leaving at once, without a walk:
/// nobody is left behind, the downed come home too, and nobody died.
#[test]
fn after_a_fight_won_the_deck_is_frozen_and_everybody_alive_comes_home() {
    let (mut world, station) = held_arena();
    world.set_droid_waves_for_probe(1);
    world.set_droid_wave_for_probe(3);
    open_the_room(&mut world);
    for who in 1..=4 {
        ashore(&mut world, who);
    }
    // No armour, so the shot takes hit points rather than dents it.
    let gear = world.aboard.room.gear(2);
    world.aboard.room.issue(
        2,
        Gear {
            armour: None,
            ..gear
        },
    );
    world.aboard.room.wound(2, 40.0);
    world.aboard.room.knock_out_for_probe(3);
    world.aboard.room.wound(4, 1000.0);
    wreck_them_all(&mut world);
    for _ in 0..200 {
        world.step(&[]);
        if world.droid_station_cleared(station) {
            break;
        }
    }
    assert!(world.droid_station_cleared(station), "the arena cleared");
    assert!(world.run.fought, "and there was a fight");
    assert!(world.fight_over(), "so the fight is over");
    let room = &world.aboard.room;
    assert!(!room.is_downed(1) && !room.is_downed(2));
    assert!(room.health(2) < bims::health::MAX_HEALTH, "2 is hurt");
    assert!(room.is_downed(3) && room.is_downed(4), "3 and 4 are downed");
    for who in 1..=4 {
        assert!(!world.inside_ship(who), "{who} is outside");
    }
    assert!(world.left_behind().is_empty(), "{:?}", world.left_behind());

    // Frozen: a long while later nothing has moved and nobody bled out.
    let crew = world.aboard.crew_count();
    let before: Vec<_> = (0..crew).map(|who| world.aboard.position(who)).collect();
    let down_left = world.aboard.room.down_left(4);
    assert!(down_left.is_some(), "4 is counting down");
    let clock = world.mission_steps();
    for _ in 0..600 {
        world.step(&[]);
    }
    let after: Vec<_> = (0..crew).map(|who| world.aboard.position(who)).collect();
    assert_eq!(before, after, "nobody moved");
    assert_eq!(
        world.aboard.room.down_left(4),
        down_left,
        "the countdown stands"
    );
    assert!(world.aboard.room.is_downed(3) && world.aboard.room.is_downed(4));
    assert_eq!(world.mission_steps(), clock, "the mission clock stops");
    let at = world.aboard.gangway.unwrap();
    let events = world.step(&[Command::Crew {
        slot: 0,
        order: bims::order::CrewOrder::SendTo {
            who: 1,
            x: at.x as f32,
            y: at.y as f32,
        },
    }]);
    assert!(
        events.contains(&WorldEvent::Refused {
            slot: 0,
            why: Refusal::FightOver
        }),
        "{events:?}"
    );

    // Back to ship: the one press, and the ship goes with everybody.
    let mut events = world.step(&[Command::Return { slot: 0 }]);
    if world.in_mission() {
        events.extend(world.step(&[]));
    }
    assert!(!world.in_mission(), "the ship left without a walk");
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::LeftBehind { .. })),
        "nobody left behind: {events:?}"
    );
    for who in 1..=4 {
        assert!(world.aboard.room.is_alive(who as usize), "{who} alive");
        assert!(world.inside_ship(who), "{who} aboard");
    }
    assert_eq!(world.aboard.crew_count(), COMBAT_CREW, "no bot lost");
}

/// Without a fight won, nobody is taken home: a quiet site's departure
/// leaves a crew member on its feet outside behind, as it always did.
#[test]
fn without_a_fight_won_nobody_outside_is_taken_home() {
    let mut world = crewed_world(playtest_ship(), REFERENCE_MONEY, 1, 3);
    world.step(&[]);
    assert!(!world.run.fought, "a quiet site is no fight");
    ashore(&mut world, 2);
    assert!(!world.comes_home(2));
    assert_eq!(world.left_behind(), vec![2]);
    let events = world.leave_for_probe();
    assert!(events.contains(&WorldEvent::LeftBehind { who: 2 }));
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

/// **`would_travel` says beforehand what a command will do**: asked
/// before every vote of a three-player trip — a proposal, yeses short of
/// all, the last yes — and before a player going carries one, it answers
/// what the step then does, and changes nothing by being asked. Nothing
/// travels from inside a mission.
#[test]
fn would_travel_foretells_the_trip() {
    let mut world = basic(3);
    let a = another_site_here(&world);
    let propose = |slot| Command::Propose {
        slot,
        star: a.star,
        station: a.station,
    };
    assert!(!world.would_travel(&propose(0)), "mid-mission");
    to_the_map(&mut world);
    for command in [
        propose(0),
        Command::Accept { slot: 1, yes: true },
        Command::Accept {
            slot: 1,
            yes: false,
        },
        Command::Accept { slot: 1, yes: true },
        Command::Accept { slot: 2, yes: true },
    ] {
        let before = crate::world_checksum(&world);
        let said = world.would_travel(&command);
        assert_eq!(crate::world_checksum(&world), before, "only asked");
        let events = world.step(&[command]);
        assert_eq!(said, travelled(&events), "{command:?}");
    }
    assert!(world.in_mission(), "the last yes went");
    // And a player going, when that carries it.
    let mut world = basic(2);
    to_the_map(&mut world);
    world.step(&[propose(0)]);
    let gone = Command::PlayerGone { slot: 1 };
    assert!(world.would_travel(&gone));
    assert!(travelled(&world.step(&[gone])));
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

// --- no re-entry without time passing (feature 105) -------------------------------

/// The site the crew are at is never a destination: refused as a quote,
/// refused as a proposal, and the map lists it as where they are.
#[test]
fn the_site_the_crew_are_at_cannot_be_chosen() {
    let mut world = basic(1);
    to_the_map(&mut world);
    let here = world.current_site().expect("the crew are at a site");
    assert_eq!(world.travel_quote(here), Err(Refusal::AlreadyHere));
    let listed = world
        .travel_quotes()
        .into_iter()
        .find(|(site, _)| *site == here)
        .expect("the map lists where the crew are");
    assert_eq!(listed.1, Err(Refusal::AlreadyHere));
    let clock = world.clock_minutes;
    let events = travel_to(&mut world, here);
    assert!(!travelled(&events), "no trip");
    assert!(
        events.iter().any(|e| matches!(
            e,
            WorldEvent::Refused {
                why: Refusal::AlreadyHere,
                ..
            }
        )),
        "refused, and said so: {events:?}"
    );
    assert!(world.run.proposal.is_none(), "nothing on the table");
    assert_eq!(world.clock_minutes, clock, "and the clock where it was");

    // Somewhere else, and back: the way back is a trip like any other,
    // and neither moves the clock (the map rework).
    let there = another_site_here(&world);
    assert!(travelled(&travel_to(&mut world, there)));
    to_the_map(&mut world);
    assert_eq!(world.travel_quote(there), Err(Refusal::AlreadyHere));
    assert!(
        world.travel_quote(here).is_ok(),
        "where they were is a trip"
    );
    assert!(travelled(&travel_to(&mut world, here)));
    assert_eq!(world.clock_minutes, clock, "nothing within a system");
}

/// Every quote on the map is a day a lane crossed and nothing within the
/// system (the map rework, the second map rework), whatever the distance.
#[test]
fn every_quote_is_a_day_a_lane_and_nothing_in_the_system() {
    let mut world = basic(1);
    to_the_map(&mut world);
    let quotes: Vec<_> = world
        .travel_quotes()
        .into_iter()
        .filter_map(|(_, q)| q.ok())
        .collect();
    assert!(quotes.iter().any(|q| q.jump), "a jump on the list");
    assert!(quotes.iter().any(|q| !q.jump), "a trip in the system too");
    assert!(quotes.iter().any(|q| q.hops == 2), "a trip two lanes off");
    for q in &quotes {
        assert_eq!(q.jump, q.hops > 0, "{q:?}");
        let minutes = data::JUMP_MINUTES * u64::from(q.hops);
        assert_eq!(q.minutes, minutes, "{q:?}");
        assert_eq!(q.days * time::DAY, minutes as f64, "{q:?}");
    }
}

// --- the measurements -------------------------------------------------------------

/// How long trips are for the default ship, over ten galaxies: every site
/// in the spawn system from the spawn, and every site of every system one
/// hop off — and, with it, how far the crew start from the machines'
/// origin and what a run there comes to on the world clock and in the
/// waves (feature 105; task 147 for the waves).
/// Printed, not asserted — `cargo test --release -p world -- --ignored
/// --nocapture travel_days_over_ten_galaxies`. The root `CLAUDE.md`
/// carries what it said.
#[test]
#[ignore]
fn travel_days_over_ten_galaxies() {
    let mut here: Vec<f64> = Vec::new();
    let mut hop: Vec<f64> = Vec::new();
    let mut to_origin: Vec<f64> = Vec::new();
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
        to_origin.push(f64::from(world.hops_from_origin(star)));
        for (_, quote) in world.travel_quotes() {
            // The site the crew are at is refused, so every quote is a
            // trip somewhere else.
            let Ok(q) = quote else { continue };
            if q.jump {
                hop.push(q.days);
            } else {
                here.push(q.days);
            }
        }
    }
    let show = |name: &str, unit: &str, v: &mut Vec<f64>| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        if v.is_empty() {
            println!("{name}: none");
            return;
        }
        let at = |f: f64| v[((v.len() - 1) as f64 * f).round() as usize];
        println!(
            "{name}: n={} min={:.2} p25={:.2} median={:.2} p75={:.2} max={:.2} {unit}",
            v.len(),
            v[0],
            at(0.25),
            at(0.5),
            at(0.75),
            v[v.len() - 1]
        );
    };
    show("in the system", "days", &mut here);
    show("one hop", "days", &mut hop);
    show("to the origin", "hops", &mut to_origin);
    // A run to the origin at the medians — a jump a hop, and a jump and a
    // trip in the system a hop — and the waves on arrival there, by the
    // number of players.
    let median = |v: &Vec<f64>| v[(v.len() - 1) / 2];
    let (jump, trip, hops) = (median(&hop), median(&here), median(&to_origin));
    for (name, per_hop) in [
        ("a jump a hop", jump),
        ("a jump and a trip a hop", jump + trip),
    ] {
        let days = hops * per_hop;
        let scaling = crate::droid::WaveScaling::DEFAULT;
        let day = days as u32 + 1;
        let sizes: Vec<u32> = (1..=4)
            .map(|players| scaling.size(players, 0, day))
            .collect();
        println!(
            "to the origin, {name}: {days:.0} days: waves of {sizes:?} for 1..=4 players, \
             {} of them, most at {:?}",
            scaling.waves(day),
            scaling.usual_tier(day),
        );
    }
}
