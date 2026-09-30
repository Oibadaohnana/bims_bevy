//! The Machine Heart (feature 108): the fortress at the origin, the fight's
//! three phases, the win, the fortress put back when the crew leave, the
//! map's preview on arrival and the fight the same on two worlds. The
//! machines themselves — the core's emitters, the sealed shell, the
//! pictures — are the room's (`crates/game/src/heart.rs`).

use bims::combat::{Gear, WeaponKind};
use bims::droid::{Beam, DroidKind, DroidPart};
use shipdesign::fixture::{combat_ship, flyer};
use worldgen::GalaxyType;

use crate::checksum::world_checksum;
use crate::data;
use crate::event::WorldEvent;
use crate::fixture::{REFERENCE_MONEY, simulation_world};
use crate::heart::{self, HeartPhase};
use crate::run::{Phase, Site};
use crate::world::{Command, World};

/// A crew of `crew`, `players` of them players, a gun in every hand,
/// docked at the fortress the probe lays at their own star — the `heart`
/// command's world, made by hand.
fn at_the_heart(players: u32, crew: u32) -> World {
    let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (star, station) = crate::spawn(&galaxy).unwrap();
    let mut world = World::start_with_crew(
        combat_ship(),
        REFERENCE_MONEY,
        players,
        crew,
        data::DEFAULT_SEED,
        GalaxyType::SpiralTwoArm,
        star,
        station,
    )
    .unwrap();
    let kinds = WeaponKind::ALL.iter().copied().cycle();
    for (who, kind) in kinds.take(crew as usize).enumerate() {
        let gear = world.aboard.room.gear(who);
        world.aboard.room.issue(
            who,
            Gear {
                weapon: Some(kind.basic()),
                ..gear
            },
        );
    }
    assert!(world.heart_dock_for_probe(), "the fortress is laid");
    world
}

/// Stepped until the core, the conduits and the fabricators stand on the
/// deck.
fn laid(world: &mut World) -> Vec<WorldEvent> {
    let mut events = Vec::new();
    for _ in 0..40 {
        events.extend(world.step(&[]));
        if world.heart_status().is_some() {
            return events;
        }
    }
    panic!("the Machine Heart was never laid");
}

/// The residents' room.
fn room(world: &mut World) -> &mut bims::game::Game {
    &mut world.residents.as_mut().unwrap().aboard.room
}

/// Every machine of the waves destroyed, the Heart's own left standing:
/// a fight with nothing in it but the core and its machines.
fn clear_the_wave(world: &mut World) {
    let room = room(world);
    for i in 0..room.droid_count() as usize {
        if room.droid(i).is_some_and(|d| !d.kind.is_structure()) {
            room.strike_droid(i, DroidPart::Chassis, 1e9);
        }
    }
}

/// The droid index of each of the Heart's machines of `kind`.
fn of_kind(world: &mut World, kind: DroidKind) -> Vec<usize> {
    let room = room(world);
    (0..room.droid_count() as usize)
        .filter(|&i| room.droid(i).is_some_and(|d| d.kind == kind))
        .collect()
}

/// The core's droid index.
fn core(world: &mut World) -> usize {
    of_kind(world, DroidKind::Core)[0]
}

/// Crew member `who` stood `tiles` from the core along `dir`, on the
/// joined deck.
fn beside_the_core(world: &mut World, who: usize, dir: bims::math::Vec2, tiles: f32) {
    let i = core(world);
    let at = room(world).droid(i).unwrap().pos + dir * (tiles * shipdesign::TILE as f32);
    let on_deck = world.residents_point_on_deck_for_probe(at).unwrap();
    world.aboard.room.put_for_probe(who, on_deck);
}

fn phase(world: &World) -> HeartPhase {
    world.heart_fight().unwrap().phase
}

// --- the fortress ---------------------------------------------------------------

