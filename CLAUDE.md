# Working on Bims

A 2D top-down game: all the simulation is Rust, and so is the window —
`crates/app` is a Bevy binary that draws every crate's shape buffer through
egui and lays the panels out round it. `README.md` explains how the game
itself fits together; this file is about working on it.

## Running it

`nix run .` is the one command: it **builds and opens the window**. There are
eight things to run, and each is a name rather than a flag:

| command | `cargo run` | opens |
| --- | --- | --- |
| `nix run .` / `nix run .#game` | `cargo run -- game` | the whole game in order — menu, setup or lobby, world and station, ship design, then the world docked where you said |
| `nix run .#simulation` | `cargo run -- simulation` | straight into the world on the playtest ship |
| `nix run .#design` | `cargo run -- design` | straight into the yard, the playtest ship given, docked where the simulation docks — how a change to the designer is looked at |
| `nix run .#room` | `cargo run -- room` | the behaviour test room — Bims on a deck |
| `nix run .#test` | `cargo run -- test` | the simulation somewhere else each time — docked at a random station somebody lives on, in a random galaxy, on the **combat ship** with one crew member (four bunks to spare) and a **mercenary for hire** at the dock whatever the roll said (`Session::mercenary_for_probe`) |
| `nix run .#test_planet` | `cargo run -- test_planet` | `test` **set down on a planet**: the same random galaxy and roll, made among the systems whose first planet with ground has friendly people (`world::spawn_with_ground`, `ship::session::pick_ground`), and the ship landed at its settlement the way `BIMS_LANDED=1` lands the simulation (`Session::land_for_probe`) — the mercenary asked for first, so the settlement's room has one too |
| `nix run .#combat` | `cargo run -- combat` | the fight: the **combat ship** (`shipdesign::fixture::combat_ship`, the playtest ship with bunks and chairs for five) with **fourteen crew** (`COMBAT_CREW`: five at the bunks, nine standing on the deck), a gun in every hand — the five kinds dealt round — docked at the spawn rebuilt as the **arena** (`world::station::arena`, 72 tiles across with bunks for a garrison) and made **hostile**: its people are enemies — `ARENA_GARRISON`, fifteen, whatever the crew's worth — and a recruited crew member draws its weapon and shoots at any it can see. `Session::combat` is all of it; `--combat` is taken too |
| `nix run .#stationbuilder` | `cargo run -- stationbuilder [name]` | the **station builder**, a tool rather than a screen of the game: a grid to sketch a station's rough shape on — deck, wall, door, airlock, painted as rectangles or with a pen, the skin drawn wherever deck touches void — saved by Ctrl+S as text to `stations/<name>.txt` (`name` defaults to `sketch`; `BIMS_STATIONS_DIR` moves the directory, and the nix wrapper points it at `$PWD/stations`) and read back the next time that name is opened. The file is one character a tile, for a `world::station::Plan` to be written from by hand. `crates/app/src/screens/station.rs` |

`cargo run` (with `-p app`, or bare — `default-members` makes the app the
default) builds from the working tree, which is the one to use while editing,
and `cargo run --release -- test` is the release build docked somewhere new;
`nix run` builds from the **git tree**. `bims --self-check` prints whether
the build agrees with the pinned constants and exits non-zero if not.

**The window wants the shell, and so does the build.** winit and wgpu open
the window system's libraries and the Vulkan loader at run time, and
`shell.nix` puts them on `LD_LIBRARY_PATH`; and since the sound went in,
the app *links* ALSA — cpal, under Bevy's audio, finds it through
pkg-config — so a `cargo build -p app` outside the shell dies in
`alsa-sys` with a misleading "build failed". Build and run inside
`nix-shell shell.nix` (or `nix develop`), or through `./check`, which
re-enters the shell itself when either is missing. A `cargo run` that
opens nothing, or dies looking for `libvulkan`, is this. The rules crates
alone (`cargo test -p bims`, `-p world`) build anywhere.

