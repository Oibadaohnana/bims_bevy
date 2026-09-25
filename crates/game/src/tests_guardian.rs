//! The Guardian (feature 100), as far as the room is concerned: its
//! shield, which stops a bolt or a blow from the front and nothing from
//! anywhere else, its turn, which is whole sub-steps at no more than its
//! rate and held through a wind-up, and what its legs and its arms do.
//! The wave it comes in and the seam its shield is handed across are the
//! world's (`crates/world/src/tests_guardian.rs`).

use crate::balance;
use crate::combat::{Combat, Tier, WeaponKind, shield_stops};
use crate::droid::{Beam, Droid, DroidKind, DroidPart};
use crate::game::Game;
use crate::math::{Rect, Vec2, vec2};
use crate::room::{ROOM_H, ROOM_W, TILE};
use crate::sight::Sight;

const DT: f32 = 1.0 / 60.0;

/// A hall thirty tiles square with nothing in it, for bolts to fly in.
fn open_sight() -> Sight {
    let interior = Rect::from_min_size(Vec2::ZERO, vec2(30.0 * TILE, 30.0 * TILE));
    Sight::new(interior, interior, TILE, &[], &[])
}

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

/// A Guardian at `at` facing `dir`, at tier three as it only ever comes.
fn guardian_at(at: Vec2, dir: Vec2) -> Droid {
    Droid::new(DroidKind::Guardian, Tier::Three, 0, 1, at, 0.0, 11).with_facing(dir)
}

/// How far apart two unit vectors point, in degrees — the test's own
/// trigonometry, never the machine's.
fn degrees_between(a: Vec2, b: Vec2) -> f32 {
    a.dot(b).clamp(-1.0, 1.0).acos().to_degrees()
}

#[test]
fn the_shield_s_rule_is_a_dot_product_and_its_edge_is_the_front() {
    let heading = vec2(1.0, 0.0);
    assert!(shield_stops(heading, vec2(1.0, 0.0)), "dead ahead");
    // Exactly 60° off the heading: cos 60° is a half to the bit, and the
    // edge is stopped.
    let edge = vec2(0.5, 0.75f32.sqrt());
    assert_eq!(heading.dot(edge), balance::GUARDIAN_SHIELD_COS);
    assert!(shield_stops(heading, edge), "the edge itself is the front");
    // A hair outside it is not.
    assert!(!shield_stops(heading, vec2(0.4999, 0.866_08)));
    assert!(!shield_stops(heading, vec2(0.0, 1.0)), "the flank");
    assert!(!shield_stops(heading, vec2(-1.0, 0.0)), "the back");
}

#[test]
fn a_bolt_from_the_front_is_stopped_at_the_plate_and_from_outside_it_lands() {
    let sight = open_sight();
    let centre = vec2(15.0 * TILE, 15.0 * TILE);
    let heading = vec2(1.0, 0.0);
    let sniper = WeaponKind::SniperRifle.basic();
    // A shooter six tiles off, at so many degrees round from the heading.
    let hits_from = |degrees: f32| -> usize {
        let dir = Vec2::from_angle(degrees.to_radians());
        let from = centre + dir * (6.0 * TILE);
        let mut combat = Combat::new(5);
        combat.set_targets(vec![Some((centre, WeaponKind::Sweeper.at(Tier::Three)))]);
        combat.set_shields(&[Some(heading)]);
        let mut hits = 0;
        for _ in 0..40 {
            combat.fire(from, centre, sniper, false, false);
            for _ in 0..60 {
                combat.step(DT, &sight, &[]);
            }
            hits += combat.take_hits().len();
        }
        hits
    };
    // Inside the ±60° arc — dead ahead, and at fifty-five degrees either
    // side — nothing lands, however true the aim.
    for degrees in [0.0, 55.0, -55.0] {
        assert_eq!(hits_from(degrees), 0, "{degrees}° off the heading");
    }
    // Outside it the bolts land as bolts do: a sniper rifle at six tiles
    // hits nine in ten.
    for degrees in [65.0, -65.0, 90.0, 180.0] {
        let hits = hits_from(degrees);
        assert!(hits > 25, "{degrees}° off the heading: {hits} of 40");
    }
}