/// **Every galaxy has exactly one fortress, at the origin**: the origin's
/// system lists it among its sites once, no other system does, and the
/// same seed gives the same fortress. Laid, it is tier three whatever the
/// probes' dial says, and it has the conduits its players call for.
#[test]
fn every_galaxy_has_one_fortress_at_the_origin_at_tier_three() {
    for (seed, kind) in [
        (data::DEFAULT_SEED, GalaxyType::SpiralTwoArm),
        (0x_5eed_0001, GalaxyType::Round),
        (0x_5eed_0002, GalaxyType::Spiral),
    ] {
        let galaxy = worldgen::Galaxy::new(seed, kind);
        let (star, station) = crate::spawn(&galaxy).unwrap();
        let open =
            || World::start(flyer(2), REFERENCE_MONEY, 1, seed, kind, star, station).unwrap();
        let world = open();
        let origin = world.droid_origin();
        let hearts = |star: u32| {
            world
                .sites_at(star)
                .into_iter()
                .filter(|s| heart::is_heart(s.station))
                .count()
        };
        assert_eq!(hearts(origin), 1, "seed {seed:x}: one at the origin");
        assert_eq!(hearts(world.star_id), 0, "none at the crew's own star");
        for &next in galaxy.lanes(origin) {
            assert_eq!(hearts(next), 0, "none next door to it");
        }
        // The same seed, the same fortress.
        let system = galaxy.system(origin).unwrap();
        let one = heart::blueprint(&system, seed, origin);
        let other = open();
        let two = heart::blueprint(
            &worldgen::Galaxy::new(seed, kind)
                .system(other.droid_origin())
                .unwrap(),
            seed,
            other.droid_origin(),
        );
        assert_eq!(one, two, "seed {seed:x}: the same fortress");
    }
    for players in [1u32, 2] {
        let mut world = at_the_heart(players, players + 1);
        world.set_droid_tier_for_probe(Some(bims::combat::Tier::One));
        laid(&mut world);
        assert_eq!(world.droid_tier(), bims::combat::Tier::Three);
        let status = world.heart_status().unwrap();
        assert_eq!(status.conduits, heart::conduits_for(players));
        assert_eq!(status.conduits_left, heart::conduits_for(players));
        assert_eq!(status.fabricators_left, data::HEART_FABRICATORS);
        assert_eq!(status.core_max, heart::core_health_for(players));
        for d in room(&mut world).droids() {
            assert_eq!(d.tier, bims::combat::Tier::Three, "{:?}", d.kind);
        }
        // And no two conduits stand in one room while rooms are left.
        let conduits: Vec<_> = of_kind(&mut world, DroidKind::Conduit);
        let spots: Vec<_> = conduits
            .iter()
            .map(|&i| room(&mut world).droid(i).unwrap().pos)
            .collect();
        for (i, a) in spots.iter().enumerate() {
            for b in &spots[i + 1..] {
                assert!(
                    (*a - *b).len() > 6.0 * shipdesign::TILE as f32,
                    "two conduits side by side: {a:?} {b:?}"
                );
            }
        }
    }
}

// --- the fight ------------------------------------------------------------------

