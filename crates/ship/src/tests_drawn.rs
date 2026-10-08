//! A world steps the same whether or not, and however often, it is drawn.
//!
//! A run with company is lockstep: the host draws its world after nearly
//! every step, a guest's copy of the host's timeline (task 156) is never
//! drawn, and a guest without the rollback draws between other steps than
//! the host does. So nothing a frame does — `Session::age_effects`,
//! `set_blend`, `render` and the rooms' `observe`, light map, blend and
//! fade under it — may move anything a step reads. It once did: the
//! crew's trace put the doors as they stood at the frame into the sight
//! the rules read (`Sight::set_shut`), and the step read them before its
//! own fight put them back — a medic's healing circle, a Healing Sentry,
//! a runner's look, a command's line — so a world drawn healed, aimed and
//! ran otherwise than one that was not, and the two copies parted.
//!
//! Two tests, each comparing the copies' checksums and every crew
//! member's hit points to the bit at every step. A whole fight — a medic's
//! circle and beam, an engineer's sentries and mines, the bots led, waves
//! of machines — in three copies on the same orders: one drawn after every
//! step as the app draws a frame, one every third step, one never. And the
//! case that parted them, staged: a medic's circle reaching a crewmate
//! through the station's door of the mated airlock, one of the station's
//! people stood at that door and taken away again every few steps — the
//! door is open to the eye while somebody is at it, and the station's
//! people move after the crew's room has stepped.

use bims::combat::{Gear, Tier, WeaponKind};
use bims::math::Vec2;
use bims::order::{CrewOrder, angle_code};
use world::{Class, Command, World};

use crate::Session;
use crate::game::Game;
use crate::session::galaxy_type;

const W: f32 = 1400.0;
const H: f32 = 900.0;
const TILE: f32 = 52.0;
const STEPS: u64 = 3_000;

/// The combat ship's sixteen at the arena, for two players — a medic
/// and an engineer at the top level with every rank bought, so the
/// circle, the beam and both sentries are in the fight — the last four
/// field medics, a gun of every kind dealt down them, the dock the
/// machines' with `wave` a wave. The same every time it is built.
fn fight(wave: u32) -> Session {
    use shipdesign::fixture::{COMBAT_CREW, combat_ship};
    let seed = world::data::DEFAULT_SEED;
    let spawn = world::spawn(&worldgen::Galaxy::new(seed, galaxy_type(0)));
    let (star, station) = spawn.expect("the seed has a dock");
    let mut game = Game::start_with_crew(
        combat_ship(),
        world::data::SIMULATION_MONEY,
        2,
        COMBAT_CREW,
        0,
        seed,
        galaxy_type(0),
        star,
        station,
        W,
        H,
    )
    .expect("a world");
    assert!(game.world.arena_dock_for_probe(), "the arena");
    let room = &mut game.world.aboard.room;
    let kinds = WeaponKind::ALL.iter().copied().cycle();
    for (who, kind) in kinds.take(room.crew_count() as usize).enumerate() {
        let gear = room.gear(who);
        room.issue(
            who,
            Gear {
                weapon: Some(kind.basic()),
                ..gear
            },
        );
    }
    let mut session = Session::resumed(2, 0, game, seed, 0, spawn);
    session.field_medics_for_probe(crate::session::COMBAT_MEDICS);
    session.dress_crew();
    let world = &mut session.game.as_mut().expect("a game").world;
    for (slot, class) in [Class::Medic, Class::Engineer].into_iter().enumerate() {
        world.pick_class_for_probe(slot as u32, class, Tier::Two);
        world.set_ranks_for_probe(slot as u32, [world::class::MAX_RANK; world::class::SLOTS]);
    }
    world.set_droid_wave_for_probe(wave);
    assert!(session.infest_the_dock_for_probe(Some(Tier::Two), 0.5, 6));
    session
}

fn world_of(session: &Session) -> &World {
    &session.game.as_ref().expect("a game").world
}

fn tile_of(p: bims::math::Vec2) -> (i32, i32) {
    ((p.x / TILE).floor() as i32, (p.y / TILE).floor() as i32)
}

