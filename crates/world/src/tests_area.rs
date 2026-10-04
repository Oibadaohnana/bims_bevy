//! An Area defend (October 2026): a town's defence is holding its FOB —
//! the ring at its crossing, sandbags round it — for five minutes of
//! waves without end, the machines taking it by standing in the ring with
//! nobody of the crew's side there; and every wave of a mission the size
//! of its first.

use shipdesign::fixture::flyer;
use worldgen::math::dvec2;

use crate::data;
use crate::event::WorldEvent;
use crate::fixture::{REFERENCE_MONEY, open_simulation_world};
use crate::world::World;

/// The town the ship lands at, its defence an Area defend: the machines'
/// waves whatever the day, the first a handful of steps off and `size`
/// machines each. `None` where the roll put no friendly town to land at.
fn an_area_defend(size: Option<u32>) -> Option<(World, u32)> {
    let mut world = open_simulation_world(flyer(2), REFERENCE_MONEY, 2);
    world.set_defense_by_machines_for_probe();
    world.set_defense_delay_for_probe(data::STEP_MINUTES * 4.0);
    if let Some(n) = size {
        world.set_droid_wave_for_probe(n);
    }
    if !world.land_for_probe() {
        return None;
    }
    let id = world.ship.state.alongside()?;
    if !world.site_threatened(id) {
        return None;
    }
    assert!(
        world.is_area_defense(id),
        "a town's defence is an Area defend"
    );
    Some((world, id))
}

/// Step until the predicate holds, or give up after `steps`; the events
/// said on the way.
fn until(
    world: &mut World,
    steps: u32,
    mut done: impl FnMut(&World) -> bool,
) -> (bool, Vec<WorldEvent>) {
    let mut said = Vec::new();
    for _ in 0..steps {
        said.extend(world.step(&[]));
        if done(world) {
            return (true, said);
        }
    }
    (false, said)
}

fn destroy_the_wave(world: &mut World) {
    let Some(residents) = world.residents.as_mut() else {
        return;
    };
    let room = &mut residents.aboard.room;
    for i in 0..room.droid_count() as usize {
        for _ in 0..60 {
            room.strike_droid(i, bims::droid::DroidPart::Chassis, 100.0);
        }
    }
    assert_eq!(world.droids_standing(), 0, "a machine survived the lot");
}

/// The ring's middle in the residents' room's units.
fn middle_ashore(world: &World, id: u32) -> bims::math::Vec2 {
    let area = world
        .defense(id)
        .and_then(|d| d.area.clone())
        .expect("a ring");
    let residents = world.residents.as_ref().expect("the town's room");
    residents
        .aboard
        .to_room(dvec2(area.x as f64, area.y as f64))
}

/// The first step lays the FOB: the ring at the crossing, sandbags on its
/// open ground as low cover in both rooms, and the crew stood in it.
#[test]
fn an_area_defend_lays_its_fob_and_stands_the_crew_in_it() {
    let Some((mut world, id)) = an_area_defend(Some(2)) else {
        return;
    };
    world.step(&[]);
    let area = world
        .defense(id)
        .and_then(|d| d.area.clone())
        .expect("a ring");
    assert_eq!(
        area.left,
        data::AREA_HOLD_STEPS,
        "the hold waits for the first wave"
    );
    assert!(!area.bags.is_empty(), "sandbags round the FOB");
    let t = shipdesign::TILE as f64;
    let reach = data::AREA_RADIUS_TILES * t;
    for &(x, y) in &area.bags {
        let p = dvec2((x as f64 + 0.5) * t, (y as f64 + 0.5) * t);
        let d = p.sub(dvec2(area.x as f64, area.y as f64)).length();
        assert!(d < reach, "a bag outside the ring at {x},{y}");
    }
    let residents = world.residents.as_ref().expect("the town's room");
    assert_eq!(
        residents.aboard.room.laid_cover().len(),
        area.bags.len() + 1,
        "the bags and the post are cover in the town's room"
    );
    assert_eq!(world.aboard.room.laid_cover().len(), area.bags.len() + 1);
    let ashore = world.aboard.crew_ashore();
    let middle = dvec2(area.x as f64, area.y as f64);
    let first = ashore[0].expect("crew member 0 ashore");
    assert!(
        first.sub(middle).length() < reach,
        "the crew are stood in the ring"
    );
    assert!(world.area_in_room().is_some());
}

