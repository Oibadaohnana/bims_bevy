// Dumps the machines' own drawings as SVG on a bare ground, a row a kind and a
// column a state, so a wreck can be looked at without a window (feature 83).
// The rack `World::stage_droids_for_probe` lays out for `BIMS_DROIDS=1`, with
// no room under it: `Droid::draw` wants nothing but a `DrawList`.
//
//     ./target/probes/droidwreck > /tmp/droids.svg
//
// An argument is how many seconds old the wrecks are, for looking at the ember
// dying away: `droidwreck 12` is a rack of cold ones.

include!("modules.rs");

use droid::{Droid, DroidKind, DroidPart};

const ACROSS: f32 = 150.0;
const DOWN: f32 = 170.0;

fn main() {
    // `droidwreck wrecks [age]` is the last column alone, spread out and
    // a row of three, for looking at the wrecks close up.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let only_wrecks = args.first().is_some_and(|a| a == "wrecks");
    let age: f32 = args
        .iter()
        .find_map(|a| a.parse().ok())
        .unwrap_or(1.0);
    let states: Vec<u32> = if only_wrecks { vec![4] } else { (0..5).collect() };
    let (across, down) = if only_wrecks { (0.0, 260.0) } else { (ACROSS, DOWN) };
    let w = 100.0 + across * states.len() as f32 + if only_wrecks { 160.0 } else { 100.0 };
    let h = 90.0 + down * 3.0;

    let mut list = draw::DrawList::new();
    for (row, kind) in DroidKind::ALL.into_iter().enumerate() {
        for (col, &state) in states.iter().enumerate() {
            let at = math::vec2(100.0 + across * col as f32, 90.0 + down * row as f32);
            let mut d = Droid::new(kind, combat::Tier::One, 0, 1, at, 0.0, row as u64 * 16 + 5);
            match state {
                // Idle: as it comes.
                0 => {}
                // Firing, or a Husk striking: held at the instant.
                1 => d.lit = true,
                // The arms gone, then the legs.
                2 => {
                    d.strike(DroidPart::Arms, d.body.max(DroidPart::Arms));
                }
                3 => {
                    d.strike(DroidPart::Legs, d.body.max(DroidPart::Legs));
                }
                // Destroyed: the wreck.
                _ => d.destroy(),
            }
            // Aged so the sparks of the hit have died away and the wreck
            // is the picture rather than the instant it became one.
            let mut left = age;
            while left > 0.0 {
                d.walk(1.0 / 60.0);
                left -= 1.0 / 60.0;
            }
            d.draw(&mut list);
        }
    }

    let stride = draw::STRIDE;
    println!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w} {h}\" width=\"{w}\" height=\"{h}\">"
    );
    println!("<rect width=\"100%\" height=\"100%\" fill=\"#1b2026\"/>");
    for s in list.data().chunks(stride) {
        let (kind, x, y, w, h, rot, radius, line) =
            (s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]);
        let (r, g, b, a) = (
            (s[8] * 255.0).round(),
            (s[9] * 255.0).round(),
            (s[10] * 255.0).round(),
            s[11],
        );
        let paint = if line > 0.0 {
            format!(
                "fill=\"none\" stroke=\"rgb({r},{g},{b})\" stroke-opacity=\"{a}\" stroke-width=\"{line}\""
            )
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
    println!("</svg>");
}