/// **The core takes nothing while any conduit stands**, and destroying the
/// last conduit starts phase two.
#[test]
fn the_core_takes_nothing_while_a_conduit_stands() {
    let mut world = at_the_heart(1, 2);
    laid(&mut world);
    clear_the_wave(&mut world);
    let i = core(&mut world);
    let full = room(&mut world)
        .droid(i)
        .unwrap()
        .body
        .health(DroidPart::Chassis);
    room(&mut world).strike_droid(i, DroidPart::Chassis, 1e9);
    world.step(&[]);
    let d = room(&mut world).droid(i).unwrap();
    assert!(!d.destroyed, "sealed");
    assert_eq!(d.body.health(DroidPart::Chassis), full);
    assert_eq!(
        d.shield(),
        Some(bims::math::Vec2::ZERO),
        "a shell all round"
    );
    let conduits = of_kind(&mut world, DroidKind::Conduit);
    let (last, rest) = conduits.split_last().unwrap();
    for &c in rest {
        room(&mut world).strike_droid(c, DroidPart::Chassis, 1e9);
    }
    world.step(&[]);
    assert_eq!(
        phase(&world),
        HeartPhase::Sealed,
        "one conduit still stands"
    );
    room(&mut world).strike_droid(i, DroidPart::Chassis, 1e9);
    assert_eq!(
        room(&mut world)
            .droid(i)
            .unwrap()
            .body
            .health(DroidPart::Chassis),
        full
    );
    room(&mut world).strike_droid(*last, DroidPart::Chassis, 1e9);
    let events = world.step(&[]);
    assert_eq!(phase(&world), HeartPhase::Exposed);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::HeartExposed { .. }))
    );
    world.step(&[]);
    let d = room(&mut world).droid(i).unwrap();
    assert!(!d.heart.sealed && d.shield().is_none(), "exposed");
    room(&mut world).strike_droid(i, DroidPart::Chassis, 10.0);
    assert_eq!(
        room(&mut world)
            .droid(i)
            .unwrap()
            .body
            .health(DroidPart::Chassis),
        full - 10.0,
        "and it is hurt now"
    );
}

/// **Every conduit shot down brings a wave** in by the airlocks, on top of
/// whatever still stands — one a conduit, two for two downed in one step,
/// none for a conduit already answered — and the waves by the clock still
/// to come are left as they were.
#[test]
fn every_conduit_shot_down_brings_a_wave() {
    let mut world = at_the_heart(1, 2);
    laid(&mut world);
    let machines = |world: &mut World| {
        let room = room(world);
        (0..room.droid_count() as usize)
            .filter(|&i| room.droid(i).is_some_and(|d| !d.kind.is_structure()))
            .count()
    };
    let (wave, left) = world.droid_wave_standing().unwrap();
    let before = machines(&mut world);
    assert!(before > 0, "the first wave is aboard");
    let size = world.droid_wave_size() as usize;
    let conduits = of_kind(&mut world, DroidKind::Conduit);
    assert!(conduits.len() >= 3);
    room(&mut world).strike_droid(conduits[0], DroidPart::Chassis, 1e9);
    let events = world.step(&[]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::DroidReinforcements { .. })),
        "a wave is said"
    );
    assert_eq!(world.droid_wave_standing(), Some((wave + 1, left)));
    assert_eq!(
        machines(&mut world),
        before + size,
        "added, nothing cleared"
    );
    assert_eq!(world.heart_fight().unwrap().links_down, 1);
    let events = world.step(&[]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::DroidReinforcements { .. })),
        "a conduit is answered once"
    );
    room(&mut world).strike_droid(conduits[1], DroidPart::Chassis, 1e9);
    room(&mut world).strike_droid(conduits[2], DroidPart::Chassis, 1e9);
    world.step(&[]);
    assert_eq!(world.droid_wave_standing(), Some((wave + 3, left)));
    assert_eq!(world.heart_fight().unwrap().links_down, 3);
    assert_eq!(machines(&mut world), before + 3 * size, "two more came");
}