**A run with nobody at the keyboard** is `crates/app/src/dev.rs`:
`BIMS_SMOKE_FRAMES=n` runs `n` frames in a hidden window and exits;
`BIMS_SCREENSHOT=file.png` saves the frame thirty before the end;
`BIMS_POINTER="40:move:600,250;60:click:600,250;90:right:300,400"` and
`BIMS_KEYS="60:Escape,90:M"` drive the pointer and the keys at those frames,
in logical points from the window's top left. `wheel` and `wheelup` at
a point are a notch of the wheel, which zooms. `BIMS_AT_BELT=1` opens the
simulation holding at a belt with its mining site laid out, for looking at
the outside without flying there; `BIMS_LANDED=1` opens it set down on
the spawn system's first planet with ground — the pad, the ground and
the settlement beside it — and `BIMS_LANDING=0.7` over that planet with
the landing run seven tenths of the way down, for looking at the descent
(feature 52, `crates/world/CLAUDE.md`); `BIMS_FIGHT=1` opens `combat` with the
crew member recruited inside the station's door and one of its people a
few tiles down the corridor, for looking at a fight without walking the
station for one. `BIMS_AFIELD=1` opens the simulation landed with the
crew member walked out onto the plain west of the ship and a minute gone
by, and `BIMS_ZOOM=0.3` zooms the game view out by that factor once it
is fitted (a scripted wheel does not reach it): the two together are how
the plain and its fog are looked at. `BIMS_TRADE=1` opens the simulation with the station's
trade window up, for looking at the cart; `BIMS_ARMOURY=1` opens it
with the armoury window up, for looking at the lockers' grid — a
`press`, `move`s and a `release` in `BIMS_POINTER` drag a thing across
it. `BIMS_LAMPS_OUT=n` shoots the `n` lamps nearest the crew member
out at open and leaves the next one failing, for looking at the dark
round a lamp that is out and a failing lamp's flicker (`BIMS_FIGHT=1
BIMS_LAMPS_OUT=3` is the lobby dark). `BIMS_SOUND_LOG=1` prints
every clip as it is played and every bed as it fades up or out, which is
how a sound is *heard* from a terminal — `BIMS_SOUND_LOG=1 BIMS_FIGHT=1 BIMS_SMOKE_FRAMES=900 bims
combat | grep ^sound:` is a fight's worth. Move at least a frame before
clicking — egui hit-tests a click against the widgets laid out on the
previous frame. That is how a change to a screen is *looked at* from a
terminal: run it, read the PNG. Two things about it: on Wayland the
compositor decides a hidden window's size and it can be small, so read the
picture for what is drawn rather than where; and a scripted click goes out
as a `WindowEvent` as well as a typed message, because bevy_egui reads the
former and Bevy's own input the latter. A smoke run's window is
`bims-smoke` to the window system (`dev::window_name`; the game's is
`bims`) — a Wayland window cannot be hidden, and `BIMS_FULLSCREEN=1`
makes it the whole screen — and `./check` gives Hyprland a live rule that
parks that class on workspace `SMOKE_WORKSPACE` (2) without switching to
it, so the nine runs go by unseen. `BIMS_SAVES_DIR` moves the saves,
and `./check` points a smoke run's at `target/check/saves`.

## `AGENTS` is how many of you there are — read it first

`AGENTS` at the root is a plain text file holding **one number**: how many
agents are working on this tree right now. Before doing anything else,
read it, add one, write the new number back, and remember it as yours:
`1` means you are alone, `2` means you are the second alongside one that
is already at work. The agent with the **highest number runs the final
checks** — `./check`, the smoke runs — when its work is done, and puts
the file back to `0` afterwards; every other agent finishes its code,
runs the tests of the crates it touched, and leaves `./check` to the
one above it.

## It is a workspace, and the app is one crate of eleven

Everything is under `crates/`. `app` is the **one binary**: Bevy, egui, the
four screens, the pointer, and every word on the screen. The rest are
libraries. `game` is the room — the Bims' simulation, which `world` runs
aboard the designed ship and the app runs on its own as the test room.
`ship` is the design phase and the game it starts: `Session` (the editor,
then the game), the cameras and the painters. `lobby` is the World tab's
galaxy, a camera and a pick. Beside them are eight libraries that have to
give the same answer in more than one place: `worldgen` (the galaxy, systems
and station blueprints, which a native server will one day generate
identically), `physics` (ship mass, thrust and travel time), `shipdesign`
(what a ship is made of and the rules for putting one together), `flight`
(what a design does when you push it, and the closed-form plan that flies a
trip), `world` (one star system, the ship in it, and the one clock they both
run on), `economy` (money: whole euros, the crew's shared pool, and sums that
must not wrap), `health` (one body's health points, what is wrong with it
and what that costs) and `time` (how long a day is).

Things about that which are easy to get wrong:

- **Profiles only work at the workspace root.** A `[profile.*]` in a member's
  `Cargo.toml` is *silently ignored*. The root has `opt-level = 1` for our
  own crates in dev and `3` for every dependency, because a debug build of
  Bevy is unplayable; keep it there.
- **A bare `cargo test` is the app alone.** `default-members` makes a bare
  cargo command mean `app`, so `cargo test` runs one crate of eleven. Use
  `cargo test --workspace`, which is what `./check` runs. `game` has no unit
  tests; its tests are the native probes in `scratchpad/`.
- **The rules go in `shipdesign` and `world`, never in `ship` or `app`.**
  `ship` may ask whether a part can be placed or a trip can be flown; it may
  not decide, and the app may not either. A native server has to give the
  same answer, and two numbers — `design_hash`, which is what an Accept is
  recorded against, and `world_checksum`, which is what a client will one day
  be verified against — have to come out **identical everywhere**. That is
  why nothing in `shipdesign`'s data or its hash is a `usize` or a float and
  why no `HashMap` is iterated in it. Both are pinned against written-down
  constants in `shipdesign::fixture` and `world::fixture`, by
  `crates/shipdesign/src/tests.rs` and `crates/world/src/tests.rs` one at a
  time and by `ship::session::self_check` (`bims --self-check`) all at once.

  `world_checksum` **rounds its floats onto a grid on the way in**, and that
  is not sloppiness: everything in `flight` that is not arithmetic is `sin`,
  `cos`, `atan2` and `sqrt`, which two platforms' libms may disagree about in
  the last bit. Positions go in at a thousandth of a unit and angles at a
  millionth, which is orders of magnitude finer than any real divergence and
  orders coarser than a rounding one.
- **No strings come out of the rules crates, and no sounds.** The room hands
  over crew member 0 and spot code 6; `ship` hands over a part kind and an
  issue code; `world` hands over a `WorldEvent`. Every word is
  `crates/app/src/names.rs`, and `format.rs` is the only place the euro sign,
  the digit grouping and the clock's colon exist. The same for what is
  heard: the room says a door started sliding or a bolt landed as a
  `bims::cue::Cue` with a place (`Game::take_cues`), and
  `crates/app/src/sound.rs` is where the recordings are and what each cue
  is played as. That is deliberate — a server has no words to say and no
  speaker — so keep it that way: a new part, event, job or noise is a
  variant there and a name or a clip here, and `names.rs`'s tests pin the
  tables' lengths against the enums.
- **`crate::time`, never `time::`.** `crates/game/src/clock.rs` reaches the
  `time` crate through the crate root, because the probes link nothing and
  stand a plain `mod time;` over the same file — see `scratchpad/modules.rs`.
  Spelled `time::` it would only resolve through the extern prelude and every
  probe would need a `--extern` and a build step behind it.

## How the app is put together

- **One frame is one system per screen**, in `EguiPrimaryContextPass`:
  step the simulation, lay the panels out, read the pointer, paint the
  shapes, put the words on top. `Screen` in `main.rs` is the state machine
  and each screen's plugin runs only in its state.
- **The shape buffer is drawn by egui**, not by a Bevy mesh:
  `shapes.rs` tessellates the twelve floats a shape into an `egui::Mesh`,
  and `canvas::paint_shapes` adds it to a painter clipped to the canvas —
  the background layer for a canvas between the panels, the panel's own
  painter for one inside it (the lobby's galaxy and diagram). Bevy draws the
  clear colour and nothing else. This matters because egui panels paint an
  opaque fill: a Bevy mesh under a `CentralPanel` is a mesh nobody sees,
  which is how the galaxy preview was blank for a build.
- **Anti-aliasing is feathering, in `shapes.rs`, and nothing else.**
  bevy_egui paints into the window's *unsampled* target, so a camera's
  `Msaa` never reaches the mesh; every edge is instead ramped a pixel wide
  from its colour to transparent the way epaint draws its own shapes, and a
  stroke thinner than a pixel is drawn a pixel wide and fainter. The pixel
  is `painter.pixels_per_point()`, so it stays one device pixel under any
  UI scale. A new kind of shape has to go through `fill` or `stroke` there,
  or it comes out jagged beside everything else.
- **The Esc sheet is `settings.rs`**: five pages in one window — the
  menu (the UI scale, a button each for Audio and Controls, and Save
  and Load), the audio page (`sound::Mix`: master, effects, ambience,
  mute), the controls page (every key, rebindable), and the save and
  load pages (`save.rs`). A screen holds it as `Option<Sheet>`, opens it
  on Esc at the menu, and Esc closes it from any page — except while
  the controls page is waiting on a key (`Keys::listening`), when Esc is
  that page's to cancel with.
- **A save is the world as RON text, and the rules crates derive serde
  for it behind a feature.** `ship::save` (feature 53) writes
  `Session`'s world with the four numbers round it — players, the local
  slot, the galaxy type's code, the spawn — and reads one back into a
  session stood up the way `simulate_on` stands one up (`Game::resume`:
  the cameras fitted, nothing aimed). Every type in `game`, `world`,
  `flight`, `shipdesign`, `economy`, `health`, `worldgen` and `physics`
  carries `#[cfg_attr(feature = "serde", derive(...))]`, and each crate's
  `serde` feature switches on its dependencies' — a *feature*, off by
  default, because the probes compile the room's modules with no crates
  at all. `ship` turns `world/serde` on and depends on `serde` and `ron`
  outright (both were in the lock already, under Bevy). Two things are
  left out of a save with `serde(skip)`: the room's draw buffer and a
  surface's lazily built settlement, both functions of what is saved.
  A change to a saved type's shape is a bump of `SAVE_VERSION`, and an
  old file is refused by that rather than read wrong;
  `a_game_saved_and_read_back_is_the_same_game` in `crates/ship` is the
  round trip — checksum, crew positions and the picture, as read back
  and six hundred steps on. The app's side is `crates/app/src/save.rs`:
  the directory (`$XDG_DATA_HOME/bims/saves`, or `BIMS_SAVES_DIR`,
  which `./check` points at `target/check/saves`), one `<name>.ron`
  each, names by the sketches' rule, and the two pages — the Esc
  sheet's, and the menu's Load window. A page hands back a
  `save::Request` and the screen does it: the game screen writes
  `session.save()`, and a load anywhere replaces the `ShipSession`
  resource and stands a fresh `GameScreen` up (`GameScreen::fresh`, what
  `open` builds too) so the panels, the log and the aim start over and
  the first frame fits the loaded world to the canvas. Save is greyed
  out in the design phase: there is no world yet.