/// The waves never run out while the hold has time, and they are on a
/// clock from each landing whether or not the last is down — they stack —
/// twenty-five seconds after the first and a second sooner a wave. Once the hold is
/// over no wave lands, and those on the ground destroyed are the site held.
#[test]
fn the_waves_come_on_a_clock_until_the_hold_is_over_and_the_last_are_destroyed() {
    let Some((mut world, id)) = an_area_defend(Some(1)) else {
        return;
    };
    let (landed, _) = until(&mut world, 40, |w| w.droids_standing() > 0);
    assert!(landed, "the first wave never landed");
    // Five more waves, nobody destroyed: each lands its gap after the last.
    // Nobody shooting at them: the town's guard and defenders dead, the
    // crew stood by the pad every step.
    nobody_under_arms(&mut world);
    let pad = pad_on_deck(&world);
    let mut gaps = Vec::new();
    let mut most = 0;
    for _ in 0..5 {
        let mut steps = 0u64;
        let mut came = false;
        let mut said = Vec::new();
        for _ in 0..2_000 {
            for who in 0..world.aboard.room.crew_count() as usize {
                world.aboard.room.put_for_probe(who, pad);
            }
            // And the machines held where they landed, out of the ring.
            if let Some(residents) = world.residents.as_mut() {
                let room = &mut residents.aboard.room;
                for i in 0..room.droid_count() as usize {
                    if let Some(d) = room.droid_mut_for_probe(i) {
                        d.stun(1.0, false);
                    }
                }
            }
            said.extend(world.step(&[]));
            steps += 1;
            most = most.max(world.droids_standing());
            if world
                .defense(id)
                .is_some_and(|d| d.wave as usize == gaps.len() + 2)
            {
                came = true;
                break;
            }
        }
        assert!(came, "a wave did not come while the hold had time");
        assert!(
            !said
                .iter()
                .any(|e| matches!(e, WorldEvent::TownHeld { .. })),
            "held with time left"
        );
        gaps.push(steps);
    }
    assert_eq!(
        gaps,
        [1_500, 1_440, 1_380, 1_320, 1_260],
        "twenty-five seconds, then a second sooner a wave"
    );
    assert!(most > 1, "the waves stack: {most} up at once at most");
    // The hold at its end: the time runs out with the waves still up, and
    // no wave comes after it.
    world.set_area_left_for_probe(3);
    let (_, said) = until(&mut world, 5, |_| false);
    assert!(
        said.iter()
            .any(|e| matches!(e, WorldEvent::AreaTimeUp { .. })),
        "the end of the hold is said"
    );
    destroy_the_wave(&mut world);
    let (held, said) = until(&mut world, 1_000, |w| w.site_cleared(id));
    assert!(held, "the last waves destroyed and the site not held");
    assert!(
        said.iter()
            .any(|e| matches!(e, WorldEvent::TownHeld { .. }))
    );
    assert_eq!(world.droids_standing(), 0, "a wave came after the hold");
    assert!(!world.lost);
}

/// The machines in the ring with nobody of the crew's side in it count
/// up; a friend in it stops the count, and none of them in it puts it
/// back. Twenty seconds of it is the FOB taken and the run lost.
#[test]
fn the_machines_alone_in_the_ring_take_it_and_the_run_is_lost() {
    let Some((mut world, id)) = an_area_defend(Some(2)) else {
        return;
    };
    let (landed, _) = until(&mut world, 40, |w| w.droids_standing() > 0);
    assert!(landed, "the first wave never landed");
    // Nobody of the town's under arms: the guard and the defenders dead.
    {
        let residents = world.residents.as_mut().expect("the town's room");
        let room = &mut residents.aboard.room;
        for who in 0..room.crew_count() as usize {
            if residents.defender.get(who).copied().unwrap_or(false)
                || who == crate::surface::GUARD as usize
            {
                room.kill_for_probe(who);
            }
        }
    }
    // The crew far off by the pad, and the machines in the middle of the
    // ring, held there every step.
    let t = shipdesign::TILE as f64;
    let pad = world
        .aboard
        .from_station(dvec2(4.5 * t, 47.5 * t))
        .expect("joined");
    let pad = bims::math::vec2(pad.x as f32, pad.y as f32);
    let middle = middle_ashore(&world, id);
    let hold = |world: &mut World, crew_in: bool| {
        for who in 0..world.aboard.room.crew_count() as usize {
            let at = if crew_in && who == 0 {
                world.area_in_room().map_or(pad, |(c, _)| c)
            } else {
                pad
            };
            world.aboard.room.put_for_probe(who, at);
        }
        let residents = world.residents.as_mut().expect("the town's room");
        for i in 0..residents.aboard.room.droid_count() as usize {
            if let Some(d) = residents.aboard.room.droid_mut_for_probe(i) {
                d.pos = middle;
            }
        }
    };
    for _ in 0..60 {
        hold(&mut world, false);
        world.step(&[]);
    }
    let (share, _) = world.area_taken_share().expect("an Area defend");
    assert!(share > 0.0, "the machines alone in the ring count up");
    // A friend in the ring holds the count where it is.
    hold(&mut world, true);
    world.step(&[]);
    let (before, _) = world.area_taken_share().expect("an Area defend");
    for _ in 0..30 {
        hold(&mut world, true);
        world.step(&[]);
    }
    let (after, contested) = world.area_taken_share().expect("an Area defend");
    assert_eq!(before, after, "a friend in the ring stops the count");
    assert!(contested);
    // Alone again for the whole count: taken, and the run lost.
    let mut said = Vec::new();
    for _ in 0..data::AREA_CAPTURE_STEPS + 10 {
        hold(&mut world, false);
        said.extend(world.step(&[]));
        if world.lost {
            break;
        }
    }
    assert!(
        said.iter()
            .any(|e| matches!(e, WorldEvent::AreaTaken { .. })),
        "the FOB taken is said"
    );
    assert!(world.lost, "the FOB taken is the run lost");
    assert!(world.area_fell());
}