/// **Exposed, the core fires one beam** — never two — at a crew member it
/// can see, and **the fabricators build on their interval**; a fabricator
/// destroyed builds no more.
#[test]
fn exposed_the_core_fires_one_beam_and_the_fabricators_build() {
    let mut world = at_the_heart(1, 2);
    laid(&mut world);
    clear_the_wave(&mut world);
    assert!(world.set_heart_phase_for_probe(HeartPhase::Exposed));
    world.step(&[]);
    assert_eq!(phase(&world), HeartPhase::Exposed);
    let started = world.run.mission_steps;
    beside_the_core(&mut world, 0, bims::math::vec2(-1.0, 0.0), 4.0);
    beside_the_core(&mut world, 1, bims::math::vec2(1.0, 0.0), 4.0);
    let i = core(&mut world);
    let mut fired = false;
    for _ in 0..240 {
        world.step(&[]);
        let d = room(&mut world).droid(i).unwrap();
        assert_eq!(d.heart.emitters, 1);
        assert_eq!(d.heart.beams[1], Beam::Ready, "the second emitter is idle");
        fired |= matches!(d.heart.beams[0], Beam::Sweep { .. });
    }
    assert!(fired, "the core swept its beam at the crew");

    // The first build is an interval after the core was exposed.
    let built = |world: &World| world.heart_fight().unwrap().built;
    assert_eq!(built(&world), 0);
    while world.run.mission_steps < started + data::HEART_FABRICATOR_INTERVAL {
        world.step(&[]);
    }
    assert_eq!(built(&world), data::HEART_FABRICATORS, "one a fabricator");
    // One fabricator down, and the next build is one machine.
    let fabricators = of_kind(&mut world, DroidKind::Fabricator);
    room(&mut world).strike_droid(fabricators[0], DroidPart::Chassis, 1e9);
    let again = world.heart_fight().unwrap().next_build.unwrap();
    while world.run.mission_steps < again {
        world.step(&[]);
    }
    assert_eq!(
        built(&world),
        data::HEART_FABRICATORS + data::HEART_FABRICATORS - 1
    );
}

/// **Under the overload fraction the core fires two beams**, at two
/// different crew members when two are in sight, sweeps faster, and the
/// fabricators build faster.
#[test]
fn overloaded_the_core_fires_two_beams_faster_and_builds_faster() {
    let mut world = at_the_heart(1, 2);
    laid(&mut world);
    clear_the_wave(&mut world);
    assert!(world.set_heart_phase_for_probe(HeartPhase::Overload));
    let events = world.step(&[]);
    assert_eq!(phase(&world), HeartPhase::Overload);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::HeartOverload { .. }))
    );
    let now = world.run.mission_steps;
    let due = world.heart_fight().unwrap().next_build.unwrap();
    assert!(
        due - now <= data::HEART_FABRICATOR_INTERVAL / data::HEART_OVERLOAD_SPAWN_FACTOR,
        "the next build on the faster clock"
    );
    beside_the_core(&mut world, 0, bims::math::vec2(-1.0, 0.0), 4.0);
    beside_the_core(&mut world, 1, bims::math::vec2(1.0, 0.0), 4.0);
    let i = core(&mut world);
    let mut both = false;
    for _ in 0..240 {
        world.step(&[]);
        let d = room(&mut world).droid(i).unwrap();
        assert_eq!(d.heart.emitters, 2);
        assert_eq!(d.heart.pace, data::HEART_OVERLOAD_SWEEP_FACTOR);
        if let (Beam::WindUp { mark: a, .. }, Beam::WindUp { mark: b, .. }) =
            (d.heart.beams[0], d.heart.beams[1])
        {
            assert_ne!(a, b, "two beams, two targets");
            both = true;
        }
    }
    assert!(both, "both emitters wound up at once");
    // And the sweep laid on the crew's deck goes at the overload's pace.
    let paced = world
        .aboard
        .room
        .sweeps()
        .iter()
        .all(|s| s.pace == data::HEART_OVERLOAD_SWEEP_FACTOR);
    assert!(paced);
}

/// **The core at nothing is the run won**, once, with a player's Bim dead:
/// `run_won` said once, the fortress cleared, and the win kept.
#[test]
fn the_core_destroyed_wins_the_run_once_with_a_player_dead() {
    let mut world = at_the_heart(2, 3);
    laid(&mut world);
    clear_the_wave(&mut world);
    world.aboard.room.kill_for_probe(1);
    world.step(&[]);
    assert!(!world.lost, "one player of two is dead");
    assert!(world.set_heart_phase_for_probe(HeartPhase::Exposed));
    world.step(&[]);
    let i = core(&mut world);
    room(&mut world).strike_droid(i, DroidPart::Chassis, 1e9);
    let events = world.step(&[]);
    let won = events
        .iter()
        .filter(|e| matches!(e, WorldEvent::RunWon))
        .count();
    assert_eq!(won, 1, "said once: {events:?}");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::HeartDestroyed { .. }))
    );
    assert!(world.is_won());
    assert_eq!(phase(&world), HeartPhase::Destroyed);
    let id = world.heart_station().unwrap();
    assert!(world.droid_station_cleared(id));
    // And never again, whoever dies after.
    world.aboard.room.kill_for_probe(0);
    for _ in 0..5 {
        let events = world.step(&[]);
        assert!(!events.iter().any(|e| matches!(e, WorldEvent::RunWon)));
    }
    assert!(world.is_won() && !world.lost, "a run won stays won");
}