- **Every hotkey is an `Action` in `keys.rs`, never a key in a screen.**
  `Keys` is a Bevy resource the screens read (`keys.pressed(i,
  Action::Map)`), the crew panels get a copy each frame for the armoury's
  Turn key and the hints, and the Controls page rebinds any of them —
  click the key, press another, Esc keeps the old one. Two actions may
  share a key (Recruit and Turn both start on R, told apart by what is
  in hand); the page says "also …" rather than refusing. Esc is not an
  action. The bindings are saved to `$XDG_CONFIG_HOME/bims/keys`
  (`~/.config/bims/keys`), one `action=Key` a line, and read at start —
  the one thing the sheet keeps between runs so far. Tab is the
  Inventory action: it opens and shuts the crew member's inventory —
  and opens with it the nearest thing within reach, a container or a
  body down, the rest of them a click away on the **Nearby** strip over
  the window (`CrewPanels::nearby`, a `Near` list the screens rebuild
  every frame off the room's `within_reach` and the world's
  `in_reach_of_body`). `BIMS_KEYS` knows `Tab` and `Space` by name.
  **egui moves keyboard focus on Tab**, and a widget with focus is egui
  wanting the keyboard, so the frame after a Tab every key would have
  been egui's: `keys::release_tab_focus` at the top of a screen's frame
  surrenders the focus Tab gave (`tab_took_focus` on the screen). A
  text field that has focus keeps it.