/// What the two players do this step, read off the world as it stands:
/// each walks at the nearest machine it sees, or off from it when close,
/// aiming and firing at it; the medic keeps his circle on and beams the
/// engineer now and then, and the engineer lays a sentry, a Healing
/// Sentry and a mine beside himself in turn.
fn orders(world: &World, step: u64) -> Vec<Command> {
    let room = &world.aboard.room;
    let targets = room.hostiles_for_probe();
    let mut out = Vec::new();
    for slot in 0..2u32 {
        let who = slot as usize;
        let phase = step + slot as u64 * 37;
        let me = room.bim_pos(who);
        // Seen or believed: the players make for the machines whether or
        // not one is in sight yet.
        let nearest = targets
            .iter()
            .flatten()
            .map(|t| (t.at, !t.stale))
            .min_by(|a, b| (a.0 - me).len().total_cmp(&(b.0 - me).len()));
        // The engineer's laying is an errand a word on the keys calls off:
        // he lays one every `LAY_EVERY` steps and is let be while he does.
        let laying = slot == 1 && phase % LAY_EVERY >= LAY_EVERY / 2;
        if phase % 15 == 0 && !laying {
            let (aim, fire, walk) = match nearest {
                Some((at, seen)) => {
                    let d = at - me;
                    let aim = d.angle();
                    let walk = if d.len() > 7.0 * TILE {
                        Some(aim + 0.3 * ((phase / 15) % 3) as f32 - 0.3)
                    } else if d.len() < 3.0 * TILE {
                        Some(aim + std::f32::consts::PI)
                    } else if (phase / 15) % 4 == 0 {
                        None
                    } else {
                        Some(aim + std::f32::consts::FRAC_PI_2)
                    };
                    (aim, seen, walk)
                }
                None => {
                    let aim = (phase / 15) as f32 * 0.7;
                    (aim, false, Some(aim))
                }
            };
            out.push(Command::Crew {
                slot,
                order: CrewOrder::Control {
                    walk: walk.map(angle_code),
                    aim: angle_code(aim),
                    fire,
                    sprint: false,
                },
            });
        }
        // The bots told to fight their way to the nearest machine, to fall
        // back to the ship through the airlock with the machines after
        // them, or to follow.
        if phase % 260 == 10 {
            let order = match ((phase / 260) % 3, nearest) {
                (0, Some((at, _))) => world::Standing::Attack { tile: tile_of(at) },
                (1, _) => world::Standing::Retreat,
                _ => world::Standing::Follow,
            };
            out.push(Command::Orders { slot, order });
        }
        let (mx, my) = tile_of(me);
        if slot == 0 && phase % 70 == 35 {
            let abilities = [
                Command::HealingCircle { slot, on: true },
                Command::Beam {
                    slot,
                    patient: Some(1),
                },
                Command::HealDrone { slot },
            ];
            out.push(abilities[(phase / 70) as usize % abilities.len()]);
        }
        if slot == 1 && phase % LAY_EVERY == LAY_EVERY / 2 {
            // The first free tile beside him, a ring out.
            let free = [(0, 1), (1, 0), (0, -1), (-1, 0), (1, 1), (-1, -1)]
                .into_iter()
                .map(|(dx, dy)| (mx + dx, my + dy));
            let lays = [
                world::DeployKind::Sentry,
                world::DeployKind::HealingSentry,
                world::DeployKind::Mine,
            ];
            let kind = lays[(phase / LAY_EVERY) as usize % lays.len()];
            let tile = free
                .clone()
                .find(|&t| world.can_deploy(slot, kind, t).is_ok())
                .unwrap_or((mx, my + 1));
            // The keys let go first, or the walk they hold is his.
            out.push(Command::Crew {
                slot,
                order: CrewOrder::Control {
                    walk: None,
                    aim: angle_code(0.0),
                    fire: false,
                    sprint: false,
                },
            });
            out.push(match kind {
                world::DeployKind::Sentry => Command::Sentry { slot, tile },
                kind => Command::Deploy {
                    slot,
                    kind,
                    x: tile.0,
                    y: tile.1,
                },
            });
        }
    }
    out
}

