//! The tier-two machines (task 157), as far as the room is concerned: the
//! Bomber's bomb — rolled within reach, stopped short of a wall, bursting on
//! the crew and on the machines alike — the Lancer's charge, which follows
//! its mark and then holds the line it locked, and is put out by a stun or
//! its arms, and the Conductor's link, mark, blink and strike call. Which
//! waves they come in is the world's (`crates/world/src/tests_tier_two.rs`).

use crate::balance;
use crate::combat::{Tier, WeaponKind};
use crate::droid::{Droid, DroidKind, DroidPart, Rail};
use crate::game::Game;
use crate::math::{Vec2, vec2};
use crate::room::{ROOM_H, ROOM_W, TILE};

const DT: f32 = 1.0 / 60.0;

/// The bare room as a machines' room: its two Bims dead and its bodies
/// hostile, so what fights in it is whatever machine a test adds.
fn machines_room() -> Game {
    let mut game = Game::bare(3, ROOM_W, ROOM_H);
    game.set_autonomous(false);
    game.set_hostile_bodies(true);
    game.kill_for_probe(0);
    game.kill_for_probe(1);
    game.simulate(DT);
    game
}

/// A machine of `kind` at `at`, its legs shot away so that where it stands
/// is the test's and not its walk's.
fn planted(game: &mut Game, kind: DroidKind, at: Vec2) -> usize {
    let i = machine(game, Droid::new(kind, Tier::Two, 0, 1, at, 0.0, 17));
    let legs = game.droids()[i].body.max(DroidPart::Legs);
    game.strike_droid(i, DroidPart::Legs, legs);
    i
}

/// A machine put on the deck: its index among the machines, which is what
/// `droids()` and `strike_droid` take (`add_droid` answers the body's).
fn machine(game: &mut Game, droid: Droid) -> usize {
    game.add_droid(droid) - game.crew_count() as usize
}

/// The middle of the bare room.
fn middle() -> Vec2 {
    vec2(ROOM_W * 0.5, ROOM_H * 0.5)
}

#[test]
fn a_bomber_rolls_a_bomb_at_a_bim_in_reach_and_again_after_its_cooldown() {
    let mut game = machines_room();
    let at = middle() - vec2(4.0 * TILE, 0.0);
    planted(&mut game, DroidKind::Bomber, at);
    let pistol = WeaponKind::LaserPistol.basic();
    // Nine tiles off — past its reach — nothing is rolled.
    let far = at + vec2(9.0 * TILE, 0.0);
    for _ in 0..(60 * 3) {
        game.set_hostiles(vec![Some((far, pistol))]);
        game.simulate(DT);
        assert!(
            !game.take_shots().iter().any(|s| s.bomb),
            "nothing at nine tiles"
        );
    }
    // Six tiles off: one a moment after, rolled five tiles at it, and the
    // next after its cooldown.
    let near = at + vec2(6.0 * TILE, 0.0);
    let mut bombs = Vec::new();
    for frame in 0..(60 * 9) {
        game.set_hostiles(vec![Some((near, pistol))]);
        game.simulate(DT);
        for shot in game.take_shots().into_iter().filter(|s| s.bomb) {
            assert!(shot.grenade, "a bomb is a grenade's shot");
            assert_eq!(shot.from, at, "rolled from where it stands");
            let rolled = (shot.at - shot.from).len();
            assert!(
                rolled <= balance::BOMB_REACH * TILE + 0.01 && rolled > 4.0 * TILE,
                "rolled {} tiles",
                rolled / TILE
            );
            assert!(
                (shot.at.y - at.y).abs() < 1.0,
                "along the line to the Bim: {:?}",
                shot.at
            );
            bombs.push(frame);
        }
    }
    assert_eq!(bombs.len(), 2, "one, then one a cooldown on: {bombs:?}");
    let first = (bombs[0] + 1) as f32 * DT;
    assert!(
        (first - balance::BOMB_FIRST).abs() < 0.05,
        "the first a moment after it saw the Bim: {first}"
    );
    let gap = (bombs[1] - bombs[0]) as f32 * DT;
    assert!(
        (gap - balance::BOMB_COOLDOWN).abs() < 0.05,
        "a cooldown apart: {gap}"
    );
}

