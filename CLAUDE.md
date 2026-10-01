# Working on Bims

A 2D top-down co-op roguelike shooter — humans against the machines, one
ship taken from site to site. All the simulation is Rust; `crates/app` is
the one Bevy binary. `README.md` explains the game; this file is how to
work on it. The long version of this file (every feature's history and
measurements) is `git show 5f9da69:CLAUDE.md`; each commit message says
what its task moved.

## Before anything: `AGENTS`, and after: commit

`AGENTS` at the root holds one number: how many agents are working on the
tree. Read it, add one, write it back — it is awareness only (re-read a
file before editing; keep out of others' way). At the end of the session:

1. write the number back one lower;
2. stop every background task you started (`pgrep -af "sleep "`); leave
   other agents' processes alone (`agents.sh status` lists them);
3. **commit your own work**: `git add` the files *you* touched, one by one
   (never `-A`, `-a` or `.`), message led by the task number the player
   gave you — `86. medics carry the wounded off the deck`. Commit even if
   unfinished, and say so. No `stash`, `checkout`, `reset`, `rebase` or
   push. The player may have committed the tree mid-task; check `git log`.

## Running it

Build and run **inside `nix-shell shell.nix`** (or `nix develop`): the app
links ALSA and opens Vulkan at run time, so outside the shell it fails
with a misleading error. `./check` and `./hidden` re-enter the shell
themselves. The rules crates alone (`cargo test -p world`) build anywhere.

`cargo run -- <command>` builds the working tree (the app is the default
member); `nix run .#<command>` builds the **git tree**. `cargo run -- list`
prints every command with a line each — that is the authoritative list.
The main ones:

| command | opens |
| --- | --- |
| `game` | the whole run: menu, lobby, then the first mission |
| `simulation` | straight into the world on the playtest ship |
| `design` | the ship designer (yard) |
| `test` / `test_planet` | a random station / planet settlement, combat ship |
| `droids` | the fight: 16 crew vs droid waves in the arena |
| `combat_droids_<class>` | `droids` with that class at level 16, every skill point unspent (`BIMS_RANKS=q,c,e,r` buys ranks) |
| `tier2_test`, `tier3_test` | `droids` with everything at that tier |
| `droids_planet`, `defense` | a town held by / attacked by the machines |
| `crisis`, `jammer`, `guardian`, `relics`, `heart`, `manufacturers` | one mechanic each, staged |
| `end` | the Heart with company: a lobby at code `THEEND`, Start once a second player joins, ten plain classless bots, tier-three kit, the setup's difficulty on day 60 (`BIMS_END_DAY`), the first mission not eased; `end offline` is it alone from the game setup, no relay |
| `stationbuilder [name]` | a sketch tool for station layouts |

`BUILD` at the root is the build number shown in every window's top left
corner and the nix packages' version; `ship` (`~/nixcfg/scripts/server-ship.sh`)
moves it up 0.1 each time it pushes the repo to the server — leave it be.
`bims --self-check` checks the pinned constants. Esc → Restart puts any
run back to its start.

### Running with nobody at the keyboard

Always through **`./hidden`** (a headless weston; never bare on the
desktop):

    BIMS_SMOKE_FRAMES=60 BIMS_SCREENSHOT=shot.png ./hidden target/debug/bims simulation

- `BIMS_SMOKE_FRAMES=n` runs `n` frames and exits; `BIMS_SCREENSHOT` saves
  the frame thirty before the end; `BIMS_WINDOW=1400x900` sizes it.
- `BIMS_POINTER="40:move:600,250;42:click:600,250;90:right:300,400"`
  (also `press`/`release`, `wheel`/`wheelup`) and `BIMS_KEYS="60:Escape,90:M"`
  (`+Shift`/`-Shift` to hold) drive input at those frames, in points at
  100% UI scale. **Move a frame before clicking** — egui hit-tests against
  the previous frame's layout.
- A smoke run is silent and opens at 100% UI scale.
- `BIMS_PERF=1 BIMS_SMOKE_FREE=1` prints where a frame went (release
  build, idle machine — other agents' builds skew it).
- Many more dials (`BIMS_CLASS`, `BIMS_RANKS`, `BIMS_DROID_WAVE`,
  `BIMS_FREEZE`, `BIMS_ZOOM`, `BIMS_MAP`, `BIMS_ARMORY`, `BIMS_STATION_SEED`,
  …) are documented at the top of `crates/app/src/dev.rs` and in
  `crates/ship/src/session.rs`; grep for `BIMS_`.
- `scaling.ron` at the root is the wave formula's dials (base, per
  player, per time step, the step's days, waves a station, the first
  mission's ease, and `early_ease` machines fewer a wave for the first
  `early_days` days; `world::droid::WaveScaling`, `BIMS_SCALING=file` names
  another) and `rewards.ron` what a fight pays and things cost (xp and
  money an enemy down, a defence's share, the buyback, relic, combine
  and shelf prices; `world::rewards::Rewards`, `BIMS_REWARDS`), both
  read again whenever saved while the game runs
  (`crates/app/src/wavecfg.rs`). Keep `rewards.ron` at the constants —
  its test says so. `scaling.ron` is the player's: the game setup's
  Difficulty shows its base, per player and per step, and *Save as
  default* writes them into it (only it has to parse). A smoke run that
  presses that button wants `BIMS_SCALING` pointed at a scratch copy.
  `audio.ron` (`BIMS_AUDIO`) is the player's volume for each sound, a
  multiple of `sound.rs`'s level — one a clip, plus the weapons that
  borrow a clip; also read again when saved. A new `Clip` wants its line
  in `sound.rs`'s `volumes!` and in the file (its test says so).
  `weapons.ron` (`BIMS_WEAPONS`) is every weapon's damage, `(near,
  far)` at tier one (`bims::balance::WeaponDamage`), armed for every
  room in the process (`WeaponKind::stats` reads it) and read again
  when saved; neither saved nor hashed, so two players' files have to
  agree. Keep it at the constants — its test says so; tune the
  constants in `balance.rs` for good.
- Two-player runs: a relay (`PORT=18792 target/debug/bims-server`),
  `BIMS_SERVER=ws://127.0.0.1:18792`, `BIMS_AUTO=create` on the host and
  `BIMS_AUTO=join:<code>` on the guest; `scratchpad/duo_resync.sh` is a
  working pair.

