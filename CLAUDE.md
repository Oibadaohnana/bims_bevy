# Working on Bims

A 2D top-down co-op roguelike shooter — humans against the machines, one
ship taken from site to site. All the simulation is Rust; `crates/app` is
the one Bevy binary. `README.md` explains the game; this file is how to
work on it. The long version of this file (every feature's history and
measurements) is `git show 5f9da69:CLAUDE.md`; each commit message says
what its task moved.

## Before anything: who else is here, and after: commit

Other agents share the tree. `~/nixcfg/scripts/agents.sh status` (run
from the root) lists them — how many, each one's task and what it is
doing right now; look before anything sweeping. It is awareness only
(re-read a file before editing; keep out of others' way). There is no
counter file to bump. At the end of the session:

1. stop every background task you started (`pgrep -af "sleep "`); leave
   other agents' processes alone (`agents.sh status` lists them);
2. **commit your own work**: `git add` the files *you* touched, one by one
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
| `test` / `test_planet` | a random station / planet settlement, combat ship |
| `droids` | the fight: 16 crew vs droid waves in the arena |
| `combat_droids_<class>` | `droids` with that class at the top level (20), every skill point unspent (`BIMS_RANKS=q,c,e,r` buys ranks) |
| `tier2_test`, `tier3_test` | `droids` with everything at that tier |
| `droids_planet`, `defense` | a town held by / attacked by the machines |
| `crisis`, `jammer`, `guardian`, `bomber`, `lancer`, `conductor`, `relics`, `heart`, `manufacturers` | one mechanic each, staged |
| `breaches`, `sabotage`, `evacuation`, `nests`, `overseer`, `heist`, `prison`, `fuelrun`, `salvage`, `bombs`, `doors`, `chief` | the combat crew in that mission (the second set's dock laid out for it); the game setup's Dev tab forces one in a run |
| `end` | the Heart with company: a lobby at code `THEEND`, Start once a second player joins, ten plain classless bots, tier-three kit, the setup's difficulty on the Heart's day 33 (`BIMS_END_DAY`, in days gone: 32), the first mission not eased; `end offline` is it alone from the game setup, no relay |
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
- `scaling.ron` at the root is the whole of how the enemies scale
  (task 147, areas since October 2026: enemies per player and per bot,
  and four areas — area 0, tiers one to three — each so many days with
  its growth, waves, extras, elite and defenders, all read off the run
  day; the floor's length follows the areas; `world::droid::WaveScaling`,
  `BIMS_SCALING=file` names another) and `rewards.ron` what a fight pays and things cost (xp and
  money an enemy down, a defence's share, the buyback, combine and
  shelf prices; `world::rewards::Rewards`, `BIMS_REWARDS`), both
  read again whenever saved while the game runs
  (`crates/app/src/wavecfg.rs`). Keep `rewards.ron` at the constants —
  its test says so. `scaling.ron` is the player's: the game setup's
  Scaling tab shows every dial, and *Save as default* writes them into
  it (only it has to parse). A smoke run that presses that button wants `BIMS_SCALING`
  pointed at a scratch copy. **Ascensions** (`world::ascension`, 0–5,
  each on top of the ones under it) are picked on the setup's Crew tab
  and laid over the scaling for the run; a win opens the next in
  `profile.ron` (`BIMS_PROFILE_DIR` for a scratch one, `BIMS_ASCENSION=n`
  opens and picks `n` for a look).
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
  `keys.ron` (`BIMS_HOTKEYS`), when there is one, is the game's default
  hotkeys over `keys::Action::default_key` — written by the Controls
  page's *Save as defaults*, read at start. A player's own profile is
  `~/.config/bims/keys` (its `profile=` line picks it or the defaults);
  a smoke run that touches the Controls page wants `XDG_CONFIG_HOME`
  and `BIMS_HOTKEYS` pointed at scratch.
- Two-player runs: a relay (`PORT=18792 target/debug/bims-server`),
  `BIMS_SERVER=ws://127.0.0.1:18792`, `BIMS_AUTO=create` on the host and
  `BIMS_AUTO=join:<code>` on the guest; `scratchpad/duo_resync.sh` is a
  working pair. A guest plays the host's steps through a playout buffer
  (`crates/app/src/playout.rs`, task 148; the keys file's
  `network-buffer`, 0 off); `BIMS_NET_JITTER=<ms>` shakes a guest's line
  and `scratchpad/duo_jitter.sh` (`BUFFER=0` for off) compares the two.
  In a mission a guest runs **ahead** of the host instead (task 156,
  `crates/app/src/rollback.rs`): its own orders played at once and
  stamped with their step, the host applying each at that step, the
  host's timeline kept beside the world shown and copied over it when
  another player's order parts them. What ends a run is read off the
  host's timeline. `BIMS_ROLLBACK=0` is the old way;
  `scratchpad/duo_rollback.sh` (`ROLLBACK=0`) walks a guest about.

## Verifying a change

**Test your own change, not the whole game.** Several agents share the
tree and one `target/`, and a whole crate's suite holds the build lock
and every core: `world`'s 500 tests are 3½ minutes in release on an idle
machine, eight beside other agents' builds. So:

- **Run the tests that cover what you changed, by filter** — the test you
  added and the module beside it: `cargo test -p world tests_area`,
  `cargo test -p bims module::`. Not a whole `world` or `ship` suite, and
  not the whole-run readings (`the_reference_run_comes_out…`,
  `the_run_plays_as_it_did…`, `ship`'s `PINNED`/`PICTURES`) — those are
  the test-all sweep's. A crate small and quick (`wire`, `shipdesign`,
  `economy`, `time`) you may run whole.
- **One profile**: the plain one (no `--release`), which the app's builds
  share; a second profile is a second build of the whole tree. Only a
  timing wants `--release`.
- **Format only your files**: `rustfmt --edition 2024 <file>…` — `cargo
  fmt` rewrites other agents' half-done files under them.
- `cargo clippy -p app --no-deps` (warnings are errors, app only) if you
  touched the app, and — for anything on screen — one smoke run through
  `./hidden` that shows it.
- **Say in your commit and report which tests you ran**, and whether the
  change is meant to alter play — the sweep re-pins the whole-run
  readings off those words rather than every agent re-pinning them in
  turn.

**Do not run `./check` over the tree** unless your task says so: a red
step in a file you never opened is someone else's. The player launches a
test-all agent for the sweep; that agent runs `./check full`, and when a
pinned reading moved for changes the log says were meant to alter play,
re-pins it with a note naming them.

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
- a change to a saved type's shape bumps `SAVE_VERSION` (`ship::save`).
  **`wire::PROTOCOL` is never bumped by hand**: it is `BUILD` in tenths
  over a thousand, and `ship` moves `BUILD` (and redeploys the relay)
  with every push, so a change to what crosses the wire or to generation
  both ends agree on needs nothing more than a line in your commit.

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
  `BIMS_SHAPES=cpu` is the old path, for comparison. Bevy keeps a
  layer's place in the draw order from when it was queued and re-queues
  it only on a mesh/material change, never a `Transform`: a layer whose
  z moves must `set_changed()` its `Mesh2d` (`scene::shape_layer`), or it
  stays at its old z — the first map's deck under its late backdrop.
- **Floors, walls and ground are textures** (`surfaces.rs`): a shape of
  kind `16 + surface` (`32 +` for a triangle; `ship::draw::Surface`,
  `DrawList::surface`) is a rect filled with that layer of one texture
  array times its colour, its `radius`/`line` the **anchor** — where its
  centre is in the texture, in the painter's frame, so a deck's plates
  line up with its tiles (the open ground is tied to the world instead).
  A texel is the surface over its average at a quarter scale, so the
  colour is the surface's average and the CPU path draws it flat. The
  pictures are `crates/app/textures/*.png`, made by
  `textures/make.py` (seeded numpy; its header says how to run it); a
  tile is only 40–100 px on screen, so detail under ~12 texels is lost.
  `BIMS_SURFACES=0` draws them flat. A new surface is a `Surface`
  variant, a `SURFACES` row and a maker in `make.py`. `shape.wgsl`
  multiplies the coverage in **after** the sRGB-to-linear step (egui and
  the CPU path before), or two same-coloured shapes meeting show a dark
  seam. A surface tied to the world (the open ground, water) is sampled
  twice, the second time larger and turned, so its 8-tile repeat never
  shows at a far zoom. Surfaces are drawn 20% towards grey
  (`SURFACE_SATURATION`). **Objects** wear a texture too: a plain kind
  `+ 8` (`KIND_TEXTURED`, set by `DrawList::textured_from` over what a
  painter drew of the objects — `hull_tiles`, `Room::draw`,
  the deployables, the plain's wild) lays `object.png` (wear, grime) over
  a fill, `+ 64` (`KIND_FOLIAGE`, `foliage_from`) lays `foliage.png`
  (leaves), `+ 128` (`KIND_CLOTH`, the room's `clothed_from` over a
  body's coverall and kit) lays `cloth.png` (folds, two seams) started at
  the shape's centre wherever it stands, so a walking body keeps its
  folds, and `+ 4` (`KIND_SWAY`) **sways in the wind**: `shapes::pack`
  moves the shape by `shapes::sway(x, y, t)` off the window's clock
  (`surfaces::wind`), so a cached picture sways too. A fill under 5 world
  units, translucent or glowing is left plain.
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
- Clips are `include_bytes!`'d `.ogg`s in `Sounds/game/` made by
  `crates/app/sounds/prepare.sh` (cut from the recordings in `Sounds/`)
  and `crates/app/sounds/abilities.py` (the classes' abilities,
  synthesised, played off the world's ability events by
  `Sounds::ability`); every level lives in `sound.rs`'s tables. **The
  player edits those clips by hand** (`Sounds/game/README.md`), so run
  either script only with the names of the clips you mean to re-make
  (`prepare.sh smoke_bang`, `abilities.py downed`) — a bare run
  overwrites every clip — and a new clip wants its line in that README.

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
| `crates/ship/CLAUDE.md` | the game view: cameras, painters, `Session` |
| `crates/shipdesign/CLAUDE.md` | parts, layers, power, cargo, the hash, the validator |
| `crates/worldgen/CLAUDE.md` | the generator and its checksum |
| `crates/flight/CLAUDE.md`, `crates/lobby/CLAUDE.md` | trip plans; the lobby's galaxy |
| `scratchpad/CLAUDE.md` | the native probes; looking at a room without a window |