#[test]
fn a_bomb_stops_short_of_a_wall() {
    let game = machines_room();
    let from = vec2(ROOM_W - 2.0 * TILE, ROOM_H * 0.5);
    let stop = game.bomb_stop(from, from + vec2(4.0 * TILE, 0.0));
    assert!(stop.x > from.x + 0.5 * TILE, "it rolled: {stop:?}");
    assert!(stop.x < ROOM_W, "and stopped inside the wall: {stop:?}");
    // In the open it goes the whole way.
    let open = middle();
    let to = open + vec2(3.0 * TILE, 0.0);
    assert!((game.bomb_stop(open, to) - to).len() < 1.0);
}

#[test]
fn a_bomb_bursts_after_its_fuse_on_the_crew_and_on_the_machines_alike() {
    // The crew's room: Bim 0 a tile from where the bomb stops, Bim 1 well
    // out of it, and a machine standing beside it.
    let mut game = Game::bare(4, ROOM_W, ROOM_H);
    game.set_autonomous(false);
    let spot = middle();
    game.put_for_probe(0, spot + vec2(TILE, 0.0));
    game.put_for_probe(1, spot + vec2(-6.0 * TILE, 0.0));
    let machine = Some((spot + vec2(0.0, TILE), WeaponKind::LaserPistol.basic()));
    game.set_hostiles(vec![machine]);
    game.enemy_bomb(spot - vec2(4.0 * TILE, 0.0), spot, 40.0);
    assert!(game.grenades()[0].bomb && game.grenades()[0].hostile);
    let whole = game.health(0);
    let mut burst_at = None;
    let mut machine_hits = Vec::new();
    for frame in 0..(60 * 3) {
        game.set_hostiles(vec![machine]);
        game.simulate(DT);
        machine_hits.extend(game.take_hits().into_iter().filter(|h| h.blast));
        if burst_at.is_none() && game.health(0) < whole {
            burst_at = Some(frame);
        }
    }
    let burst_at = burst_at.expect("the Bim beside it was hurt");
    assert!(
        ((burst_at + 1) as f32 * DT - balance::BOMB_FUSE).abs() < 0.05,
        "after its fuse: frame {burst_at}"
    );
    assert_eq!(game.health(1), whole, "the Bim out of its reach was not");
    assert_eq!(machine_hits.len(), 1, "the machine in it took the burst");
    assert_eq!(machine_hits[0].by, None, "an enemy's, nobody's of the crew");
}

#[test]
fn a_lancer_follows_its_mark_then_fires_along_the_line_it_locked() {
    let mut game = machines_room();
    let at = middle() - vec2(5.0 * TILE, 0.0);
    let i = planted(&mut game, DroidKind::Lancer, at);
    let pistol = WeaponKind::LaserPistol.basic();
    let first = at + vec2(8.0 * TILE, 0.0);
    // While it follows, the Bim steps aside; once it has locked, it steps
    // aside again — the slug goes where it stood at the lock.
    let at_lock = first + vec2(0.0, 2.0 * TILE);
    let after = first + vec2(0.0, -2.0 * TILE);
    let mut charged_at = None;
    let mut shot = None;
    for frame in 0..(60 * 3) {
        let charge = match game.droids()[i].rhythm.rail {
            Rail::Charging { left, .. } => Some(left),
            _ => None,
        };
        if charge.is_some() && charged_at.is_none() {
            charged_at = Some(frame);
        }
        let bim = match charge {
            None => first,
            Some(left) if left > balance::LANCER_LOCK - 0.05 => at_lock,
            Some(_) => after,
        };
        game.set_hostiles(vec![Some((bim, pistol))]);
        game.simulate(DT);
        if let Some(s) = game
            .take_shots()
            .into_iter()
            .find(|s| s.weapon.kind == WeaponKind::Rail)
        {
            shot = Some((frame, s));
            break;
        }
    }
    let charged_at = charged_at.expect("it charged");
    let (fired, shot) = shot.expect("it fired");
    let charge = (fired - charged_at + 1) as f32 * DT;
    assert!(
        (charge - (balance::LANCER_TRACK + balance::LANCER_LOCK)).abs() < 0.05,
        "after its charge: {charge}"
    );
    let line = (shot.at - shot.from).normalize_or_zero();
    let locked = (at_lock - at).normalize_or_zero();
    assert!(
        line.dot(locked) > 0.999,
        "along the line it locked: {line:?} against {locked:?}"
    );
    assert!(
        (shot.at - shot.from).len() >= 12.0 * TILE,
        "out to the rail's reach"
    );
}