## Verifying a change

**Test your own change, not the whole game.** Run the crates you edited
(`cargo test -p <crate>`), `cargo fmt`, `cargo clippy -p app --no-deps`
(warnings are errors, app only) if you touched the app, and — for
anything on screen — one smoke run through `./hidden` that shows it.
**Do not run `./check` over the tree** unless your task says so: several
agents share it, and a red step in a file you never opened is theirs. The
player launches a test-all agent for the sweep. Say in your report which
crates you ran.

- A bare `cargo test` is the app alone (`default-members`); use `-p` or
  `--workspace`.
- `./check` (quick) skips the `world` and `ship` tests for time;
  `./check full` runs them with `BIMS_SWEEP=1`. `./check <step>` runs one
  step, `./check --list` explains them.
- `cargo build` proves nothing about the window — look at a screenshot.

### Pinned numbers

These must come out identical everywhere and are pinned against
constants; a change that moves one says why in the constant's note:

- `design_hash` (`shipdesign::fixture`) and `world_checksum` /
  `REFERENCE_CHECKSUM` (`world::fixture`);
- `SURVIVORS` (`crates/world/src/tests_survivors.rs`) and `PINNED` /
  `PICTURES` (`crates/ship/src/tests_survivors.rs`) — readings of whole
  seeded runs. **If one moves, the run plays differently**: find why (a
  draw on the room's stream added or removed, an order of operations
  changed, a solid gone from a nav grid) and put it back, unless the
  change is meant to alter play;
- a change to a saved type's shape bumps `SAVE_VERSION` (`ship::save`);
  a change to what crosses the wire, or to generation both ends must
  agree on, bumps `wire::PROTOCOL` (and the relay wants redeploying).

