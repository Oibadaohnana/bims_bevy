// Dumps one frame of the real draw buffer as SVG, so the room can be looked at
// without a window. A bare room, two Bims in it, and whatever the first
// argument asks for: "start" for a couple of seconds in, "down" for one of
// them out cold with its gun dropped beside it, "dead" for one dead and one
// out cold side by side — the two figures down against each other.

include!("modules.rs");

use game::Game;

fn main() {
    let mut game = Game::bare(21, room::ROOM_W, room::ROOM_H);
    let want = std::env::args().nth(1).unwrap_or_else(|| "start".into());
    // Out cold on the deck, its gun dropped beside it: ten wounds on the
    // body and the blood run down until it drops. For looking at the
    // dropped weapon and the fallen figure together.
    let down = want == "down";
    if down {
        // Nobody dresses her first: James would, and she would never drop.
        game.set_autonomous(false);
        for _ in 0..10 {
            game.wound(1, health::Part::Body, 1.0);
        }
    }
    // Both figures down side by side: James dead and Kate out cold, for
    // looking at the two figures down against each other.
    let dead = want == "dead";
    if dead {
        game.set_autonomous(false);
        game.put_for_probe(0, math::vec2(380.0, 330.0));
        game.put_for_probe(1, math::vec2(560.0, 330.0));
        game.kill_for_probe(0);
        game.knock_out_for_probe(1);
    }
    let mut n = 0;
    loop {
        game.update(1.0 / 60.0);
        n += 1;
        if down {
            if game.is_unconscious(1) {
                break;
            }
        } else if dead {
            if game.is_unconscious(1) && n >= 30 {
                break;
            }
        } else if n >= 120 {
            break;
        }
        if n > 24 * 60 * 60 * 3 {
            eprintln!("never reached it");
            break;
        }
    }
    eprintln!("frame {n}");
    let shapes: Vec<f32> = game.shapes().to_vec();

    let stride = draw::STRIDE;
    println!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" width=\"{}\" height=\"{}\">",
        room::ROOM_W,
        room::ROOM_H,
        room::ROOM_W,
        room::ROOM_H
    );
    println!("<rect width=\"100%\" height=\"100%\" fill=\"#0c1210\"/>");
    for s in shapes.chunks(stride) {
        let (kind, x, y, w, h, rot, radius, line) =
            (s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]);
        let (r, g, b, a) = (
            (s[8] * 255.0).round(),
            (s[9] * 255.0).round(),
            (s[10] * 255.0).round(),
            s[11],
        );
        let paint = if line > 0.0 {
            format!("fill=\"none\" stroke=\"rgb({r},{g},{b})\" stroke-opacity=\"{a}\" stroke-width=\"{line}\"")
        } else {
            format!("fill=\"rgb({r},{g},{b})\" fill-opacity=\"{a}\"")
        };
        let turn = rot.to_degrees();
        if kind == 1.0 {
            println!(
                "<ellipse cx=\"0\" cy=\"0\" rx=\"{}\" ry=\"{}\" {paint} transform=\"translate({x} {y}) rotate({turn})\"/>",
                w.abs() / 2.0,
                h.abs() / 2.0
            );
        } else {
            println!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{radius}\" {paint} transform=\"translate({x} {y}) rotate({turn})\"/>",
                -w / 2.0,
                -h / 2.0,
                w.abs(),
                h.abs()
            );
        }
    }
    // The names, which the host draws rather than the buffer.
    for (who, name) in ["James", "Kate"].iter().enumerate() {
        let p = game.bim_pos(who);
        println!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-family=\"sans-serif\" font-size=\"13\" font-weight=\"600\" fill=\"{}\" stroke=\"rgba(6,10,9,0.85)\" stroke-width=\"3\" paint-order=\"stroke\">{name}</text>",
            p.x,
            p.y - 46.0,
            if who == 0 { "#7fd1a8" } else { "#e7efe9" }
        );
    }
    println!("</svg>");
}