- **The pack is the lockers' grid again, ten across by five down** — the same
  `grid::lockers` widget, `CrewPanels::pack_drag`, and a drop is
  `GearOrder::Repack` → `Command::Repack`. The Loot window draws a body's
  pack on it too, with `movable` off: looked at, not tidied.
- **The UI scale is egui's zoom factor** (`theme::ui_scale_row`, on the Esc
  sheet's menu): it scales the type, the panels and the canvas alike, and
  bevy_egui divides the pointer by it, so nothing in the screens has to
  know. A `BIMS_POINTER` script is in points *before* the zoom, so drive a
  screen at 100% or the clicks land elsewhere.
- **The pointer is egui's.** `canvas::Pointer::read` takes the pointer out
  of the egui context, and `Pointer::on(rect)` answers `None` whenever egui
  wants it — over a panel, a window, a menu — so a click on a panel is the
  panel's and a click beside it is the canvas's by the same rule egui uses.
  Do not read Bevy's `ButtonInput<MouseButton>` for the canvas: the two
  would disagree about a click on a floating panel.
- **egui's context lock is not reentrant.** `ctx.input(|i| ...)` holds it;
  calling `ctx.is_pointer_over_egui()` or anything else on `ctx` *inside*
  that closure deadlocks, and epaint's debug build panics after ten seconds
  saying so. Ask everything you need of `ctx` before the closure.
