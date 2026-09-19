// What the room says it sounds like: does a stew for the store come with
// the knife strokes that cut it, and does a Bim going to the heads come
// with the door sliding open and shut?
//
// The cues are the room's word to the app about what to play — see
// `crates/game/src/cue.rs`. What is checked here is that the room says
// them at all, the right number of times, and says nothing when nothing
// is happening; what they sound like is `crates/app/src/sound.rs`'s.

include!("modules.rs");

use cue::Cue;
use game::Game;
use manager::Stock;

const STEP: f32 = 1.0 / 60.0;
const FRAMES_PER_HOUR: u32 = 60 * 60;

fn main() {
    let mut fails = 0;
    macro_rules! check {
        ($what:expr, $ok:expr, $extra:expr) => {
            if $ok { println!("  ok   {}", $what); }
            else { println!("  FAIL {} — {}", $what, $extra); fails += 1; }
        };
        ($what:expr, $ok:expr) => { check!($what, $ok, "") };
    }

    // --- a quiet room says nothing ----------------------------------------

    let mut game = Game::new(3, 960.0, 640.0);
    let mut early = Vec::new();
    for _ in 0..60 {
        game.update(STEP);
        early.extend(game.take_cues());
    }
    check!(
        "the first second of a day is silent",
        early.is_empty(),
        format!("{early:?}")
    );

    // --- a stew is chopped --------------------------------------------------

    game.set_target(Stock::Stew, 1);
    let mut cues = Vec::new();
    let mut frames = 0;
    for _ in 0..(6 * FRAMES_PER_HOUR) {
        game.update(STEP);
        frames += 1;
        cues.extend(game.take_cues());
        if game.store_stew() >= 1 {
            break;
        }
    }
    check!("a stew reaches the shelf", game.store_stew() >= 1);
    let chops = cues.iter().filter(|c| c.cue == Cue::Chop).count();
    // A stew is two things through the knife, five strokes each
    // (`task::CHOPS`); a meal somebody wanted meanwhile is more, never fewer.
    check!(
        "and the knife was heard ten times, or a meal's worth more",
        chops >= 10 && chops % 5 == 0,
        format!("{chops} strokes in {frames} frames")
    );
    let board = game.board_pos_for_probe(0);
    check!(
        "every stroke on the board",
        cues.iter()
            .filter(|c| c.cue == Cue::Chop)
            .all(|c| (c.at - board).len() < 1.0),
        format!("{:?} vs board {:?}", cues.iter().find(|c| c.cue == Cue::Chop).map(|c| c.at), board)
    );
    check!(
        "and no shot, no blow, no bolt in a room with nobody to fight",
        !cues.iter().any(|c| matches!(
            c.cue,
            Cue::Shot { .. } | Cue::Impact { .. } | Cue::Ricochet | Cue::Blow { .. }
        ))
    );

    // --- the heads' door slides ---------------------------------------------

    // Through the rest of the day: somebody goes to the heads, the door
    // opens for them and shuts behind them, and opens and shuts again on
    // the way out. Each opening is one cue, each shutting one, and never
    // two openings in a row.
    let mut cues = Vec::new();
    for _ in 0..(18 * FRAMES_PER_HOUR) {
        game.update(STEP);
        cues.extend(game.take_cues());
    }
    let doors: Vec<Cue> = cues
        .iter()
        .filter(|c| matches!(c.cue, Cue::DoorOpens | Cue::DoorShuts))
        .map(|c| c.cue)
        .collect();
    check!(
        "the heads' door is heard in a day",
        doors.len() >= 2,
        format!("{doors:?}")
    );
    check!(
        "opening first",
        doors.first() == Some(&Cue::DoorOpens),
        format!("{doors:?}")
    );
    let alternates = doors.windows(2).all(|w| w[0] != w[1]);
    check!(
        "and opening and shutting turn and turn about",
        alternates,
        format!("{doors:?}")
    );
    let door = game.bath_door_for_probe();
    check!(
        "every one at the door",
        cues.iter()
            .filter(|c| matches!(c.cue, Cue::DoorOpens | Cue::DoorShuts))
            .all(|c| (c.at - door).len() < 1.0)
    );

    if fails > 0 {
        println!("{fails} FAILED");
        std::process::exit(1);
    }
    println!("all ok");
}