/// **Leaving the fortress before the core is down puts it back whole**:
/// its conduits, fabricators, core and phase are the fight the crew met,
/// and nothing is paid.
#[test]
fn leaving_the_fortress_puts_it_back_whole() {
    let mut world = at_the_heart(1, 2);
    laid(&mut world);
    let id = world.heart_station().unwrap();
    clear_the_wave(&mut world);
    assert!(world.set_heart_phase_for_probe(HeartPhase::Overload));
    let fabricators = of_kind(&mut world, DroidKind::Fabricator);
    room(&mut world).strike_droid(fabricators[0], DroidPart::Chassis, 1e9);
    world.step(&[]);
    assert_eq!(phase(&world), HeartPhase::Overload);
    let money = world.money;
    world.leave_for_probe();
    assert_eq!(
        world.run.phase,
        Phase::Map,
        "no reward for an uncleared site"
    );
    assert_eq!(world.money, money, "nothing paid");
    let it = world.infestation(id).unwrap();
    assert!(!it.cleared);
    assert!(
        it.heart.is_none(),
        "the fight as the crew met it: not begun"
    );
    assert!(!it.settled);

    // And the fortress met again is the whole of it.
    let mut world_again = at_the_heart(1, 2);
    laid(&mut world_again);
    let status = world_again.heart_status().unwrap();
    assert_eq!(status.phase, HeartPhase::Sealed);
    assert_eq!(status.conduits_left, status.conduits);
    assert_eq!(status.fabricators_left, data::HEART_FABRICATORS);
    assert_eq!(status.core_health, status.core_max);
}

/// **The map's preview is the fortress's strength on arrival**: the
/// conduits, the core, and the waves' size and count read at the arrival
/// day are what the fight is built with.
#[test]
fn the_map_s_preview_is_the_fortress_on_arrival() {
    let mut world = simulation_world(combat_ship(), REFERENCE_MONEY, 1);
    // The origin a lane away, and the clock two days short of the day
    // this system falls, so the lane inward is not jammed yet.
    let next = world.reachable_stars()[0];
    world.set_droid_origin_for_probe(next);
    world.set_day_for_probe(3);
    world.step(&[]);
    world.leave_for_probe();
    let site = Site {
        star: next,
        station: heart::heart_id(next),
    };
    let quote = world
        .travel_quote(site)
        .expect("the fortress is a trip away");
    let preview = quote.heart.expect("a fortress has a preview");
    assert!(quote.infested && quote.tier == bims::combat::Tier::Three);
    // Another site has none.
    assert!(
        world
            .travel_quotes()
            .into_iter()
            .filter(|(s, _)| *s != site)
            .filter_map(|(_, q)| q.ok())
            .all(|q| q.heart.is_none())
    );
    world.step(&[Command::Propose {
        slot: 0,
        star: site.star,
        station: site.station,
    }]);
    assert_eq!(world.run.phase, Phase::Mission, "the trip was made");
    assert_eq!(world.ship.state.alongside(), Some(site.station));
    laid(&mut world);
    let status = world.heart_status().unwrap();
    assert_eq!(status.conduits, preview.conduits);
    assert_eq!(status.core_max, preview.core_health);
    assert_eq!(world.droid_wave_size(), preview.wave_size);
    let it = world.infestation(site.station).unwrap();
    assert_eq!(it.waves_left + 1, preview.wave_count);
}