/// How often the engineer lays something, in steps: the longest laying
/// (a Healing Sentry, four minutes at the top rank) is half of it.
const LAY_EVERY: u64 = 600;

/// One step of a session's game with these orders, the way the app takes
/// it: queued, then stepped.
fn step_with(session: &mut Session, commands: &[Command]) {
    let game = session.game.as_mut().expect("a game");
    game.queued.extend_from_slice(commands);
    session.world_step();
}

/// A frame, as the app draws one between steps: the passing lights aged
/// on the window's clock, the bodies blended part of the way to the next
/// step, and the picture built — both rooms drawn, the crew's sight
/// traced and its light map made.
fn frame(session: &mut Session) {
    session.age_effects(1.0 / 60.0);
    session.set_blend(Some(0.5));
    session.render();
}

/// A copy agrees with the world never drawn: by checksum, and by every
/// crew member's hit points to the bit — a heal the one had and the other
/// did not is in the bars long before it is anywhere the checksum reads.
fn agree(name: &str, step: u64, copy: &World, reference: &World) {
    let (a, b) = (copy.checksum(), reference.checksum());
    let (room, undrawn) = (&copy.aboard.room, &reference.aboard.room);
    let bars = |room: &bims::game::Game| -> Vec<u32> {
        (0..room.crew_count() as usize)
            .map(|who| room.health(who).to_bits())
            .collect()
    };
    if a == b && bars(room) == bars(undrawn) {
        return;
    }
    let parted: Vec<String> = (0..room.body_count() as usize)
        .filter(|&i| room.body_pos(i) != undrawn.body_pos(i))
        .map(|i| {
            format!(
                "body {i} at {:?}, undrawn at {:?}",
                room.body_pos(i),
                undrawn.body_pos(i)
            )
        })
        .chain(
            (0..room.crew_count() as usize)
                .filter(|&who| room.health(who).to_bits() != undrawn.health(who).to_bits())
                .map(|who| {
                    format!(
                        "crew member {who} at {} hit points, undrawn at {}",
                        room.health(who),
                        undrawn.health(who)
                    )
                }),
        )
        .take(8)
        .collect();
    panic!(
        "the world drawn {name} parted from the one never drawn at step {step} \
         (checksum {a:#x} against {b:#x}):\n  {}",
        parted.join("\n  ")
    );
}

#[test]
fn a_fight_steps_the_same_drawn_every_step_every_third_or_never() {
    let mut drawn = fight(14);
    let mut third = fight(14);
    let mut never = fight(14);
    let (mut circling, mut laid, mut machines_down) = (0u32, 0u32, 0u32);
    let mut standing = world_of(&never).droids_standing();
    for step in 0..STEPS {
        let commands = orders(world_of(&never), step);
        step_with(&mut drawn, &commands);
        step_with(&mut third, &commands);
        step_with(&mut never, &commands);
        frame(&mut drawn);
        if step % 3 == 0 {
            frame(&mut third);
        }
        let reference = world_of(&never);
        agree("every step", step, world_of(&drawn), reference);
        agree("every third step", step, world_of(&third), reference);
        circling += u32::from(reference.is_circling(0));
        laid = laid.max(reference.deployables.len() as u32);
        let now = reference.droids_standing();
        machines_down += standing.saturating_sub(now);
        standing = now;
    }
    eprintln!("circling {circling} steps, {laid} laid at most, machines down {machines_down}");
    // And it was the fight it was staged to be.
    assert!(machines_down >= 4, "machines went down: {machines_down}");
    assert!(circling > 0, "the medic's circle was on");
    assert!(laid > 0, "the engineer laid something");
}

