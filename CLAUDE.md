# Working on Bims

A 2D top-down game: all the simulation is Rust, and so is the window —
`crates/app` is a Bevy binary that draws every crate's shape buffer through
egui and lays the panels out round it. `README.md` explains how the game
itself fits together; this file is about working on it.

## Running it

`nix run .` is the one command: it **builds and opens the window**. There are
twenty-two things to run, and each is a name rather than a flag — and
`cargo run -- list` prints every one of them with a line each, which is
the build's own answer where this table is a copy:

| command | `cargo run` | opens |
| --- | --- | --- |
| `nix run .` / `nix run .#game` | `cargo run -- game` | the whole game in order — menu, setup or lobby, world and station, then **the run** (feature 102): no design phase, the default ship (the playtest ship) docked where you said and `START_MONEY_PER_BIM` (5 000) a player's Bim in the pool (`Session::run`, `screens::designer::start_run`) — see *A run* below |
| `nix run .#simulation` | `cargo run -- simulation` | straight into the world on the playtest ship |
| `nix run .#design` | `cargo run -- design` | straight into the yard, the playtest ship given, docked where the simulation docks — how a change to the designer is looked at |
| `nix run .#test` | `cargo run -- test` | the simulation somewhere else each time — docked at a random station somebody lives on, in a random galaxy, on the **combat ship** with one crew member and a **mercenary for hire** at the dock whatever the roll said (`Session::mercenary_for_probe`). The dock is the spawn, a hub; **`BIMS_STATION_SEED=<n>`** rebuilds it as the station seed `n` generates (feature 112), and **`BIMS_STATION_KIND=Relay`** (any kind) over its own — the run prints `station: <kind> seed <n> plan <plan>` |
| `nix run .#test_planet` | `cargo run -- test_planet` | `test` **set down on a planet**: the same random galaxy and roll, made among the systems whose first planet with ground has friendly people (`world::spawn_with_ground`, `ship::session::pick_ground`), and the ship landed at its settlement (`Session::land_for_probe`) — the mercenary asked for first, so the settlement's room has one too. **`BIMS_STATION_SEED=<n>`** draws the town from seed `n` (its planet's biome and population kept, feature 112; the run prints `town: seed <n> biome <b> population <p>`), and on `droids_planet` too |
| `nix run .#tier2_test` | `cargo run -- tier2_test` | `droids` with **everybody's kit at tier two** — every crew member's gun at it (its kind as the fight dealt it) and a fresh helm, kevlar and leg guards at it on, pieces of the world's — and the machines at tier two (`Session::droids_at_tier`, `World::outfit_for_probe`), so the fight is looked at with nothing at tier one on either side. It was the human garrison's fight until every enemy was a machine (feature 102) |
| `nix run .#tier3_test` | `cargo run -- tier3_test` | the same at **tier three** |
| `nix run .#droids` | `cargo run -- droids` | **the fight** — the **machines** (feature 83): the **combat ship** (`shipdesign::fixture::combat_ship`, the playtest ship with bunks and chairs for five) with **sixteen crew** (`COMBAT_CREW`: five at the bunks, eleven standing on the deck), a gun in every hand — the seven kinds dealt round, each at its lowest tier (the minigun two, the rail lance three; task 115) — the **last four of them hired field medics** (`session::COMBAT_MEDICS`, feature 86), docked at the spawn rebuilt as the **arena** (`world::station::arena`, 72 tiles across) that the **droids hold**. `Session::combat` builds the ship, the crew and the arena and `Session::droids` hands the arena to the machines; the `combat` command that stopped at the first half — the arena's own people turned against the crew — is gone with every other human enemy (feature 102), and so are its `combat_<class>` runs and `--combat`; `BIMS_FIGHT`, which staged a fight with a station's *people* on the simulation, went with the human enemies themselves (feature 104). Its people are gone (`World::people_of` is nought for a held station) and a wave of machines stands about it instead: Wardens a sixth, Husks a third, Troopers the rest, **sixteen a wave** (`session::COMBAT_WAVE`) — the game's own `droid::wave_size` is the base, the *players* and the world clock and nothing else (feature 105), which for one player at day nought is three, and sixteen crew against three is not the fight this command is for. `DROID_REINFORCE_STEPS` is **a minute of the mission clock** here — a real second at 1× — rather than the game's thirty real seconds, so the next wave is watched landing at the far airlock rather than waited for, and the station has **three waves** rather than the formula's two at day nought (`DROID_WAVES_IN_PROBE`), since one wave landing and then a cleared station is not what these commands are for — the red line along the top says which wave is on the deck, how many of it are standing, and, the moment the last of them is down, **how long until the next lands**. `BIMS_DROID_TIER=2` brings them at a tier, `BIMS_DROID_WAVES=5` gives the station that many waves, `BIMS_DROID_REINFORCE=600` makes the wait between them that many minutes of the mission clock — a minute is a real second at 1× and two and a half *frames* at 24×, so the countdown cannot be caught by a scripted run without lengthening it — and `BIMS_DROID_WAVE=32` makes a wave that many whatever the formula says, which is how the measurements below were taken; `BIMS_DROIDS=1` replaces the wave with a **showcase** — a row a kind and a column a state: idle, firing or striking, arms at nothing, legs at nothing, destroyed — so all fifteen drawings are one screenshot (`World::stage_droids_for_probe`). **`BIMS_STATION_SEED=<n>`** (and `BIMS_STATION_KIND`) fights it in the station seed `n` generates rather than the arena (feature 112, `Session::regenerate_dock_for_probe`) |
| `nix run .#combat_droids_engineer` … `#combat_droids_commander` | `cargo run -- combat_droids_medic` | that **same fight with the class in hand** (features 79 and 83, `Launch::DroidsAs`): one command a class — `Class::ALL` bar `None`, spelled as `names::CLASS_NAMES` spells it, lower case — and nothing else about the run differs: the same combat ship, the same sixteen crew, the same droid-held arena and the same two dials, with `World::set_class(0, …)` on top (`dev::class_crew`, which takes the command's class and lets `BIMS_CLASS` override it). It opens at the class's **top level** (`dev::combat_class_level`, feature 80): the tenth with all of the class's talents still to choose (seven), or — for a **ranked kit**, the **soldier's** (task 124), the **engineer's** (task 127), the **commander's** (task 129) and the **medic's** (task 130) — the **sixteenth with sixteen skill points** to spend (Ctrl and Q/C/E/R, a Ctrl-click on a box, or the Skills tab): the soldier's Frag Grenade, Weak Spot, Brace and Rampage, the engineer's EMP, Healing Sentry, Sandbags and Sentry, the commander's Battle Cry, Command Aura, Rally and Reinforcements, the medic's Nanite Burst, Healing Aura, Heal Beam and Cloak; `BIMS_LEVEL=n` says otherwise, and **`BIMS_RANKS=q,c,e,r`** (`BIMS_RANKS=4,4,4,4`) sets a ranked kit's four ranks outright, capped by the level's gates (`World::set_ranks_for_probe`), and puts the charges they give in hand. **An engineer with no rank has no charge at all**: a charge is a counter the world keeps (`World::charges_held`), filled by the ranks and never a kit in a pack, so `BIMS_RANKS=4,4,4,4 bims combat_droids_engineer` is the whole kit to look at. A commander's **Reinforcements** come at a mission's start, and the probe's mission began before its ranks were set, so `BIMS_RANKS` brings them there and then (`World::reinforce_for_probe`): `BIMS_RANKS=0,0,0,4 bims combat_droids_commander` is four Bims of the Republic beside him, a chevron in his colour over each. His squad's attack is on **B** since E became his Rally, fall back on T and stand ground on Z. A medic's **R** cloaks the crew member under the pointer, or himself with the pointer on nobody: `BIMS_RANKS=4,4,4,4 bims combat_droids_medic` is his whole kit, the aura's green ring round him and a burst's ring running out to its reach on `Q`. The `combat_<class>` runs beside these were the human garrison's fight, and went with it (feature 102). The parsing is `main.rs::class_named`, and `bims list` prints the lot |
| `nix run .#droids_planet` | `cargo run -- droids_planet` | `test_planet` with the **town** droid-held: the same random galaxy and roll, the ship set down at the settlement, and the settlement's people replaced by the machines, whose lander sets down on the plain beyond the north gate for an odd wave and the south for an even one. The same minute's reinforcements and the same two dials |
| `nix run .#crisis` | `cargo run -- crisis` | the **crisis** a day before it first spreads (feature 92): `test`'s own random galaxy and random dock and the machines' origin forced **two hyperlane hops** from the crew's own star (`Session::crisis_for_probe`, `session::CRISIS_HOPS`) where the roll's own floor is eight. The origin is theirs from day nought, as in every run since feature 102, and the clock is wound to the eve of the day the ring round it turns (`DROID_SPREAD_DAYS`, five) — so the next stars turn red on the galaxy chart within a day of the clock — a day the crew have to travel, since only travel moves the world clock (feature 103) — and the crew's own system five days after that. The chart is where it is looked at: the lanes are drawn faintly under the stars, an infested star is crossed in the enemy's red **charted or not**, and the panel says under the star you pick which day it is due (`screens/game.rs::crisis_line`). `BIMS_CRISIS_DAY=n` moves the day the origin turns, and the clock opens a day short of the next ring whatever it says, so the dial is about what the *rest* of the galaxy's days come out at rather than about how long to wait |
| `nix run .#jammer` | `cargo run -- jammer` | the **jammer** (feature 93): `crisis`'s own random galaxy, random dock and origin **two hops off**, with the clock wound *past* the day this system falls rather than a day short of the first — so the crew open **inside** an infested system, every station of it in the machines' hands (`Session::jammer_for_probe`, `World::infest_here_for_probe`), a wave aboard the one they are tied up at and `DROID_REINFORCE_STEPS` a minute of the mission clock. Two things are looked at from here. The **jam**: the chart lights the lanes out of the ship's star in the hyperdrive's violet, draws the route to whatever star is picked along them, and **bars in red every step of it a jammer would turn back** — a jump *inward*, towards where the machines began, is refused while the jammer station stands (`Refusal::Jammed`), and the panel says which station holds it. And the **tier**: two hops is inside `DROID_TIER_THREE_HOPS`, so the machines come at **tier three** without a dial. `BIMS_DROID_TIER=1` says otherwise, and `BIMS_DROID_WAVES`/`BIMS_DROID_REINFORCE` are `droids`' own |
| `nix run .#defense` | `cargo run -- defense` | **defending a town** (feature 94): `test_planet`'s own random galaxy and roll — the ship set down at a settlement whose people are friendly — with the machines' origin forced **one hyperlane hop off** and the crisis's first day wound to nought, so the town's system is on the **front** (`World::front` of it is one) — which since task 111 is only the tier and the prices, since every site that is neither a trader nor an enemy's is *threatened* from the first day. The map says so under its planet's icon, `DEFEND` in amber. A minute of the mission clock after the landing (`DEFENSE_DELAY_STEPS` is twenty of them in the game, a real twenty seconds at 1×, task 111; `BIMS_DEFENSE_DELAY=n` minutes over the command's own one) a wave sets down outside a gate and walks in, and the red line along the top counts it the way it counts a held station's. The fight is the one that happens **inside one room**: the town's **guard and its mercenaries** take arms and fight the machines where they stand, everybody else walks into the nearest house and stays there, the crew never aim at a townsperson and the machines aim at both. Hold the last wave and the town is **held** — friendly for good, trading and hiring even after its system falls, its map tag *held* — and some of its people join the crew; go back to the ship before the last wave is down and the town falls to the machines (feature 103). `BIMS_DROID_WAVES`/`BIMS_DROID_REINFORCE`/`BIMS_DROID_WAVE` are `droids`' own (`Session::defense_for_probe`); **`BIMS_STATION_SEED=<n>`** draws the town from seed `n` (feature 112) |
| `nix run .#guardian` | `cargo run -- guardian` | **the Guardian** (feature 100): `droids` at **tier three** with every wave exactly **one Guardian and two Troopers** (`Session::guardian`, `World::set_droid_kinds_for_probe`) and `DROID_REINFORCE_STEPS` a minute of the mission clock, so the machine is looked at on its own with a fight going on round it — its shield stopping the crew's bolts from the front and flaring where they stop, its turn, its wind-up and its beam. `BIMS_DROID_WAVES` and `BIMS_DROID_REINFORCE` are `droids`' own; the tier and the wave are the command's whatever `BIMS_DROID_TIER` and `BIMS_DROID_WAVE` say |
| `nix run .#relics` | `cargo run -- relics` | **the relics** (feature 106): `droids` with **one wave of four** (`Session::relics`, `session::RELICS_WAVE`) at the arena's own tier and the reinforcement clock a minute — short enough to clear, so going back to the ship after opens the **reward screen**: three relics, drawn by the day's odds (task 117), to choose from together. `BIMS_RELICS=<id>,<id>` gives the steered Bim those relics at the start (a relic's name in lower case with `_` for the spaces, `focusing_lens`, or its code); `BIMS_REWARD=1` opens straight on the reward screen, the site cleared by the probe (`Session::reward_for_probe`); `BIMS_CACHE=1` with a relic cache opened on the site's research desk and its one relic being chosen in the mission (`Session::cache_for_probe`); and `BIMS_WIN=1`, on **any** command, wins the run the next time a site is cleared with machines in it (`World::set_win_on_clear`), which is how the victory screen and the profile's unlocks are looked at — point `BIMS_PROFILE_DIR` at a scratch directory first, or the win lands in your own profile |
| `nix run .#heart` | `cargo run -- heart` | **the Machine Heart** (feature 108): `Session::combat`'s ship and sixteen crew with **everybody's kit at tier three** (`World::outfit_for_probe`), the machines' origin put at the crew's own star and the ship **docked at its fortress** (`Session::heart`, `World::heart_dock_for_probe`) — the core in the hub sealed by its conduits, the fabricators beside it, the waves the game's own formula and the next a minute of the mission clock after the last is down. `BIMS_HEART_PHASE=2` opens with every conduit down (the core exposed), `BIMS_HEART_PHASE=3` with the core's health just under the overload as well (`World::set_heart_phase_for_probe`, after the Heart is laid); `BIMS_DROID_WAVES=n` gives the fortress that many waves. The fortress is past the ship's own lobby and down the west arm: `BIMS_ZOOM=0.28 BIMS_WINDOW=2000x1300 BIMS_KEYS="40:X,700:X,720:X" BIMS_POINTER="45:move:1240,675;47:click:1240,675;725:move:1720,675;727:click:1720,675"` walks the crew in to the lobby and then to the hub |
| `nix run .#manufacturers` | `cargo run -- manufacturers` | **the Manufacturers** (feature 109): `Session::combat`'s ship and sixteen crew taken to the **nearest site of theirs** (`World::manufacturer_dock_for_probe`, a trip a lane) with the world clock put at **day eight** — their people in tier-one gun and armour and about half the garrison **Troopers** beside them. `BIMS_MANUFACTURER_DAY=0` is their people alone with pistols, ten or more their own waves in the machines' tier of kit, thirty seconds apart unless `BIMS_DROID_REINFORCE` says; `BIMS_DROID_WAVE` sizes the garrison. The site is down the airlock: `BIMS_KEYS="40:X,70:V" BIMS_POINTER="45:move:1000,470;47:click:1000,470;60:move:1010,480;62:right:1010,480" BIMS_ZOOM=0.8` walks James and the crew in |
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
a point are a notch of the wheel, which zooms. The dials that staged the
old game went with it in feature 104 — `BIMS_LANDED` and `BIMS_LANDING`
(the landing and its descent), `BIMS_FIGHT` (a fight with a station's
*people*) and `BIMS_RAID` (a raider and its boarders, with the red warning
along the top) — and a planet is looked at through the commands that land
on one (`test_planet`, `droids_planet`, `defense`) or `BIMS_AFIELD` below.
`BIMS_LOST=1` opens the simulation with every crew member shot where they
stand, so the end screen is what the next frame is. `BIMS_AFIELD=1` opens the simulation landed with the
crew member walked out onto the plain west of the ship and a minute gone
by, and `BIMS_ZOOM=0.3` zooms the game view out by that factor once it
is fitted (a scripted wheel does not reach it): the two together are how
the plain and its fog are looked at. **`BIMS_TRADER=1`** opens any run
at the nearest open **trader** (task 114): the mission left and the trip
taken, the Trader panel up on the map (it prints `trader: star <s>
station <t>`); **`BIMS_ARMORY=1`** (or the
old `BIMS_ARMOURY=1`) opens any run with the **Armory panel** up (task
113) — a `press`, `move`s and a `release` in `BIMS_POINTER` drag a thing
across it, and `BIMS_MAP=1` beside it is the panel on the map, where it
is not read-only. `BIMS_GRAVES=n` leaves `n` of the station alongside **dead where
they stand** and builds its room again over the bodies (feature 85,
`World::lay_graves_for_probe`), for looking at the dead lying on a
station's deck without fighting, leaving and coming back —
`BIMS_GRAVES=4 BIMS_ZOOM=0.6` is the aftermath from far enough off to
see it. `BIMS_DYING=n` **downs** `n` of the crew — the bar at nought
and the thirty-second countdown running (`Session::maim_for_probe`,
task 120) — for looking at the **countdown ring** over a body on the
deck and at the downed block under the health bar, and at the bots and
field medics coming to revive them; the player's own Bim is left out of it
unless `n` reaches the whole crew, so the picture is taken from
somebody still walking about (`BIMS_DYING=3` on `droids`).
`BIMS_FIELD_MEDIC=n` makes the **last `n` of the crew hired field
medics** (feature 86, `Session::field_medics_for_probe`) — the contract
and a medic's revive time, no money taken — the last rather
than the first since slot 0 is the player's own and the rescue is a
*bot's* branch (`Game::bot_stand`). **`droids` already sails with four
of them** (`session::COMBAT_MEDICS`), and so does everything built on
it — the tier tests and the `combat_droids_<class>` runs — since a fight
with nobody who may carry
is a fight where a body down stays where it fell; the dial asks for the
same crew members from the same end, so setting it at four or under
changes nothing and setting it higher reaches further up the crew. `BIMS_CARRY=1` downs a crew member and
puts it in the arms of somebody who may carry, for looking at a body
being carried off the deck without waiting for a fight to put one there.
Both want **more than one aboard**, so they are `droids`' and not the
simulation's, which sails with a crew of one: `BIMS_CLASS=medic
BIMS_CARRY=1 bims droids` is a body in the arms (the Carry box lit, with
no count on it, is what says so), and `BIMS_DYING=2 BIMS_SMOKE_FRAMES=600
bims droids` is one of its field medics going and fetching for itself.
`H` is the carry's key in `BIMS_KEYS` and `G` the held revive (a
`+G` … `-G` pair would hold it; a single `G` is one frame's press). (`BIMS_BANDAGES` and
`BIMS_MEDKITS` went with the medicine in task 120.)
**`BIMS_KITS=n`** does the same for the engineer's three **charges**
(feature 88, task 127): exactly `n` EMPs, Healing Sentries and sandbags
on everybody — counters, never kits — and their cooldowns started
afresh. `BIMS_RANKS=4,4,4,4 BIMS_KITS=0 bims combat_droids_engineer` is
the one state a scripted run cannot walk itself into — no charge in hand
and the whole wait ahead — so the seconds swept over the boxes on the
hero panel are a screenshot rather than a pointer hunting a Bim that is
walking away. The engineer's sentry (R) is on a cooldown of its own,
counted from the laying, and not a charge.
**`BIMS_GRENADES=n`** is that dial for the soldier's **grenade
charges** (feature 90): since a grenade is a charge on a cooldown like
a kit — as many and as quick as Frag Grenade's rank says (task 124:
one at thirty seconds at the first rank, two at twenty at the fourth),
and **no class makes anything to use a skill** —
`BIMS_RANKS=4,0,0,0 BIMS_GRENADES=0 bims combat_droids_soldier` is
the `19s` in the corner of the Q box, and `BIMS_GRENADES=1` a soldier
with one throw in hand and the next on its way.
**`BIMS_CRISIS_DAY=n`** is the `crisis` command's own (feature 92): the
machines' first star turns on day `n` instead of day nought, and
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
the game's own is `DEFENSE_DELAY_STEPS` (twenty minutes of the mission clock, a real
twenty seconds at 1× — since task 111 every site's prep time). Raise it,
`BIMS_DEFENSE_DELAY=20`, to look at the prep countdown (`Prepare: 0:14`
in the red chip along the top) before the shooting starts;
`BIMS_DROID_WAVES=1 bims defense` is a fight that can be held to the end
in one sitting. **Since task 111 every run command whose dock is
neither a trader nor held by an enemy is a defence too** — `simulation`,
`game`, `test`, `test_planet`, `crisis` — with the game's own twenty
seconds.
**`BIMS_FREEZE=n+f`** pauses the game `f` frames after the crew's room
hears its `n`th shot or blow, and **`BIMS_FREEZE=down:n+f`** after the
`n`th machine destroyed (feature 98): a muzzle's glow, a bolt's flash
and a cut are a handful of frames each and a fight's timing moves from
run to run, so a frame count cannot catch one — and a pause holds the
passing lights still, so the screenshot taken later is that instant.
`BIMS_FREEZE=2+1 bims droids` is the fight's second shot held in the
air, and `BIMS_FREEZE=down:1+1 bims droids` the first machine bursting.
**`BIMS_FREEZE=sweep:n+f`** counts a Guardian's beams laid in the crew's
room and **`shield:n+f`** bolts and blows stopped on its shield
(`Cue::Shielded`, feature 100). The machines of `bims guardian` never
come aboard, so the crew are sent in: `BIMS_KEYS="40:X,70:V"
BIMS_POINTER="45:move:1250,420;47:click:1250,420;60:move:1250,440;62:right:1250,440"`
puts an attack banner down in the station's lobby, walks James after it
and has the camera follow him, and `BIMS_FREEZE=sweep:1+10` over that
is the beam mid-sweep. At `BIMS_ZOOM=0.5` in a `BIMS_WINDOW=2800x1800`
(pointer coordinates doubled) the Guardian is in frame, for a crop; the
rack (`BIMS_DROIDS=1 BIMS_ZOOM=0.55` and a few `wheelup`s over its
fourth row) is the idle, the wind-up and the wreck up close.
`BIMS_LAMPS_OUT=n` shoots the `n` lamps nearest the crew member
out at open and leaves the next one failing, for looking at the dark
round a lamp that is out and a failing lamp's flicker (`BIMS_LAMPS_OUT=3`
on the simulation is the deck round the crew member dark).
`BIMS_CLASS=engineer` (a name from
`names::CLASS_NAMES`, or its place in `world::Class::ALL`) puts the
class on slot 0 of any launch (`dev::class_crew`) — and on the setup
tab's chooser — and `Q`/`E` in `BIMS_KEYS` press the class's two keys
over the deck tile under `BIMS_POINTER` (features 74, 75 and 76: an
engineer's EMP and sandbags, and `C`/`R` its Healing Sentry and sentry
(task 127) — sandbags laid four minutes of the clock later at the first
rank, four seconds at 1×, the only speed since task 119 — a soldier's grenade and brace, `BIMS_CLASS=soldier`;
a grenade bursts two seconds after `Q`, so `Q` at frame 60 is a burst at
about 180 at 1×; a medic's Nanite Burst and heal beam, `BIMS_CLASS=medic`,
whose `E` wants the pointer **over another crew member** rather than over
a tile, and whose `R` cloaks the crew member under the pointer or himself
(task 130; each wants its rank, `BIMS_RANKS`); a tank's taunt and wall, `BIMS_CLASS=tank`, whose two keys want
nothing under the pointer at all — `BIMS_CLASS=tank BIMS_LEVEL=3
BIMS_KEYS="60:E,90:Q"` on `droids` puts the shield ring and the taunt's
dashed radius in one picture; a commander's rally and squad orders,
`BIMS_CLASS=commander`, whose **E** wants an enemy under the pointer and
whose `X` and `Z` — the squad's other two keys, feature 78 — want a deck
tile or nothing at all: `BIMS_CLASS=commander BIMS_LEVEL=3
BIMS_KEYS="60:Z,90:Q"` on `droids` is the squad held and the rally
called, with the aura's ring round him throughout). Hunting a crewmate with a scripted pointer is a poor way to look
at a beam, so **`BIMS_BEAM=1`** posts crew member 1 a tile from the
medic, **its bar at three fifths**, and links the beam
(`World::beam_for_probe`, `dev::beam_crew`), the beam's first rank bought
if `BIMS_RANKS` gave it none — `BIMS_CLASS=medic BIMS_BEAM=1
BIMS_SMOKE_FRAMES=90` on `droids` is the beam's line in one picture
(`BIMS_BEAM=surge` went with the surge in task 130). Both are posted where they stand, since a patient short of
nothing but hit points would walk off about its round and the beam
break at its range. At 1× a first-rank beam puts back two hit points a
second (task 130), so the green numbers over the patient climb by two a
second.
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
**`BIMS_SHAPES=cpu`** tessellates the world canvas's shapes on the CPU
into a mesh a frame, as before task 121, where the default hands them to
the GPU as records (*The world canvas's shapes drawn on the GPU* below):
the same binary both ways, which is how the two pictures are diffed and
the two costs timed. **`BIMS_LIGHTMAP=cpu`** does the same for the crew's
light map — the room's own march, where the default draws it on the GPU —
and **`BIMS_LIGHTMAP=check`** works it out both ways every frame and
prints how many bytes differ when a smoke run exits (*The crew's light
map drawn on the GPU* below).
`BIMS_SOUND_LOG=1` prints
every clip as it is played and every bed as it fades up or out, which is
how a sound is *heard* from a terminal — `BIMS_SOUND_LOG=1 BIMS_SMOKE_FRAMES=900 bims
droids | grep ^sound:` is a fight's worth. **A smoke run is silent**:
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

**There is no cap on a wave since task 132**: `DROID_WAVE_MAX` went, to
be balanced another way, and the formula grows for as long as the clock
runs. The section is kept for what it measured — what a wave of a given
size costs. The cap was sixteen because a machine is a body stepped, a
stand scored and a line traced, not because sixteen is the right number
of enemies. (`combat`, in this section's measurements and in feature 96's
and 97's below, is the command of that name as it was when they were
taken: `droids`' own ship, crew and arena, with the arena's people for
the enemy. It went in feature 102.) The measurement, taken in a
**release** build on this machine:

| wave | world step | frame, unpaced |
| --- | --- | --- |
| none (`combat`) | — | 9.2 ms |
| 16 | 0.185 ms | 9.2 ms |
| 32 | 0.292 ms | 9.0 ms |
| 64 | 0.506 ms | 9.9 ms |

The step is `cargo test --release -p world -- --ignored --nocapture
droid_waves_cost` (`tests_droid::droid_waves_cost_this_much_a_step`),
which forces the size with `World::set_droid_wave_for_probe` — raising
the cap alone never makes a wave bigger than the formula does, which
counts the players and the world clock and nothing else (feature 105). A world step is a sixtieth of a frame's budget
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
machines in it drew in the same nine milliseconds, and sixty-four of
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
one fan of rays, fourteen march fourteen, and `combat` and `droids` were
the two commands with fourteen.

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
the enemy. `bims droids`, whose crew are dealt the seven guns round and
so carry pistols among them, is where it is looked at — `BIMS_FREEZE=2+1`
holds a bolt in the air — and
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

## The world canvas's shapes drawn on the GPU (task 121)

The tessellation feature 97 left standing is gone from the world's
canvas: **a shape is sixteen floats in a storage buffer, and the GPU
draws it.** `shapes::pack` turns the painters' twelve floats a shape into
a `shapes::Record` in window points — every decision that is one a
*shape* made exactly as `ShapeBuf::replay` makes it: the cull, `Paint::of`,
a hairline's fade, a stroke with no hole left becoming a fill, the ramp
narrowed for a shape thinner than a pixel, how many points a curve gets.
`scene::WorldCanvas::shapes` hands the records to a `ShapeMaterial`
layer, and `shape.wgsl` draws a quad a record and works out in each
pixel what the triangles would have given it. The quads are one mesh a
power of two of them (`QuadMeshes`, shared by every layer that size, the
record's index in the vertex), and a layer's buffer keeps its size while
its count stays under that power of two, so a frame writes the records
into a buffer that already exists and nothing else. The panels' canvases
(the lobby's galaxy and diagram, the setup's portrait) are egui's and are
tessellated by `ShapeBuf` as before, and so are the fog's pictures, which
are textures. **`BIMS_SHAPES=cpu`** draws the world's canvas the old way
too — a `ShapeBuf` mesh a frame — in the same binary, which is what every
comparison below was made against.

**Why the shader is the same picture.** Every polygon `ShapeBuf` makes is
convex, and its feather is the ring moved out and in half a pixel along
the mitres with the colour ramped across the band. Inside the band along
an edge the colour is linear in the distance to that edge's line, and the
bands meet on the mitres, which is where two edges' lines are equally far.
So a pixel gets `0.5 − D/f`, clamped, `D` the largest signed distance to
any edge's line **of the polygon `ShapeBuf` builds** — the chords of a
rounded corner (folded into one quadrant, since the ring is the same
mirrored), the sides of the 8-to-64-sided ellipse — not of a smooth curve;
a stroke is two of those. The exception is the **triangle**: `normals`
caps a mitre sharper than sixty degrees, and the hull's 45° triangles are
all capped, which bends the band along the whole hypotenuse, so no
distance formula gives it. For a triangle the shader builds the same fan
and bands `ShapeBuf` would (capped mitres and all, `moved`, `band`) and
interpolates across whichever triangle holds the pixel, as the rasteriser
interpolated the vertex colours. The colour out is `canvas.wgsl`'s
arithmetic, premultiplied, the sRGB curve carried past one — so the bolt's
emissive core is still past white for the bloom, `BIMS_BLOOM=0` still
works, and the clip is the same scissor.

**What the picture was checked against**: the same binary paused at frame
5 (`BIMS_KEYS="5:Space"`, 160 frames, 1400×900), GPU against
`BIMS_SHAPES=cpu`, pixel by pixel of the 1.26 million — `cpu` against
itself is nought in every one of these:

| screen | pixels differing | by more than 2 levels | by more than 8 | most |
| --- | --- | --- | --- | --- |
| `design` | 3 009 | 130 | 53 | 35 |
| `design`, `BIMS_BLOOM=0` | 2 791 | 132 | 52 | 35 |
| `simulation` | 3 064 | 46 | 8 | 38 |
| the galaxy chart | 6 042 | 3 | 3 | 22 |
| `simulation`, `BIMS_AFIELD=1 BIMS_ZOOM=0.5` | 1 244 | 47 | 22 | 23 |

The ones and twos are along edges, float arithmetic against the
rasteriser's barycentrics; the few past eight are shapes smaller than a
pixel — the yard's rows of lights, dots under a pixel across — where the
rasteriser snapped the old triangles' corners to its 1/256-pixel grid and
the shader does not. By eye there is no difference, and the bloom round a
frozen bolt (`BIMS_FREEZE=2+1` with the guardian recipe's keys on
`droids`) is the same. `droids` is not in the table because it is not the
same picture twice even on the CPU path (21 696 pixels, the fight having
moved before the pause). No pinned number moved: `PICTURES` hashes the
shape buffer, which is untouched.

**What it cost and saved**, release, this machine (Ryzen 7 3700X, Navi 32
on RADV), 1400×900, `BIMS_PERF=1 BIMS_SMOKE_FREE=1 BIMS_SMOKE_FRAMES=400`,
the median of five runs each, the two paths run in turn so a drift lands
on both, bloom on (bloom off comes out the same within the spread):

| | frame by the clock, `cpu` | GPU | tessellate → pack | Bevy+egui | GPU main pass |
| --- | --- | --- | --- | --- | --- |
| `droids` | 8.14 ms (7.77–8.96) | 4.39 ms (4.14–4.49) | 2.06 → 0.16 | 1.81 → 1.03 | 0.086 → 0.145 |
| `droids`, `BIMS_DROID_WAVE=32` | 8.16 (7.68–8.71) | 4.35 (4.21–4.38) | 2.06 → 0.15 | 1.78 → 0.97 | 0.086 → 0.145 |
| `simulation` | 4.93 (4.87–5.39) | 2.26 (2.24–2.73) | 1.43 → 0.12 | 1.95 → 0.70 | 0.074 → 0.130 |
| `design` | 4.52 (4.34–4.78) | 1.90 (1.76–2.43) | 2.18 → 0.07 | 1.35 → 0.98 | 0.031 → 0.052 |
| the galaxy chart | 4.82 (4.67–5.09) | 3.61 (3.57–3.84) | 0.85 → 0.08 | 1.06 → 0.69 | 0.030 → 0.034 |

**Read the clock with care.** Unpaced, the fight's frame halves, but only
part of that is work saved: the world steps sixty times a second of real
time and the light map is marched again only for a body that moved, so a
frame twice as fast has half the steps and fewer marches in it, and the
shape buffer's row came down from 3.5 to 2.5 ms *per frame* without doing
less per second. Paced at sixty frames a second (`BIMS_SMOKE_FREE` unset,
three runs each), where every frame is the same stretch of game, the
shape buffer is 5.03 ms on the CPU path and 5.07 on the GPU's, and what
is saved is the tessellation and nothing else: the screen's frame 7.93 →
6.13 ms in `droids`, 3.09 → 1.89 in `simulation`, 2.88 → 0.88 in
`design`. The other saving is off the app's thread: the render thread no
longer allocates and copies a mesh of every triangle each frame (about
1.3–1.6 ms of its time by `perf`), which is most of the drop in the
"Bevy and egui" row. The GPU pays about 0.03–0.06 ms a frame more in the
main pass for it; the bloom is unchanged. `canvas to bevy` (`scene::sync`)
went from 0.006 to 0.067 ms, the records turned into bytes. Before this
change the old binary measured the same as `BIMS_SHAPES=cpu` does, so the
dial is the old cost.

**Not done, and why** (measured first, in the task's own analysis):

- **The light map** was left out of the first half and then done, at the
  user's word, as the same picture byte for byte: *The crew's light map
  drawn on the GPU* below.
- **A persistent buffer for the old triangles** instead of the records:
  only the render thread's copy goes, and the app's thread bounds the
  frame. The records made it moot.
- **The fog's blur and compose on the GPU**: about 0.1 ms, only on a
  frame the map changed, for a possible level either way.
- **Not GPU work, and bigger than the last two**: the ship's design grid
  rebuilt every frame by `dock::port` and `Aboard::on_ship` (0.5–0.9 ms)
  and `World::reachable_stars` generating the galaxy every frame the
  chart is up (2.2 ms of its 2.6 ms "canvas ui"). Task 122's.

The measurements are one machine's, and the GPU columns are Bevy's
timestamps, which egui's own pass is not among.

## The crew's light map drawn on the GPU (task 121, second half)

**The march, the composing, the blur and the colouring of the crew's
light map run on the GPU**, and the picture is
`Sight::light_map_on_cpu`'s and `fogmap.rs`'s **byte for byte** — not
near it: the pinned `the_light_map_is_the_same_picture_it_was` still
tests the CPU march it always tested, and nothing was re-pinned. How
that is possible is the whole design:

- **The room hands over inputs, not a picture.** With
  `bims::sight::set_host_draws` on (the app turns it on at start unless
  `BIMS_LIGHTMAP=cpu`), `Sight::light_map` calls `light_inputs` instead
  of marching: the lamps' fields built as before (`powf` stays on the
  CPU), each body's eyes compared with the ones last handed over by the
  CPU's own half-a-pixel rule and handed over again only when they moved
  — every body again when `cells_version` moved, which is exactly when
  the CPU throws its views away — and it all goes out as
  `LightMap::inputs` (`LightInputs`: the cells a byte a tile, the two
  fields, the views with a revision each, and three tables), the two
  planes left
  empty and the extent kept, so `Session::light_map` and
  `light_map_on_screen` are unchanged.
- **The only division a ray makes is made on the CPU.** `march_rays`
  divides once a ray, to find its first two edge crossings; after that
  the walk is additions and comparisons, which the GPU rounds as the CPU
  does. So each marched eye carries its 4096 rays' first crossings
  (`EyeInputs::t0`, `ray_starts`, the same expression), the per-ray steps
  are `sight::ray_table()`, and an infinite crossing goes up as
  `f32::MAX`, which takes the same turns. The dark rule's distance to the
  body is a bit a tile worked out on the CPU (`ViewInputs::near`), and a
  peek's `admits` is integers. Which pixels a body sees is a **set** —
  the order the rays run in cannot change it — so 4096 threads an eye
  writing bits with `atomicOr` is the CPU's answer.
- **Every float the composing and the colouring read is a table.** The
  dark, the lamplight seen and the lamplight under the fog are 256-entry
  tables made on the CPU by `compose`'s own arithmetic
  (`LightInputs::{dark, glow_seen, glow_fog}`), and the fog's texel is a
  65 536-entry table of `fogmap::texel` (the CPU's upload now goes
  through that function too). The blur is integers. So the shader rounds
  nothing the CPU rounds differently.
- **The composing runs over the whole map** whenever anything changed,
  where the CPU composes the changed box: the same bytes, since outside
  the box nothing it reads moved — the check below is what says so.
- **Where it runs**: `crates/app/src/lightmap.rs`, a system in Bevy's
  `RenderGraph` schedule between `Begin` and `Render`, on the render
  world's copy of the frame's `LightJob` (extracted every frame and
  emptied at the start of the next, so a frame that draws no fog draws
  nothing). It writes what changed, runs `lightmap.wgsl`'s three
  passes — `march` (a thread a ray of a moved eye), `compose` and `blur`
  (a thread a pixel) — and copies the texels into the fog's `Image`
  (made blank, `RENDER_WORLD`, by `FogTexture::blank`), submitted before
  the main pass of the same frame, so the fog is never a frame behind.
  Raw wgpu through `RenderDevice`: no Bevy shader asset, no pipeline
  cache, compiled once at the first map. The pass carries a Bevy GPU span
  (`perf: gpu light map`). The planet plain's own fog
  (`terrain::Plane::picture`) is still the CPU's.
- **Nothing drawn here is read back.** Until task 128 the GPU kept the
  *explored memory* — every pixel a line of sight had ever reached, the
  grey over a stranger's deck — and read it back before a save
  (`lightmap::GiveBack`, `Game::give_back_explored`). The fog is one fog
  now (*One fog over the whole map* below), so there is no memory, no
  read-back and no lag: the GPU's map is this frame's inputs and nothing
  else. A map is still keyed by the sight's `PictureId` (fresh for every
  sight made, cloned or loaded), since the views' and the cells'
  revisions are one sight's. Nothing the simulation reads is in any of
  this: the rules read the tile masks (`lit`, `seen`).
- **`BIMS_LIGHTMAP`**: `cpu` is the room's march as it always was;
  `check` works both out every frame, reads the GPU's map and texture
  back and counts the bytes that differ — printed at a smoke run's exit.

**What the check said**, 1 500 frames each (`BIMS_LIGHTMAP=check`, the
crew walked in with the guardian recipe's keys): `droids`, `defense` (a
town, landed, daylight), `test` (a random station), `jammer`,
`simulation` and `test_planet` — **0 map bytes, 0 explored pixels and
0 texel bytes differing** in every one, and in 150 reads of the explored
memory 0 pixels wrong (13 to 558 *behind* over 25 reads, the frame or two
of lag). Since task 128 there is no explored memory, and 899 frames of
the walked-in `droids` came out at 0 map bytes and 0 texel bytes.
`a_host_marching_the_inputs_draws_the_map_this_crate_draws` in
`crates/game/src/sight.rs` is the same walk and composing written out in
Rust against `light_map_on_cpu`, frame by frame — a peek, a door
shutting, a lamp shot out, a body fewer, a closet nobody sees — so a
change to the inputs is caught without a GPU.

## One fog over the whole map (task 128)

**The fog is Dota's**: the whole map of a station, a town and the plain
is always drawn, and every fogged pixel nobody sees is `MAP_FOG` (0.62)
— the crew's own ship, a home station, a neutral or a hostile one, the
plain, looked at before or not. There is no black and no grey: the
explored memory went (`Sight::explored`, `explored_px`,
`Plane::explored`, the plain pictures' bits, the GPU's buffer and its
read-back), and so did `FOG_BLACK`, `FOG_GREY`, `MAP_GREY`, the terrain's
`VEIL_GREY`/`VEIL_BLACK` (one `VEIL_FOG`) and `CELL_FRIENDLY`. What is
seen is unchanged — nought lit, `MAP_DARK × (1 − light)` unlit, the glow
as it was — and the lamplight shows under the fog at `GLOW_UNDER_FOG` on
every structure (the plain has none). **What the fog hides is who is
standing there**: the crew are always drawn, everybody else only in sight
and the linger after (`Game::body_seen`, `seen_for`). The trace, the
dark range, the peeks, `seen_at`, `last_seen`, the alarm and every AI are
untouched: this is the picture's alone, and `SURVIVORS` did not move.

- **The stance is not the fog's any more.** `Stance` stays — the world
  reads it for who is at war — but the sight keeps none:
  `Sight::set_stance`/`set_foreign` went, and `Game::set_stance` and
  `set_foreign` keep theirs for `is_aboard` alone.
- **Looked at from outside** (`Fog::All`, `Sight::draw(list, true)`) is
  every fogged tile in the one `FOG`, whoever's it is: the structure shows
  through, nobody is drawn. The far plate beyond the residents' range
  (`HULL_UNKNOWN`) is unchanged.
- **The rule is pinned**: `what_the_crew_see_is_brighter_than_the_fog_on_any_deck`
  (`sight.rs`: a seen pixel at no light and at full light against an
  unseen one, and the same deck friendly, neutral and hostile the same
  picture byte for byte) and
  `what_the_crew_see_of_the_plain_is_brighter_than_the_fog`
  (`terrain.rs`). `the_light_map_is_the_same_picture_it_was` did not
  move: its room is all the crew's own, and only unseen pixels of
  somebody else's structure changed.
- **What now shows through the fog.** A few pictures under the fog move
  because of a body nobody sees, and they were hidden under the black
  before; none was changed: the joined deck's powered doors, which open
  for the station's people and the machines (`Game::set_visitors`); a
  lamp shot out or flickering in a fight nobody sees (its glow under the
  fog, and the glass the ship painter draws); the residents' room's
  blood, scorches and footprint trails (`Bim::trail`, drawn for every
  Bim that walks); a machine's thrown plates (`Fx::draw_debris`, not
  gated by `body_seen`); the machines' lander on the plain and their ship
  at a station's far airlock (`World::droid_ship`, there while any of the
  wave stands); and the engineer's deployables, whose damage and loss say
  where an unseen enemy is. The bodies themselves — a corpse, a wreck, a
  grave — are gated, and what flies over the fog (the bolts, a burst's
  flash, a sweep) always showed.

**What it saved**, release, this machine (Ryzen 7 3700X, Navi 32),
1400×900, the same binary with `BIMS_LIGHTMAP=cpu` against the default,
interleaved, bloom on, on top of task 122's two cores. Unpaced, four runs
each:

| | frame by the clock, `cpu` | GPU | the light map on the CPU | GPU pass |
| --- | --- | --- | --- | --- |
| `droids` | 3.11 ms (3.07–3.26) | 2.41 ms (2.35–2.43) | 0.58 → 0.04 | 0.20 ms on ~1 frame in 7 |
| `droids`, the crew walked in | 3.87 (3.80–4.21) | 2.59 (2.58–3.00) | 0.88 → 0.04 | 0.26 ms on ~1 in 6 |
| `defense`, walked in | 3.92 (3.71–4.12) | 3.37 (3.23–3.44) | 0.12 → 0.02 | 0.55 ms (a town's map is bigger) |
| `simulation` | 1.78 (1.71–1.84) | 1.75 (1.74–1.77) | nothing to march | — |

Paced at sixty frames a second, which is how the game is played and
where more moves a frame, three runs of the walked-in `droids`: the
screen's frame **7.16 → 3.13 ms**, the light map on the CPU 3.24 → 0.12
ms (3.1 views marched a frame, 2.84 ms of marching), the GPU pass 0.47 ms
on about four frames in five — and by the clock the CPU path **could not
hold sixty** there (17.5 ms a frame) where the GPU path does (16.84).
`simulation` is inside the noise: a crew of one standing still marches
nothing either way.

**Measuring on this machine wants it idle.** A first set of these runs
said the GPU path was a millisecond *slower* by the clock; the desktop
was running a game on the same GPU and another agent's tests were on
the cores (load average 16). With the machine quiet the same binary gave
the table above. A run whose range reaches past ten milliseconds is that,
and is thrown away.

## The shape buffer on two cores (task 122)

`Session::render` builds the **stations' own pictures while the crew's
room draws itself**, and nothing else of it runs beside anything. A
station's picture is its rim, its tiles and the shade along its walls
(`world_paint::station_picture`), built from its design and a
`StationWork` — its door and how far it stands open, whether its room is
open, a settlement's biome — read off the world first
(`stations_to_build`), so the job borrows `World::stations` and
`World::surfaces` and nothing else while the crew's room holds
`World::aboard`. `world_paint::stations` then takes each prebuilt picture
whose `StationWork` is the same, builds any it lacks the same way, and
adds the lights, the lamps' glass, a relic cache and the machines' ship
serially, in the order it always did; `world_paint::paint` is
`paint_with` given none, so every other caller draws as before. One
thing moved in the order: **the picture clocks** — `Game::frame` and
`tick_airlock` — are ticked before the rooms draw rather than after,
since the station's airlock is drawn off `airlock_ajar`; neither room
reads either.

**How the two jobs run is the host's word**, `ship::fork`: `set_join`
takes a `Join` once, and without one it is `fork::serial` — every test,
probe and server, and a target with no threads. The app's is
`pool_join` in `main.rs`: the stations' job on **Bevy's compute pool**,
the crew's room on the frame's own thread, so no second pool competes
with Bevy's. `BIMS_RENDER_JOIN=serial` runs them in turn and `=thread`
spawns a thread a frame (`fork::scoped`), which is how the three were
measured against each other. `tests_render.rs` in `crates/ship` is the
proof it is the same picture: two sessions stepped alike, one drawn with
`serial` and one with `scoped`, every float of every frame bit for bit
and the fog's cut, on the simulation's dock, the droids' arena, a town, the
map and the yard; `PICTURES` holds the serial buffer to what it was.

**`BIMS_PERF=1` breaks the shape buffer down** since this task
(`bims::timing`, std only, a module of the room's so the painters under
the app can reach it): the crew room with its `observe` and light map
(and the views marched in it, and how many a frame), the station room,
the ship's own state, the world painter and inside it the stations — a
station's picture as grid and skip, hull tiles and shade, and its lights
— and the editor's painter. **Measure the light map paced**: at two
hundred frames a second a walking body moves under half a light-map
pixel a frame and is marched a third as often as at sixty, so an
unpaced run puts the march at a third of what a player's frame pays.

What it measured, a Ryzen 7 3700X (8 cores, 16 threads) and a Navi 32
Radeon, 1400x900, release, the shape buffer's own CPU time a frame, the
median and the spread; paced at sixty is three runs, unpaced five, the
three modes taken in turn:

| | serial | thread a frame | Bevy's pool |
| --- | --- | --- | --- |
| `droids`, paced | 5.22 [5.12–5.58] | 4.68 [4.52–4.75] | **4.65** [4.64–4.73] |
| the fight (crew sent in), paced | 5.55 [5.53–5.59] | 5.09 [5.06–5.11] | **5.04** [5.03–5.08] |
| `simulation`, paced | 1.46 [1.46–1.49] | 1.44 [1.44–1.45] | 1.39 [1.38–1.43] |
| `droids`, unpaced | 2.81 [2.61–3.41] | 2.61 [2.58–2.75] | 2.56 [2.45–2.58] |
| `simulation`, unpaced | 1.41 [1.35–1.46] | 1.45 [1.43–1.47] | 1.41 [1.39–1.45] |

So a fight's frame is half a millisecond lighter — 0.5 to 0.6 ms, clear
of the spread — where the crew's room (3.6–4.0 ms, the light map most of
it) is longer than the station's picture (0.8 ms) it hides; the most it
could hide was that 0.8, and the rest is the handing over. The
simulation's crew room is 0.03–0.2 ms, so there is next to nothing to
hide behind and the gain is inside the noise. A thread a frame costs
20–30 µs to start (measured apart), about as much as the pool's
difference from it here.

**What a paced frame is made of now**, `droids`: the crew's room 3.6 ms
of which the light map is 3.0 (3.6 views marched a frame, 0.82 ms a
view, about 70% of frames marching and up to sixteen views in one), the
station's picture 0.85 on the pool (grid and skip 0.29, hull tiles 0.56),
the world painter 0.5, the ship's state 0.17, the station's room 0.02.

**Not done, and why:**

- **The light map a body at a time on the pool** — the largest share and
  the same picture bit for bit, but the lighting is going to the GPU,
  which replaces the march rather than needing it split.
- **`observe` on the pool**: its trace ORs every eye's tiles, so a split
  would be the same answer, but it is also where `Sight::set_shut`
  rewrites the cells the fight's line of sight reads (task 121's note
  above), and that is to be decided before anything moves it.
- **The two rooms side by side**: the station's is 0.02 ms, less than
  handing it over costs.

### A station's picture is kept from frame to frame

The overlap's follow-up, the same task: **a station's picture is built
only when what it is built from changed**, and used again otherwise.
`ship::Game::kept_stations` holds this frame's `world_paint::KeptStation`s
— the picture with its `StationWork` and a **copy of its design's build
area and every part**, id, kind, place and turn, in the design's order —
and `KeptStation::fits` compares them outright each frame, not by a hash
(`design_hash` sorts the parts and leaves the ids out, and the picture is
drawn in the design's order and skips by id, so it would not do). A
design written anywhere, for any reason, is a picture built again;
nothing had to be told to bump a counter. `Session::render` asks
`stale_stations` which no longer fit — the first frame a station is drawn
whole, a design that changed, **an airlock swinging** (its opening is in
the `StationWork`) — builds only those, beside the crew's room as before,
and `keep_stations` keeps this frame's and nothing else. The lights, the
lamps' glass, a cache's ring and the machines' ship change every frame
and are drawn after it in a list of their own, placed straight after the
kept picture, which is the same floats as one list placed. Only
`Session::render` keeps anything: `world_paint::paint` hands the painter
none, so no other caller can be drawn a picture of a design since
changed. `a_kept_station_picture_is_the_one_drawn_from_nothing` in
`tests_render.rs` holds the kept frames to `paint`'s from nothing, a
still frame to building nothing, and a floor tile taken off the dock to
a picture built again without it.

What it measured, the same machine and the same way, the shape buffer's
CPU time a frame against the overlap alone (both on the pool):

| | overlap | kept |
| --- | --- | --- |
| `simulation`, paced | 1.42 [1.38–1.43] | **0.90** [0.90–0.91] |
| `simulation`, unpaced | 1.36 [1.36–1.41] | **0.72** [0.72–0.73] |
| `droids`, unpaced | 2.50 [2.38–2.53] | **1.63** [1.61–1.70] |
| the fight, paced | 5.46 [5.14–7.99] | **4.84** [4.82–5.58] |

The simulation's shape buffer is about half what it was, on one core;
in the fight the station's picture is built on about one frame in nine
— the airlock swinging as the crew come and go — and 0.1 ms a frame is
what is left of it. Finding which pictures still fit costs about 0.12
ms a frame of the ship's state (the stations in view read twice and the
parts compared), which the table includes. Paced `droids` is left out:
another agent's builds landed on those runs, and the crew's room, which
the cache does not touch, came out anywhere from 3.6 to 6.5 ms.

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

Bims became a roguelike top-down co-op shooter — humans against the
machines, one default ship taken from site to site, time and money the
only resources — in three steps, and **this one switched off what the new
game does not use and put the run on its new footing; nothing was
deleted.** Step two built the travel and mission loop (feature 103, *The
loop* below) and step three deleted the dead code (feature 104, *The old
game deleted* below). What was off was off behind a **switch on the
world**, saved and in `world_checksum`, off in every run and on only for
the tests of what it switched. Three of the four went in feature 104 with
what they switched:

- **`World::needs_enabled`** (`set_needs_enabled`) was the room's whole
  life sim — the needs draining and sending anybody on an errand, a pot
  going bad, the bays grown and tended, the bunks dealt and counted
  against a hire, the mess biting, the food in a dark cold store spoiling.
  The world told every room every step (`hand_the_rooms_the_needs`, since
  a room is built afresh at every dock), and the room held it as
  `bims::game::Game::needs_enabled` — on in the classic room, so `bims
  room` and the needs probes in `scratchpad/` stayed what they had been.
  Off, `tick_bim` stood the levels still, handed `Health::update_held` a
  fed and rested body (so the blood, the wounds and the mending still
  ran), started no need's errand, offered no cook, tend or sweep, and
  `hit_at` opened no menu on a fixture that only served a need — which is
  the room as it is now, with everything the switch hid deleted.
- **`World::human_foes_enabled`** was the generator's hostile stations and
  towns put on the `hostile` list (`rolled_hostile`, at a start, a jump
  and the switch), raiders coming (`run_raid`'s quiet arm), an enemy's
  shelf laid out and plundered, and a station's dead looted; a probe's own
  `set_hostile` worked with it off, and `raid_for_probe` switched it on.
  The generator still **rolls** `StationBlueprint::hostile` — it is in the
  galaxy checksum, it is where the tier-two research keys lie, and the
  spawn still skips a blueprint rolled hostile — but nothing reads it as a
  stance: `World::stance` says a droid-held station is hostile, home
  friendly and the rest neutral, and the lobby rings nothing in red.
- **`World::radiation_enabled`** was stage eight's dose; off, nobody was
  exposed, in a suit or out of one. The dose, the stage and the `health`
  crate under them are gone.
- **`World::shipyard_enabled`** is still a switch, and still off in every
  run: a construction site placed (`SiteRefusal::NoShipyard`,
  `Refusal::NoShipyard`) and what the ship lives on bought
  (`World::buyable` — gear, and nothing else, with it off;
  `Session::sold_here` greys the rest). The class deployables are not
  parts and are untouched; the Build tab is not shown.

The rest of the footing: **the crisis is there from day nought**
(`crisis_first_day` is nought; `DROID_FIRST_DAY`, which nothing read after
this, went in feature 104), so the origin is theirs at the start and a
system due is theirs whole the moment the crew are in it; **the `game`
flow has no design phase** — Start is `screens::designer::start_run`,
which stands `Session::run` up on every machine of a lobby from the same
numbers: the playtest ship and `money_per_bim` a player's Bim, the lobby's
default `START_MONEY_PER_BIM`; the setup tab has no ship-size row. **A
station's people keep a routine** (`bims::routine`, dealt by
`Residents::deal_roles` from `World::open_residents`): a role off the
station's seed and the body's place — guard, trader, worker, civilian —
and a round of stops derived from the room's own fixtures
(`routine::Anchors::of`), with a wander over reachable open deck as every
role's fallback. The round rides on the `Bim` (`adopt` shifts it with the
body) and is walked by `Game::keep_to_routine` while the body is its own —
not under arms, posted, sheltering, running or given an errand; until
feature 104 that was only with the needs off, and never for an enemy's
garrison. The commands: `droids` is the fight; `combat`, `combat_<class>`
and `raid` went; `tier2_test` and `tier3_test` are `droids` at a tier.
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
  the clock stands still — and the rooms' own clocks with it — and
  everything inside a mission runs on the **mission clock**,
  `World::mission_steps`, nought on arrival: the droid waves
  (`DROID_REINFORCE_STEPS`), a town's first wave (`DEFENSE_DELAY_STEPS`)
  and every class cooldown. The world map is the chart with a column down its right
  (`screens/worldmap.rs`, laid out anew in feature 107); the helm's four orders were refused
  (`Refusal::TravelIsResolved`) until feature 104 deleted them, the helm
  and the flown trip together. Nothing tells a room its clock stands
  still any more: `Game::simulate` never advances it, `Game::wind_clock`
  is the one thing that moves it, and the world winds every room it
  builds to the world's minutes (`crates/game/CLAUDE.md`, "The needs
  deleted"). Until feature 104 the world said so every step through
  `Game::set_clock_runs(false)`, which went with the needs.
- **The old game's clock was a switch**, `World::set_free_clock`, off in
  every run and on for the tests of flight, raids, wages and the day by the
  step, in the pattern feature 102 set. Feature 104 deleted it with the
  flown trip: a test that wants the clock on steps a mission and reads
  `mission_minutes()` (as the engineer tests' `run_for_seconds` learnt to),
  and wages are paid by `World::travel` alone (`pay_wages_due`).
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
system and the charge is twenty minutes. Since feature 105 (below) every
trip is at least `MIN_TRAVEL_HOURS` (a day) and the site the crew are at
is refused, so the table's shortest trips are a day now.

## Time is what the machines scale on (feature 105)

The fourth step of the redesign. **`droid::wave_size(players,
time_steps)` and `droid::wave_count(time_steps)` read the world clock and
the number of player Bims and nothing else**: a wave is
`DROID_WAVE_BASE` (2) + the players + a step every `ENEMIES_HOURS`
(three weeks) of the world clock — capped at `DROID_WAVE_MAX` until
task 132 took the cap away; a station's count is `DROID_WAVES_BASE` (2) + a wave every second step.
Bots, mercenaries, recruits, the crew's worth and their levels never
enlarge a wave — getting stronger makes the fight easier and money kept
is not punished. `World::worth` and `start_worth` stay, for
`mercenary::how_many` alone; `worth_steps`, `day_steps`, `crew_levels`
and `ENEMIES_DAYS` went. `ENEMIES_HOURS` was set off
`tests_mission::travel_days_over_ten_galaxies`, which prints the run to
the origin now as well — its doc comment in `data.rs` carries the
reasoning.

With time the only lever, **a trip always moves the clock**: the site
the crew are at is refused (`Refusal::AlreadyHere`, 92; the map lists it
as *here*), and every trip is at least `data::MIN_TRAVEL_HOURS` (24) —
`TravelQuote::minimum` says when that is what it is, and the map writes
*the minimum* beside it. A site left uncleared is put back as the crew
met it, so without those two a crew could step out and back in and meet
the same fight at the same strength as often as it liked.

The `droids` commands force their waves to `session::COMBAT_WAVE`
(sixteen, what the old formula gave their sixteen crew), so the fight
they are for did not shrink to three. **`SURVIVORS` moved on purpose**
(the constant's note: the minimum trip, and nothing else — both its
worlds force the wave size); `REFERENCE_CHECKSUM` did not.
`tests_droid.rs` and `droid::tests` pin the inputs, `tests_mission.rs`
the two travel rules.

**Not done, because it is not there**: the spec's Commander
"call-in-reinforcements" (to become once a mission) and its "temporary
Republic soldiers" — the commander has a rally and a squad of the crew's
own bots, and nothing calls a soldier in — and "liberation", which nothing
in the code does yet.

## Relics and unlocks: research left the game (feature 106)

The fifth step of the redesign. The world's half is
`crates/world/CLAUDE.md` ("Relics, and research gone"); the player's is
`README.md` ("Relics, and what a won run unlocks"). What to hold on to:

- **Research is out of the game flow.** `World::research`, the five
  research commands, their events (44–47, 65–66) and refusals (22–25,
  39–40), `station_keys` on the world and in every system's memory, and
  `Station::key` with its roll are **deleted**; the workbench upgrades
  without a node, `craft_orders` and `can_place_site` ask no tree.
  `shipdesign::research` and the designer are untouched — the palette is
  still `Research::new()`'s — and the key resources still exist in
  `physics::ResourceId`, unmade and untaken.
- **Tier two waits on time**: `World::site_tier(star, station, clock)` is
  the one rule the wave (`droid_tier`) and the map's quote read — tier
  three within `DROID_TIER_THREE_HOPS` of the origin, else tier two past
  `data::ENEMY_TIER2_HOURS` (a fortnight) on the ramp from the crew's own
  star (`relic::tier_two_rolled`: `hops` in `ENEMY_TIER2_SURE_HOPS`,
  sure from there, off the galaxy's seed a site), else tier one.
  `World::home_hops` is derived and never saved, as `droid_hops` is.
- **A relic is a row** of `world::relic::RELICS` — id, tier, whether a new
  profile has it, and an `Effect` of three kinds of hook: a stat
  modifier, a trigger with conditions, or the overcharge the room counts.
  Every number is a `data.rs` constant, every word `names.rs`
  (`RELIC_NAMES`, `relic_line`). The world's side is `relics.rs`, a child
  of `world` like `mission.rs`; the state is `Run::relics` (saved, and in
  `world_checksum` whole).
- **Randomness is off the galaxy's seed, never a fight's stream**: an
  offer is seeded by the site and the count of offers so far
  (`draw_relics`), a cache and the tier-two ramp by the site
  (`relic::site_roll`). Nothing a relic rolls moves a draw the room makes,
  which is why `SURVIVORS` and the ship's `PINNED` did not move.
- **The reward screen is a phase**, `run::Phase::Reward` (code 2): the
  step does nothing in it but the commands, as on the map, and the map
  comes up when the choice is carried (`relic_if_carried`). A trip
  proposed in it is `Refusal::ChoosingRelic`.
- **The profile is the app's** (`crates/app/src/profile.rs`,
  `bims/profile.ron` beside the saves, or `BIMS_PROFILE_DIR`); the rule
  for what a win unlocks is the world's (`relic::Profile::record_run`).
  The host's pool and open classes cross the lobby in `SettingsWire`
  (`relics`, `classes` as bits) and are kept for the run as
  `profile::RunUnlocks`; `start_run` sets the pool before the first step.
  The end screen writes the win into the profile the first frame it
  shows it (`profile::Victory`), once.
- **`World::run_won` is the one place a run is won** — said once as
  `WorldEvent::RunWon`; the app ends the run on it as on `CrewLost`. The
  Machine Heart's core destroyed calls it (feature 108); `BIMS_WIN=1`
  still does too, as a way to the victory screen without the fight.

**What moved.** `SAVE_VERSION` **37**, `wire::PROTOCOL` **29** (the relay
wants redeploying), `REFERENCE_CHECKSUM` `0x_dced_2c7b_c7e4_bf7e` (the
research and the keys out of the hash, the relics, the cache and the
shots in), and the ship's `PICTURES` for `simulation_deck` and
`droids_deck`: the gold ring round the spawn's research desk went with
its key, and drawing it by the old rule again gave the old numbers back
bit for bit. `SURVIVORS` and `PINNED` did not move.

**Not done, because it is not there**: a bandage heals nothing — it
closes wounds — so *Trauma Kit* raises the medkit and the beam and not
the bandage. And the win itself waited on the end boss — feature 108, below.

## The Machine Heart, and winning a run (feature 108)

The sixth step of the redesign: the end boss, and the win it gives. The
world's half is `crates/world/CLAUDE.md` ("The Machine Heart"), the room's
`crates/game/CLAUDE.md` ("The Machine Heart's machines"), the player's
`README.md` ("The Machine Heart"). What to hold on to:

- **The fortress is a derived station**, the way a derived jammer is:
  `world::heart` rolls it off the origin star's own stream (its id
  `HEART_BASE | star`), `World::settle_heart` lays it — called at the end
  of `settle_jammer`, so at the start, a jump, every load and the step a
  system falls — and it is never saved. Its layout is `Plan::Fortress`:
  the hub at the arena's size less the big plant in the middle (the core
  stands there) and with two standing lights in the hub, since the hub's
  own lamps leave its middle tile dark. It is never the jammer.
- **The core, the conduits and the fabricators are machines**:
  `DroidKind::{Core, Conduit, Fabricator}` (5–7, `DroidKind::HEART`, not in
  `ALL`), one health each (`DroidBody::solid`), laid first on the deck so
  a wave cleared keeps them (`Game::clear_wave_droids`), never counted as
  a wave standing. So every bolt, blow, grenade, bounty and wreck the
  fight had serves them unchanged. A sealed core's shield is the zero
  vector, which `combat::shield_stops` reads as all round, and nobody aims
  at one of their own accord (`Target::sealed`).
- **The phase is the world's**, on the fortress's `Infestation::heart`
  (`HeartFight`, saved and hashed — hashed only where it is, so no other
  fight's number moved) and read off the room each step
  (`World::heart_step`, before the loss is checked); what the phase allows
  is told to the core on `Droid::heart` (`HeartState`). The core down is
  `World::run_won`, and a run won stays won (`check_run_lost`).
- **Leaving puts it back** through the site snapshot that already put a
  held station back: the fight is on the `Infestation`.
- **The map's preview** is `TravelQuote::heart` (`World::heart_preview`),
  off the same `wave_size_at` / `wave_count_at` the fight uses, so a
  quote and the fight it is for agree.
- **The run's summary** is `World::run_summary`: `Run::{machines_destroyed,
  sites_cleared, systems_liberated}` (a system is liberated when its
  jammer is the site cleared) beside the days and the deaths.

**What moved.** `SAVE_VERSION` **38**, `wire::PROTOCOL` **30** (the relay
wants redeploying), `REFERENCE_CHECKSUM` `0x_d534_2dda_6563_babe` (the run's
three counters in the hash). `SURVIVORS` and the ship's `PINNED` and
`PICTURES` did **not** move: `DroidBody`'s `Debug`, which the survivors'
reading takes a machine's body by, is written out by hand so a body of four
parts prints what it always printed.

**Not done, because it is not there**: "systems liberated" has no
liberation mechanic behind it, so it counts jammers cleared.

## The Manufacturers (feature 109)

The human enemy, from the first day of a run: the people who built the
machines. The world's half is `crates/world/CLAUDE.md` ("The
Manufacturers"), the room's `crates/game/CLAUDE.md` ("A Manufacturer is a
hostile Bim"), the player's `README.md` ("The Manufacturers"). What to hold
on to:

- **A site of theirs is a held site with a flag** — an `Infestation` with
  `manufacturers` set — so everything a held station has (hostile, nobody
  living there, the waves, the clear, the bounty, the relics, the
  put-back on leaving) is the machines' machinery unchanged, and
  `World::is_droid_held` reads as "held by an enemy". What differs is who
  is laid on the deck (`World::lay_manufacturers`: their people as hostile
  Bims, `Game::enlist_manufacturer`, and Troopers beside them before day
  ten) and three numbers: one wave before day ten, the machines' count
  after, thirty seconds between (four hours of the mission clock until
  the station waves were brought to half a minute apart).
- **Which sites is stateless** (`world::manufacturer::holds`): a roll a
  site off the galaxy's seed and at least two made up within two lanes of
  home, none in it — derived at the start and at every load, never saved.
  Never infested, never a jammer, never taken by the spread.
- **The whole schedule is `data::MANUFACTURER_*`**, gear by day included.
- **Experience changed for every enemy**: ten once, at its first down or
  death; `XP_ENEMY_DEAD` went. That moved `SURVIVORS`, and nothing else of
  the feature did (its note).

**What moved.** `SAVE_VERSION` **39**, `wire::PROTOCOL` **31** (the relay
wants redeploying), `SURVIVORS`. `REFERENCE_CHECKSUM` and the ship's
`PINNED` and `PICTURES` did **not** move.

## Procedural stations and towns (feature 112)

Every station but the spawn, and every planet's town, is a layout
**generated from its seed** rather than one of six drawn plans or one
street template. The world's half is `crates/world/CLAUDE.md` ("A station
is generated from its seed", "A town's streets are drawn from its seed");
the player's is `README.md`. What to hold on to:

- **`Plan::Generated`** is what `Plan::rolled` answers for every station;
  `station::resolve` builds it through `crate::stationgen` and, when
  `stationgen::ATTEMPTS` (24) candidates all fail, falls back to the drawn
  plan the seed rolled before (`Plan::hand_rolled`), which `Station::plan`
  then says. The spawn is still `Plan::Hub` (`World::start`'s `replan`),
  the arena still `station::arena`, the fortress still `Plan::Fortress`:
  their designs did not move. The town's is `surface::build_town`, falling
  back to the old template (`surface::template_floor`).
- **The seam is the `Floor`**: the generators draw one and `furnish_placer`
  fills it unchanged. A candidate is furnished and then checked by a flood
  stricter than the room's navigation (`stationgen::reach`: doors and
  airlocks reached by a body two tiles wide, spots and deck from there a
  tile at a time); the room's own `Nav` walk stays in the tests.
- **Pure in the seed**: integers, `worldgen::rng` off salts of their own
  (`stationgen::GEN_SALT`, `towngen::TOWNGEN_SALT`), so the plan roll, the
  dressing and the wild are the streams they were.
- **Gates are data** (`surface::Gate`, on `Station::gates` and
  `Floor::gates`): two or three, where a street meets the north, south or
  east wall. `surface::gates()` went, and `GATE_X0` and `GATE_WIDTH` are the
  template's private constants now; the
  guard's round reads the station's gates, and a wave's lander takes them
  in turn (`surface::gate_for_wave` — the template's north-then-south is
  that rule on two gates).
- **Dials**: `BIMS_STATION_SEED=<n>` (and `BIMS_STATION_KIND`) on `test`
  and the `droids` commands rebuilds the dock as that seed generates it,
  and on `test_planet`, `droids_planet` and `defense` draws the town from
  it; a run prints `station:` or `town:` lines saying what to ask for
  again. `BIMS_STATION_PLAN=Ring` (any `Plan::ALL` name) forces a plan on
  every station, `=legacy` the old layouts everywhere — what the pins were
  checked against.
- **Looking at one without a window**: `BIMS_NAV_MAP=Generated
  BIMS_STATION_SEED=<n> BIMS_STATION_KIND=Orbital cargo test -p world
  nav_map -- --ignored --nocapture` prints a station; `BIMS_NAV_MAP=Surface`
  with `BIMS_TOWN_SEED`, `BIMS_TOWN_BIOME`, `BIMS_TOWN_POPULATION` a town;
  `BIMS_SKETCH=<name>` beside either writes it in the station builder's
  format to `stations/<name>.txt` (`station::sketch_text` — fixtures, the
  skin and a town's open ground are what the format cannot say). A whole
  town without the fog is `BIMS_TOWN_SEED=<n> BIMS_SVG_REACH=58
  BIMS_SVG_SHIFT=48 cargo test -p ship a_landed_picture -- --ignored
  --nocapture`, an SVG; a window screenshot of a landed town is black
  until somebody has looked at it. `print_candidate` and `print_town` in
  `tests_layoutgen.rs` print a candidate with where the flood stopped.

**What it measured** (release, this machine): building every station of a
system went from 0.90 ms to 6.3 ms (`tests_layoutgen::layout_timings`;
`shipdesign::validate` in the loop was 85% of a first 42 ms and is the
tests' now), and a town from 1.5 ms to about 14 ms — built when landed on,
as before, so nothing was made lazy. Over a thousand seeds a kind nothing
fell back; the spread is `generated_stations_keep_their_invariants` under
`BIMS_SWEEP=1`. The plain `a_station_s_rooms_and_a_one_tile_corridor_can_be_walked`
went from 12.5 s to 20.2 s (every kind generated at one seed).

**What moved**: `SAVE_VERSION` **41**, `wire::PROTOCOL` **33** (the relay
wants redeploying), `REFERENCE_CHECKSUM` (its second mission is at an
orbital that rolled the hub and is generated now), `SURVIVORS`, and the
ship's `PINNED` for `test_planet`, `droids_planet` and `defense` (towns).
Every one came back to its old number under the old layouts
(`the_pins_come_back_under_the_old_layouts`,
`the_commands_come_back_under_the_old_layouts`, both run alone with
`--exact`). `PICTURES` and the design-hash pins did not move.

## No storage: the ship's holdings and each Bim's loadout (task 113)

Nothing is stored anywhere. What the crew own is abstract, and it is two
things: **the ship's holdings** — the pool (`World::money`), the
**armory** (every weapon and piece of armour nobody wears, one
`world::Stored` each with an id that only climbs) and the **research
keys** (a count, added the moment one is picked up, `World::pick_up_key`)
— and **each Bim's loadout**: one weapon slot and one slot a part of the
body, the room's `bims::combat::Gear`, and nothing else. Bandages,
medkits and the class kits are **charges** on the body
(`Gear::charges`, a count by resource code), never things. The world's
half is `crates/world/CLAUDE.md` ("Nothing is stored"), the room's
`crates/game/CLAUDE.md` ("A loadout and its charges"), the player's
`README.md` ("The Armory"). What to hold on to:

- **Gone**: the hold's gear counts (a design's gear cargo goes into the
  armory at `World::start` and its count to nought, `stock_the_armory`),
  `World::{pieces, guns, grids, bench, craft_targets, auto_upgrade}`,
  `crate::grid`, `Where`, `FetchKind`, the pack (`PACK_CELLS`, `LootCell`,
  `Item::Key`), looting, a knocked-out Bim dropping its gun and fetching
  it (`Kind::Fetch`, `HIT_DROPPED`), the workbench's slots, upgrades and
  repairs, the drug lab's medkit, the room's bench carry (`Kind::Ferry`)
  and craft chain (`Kind::Craft`, `Job::{Haul, Craft}` — the work list is
  `Build` and `Medical`, codes 0 and 1), and the engineer's *Armourer*
  (level six is *Higher quality armour* alone, given outright —
  `class::fixed_at`; `Talent::Armourer` keeps its code, never offered).
  The furniture stays as pictures and solids.
- **A loadout changes between missions, or in one aboard the ship** —
  `Phase::Map` and `Phase::Reward` (there is no trade phase yet) — through
  `Command::{Equip, Unequip, Offer, AnswerOffer}`: in a mission an equip
  or unequip goes through only while the Bim changed (and one a thing is
  taken off) is alive and **inside the ship** (`World::inside_ship`), so
  a crew arriving at an attack or a defence kits out from the armory
  before stepping off; out on the deck it is refused `GearLocked`, and
  an offer is refused in any mission (`World::may_change_now` is what the
  panel greys by). **Every run on the combat ship** — every command but
  `game`, `simulation` and `design` — opens with every weapon and piece
  at every tier it is made at in the armory, for playtesting
  (`World::stock_every_thing_for_probe`, `screens::game::open`). A player changes its own Bim and any bot
  (`World::may_change`), never another player's (`NotYours`): a thing
  passes to another player only as an **offer** it accepts, withdrawn
  when either side's slot changes or a mission starts. A thing off a slot
  goes into the armory; onto a filled slot, the old one does.
- **Armour is never destroyed**: at nought it stays worn and protects
  nothing for the rest of the mission; **every piece is whole at a
  mission's start** (`mend_all_armour`), and so are the charges — **set**
  to their start amounts, not topped up (`fill_charges`).
- **Death**: a player's Bim that dies is out for the rest of the mission
  and **respawns when it ends** with everything it wore, its relics,
  class, level and talents, the pool paying `BUYBACK_COST` or what it
  holds, down to nought (`respawn_the_fallen`, `WorldEvent::Respawned`);
  there is no buyback queue. A bot dead or left behind is gone, the
  penalty paid, **its loadout into the armory** (`store_loadout`).
- **Trading** puts gear into the armory and sells it out of it
  (`World::held`, lowest tier first); nothing is ever short of room.
- **The Armory panel** is the app's (`CrewPanels::armory_window`), Tab
  and the tray's Armory button on every screen of a run; in a mission
  only a column inside the ship takes a drag. `BIMS_ARMORY=1` opens with
  it up.

**What moved**: `SAVE_VERSION` **42**, `wire::PROTOCOL` **34** (the relay
wants redeploying), `REFERENCE_CHECKSUM` (the holdings and every loadout
are hashed where the pieces, the guns, the grids and the bench were), and
`SURVIVORS` and the ship's `PINNED` (their notes say why: the reading
takes a `Gear` by its `Debug`, which lost the pack, and the run itself
plays differently on purpose — the armour mended and the charges set at
every mission's start, the dead back at its end, no gun dropped).
`tests_holdings.rs` in `crates/world` is the task's own tests.

## The trader (task 114)

Gear is bought at a **trader**, and a trader is visited **entirely on the
world map**: no room is loaded and no mission runs. The world's half is
`crates/world/CLAUDE.md` ("The trader"), the player's `README.md` ("The
trader"). What to hold on to:

- **Desk trading is gone**: `Command::{Buy, Sell, ToDesk}`,
  `World::{at_the_desk, desk_spot, walk_to_desk, man_the_desk_for_probe,
  buyable}`, `Refusal::NotAtTheDesk` (21), `WorldEvent::Traded` (8, 9),
  the app's trade window and cart in a run (`Order::{Deal, ToDesk}`, the
  tray's Trade button, the desk's menu row, `BIMS_TRADE`), and the
  generator's per-station gear-trade roll (`WEAPON_TRADE_CHANCE`,
  `ARMOUR_TRADE_CHANCE`, `Stock::{weapon_trade, armour_trade}` — no shelf
  holds gear). The yard's Station panel in the design phase is the
  editor's and is untouched. The desk stays as furniture.
- **Which sites are traders is stateless** (`world::trader`, the
  Manufacturers' pattern): **a system** has one, rolled at
  `data::TRADER_SYSTEM_CHANCE` (10) off the galaxy's seed, plus at least
  `TRADER_NEAR_SITES` within `TRADER_NEAR_HOPS` of home
  (`World::trader_near`, derived), and its trader is the lowest station
  with a desk that is not home, not the Manufacturers' and not derived
  (`trader::pick`); the machines' jammer then stands on the lowest
  *other* station, or a derived one (`World::jammer_site_among`, the
  one rule). It was twenty in a hundred a station until the galaxy of
  `GENERATOR_VERSION` 8 below
  (`tests_trader::a_trader_in_one_system_in_ten_and_the_chart_marks_them`).
- **`run::Phase::Trade`** (code 3): `travel` to a trader calls
  `arrive_at_trader` instead of `arrive_at` + `begin_mission`. The step
  does nothing in it but the commands, as on the map; `Propose`/`Accept`
  work there. The trader met is kept on `Run::traders` (its shelf,
  `None` where bought, and its relic) from the first arrival; the relic's
  vote is `Run::trade_relic`, through the same `ProposeRelic`/`AcceptRelic`.
- **Closed** is `World::trader_closed_on(star, day)`: infested by then and
  not `World::liberated(star)` (every non-Manufacturer infestation of the
  system cleared, off the world or the system's memory). The quote
  refuses `TraderClosed` (102) or `ClosedOnArrival` (103); the crisis
  passes a trader by (`spread_crisis`).
- **New commands** `BuyShelf { index, to }` and `Combine { a, b }`; new
  refusals 102–107, events 126–128; constants `TRADER_WEAPONS`,
  `TRADER_ARMOUR`, `RELIC_PRICE`, `COMBINE_FEE` in `world::data`.

**What moved.** `SAVE_VERSION` **43**, `wire::PROTOCOL` **35** (the relay
wants redeploying), and the four `worldgen::fixture::REFERENCE_CHECKSUMS`
(the shelves lost their gear bits; not a `GENERATOR_VERSION` bump, since
no layout and no other draw moved), and the ship's `PINNED` for
`jammer` alone: `infest_here_for_probe` passes a trader by as the crisis
does (its note; the old number returns with the probe infesting it).
`REFERENCE_CHECKSUM`, `SURVIVORS` and `PICTURES` did **not** move: the
traders are hashed only where there are any, and no pinned run meets one.

## Six hundred stars, every one somewhere to go (worldgen 8)

A laned star with no site on it was a dead end — two in five were — and
nothing on the galaxy chart led to the world map's list. So:

- **`worldgen::GENERATOR_VERSION` 8**: `STAR_COUNT` 600 (was 1 000), and
  every system has a planet to land on (`system::ensure_landable`, a
  body's kind turned to a rocky planet the way `ensure_parent_for` turns
  one, nothing moved) and a station (an orbital, else a derelict, else a
  relay, put there after the pruning where the rolls left none).
  `STATION_SHARE` still decides whether the extras are rolled.
- **A trader in one system in ten** (the trader bullet above).
- **The galaxy chart** (`screens/game.rs`, `lobby::preview`): a click on a
  star picks its first place on the list — a trader first — and scrolls
  the list to it (`WorldMap::pick_star`), so the card's *Propose* is the
  trip; the corner panel says the star's tier, whether it has a trader
  and how many lanes off it is (`chart_star_lines`). A star past tier one
  is ringed amber (2) or red (3) (`Marks::tiers`, off
  `World::system_tiers` — the least and the most tier the star's sites
  come at, the site rule without generating the system), a trader's
  system has a green square (`Marks::traders`, `World::trader_stars`,
  worked out once when the chart is made), and zoomed in past
  `CHART_TIERS_ZOOM` — and always round the ship's star — every star has
  `T1`/`T2`/`T3`/`T1–2` under it.

**What moved.** `SAVE_VERSION` **47**, `wire::PROTOCOL` **39** (the relay
wants redeploying), `worldgen`'s `REFERENCE_CHECKSUMS`,
`REFERENCE_CHECKSUM`, `SURVIVORS`, the ship's `PINNED` (all eleven) and
`PICTURES`' two decks, and the three old-layout checks
(`*_BEFORE_112`) — all for another galaxy on every seed; the designer's
pictures and the design hashes did not move. The measurements in *How
fast the crisis crosses a galaxy*, *How many systems the machines have to
build a jammer in* and the trip table in *The loop* were taken on the
thousand-star galaxy and have not been taken again.

## The minigun and the rail lance (task 115)

Two carried weapons that exist only from a tier up, and only ever the
crew's — bought at a trader or combined. The room's half is
`crates/game/CLAUDE.md` ("The minigun and the rail lance"). What to hold
on to:

- **`WeaponKind::min_tier`** — tier one for the five old kinds, two for
  `Minigun` (9), three for `RailLance` (10) — and **a weapon below it
  never exists**: `WeaponKind::at` debug-asserts it, `basic()` is the
  kind at it, and every path that makes one asks `made_at` first — the
  trader's shelf draws over the valid pairs alone
  (`trader::shelf_candidates`, one draw a thing), a combine of two
  tier-two miniguns makes a tier-three one and a lance combines into
  nothing (`TopTier`), `outfit_for_probe` keeps a kind at its own lowest
  when asked for less, and `BIMS_WEAPON`/`BIMS_ENEMY_WEAPON` take
  `minigun` and `lance` with an optional tier (`minigun1`, `lance2` are
  no weapon). No issue, hire, garrison or machine table names them.
- **Both are `ResourceId`s** (`Minigun` 18, `RailLance` 19) because the
  book prices are keyed by one (`economy::trade_price`: 5 000 and 6 000,
  times `TIER_PRICE` like every gun), so `CARGO_SLOTS` is **20**.
- **The combat crew deals `WeaponKind::ALL`**, seven now, so four of
  `droids`' sixteen carry one of the two. `BIMS_WEAPON_ALL=1` beside
  `BIMS_WEAPON` arms the whole crew with it, which is how the two were
  measured.
- **The minigun's report** is the rifle's clip at 0.2, heard every second
  bolt (`sound::Kind::Minigun`, a 0.15 s cool-down): every bolt played
  stacked to +4.4 dB over one report, every second one to +1.8 (the
  rifle's own burst is +0.9). The picture and the hits are every bolt's.

**What a fight of them cost** (release, this machine, `droids` with all
sixteen on the one gun, `BIMS_PERF=1 BIMS_SMOKE_FREE=1
BIMS_SMOKE_FRAMES=2400`, the crew walked in with the guardian recipe's
keys, three runs each): by the clock 9.79–9.92 ms a frame on miniguns and
9.67–10.07 on tier-two auto rifles; the screen's own frame 8.2 against
8.3; the GPU's bloom 0.247 ms and main pass 0.140 ms on both. Ten dashes
a second a gun cost nothing that shows.

**What moved.** `SAVE_VERSION` **44**, `wire::PROTOCOL` **36** (the relay
wants redeploying); `shipdesign`'s `REFERENCE_HASH` and `PLAYTEST_HASH`
(two more empty cargo slots), `worldgen`'s `REFERENCE_CHECKSUMS` (every
station's price lean two entries longer, drawn last on its own branch —
not a `GENERATOR_VERSION` bump), `REFERENCE_CHECKSUM` (the design hash in
it), and the ship's `PINNED` for `droids`, `tier2_test` and
`combat_droids_medic` and `PICTURES` for `droids_deck` (the deal; all came
back with the deal cut to the old five). `SURVIVORS` did **not** move: its
own `arm` deals the five kinds the reading was taken with.

## The arc greaves and the Reflective plate (task 116)

Two pieces of armour that exist only from a tier up, and only ever the
crew's — bought at a trader or combined, both passive and the same on a
player's Bim and a bot. The room's half is `crates/game/CLAUDE.md` ("The
arc greaves and the Reflective plate"), the world's `crates/world/CLAUDE.md`.
What to hold on to:

- **`ArmourKind::min_tier`** — tier one for the three basic pieces, two
  for `ArcGreaves` (4, legs), three for `ReflectivePlate` (5, body) — and
  `Piece::new` debug-asserts it; `ArmourKind::BASIC` is the three every
  outfit, start, hire and hostile wears, and nothing else names the two.
  Both are `ResourceId`s (20, 21) for the book prices (1 500 and 2 500,
  times `TIER_PRICE`), so `CARGO_SLOTS` is **22**.
- **The plate sends a hostile bolt back** 40 % of the times it lands on
  the body while whole (`REFLECT_ODDS`), a friendly bolt of the same gun
  at half damage, the wearer's, back along the line — rolled only for a
  body wearing one, so no fight without a plate draws differently. It has
  no tier-three dodge. Beams and blows are never sent back.
- **The greaves discharge on a blow**: an enemy's melee blow landing on
  whole greaves throws `ARC_DAMAGE` (10) times the tier's armour factor
  into every live enemy within two tiles, once a second a wearer; never
  off a bolt, and it costs them nothing.
- **`BIMS_ARMOURED=mirror`** is a tier-three plate with the basic helm and
  leg guards, **`BIMS_ARMOURED=arc`** tier-two greaves with the basic helm
  and kevlar (`dev::armour_set`); `=1` is what it was. `BIMS_FREEZE=shield:1+2
  BIMS_ARMOURED=mirror` with the guardian recipe's keys on `droids` holds
  the first bolt sent back in the air, since a reflection is heard as
  `Cue::Shielded`.

**What moved.** `SAVE_VERSION` **45**, `wire::PROTOCOL` **37** (the relay
wants redeploying); `shipdesign`'s `REFERENCE_HASH` and `PLAYTEST_HASH`
and `worldgen`'s `REFERENCE_CHECKSUMS` (two more resources, as in task
115), and `REFERENCE_CHECKSUM` (the design hash in it). `SURVIVORS`, the
ship's `PINNED` and `PICTURES` did **not** move: nobody in a pinned run
wears either piece.

## Every site is an attack, a defence or a trader (task 111)

Every site the map lists is exactly one of three, from the **first day**
of a run: **ATTACK** (an enemy holds it — the machines, the Manufacturers
or the Machine Heart), **TRADER** (task 114's sites) or **DEFEND** —
everything else, stations, derelicts and towns alike, the spawn a run
opens at among them. The world's half is `crates/world/CLAUDE.md`
("Every site is an attack, a defence or a trader"), the player's
`README.md`. What to hold on to:

- **`World::site_kind` and `site_threatened` are the rules**
  (`town_threatened` went): a defence starts the first joined step at any
  threatened site, the crew are stood ashore just inside its airlock
  (`Game::stand_at`), and the first wave lands `DEFENSE_DELAY_STEPS`
  (**1 200**, twenty seconds at 1×) later at the far airlock or a town's
  gate. Armed **defenders** (`Residents::defender`, `defense::defenders`
  of the day, `data::DEFENDERS_*`) fight beside the site's people, count
  as players towards the wave, and are nobody's loss. Won: the bounty
  paid (a win now waits for every wreck to be counted), a town held for
  good as before, a station or derelict cleared and no more; left early,
  it falls.
- **The map says it three ways**: the list's rows lead with the kind's
  word in its colour (`worldmap::row_job`), the card with it large and a
  `?`, and the system map rings every site in its kind's colour
  (`world_paint::site_ring`: red, amber, green; faded or blue when its
  fight is over) and writes the word under every icon
  (`Session::site_marks`, which replaced `landing_sites` and
  `trader_marks`). The HUD's red chip says `Prepare: 0:14` before the
  first wave (`names::defense_prepare`).
- **Mining sites are gone**: no `StationKind::MiningOutpost` is generated
  (the variant keeps its code) and the belt's pickaxe left the map.
- **Tests**: `World::set_quiet_sites_for_probe(true)` makes every
  non-trader, non-held site a peaceful stop; `fixture::simulation_world`
  and `crewed_world` set it, `open_simulation_world`/`open_crewed_world`
  do not — `reference_run_world`, `SURVIVORS`, `PINNED` and `PICTURES`
  play the game. A scripted run walks the players back aboard before
  *Back to ship* can carry (`fixture::walk_the_players_aboard`).

**What moved.** `SAVE_VERSION` **46**, `wire::PROTOCOL` **38** (the relay
wants redeploying), `REFERENCE_CHECKSUM`, `SURVIVORS`, the ship's
`PINNED` (all eleven) and `PICTURES`' `simulation_deck` (the run, not the
drawing: it came back with the sites quiet), the three old-layout checks,
and `worldgen`'s `REFERENCE_CHECKSUMS` (the outposts; no
`GENERATOR_VERSION` bump — a station share).

## Relics drop by the day (task 117)

How a relic enters a run. The world's half is `crates/world/CLAUDE.md`
("Relics drop by the day"), the player's `README.md` ("Relics, and what
a won run unlocks"). What to hold on to:

- **A relic's tier is developer data.** It sets the odds of drawing it
  and its trader price (`data::RELIC_PRICE`: 1 500, 3 000, 5 000) and
  nothing else, and is never shown: `names::relic_tier`, the "Tier n"
  on the sheet, the reward window, the victory screen and the trader's
  price line, and `RelicChoice::tier` are gone.
- **One roll for the reward, the cache and the trader**
  (`relic::offer(pool, day, n, seed)`): a tier by `relic::tier_odds` of
  the world clock's day (`RELIC_ODDS_START` [70, 25, 5] to
  `RELIC_ODDS_END` [40, 35, 25] by `RELIC_ODDS_FULL_DAY` 30, linear), a
  relic of it, and an empty tier rolled again among the tiers left. The
  site's enemy tier is not read.
- **The pool loses a relic only when a Bim gets it** — chosen off a
  reward or a cache (pending counts), or bought. An offer passed over
  stays in; a lost pending relic comes back; a trader's relic is kept
  out of other draws while it is on an open trader's table and back in
  the running once that trader closes.
- **A new profile starts with 23 relics** and a win unlocks two of the
  fourteen others in `Relic::ALL`'s order; an old profile is read with
  every starting relic in it (`Profile::unlocked`).

## Five patches of relics (task 118)

Twenty-five relics, codes 12–36 — *Dismantler*, *Lifeline*, *Flanker*,
*Command Net*, *Supply Line*, two tier ones, two tier twos and a tier
three each — and *Kill Relay* at three seconds. The world's half is
`crates/world/CLAUDE.md` ("Five patches of relics"), the player's
`README.md` (the second table under "Relics"). What to hold on to:

- **Still a row a relic.** `relic::RELICS` grew the hooks the patches
  needed: `Stat::{MachineDamage, DamageTaken, TraderPrices}`,
  `When::{OnLimb, Crippled, Flanked}` asked against a
  `relic::Situation`, `Trigger::{CrewKill, FlankKill, BandagedCrewmate,
  Bandaged, CrewmateDowned}`, `Action::{Sprint, Unseen, Tether, Shelter,
  RallyUp, Heal}`,
  a hook's `cooldown`, `Effect::OnEach` (two hooks), `Effect::Aura` and
  `Effect::Rule` (one relic's own rule). Every number is
  `world::data`'s, every word `names.rs`'s.
- **Where each is read** is `crates/world/src/relic_hooks.rs`, a child
  of `world`: a crew hit landing on a machine (`land_on_machines`,
  before `visit` lands the rest), the skill (`lift_by_relic_hooks`:
  *War Chest*, *Sprint Coil*, the auras, *Tether Field* into the room's
  new `Skill::damage_taken`), the healing (`relics_mend`, and
  `Trigger::Bandaged`), a kill (`machine_kills_noted`, with how the
  machine went), a crewmate down (`settle_relic_downs`, a stage after
  `settle_relics`), the clear and the trader. The room's half is two
  numbers it is handed — `Skill::damage_taken` and a shield's front a
  shooter (`Game::set_shield_fronts`, *Wide Angle Optics*) — and
  `Droid::front()`, the facing every kind has for a flank, and
  `Game::heal` (`Health::heal`).
- **The Lifeline patch heals hit points and never touches the blood**,
  since the health system is going to lose its blood: *Pressure Seal*
  regenerates HP, *Clot Booster* regenerates HP for its seconds after
  going down (both `relics_mend`, a stage right after
  `settle_relic_downs`), and *Quick Wrap* heals HP with every dressing
  (`Action::Heal` on `Trigger::Bandaged`). *Tether Field* still fires on
  a dressing, and so waits on bandages staying in the game.
- **Nothing draws from a stream and nothing runs with no relic held**:
  a crew holding none lands every hit, and draws every roll, exactly as
  before — `SURVIVORS` and the ship's `PINNED` did not move.
- **`Command::Restock`** (*Restock Codes*): at a trader, a player holding
  it, once a visit (`Relics::restocked`, cleared on arrival) — refusals
  `NoRestock` (108) and `Restocked` (109), `WorldEvent::Restocked` (129);
  the shelf rolled again off the world clock's minute
  (`trader::reroll_shelf`), the relic untouched. The trader panel's
  **Restock the shelf** button.
- **`BIMS_RELICS`** takes a name with or without its apostrophe
  (`marksmans_habit`).

**What moved** (with task 117, once for both): `SAVE_VERSION` **48**,
`wire::PROTOCOL` **40** (the relay wants redeploying), and
`REFERENCE_CHECKSUM` (a new profile's pool is fifteen relics longer, and
a choice lost its tier). `SURVIVORS`, the ship's `PINNED` and `PICTURES`
did **not** move. The new relic state is hashed only where there is any.
`tests_relic_patches.rs` is the task's tests.

## One speed, and experience alike for every class (task 119)

The world's half is `crates/world/CLAUDE.md` ("One speed, and experience
alike for every class"), the player's `README.md` ("Speed, and who
decides", "Classes and levels"). What to hold on to:

- **`world::Speed` is `Paused` and `Real`**: 3×, 10×, 24× and the top
  speed went with their keys (`keys::Action::{Speed3, Speed10, Speed24,
  SpeedTop}`) and `data::{DAY_SPEED, TOP_SPEED}`. **Space** toggles
  pause, **1** sets going; a pause by any player pauses everybody.
  Nothing was rebalanced: every in-mission timer is read at 1×, a minute
  of the clock a real second. Travel still puts its minutes on at once.
- **Experience is two things for every class**: an enemy down within
  fifty tiles `XP_ENEMY_DOWN` (10), its death `XP_ENEMY_DEAD` (5) on top.
  `XP_BUILT`, `XP_HEALED`, `XP_HIRE`, `TANK_HITS_PER_XP` and
  `settle_tanks` went.
- The many "at 24×" notes in this file are history: the measurements
  were taken when there was one.

**What moved.** `SAVE_VERSION` **49**, `wire::PROTOCOL` **41** (the
relay wants redeploying), `REFERENCE_CHECKSUM` (the reference run's two
speed requests are 1× where they were 24×; the speed alone moved it) and
`SURVIVORS` (the experience alone moved it — its note says how that was
checked). The ship's `PINNED` and `PICTURES` did **not** move.

## One bar of hit points: downed, revived, no medicine (task 120)

The room's half is `crates/game/CLAUDE.md` ("One bar of hit points"), the
world's `crates/world/CLAUDE.md` (the same), the player's `README.md`.
What to hold on to:

- **Every Bim is one bar**, `health::MAX_HEALTH` (100); the machines keep
  their parts. A hit rolls its part only to choose the armour that takes
  it first; what gets through comes off the bar; nothing mends by
  itself. **Gone**: the blood, bleeding, wounds, the parts' health,
  traumas and lasting injuries, lost legs, treatment, bandages, medkits
  (items, charges, cooldowns, the room's stock), the drug lab part and
  its recipe, running scared, and every pace or work penalty but the
  downed one.
- **Nought is downed**: no acting, no target, a thirty-second countdown
  of steps (frozen while paused), then dead. Any crew Bim revives a
  downed crewmate by standing beside it — ten seconds, a medic four,
  *Trauma Kit* two off, never under one — standing still and holding its
  fire; one reviver counts; up at three tenths and 30 % slower for the
  rest of the mission. Bots revive of their own accord (out of harm, or
  the room calm); a player's own Bim on an order. A Manufacturer and a
  station's or a town's people are never revived.
- **The medic's beam heals hit points** (`class::HEAL_BEAM_HP`, marked
  for tuning) and may be turned on the medic itself.
- **Blood stays as a picture**: a splash where a hit took hit points, a
  trail under twenty, a pool under the dead; a machine never bleeds.
- **No-op talents** keep their slots and say "No effect for now": listed
  in `crates/world/CLAUDE.md`.

**What moved.** `SAVE_VERSION` **50**, `wire::PROTOCOL` **42** (the
relay wants redeploying), `REFERENCE_CHECKSUM`, `SURVIVORS`, the ship's
`PINNED` (all eleven) and `PICTURES`' two decks — each note says why —
`shipdesign`'s `REFERENCE_HASH`, `PLAYTEST_HASH` and `PLAYTEST_PARTS`
(666: the drug lab off the part list, codes after it closed up, `ALL`
48) and `worldgen`'s `REFERENCE_CHECKSUMS` (no shelf stocks a medkit or
a bandage). `ship::paint::PART_COLORS` lost a stale smelter row it had
kept since feature 95, so every part after the old smelter is its own
colour again.

## The soldier's ranked kit (task 124)

The soldier has no talents: four abilities, **four ranks each, bought
with a skill point a level**, Dota's way. The world's half is
`crates/world/CLAUDE.md` ("The soldier's ranked kit"), the player's
`README.md`. What to hold on to:

- **Levels are the class's own table.** `class::level_xp(class)` —
  `RANKED_LEVEL_XP` (sixteen, its top 3 200 like the others' tenth) for
  a class with a ranked kit (`class::ranked`: the soldier; the engineer
  is task 127), `LEVEL_XP` (ten) for the rest — and
  `Progress::{level, to_next, gain}` take the class; `World::level_of(who)`
  is the one to ask. A point a level, the first included; unspent points
  carry over.
- **`Command::RankUp { slot, ability_slot }`** (0–3 for Q C E R, heard
  between missions too) → `WorldEvent::RankedUp { who, class,
  ability_slot, rank }` (130); refused `NoRankedKit` (110),
  `NoSkillPoint` (111), `TopRank` (112) or `RankLocked` (113). Q, C and E
  rank `n` at level `2n − 1`, R at 6, 9, 12, 15 (`class::rank_level`).
  The ranks are `Progress::ranks`, saved, hashed only where any is
  bought, kept through a death like the picks. The app's
  `screens::game::rank_up` (Ctrl and a slot's key, a Ctrl-click on its
  box) sends it; so does the Skills tab's button.
- **Q Frag Grenade** reads its rank in `World::{charges, charge_cooldown,
  grenade_damage, grenade_radius}`; **C Weak Spot** is `Skill::crit_chance`,
  rolled where a hit lands off **the world's own crit stream**
  (`World::crit_rng`, lent to the crew's room for its step), the bonus
  `flat × (crit − 1)` added by the world after every relic factor and
  before the armour (`World::crit_extra`); **E Brace** is
  `Skill::{miss_cut, damage_taken, deadeye}`; **R Rampage** is
  `Command::Rampage` (`Refusal::NotLearnt` 114, `AlreadyActive` 115) and
  `World::soldiers` (`crate::soldier`), read into the skill while it runs.
- **Nothing moves for a crew without a soldier's ranks**: a skill with no
  miss cut is the weapon's odds to the bit, no chance draws nothing off
  the crit stream, and the new state is hashed only where there is any,
  so `REFERENCE_CHECKSUM` did not move for this task. `SURVIVORS` and the
  ship's `PINNED` did, on purpose: their soldier starts with no grenade
  at rank nought where it had two. They were re-pinned once with task
  127's engineer (whose charges moved them too), the note saying both.
  `PICTURES` did not move.
- **The app**: the hero panel's four boxes carry rank pips and a "+"
  while a point could buy the next rank, the unspent points sit by the
  experience bar, a critical hit's impact flare is twice the size and
  brightness (`fx::Fx::critical`), and the character sheet's Skills tab
  shows a ranked kit's four abilities rank by rank
  (`CrewPanels::ranked_skills`). `combat_droids_soldier` opens at the
  sixteenth level with sixteen points; `BIMS_RANKS=q,c,e,r` sets the
  ranks.

**What moved.** `SAVE_VERSION` **51**, `wire::PROTOCOL` **44** (the
relay wants redeploying). The soldier's talents (codes 14–27) are
deleted, their codes left free; `names::TALENT_NAMES` is indexed by code
with those places empty.

## The engineer's ranked kit, and charges without kits (task 127)

The engineer is the second ranked kit, on task 124's rank system: **Q
EMP, C Healing Sentry, E Sandbags, R Sentry** (the ultimate), four ranks
each. The world's half is `crates/world/CLAUDE.md` ("The engineer's ranked
kit, and charges without kits"), the room's `crates/game/CLAUDE.md`, the
player's `README.md` ("The engineer"). What to hold on to:

- **A charge is a counter the world keeps** (`World::charges_held`), never
  a thing in a pack — the soldier's grenade included. `ResourceId::
  {SandbagKit, SentryKit, Grenade}` are gone from every crate, their codes
  15–17 left free: **`ResourceId::ALL` is 19 long and `CODES` 22**, so a
  code is looked up with `ResourceId::from_code` and never used to index
  `ALL`; `CARGO_SLOTS` and the market's leans are `CODES` long.
  `Charge::Sentry` (1) is gone; `HealingSentry` (3) and `Emp` (4) are new.
- **No talents, no kits, no *reused* charges**: the engineer's talents
  (codes 0–13) are deleted and their names blank in `TALENT_NAMES`;
  laying is nobody's experience (task 119's rule, kept at the user's
  word over the spec's `XP_BUILT`), so `reused_kits` went.
- **New commands** `Command::Sentry { slot, tile }` and `Command::Emp {
  slot, x, y }`; `Command::Deploy` carries a `DeployKind` (sandbags or a
  Healing Sentry). New events `EmpThrown` (132) and `SentryDone` (133).
- **The machines can be stunned** (`Droid::stun`): an EMP's burst in the
  crew's room notes the targets in its radius (`Game::take_stuns`) and
  the world stuns the machines among them before their room steps
  (`World::settle_stuns`).
- **Looking at it**: `BIMS_RANKS=4,4,4,4 bims combat_droids_engineer`, then
  `Q`/`C`/`E`/`R` in `BIMS_KEYS` over a deck tile under `BIMS_POINTER`;
  `BIMS_KITS=n` sets the three charges.

**What moved.** `SAVE_VERSION` **52**, `wire::PROTOCOL` **45** (the relay
wants redeploying), `REFERENCE_CHECKSUM` (the counters hashed where the
re-used kits were), `worldgen`'s `REFERENCE_CHECKSUMS` (three price leans
fewer, no bump), and `SURVIVORS` and the ship's `PINNED` for `game` —
with task 124's soldier, each note saying both. `shipdesign`'s hashes and
`PICTURES` did not move.

## The commander's ranked kit (task 129)

The commander is the third ranked kit, on task 124's rank system: **Q
Battle Cry, C Command Aura, E Rally, R Reinforcements** (a passive
ultimate), four ranks each. The world's half is `crates/world/CLAUDE.md`
("The commander's ranked kit"). What to hold on to:

- **Two base traits outside the slots**: squad orders (attack, fall back,
  stand ground) at `SQUAD_RANGE` from the first level, an attack marking
  **one** enemy (`SquadKind::Attack { enemy }`), and hires at
  `HIRE_DISCOUNT_PERCENT` off. His talents (codes 55–68) are gone, their
  codes free; `Refusal::NoRallyYet` (71) went — a shout at rank nought is
  `NotLearnt`.
- **The squad's attack moved off E onto B** (`keys::Action::SquadAttack`),
  since E is the Rally now; fall back stays on T and stand ground on Z,
  and the three have boxes after the four slots.
- **Battle Cry and Rally fix their reach at the call**
  (`Commander::{cried, rallied}`): a Bim that walks out keeps it, one that
  walks in gets nothing. The aura is damage alone and takes in the
  commander himself.
- **A reinforcement is a crew member marked with its commander**
  (`World::reinforcements`), laid at `begin_mission` on free deck within
  five tiles of him (`Game::free_tiles_near`, `enlist_reinforcement`). One
  that dies is **hidden, not removed**, until the mission's end
  (`Game::vanish`, `Bim::gone`): removing a crew index mid-fight would
  shift every index the room and the world keep, and drop everybody's
  errand with `take_crew`. At the mission's end all go
  (`send_reinforcements_home`), before anything is paid or buried.
- **Looking at it**: `BIMS_RANKS=4,4,4,4 bims combat_droids_commander`
  (the reinforcements are brought there and then by
  `World::reinforce_for_probe`).

**What moved.** `SAVE_VERSION` **53**, `wire::PROTOCOL` **46** (the relay
wants redeploying). `REFERENCE_CHECKSUM`, `SURVIVORS` and `PINNED` are
meant not to move: the new state is hashed only where there is any, and
the attack's one mark is hashed as the list of one it was.

## The medic's ranked kit (task 130)

The medic is the fourth ranked kit, on task 124's rank system: **Q
Nanite Burst, C Healing Aura** (passive), **E Heal Beam, R Cloak** (the
ultimate), four ranks each; the tank is the one class of talents left.
The world's half is `crates/world/CLAUDE.md` ("The medic's ranked kit"),
the room's `crates/game/CLAUDE.md` ("The medic's cloak"), the player's
`README.md` ("The medic"). What to hold on to:

- **Base traits**: a revive in `MEDIC_REVIVE_SECONDS` (4) as before, and
  a crewmate a medic of the class revives gets up at
  `class::MEDIC_REVIVED_TO` (0.4) where anybody else's is at
  `health::REVIVED_TO` — the room reads it off the helper's
  `Skill::revived_to`. A hired field medic revives in four seconds and
  gets its patient up at 0.3.
- **The surge is gone**, and the medic's talents (codes 28–41) with it,
  their codes free; `Refusal::{NoSurgeYet, NotCharged, NotLinked}`
  (65–67) and `WorldEvent::Surged` (78) went. **The room's surge timer
  stays**: *Phase Harness* and *Lifeline* still set it
  (`Game::set_surge`), and the checksum still hashes it, a nought where
  the medic's charge was.
- **`World::heal_factor(who)`** is the Healing Aura, read where the healed
  Bim stands, and every heal of the world's goes through
  `World::heal_crew` — the beam, the burst, a Healing Sentry, *Pressure
  Seal*, *Clot Booster*, *Quick Wrap*. A revive never asks.
- **`HEAL_BEAM_HP` is 120** an hour (two a second at 1×); the ranks
  multiply it for the beam and the engineer's Healing Sentry reads it
  unranked — so every Healing Sentry heals four times what it did.
- **A cloak is the world's** (`World::cloaks`, `crate::medic::Cloak`):
  a cloaked crew member is `None` on every enemy's list — the Signal
  Scrambler's path (`World::hidden_from_enemies`) — so no enemy picks it
  and one aiming drops it that step; `Skill::holds_fire` and a `walk`
  factor while it lasts; every class key refused `Refusal::Cloaked`
  (118). An enemy with nobody left it may pick holds where it stands
  (`Game::set_targets_withheld`, told only while a target was withheld,
  so no other fight moves). Its speed is `Skill::walk`, the always-on
  pace, rather than `pace`, which applies only with an enemy in sight.
- **Looking at it**: `BIMS_RANKS=4,4,4,4 bims combat_droids_medic`, then
  `Q`, `E` over a crewmate and `R` over one (or over nobody for himself)
  in `BIMS_KEYS`; `BIMS_BEAM=1` links the beam, buying its first rank if
  none is bought.

**What moved.** `SAVE_VERSION` **54**, `wire::PROTOCOL` **47** (the
relay wants redeploying). `REFERENCE_CHECKSUM` and `SURVIVORS` are meant
not to move (a nought hashed where the charge was; the new state hashed
only where any); the ship's `PINNED` for `combat_droids_medic` will move,
the medic there being a ranked kit now, and so may any pin whose run
meets a Healing Sentry, since its heal is four times what it was.

## The Manufacturers attack a defence before day ten (task 131)

Before `MANUFACTURER_DROIDS_LOST_DAY` (ten) every wave that lands on a site
the crew defend is **the Manufacturers'**: their people, armed by the day,
and the day's share of Troopers (`manufacturer::trooper_percent`: none on
day nought, a tenth from day five, a quarter on seven, half on eight, three
in five on nine) — the table their own garrisons already used. From day ten
the machines, as before. Their people stand in the site's **friendly** room
beside its own, so a Manufacturer there is an **intruder**
(`bims::game::Game::is_intruder`, `crates/game/src/intruder.rs`) and
fights the machines' way, off their list. The world's half is
`crates/world/CLAUDE.md`, the room's `crates/game/CLAUDE.md` ("An
intruder"), the player's `README.md` ("The Manufacturers"). The HUD's line
says `MANUFACTURERS — wave 1 of 2, 3 up` for them.
`World::set_defense_by_machines_for_probe` keeps a defence's waves the
machines' whatever the day, for the tests of the machines' fight (which
run at day nought). **So `defense`, `test`, `test_planet`, `simulation`
and `game` now meet the Manufacturers at their first defence**: every one
opens before day ten. No save or protocol bump (a field with a default);
`REFERENCE_CHECKSUM`, `SURVIVORS` and the ship's `PINNED` move with the
run, on purpose, and are the test agent's to re-pin.

## Stations with wings, and a way in anywhere (task 131, the stations)

The player's second task numbered 131. A generated station (feature 112)
is no longer only a ladder of corridors: the ladder is the **core**, most
often a compact one, and **wings** grow off the ends it leaves against
the skin — a boom, sometimes a dog-leg, rooms along it and a module
across its end (`stationgen::grow_wings`) — so the silhouette is a core
with things sticking out of it. **The airlocks after the port may open
into a room** as well as a corridor (chosen after a trial furnishing, so
nothing a room's furnisher stands is in the way), three or four on an
orbital, and **the machines come aboard by every airlock but the port in
turn**, the farthest first (`droid::arrival_airlock_at`, for every
station now and not only the Machine Heart) — so a wave can land in a
store, in somebody's quarters or at the far end of a boom. The world's
half is `crates/world/CLAUDE.md` ("Wings", "Airlocks, the array, the
cover", "Where a wave arrives"), the player's `README.md`.
`BIMS_STATION_SEED=<n> bims droids` fights in one, and `BIMS_STATION_KIND=Orbital
BIMS_STATION_SEED=<n> cargo test --release -p world --lib print_candidate
-- --ignored --nocapture` prints one.

**What moved.** `wire::PROTOCOL` **48** (the relay wants redeploying:
two ends on different generators build different stations). No
`SAVE_VERSION` bump — a station's design is saved as it stands. Every
generated station is another building, and the arena's waves (`droids`
and everything on it) now come in by its east, north and south lobbies in
turn, so `REFERENCE_CHECKSUM`, `SURVIVORS` and the ship's `PINNED` (and
`PICTURES`' `droids_deck`, where a wave lands elsewhere) are expected to
move, on purpose, and are the test agent's to re-pin. The drawn plans,
the spawn's hub, the arena's and the fortress's designs, the towns and
`worldgen` did not move.

## The old game deleted (feature 104)

The third step of the redesign: **everything features 102 and 103 switched
off is deleted**, and nothing a run does moved. The last commit that has
the old game is tagged **`needs-sim-final`** (29d5d54, feature 103's) —
`git show needs-sim-final:<path>` is how a deleted file is read again.

**What went:**

- **The needs**, and everything only reachable with them on: hunger,
  sleep, the heads, the shower, company, the mess and the surroundings;
  food, cooking, the pot, the dishwasher, spoilage and the cold store's
  clock; the bay's growing and the manager's stock targets; poisoning,
  malnutrition and the diary's every entry but a crewmate's death. In
  the room that was `needs.rs`, `social.rs`, `schedule.rs`, `galley.rs`,
  `dish.rs`, `bath.rs`, `hydro.rs`, `manager.rs`, most of `filth.rs` and
  every errand in `task.rs` that served them; in the world the residents'
  larder, `World::spoil`, `WorldEvent::FoodSpoiled`, the bay's power, the
  guard's post (which only the needs sent a guard to) and the hire's bunk
  check (`Refusal::NoBunk`); in the app the need bars, the Schedule tab,
  the stock rows and every menu on a fixture that only served a need. The
  switch, `needs_enabled`, went on both sides.
- **The `room` command and the behaviour test room** —
  `crates/app/src/screens/room.rs`, `Launch::Room`, `Screen::Room`, the
  flake's `room` app and `./check`'s `room` window. The classic room
  (`Game::new`) is no longer a game mode; the tests that used it as an
  arena have a bare room to stand in.
- **Radiation**: the whole `crates/health` crate — it was the dose, the
  sickness and the cancer, and nothing else ever changed a body's
  `HealthState` — with `World::health`, stage eight (`run_health`),
  `WorldEvent::Health`, `radiation_enabled` and the app's dose readout.
  A walk outside still wants a suit (`suit_ok`).
- **Human enemies**: `raid.rs`, `plunder.rs`, `human_foes_enabled`,
  `rolled_hostile`, the world's `hostile` list and `set_hostile`, the
  garrison formula (`station::enemies_of` and what it was built of),
  `Command::Execute` and `Command::Plunder`, the loot of a station's dead,
  `Plan::Raider`, `stage_fight_for_probe` and `Session::make_dock_hostile`;
  `BIMS_FIGHT`, `BIMS_RAID`, the raid warning, the plunder window and the
  Kill row. The tests that staged a human fight to test a class or the
  combat were ported to machines; the tests of human-only mechanisms went.
- **The flown trip**: `ShipState` is `Docked` and `Holding` and nothing
  else — the trip, the push-off, the docking run, a jump's charging, the
  landing and its descent went, with `Command::{Confirm, Abort, Jump,
  Land}`, the helm job (`Job::Helm`, `Command::ToHelm`), the game's
  exhaust, `landing_for_probe` and **the free clock**. `BIMS_LANDED`,
  `BIMS_LANDING` and `./check`'s `landed` and `raided` windows went with
  them. `DROID_FIRST_DAY`, unread since feature 102, went too.

**What stayed, and why:**

- **`shipdesign`, the designer and the `design` command are untouched** —
  not a file under `crates/shipdesign` changed, so its pins did not move.
  Its radiation warning is the designer's own check and stays, and so does
  the validator's list of galley, heads and bunks (*What it checks* in
  `README.md`).
- **The parts that do nothing stay, as pictures.** The bunks, the galley,
  the heads, the shower, the hydroponic bay, the cold store, the table and
  chairs, the dishwasher and the broom locker are still in the part list,
  and the room still draws them — in the state a fresh room draws them,
  which is what the designer shows through `ship::paint::fixtures` →
  `Room::draw_fixtures` and what a run showed anyway, since nothing changed
  them with the needs off. Their drawing moved out of the deleted modules
  into a module of its own, `crates/game/src/fixtures.rs` (`Worktop`,
  `Hob`, `Fridge`, `Dishwasher`, `Berth`, `Locker`, `Bay`, `Heads`,
  `Stills`), with each fixture's state cut down to what its picture
  needs.
- **Their frames stay solids, in the same order.** `Room::solids()` feeds
  the nav grids, the push-out and the wander, so a solid dropped or moved
  is a route that moved: every fixture frame is still a solid, stand-ins
  included. The classic bath compartment's three wall rectangles were
  the one exception: zero-sized and off the map aboard, they were dropped
  with the classic room, and the survivor tests stayed green, so nothing
  moved.
- **Blood on the deck stays**, as a blood-only grid — `blood::Blood` in
  `crates/game/src/blood.rs`, what `filth.rs` was, cut down to blood. Its
  rolls are on the room's one stream, which every fight draws from: a
  drop's two `signed()` in `Bim::tick_drips`, the boots' `chance(0.25)`
  when a step crosses a tile edge off a tile with blood enough on it
  (`Blood::track`), and the splash of a cut or a burst (`Blood::splash`,
  off `blast` and `strike_stripping`). Taking any of those draws away,
  reordering them, or changing the state that decides them would re-roll
  every fight after the first drop.
- **The cold store stays a container** (`Container::Fridge`, on the
  Nearby strip): the drug lab's medkit is made of vegetables, and the hold
  keeps vegetables there.
- **`born_year` and `born_day` stay**, though only the About tab reads
  them: they are two draws per Bim at its making, and dropping them would
  re-roll everything after.
- **The generator's `hostile` roll stays** — `worldgen` is untouched, the
  roll is in the galaxy checksum, `station::key_tier` puts the tier-two
  keys on it, and `spawn` still skips a hostile blueprint — and so does
  `World::stance`: a droid-held station hostile, home friendly and the
  rest neutral.
- **`World::jump`** stays (a trip to the next star swaps the system with
  it, `jump::landing_point` and all), and so do `land_for_probe`,
  `shipyard_enabled`, the `flight` and `physics` crates (a trip is quoted
  off `Ship::dynamics` by `physics::travel_days`) and `hull::Firing`,
  which the designer draws with.

**How it is known that nothing moved.** Two tests pin a reading of **only
the state that outlived the deletion**, taken off `needs-sim-final` before
anything was deleted, and their constants are **never edited** — save by a
change *meant* to alter how a run plays, which says why in the constant's
own note (`SURVIVORS` has moved so for the follow-up to feature 104 that
pays experience for machines downed in a town's defence, the Guardian in
the town run's tier-three wave (feature 100), the least trip (feature 105)
and the experience of feature 109):

- `crates/world/src/tests_survivors.rs` — `SURVIVORS`: a seeded run, two
  players and four bots on the combat ship with a gun in every hand: the
  trip to a station of the spawn system handed to the machines and the
  fight there; back to the ship, a jump to a star next door and a mission
  there; and a second world whose own system is on the front, the trip to
  its town and the town's defence. About forty seconds: `cargo test -p
  world --lib tests_survivors`.
- `crates/ship/src/tests_survivors.rs` — `PINNED`: every command's
  `Session` built the way `screens::game::open` builds it — `simulation`,
  `game`, `droids`, `tier2_test`, `combat_droids_medic`, `test`,
  `test_planet`, `droids_planet`, `defense`, `crisis` and `jammer`, the
  random ones at a written-down seed and roll and the dials at the
  commands' own values — stepped a while and read. Beside it, `PICTURES`
  pins the **pictures** bit for bit, since the fixtures that do nothing
  stayed as visuals: the designer's fixtures on the playtest ship and the
  combat ship (`paint::fixtures`), and a run's deck (`Session::render`) on
  `simulation` and on `droids` after a while of the fight. A picture drawn
  the same another way moves those, so one is re-pinned only by a change
  that says in the test's note why it is the same picture. About two
  minutes for the file: `cargo test -p ship --lib tests_survivors`.

The reading is `world::fixture::Survivors`: the world clock and the
mission clock, the run's phase, missions, deaths and pending bounty, the
pool, the star, where the ship is alongside, every crew member's class,
experience and picks, the fallen, every body on the crew's deck and the
site's — where it is, alive or down, its blood, each part's health,
wounds and trauma, its gear — every machine's place, kind, tier and body,
and every site's state: the machines' hold on it, a town's fight, the
towns held, and this system's places cleared, held and threatened. An
enum goes in **by its name**, so a variant deleted or a discriminant
closed up moves nothing that did not itself move. `world_checksum` could
not do this job — the switches and the needs were in it — so it and
`REFERENCE_CHECKSUM` moved (re-pinned in `world::fixture`, with
`reference_run_world` rewritten without flight), and so did
`SAVE_VERSION` and `wire::PROTOCOL`. **If a survivor number moves, a run
plays differently**: find why — a draw on the room's stream removed, an
order of operations changed, a solid gone from a nav grid, a room told
something it was not told before — and put it back.

The numbers that did move, each once for the whole feature:
**`SAVE_VERSION` 35** and **`wire::PROTOCOL` 27**; `REFERENCE_CHECKSUM`
is `0x_5359_7c7a_29be_b3e2`, re-pinned by the world's half and not moved
again by the room's, whose needs had already left the world's hash. The
`design_hash` pins did not move: not a file under `crates/shipdesign`
changed.

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

## It is a workspace, and the app is one crate of thirteen

Everything is under `crates/`. `app` is the game's **one binary**: Bevy,
egui, the four screens, the pointer, and every word on the screen. `game`
is the room — the Bims' simulation, which `world` runs aboard the ship and
on every station's deck. `ship` is the design phase and the game it
starts: `Session` (the editor, then the game), the cameras and the
painters. `lobby` is the World tab's galaxy, a camera and a pick. Beside
them are seven libraries that have to give the same answer in more than
one place: `worldgen` (the galaxy, systems and station blueprints, which a
native server will one day generate identically), `physics` (ship mass,
thrust and travel time), `shipdesign` (what a ship is made of and the
rules for putting one together), `flight` (what a design does when you
push it, and the closed-form plan the flown trip was read off), `world`
(one star system, the ship in it, the run, and the clocks they all run
on), `economy` (money: whole euros, the crew's shared pool, and sums that
must not wrap) and `time` (how long a day is). And two are the
multiplayer's: `wire`, what a client and the relay say to each other, and
`server`, the relay itself (`bims-server`), which links `wire` and nothing
else of the workspace. The fourteenth, `health` — a body's radiation dose,
its sickness and its cancer — went with the radiation (feature 104); the
body's parts, blood and wounds were always the room's
(`crates/game/src/health.rs`).

Things about that which are easy to get wrong:

- **Profiles only work at the workspace root.** A `[profile.*]` in a member's
  `Cargo.toml` is *silently ignored*. The root has `opt-level = 1` for our
  own crates in dev and `3` for every dependency, because a debug build of
  Bevy is unplayable; keep it there.
- **A bare `cargo test` is the app alone.** `default-members` makes a bare
  cargo command mean `app`, so `cargo test` runs one crate of thirteen. Use
  `cargo test --workspace`, which is what `./check` runs. `game` (the `bims`
  package) has unit tests of its own now, on the bare room `Game::bare`, and
  the three native probes left in `scratchpad/` beside them.
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

- **The HUD is minimal and hero-centred** (feature 107,
  `screens/hud.rs`, whose module note says where each piece sits and
  why none lands on another at 1280×720 or larger): portraits top left
  (`hud::portraits`), one frame top centre with **one warning chip** —
  `hud::threats`, most urgent first by `ThreatKind`'s declaration order,
  `+N` for the rest — the hero panel at the foot (`hud::hero_panel`,
  the ability boxes in it, and since feature 110 the player's own
  health big: the one bar and its number (task 120), and a beating red
  frame with *CRITICAL* while `hud::Hero::critical` holds — downed, or
  under twenty — with the countdown under a downed Bim's greyed cover),
  the tray bottom left
  (`CrewPanels::tray`: Armory — the Armory panel, where the Stash was
  until task 113 — Squad, Map, and Trade at a desk), *Back to ship* and the log
  bottom right (`hud::Log`: four lines, eight seconds each, the same
  words or one source's experience folded inside a second), the side
  panel on the right only for a **crewmate** picked
  (`CrewPanels::inspected` — the player's own is picked from the start),
  and the character sheet on K (`CrewPanels::character_sheet`). Each
  piece reads the rectangles the others took this frame or last
  (`ctx.memory(area_rect)`) and stands clear of them; a `ScrollArea`
  inside one wants `min_scrolled_height` as well as `max_height`, or
  egui gives a scrolling body its sixty-four points and no more. The
  experience in the log is read off `Progress::xp` going up, since the
  world says a level and not the points. A player whose Bim is out
  watches a crewmate through `ship::game::Game::spectate`, which the
  cameras follow in place of `local`. **`BIMS_SHEET=1`** opens with the
  sheet up, **`BIMS_TRAY=squad`** with that panel open, and
  **`BIMS_OUT=1`** lays the HUD out as if the player's own Bim were out
  without touching the world (a Bim out in a game of one is a run lost).
- **A fight won has a screen of its own** (`screens/fightwon.rs`): a
  modal the frame the site of the mission is cleared (`Run::fought` and
  `Run::cleared_here`, neither won nor lost, no departure check asking) —
  machines destroyed, the Manufacturers down, the bounty paid, the pool,
  each player's experience and levels, the bots' together, the relics
  kept, the townsfolk joined and the crew lost — with *Back to ship*
  (the bottom-right button's own `Order::ReturnToShip`) and *Stay here*.
  The world keeps no per-mission numbers, so `FightTally` notes the
  run's totals and every crew member's experience the first frame of a
  mission and adds the bounty and the relics up off the events. What
  follows is the game's own flow: the relic reward on the map, then the
  map. `BIMS_DROID_WAVE=2 BIMS_DROID_WAVES=1 BIMS_KEYS="40:X,80:4"
  BIMS_POINTER="45:move:1250,420;47:click:1250,420"
  BIMS_SMOKE_FRAMES=1500 bims droids` is the crew sent in and the
  screen up.
- **One frame is one system per screen**, in `EguiPrimaryContextPass`:
  step the simulation, lay the panels out, read the pointer, paint the
  shapes, put the words on top. `Screen` in `main.rs` is the state machine
  and each screen's plugin runs only in its state.
- **The canvas between the panels is Bevy's; a canvas inside a panel is
  egui's** (feature 97, *Bloom* below). For the world's canvas — the
  deck, the map, the chart, the yard — a screen hands the twelve floats a
  shape to `scene::WorldCanvas` (`world_canvas.shapes(..)`, a system
  parameter of the screens that have one), which makes each call a
  **layer**: a `Mesh2d` on the one camera, a z apart in the order
  painted, under egui and under the bloom — its shapes packed into
  records the GPU draws (`shapes::pack`, `shape.wgsl`, task 121, *The
  world canvas's shapes drawn on the GPU* above), or tessellated by
  `shapes::ShapeBuf` under `BIMS_SHAPES=cpu`. For a canvas inside a panel — the lobby's
  galaxy and diagram, the setup's portrait — `canvas::paint_shapes` adds
  an `egui::Mesh` of `ShapeBuf`'s triangles to the panel's own painter as
  before, **because egui
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
  comes out jagged beside everything else — **and into `shapes::pack`
  and `shape.wgsl` as well** (task 121), which draw the world's canvas
  from the same rule: a new shape that only `ShapeBuf` knows is a shape
  the deck does not draw.
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
  `flight`, `shipdesign`, `economy`, `worldgen` and `physics`
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
  share a key (Turn shares R with the fourth ability slot, and is read
  only in the yard and the armoury); the page says "also …" rather than
  refusing. Esc is not an action. The bindings are saved to
  `$XDG_CONFIG_HOME/bims/keys` (`~/.config/bims/keys`), one `action=Key`
  a line, and read at start — with the **edge-scroll speed**
  (`edge-scroll-speed=1.0`, the Esc sheet's slider), the one thing the
  sheet keeps between runs so far. **Four ability slots** (task 123):
  `Ability1`–`Ability4` on **Q C E R** (`ability-1`…`ability-4` in the
  file, an old `class-primary`/`class-secondary` read as 1 and 3); Q and
  E do what the class's two keys did, C and R are empty for every class
  and their boxes empty frames. **Ctrl and a slot's key, or a Ctrl-click
  on its box, is that slot's rank-up** and never the ability
  (`Keys::rank_up_asked`, `Keys::used`, `rank_up_by_click`), both going
  through the one `screens::game::rank_up`, which sends nothing yet —
  task 124 connects it; bevy_egui 0.42 reports Ctrl+C as the key *and*
  `Event::Copy`, and either is read. So **Select is F1 and Recruit L**.
  **The pointer against the window's edge pans** whatever a middle drag
  pans (`canvas::edge_pan`, `EDGE_SCROLL_ZONE` 8 points,
  `designer::PAN_SPEED` × the setting), in the game and the yard — off
  with the window unfocused, the Esc sheet up, a middle drag under way,
  or a `BIMS_POINTER` script driving the pointer. **A right-click on
  the deck moves the player's own Bim and nobody else**, the moment the
  button goes down (`bims::game::Game::orderable`), and **under arms a
  bot cannot be selected** — the crew taking arms lets go of any pick of
  one, and neither a click nor a marquee takes one again. **F is the
  attack-move**: it arms the pointer (the red crosshair), and the next
  click sends the player's own Bim there recruited, standing still to
  shoot whatever comes into its sights and walking on once nothing is
  left (`CrewOrder::AttackMove`, `Game::keep_attack_moving`). **A
  right-click on an enemy attacks it** (task 126, Dota's attack order;
  the attack-move's click on one too): `Game::enemy_at` finds the
  target under the pointer where the crew see — the cursor is a
  crosshair over one — and `CrewOrder::Attack { enemy }` has the
  player's own Bim fire at that one alone (`Combat::aim_only`) and walk
  after it until it has a shot (`Game::chase`), red brackets on the
  enemy, until it is down or another order is given. Every
  order's spot is pinged the way Dota pings one — arrows closing on it,
  green for a walk, red for an attack-move, over the fog, on the
  window's clock (`Game::draw_pings`). **X and Y are the two orders
  every player has for the bots that follow them** (feature 84; F and T
  until the attack-move took F, and the commander's squad fall back
  moved from X to T): X arms the pointer — the system's cursor goes and
  a red crosshair takes its place — and the next click on the deck puts
  an **attack banner** down there for the crew to fight their way to; Y
  calls them back to the ship — a blue **defend sign**
  (`theme::defend_banner`) stands on the spot they gather on, the deck
  just inside the ship's own airlock, for as long as the order does;
  **either key again lets them follow again** — X with a banner already
  down takes that banner up rather than arming the pointer for another
  one, which is the only press there is that does it: the world reads
  the *same* order given again as a release and "the same order" means
  the same **tile**, so a second banner anywhere else is a fresh attack
  and a crew left under one after a fight stands at it for ever, taking
  no errand (`screens::game::attack_key`); a banner **goes with the
  deck it stands on**, so leaving a place puts the crew back to
  following of its own accord —
  and Esc or a right-click puts the armed pointer away. That is what
  moved the camera's Follow onto **V**. Tab is the
  Inventory action: it opens and shuts the **Armory panel** (task 113,
  `CrewPanels::armory_window`) on every screen of a run — the deck, the
  map, the galaxy chart and the reward screen — read-only in a mission,
  with the **Nearby** strip (an engineer's deployables to pack up,
  `CrewPanels::nearby`) on it there. `BIMS_KEYS` knows `Tab` and `Space` by name and every letter the
  bindings use — `T` and `V` among them, which it did not until feature
  84's two orders were looked at from a terminal — and `+Shift` at one frame with `-Shift` at a later one holds Shift across a click between them — a Shift order, feature 69 (bevy_egui reads the modifier a frame late, so leave a frame or two each side).
  **egui moves keyboard focus on Tab**, and a widget with focus is egui
  wanting the keyboard, so the frame after a Tab every key would have
  been egui's: `keys::release_tab_focus` at the top of a screen's frame
  surrenders the focus Tab gave (`tab_took_focus` on the screen). A
  text field that has focus keeps it.
- **Nothing is stored, and the Armory panel is all of what the crew own**
  (task 113, *No storage* below): `CrewPanels::armory_window` off an
  `ArmoryView` the screen builds every frame (`screens::game::armory_of`)
  — a column a crew member with its HUD portrait (`hud::portrait`), its
  class and its four slots, the armory, the money and the keys. A thing
  moves by egui's own drag and drop (`dnd_drag_source`,
  `dnd_drop_zone`, an `ArmoryDrag` payload) or a right-click's rows, and
  every move is a `GearOrder` → `Command::{Equip, Unequip, Offer,
  AnswerOffer}`; the world's refusal is the log's line. There is no
  pack, no container window, no Loot window and no workbench window.
- **The UI scale is egui's zoom factor** (`theme::ui_scale_row`, on the Esc
  sheet's menu): it scales the type, the panels and the canvas alike, and
  bevy_egui divides the pointer by it, so nothing in the screens has to
  know. **A window opens at 150%** (`theme::UI_SCALE_DEFAULT`, set once by
  the theme's first pass) — **except a smoke run** (`BIMS_SMOKE_FRAMES`),
  which opens at 100%, since a `BIMS_POINTER` script is in points
  *before* the zoom and every recipe and screenshot size in these notes
  was written at 100%. `BIMS_UI_SCALE=1.5` (or `150`) opens either at
  that, which is how the default is looked at from a terminal. The
  choice on the Esc sheet is not kept between runs.
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
- **The crew's panels are one module**, `crew.rs`, and the screen hands
  the room its coordinates: a canvas point is the room's through
  `Session::room_point`, the pointer read back through the ship's camera
  and heading. Every `Game` method that takes a point takes a room point.
  (The test room's screen, gone in feature 104, read its own through
  `Game::view_scale/offset`.)
- **The health block is one bar** (task 120): the bar on the right-hand
  panel is the biggest thing on it (`crew::BAR_W`, `theme::health_bar`,
  `crew::HEALTH_NUMBER`), and under it a downed Bim has a red block —
  *DOWNED*, the seconds it has left, and who is reviving it or how —
  one slowed by a down says so until the mission ends, one under twenty
  says it is bleeding, and the dead say they died. The peril block, the
  per-part bars and the medicine's two boxes on the hero panel went with
  the blood, the traumas and the charges.
- **A downed Bim wears a countdown ring on the deck.**
  `theme::downed_ring` (on `countdown_ring`): a red ring emptying
  clockwise from twelve o'clock with the seconds in the middle, over
  every downed Bim — the crew, a town's, a Manufacturer — off
  `Game::down_left`, and small on the portraits. It replaced the red
  cross of the dying state. A right-click on a downed crewmate orders
  the player's own Bim to revive it (`CrewOrder::Revive`), or opens the
  menu with the reason greyed on its *Revive* row when it cannot.
  `BIMS_DYING=n` is how it is looked at.
- **A highlight is not a tooltip.** Resting on a row that names a fixture
  rings it on the deck (`CrewPanels::points`); nothing pops up. The ring is
  worked out afresh every frame from what is hovered, so a panel folding
  away under the pointer cannot leave one lit. Tooltips hang only off an
  underlined word (`theme::asks`) or a `?` (`theme::question_mark`), never
  a row or a bar.
- **A fixture menu opened by a press must not be shut by it.** `Menu::fresh`
  is that guard: the click-away check skips the frame the menu opened on.
- **The class's two keys have two boxes on the hero panel, and a
  level asks outright** (feature 80, `screens/game.rs`; a bar of their
  own at the foot of the canvas until feature 107). `ability_boxes`
  reads one `AbilityBox` a key off the world every frame — nothing kept
  between frames — and `ability_row` lays them out inside
  `hud::hero_panel`, which stands clear of the tray and the character
  sheet the way the bar stood clear of the tray; each box is the key in
  one corner, the picture in the middle, **how many are left** in the
  other, and its name and tip on a hover. What is counted is
  the world's (`World::{sentries_left, kits_of, grenades_of,
  beam_patients, squad_members}`) and what greys a box out is
  `world::class::key_level` — every class's **E** from the first level
  and its **Q** from the third — with the cooldown over the picture. The
  pictures are the deck's own marks (`theme::{surge_mark, wall_mark, taunt_ring,
  aura_ring, squad_mark, heal_beam}`, and `brace_mark` which is the
  box's alone), or the thing itself out of `icons.rs` for a kit and a
  grenade, so a box and what the key does are one picture. The box
  never decides anything: `class_key` is still what the press goes
  through, and it knows about the pointer as well.
  (The medicine's two boxes that stood past a rule at the right-hand
  end went with the medicine in task 120.) **A cooldown is
  drawn the way Dota 2 draws one**, on every box that has one: with
  nothing left, the share still to come is laid dark over the whole box
  and swept back clockwise from twelve o'clock as it runs out
  (`cooldown_sweep`, a fan from the middle through the corners, with a
  hand) and the seconds in the middle; with some left and the next on
  its way, the count sits on a disc in the corner and a **ring** round
  the disc fills clockwise (`recharge_badge`). `Face::charges` is the
  reading for any stock of charges — the engineer's kits and the
  grenade — off `World::{charges, charges_of,
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
  stands: a relic's surge halo, the grenade in flight and its burst, the
  squad bracket, the aura's and the rally's rings on everybody *else*.
  **The numbers are the screen's own state and nothing else's**: neither
  the room nor the world records how much a beam put back, so
  `GameScreen::heals` watches each beamed body's blood and parts frame
  by frame, gathers the gain into whole numbers and floats one at most
  every `HEAL_GAP` — at 24× a beam puts back a dozen points a second,
  and a number a frame is a green smear rather than a figure.
- **A level is spent on the character sheet's talent tree** (feature
  83's Skills tab, moved onto the sheet by feature 107,
  `CrewPanels::talents`). The class's ten levels
  run down the sheet, numbered, with the spine beside them lit as far as
  the level reached: a level with nothing to choose is one slot across
  the width (`names::level_name`, the same three levels `level_line`
  describes), and a pick level is two side by side. A box is coloured for
  its state — learnt, **given up** (struck through: the other side of a
  level chosen at), open for a point, or waiting on a level — and over
  the tree is how many **skill points** are left, which is every pick
  level reached and not chosen at. A click picks a slot and the lines
  **under** the tree — under it since feature 107, the sheet hanging from
  the top of the canvas, so what is written below cannot shove the tree
  from under the pointer; beside it, as it was in the tray, it made the
  sheet too wide to leave the hero panel room — say the slot's name, the level it sits at,
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
  type on the tree is `SKILL_TEXT`/`SKILL_BOX_TEXT`/`SKILL_NAME_TEXT`
  rather than the panels' small, and the tree's geometry is
  `skill_tree_size` off `SKILL_GUTTER`/`SKILL_BOX_W`/`SKILL_BOX_H`, the
  whole `SHEET_W` across. The
  pick is the same
  `Order::PickTalent` the crew panel's row sends — the panel's row is
  still there — and the tree asks nothing of the world it is not handed:
  `ClassView` (`picks` and `xp` as well as `talents` now) and `world::class`'s own
  tables are all of it. Nothing opens by itself any more: the hero
  panel carries a **+1** while a point is waiting, and it opens the
  sheet, as K and the player's own portrait twice do.
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
  station's, and a planet's air by its biome — temperate, desert, arctic
  — set down at a settlement, `Bed::of_biome`; the engines' bed and their
  start went with the flown trip in feature 104) loops the whole time at whatever level the
  screen asks for *every frame*, and fades out when nobody asks, so a
  screen that closes takes its sound with it. Cues are thinned with a
  cool-down **per kind and per place**: at 24x a frame holds many of one
  noise from one place, which is one sound, but two guns in one frame
  are two shots — a fight puts gunners on the same cadence, and a
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
two pinned numbers, clippy on the app, and a smoke run of **four windows**
— the menu (`game`), the world (`simulation`), the yard (`design`) and a
fight (`droids`) — sixty frames each on `./hidden`'s headless compositor,
all four at once, with a screenshot of the last three left in
`target/check/`;
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
`./check full`. The windows go the same way — four in the quick tier, all
at once (`SMOKE_AT_ONCE`), since a run is seven seconds and mostly Bevy
starting up; every other window is one of those with a dial moved and is
the full tier's, `restarted` (feature 79) among them. The `room` window
went with the test room, and `raided` and `landed` with the raid and the
landing (feature 104).
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
| `crates/game/CLAUDE.md` | the room: routes, errands, doors, sight and the dark, the fight, the machines, medicine and carrying, the classes' room halves, blood on the deck, the fixtures as pictures, the bare room the tests stand in |
| `crates/shipdesign/CLAUDE.md` | the rules: parts, layers, power, cargo, money, the hash, the validator, the playtest ship |
| `crates/world/CLAUDE.md` | the world: the clocks and the stages, the run, travel and missions, stations and docking, the fight across two rooms, the machines and the crisis, the classes, the crew aboard, crafting and money, the walk outside |
| `crates/ship/CLAUDE.md` | the designer and the game view: the camera, the painters, drags, `Session` |
| `crates/flight/CLAUDE.md` | a plan is read, never integrated; the two pinned placeholder numbers |
| `crates/lobby/CLAUDE.md` | "has a station" is answered by generating the system; a crew never starts at an enemy's |
| `crates/worldgen/CLAUDE.md` | the generator: the checksum, up to six stations a system, which of them are hostile, who sells what |
| `scratchpad/CLAUDE.md` | the three native probes left (`crowding`, `layout`, `droidwreck`), and how to look at a room without a window |

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
