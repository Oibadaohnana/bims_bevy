# The probes

Notes on `scratchpad/` — the native probes and the layout dump. What they
test is in the crates' own `CLAUDE.md` files. The node harnesses that used
to live beside them (`*.mjs`, `stub.mjs`) went with the browser host in the
port to Bevy; what they checked about the screens is `./check smoke` and
`crates/app`'s tests now, and a screen is looked at by running it with
`BIMS_SMOKE_FRAMES` and `BIMS_SCREENSHOT` — see the root `CLAUDE.md`.

## What is here, since feature 104

Three probes, all on the room alone and linking nothing:

- **`crowding.rs`** — two Bims walking into each other on a bare room
  (`Game::bare`): twelve seeds of a head-on meeting that must all get past,
  and the same walk timed clear and through the other Bim, which must be a
  little slower and no more. It is what "the crew pass through each other"
  in `crates/game/CLAUDE.md` rests on.
- **`layout.rs`** — one frame of the real draw buffer as SVG, on a bare
  room; see "Looking at the room without a window" below.
- **`droidwreck.rs`** — the machines' own drawings as SVG on a bare ground,
  a row a kind and a column a state, with no room at all
  (`Droid::draw` wants nothing but a `DrawList`): `droidwreck > x.svg` is
  the rack, `droidwreck wrecks 12` the wrecks close up and twelve seconds
  cold.

Everything else that used to be here — `crew`, `cues`, `diary`, `neglect`,
`poison`, `priority`, `probe`, `social`, `spread`, `stew` and `sweep` — was
the needs, or a day in the classic room, and went with them in feature 104.
What the room does now is pinned by its own tests (`cargo test -p bims`,
on `Game::bare`) and by the survivor tests (`crates/world/src/tests_survivors.rs`,
`crates/ship/src/tests_survivors.rs`), which hold a whole run still where a
probe held a seeded day.

**Build one into `target/probes/`, never into `scratchpad/`**:
`rustc --edition 2024 -O scratchpad/crowding.rs -o target/probes/crowding`,
which is what `./check full`'s `probes` step does for every `*.rs` here but
`modules.rs` and `layout.rs` (a picture, not a test), each under
`PROBE_SECONDS` of its own. The `crowding` and `layout` binaries that are
still tracked in `scratchpad/` were built before feature 104 from the
classic room — the rule below is why they must never be run. And never pipe `rustc` into `head`: `head` closing
the pipe kills the compiler with SIGPIPE before it writes the binary, and
the "no such file" that follows looks like a compile error that is not
there.

## Probes must share one module list

The native probes in the scratchpad declare the crate's modules by `#[path]`.
When they each carried their own copy of that list, adding a new `crates/game/src/*.rs`
meant every probe but the newest failed to *compile* — and `rustc` failing
leaves the previous binary in place, so running it printed a confident pass
from stale code. Three separate rounds of that happened. They now
`include!("modules.rs")` from one shared file; keep it that way, and if you
ever see a probe pass that you expected to fail, check it actually rebuilt.

`modules.rs` lists every module of the room — `balance`, `bim`, `blood`,
`character`, `clock`, `combat`, `cue`, `door`, `draw`, `droid`, `fixtures`,
`fx`, `game`, `health`, `math`, `memory`, `nav`, `order`, `rng`, `room`,
`routine`, `sight`, `task`, `terrain` and `work` — and the shared `time`
crate stood over its own `lib.rs` as a plain module, which is why the room
reaches it as `crate::time`.

`crates/game/src/aboard.rs` is deliberately **not** on that list. It is the
one module of the room that names a `ShipDesign`, and the probes link no
other crate — which is why the rest of the room takes a `room::Layout` of
plain rects and `aboard.rs` is the only place that builds one from a
design. A second use of `shipdesign` anywhere else in `crates/game` breaks
every probe at once.

## A new `crates/game/src/*.rs` is a line in `modules.rs`, and the probes say so loudly

Feature 83 added `crates/game/src/droid.rs`, and `game.rs` gained a
`use crate::droid::…` with it. Every probe then failed to compile with
`unresolved import crate::droid` — the shared module list had no
`mod droid;` — and `./check probes` went red while every other step
stayed green, since `cargo` builds the crate by its own `lib.rs` and
never looks at `modules.rs`.