- **The first canvas size is a fit, not a resize.** A session is made
  before the panels have been laid out, so its cameras start at the window's
  size; the first frame that knows the real canvas calls `Session::fit`,
  which starts the views again from that size (the whole build area, the
  whole hull), and every later change is a plain `resize`. Without that the
  designer opens half hidden under its own checks panel.
- **The crew's panels are one module**, `crew.rs`, shared by the room and
  the game, and the screen hands the room its coordinates: on the room's
  screen a canvas point is the room's through `Game::view_scale/offset`; on
  the ship's it is `Session::room_point`, the pointer read back through the
  ship's camera and heading. Every `Game` method that takes a point takes a
  room point.
- **Every container window is `grid::lockers`, and the rule for a drop
  is the world's.** The shelves, the cold store and the lockers are the
  world's grids (`World::grids`, `crates/world/CLAUDE.md`), every thing
  drawn over its footprint with a stack's count in the corner; only the
  research desk is `grid::grid`, a cell. The widget carries a thing on a
  drag, turns it on `R` and hands back a `Moved` — the drag itself lives
  on `CrewPanels::locker_drag` between frames. Whether the ghost is
  green is `Grid::fits` asked of the `Hold` snapshot, never worked out
  here, and the drop goes through the seam as `Command::Arrange` like
  every other change to the hold. `BIMS_ARMOURY=storage` and `=fridge`
  open those windows for a screenshot, and `=workbench` the one
  container that is not a class of the hold: the workbench's three
  slots (`CrewPanels::bench_window`, `Hold::bench`), two `grid::grid`s
  with the Upgrade button between them — greyed with the world's own
  refusal (`World::can_upgrade`) — and a thing goes onto it from the
  pack by Ctrl-click or its Bench row (`GearOrder::StowOnBench`) and
  off it by Ctrl-click or Take (`FetchKind::Bench`); see
  `crates/world/CLAUDE.md`, "Two of a kind go onto the workbench". A
  thing's picture over a long footprint is `icons::laid`: the long guns
  have a wide drawing (`draw_wide`), everything else sits square in the
  middle, and a turned thing is drawn upright into a `Sketch` — the
  shapes gathered rather than painted — and turned a quarter, since a
  painter cannot turn a shape once it has it.
