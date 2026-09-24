# Working on Bims

A 2D top-down game: all the simulation is Rust, and so is the window —
`crates/app` is a Bevy binary that draws every crate's shape buffer through
egui and lays the panels out round it. `README.md` explains how the game
itself fits together; this file is about working on it.

## Running it

`nix run .` is the one command: it **builds and opens the window**. There are
nineteen things to run, and each is a name rather than a flag — and
`cargo run -- list` prints every one of them with a line each, which is
the build's own answer where this table is a copy:

| command | `cargo run` | opens |
| --- | --- | --- |
| `nix run .` / `nix run .#game` | `cargo run -- game` | the whole game in order — menu, setup or lobby, world and station, then **the run** (feature 102): no design phase, the default ship (the playtest ship) docked where you said and `START_MONEY_PER_BIM` (5 000) a player's Bim in the pool (`Session::run`, `screens::designer::start_run`) — see *A run* below |
| `nix run .#simulation` | `cargo run -- simulation` | straight into the world on the playtest ship |
| `nix run .#design` | `cargo run -- design` | straight into the yard, the playtest ship given, docked where the simulation docks — how a change to the designer is looked at |
| `nix run .#room` | `cargo run -- room` | the behaviour test room — Bims on a deck |
| `nix run .#test` | `cargo run -- test` | the simulation somewhere else each time — docked at a random station somebody lives on, in a random galaxy, on the **combat ship** with one crew member (four bunks to spare) and a **mercenary for hire** at the dock whatever the roll said (`Session::mercenary_for_probe`) |
| `nix run .#test_planet` | `cargo run -- test_planet` | `test` **set down on a planet**: the same random galaxy and roll, made among the systems whose first planet with ground has friendly people (`world::spawn_with_ground`, `ship::session::pick_ground`), and the ship landed at its settlement the way `BIMS_LANDED=1` lands the simulation (`Session::land_for_probe`) — the mercenary asked for first, so the settlement's room has one too |
| `nix run .#tier2_test` | `cargo run -- tier2_test` | `droids` with **everybody's kit at tier two** — every crew member's gun at it (its kind as the fight dealt it) and a fresh helm, kevlar and leg guards at it on, pieces of the world's — and the machines at tier two (`Session::droids_at_tier`, `World::outfit_for_probe`), so the fight is looked at with nothing at tier one on either side. It was the human garrison's fight until every enemy was a machine (feature 102) |
| `nix run .#tier3_test` | `cargo run -- tier3_test` | the same at **tier three** |
| `nix run .#droids` | `cargo run -- droids` | **the fight** — the **machines** (feature 83): the **combat ship** (`shipdesign::fixture::combat_ship`, the playtest ship with bunks and chairs for five) with **sixteen crew** (`COMBAT_CREW`: five at the bunks, eleven standing on the deck), a gun in every hand — the five kinds dealt round — the **last four of them hired field medics** (`session::COMBAT_MEDICS`, feature 86), docked at the spawn rebuilt as the **arena** (`world::station::arena`, 72 tiles across) that the **droids hold**. `Session::combat` builds the ship, the crew and the arena and `Session::droids` hands the arena to the machines; the `combat` command that stopped at the first half — the arena's own people turned against the crew — is gone with every other human enemy (feature 102), and so are its `combat_<class>` runs and `--combat`. Every dial the notes below write against `combat` is `droids`' now, the same ship and crew; `BIMS_FIGHT`, which stages a fight with a station's *people*, is the simulation's alone. Its people are gone (`World::people_of` is nought for a held station) and a wave of machines stands about it instead: Wardens a sixth, Husks a third, Troopers the rest, sized by `droid::wave_size` (the base, the crew, the calendar, the worth and the levels, added — never doubled — and capped at `DROID_WAVE_MAX`). `DROID_REINFORCE_STEPS` is **a minute of the mission clock** here — a real second at 1× — rather than two real minutes, so the next wave is watched landing at the far airlock rather than waited for, and the station has **three waves** rather than the formula's two at day nought (`DROID_WAVES_IN_PROBE`), since one wave landing and then a cleared station is not what these commands are for — the red line along the top says which wave is on the deck, how many of it are standing, and, the moment the last of them is down, **how long until the next lands**. `BIMS_DROID_TIER=2` brings them at a tier, `BIMS_DROID_WAVES=5` gives the station that many waves, `BIMS_DROID_REINFORCE=600` makes the wait between them that many minutes of the mission clock — a minute is a real second at 1× and two and a half *frames* at 24×, so the countdown cannot be caught by a scripted run without lengthening it — and `BIMS_DROID_WAVE=32` makes a wave that many whatever the formula says, which is how the measurements below were taken; `BIMS_DROIDS=1` replaces the wave with a **showcase** — a row a kind and a column a state: idle, firing or striking, arms at nothing, legs at nothing, destroyed — so all fifteen drawings are one screenshot (`World::stage_droids_for_probe`) |
| `nix run .#combat_droids_engineer` … `#combat_droids_commander` | `cargo run -- combat_droids_medic` | that **same fight with the class in hand** (features 79 and 83, `Launch::DroidsAs`): one command a class — `Class::ALL` bar `None`, spelled as `names::CLASS_NAMES` spells it, lower case — and nothing else about the run differs: the same combat ship, the same sixteen crew, the same droid-held arena and the same two dials, with `World::set_class(0, …)` on top (`dev::class_crew`, which takes the command's class and lets `BIMS_CLASS` override it). It opens at the **tenth level** (`dev::COMBAT_CLASS_LEVEL`, feature 80) with all seven of the class's talents still to choose, so the tray opens on the **Skills** tab (feature 83) with seven points to spend; `BIMS_LEVEL=n` says otherwise. The **engineer of such a run has the charges its class deals it** (`world::deploy::SENTRY_CHARGES`, one, beside `SANDBAG_CHARGES`, three). The `combat_<class>` runs beside these were the human garrison's fight, and went with it (feature 102). The parsing is `main.rs::class_named`, and `bims list` prints the lot |
| `nix run .#droids_planet` | `cargo run -- droids_planet` | `test_planet` with the **town** droid-held: the same random galaxy and roll, the ship set down at the settlement, and the settlement's people replaced by the machines, whose lander sets down on the plain beyond the north gate for an odd wave and the south for an even one. The same minute's reinforcements and the same two dials |
| `nix run .#crisis` | `cargo run -- crisis` | the **crisis** a day before it first spreads (feature 92): `test`'s own random galaxy and random dock and the machines' origin forced **two hyperlane hops** from the crew's own star (`Session::crisis_for_probe`, `session::CRISIS_HOPS`) where the roll's own floor is eight. The origin is theirs from day nought, as in every run since feature 102, and the clock is wound to the eve of the day the ring round it turns (`DROID_SPREAD_DAYS`, five) — so the next stars turn red on the galaxy chart within a day of the clock — a day the crew have to travel, since only travel moves the world clock (feature 103) — and the crew's own system five days after that. The chart is where it is looked at: the lanes are drawn faintly under the stars, an infested star is crossed in the enemy's red **charted or not**, and the panel says under the star you pick which day it is due (`screens/game.rs::crisis_line`). `BIMS_CRISIS_DAY=n` moves the day the origin turns, and the clock opens a day short of the next ring whatever it says, so the dial is about what the *rest* of the galaxy's days come out at rather than about how long to wait |
| `nix run .#jammer` | `cargo run -- jammer` | the **jammer** (feature 93): `crisis`'s own random galaxy, random dock and origin **two hops off**, with the clock wound *past* the day this system falls rather than a day short of the first — so the crew open **inside** an infested system, every station of it in the machines' hands (`Session::jammer_for_probe`, `World::infest_here_for_probe`), a wave aboard the one they are tied up at and `DROID_REINFORCE_STEPS` a minute of the mission clock. Two things are looked at from here. The **jam**: the chart lights the lanes out of the ship's star in the hyperdrive's violet, draws the route to whatever star is picked along them, and **bars in red every step of it a jammer would turn back** — a jump *inward*, towards where the machines began, is refused while the jammer station stands (`Refusal::Jammed`), and the panel says which station holds it. And the **tier**: two hops is inside `DROID_TIER_THREE_HOPS`, so the machines come at **tier three** without a dial. `BIMS_DROID_TIER=1` says otherwise, and `BIMS_DROID_WAVES`/`BIMS_DROID_REINFORCE` are `droids`' own |
| `nix run .#defense` | `cargo run -- defense` | **defending a town** (feature 94): `test_planet`'s own random galaxy and roll — the ship set down at a settlement whose people are friendly — with the machines' origin forced **one hyperlane hop off** and the crisis's first day wound to nought, so the town's system is on the **front** (`World::front` of it is one) and the town is *threatened*. The map says so under its planet's icon, in the enemy's red, where it would otherwise say *land*. A minute of the mission clock after the landing (`DEFENSE_DELAY_STEPS` is sixty of them in the game, a real minute at 1×; `BIMS_DEFENSE_DELAY=n` minutes over the command's own one) a wave sets down outside a gate and walks in, and the red line along the top counts it the way it counts a held station's. The fight is the one that happens **inside one room**: the town's **guard and its mercenaries** take arms and fight the machines where they stand, everybody else walks into the nearest house and stays there, the crew never aim at a townsperson and the machines aim at both. Hold the last wave and the town is **held** — friendly for good, trading and hiring even after its system falls, its map tag *held* — and some of its people join the crew; go back to the ship before the last wave is down and the town falls to the machines (feature 103). `BIMS_DROID_WAVES`/`BIMS_DROID_REINFORCE`/`BIMS_DROID_WAVE` are `droids`' own (`Session::defense_for_probe`) |
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
a point are a notch of the wheel, which zooms. `BIMS_LANDED=1` opens it set down on
the spawn system's first planet with ground — the pad, the ground and
the settlement beside it — and `BIMS_LANDING=0.7` over that planet with
the landing run seven tenths of the way down, for looking at the descent
(feature 52, `crates/world/CLAUDE.md`); `BIMS_FIGHT=1` opens the simulation
with the dock made hostile by hand, the crew member recruited inside the
station's door and one of its people a few tiles down the corridor — the old
game's fight with **people**, which no run meets since feature 102 and which
is only staged where a station has people (a droid-held arena has none, so
it does nothing on `droids`). `BIMS_RAID=1` — a raid is the old game's too,
and asking for one switches the world's human foes on — opens the simulation off its berth in
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
see it. `BIMS_DYING=n` puts `n` of the crew **in a dying state** — a
part of each taken to nothing, so its trauma is rolled and untreated,
and wounds open on it besides (`Session::maim_for_probe`, a different
part each so a crew of three shows three different traumas) — for
looking at the **red cross** over a body on the deck and at the **peril
block** under the health bar; the player's own Bim is left out of it
unless `n` reaches the whole crew, so the picture is taken from
somebody still walking about (`BIMS_FIGHT=1 BIMS_DYING=3` on `combat`).
`BIMS_FIELD_MEDIC=n` makes the **last `n` of the crew hired field
medics** (feature 86, `Session::field_medics_for_probe`) — the contract
and a medic's charges of medicine, no money taken — the last rather
than the first since slot 0 is the player's own and the rescue is a
*bot's* branch (`Game::bot_stand`). **`combat` already sails with four
of them** (`session::COMBAT_MEDICS`), and so does everything built on
it — the tier tests, `droids`, the `combat_<class>` and
`combat_droids_<class>` runs — since a fight with nobody who may carry
is a fight where a body down stays where it fell; the dial asks for the
same crew members from the same end, so setting it at four or under
changes nothing and setting it higher reaches further up the crew. `BIMS_CARRY=1` takes a crew member out cold and
puts it in the arms of somebody who may carry, for looking at a body
being carried off the deck without waiting for a fight to put one there.
Both want **more than one aboard**, so they are `combat`'s and not the
simulation's, which sails with a crew of one: `BIMS_CLASS=medic
BIMS_CARRY=1 bims combat` is a body in the arms (the Carry box lit, with
no count on it, is what says so), and `BIMS_FIGHT=1 BIMS_FIELD_MEDIC=1
BIMS_DYING=2 BIMS_SMOKE_FRAMES=600` is one going and fetching for
itself. `G` is the carry's key in `BIMS_KEYS`.
**`BIMS_BANDAGES=n`** puts exactly `n` dressings in **every** crew
member's pack and starts the bandage cooldown afresh, and
**`BIMS_MEDKITS=n`** the same for the medkit: since the medicine became
**everybody's charges** — a medkit and five bandages each, a medic four
and ten, back a minute and thirty seconds of the clock after each is
used (`class::Charge::{Medkit, Bandage}`, `crates/world/CLAUDE.md`,
"The medicine is everybody's charges") — nought is the empty box at
the foot of the canvas with its **sweep** running, and `BIMS_BANDAGES=2`
a part stock with the **ring** round its count filling. Five go in one
box over two cells by two, so `BIMS_BANDAGES=7` is a full box beside a
part one, which is how the count in a cell's corner is looked at —
`BIMS_ARMOURY=1 BIMS_BANDAGES=7` on the simulation, or `BIMS_FIGHT=1
BIMS_BANDAGES=7 BIMS_DYING=2` for a crew that binds its own wounds as it
runs; `BIMS_MEDKITS=0 BIMS_BANDAGES=2 bims combat` is the two medicine
boxes' two states in one picture.
**`BIMS_KITS=n`** does the same for the engineer's two **charges**
(feature 88): exactly `n` of each kit in every pack and both cooldowns
started afresh. `BIMS_KITS=0 bims combat_engineer` is the one state a
scripted run cannot walk itself into — no charge in hand and the whole
wait ahead — so the seconds in the corner of the two boxes at the foot
of the canvas (`59s` for the sentry, `44s` for the bags) are a
screenshot rather than a pointer hunting a Bim that is walking away.
**`BIMS_GRENADES=n`** is that dial for the soldier's **grenade
charges** (feature 90): since a grenade is a charge on a cooldown like
a kit — two of them, thirty seconds each, and **no class makes
anything to use a skill** — `BIMS_GRENADES=0 bims combat_soldier` is
the `29s` in the corner of the Q box, and `BIMS_GRENADES=1` a soldier
with one throw in hand and the next on its way.
**`BIMS_CRISIS_DAY=n`** is the `crisis` command's own (feature 92): the
machines' first star turns on day `n` instead of `DROID_FIRST_DAY`, and
the clock opens a day short of whatever it says — so the dial is about
what the *rest* of the galaxy's days come out at (five a hop after it)
rather than about how long to wait. `BIMS_CRISIS_DAY=0 bims crisis` opens
with the origin already red on the chart, two hops from the crew.
**`BIMS_DROID_TIER`** is the one dial the `jammer` command changes the
meaning of (feature 93): unset is no longer "tier one" but **the
distance rule** — tier three within `DROID_TIER_THREE_HOPS` (two) hops
of the machines' origin and tier one beyond — so `BIMS_DROID_TIER=1 bims
jammer` is the way to see a wave that is *not* at tier three, and
`bims droids` is unmoved, its arena being nowhere near the origin.
**`BIMS_DEFENSE_DELAY=n`** is the `defense` command's own (feature 94):
how long after the crew set down at a threatened town the first wave
lands, in minutes of the clock, where the command's own is a minute and
the game's own is `DEFENSE_DELAY_STEPS` (sixty minutes of the mission clock, a real minute at 1×). Raise it to look at
the hour the crew have to walk the town, trade and hire before the
shooting starts; `BIMS_DROID_WAVES=1 bims defense` is a fight that can
be held to the end in one sitting.
**`BIMS_FREEZE=n+f`** pauses the game `f` frames after the crew's room
hears its `n`th shot or blow, and **`BIMS_FREEZE=down:n+f`** after the
`n`th machine destroyed (feature 98): a muzzle's glow, a bolt's flash
and a cut are a handful of frames each and a fight's timing moves from
run to run, so a frame count cannot catch one — and a pause holds the
passing lights still, so the screenshot taken later is that instant.
`BIMS_FIGHT=1 BIMS_WEAPON=sniper BIMS_ENEMY_WEAPON=sniper
BIMS_FREEZE=2+1 bims simulation` is two beams crossing.
`BIMS_LAMPS_OUT=n` shoots the `n` lamps nearest the crew member
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
medic with a wound on it, **its blood at three fifths** and links the
beam (`World::beam_for_probe`, `dev::beam_crew`), and `BIMS_BEAM=surge`
triggers the surge over that — `BIMS_CLASS=medic BIMS_BEAM=surge
BIMS_SMOKE_FRAMES=90` on `combat` is the beam's line and both halos in
one picture. The blood is short on purpose (feature 91): a beam stops
the bleeding dead, so a patient wounded and beamed in the same breath
sits at full blood for ever and the **green numbers** over it never
count anything. At 1× a beam puts back half a point a second, so a
`+1` every two seconds; `3×` on the strip is one every two thirds of
one, which is how they are looked at in a short run.
`BIMS_SMOKE_FREE=1` drops the sixtieth-of-a-second pacing a smoke run
holds itself to, so the "ms a frame" it prints is what the machine
actually took rather than a sixtieth — the one way to measure a heavy
frame from a terminal, and what the droid waves were measured with.
**`BIMS_PERF=1`** says where that frame went (feature 96): the world's
steps, the panels' layout, the shape buffer, the tessellation and the
words over it, each timed and printed as a table when the run exits, with
what is left over for Bevy and egui — *Where a frame goes* below is what
it is for and what it said. The two go together, since a paced frame is a
sixtieth however heavy it is. Under the table it prints the **GPU's**
time for each pass Bevy times itself — the main pass the world's canvas
is drawn in, the bloom, the copy to the window (`perf: gpu …`, off
Bevy's `RenderDiagnosticsPlugin`, which `BIMS_PERF` adds) — where the
device has timestamp queries; egui's pass is not one of them.
**`BIMS_BLOOM=0`** turns the bloom off and the HDR target with it
(feature 97, *Bloom* below): the same picture drawn the same way with
the glow and nothing else taken out, which is how the two are compared
— and what a GPU that would rather not is given.
`BIMS_SOUND_LOG=1` prints
every clip as it is played and every bed as it fades up or out, which is
how a sound is *heard* from a terminal — `BIMS_SOUND_LOG=1 BIMS_FIGHT=1 BIMS_SMOKE_FRAMES=900 bims
combat | grep ^sound:` is a fight's worth. **A smoke run is silent**:
it opens with the audio page's mute ticked (`dev::silent`), and
`./hidden` hands whatever it runs `BIMS_SOUND=0`, so `./check` and every
agent's window make no noise — the log prints all the same, the mute
being after it. `BIMS_SOUND=1` is how one is heard, and `BIMS_SOUND=0`
mutes an ordinary run. Move at least a frame before
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

## Where a frame goes, and what a fight cost (feature 96)

**`BIMS_PERF=1` times the named parts of a frame** and a smoke run prints
what they came to when it exits, beside the frame time it already prints
(`crates/app/src/perf.rs`). It is a dev dial like the rest: off — which is
every ordinary run — a scope is an atomic load and a branch, so the
normal build carries no clock. The first `perf::WARMUP` (100) frames are
thrown away and the timers started again after them, since a window's
first frames are Bevy coming up, the canvas being fitted and the room
being laid out. The measurement is

    BIMS_PERF=1 BIMS_SMOKE_FREE=1 BIMS_SMOKE_FRAMES=400 \
      ./hidden target/release/bims <command>

— `BIMS_SMOKE_FREE` being what drops the sixtieth-of-a-second pacing, so
the number is what the machine took. The report is a tree: `frame` is the
screen's whole system and the rows under it are parts of it, so what is
left between `frame` and the wall clock is **Bevy and egui** — input,
egui's own tessellation of the panels, and the meshes handed to the GPU —
which is not ours to put a scope inside of, and is the one thing here that
could not be measured from within. The render thread is pipelined with the
app thread besides, so that row is the app side of it and not the GPU's.

**What a fight's frame was made of**, `combat` in a release build on this
machine, 1400x900, before anything was changed:

| part | ms a frame | share |
| --- | --- | --- |
| the shape buffer (`Session::render`) | 4.27 | 47% |
| tessellation (`shapes.rs`, 15 300 shapes) | 2.10 | 23% |
| bevy and egui | 1.97 | 21% |
| the panels' layout | 0.30 | 3% |
| the world's steps | 0.37 | 4% |
| everything else on the screen | 0.18 | 2% |

So **the picture is the frame and the simulation is not**: at 1× the
world's steps are a twenty-fifth of it, and even at 48× on the simulation
they are 2.4 ms against the picture's 3.6. Inside the shape buffer, the
crew's **room's own draw** was 2.37 ms of the 4.27 and `world_paint` 1.23
— and inside the room, `Sight::light_map` was 2.3 of that 2.37, against
0.05 ms in the simulation. That is the shape of it: one crew member marches
one fan of rays, fourteen march fourteen, and `combat` and `droids` are the
two commands with fourteen.

**What was done about it** is the march, and nothing else — the same
pixels, the same picture. `Sight::march` takes its callback **by type**
rather than as a `&mut dyn FnMut`: it is called once a *pixel*, some
hundreds of thousands of times for one pair of eyes on a station's deck,
and an indirect call there was a third of the cost. The `RAYS` (4096)
directions are worked out **once** into a table rather than two trig calls
a ray, eight thousand of them for every eye that moved half a pixel. And
`seen` is cleared with `fill` rather than a loop. Nothing about the rule
moved: `sight::tests::the_light_map_is_the_same_picture_it_was` pins both
planes of a marched room by hash, and the numbers in it were read off the
**old** march.

| command | before | after |
| --- | --- | --- |
| `simulation` (docked, 1×) | 3.9 ms | 3.9 ms |
| `simulation` at 24× | 6.9 ms | 6.4 ms |
| `simulation` at 48× | 8.8 ms | 8.1 ms |
| the galaxy chart open | 7.3 ms | 7.3 ms |
| `design` | 4.5 ms | 4.5 ms |
| **`combat`** | **9.4 ms** | **7.8 ms** |
| **`droids`** | **9.3 ms** | **7.2 ms** |

Three runs each of the two fights and two of the rest, the median; the
same machine and the same build bar the one file. The crew of one is
unmoved because one fan of rays was never the cost, and the designer and
the chart draw no room at all.

**Two things left standing, measured and not fixed**, since the task was
the largest cost and not every cost. The **galaxy chart** costs 4.81 ms a
frame in `canvas ui` — 64% of that screen's frame — which is the block at
the top of the chart's arm in `screens/game.rs` walking every star of the
galaxy for what is visited, what is infested and what is reachable, every
frame, and plotting the route again with it; none of that changes between
frames unless the ship moves or the pick does. And the **tessellation**,
2.1 ms, is now the largest single row of a fight's frame: 15 300 shapes
and 183 000 floats a frame, most of them the docked station's hull, which
is rebuilt from nothing every frame though it only moves when the camera
does.

## Bloom, and the world's canvas drawn by Bevy (feature 97)

Real bloom — Bevy's own, on an HDR camera — over the world's canvas, and
nothing else. It wanted the canvas moved out of egui, because **nothing
egui draws can be bloomed**: bevy_egui paints straight into the camera's
target in a pass of its own, after the main pass, and its vertex colours
are bytes, so no egui colour is ever past white. So the canvas between
the panels is a stack of Bevy meshes now (`crates/app/src/scene.rs`),
and the panels, the words and the pointer are egui's as they were — the
pointer untouched, since it is still read out of the one egui context on
the one camera. One frame reaches the window in this order:

1. **The main pass**: the canvas's layers in the order the screen painted
   them — `WorldCanvas::shapes` for shapes, `FogTexture::paint` for a
   fog picture — each a `Mesh2d` a z apart. On the game screen that is
   the world *under* the fog, the plain's fog and the light map, then the
   world *over* the fog: the shots, the rings, the marquee
   (`Session::fog_split`, `crates/ship/CLAUDE.md`). A shot is over the
   fog **now**; under egui it was under it, the fog being laid over the
   whole buffer.
2. **The bloom**, `Bloom` with `BloomCompositeMode::Additive`, threshold
   `BLOOM_THRESHOLD` (1.0) and intensity `BLOOM_INTENSITY` — the named
   constants at the top of `scene.rs`, the one place. **Additive** is the
   point: the energy-conserving mode mixes the whole picture towards the
   blurred one and would dim every wall whether anything glowed or not,
   and with the threshold at white a pixel no glow reaches gets nought
   added. Only a colour **past one** is over it — an emissive one.
3. **No tonemapping**: `Tonemapping::None` (a `Camera2d`'s own default),
   so the pass returns before it binds anything — no palette shift, no
   lookup table wanted, and no pink image from a missing
   `tonemapping_luts`. Past-white is clipped to white on the way out.
4. **egui**, pinned after the whole post-process by `egui_after_bloom`,
   an empty system in the render schedule: bevy_egui orders its 2D pass
   after the main pass and after `bevy_ui`'s, and there is no `bevy_ui`
   in this build, so without the pin its pass was free to run before the
   bloom — and the glow would have been laid over the panels.

**An emissive colour is a channel past one**, in the same sRGB floats a
painter always wrote: `shapes::Paint` keeps it, and the canvas's shader
(`canvas.wgsl`, egui's own fragment arithmetic over again, so a layer is
the picture egui drew) carries the sRGB curve on past white, so 1.5 is
about two and a half times white's light. Everything at or under white is
made exactly as egui made it — through `Color32`, rounded to the byte —
and blended premultiplied as egui blends, clipped to egui's own scissor.
A canvas inside a panel is still egui's, and an emissive colour there is
white. `draw::Color::glowing(by)` is how a painter says it, and **the one
emissive thing in the game so far is the pistol bolt's core**
(`combat.rs`, `BOLT_CORE_TINT`, `BOLT_CORE_HEAT`): its white pulled
towards the side's colour and made 2.4 times as bright, so the core is
drawn white-hot and the air round it glows blue for the crew and red for
the enemy. `BIMS_FIGHT=1 BIMS_WEAPON=pistol BIMS_ENEMY_WEAPON=pistol bims
combat` is where it is looked at, and
`the_pistol_bolt_s_core_is_the_one_thing_brighter_than_white` pins it.

**`BIMS_BLOOM=0`** takes the bloom off and the HDR target with it — the
window's own eight bits, as before — for comparing the two and for a GPU
that would rather not.

**What the picture was checked against.** A paused frame of `combat`,
`droids` at thirty-two, the galaxy chart and the yard, taken with the
build before this and with this, pixel by pixel. With the bloom off,
46–84 pixels of the 1.26 million differ, by one or two levels in 255: the
same picture. With the bloom on and nothing emissive on the screen,
12 000–62 000 pixels differ by **one** level, none to 600 by two (two
more by six in the yard), and ten by fourteen — the one-level ones along translucent edges, where the float
target keeps what an eight-bit one rounds after every blend, and the ten
on the edge of a single icon in the inventory panel, egui's `SHINE`
(`icons.rs`, white over alpha seventy — *additive*), which the old target
clamped part way through the blend and the float one clamps at the end.
No glow and no shift in the palette: the bloom picks up nothing that is
not emissive.

**The features are the ones the app already had.** `2d_bevy_render`
brings `bevy_post_process` (the bloom), `bevy_sprite_render`
(`Mesh2d`, `Material2d`), `bevy_core_pipeline` (`Core2d`, `Tonemapping`)
and `bevy_render` (`Hdr` is `bevy_camera`'s, the render diagnostics
`bevy_render`'s); nothing was added to `Cargo.toml`. Not enabled, and not
wanted: `tonemapping_luts` (and the `ktx2`/`zstd` it needs), `smaa_luts`,
`bevy_ui_render`.

What it cost, a frame, unpaced, release, this machine (a Ryzen 7 3700X
and a Navi 32 Radeon, RX 7700 XT or 7800 XT), 1400x900 — the median of five runs each,
the three builds taken in turn so a drift in the machine lands on all of
them alike:

| | before (egui) | bloom on | `BIMS_BLOOM=0` | GPU: bloom | GPU: main pass |
| --- | --- | --- | --- | --- | --- |
| `combat` | 7.77 ms | 7.73 ms | 7.14 ms | 0.243 ms | 0.113 ms |
| `droids`, `BIMS_DROID_WAVE=32` | 7.24 ms | 7.15 ms | 7.05 ms | 0.243 ms | 0.113 ms |
| the galaxy chart | 7.34 ms | 7.55 ms | 7.50 ms | 0.243 ms | 0.046 ms |
| `design` | 4.41 ms | 4.08 ms | 3.87 ms | 0.243 ms | 0.043 ms |

Three things to read off it. **The frame is the CPU's**, and the bloom is
not on the CPU at all: it is a quarter of a millisecond of the GPU's,
the same on every screen since it is a pass over the whole window, and
Bevy renders on its own thread a frame behind the app's, so it shows on
the wall clock only as noise — a fight's five runs spread 7.0 to 8.5 ms.
**The tessellation is where it always was** (2 ms in a fight): what moved
is who is handed the triangles — `scene::sync`, 0.005 ms, hands Bevy the
arrays without a copy, and the copy into the vertex buffer is the render
thread's, where egui's copy of the same mesh used to be. And **the GPU
column is the new picture's alone**: egui's pass has no timestamps, so
what the old canvas cost the GPU is not a number anybody has.
`BIMS_PERF=1` prints these rows (`perf: gpu …`) — the measurement is the
one feature 96 describes, run with `BIMS_BLOOM=0` and without.

## How fast the crisis crosses a galaxy (feature 92)

The lane graph is three nearest neighbours a star plus whatever a spanning
tree still needs (`worldgen::galaxy::weave`), and the spread is one hop
every `DROID_SPREAD_DAYS` (five) from a star rolled at least
`DROID_ORIGIN_MIN_HOPS` (eight) hops from the crew's own. What that
comes out at, over ten galaxy seeds — `cargo test -p world -- --ignored
--nocapture crisis_spread_over_ten_seeds`, which takes the origin the
way `World::start` takes it, from the dock `spawn_anywhere` picks:

| | min | median | max |
| --- | --- | --- | --- |
| lanes a star | 3 | 4 | 7–8 |
| hops from the origin | — | 35–57 | 96–143 |
| the whole galaxy infested | day 490 | — | day 725 |

(Measured with the first day at ten. Since feature 102 the crisis is there
from day nought, so every day in this section is ten earlier: the whole
galaxy by day 480 to 715, the median star by day 175 to 285.)

Three things fall out of it. **Every star has at least three lanes** and
the busiest has seven or eight — a lane is undirected, so a star out on
the rim is picked by neighbours that were not its own picks. **A galaxy is
about a hundred and twenty hops across**, not thirty, because the arms are
long and the graph is a web laid over them rather than a mesh: the spread
follows the arms. And so **the whole galaxy falls somewhere around day
500 to 725**, with the median star at day 185 to 295 — the crisis is a
season rather than a raid, and a crew that flies *away* from it buys
itself years. If it ever wants to be faster, the number to move is
`DROID_SPREAD_DAYS`, not `LANE_NEIGHBOURS`: a fourth lane a star cuts the
hop count hard and makes the galaxy a fortnight wide.

## How many systems the machines have to build a jammer in (feature 93)

Every infested system has exactly one jammer, on the orbital station with
the lowest id — and where a system has no orbital station at all the
machines put one there themselves (`world::jammer`). How often that is,
and how far the crew start from the origin, over ten galaxy seeds —
`cargo test --release -p world -- --ignored --nocapture
jammers_over_ten_seeds` (`tests_jammer::jammers_over_ten_seeds`), which
takes the origin the way `World::start` takes it, from the dock
`spawn_anywhere` picks:

| | min | median | max |
| --- | --- | --- | --- |
| systems needing a derived jammer, of a thousand | 380 | ~400 | 421 |
| the same as a share | 38% | 40% | 42% |
| hops from the crew's own star to the origin | 19 | ~35 | 69 |

Two things fall out of it. **Two systems in five have no station of their
own**, which is a lot more than "the odd one" — so the derived jammer is
the ordinary case and not a corner, and it had to be a real station with
a hull the crew can dock at and fight through rather than a marker. And
**the crew start between nineteen and sixty-nine hops from the origin**,
where the roll's own floor is eight (`DROID_ORIGIN_MIN_HOPS`): a galaxy
is about a hundred and twenty hops across and its arms are long, so a
roll that only asks for eight lands far further off than eight in
practice. The jam is therefore about the crew choosing to push *towards*
the machines and finding the way back shut behind them, not about the
crisis arriving and trapping them where they are.

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

**And it is committing what you changed.** The last thing a session does,
after the number is back and the shells are shut, is **commit its own
work**: `git add` the files *you* touched — named one by one, never `git
add -A`, `git commit -a` or a bare `.`, since the tree holds the other
agents' half-finished edits as well as yours — and commit them with the
**task number the player gave you in the prompt** at the front of the
message and a line saying what the change was: `86. medics carry the
wounded off the deck`. A new file wants the same `git add` it wanted
anyway (see "New files need `git add`"). Nothing else: no `git stash`,
`checkout`, `reset` or `rebase`, which would take somebody else's work
with them, and no push. If the work is unfinished or a test you owe is
red, commit it all the same and say so in the message — one numbered
commit a session is what makes a task's changes findable afterwards, and
an agent that leaves its work uncommitted leaves the next one guessing
which lines in the tree are whose.

## A run: what the first step of the roguelike switched off (feature 102)

Bims is becoming a roguelike top-down co-op shooter — humans against the
machines, one default ship flown from site to site, time and money the
only resources — in three steps: **this one switched off what the new
game does not use and put the run on its new footing; nothing was
deleted.** Step two builds the travel and mission loop, and step three
deletes the dead code. What is off is off behind a **switch on the world**,
saved and in `world_checksum`, and each is **off in every run** and on only
for the tests of what it switches:

- **`World::needs_enabled`** (`set_needs_enabled`): the room's whole life
  sim — the needs draining and sending anybody on an errand, a pot going
  bad, the bays grown and tended, the bunks dealt and counted against a
  hire, the mess biting, the food in a dark cold store spoiling. The world
  tells every room every step (`hand_the_rooms_the_needs`, beside
  `set_players`, since a room is built afresh at every dock), and the room
  holds it as `bims::game::Game::needs_enabled` — **on** in the classic room,
  so `bims room` and every probe in `scratchpad/` are what they were. Off,
  `tick_bim` stands the levels still, hands `Health::update_held` a fed and
  rested body (so the blood, the wounds and the mending still run), starts
  no need's errand, offers no cook, tend or sweep, and `hit_at` opens no
  menu on a fixture that only served a need.
- **`World::human_foes_enabled`**: the generator's hostile stations and
  towns put on the `hostile` list (`rolled_hostile`, at a start, a jump and
  the switch), raiders coming (`run_raid`'s quiet arm), an enemy's shelf
  laid out and plundered, and a station's dead looted. A probe's own
  `set_hostile` still works with it off — `Session::combat` builds the
  arena `droids` hands to the machines that way — and `raid_for_probe` /
  `raid_coming_for_probe` switch it on, a raid being asked for. The
  generator still **rolls** `StationBlueprint::hostile` (it is in the galaxy
  checksum, and it is where the tier-two research keys lie), but nothing
  reads it as a stance: the lobby lets a crew start anywhere and rings
  nothing in red.
- **`World::radiation_enabled`**: stage eight's dose. Off, nobody is
  exposed, in a suit or out of one.
- **`World::shipyard_enabled`**: a construction site placed
  (`SiteRefusal::NoShipyard`, `Refusal::NoShipyard` 82) and what the ship
  lives on bought (`World::buyable` — gear, and nothing else, with it off;
  `Session::sold_here` greys the rest). The class deployables are not
  parts and are untouched; the Build tab is not shown.

The rest of the footing: **the crisis is there from day nought**
(`crisis_first_day` is nought; `DROID_FIRST_DAY` is no longer read and step
three deletes it), so the origin is theirs at the start and a system due
is theirs whole the moment the crew are in it; **the `game` flow has no
design phase** — Start is `screens::designer::start_run`, which stands
`Session::run` up on every machine of a lobby from the same numbers: the
playtest ship and `money_per_bim` a player's Bim, the lobby's default
`START_MONEY_PER_BIM`; the setup tab has no ship-size row. **A station's
people keep a routine** (`bims::routine`, dealt by
`Residents::deal_roles` from `World::open_residents`): a role off the
station's seed and the body's place — guard, trader, worker, civilian —
and a round of stops derived from the room's own fixtures
(`routine::Anchors::of`), with a wander over reachable open deck as every
role's fallback. The round rides on the `Bim` (`adopt` shifts it with the
body), is walked by `Game::keep_to_routine` only with the needs off and
only while the body is its own — not under arms, posted, sheltering,
running or given an errand — and is not dealt to an enemy's garrison.
The commands: `droids` is the fight; `combat`, `combat_<class>` and `raid`
are gone; `tier2_test` and `tier3_test` are `droids` at a tier.
`tests_run.rs` in `crates/world` is the feature's own tests.

## The loop: travel and missions (feature 103)

The second step of the redesign: **world map → travel → mission → back to
ship → world map**, every run command under it (`game`, `simulation`,
`test`, `test_planet`, `crisis`, `jammer`, `defense`, `droids`,
`droids_planet`, the tier tests and the class runs all open in their first
mission at the site they set up). The world's half is `crates/world/CLAUDE.md`
("The loop"); the player's is `README.md` ("The loop"). What to hold on to:

- **Only travel moves the world clock.** A trip is resolved, not flown:
  the last yes of every connected player puts `clock_minutes` on by the
  trip's whole minutes in one go (`World::travel`) and the crew arrive
  docked or landed with a mission begun. During a mission and on the map
  the clock stands still — the rooms are told so too (`Game::set_clock_runs`)
  — and everything inside a mission runs on the **mission clock**,
  `World::mission_steps`, nought on arrival: the droid waves
  (`DROID_REINFORCE_STEPS`), a town's first wave (`DEFENSE_DELAY_STEPS`)
  and every class cooldown. The helm's four orders are refused
  (`Refusal::TravelIsResolved`) and the strip at the top is the world map
  (`screens/worldmap.rs`) rather than the helm.
- **The old game's clock is a switch**, `World::set_free_clock`, off in
  every run and on for the tests of flight, raids, wages and the day by the
  step, in the pattern feature 102 set. A new test of any of those turns it
  on right after building its world, or its `Confirm` is refused and a
  loop that waits on `clock_minutes` never ends (the engineer tests'
  `run_for_seconds` did exactly that until it read `mission_minutes()`).
- **The run's commands apply at once** (`World::applies_at_once`), since
  nothing steps on the map: `Propose`, `Accept`, `Return`, `LeaveBehind`
  and `PlayerGone` — the last said by the **host** alone, off the roster,
  since the world cannot know who is at a keyboard.
- **Dying.** A dead player's Bim is out until bought back at a mission's
  start (`BUYBACK_COST`, longest dead first), its progress kept; a dead bot
  costs `BOT_DEATH_PENALTY` and is gone at the end of the mission; the run
  is lost when every player's Bim is dead at once.
- **The Republic pays for machines now**, by their tier, and the bounty is
  pending until the site is cleared.

**Looking at it from a terminal.** `BIMS_MAP=1` opens any run between
missions with the map up (`Session::map_for_probe`), and `BIMS_DEPART=1`
with the departure check asking — crew member 1 out cold just inside the
station's door and the player's own Bim having pressed *Back to ship*
aboard (`Session::depart_for_probe`; it wants a crew of two, so `droids`).
The *Back to ship* button is at the bottom right, `(1290, 867)` at
1400×900, and the map's first row at about `(650, 109)` with *Propose* at
`(623, 392)` once a row is picked — `BIMS_POINTER="40:move:1290,867;
42:click:1290,867;80:move:650,109;82:click:650,109;110:move:623,392;
112:click:623,392"` on `simulation` is the whole loop in 220 frames. A
scripted run prints `left:`, `proposed:`, `accepted:`, `travelled:`,
`gone:` and `refused:` lines, and **`scratchpad/duo_resync.sh travel`** is
the pair: both press *Back to ship*, the host proposes, the guest accepts,
and both must print the same `travelled:` line with the checksums agreeing
after it. `net::tests::two_ends_leave_vote_and_travel_as_one_world` is the
same through the in-process relay.

**How long a trip is**, for the default ship over ten galaxies —
`cargo test --release -p world -- --ignored --nocapture
travel_days_over_ten_galaxies` (`tests_mission.rs`), every site of the
spawn's system from the spawn and every site of every system a lane away:

| | n | min | p25 | median | p75 | max |
| --- | --- | --- | --- | --- | --- | --- |
| in the system | 83 | 0.11 | 1.41 | 2.85 | 5.82 | 11.46 days |
| one hop | 230 | 0.31 | 1.22 | 2.42 | 4.04 | 12.08 days |

So a trip is two or three days as a rule — the crisis crosses a hop every
`DROID_SPREAD_DAYS` (five) — and a jump is usually no longer than a trip
across the system, since a jump lands the ship well inside its target
system and the charge is twenty minutes. Nothing stops a crew going to the
site it is at (a trip of nought minutes); step four of the redesign puts a
floor under a trip.

**Not done, because it is not there**: the spec's Commander
"call-in-reinforcements" (to become once a mission) and its "temporary
Republic soldiers" — the commander has a rally and a squad of the crew's
own bots, and nothing calls a soldier in — and "liberation", which nothing
in the code does yet.

## Money, not materials (feature 95)

The economy used to be a chain: ore mined off a belt, smelted into metal,
worked into components and emitters, and those built into parts and guns.
**It is money now**, and the change reached every crate:

- **Eight resources are gone** — ore, metal, components, galvum, emitters,
  rock, fibre and the armoury's vest — and the discriminants after each
  were closed up rather than left as holes (`physics::ResourceId`, 18 of
  them). Nothing saved has to load, so nothing was owed a hole. That moved
  every design hash, both galaxy checksum sets and `REFERENCE_CHECKSUM`,
  and bumped `GENERATOR_VERSION` to 7.
- **`Storage::Shelf` is gone with them**: three classes left, and
  `PartKind::Shelf` is locker class. `PartKind::Smelter` is gone.
- **A part has a `mass` column** where it had a `recipe`, set to exactly
  what its recipe weighed, so nothing about how a ship flies moved. A
  **construction site costs its part's price**, paid out of the pool
  anywhere — docked, holding or landed — and a deconstruction gives the
  whole price back. There is no hauling: `Job::Mine` and the haul half of
  `Job::Haul` are gone, and so is `World::free` and every reservation.
- **`RECIPES` is one row**: two vegetables into a medkit at the drug lab.
  The workbench and the armoury are still worked at — upgrades, and the
  cabinet — through `shipdesign::recipes::is_workstation`.
- **Mining is gone**: `crates/world/src/mining.rs`, `bims::game::Eva`, the
  marks, the Actions tab and `BIMS_AT_BELT` with it. Belts stay as bodies.
- **The research tree is five nodes**, and everything behind a deleted one
  is known at the start.
- **Gear is bought**, where a place has the trade: two flags a market,
  rolled off its own seed, and every tier on sale at the book times 1, 4
  and 16 (`economy::TIER_PRICE`).
- **The Republic pays a bounty** for an enemy taken down, by its gear tier,
  once per enemy — which is where money comes from in a fight.

The detail is in `crates/world/CLAUDE.md` ("Construction is stage 7",
"Money, the bounty and what a crew are worth"),
`crates/shipdesign/CLAUDE.md` and `crates/worldgen/CLAUDE.md`, and the
player's half is `README.md` (*Making things*, *Trading*, *Building,
aboard*).

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
- **The canvas between the panels is Bevy's; a canvas inside a panel is
  egui's** (feature 97, *Bloom* below). `shapes.rs` tessellates the twelve
  floats a shape into triangles either way. For the world's canvas — the
  deck, the map, the chart, the yard, the test room — a screen hands them
  to `scene::WorldCanvas` (`world_canvas.shapes(..)`, a system parameter
  of the three screens that have one), which makes each call a **layer**:
  a `Mesh2d` on the one camera, a z apart in the order painted, under
  egui and under the bloom. For a canvas inside a panel — the lobby's
  galaxy and diagram, the setup's portrait — `canvas::paint_shapes` adds
  an `egui::Mesh` to the panel's own painter as before, **because egui
  panels paint an opaque fill**: a Bevy mesh under a panel is a mesh
  nobody sees, which is how the galaxy preview was blank for a build. So
  nothing under a panel may be moved to Bevy, and the world's canvas works
  only because nothing egui paints over it is opaque — the floating
  panels over the deck are translucent, and it shows through them as it
  always did. The **words** over the deck are still egui's, painted after
  on the background layer (`canvas::canvas_painter`), so they are over
  everything the canvas draws.
- **Anti-aliasing is feathering, in `shapes.rs`, and nothing else.** The
  camera is `Msaa::Off` and egui paints into the window's *unsampled*
  target anyway, so every edge is ramped a pixel wide from its colour to
  transparent the way epaint draws its own shapes, and a stroke thinner
  than a pixel is drawn a pixel wide and fainter. The pixel is
  `pixels_per_point()`, so it stays one device pixel under any UI scale.
  A new kind of shape has to go through `fill` or `stroke` there, or it
  comes out jagged beside everything else.
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
  **either key again lets them follow again** — F with a banner already
  down takes that banner up rather than arming the pointer for another
  one, which is the only press there is that does it: the world reads
  the *same* order given again as a release and "the same order" means
  the same **tile**, so a second banner anywhere else is a fresh attack
  and a crew left under one after a fight stands at it for ever, taking
  no errand (`screens::game::attack_key`); a banner **goes with the
  deck it stands on**, so a dock, an undock, a landing or a lift-off
  puts the crew back to following of its own accord —
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
- **The health block says what the Bim is dying *of*, not only how much
  is left.** The bars on the right-hand panel are the biggest thing on
  it (`crew::BAR_W`, `theme::health_bar`, `crew::HEALTH_NUMBER`), and
  under them is the **peril block** — `crew::perils`, one `Peril` a
  cause with the rate, the countdown at that rate, where the loss is
  coming from and what stops it. Two things can kill a body and the
  block works both out **from the body**, since nothing records a
  cause: the blood, which is every open wound at
  `health::BLEED_PER_WOUND` an hour plus every untreated trauma's own
  `bleed()` — `Health::update_held`'s own sum, so the number on the
  panel is the number the room subtracts — and extreme malnutrition at
  `health::HEALTH_DRAIN`, whose countdown is left off while a trauma
  holds the head or the body at nothing, because death there is the two
  of them empty *and clean*. The block is graded: red and "DYING OF"
  for a body in a dying state or starving, the caution colour and
  "LOSING" for one that is only bleeding through wounds a bandage
  closes — a scratch that would empty it in ten hours is worth a number
  and not a fright. A part a trauma holds at nothing names that trauma
  in its own row, and the dead say what of (`crew::death_line`, read
  off the body the way `Health::is_dead` decides). The words are
  `names::PERIL_*` and `names::DEATH_*`.
- **A Bim in a dying state wears a red cross on the deck.**
  `theme::dying_cross` — a white disc with a medical cross on it rather
  than another coloured ring, because every other mark out there is a
  ring of some colour and this one has to say *that* one. Drawn over
  the head in both screens (`screens::game`, `screens::room`) off
  `Game::is_dying`, and for the **living only**: a trauma stays on a
  corpse, and a cross over one would be asking for a medkit nothing can
  be done with. `BIMS_DYING=n` is how it is looked at.
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
  **Past a rule at the right-hand end are the medicine's two boxes**,
  which every crew member has whatever its class — a classless one has
  them alone — `medicine_boxes`: the medkit and the bandages it carries,
  both charges since the medicine became everybody's
  (`crates/world/CLAUDE.md`, "The medicine is everybody's charges"). No
  key casts them (`AbilityBox::action` is `None`). **A cooldown is
  drawn the way Dota 2 draws one**, on every box that has one: with
  nothing left, the share still to come is laid dark over the whole box
  and swept back clockwise from twelve o'clock as it runs out
  (`cooldown_sweep`, a fan from the middle through the corners, with a
  hand) and the seconds in the middle; with some left and the next on
  its way, the count sits on a disc in the corner and a **ring** round
  the disc fills clockwise (`recharge_badge`). `Face::charges` is the
  reading for any stock of charges — the engineer's kits, the grenade
  and the medicine alike — off `World::{charges, charges_of,
  charge_cooldown, charge_cooldown_left}`; the taunt and the rally sweep
  over their own whole cooldown.
- **Every ability says on the deck what it is doing, on the Bim doing
  it** (feature 91, `screens/game.rs`'s overlay block). The rule is that
  a box at the foot of the canvas is the *player's own* readout and a
  ring at a radius says how far something reaches, so neither tells
  another player what the Bim beside them is *up to* — that wants a mark
  on the body. So: a **braced** soldier is drawn braced (the room's own
  figure, below); an engineer laying a kit or anybody putting a site
  together has a **charge bar on the tile** (`theme::work_bar` off
  `Game::working_at`, the walk to it counted, so the bar stands on the
  tile that is being worked and not over the worker); a **beamed**
  crewmate has **green numbers** rising off it for what the beam put
  back (`theme::heal_number`, `names::heal_gain`); a **tank** wears a
  small shield over his head while his wall is up and throws rings off
  his body while he taunts (`theme::bulwark_shield`, `taunt_shout`,
  inside the wall's ring and the taunt's dashed radius, which say how
  far each reaches rather than who is holding it); and the **commander**
  calling a rally wears two chevrons (`theme::rally_call`) — he had
  nothing before, since `World::aura_reaching` answers `None` for a
  commander asked about his own aura. What was already on the body
  stands: the surge's halo, the grenade in flight and its burst, the
  squad bracket, the aura's and the rally's rings on everybody *else*.
  **The numbers are the screen's own state and nothing else's**: neither
  the room nor the world records how much a beam put back, so
  `GameScreen::heals` watches each beamed body's blood and parts frame
  by frame, gathers the gain into whole numbers and floats one at most
  every `HEAL_GAP` — at 24× a beam puts back a dozen points a second,
  and a number a frame is a green smear rather than a figure.
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