#[test]
fn a_stun_or_its_arms_shot_away_put_a_lancer_s_charge_out() {
    let pistol = WeaponKind::LaserPistol.basic();
    for stun in [true, false] {
        let mut game = machines_room();
        let at = middle() - vec2(5.0 * TILE, 0.0);
        let i = planted(&mut game, DroidKind::Lancer, at);
        let bim = at + vec2(8.0 * TILE, 0.0);
        let mut fired = false;
        for frame in 0..(60 * 3) {
            game.set_hostiles(vec![Some((bim, pistol))]);
            game.simulate(DT);
            if frame == 30 {
                assert!(
                    matches!(game.droids()[i].rhythm.rail, Rail::Charging { .. }),
                    "charging half a second in"
                );
                if stun {
                    game.droid_mut_for_probe(i).unwrap().stun(0.5, false);
                } else {
                    let arms = game.droids()[i].body.max(DroidPart::Arms);
                    game.strike_droid(i, DroidPart::Arms, arms);
                }
            }
            if frame == 31 {
                assert!(
                    matches!(game.droids()[i].rhythm.rail, Rail::Cooling { .. }),
                    "put out (stun {stun})"
                );
            }
            fired |= game
                .take_shots()
                .iter()
                .any(|s| s.weapon.kind == WeaponKind::Rail);
            if frame == 60 + 30 {
                break;
            }
        }
        assert!(!fired, "nothing fired from a charge put out (stun {stun})");
    }
}

#[test]
fn a_conductor_s_link_takes_some_of_every_hit_within_its_reach() {
    let mut game = machines_room();
    let at = middle();
    planted(&mut game, DroidKind::Conductor, at);
    let near = machine(
        &mut game,
        Droid::new(
            DroidKind::Trooper,
            Tier::One,
            0,
            1,
            at + vec2(3.0 * TILE, 0.0),
            0.0,
            5,
        ),
    );
    let far = machine(
        &mut game,
        Droid::new(
            DroidKind::Trooper,
            Tier::One,
            1,
            1,
            at - vec2(8.0 * TILE, 0.0),
            0.0,
            6,
        ),
    );
    game.simulate(DT);
    assert!(game.droids()[near].rhythm.linked);
    assert!(!game.droids()[far].rhythm.linked);
    assert_eq!(
        game.droids()[0].tethers.len(),
        1,
        "one tether, to the near one"
    );
    let whole = game.droids()[near].body.life();
    game.strike_droid(near, DroidPart::Chassis, 20.0);
    game.strike_droid(far, DroidPart::Chassis, 20.0);
    assert!(
        (whole - game.droids()[near].body.life() - 20.0 * balance::LINK_TAKEN).abs() < 1e-3,
        "the linked one took {}",
        whole - game.droids()[near].body.life()
    );
    assert!((whole - game.droids()[far].body.life() - 20.0).abs() < 1e-3);
    // The Conductor down, the link goes with it.
    let life = game.droids()[0].body.life_max();
    game.strike_droid(0, DroidPart::Chassis, life);
    game.simulate(DT);
    assert!(!game.droids()[near].rhythm.linked);
}