- **A highlight is not a tooltip.** Resting on a row that names a fixture
  rings it on the deck (`CrewPanels::points`); nothing pops up. The ring is
  worked out afresh every frame from what is hovered, so a panel folding
  away under the pointer cannot leave one lit. Tooltips hang only off an
  underlined word (`theme::asks`) or a `?` (`theme::question_mark`), never
  a row or a bar.
- **A fixture menu opened by a press must not be shut by it.** `Menu::fresh`
  is that guard: the click-away check skips the frame the menu opened on.
- **The default font has no arrows.** `▾`, `←`, `‖` come out as boxes;
  the panels say `Hide`, `< Back`, `||` instead. Check a new glyph on
  screen before trusting it.
- **Sound is `sound.rs`, and the clips are bytes in the binary.** The
  recordings are `Sounds/` at the root, left as recorded;
  `crates/app/sounds/prepare.sh` (ffmpeg) cuts and filters them into the
  `.ogg` clips beside it — one-shots trimmed to the event and peaked at
  -1 dBFS, loops
  seamed by cross-fading their own tail into their head, the ambiences
  brought down to -30 LUFS — and `sound.rs` `include_bytes!`s those, so a
  re-run of the script is a rebuild and nothing is read from disk at run
  time. Every level the game applies is in `sound.rs`'s tables, not in
  the files; the player's own volumes are `Sounds::mix`, set on the Esc
  sheet's audio page and multiplied in at the end — a one-shot's when it
  starts, so a mute does not spawn one, and a bed's every frame. A one-shot despawns itself; a **bed** (the ship's hum, a
  station's, the engines) loops the whole time at whatever level the
  screen asks for *every frame*, and fades out when nobody asks, so a
  screen that closes takes its sound with it. Cues are thinned with a
  cool-down **per kind and per place**: at 24x a frame holds a whole
  meal's chopping, which is one stroke, but two guns in one frame are two
  shots — the staged fight puts both gunners on the same cadence, and a
  cool-down per kind alone silenced every enemy shot. No audio device is
  a warning from Bevy and silence, never a failure, which is what the
  hidden smoke runs rely on.

## Verifying a change

`cargo build` succeeding proves nothing about the window: a panel laid out
over the canvas, a mesh under a panel, a click that lands on the wrong
layer — none of that is a type error.

**`./check` runs all of it.** The quick tier — `./check` — is the git-tree
check, `cargo fmt`, the workspace build, `cargo test --workspace`, clippy on
the app, and a smoke run of each of the eight windows (sixty frames, hidden,
with a screenshot of the room, the simulation, the yard, the combat
dock, the simulation landed on a planet and `test_planet` left in
`target/check/`); `./check full` adds the native probes (compiled fresh into `target/probes/`,
never the stale binaries in `scratchpad/`) and `nix flake check`.
`./check <step>...` runs a subset, `./check --list` explains each. Full
output is under `target/check/`; only the failing lines are printed. "Done"
means `./check` is green, and a step that was red before the change should
be said so out loud rather than folded into the report. It re-execs itself
under `nix-shell shell.nix` if the toolchain is not on PATH, and a global
Claude Code hook (`~/.claude/hooks/nix-toolchain-env.sh`) puts it there for
every session (`.envrc` does the same for a human with direnv).