/// Every wave of a mission is the size of its first, whatever moves the
/// formula after it — and the next mission works it out afresh.
#[test]
fn every_wave_of_a_mission_is_its_first_wave_s_size() {
    let Some((mut world, _)) = an_area_defend(None) else {
        return;
    };
    // The defence laid, its defenders fielded: what the first wave is.
    world.step(&[]);
    let first = world.droid_wave_size();
    let (landed, _) = until(&mut world, 40, |w| w.droids_standing() > 0);
    assert!(landed, "the first wave never landed");
    assert_eq!(world.run.wave_size, Some(first));
    // A much later day would make a much bigger wave: not this mission.
    world.set_day_for_probe(60);
    assert_eq!(world.droid_wave_size(), first, "fixed at the first wave");
    destroy_the_wave(&mut world);
    let (came, _) = until(&mut world, 2_000, |w| w.droids_standing() > 0);
    assert!(came);
    assert_eq!(
        world.droids_standing(),
        first,
        "the second wave as the first"
    );
    world.leave_for_probe();
    assert_eq!(world.run.wave_size, None, "worked out afresh next mission");
}

/// Before day ten a wave is the Manufacturers' people and their Troopers,
/// and their people go in among the Bims, before the machines: a wave
/// stacked on one still standing moves every machine on, and what the
/// world keeps a body (who is down, who was counted) moves with it — so
/// the last of them down is still the site held, said once.
#[test]
fn the_manufacturers_waves_stack_and_every_body_is_counted_once() {
    let mut world = open_simulation_world(flyer(2), REFERENCE_MONEY, 2);
    world.set_defense_delay_for_probe(data::STEP_MINUTES * 4.0);
    world.set_droid_wave_for_probe(4);
    if !world.land_for_probe() {
        return;
    }
    let Some(id) = world.ship.state.alongside() else {
        return;
    };
    if !world.site_threatened(id) || !world.defense_by_manufacturers() {
        return;
    }
    let (stacked, _) = until(&mut world, 4_000, |w| {
        w.defense(id).is_some_and(|d| d.wave >= 3)
    });
    assert!(stacked, "three waves landed");
    world.set_area_left_for_probe(2);
    world.step(&[]);
    // Every enemy down: their people, then the machines.
    {
        let residents = world.residents.as_mut().expect("the town's room");
        let room = &mut residents.aboard.room;
        for who in 0..room.crew_count() as usize {
            if room.is_manufacturer(who) {
                room.kill_for_probe(who);
            }
        }
    }
    world.step(&[]);
    destroy_the_wave(&mut world);
    let (held, said) = until(&mut world, 600, |w| w.site_cleared(id));
    assert!(held, "every enemy down and the site not held");
    let held_said = said
        .iter()
        .filter(|e| matches!(e, WorldEvent::TownHeld { .. }))
        .count();
    assert_eq!(held_said, 1);
    let residents = world.residents.as_ref().expect("the town's room");
    let bodies = residents.aboard.room.body_count() as usize;
    assert_eq!(residents.down.len(), bodies, "a flag a body");
    assert!(world.droids_standing() == 0);
}

/// The town's guard and defenders dead: nobody of the site's under arms.
fn nobody_under_arms(world: &mut World) {
    let residents = world.residents.as_mut().expect("the town's room");
    let room = &mut residents.aboard.room;
    for who in 0..room.crew_count() as usize {
        if residents.defender.get(who).copied().unwrap_or(false)
            || who == crate::surface::GUARD as usize
        {
            room.kill_for_probe(who);
        }
    }
}

/// A point by the pad, on the joined deck: far from the FOB.
fn pad_on_deck(world: &World) -> bims::math::Vec2 {
    let t = shipdesign::TILE as f64;
    let pad = world
        .aboard
        .from_station(dvec2(4.5 * t, 47.5 * t))
        .expect("joined");
    bims::math::vec2(pad.x as f32, pad.y as f32)
}