That is the list above working as designed: one file to edit, and one
failure rather than a stale binary printing a confident pass. But
**nothing in the crate reminds you**, so the rule is worth saying
plainly: a new module under `crates/game/src/` that anything already on
the list reaches is a `#[path]` line here in the same commit, and a module
deleted is a line taken out. Feature 104 did both at once: `blood.rs` and
`fixtures.rs` in, `needs.rs`, `social.rs`, `schedule.rs`, `galley.rs`,
`dish.rs`, `bath.rs`, `hydro.rs`, `manager.rs` and `filth.rs` out. The only
module deliberately left off is `aboard.rs`, for the reason above.

## A probe simulates; it does not draw

Every probe that measures steps the room with **`Game::simulate`**.
`Game::update` is `simulate` *plus* `render`, and a probe that steps the
clock for long calls it 86 400 times a simulated day — for a picture nobody
ever looks at. The step itself is about **two microseconds**; the picture is
about **five hundred**, nearly all of it `Sight::light_map`, which marches
the mask afresh for every body that has moved and composes it over every
pixel the move touched. So rendering was ninety-nine per cent of a probe's
run: the `./check probes` step went from ninety seconds to a hundred minutes
when the light map went in, and the longest of the needs probes went from
sixteen minutes to six seconds when it stopped drawing.

Nothing in the simulation reads what the picture works out — the mask's
`seen` is asked for by `Game::seen_at` and by the fog's own drawing and by
nothing else — which is why `Game::observe` is public: a probe that wants
`seen_at` calls it itself, a step at a time, for half a microsecond.
`layout.rs` is the one that still calls `update`, because dumping the draw
buffer is the whole of what it does.

`./check probes` runs each probe under `timeout PROBE_SECONDS` (120) for
the other half of the same lesson: a probe steps the room *until something
happens*, so a change that stops it happening is a probe that never
returns, and a check that hangs is a check nobody runs. It should fail.

## Looking at the room without a window

`scratchpad/layout.rs` dumps one frame of the real draw buffer as SVG and
takes an argument for which moment to catch: `start` for a couple of
seconds into a bare room with its two Bims, `down` for one of them out cold
with its gun dropped beside it, `dead` for one dead and one out cold side by
side — the two figures down against each other. `rsvg-convert` turns it
into a PNG. A bare room has no fixtures, so the furniture is no longer
looked at this way: the fixtures' pictures are the ship's, seen on the deck
and in the yard by running the app — `BIMS_SMOKE_FRAMES=200
BIMS_SCREENSHOT=/tmp/x.png ./hidden target/debug/bims simulation` (or
`design`), with `BIMS_POINTER` and `BIMS_KEYS` to get a screen into the
state worth seeing; the root `CLAUDE.md` has the shape of both — and pinned
bit for bit by `PICTURES` in `crates/ship/src/tests_survivors.rs`. That is
how a hull drawn mirrored or a starfield turning with the ship is caught.

## A probe that stages a meeting has to ask where a body fits

`spot_at(..) == SPOT_DECK` means "this is floor". It does **not** mean "a
body can stand here": the nav grid inflates every obstacle by `BODY_MARGIN`,
so the strip of deck along a wall or a counter front reads as floor and has
no route through it. A probe that picks a corridor that way gets `nav.path`
returning empty, `follow_path` leaves the Bim wandering, and the whole run
measures a wander while claiming to measure a walk.

`Game::put_for_probe` snaps through `nav.nearest_free` and `send_for_probe`
returns false when there was no route. Check it. And hold the *other* Bims
still first (below) — that takes away the wander but not the walk, so what
is left is only the thing under test.

## Never recruit slot 0 to hold a probe still