#[test]
fn a_conductor_s_mark_holds_every_machine_that_sees_it_to_one_body() {
    let mut game = machines_room();
    let at = middle() - vec2(4.0 * TILE, 0.0);
    planted(&mut game, DroidKind::Conductor, at);
    let trooper = planted(&mut game, DroidKind::Trooper, at + vec2(0.0, 2.0 * TILE));
    let pistol = WeaponKind::LaserPistol.basic();
    // The nearer Bim to the Conductor is marked; the Trooper stands nearer
    // the other.
    let marked = at + vec2(5.0 * TILE, -TILE);
    let other = at + vec2(4.0 * TILE, 4.0 * TILE);
    let targets = || vec![Some((other, pistol)), Some((marked, pistol))];
    let mut mark_at = None;
    let mut first_marked = None;
    let mut shots_at = Vec::new();
    for frame in 0..(60 * 9) {
        game.set_hostiles(targets());
        game.simulate(DT);
        if mark_at.is_none()
            && let Some(m) = game.droids()[0].rhythm.mark
        {
            mark_at = Some(frame);
            first_marked = Some(m.target);
        }
        let holding = game.droids()[0].rhythm.mark.is_some_and(|m| m.holding());
        for shot in game.take_shots() {
            if shot.shooter == Some(game.crew_count() as usize + trooper) && holding {
                shots_at.push(shot.at);
            }
        }
    }
    let mark_at = mark_at.expect("a mark was put on somebody");
    assert!(
        ((mark_at + 1) as f32 * DT - balance::MARK_FIRST).abs() < 0.05,
        "its first a moment after it saw them: frame {mark_at}"
    );
    assert_eq!(first_marked, Some(1), "the nearer of the two");
    assert!(!shots_at.is_empty(), "the Trooper fired while it held");
    assert!(
        shots_at.iter().all(|&s| (s - marked).len() < 1.0),
        "and only at the marked one: {shots_at:?}"
    );
}

#[test]
fn a_conductor_blinks_away_from_a_body_that_comes_near() {
    let mut game = machines_room();
    let at = middle();
    let i = machine(
        &mut game,
        Droid::new(DroidKind::Conductor, Tier::Two, 0, 1, at, 0.0, 9),
    );
    let pistol = WeaponKind::LaserPistol.basic();
    let bim = at + vec2(2.0 * TILE, 0.0);
    let mut planted_at = None;
    let mut gone_at = None;
    for frame in 0..(60 * 2) {
        game.set_hostiles(vec![Some((bim, pistol))]);
        game.simulate(DT);
        let d = &game.droids()[i];
        if planted_at.is_none() && d.rhythm.blink.is_some() {
            planted_at = Some(frame);
        }
        if gone_at.is_none() && (d.pos - at).len() > 2.0 * TILE {
            gone_at = Some(frame);
            break;
        }
    }
    let (planted_at, gone_at) = (
        planted_at.expect("it planted"),
        gone_at.expect("it blinked"),
    );
    let windup = (gone_at - planted_at) as f32 * DT;
    assert!(
        (windup - balance::BLINK_WINDUP).abs() < 0.05,
        "planted {windup} s first"
    );
    let d = &game.droids()[i];
    assert!(
        (d.pos - bim).len() > balance::BLINK_NEAR * TILE,
        "out of the body's reach: {:?}",
        d.pos
    );
    assert!(d.pos.x < at.x, "away from it: {:?}", d.pos);
    assert!(d.rhythm.blink_wait > balance::BLINK_COOLDOWN - 0.1);
}

#[test]
fn a_conductor_at_half_its_health_calls_four_bombs_round_its_mark_once() {
    let mut game = machines_room();
    let at = middle() - vec2(4.0 * TILE, 0.0);
    planted(&mut game, DroidKind::Conductor, at);
    let pistol = WeaponKind::LaserPistol.basic();
    let bim = at + vec2(6.0 * TILE, 0.0);
    for _ in 0..30 {
        game.set_hostiles(vec![Some((bim, pistol))]);
        game.simulate(DT);
    }
    assert!(!game.take_shots().iter().any(|s| s.bomb), "nothing whole");
    let half = game.droids()[0].body.life_max() * 0.55;
    game.strike_droid(0, DroidPart::Chassis, half);
    let mut bombs = Vec::new();
    for _ in 0..(60 * 3) {
        game.set_hostiles(vec![Some((bim, pistol))]);
        game.simulate(DT);
        bombs.extend(game.take_shots().into_iter().filter(|s| s.bomb));
    }
    assert_eq!(
        bombs.len(),
        balance::STRIKE_BOMBS as usize,
        "four, and once"
    );
    for b in &bombs {
        assert_eq!(b.from, b.at, "a strike's bomb only spins");
        let off = (b.at - bim).len() / TILE;
        assert!(
            (off - balance::STRIKE_SPREAD).abs() < 0.1,
            "round the body: {off} tiles"
        );
    }
}