## The workspace

Everything is under `crates/`: `app` (Bevy, egui, screens, every word and
sound), `game` (the room — the Bims' simulation; the `bims` package),
`ship` (`Session`, cameras, painters), `lobby`, and the rules crates
`worldgen`, `physics`, `shipdesign`, `flight`, `world`, `economy`, `time`;
plus `wire` and `server` (the relay, `bims-server`).

- **Rules go in `shipdesign` and `world`, never in `ship` or `app`** — a
  server must give the same answer. No `usize`, float or `HashMap`
  iteration in hashed data; `world_checksum` rounds floats onto a grid.
- **No strings or sounds come out of the rules crates.** They hand over
  codes, events and `Cue`s; every word is `crates/app/src/names.rs`
  (`format.rs` for money and time), every clip `crates/app/src/sound.rs`.
  A new part, event or noise is a variant there and a name or clip here;
  `names.rs`'s tests pin the tables' lengths.
- **Profiles only work at the workspace root** (a member's `[profile]` is
  silently ignored). Dev builds dependencies at `opt-level = 3`; keep it.
- **`crate::time`, never `time::`**, in `crates/game` — the probes in
  `scratchpad/` compile its modules without crates.
- Saves are RON through `serde` derives behind each crate's `serde`
  feature (off by default, for the probes).

## The app: things easy to get wrong

- One system per screen per frame, in `EguiPrimaryContextPass`; `Screen`
  in `main.rs` is the state machine.
- **The world's canvas is Bevy meshes** (`scene.rs`), drawn on the GPU
  from packed records (`shapes::pack`, `shape.wgsl`); a canvas **inside a
  panel** is egui's (`canvas::paint_shapes`), because egui panels paint an
  opaque fill over anything Bevy draws. A new kind of shape must go
  through `shapes.rs`'s `fill`/`stroke` **and** `pack` + `shape.wgsl`.
  `BIMS_SHAPES=cpu` is the old path, for comparison.
- Anti-aliasing is feathering in `shapes.rs` (`Msaa::Off`). The bloom
  picks up only colours past white (`draw::Color::glowing`);
  `BIMS_BLOOM=0` turns it off.
- The crew's light map runs on the GPU (`lightmap.rs`, `lightmap.wgsl`),
  byte for byte the CPU's; `BIMS_LIGHTMAP=cpu|check`. A map the CPU
  works out — a planet's plain chunks, the deck under `=cpu` — goes up
  as its two raw channels (only the changed rows) and is blurred and
  coloured by the same `blur` pass (task 140; `check` compares those
  too). Nothing on the GPU is read back into the game.
- **Soft shadows and the corners' shade** (task 140,
  `lightsoften.wgsl`, compiled *after* `lightmap.wgsl` as a second
  module — in the same module the old passes ran 15% slower on RADV, so
  keep them apart). Picture only, over the deck's light map, never the
  plain's chunks. Passes, all compute, in `draw_light_map`'s encoder
  before the main pass: `distance` (exact distance to the nearest
  wall/furniture tile within 2 tiles, from the door-free cells — lamp
  light ignores doors too; once per layout) → `soften_across/down` (the
  shown light, a Gaussian whose reach is `0.5 × distance`, capped at
  **1 tile**; once per lamp change) → `compose_soft` (AO: 0.30 at a
  wall, 0.18 at furniture, over 1.5 tiles) → `sight_across/down` (the
  edge of what is seen, the same penumbra) → the old `blur`. **The
  class cap**: a pixel the room lights (shown ≥ 32/255) is never
  softened under it or shaded darker than the dimmest lit pixel, a dark
  one never over it or into the fog; a seen pixel is ≥ half seen, an
  unseen one < 0.49. `BIMS_SOFTEN_CHECK=1` reads back and counts
  pixels that break it (0 on every scene tried; it does catch a wrong
  bound). `BIMS_SHADOWS=0`, `BIMS_AO=0` turn each off; both off is the
  old picture, 0 px on a screenshot diff. The softening's four
  buffers are group 1, allocated only with a switch on (16 bytes a map
  pixel: 16 MB at a planet's 1072×944). Measured (RX 7800 XT / RADV,
  Ryzen 3700X, release): the light-map pass +0.054 ms on the frames it
  runs (~1 in 8 in a fight), `light soften` 0.09 ms (0.18 on a planet)
  once per layout or lamp change; CPU down 0.004–0.036 ms a frame (the
  plain's CPU blur gone, the cells made only when `cells_version`
  moves). No wasm build, so compute is fine. Not done: GPU lamp shadows
  from a lamp list (a second implementation of the rule; not approved),
  and summing a lamp's flicker on the GPU (field changes are rare —
  not measurable in a 400-frame run).