/// **The fight is the same on two worlds** given the same seed and the same
/// inputs: step for step the same events and the same checksum, through
/// the seal broken, the beams and the builds.
#[test]
fn the_fight_is_the_same_on_two_worlds() {
    let open = || {
        let mut world = at_the_heart(1, 3);
        laid(&mut world);
        beside_the_core(&mut world, 0, bims::math::vec2(-1.0, 0.0), 5.0);
        beside_the_core(&mut world, 1, bims::math::vec2(0.0, 1.0), 5.0);
        world
    };
    let (mut one, mut two) = (open(), open());
    assert_eq!(world_checksum(&one), world_checksum(&two));
    for step in 0..(data::HEART_FABRICATOR_INTERVAL as usize + 200) {
        if step == 30 {
            assert!(one.set_heart_phase_for_probe(HeartPhase::Exposed));
            assert!(two.set_heart_phase_for_probe(HeartPhase::Exposed));
        }
        let a = one.step(&[]);
        let b = two.step(&[]);
        assert_eq!(a, b, "step {step}");
        if step % 100 == 0 {
            assert_eq!(world_checksum(&one), world_checksum(&two), "step {step}");
        }
    }
    assert_eq!(world_checksum(&one), world_checksum(&two));
    assert!(
        one.heart_fight().unwrap().built > 0,
        "the fabricators built"
    );
}

/// **The `end` command's crew** (`World::end_crew_for_probe`): two
/// players and ten bots at the fortress, the bots plain Bims with no
/// class, the players at the top level with every point to spend,
/// everybody in tier-three kit — and two worlds made so step alike
/// through the fight.
#[test]
fn the_end_command_s_bots_are_plain_bims_in_tier_three_kit() {
    use crate::class::{self, Class};
    use bims::combat::{ArmourKind, Tier};
    let open = || {
        let mut world = at_the_heart(2, 12);
        world.set_class(0, Class::Soldier).unwrap();
        world.set_class(1, Class::Medic).unwrap();
        world.end_crew_for_probe(Tier::Three);
        world
    };
    let world = open();
    for who in 2..12u32 {
        assert_eq!(world.class_of(who), Class::None, "bot {who} has no class");
    }
    for who in 0..12u32 {
        let gear = world.aboard.room.gear(who as usize);
        assert_eq!(gear.weapon.map(|w| w.tier), Some(Tier::Three), "{who}");
        for kind in ArmourKind::BASIC {
            let worn = gear.worn(kind.slot()).expect("worn");
            assert_eq!(worn.tier, Tier::Three, "{who}");
        }
    }
    for who in 0..2u32 {
        assert_eq!(world.level_of(who), class::LEVELS, "player {who}");
        assert_eq!(
            world.points_of(who),
            class::LEVELS,
            "player {who} spends its own"
        );
    }
    assert_eq!(
        world.class_of(0),
        Class::Soldier,
        "a player keeps its own class"
    );
    assert_eq!(world.aboard.crew_count(), 12, "nobody else came");
    let (mut one, mut two) = (open(), open());
    assert_eq!(world_checksum(&one), world_checksum(&two));
    for step in 0..600 {
        let a = one.step(&[]);
        let b = two.step(&[]);
        assert_eq!(a, b, "step {step}");
    }
    assert_eq!(world_checksum(&one), world_checksum(&two));
}

/// The `end` command's waves: the world clock put at day sixty makes
/// them a run's twelve steps on, and the first mission's ease is off with
/// the switch — a wave the formula's whole, where the first mission's
/// is otherwise `first_mission_ease` fewer.
#[test]
fn the_end_command_s_waves_are_day_sixty_s_and_not_eased() {
    let mut world = at_the_heart(2, 2);
    world.set_day_for_probe(60);
    let whole = world.scaling().size(2, world.hours_gone());
    assert_eq!(world.scaling().steps(world.hours_gone()), 12);
    let eased = world.droid_wave_size();
    assert_eq!(
        eased,
        whole
            .saturating_sub(world.scaling().first_mission_ease)
            .max(1)
    );
    world.set_first_mission_uneased_for_probe();
    assert_eq!(world.droid_wave_size(), whole);
}