#[test]
fn a_blow_from_the_front_is_stopped_and_one_from_behind_lands() {
    let centre = vec2(15.0 * TILE, 15.0 * TILE);
    let heading = vec2(1.0, 0.0);
    let blade = WeaponKind::Schword.basic();
    for (from, lands) in [
        (centre + vec2(0.8 * TILE, 0.0), false),
        (centre + vec2(-0.8 * TILE, 0.0), true),
        (centre + vec2(0.0, 0.8 * TILE), true),
    ] {
        let mut combat = Combat::new(9);
        combat.set_targets(vec![Some((centre, WeaponKind::Sweeper.basic()))]);
        combat.set_shields(&[Some(heading)]);
        combat.brawl(from, 0, blade, 42.0, true, false, Some(0));
        assert_eq!(
            combat.take_hits().len(),
            lands as usize,
            "a blow from {:?} off the middle",
            from - centre
        );
    }
}

#[test]
fn a_grenade_bursting_before_the_shield_still_reaches_the_body() {
    let mut game = Game::bare(4, ROOM_W, ROOM_H);
    game.set_autonomous(false);
    let thrower = game.bim_pos(0);
    // The machine three tiles off, its shield square on to the thrower.
    let at = thrower + vec2(3.0 * TILE, 0.0);
    let facing = (thrower - at).normalize_or_zero();
    let target = Some((at, WeaponKind::Sweeper.at(Tier::Three)));
    game.set_hostiles(vec![target]);
    game.set_hostiles_shields(&[Some(facing)]);
    game.throw_grenade(0, at, 1.0, 2.0 * TILE, 40.0);
    let mut blast = false;
    for _ in 0..(60 * 4) {
        game.set_hostiles(vec![target]);
        game.set_hostiles_shields(&[Some(facing)]);
        game.simulate(DT);
        blast |= game.take_hits().iter().any(|h| h.who == 0 && h.blast);
    }
    assert!(
        blast,
        "the burst is not a bolt, and the shield does not stop it"
    );
}

#[test]
fn a_guardian_turns_no_faster_than_its_rate_and_ends_facing_exactly() {
    for step in [DT, 0.25] {
        let mut d = guardian_at(Vec2::ZERO, vec2(1.0, 0.0));
        let want = vec2(-1.0, 0.2).normalize_or_zero();
        let start = d.facing();
        let mut t = 0.0;
        while t < 1.0 - 1e-4 {
            d.turn_toward(want, step);
            t += step;
        }
        let turned = degrees_between(start, d.facing());
        // A second is seventy-five degrees, give or take the one sub-step
        // a step's time was short of or owed.
        assert!(
            turned <= balance::GUARDIAN_TURN_DEGREES + 1.3,
            "{turned}° in a second at steps of {step}"
        );
        assert!(turned >= balance::GUARDIAN_TURN_DEGREES - 1.3, "{turned}°");
        // It keeps its length: a unit vector, turned by arithmetic alone.
        assert!((d.facing().len() - 1.0).abs() < 1e-4);
        // And two more seconds bring it round, onto the want exactly.
        for _ in 0..(2.0 / step) as usize {
            d.turn_toward(want, step);
        }
        assert_eq!(d.facing(), want, "faces it exactly once within a sub-step");
    }
    // Any other kind is never turned so; nor is a wreck.
    let mut wreck = guardian_at(Vec2::ZERO, vec2(1.0, 0.0));
    wreck.destroy();
    wreck.turn_toward(vec2(0.0, 1.0), 1.0);
    assert_eq!(wreck.facing(), vec2(1.0, 0.0));
    assert_eq!(wreck.shield(), None, "a wreck has no shield");
}

#[test]
fn a_guardian_winds_up_on_what_it_sees_and_holds_its_heading_to_the_end() {
    let mut game = machines_room();
    let at = vec2(ROOM_W * 0.5, ROOM_H * 0.5);
    game.add_droid(guardian_at(at, vec2(1.0, 0.0)));
    // Its target up the room, a quarter turn off where it faces.
    let first = at + vec2(0.0, -3.0 * TILE);
    let pistol = WeaponKind::LaserPistol.basic();
    let mut wound = None;
    for frame in 0..(60 * 4) {
        game.set_hostiles(vec![Some((first, pistol))]);
        game.simulate(DT);
        if game.droids()[0].beam.holds_heading() {
            wound = Some(frame);
            break;
        }
    }
    let wound = wound.expect("it winds up once it faces its target");
    // A quarter turn less the wind-up's fifteen degrees at 75° a second
    // is the better part of a second.
    assert!(wound >= 55, "wound up at frame {wound}");
    assert!(degrees_between(game.droids()[0].facing(), (first - at).normalize_or_zero()) <= 15.5);

    // **Held through it**: the target runs round behind the machine and
    // its heading does not move until the wind-up is let go.
    let held = game.droids()[0].facing();
    let second = at + vec2(-3.0 * TILE, 0.0);
    let mut shots = 0;
    let mut frames = 0;
    while game.droids()[0].beam.holds_heading() {
        game.set_hostiles(vec![Some((second, pistol))]);
        game.simulate(DT);
        shots += game.take_shots().len();
        frames += 1;
        if game.droids()[0].beam.holds_heading() {
            assert_eq!(game.droids()[0].facing(), held, "held at frame {frames}");
        }
        assert!(frames < 200, "the wind-up ends");
    }
    assert!(
        (frames as f32 * DT - balance::SWEEPER_WINDUP).abs() < 0.05,
        "the wind-up is {} s",
        frames as f32 * DT
    );
    assert!(shots > 0, "and at its end the Sweeper is let go");
    assert!(matches!(game.droids()[0].beam, Beam::Cooling { .. }));
    // After it, it turns again after the target that went round it.
    for _ in 0..(60 * 3) {
        game.set_hostiles(vec![Some((second, pistol))]);
        game.simulate(DT);
    }
    let now = &game.droids()[0];
    assert!(
        degrees_between(now.facing(), (second - now.pos).normalize_or_zero()) < 20.0,
        "it came round after it"
    );
}