Recruiting used to mean no more than "stands where it is until told", and
three probes recruited the whole crew on that understanding. Feature 84 gave
the word a second meaning: recruiting the **player's own** Bim is what says
the player is *leading*. `Game::led` reads slot 0's recruitment, `led` musters
the bots, and a mustered bot with nothing else claiming it gathers round its
player (`bot_stand` → `gather`) — so the other Bim turns round and walks after
the one being measured. `separate_under_arms` is the other half: it shoves
every pair of **recruited** bodies apart to `CREW_CLEARANCE`, so two recruited
Bims can no longer touch at all.

`crowding.rs` staged a head-on meeting that way and measured neither: crew 1
followed crew 0 instead of walking at it, the shove kept them thirty-eight
units apart, and all twelve seeds read as *stuck* — "the crew pass through
each other" looked dead when it was only unstageable. Recruit `1..CREW`
(`recruit_for_probe`) and leave slot 0 alone: a Bim already marching follows
its route whether it is recruited or not, so slot 0 never needed it. The
reading came straight back to the 6.40s/6.73s this file has always quoted.

## Measuring a slowdown needs the other Bim to actually stay put

`stage_meeting` in `scratchpad/crowding.rs` hands *both* crew a route across the
room. Standing one of them somewhere else afterwards with `put_for_probe` does
**not** take that route away — it marches off and gets met either way, so the
"clear run" and the "walk through somebody" runs were the same scenario and came
out identical to the frame. Send it to where it stands as well, and the
comparison means something: 6.40s clear, 6.73s through the other.

## Staging a walk over a particular tile

A probe that walked a Bim through fouled deck to see the dirt move (`spread.rs`,
gone with the dirt in feature 104) found three things that make any probe
turning on *where* a Bim goes harder than it looks:

- **Ordering a Bim to `y` does not put it on that row.** Route smoothing, the
  push-out and the other Bim all nudge it, and it settles a whole tile from
  where it was sent. A band one tile deep across the lane was missed entirely
  — the walk crossed nothing but clean deck for twenty laps and the probe
  reported the feature dead. Stage three or four rows, or read the row back
  off `bim_pos` after it has settled.
- **`is_walking()` is false while it is still turning to face the route.** A
  lap loop shaped `if !is_walking { send_somewhere_else() }` hands the Bim a
  fresh route every frame: it never moves, and the lap counter climbs happily
  the whole time. Turn round on arrival, measured as a distance from the
  target, with a frame budget as the backstop.
- **A deck tile is `TILE` across and the grid is not laid from nought.** The
  blood's grid (`Blood::cell`, as `Filth`'s was) measures from the room
  interior's own corner, so two x values twenty units apart are usually the
  same cell and sometimes not. A comparison read a few units from the thing
  it was compared against read the thing itself. Read a comparison a
  **whole `TILE`** clear of it and assert the sample range is not empty, or
  the loop quietly reads nothing and the check passes on a default.

None of those failures says anything. The probe runs, the assertions are
checked, and the answer is simply wrong.

## Three seeds is not a sample

A needs probe once asked whether a Bim left alone for a fortnight gives up by
running three seeds for three days and requiring two of the three to die. At
three per cent an hour a three-day run ends in a give-up about five times in
six — so "at least two of three" is wrong roughly once in thirteen triples,
and the seeds it pinned happened to be such a triple. It had been red for as
long as anybody had looked, and the mechanic was never broken: a hundred seeds
gave 83 deaths in three days and 96 in five, which is what the arithmetic says.

The probe went with the needs; the lesson stays for any test that samples.
Work the margin out before writing the threshold. Sixteen seeds with half of
them asked for is three standard deviations; three seeds with two asked for
is a coin toss dressed up as an assertion. And print the reading beside the
verdict — `13 of 16` is a number the next change can be compared against, and
`FAIL` on its own is not.

## A priority is not measurable by watching who does what

The obvious probe for "this job before that one" is to stage both, run it, and
see which starts first. It measures nothing: with two crew and the jobs at
different places, whichever one cannot have the job at the top of the list
simply takes the one below it, and both begin on the same frame under every
arrangement of the numbers. `Game::work_on_offer` is split out from
`do_some_work` for this: it is the *choice*, with nothing in the way, and
`work_on_offer_for_probe` reads it. Whether a chosen job is then actually
begun depends on things that have nothing to do with the list. Assert on the
choice; assert separately that the work still all gets done.
