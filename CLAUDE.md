# Working on Bims

A 2D top-down game: all the simulation is Rust, and so is the window —
`crates/app` is a Bevy binary that draws every crate's shape buffer through
egui and lays the panels out round it. `README.md` explains how the game
itself fits together; this file is about working on it.

## Running it

`nix run .` is the one command: it **builds and opens the window**. There are
twenty-three things to run, and each is a name rather than a flag — and
`cargo run -- list` prints every one of them with a line each, which is
the build's own answer where this table is a copy:

| command | `cargo run` | opens |
| --- | --- | --- |
| `nix run .` / `nix run .#game` | `cargo run -- game` | the whole game in order — menu, setup or lobby, world and station, ship design, then the world docked where you said |
| `nix run .#simulation` | `cargo run -- simulation` | straight into the world on the playtest ship |
| `nix run .#design` | `cargo run -- design` | straight into the yard, the playtest ship given, docked where the simulation docks — how a change to the designer is looked at |
| `nix run .#room` | `cargo run -- room` | the behaviour test room — Bims on a deck |
| `nix run .#test` | `cargo run -- test` | the simulation somewhere else each time — docked at a random station somebody lives on, in a random galaxy, on the **combat ship** with one crew member (four bunks to spare) and a **mercenary for hire** at the dock whatever the roll said (`Session::mercenary_for_probe`) |
| `nix run .#test_planet` | `cargo run -- test_planet` | `test` **set down on a planet**: the same random galaxy and roll, made among the systems whose first planet with ground has friendly people (`world::spawn_with_ground`, `ship::session::pick_ground`), and the ship landed at its settlement the way `BIMS_LANDED=1` lands the simulation (`Session::land_for_probe`) — the mercenary asked for first, so the settlement's room has one too |
| `nix run .#combat` | `cargo run -- combat` | the fight: the **combat ship** (`shipdesign::fixture::combat_ship`, the playtest ship with bunks and chairs for five) with **fourteen crew** (`COMBAT_CREW`: five at the bunks, nine standing on the deck), a gun in every hand — the five kinds dealt round — docked at the spawn rebuilt as the **arena** (`world::station::arena`, 72 tiles across with bunks for a garrison) and made **hostile**: its people are enemies — `ARENA_GARRISON`, fifteen, whatever the crew's worth — and a recruited crew member draws its weapon and shoots at any it can see. `Session::combat` is all of it; `--combat` is taken too |
| `nix run .#combat_engineer` … `#combat_commander` | `cargo run -- combat_medic` | that **same fight with the class in hand** (feature 79, `Launch::CombatAs`): one command a class — `Class::ALL` bar `None`, spelled as `names::CLASS_NAMES` spells it, lower case — and nothing else about the run differs, since it is `Session::combat`'s own ship, arena and garrison with `World::set_class(0, …)` on top (`dev::class_crew`, which takes the command's class and lets `BIMS_CLASS` override it). It opens at the **tenth level** (`dev::COMBAT_CLASS_LEVEL`, feature 80) with all seven of the class's talents still to choose, so the tray opens on the **Skills** tab (feature 83) with seven points to spend and the whole tree is the player's to walk down; `BIMS_LEVEL=n` says otherwise. The **engineer of such a run has the sentry kit its class deals it** (`world::deploy::ENGINEER_START_SENTRIES`, one, beside three sandbag kits): a sentry kit is otherwise made at a workbench and nobody in a scripted fight stands at one, so the class brings its own and `dev.rs` no longer has to. The parsing is `main.rs::class_named`, and `bims list` prints the lot |
| `nix run .#tier2_test` | `cargo run -- tier2_test` | `combat` with **everybody's kit at tier two** — every crew member's gun at it (its kind as `combat` dealt it) and a fresh helm, kevlar and leg guards at it on, pieces of the world's, and every one of the garrison the same, its pieces the room's own (`Session::combat_at_tier`, `World::outfit_for_probe`) — so the fight is looked at with nothing at tier one on either side. `BIMS_FIGHT=1` stages it as it stages `combat`, the resident's pistol kept at its tier |
| `nix run .#tier3_test` | `cargo run -- tier3_test` | the same at **tier three** |
| `nix run .#droids` | `cargo run -- droids` | the **machines** (feature 83): `combat`'s own fight — the combat ship, its fourteen crew, a gun in every hand — at an arena the **droids hold** instead of its garrison. Its people are gone (`World::people_of` is nought for a held station) and a wave of machines stands about it instead: Wardens a sixth, Husks a third, Troopers the rest, sized by `droid::wave_size` (the base, the crew, the calendar, the worth and the levels, added — never doubled — and capped at `DROID_WAVE_MAX`). `DROID_REINFORCE_MINUTES` is **one minute** here rather than two hours, so the next wave is watched landing at the far airlock rather than waited for, and the station has **three waves** rather than the formula's two at day nought (`DROID_WAVES_IN_PROBE`), since one wave landing and then a cleared station is not what these commands are for — the red line along the top says which wave is on the deck, how many of it are standing, and, the moment the last of them is down, **how long until the next lands**. `BIMS_DROID_TIER=2` brings them at a tier, `BIMS_DROID_WAVES=5` gives the station that many waves, `BIMS_DROID_REINFORCE=600` makes the wait between them that many minutes of the clock — a minute is a real second at 1× and two and a half *frames* at 24×, so the countdown cannot be caught by a scripted run without lengthening it — and `BIMS_DROID_WAVE=32` makes a wave that many whatever the formula says, which is how the measurements below were taken; `BIMS_DROIDS=1` replaces the wave with a **showcase** — a row a kind and a column a state: idle, firing or striking, arms at nothing, legs at nothing, destroyed — so all fifteen drawings are one screenshot (`World::stage_droids_for_probe`) |
| `nix run .#combat_droids_engineer` … `#combat_droids_commander` | `cargo run -- combat_droids_medic` | that **same wave with the class in hand** (`Launch::DroidsAs`), which is to `droids` exactly what `combat_<class>` is to `combat`: the same combat ship, the same fourteen crew, the same droid-held arena and the same two dials, with `World::set_class(0, …)` on top through the same `dev::class_crew` — so what a class does **against the machines** is the one thing two of these runs differ by. The tenth level, the Skills tab with its seven points, the engineer's two sentry kits and `BIMS_CLASS`/`BIMS_LEVEL` over the lot are `combat_<class>`'s own, since it is the one call. The parsing is the trap: `combat_droids_` starts with `combat_`, so `main.rs` tries the longer prefix first (`DROIDS_AS` before `COMBAT_AS`) or the word reads as a class nobody is called |
| `nix run .#droids_planet` | `cargo run -- droids_planet` | `test_planet` with the **town** droid-held: the same random galaxy and roll, the ship set down at the settlement, and the settlement's people replaced by the machines, whose lander sets down on the plain beyond the north gate for an odd wave and the south for an even one. The same minute's reinforcements and the same two dials |
| `nix run .#raid` | `cargo run -- raid` | the simulation **off its berth, holding in open space, with a raid on its way**: the next raid brought forward to ten minutes of the clock — ten seconds at 1× — so contact comes as you watch, the raider closing at its own pace after it (`Session::raid_coming_for_probe`, `World::raid_coming_for_probe`; `RAID_IN_MINUTES` in `screens/game.rs`). Where `BIMS_RAID=contact` opens with the raider already on the radar, this is the warning arriving |
| `nix run .#stationbuilder` | `cargo run -- stationbuilder [name]` | the **station builder**, a tool rather than a screen of the game: a grid to sketch a station's rough shape on — deck, wall, door, airlock, painted as rectangles or with a pen, the skin drawn wherever deck touches void — saved by Ctrl+S as text to `stations/<name>.txt` (`name` defaults to `sketch`; `BIMS_STATIONS_DIR` moves the directory, and the nix wrapper points it at `$PWD/stations`) and read back the next time that name is opened. The file is one character a tile, for a `world::station::Plan` to be written from by hand. `crates/app/src/screens/station.rs` |

