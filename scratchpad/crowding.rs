// Two Bims walking into each other.
//
// They do not collide: they pass straight through and both slow down. That is
// the one resolution that cannot leave anybody stuck, and being stuck is the
// real hazard here — a route is planned once and never replanned, so a Bim
// whose line goes through another has no second plan to fall back on. This
// probe stages the meeting deliberately rather than waiting for one, and
// checks both that they get past and that it costs them.

include!("modules.rs");

use bim::CREW;
use game::Game;
use math::{Vec2, vec2};
use room::{ROOM_H, ROOM_W};

const STEP: f32 = 1.0 / 60.0;

/// Stop the crew wandering off a staged walk — **every one but the player's
/// own**, which is the whole of the difference feature 84 made here.
///
/// Recruiting used to mean no more than "stands still until told". It now
/// means the player is *leading*: `Game::led` is true the moment slot 0 is
/// recruited, which musters the bots, and a mustered bot with no other claim
/// on it gathers round its player (`bot_stand` → `gather`). Recruiting both
/// therefore turned a head-on meeting into crew 1 turning round and
/// following crew 0 across the room — and `separate_under_arms` shoves every
/// pair of *recruited* bodies apart, so the two never touched either. The
/// probe measured neither passing nor crowding and every seed read as stuck.
///
/// Slot 0 does not need it: it is marching the whole way, and a Bim under
/// way follows its route whether it is recruited or not.
fn hold_the_others_still(game: &mut Game) {
    for w in 1..CREW {
        game.recruit_for_probe(w, true);
    }
}

fn main() {
    let mut fails = 0;
    macro_rules! check {
        ($what:expr, $ok:expr, $extra:expr) => {
            if $ok { println!("  ok   {}", $what); }
            else { println!("  FAIL {} — {}", $what, $extra); fails += 1; }
        };
        ($what:expr, $ok:expr) => { check!($what, $ok, "") };
    }

    // --- head-on, over and over -------------------------------------------

    let mut worst = 0.0f32;
    let mut stuck = 0;
    let mut ever_overlapped = false;
    for seed in 1..=12u64 {
        let mut game = Game::bare(seed, ROOM_W, ROOM_H);
        game.set_autonomous(false);
        hold_the_others_still(&mut game);
        let (left, right) = stage_meeting(&mut game);

        let mut arrived = [false; CREW];
        let mut frames = 0;
        for _ in 0..(20 * 60 * 60) {
            game.simulate(STEP);
            frames += 1;
            if (game.bim_pos(1) - game.bim_pos(0)).len() < 20.0 {
                ever_overlapped = true;
            }
            arrived[0] |= (game.bim_pos(0) - right).len() < 40.0;
            arrived[1] |= (game.bim_pos(1) - left).len() < 40.0;
            if arrived[0] && arrived[1] {
                break;
            }
        }
        let secs = frames as f32 * STEP;
        if !(arrived[0] && arrived[1]) {
            stuck += 1;
            println!("       seed {seed}: STUCK after {secs:.0}s");
        }
        worst = worst.max(secs);
    }
    check!("both get past each other, every time", stuck == 0, format!("{stuck} of 12 stuck"));
    check!("and they really do pass through", ever_overlapped);
    println!("       slowest crossing: {worst:.0}s");

    // --- and it costs them ------------------------------------------------
    //
    // The same walk twice: once with the other Bim parked out of the way, and
    // once with it standing in the middle of the route. The second has to take
    // longer, and by about what the slowdown says.

    let alone = walk_time(false);
    let through = walk_time(true);
    println!("       clear run {alone:.2}s, through the other {through:.2}s");
    check!("walking through somebody is slower", through > alone + 0.05, format!("{alone} vs {through}"));
    // Squeezing past costs 30% of the pace over the stretch where they are
    // within a body's width of each other, which is a small part of a long
    // walk — so the whole walk is a little longer, not a third longer.
    check!(
        "but only over the stretch where they touch",
        through < alone * 1.5,
        format!("{alone} vs {through}")
    );

    println!();
    if fails > 0 { println!("{fails} FAILED"); std::process::exit(1); }
    println!("all passed");
}

/// How long crew 0 takes to walk one clear run of deck, with the other either
/// parked well out of the way or standing squarely in the middle of it.
fn walk_time(in_the_way: bool) -> f32 {
    let mut game = Game::bare(21, ROOM_W, ROOM_H);
    game.set_autonomous(false);
    hold_the_others_still(&mut game);
    let (left, right) = stage_meeting(&mut game);
    // Crew 0 walks the run; crew 1 either stands in the middle of it or well
    // off it, where it cannot be brushed past.
    //
    // Note the second `send_for_probe` on crew 1. `stage_meeting` left it with
    // a route across the room, and standing it somewhere new does not take
    // that away — it would march off and be met either way, which made both
    // runs the same scenario and both times identical to the frame.
    game.put_for_probe(0, left);
    let parked = if in_the_way {
        (left + right) * 0.5
    } else {
        vec2(right.x, right.y + 200.0)
    };
    let parked = game.put_for_probe(1, parked);
    game.send_for_probe(1, parked);
    game.send_for_probe(0, right);

    for f in 0..(60 * 60) {
        game.simulate(STEP);
        if (game.bim_pos(0) - right).len() < 12.0 {
            return f as f32 * STEP;
        }
    }
    f32::MAX
}

/// Two spots at opposite ends of one clear run of deck that a body can walk,
/// with a route between them. Found by asking rather than written down: the
/// nav grid inflates every obstacle, so "deck" and "walkable" are different
/// questions and a hard-coded corridor goes stale when the furniture moves.
fn stage_meeting(game: &mut Game) -> (Vec2, Vec2) {
    for y in (60..540).step_by(4) {
        let (left, right) = (vec2(150.0, y as f32), vec2(700.0, y as f32));
        let a = game.put_for_probe(0, left);
        let b = game.put_for_probe(1, right);
        if (b.x - a.x) < 380.0 || (b.y - a.y).abs() > 6.0 {
            continue;
        }
        if game.send_for_probe(0, b) && game.send_for_probe(1, a) {
            return (a, b);
        }
    }
    panic!("nowhere in the room for two Bims to walk into each other");
}