- **The pointer is egui's** (`canvas::Pointer`); never read Bevy's
  `ButtonInput<MouseButton>` for the canvas.
- **egui's context lock is not reentrant**: calling anything on `ctx`
  inside `ctx.input(|i| …)` deadlocks.
- The first canvas size is `Session::fit`, later ones `resize`. Canvas
  points become room points through `Session::room_point`.
- Every hotkey is a `keys::Action`, never a literal key in a screen; Esc
  is not an action. `keys::release_tab_focus` stops Tab handing egui the
  keyboard.
- A fixture menu opened by a press must not be shut by it (`Menu::fresh`).
- The default font has no arrows (`▾`, `←` come out as boxes) — check a
  new glyph on screen.
- An ability's picture (hero panel box, Skills tab) is
  `ability_icons.rs`: one `Glyph` an ability, drawn in its class's
  family of colours (soldier blue, engineer amber, medic green, tank red,
  commander violet) on a shaded plate. A new ability is a `Glyph`, an arm
  in `Glyph::of` and a figure; convex pieces only.
- Clips are `include_bytes!`'d `.ogg`s made by
  `crates/app/sounds/prepare.sh` (cut from `Sounds/`) and
  `crates/app/sounds/abilities.py` (the classes' abilities, synthesised,
  played off the world's ability events by `Sounds::ability`); every
  level lives in `sound.rs`'s tables.

## Editing gotchas

- **Never use `|` as a perl `s|…|…|` delimiter**, and beware `${…}`
  interpolation — two files were corrupted that way. Use the Edit tool for
  anything with `|`, `$` or backticks.
- Script edits write to a temp file and copy only if it is non-empty
  (`&& [ -s tmp ] && cp`).
- `sed`/`perl` patterns anchored on leading whitespace also match
  deeper-indented lines.
- `f32::clamp` panics when its bounds cross; use `crates/game/src/math.rs`'s
  `clamp` in the room.
- `shapes.rs`'s replay assumes `draw.rs`'s field order; its test pins
  `STRIDE`.
- **New files need `git add -N`**: `nix run` and `nix flake check` build
  the git tree, so an untracked `.rs` fails with `failed to resolve mod`.
- **`grep` here is `ugrep --ignore-files`** and has skipped files; if a
  search comes back empty for something you can see, use `command grep`.

## Where the rest is

Each directory's notes load when a file under it is touched; work across
a seam wants both sides read.

| file | holds |
| --- | --- |
| `crates/game/CLAUDE.md` | the room: routes, doors, sight, the fight, machines, the classes' room halves |
| `crates/world/CLAUDE.md` | the world: clocks, the run, travel and missions, stations, the crisis, classes, relics, traders |
| `crates/ship/CLAUDE.md` | the designer and the game view: cameras, painters, `Session` |
| `crates/shipdesign/CLAUDE.md` | parts, layers, power, cargo, the hash, the validator |
| `crates/worldgen/CLAUDE.md` | the generator and its checksum |
| `crates/flight/CLAUDE.md`, `crates/lobby/CLAUDE.md` | trip plans; the lobby's galaxy |
| `scratchpad/CLAUDE.md` | the native probes; looking at a room without a window |