`cargo run` (with `-p app`, or bare — `default-members` makes the app the
default) builds from the working tree, which is the one to use while editing,
and `cargo run --release -- test` is the release build docked somewhere new;
`nix run` builds from the **git tree**. `bims --self-check` prints whether
the build agrees with the pinned constants and exits non-zero if not, and
`bims list` (`--list`, `--help`, `-h`) prints every command above with a
line each rather than opening anything — `main.rs::COMMANDS` and
`Class::ALL`, so a class added later is listed without a word written.

**Whatever it opened, Esc → Restart → Start again puts the run back to
where it started** (feature 79): the world is written out as a save the
moment the game screen opens (`crates/app/src/save.rs`'s `Beginning`,
`screens::game::remember_beginning`) and a restart reads it back through
`Session::restore` the way a load does — the same new screen round it,
the same `Packet::World` to the guests, and `Request::Restart` alongside
`Request::Load` in one arm because they are the same thing happening.
Nothing on disk is touched. The end screen (`screens::game::over`) has
the same as *Start again*, since the fight is where a restart is wanted
and where the run tends to end.

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

**A run with nobody at the keyboard** is `crates/app/src/dev.rs`, and it
is run through **`./hidden`** so it opens on nobody's desktop:
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
station for one. `BIMS_RAID=1` opens the simulation off its berth in
open space with a raider tied to it and its boarders at the ship's
airlock, locked in their face — forcing it by 900 frames, through it by
2400 — and `BIMS_RAID=contact` with the raider on the radar and
closing, for the map; the red warning along the top
(`screens/game.rs::raid_warning`, its words `names::raid_*`) is in
every one of those pictures; `BIMS_LOST=1` opens it with every crew member
shot where they stand, so the end screen is what the next frame is
(`./check` runs `raided`). `BIMS_AFIELD=1` opens the simulation landed with the
crew member walked out onto the plain west of the ship and a minute gone
by, and `BIMS_ZOOM=0.3` zooms the game view out by that factor once it
is fitted (a scripted wheel does not reach it): the two together are how
the plain and its fog are looked at. `BIMS_TRADE=1` opens the simulation with the station's
trade window up, for looking at the cart; `BIMS_ARMOURY=1` opens it
with the armoury window up, for looking at the lockers' grid — a
`press`, `move`s and a `release` in `BIMS_POINTER` drag a thing across
it. `BIMS_GRAVES=n` leaves `n` of the station alongside **dead where
they stand** and builds its room again over the bodies (feature 85,
`World::lay_graves_for_probe`), for looking at the dead lying on a
station's deck without fighting, flying away and coming back —
`BIMS_GRAVES=4 BIMS_ZOOM=0.6` is the aftermath from far enough off to
see it. `BIMS_LAMPS_OUT=n` shoots the `n` lamps nearest the crew member
out at open and leaves the next one failing, for looking at the dark
round a lamp that is out and a failing lamp's flicker (`BIMS_FIGHT=1
BIMS_LAMPS_OUT=3` is the lobby dark). `BIMS_CLASS=engineer` (a name from
`names::CLASS_NAMES`, or its place in `world::Class::ALL`) puts the
class on slot 0 of any launch (`dev::class_crew`) — and on the setup
tab's chooser — and `Q`/`E` in `BIMS_KEYS` press the class's two keys
over the deck tile under `BIMS_POINTER` (features 74, 75 and 76: an
engineer's sentry and sandbags — laid four minutes of the clock later,
so `4` for 24× first — a soldier's grenade and brace, `BIMS_CLASS=soldier`;
a grenade bursts two seconds after `Q`, so `Q` at frame 60 is a burst at
about 180 at 1×; a medic's surge and heal beam, `BIMS_CLASS=medic`, whose
`E` wants the pointer **over another crew member** rather than over a
tile; a tank's taunt and wall, `BIMS_CLASS=tank`, whose two keys want
nothing under the pointer at all — `BIMS_CLASS=tank BIMS_LEVEL=3
BIMS_KEYS="60:E,90:Q"` on `combat` puts the shield ring and the taunt's
dashed radius in one picture; a commander's rally and squad orders,
`BIMS_CLASS=commander`, whose **E** wants an enemy under the pointer and
whose `X` and `Z` — the squad's other two keys, feature 78 — want a deck
tile or nothing at all: `BIMS_CLASS=commander BIMS_LEVEL=3
BIMS_KEYS="60:Z,90:Q"` on `combat` is the squad held and the rally
called, with the aura's ring round him throughout). Hunting a crewmate with a scripted pointer is a poor way to look
at a beam, so **`BIMS_BEAM=1`** stands crew member 1 a tile from the
medic with a wound on it and links the beam
(`World::beam_for_probe`, `dev::beam_crew`), and `BIMS_BEAM=surge`
triggers the surge over that — `BIMS_CLASS=medic BIMS_BEAM=surge
BIMS_SMOKE_FRAMES=90` on `combat` is the beam's line and both halos in
one picture.
`BIMS_SMOKE_FREE=1` drops the sixtieth-of-a-second pacing a smoke run
holds itself to, so the "ms a frame" it prints is what the machine
actually took rather than a sixtieth — the one way to measure a heavy
frame from a terminal, and what the droid waves were measured with.
`BIMS_SOUND_LOG=1` prints
every clip as it is played and every bed as it fades up or out, which is
how a sound is *heard* from a terminal — `BIMS_SOUND_LOG=1 BIMS_FIGHT=1 BIMS_SMOKE_FRAMES=900 bims
combat | grep ^sound:` is a fight's worth. Move at least a frame before
clicking — egui hit-tests a click against the widgets laid out on the
previous frame. That is how a change to a screen is *looked at* from a
terminal: run it through `./hidden`, read the PNG —
`BIMS_SMOKE_FRAMES=60 BIMS_SCREENSHOT=shot.png ./hidden target/debug/bims
simulation`. **A Wayland window cannot be hidden**, so `./hidden` gives
the run a display nobody can see: it starts a headless weston (in
`shell.nix`; the script re-enters the shell for it) on a memory output
of `BIMS_WINDOW`'s size (1400x900 unless asked), points `WAYLAND_DISPLAY`
at it, drops `DISPLAY` so nothing can fall back to the desktop, and takes
the compositor down with the command; its kiosk shell gives the one
window the whole output, so the picture is exactly that size and
`BIMS_FULLSCREEN` is not needed. A run *not* through it flashes up on
the desktop as a window named `bims-smoke` (`dev::window_name`; the
game's is `bims`). Two things about a smoke run itself: a headless
output has no vblank to wait for, so a smoke run is opened without vsync
(`dev::present_mode`) and paces itself to sixty frames a second in
`smoke_exit`, since the screens step by real time; and a scripted click
goes out as a `WindowEvent` as well as a typed message, because bevy_egui
reads the former and Bevy's own input the latter. `BIMS_SAVES_DIR` moves
the saves, and `./check` points a smoke run's at `target/check/saves`.
**A game with company** is two such runs — two `./hidden`s, a compositor
each — against a relay: `BIMS_SERVER=ws://127.0.0.1:18792`
with `PORT=18792 target/debug/bims-server` up, `BIMS_AUTO=create` on the
host (it prints `lobby: <code>`) and `BIMS_AUTO=join:<code>` on the guest —
the host starts once `BIMS_AUTO_PLAYERS` (2) are in, everybody accepts a
second into the yard, and the world opens on both. `BIMS_NAME` is the
player's name, `BIMS_BIM_NAME` the Bim's and `BIMS_BIM_HAIR=mohawk:red` (a
style and a shade, by name or place in `Hair::ALL`/`Shade::ALL`) its
hair (feature 62); `BIMS_BIM_TINT=amber` (a name or a place in
`Tint::ALL`) is the **colour of the ring under the player's own Bim**
(feature 84) — one a player, a colour another player in the lobby has
taken is not offered, and a bot has no ring at all. A `BIMS_POINTER` moved
over one window's deck is the ghost pointer on the other's screenshot
(feature 60). Give the guest more frames than the host, or the host's
picture has "has left" on it.