Clippy is run on `app` only, with warnings as errors: the rules crates
predate clippy here and carry style lints that are not bugs. New code in the
app is written under it.

For the simulation itself, the modules compile natively. A scratch `main.rs`
that pulls them in by `#[path]` and declares them at the crate root (so
`crate::room` still resolves) can drive `Game` directly — run a task to
completion, assert the Bim ends up where it should — and can dump the real
draw buffer as SVG to look at the room without a window
(`scratchpad/layout.rs`).

## Editing gotchas

- Beware `sed`/`perl` substitutions anchored on leading whitespace: a pattern
  indented four spaces also matches inside a six-space-indented line. Anchor
  on something unique, and re-read the result.
- `f32::clamp` panics when its bounds cross. Use the branchless `clamp` in
  `crates/game/src/math.rs` in the room: a panic in a step is a stall with no
  message.
- The replay in `crates/app/src/shapes.rs` assumes the field *order* in
  `crates/game/src/draw.rs`, and its test pins `STRIDE` against all three
  painters'.

## New files need `git add`

`nix run .` and `nix flake check` build from the **git tree**, so a new
`crates/*/src/*.rs` that is only on disk is invisible to them — the build
fails with `failed to resolve mod <name>` even though `cargo build` is
perfectly happy. `git add -N <file>` is enough; it does not commit anything.
A whole new crate is the same trap with a louder failure: `members =
["crates/*"]` matches a directory the git tree does not have. `./check
tracked` is the step that catches both.

## Where the rest is

The notes are split by directory, and Claude Code loads each one **on
demand** — the moment a file under that directory is read or edited. Work
that crosses a seam wants the other side's notes read as well: a chain in
the room, say, is also a step of the world's clock and a menu in the app.

| file | what it holds |
| --- | --- |
| `crates/game/CLAUDE.md` | the room: routes, needs, errands, the two Bims, the diary, doors, the bay, stew, the helm as a job |
| `crates/shipdesign/CLAUDE.md` | the rules: parts, layers, power, cargo, money, the hash, the validator, the playtest ship |
| `crates/world/CLAUDE.md` | the world: the clock and its stages, stations, docking, the crew aboard, the helm, crafting, EVA |
| `crates/ship/CLAUDE.md` | the designer and the game view: the camera, the painters, drags, `Session` |
| `crates/flight/CLAUDE.md` | a plan is read, never integrated; the two pinned placeholder numbers |
| `crates/health/CLAUDE.md` | one body's health, and why a step of any length gives the same answer |
| `crates/lobby/CLAUDE.md` | "has a station" is answered by generating the system; a crew never starts at an enemy's |
| `crates/worldgen/CLAUDE.md` | the generator: the checksum, up to six stations a system, which of them are hostile, who sells what |
| `scratchpad/CLAUDE.md` | the native probes, and how to look at a room without a window |

## Never use `|` as a perl `s|…|…|` delimiter here

Two files were corrupted by `perl -0pi -e 's|…|…|'` where the pattern or the
replacement contained a `|` — a markdown table row, a `||`. The delimiter
closes at the first one, and what is left is pasted somewhere unrelated.
Interpolation is the other half of the trap: `${…}` inside a double-quoted
perl replacement is *perl* interpolation.

Use the Edit tool for anything containing `|`, `$` or backticks, and re-read
whatever you touched.

## `grep` here is `ugrep`

The shell's `grep` is a function wrapping `ugrep --ignore-files`, and it
has skipped whole files before for no reason it would say. If a search
comes back empty for something you can see, use `command grep`.