/// The airlock the ship is mated to the station by, as the crew's room
/// has it: the middle of the ship's door and of the station's.
fn mated_doors(world: &World) -> (Vec2, Vec2) {
    let port = shipdesign::port(&world.ship.design).expect("the ship has a port");
    let offset = world.aboard.offset;
    let at = |(x, y): (f64, f64)| bims::math::vec2((x + offset.x) as f32, (y + offset.y) as f32);
    let (ship_side, face) = (at(port.centre), at(port.face()));
    let doors = world.aboard.room.ship_doors_for_probe();
    let nearest = |p: Vec2| {
        doors
            .iter()
            .map(|&(c, _)| c)
            .min_by(|a, b| (*a - p).len().total_cmp(&(*b - p).len()))
            .expect("doors")
    };
    let (ship, station) = (nearest(ship_side), nearest(face + (face - ship_side)));
    assert!(
        (ship - ship_side).len() < 1.0 && ship != station,
        "the two doors of the mated airlock: {ship:?} and {station:?} about the port at {face:?}"
    );
    (ship, station)
}

/// A world drawn or not heals alike through the airlock it is mated to
/// the station by — the one door whose leaves the eye reads off who stands
/// at it, the station's people included, and they move after the crew's
/// room has stepped. A medic stands in the ship's door with his circle
/// on, a crewmate a little way into the station beyond its door, out of
/// the door's reach, and one of the station's people is stood at that
/// door and taken away again every few steps: the circle reaches through
/// it only while somebody is at it.
#[test]
fn a_medic_s_circle_through_the_airlock_heals_alike_drawn_or_never() {
    use shipdesign::fixture::combat_ship;
    let build = || {
        let spawn = crate::session::pick_dock(12_345, 0, 678);
        let mut session = Session::simulate_on(combat_ship(), 2, 12_345, 0, spawn, W, H);
        let world = &mut session.game.as_mut().expect("a game").world;
        world.pick_class_for_probe(0, Class::Medic, Tier::Two);
        world.set_ranks_for_probe(0, [world::class::MAX_RANK; world::class::SLOTS]);
        assert!(world.residents.is_some(), "a station with people aboard");
        session
    };
    let mut drawn = build();
    let mut never = build();
    let (ship, station) = mated_doors(world_of(&never));
    let along = (station - ship).normalize_or_zero();
    // A door is a tile deep; its leaves open to anybody within
    // `door::REACH` of it.
    let reach = bims::door::REACH + TILE / 2.0;
    let medic = station - along * (reach + 6.0);
    let patient = station + along * (reach + 14.0);
    let at_the_door = station + along * (TILE / 2.0 + 16.0);
    let away = station + along * (8.0 * TILE);
    let (mut healed, mut not) = (0u32, 0u32);
    for step in 0..240u64 {
        let near = (step / 3) % 2 == 0;
        for session in [&mut drawn, &mut never] {
            let world = &mut session.game.as_mut().expect("a game").world;
            let room = &mut world.aboard.room;
            room.put_for_probe(0, medic);
            room.put_for_probe(1, patient);
            let full = room.max_health(0);
            room.set_health_for_probe(0, full);
            room.set_health_for_probe(1, 50.0);
            // In the station's own room, at the same place of the
            // station's design (`Aboard::visit`'s frame, read backwards).
            let there = if near { at_the_door } else { away };
            let there = world
                .aboard
                .to_station(worldgen::math::dvec2(there.x as f64, there.y as f64))
                .expect("joined");
            let residents = world.residents.as_mut().expect("the station's people");
            let there = there.add(residents.aboard.offset);
            residents
                .aboard
                .room
                .put_for_probe(0, bims::math::vec2(there.x as f32, there.y as f32));
        }
        let commands = if step % 60 == 0 {
            vec![Command::HealingCircle { slot: 0, on: true }]
        } else {
            Vec::new()
        };
        step_with(&mut drawn, &commands);
        step_with(&mut never, &commands);
        frame(&mut drawn);
        agree("every step", step, world_of(&drawn), world_of(&never));
        if world_of(&never).aboard.room.health(1) > 50.0 {
            healed += 1;
        } else {
            not += 1;
        }
    }
    eprintln!("healed through the airlock {healed} steps, shut out {not}");
    // And the airlock was what said: both happened.
    assert!(healed > 0 && not > 0, "healed {healed}, not {not}");
}