## What a wave of machines costs (feature 83)

`DROID_WAVE_MAX` is sixteen because a machine is a body stepped, a stand
scored and a line traced, not because sixteen is the right number of
enemies. The measurement, taken in a **release** build on this machine:

| wave | world step | frame, unpaced |
| --- | --- | --- |
| none (`combat`) | — | 9.2 ms |
| 16 | 0.185 ms | 9.2 ms |
| 32 | 0.292 ms | 9.0 ms |
| 64 | 0.506 ms | 9.9 ms |

The step is `cargo test --release -p world -- --ignored --nocapture
droid_waves_cost` (`tests_droid::droid_waves_cost_this_much_a_step`),
which forces the size with `World::set_droid_wave_for_probe` — raising
the cap alone never makes a wave bigger than the formula does, which at
fourteen crew is sixteen. A world step is a sixtieth of a frame's budget
at 1× and the whole of it at 48×, so even sixty-four machines are well
inside it: the cost is **linear in the wave**, which is what the
staggered planning buys (`World::build_wave` spreads each machine's
first `plan_wait` over `PLAN_EVERY`, since a stand is scored against a
lattice of every free cell within reach of every target and sixteen of
those in one step is that walk sixteen times).

The frame is the other half, measured from a window:
`BIMS_SMOKE_FREE=1 BIMS_DROID_WAVE=n BIMS_SMOKE_FRAMES=400 ./hidden
target/release/bims droids`, `BIMS_SMOKE_FREE` being what drops the
sixtieth-of-a-second pacing a smoke run otherwise holds itself to. **A
frame is the picture, not the machines**: `combat`'s own fight with no
machines in it draws in the same nine milliseconds, and sixty-four of
them add about one. So sixteen is not where the cap has to be — it is
where it is until somebody has a reason to move it, and the reason will
be how a fight *plays* rather than what it costs.