#[test]
fn a_taunt_turns_a_guardian_from_the_nearer_target() {
    let pistol = WeaponKind::LaserPistol.basic();
    for taunting in [false, true] {
        let mut game = machines_room();
        let at = vec2(ROOM_W * 0.5, ROOM_H * 0.5);
        game.add_droid(guardian_at(at, vec2(0.0, 1.0)));
        let near = at + vec2(0.0, -3.0 * TILE);
        let far = at + vec2(-6.0 * TILE, 0.0);
        for _ in 0..(60 * 3) {
            game.set_hostiles(vec![Some((near, pistol)), Some((far, pistol))]);
            let radius = if taunting { 20.0 * TILE } else { 0.0 };
            game.set_hostiles_taunting(&[0.0, radius], &[false, false]);
            game.simulate(DT);
        }
        let d = &game.droids()[0];
        let to = |p: Vec2| (p - d.pos).normalize_or_zero();
        let (want, other) = if taunting { (far, near) } else { (near, far) };
        assert!(
            d.facing().dot(to(want)) > d.facing().dot(to(other)),
            "taunting {taunting}: it faces the one it should"
        );
        assert!(degrees_between(d.facing(), to(want)) < 20.0);
    }
}

#[test]
fn with_its_legs_gone_a_guardian_stands_but_still_turns_and_fires() {
    let mut game = machines_room();
    let at = vec2(ROOM_W * 0.5, ROOM_H * 0.5);
    let i = game.add_droid(guardian_at(at, vec2(1.0, 0.0)));
    let legs = game.droids()[0].body.max(DroidPart::Legs);
    game.strike_droid(0, DroidPart::Legs, legs);
    assert_eq!(i, game.crew_count() as usize);
    assert!(!game.droids()[0].can_move());
    let target = at + vec2(-5.0 * TILE, 0.5 * TILE);
    let pistol = WeaponKind::LaserPistol.basic();
    let mut shots = 0;
    for _ in 0..(60 * 6) {
        game.set_hostiles(vec![Some((target, pistol))]);
        game.simulate(DT);
        shots += game.take_shots().len();
    }
    let d = &game.droids()[0];
    assert_eq!(d.pos, at, "it has not walked a step");
    assert!(
        degrees_between(d.facing(), (target - at).normalize_or_zero()) < 16.0,
        "it turned round to its target"
    );
    assert!(shots > 0, "and it fired where it stands");
}

#[test]
fn arms_gone_halve_the_sweeper_s_damage_and_not_its_reach() {
    let mut d = guardian_at(Vec2::ZERO, vec2(1.0, 0.0));
    let whole = d.stats();
    assert_eq!(d.weapon.kind, WeaponKind::Sweeper);
    d.strike(DroidPart::Arms, d.body.max(DroidPart::Arms));
    let hurt = d.stats();
    assert!((hurt.damage - whole.damage * balance::DROID_ARMS_DAMAGE).abs() < 1e-4);
    assert_eq!(
        hurt.accuracy, whole.accuracy,
        "a beam rolls no odds to lose"
    );
    assert_eq!(hurt.range, whole.range);
    // Thirty at tier one, scaled by the tier like any weapon.
    assert_eq!(WeaponKind::Sweeper.basic().stats().damage, 30.0);
    let three = WeaponKind::Sweeper.at(Tier::Three).stats();
    assert!(
        (three.damage - 30.0 * balance::TIER_TWO_DAMAGE * balance::TIER_THREE_DAMAGE).abs() < 1e-3
    );
}