## `AGENTS` is how many of you there are — read it first

`AGENTS` at the root is a plain text file holding **one number**: how many
agents are working on this tree right now. Before doing anything else,
read it, add one, write the new number back, and remember it as yours:
`1` means you are alone, `2` means you are the second alongside one that
is already at work. That number is **awareness and nothing else** — it
says how many hands are in the tree so you re-read a file before editing
it and keep out of the others' way. **No agent runs `./check` for the
tree**, whatever its number: you test your own change and nothing more
(see "Verifying a change"), and the player launches an agent whose whole
task is the full check when they want one. So do not wait on another
agent's checks, do not run the others' crates' suites for them, and do
not ask who is highest.

**Putting the number back is also closing every background shell.** A
session that waited on a long build or a `./check` leaves watchers and
`until … sleep` loops behind, and one that greps for a string its own
command line contains never exits at all — it sleeps for the rest of the
day. So at the end of the session, with the same hand that writes the
number back: stop every background task you started, and check that
nothing of yours is still running (`pgrep -af "sleep "`, the task list)
before saying you are done. Leave the other agents' processes alone —
the live list of who else is at work is `agents.sh status`.

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
- **The Esc sheet is `settings.rs`**: six pages in one window — the
  menu (the UI scale, a button each for Audio and Controls, and Save,
  Load and Restart), the audio page (`sound::Mix`: master, effects, ambience,
  mute), the controls page (every key, rebindable), and the save,
  load and restart pages (`save.rs`). The restart page asks before it
  throws a run away, and what may be done is one `Allowed` the screen
  hands in: no save without a world, no load and no restart on a guest.
  A screen holds it as `Option<Sheet>`, opens it
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
  the one thing the sheet keeps between runs so far. **F and T are the
  two orders every player has for the bots that follow them** (feature
  84): F arms the pointer — the system's cursor goes and a red
  crosshair takes its place — and the next click on the deck puts an
  **attack banner** down there for the crew to fight their way to; T
  calls them back to the ship — a blue **defend sign**
  (`theme::defend_banner`) stands on the spot they gather on, the deck
  just inside the ship's own airlock, for as long as the order does;
  either key again lets them follow again,
  and Esc or a right-click puts the armed pointer away. That is what
  moved the camera's Follow onto **V**. Tab is the
  Inventory action: it opens and shuts the crew member's inventory —
  and opens with it the nearest thing within reach, a container or a
  body down, the rest of them a click away on the **Nearby** strip over
  the window (`CrewPanels::nearby`, a `Near` list the screens rebuild
  every frame off the room's `within_reach` and the world's
  `in_reach_of_body`). `BIMS_KEYS` knows `Tab` and `Space` by name and every letter the
  bindings use — `T` and `V` among them, which it did not until feature
  84's two orders were looked at from a terminal — and `+Shift` at one frame with `-Shift` at a later one holds Shift across a click between them — a Shift order, feature 69 (bevy_egui reads the modifier a frame late, so leave a frame or two each side).
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
  open those windows for a screenshot, `=plunder` the enemy's shelf
  (`CrewPanels::plunder_window`, the same grid with nothing movable,
  over `World::plunder_alongside` — `crates/world/CLAUDE.md`, "An
  enemy's shelf is loot"; with `BIMS_RAID=1` it is the raider's), and
  `=workbench` the one
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
- **The class's two keys have two boxes at the foot of the canvas, and a
  level asks outright** (feature 80, `screens/game.rs`). `ability_boxes`
  reads one `AbilityBox` a key off the world every frame — nothing kept
  between frames — and `ability_bar` lays the two out centred on the
  canvas, clamped clear of the tray at its left; each box is the key in
  one corner, the picture in the middle, **how many are left** in the
  other, the name under it and the tip on a hover. What is counted is
  the world's (`World::{sentries_left, kits_of, grenades_of,
  beam_patients, squad_members}`) and what greys a box out is
  `world::class::key_level` — every class's **E** from the first level
  and its **Q** from the third — with the cooldown over the picture and
  the surge's charge as a bar along the foot. The pictures are the
  deck's own marks (`theme::{surge_mark, wall_mark, taunt_ring,
  aura_ring, squad_mark, heal_beam}`, and `brace_mark` which is the
  box's alone), or the thing itself out of `icons.rs` for a kit and a
  grenade, so a box and what the key does are one picture. The box
  never decides anything: `class_key` is still what the press goes
  through, and it knows about the pointer as well.
- **A level is spent on the Skills tab, which is a tree like the
  research's** (feature 83, `CrewPanels::skills`). The class's ten levels
  run down the tray, numbered, with the spine beside them lit as far as
  the level reached: a level with nothing to choose is one slot across
  the width (`names::level_name`, the same three levels `level_line`
  describes), and a pick level is two side by side. A box is coloured for
  its state — learnt, **given up** (struck through: the other side of a
  level chosen at), open for a point, or waiting on a level — and over
  the tree is how many **skill points** are left, which is every pick
  level reached and not chosen at. A click picks a slot and the column
  **beside** the tree — `DETAIL_WIDTH` wide, to the right rather than
  under it, since the tray is anchored at the foot of the window and
  grows upwards and a block under the tree would shove the tree out
  from under the pointer — says the slot's name, the level it sits at,
  **what it is worth in numbers**, what it does in words and what state
  it is in, with the *Learn* button for one that is open. The numbers
  are the point of it: `names::talent_numbers` for a pick and
  `names::level_numbers` for a fixed level say every talent's figure
  **and the base it moves** off the rules crates' own constants — a
  sentry's 60 health to 90, a beam's 6 tiles to 9, a grenade's 2-second
  fuse to 1 — so a choice between two slots is a choice between two
  numbers rather than between two adjectives; `crew::skill_numbers`
  picks which of the two is asked. A talent that multiplies something
  the weapon in hand decides says the factor and one worked figure. The
  type on the tab is `SKILL_TEXT`/`SKILL_BOX_TEXT`/`SKILL_NAME_TEXT`
  rather than the panels' small, and the tree's geometry is
  `skill_tree_size` off `SKILL_GUTTER`/`SKILL_BOX_W`/`SKILL_BOX_H`. The
  pick is the same
  `Order::PickTalent` the crew panel's row sends — the panel's row is
  still there — and the tree asks nothing of the world it is not handed:
  `ClassView` (`picks` as well as `talents` now) and `world::class`'s own
  tables are all of it. `GameScreen::skills_prompt` opens the tray on the
  tab the first frame a point is actually waiting — set by a
  `WorldEvent::LevelUp` of the local slot and by an open, so a run that
  starts part-way up the tree shows the tree at once. Nothing is forced
  after that: no window stands over the deck.
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
  station's, the engines, and a planet's air by its biome — temperate,
  desert, arctic — set down at a settlement, `Bed::of_biome`) loops the whole time at whatever level the
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

**Test your own change, not the whole game.** An agent working on this
tree runs what its own change touches and stops there: the crates it
edited (`cargo test -p <crate>`, `cargo fmt`, `clippy -p app --no-deps`
if it touched the app) and, if the change is something on the screen, the
one smoke run that shows it (`./check smoke`, or a `BIMS_SMOKE_FRAMES`
run through `./hidden` with the dials your feature needs). That is what
"done" means for you. **Do not run `./check` or `./check full` over the
whole tree** unless your task asks for it in so many words: several
agents share this directory, the tree holds their half-finished work as
well as yours, and a red step in a file you never opened is theirs, not a
bug you have to chase. The player launches a **test-all agent** when they
want the whole thing checked — leave the sweep to it, and say in your
report which crates you did run so that agent knows what is already
covered. `./check <step>...` on the steps your change touches is fine
when you want one of them; the sweep is what is somebody else's.

**`./check` runs all of it.** The quick tier — `./check`, **under a minute
on a warm tree** — is the git-tree check, `cargo fmt`, the workspace
build, `cargo test` **bar `world` and `ship`**, `bims --self-check` for the
two pinned numbers, clippy on the app, and a smoke run of **five windows**
— the menu, the room, the world, the yard and a fight — sixty frames each
on `./hidden`'s headless compositor, all five at once, with a screenshot
of the last four left in `target/check/`;
`./check full` **deepens every step of it and then** adds the native probes (compiled fresh into `target/probes/`,
never the stale binaries in `scratchpad/`, each under `PROBE_SECONDS` of
its own so one that stops finishing fails rather than hanging the check)
and `nix flake check`. A probe steps the room with `Game::simulate` and
never with `update`: the picture costs some hundreds of times what the
step does and nothing in the simulation reads it — see
`scratchpad/CLAUDE.md`, "A probe simulates; it does not draw".

**What the quick tier leaves out, it leaves out by the clock.** Measured
warm on this machine, a full quick run was 330 seconds and **272 of them
were the tests**, of which `world` is 170 and `ship` 61 — and there is
nothing in `world` to skip by name: 243 of its 275 tests take between
three and ten seconds each, because every one of them steps the clock. So
the quick tier's `test` step is `cargo test --workspace --exclude world
--exclude ship` (`SLOW_CRATES` at the top of `./check`, printed beside the
count so nobody mistakes a quick run for the whole workspace), and what
stands in for the two missing suites is the new **`pins` step**, `bims
--self-check`: `design_hash` and `world_checksum` against the constants in
`shipdesign::fixture` and `world::fixture`, which is what a rules change
moves first. `./check full` runs both crates and sets **`BIMS_SWEEP=1`**
with them, which turns the station-navigation walk in
`crates/world/src/tests.rs` back into every plan on every kind at every
seed — sixty-five layouts walked tile by tile, more than every other test
in the workspace put together — where a plain run walks a slice of the
matrix with the same assertions. A new test that sweeps a matrix belongs
behind that variable too. **So a change to `world` or `ship` is not
checked by `./check` alone**: run that crate's own `cargo test -p …`, or
`./check full`. The windows go the same way — five in the quick tier, all
at once (`SMOKE_AT_ONCE`), since a run is seven seconds and mostly Bevy
starting up; every other window is one of those with a dial moved and is
the full tier's, `restarted` (feature 79) among them.
`./check <step>...` runs a subset, `./check --list` explains each. Full
output is under `target/check/`; only the failing lines are printed. All of
that is written for the **test-all agent**, the one whose task is the
sweep: for it, "done" means `./check` is green, and a **feature** is done
when `./check full` is, since that is where the tests and the windows the
quick tier skips are run. For everybody else it is a map of what the sweep
will cover later, and what is owed is the paragraph above — your own
crates and your own window. Either way, a step that was red before the
change should be said so out loud rather than folded into the report. It re-execs itself
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
