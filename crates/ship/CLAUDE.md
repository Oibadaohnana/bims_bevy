# The ship's crate

Notes on `crates/ship` — the designer, the game view, the camera, the
painters, and `Session`, which is the design phase and the game it turns
into. The rules it asks are in `crates/shipdesign/CLAUDE.md`, the world it
draws in `crates/world/CLAUDE.md`, and the screens over it are
`crates/app/src/screens/designer.rs` and `game.rs`.

> **Since the port to Bevy (September 2026):** the browser host is gone.
> Every name table this file points at in `web/*.js` is now
> `crates/app/src/names.rs`; the pages are the screens in
> `crates/app/src/screens/`; the `bims_*` and `ship_*` exports are methods on
> `bims::game::Game` and `ship::Session`. The node harnesses
> (`scratchpad/*.mjs`, `smoke.mjs`, `stub.mjs`) are gone with the host — a
> check this file says lives in one of them lives nowhere now unless it says
> so in `crates/app`'s tests or `./check smoke`, and is worth restoring as a
> unit test if the thing it guarded moves again.

## The mated airlocks are a door, and the door is a picture clock

`Game::airlock_ajar` in `crates/ship/src/game.rs` is how far the two mated
doors stand open, 0 to 1. `tick_airlock` (called from `Session::render`, beside
`frame`) eases it towards open while anybody in the joined room is within
`AIRLOCK_HAIL` — a tile and a half — of the ship's port face, and towards
shut otherwise; `hull::airlock` slides the two halves apart by it, on the
ship's door and the station's alike, since they are one passage. It is a
**picture** clock like `frame`: nothing that decides anything reads it, and
the passage is walkable whatever the door looks like — a Bim ordered
through a shut-looking door walks through it and the door opens as it
arrives. `the_airlock_opens_for_whoever_comes_to_it_and_shuts_behind_them`
in `crates/ship/src/tests.rs` pins the easing and the shutting.

If a Bim ever "cannot go through an airlock" in the browser, check the
served build first: `nix run .` serves the store copy it started with, and
the crossing was verified natively (`docked_the_ship_and_the_station_are_one_room_and_the_crew_can_cross`)
and from a right-click in `simulation-check.mjs`.

## The camera never rotates; the ship does

North is up in every view, always. A camera that followed the heading would
make a flip legible and every other moment unreadable — you could not tell
which way you were going, because "which way" would always look the same.

So the design is drawn turned by the heading (each tile emitted with `rot` set,
which the host's `ctx.rotate` applies), and the starfield, anything drawn
because it is *out there*, and the whole map are not. The pointer goes back the
same way: `Game::tile_at` is the painter's arithmetic read backwards, and
`a_screen_point_maps_back_to_the_tile_it_is_over` in `crates/ship/src/tests.rs`
checks it at four headings. A wrong sign there is a ship you cannot click on
once it has turned, and nothing else would say so.

`node scratchpad/ship-layout.mjs game` and `... map` are the two views as SVG.
After anything in `world_paint.rs` they are worth thirty seconds: a hull drawn
mirrored, or a starfield turning with the ship, is obvious there and invisible
in every assertion.

**Head up is the player's exception, and it is one number.** `Game::head_up`
(the View buttons and `N` in `crates/app/src/screens/game.rs`, `Game::head_up` at the
boundary) holds the ship square to the window in both views and turns the sky,
the station alongside and the map round it instead. It is done without a
second camera: `Game::camera_turn()` is `-heading` when it is on and nothing
otherwise, `Game::ship_turn()` is `heading + camera_turn()`, and the rule is
that **everything drawing or reading back the ship goes through `ship_turn`
and everything drawing or reading back the world goes through `camera_turn`**
— the tiles, the room aboard, the hover ring and `crew_on_screen` on one side;
the starfield, `local_node`, the map and `DrawList::turn_from` on the other;
`tile_at`, `point_at` and `pick` each on the side of what they read. A new
thing drawn in the game view has to pick a side, or it sits still while
everything round it turns. Two knock-ons: the starfield tiles a square round
the ship rather than the window when head up, or the corners go bare after
the turn; and a view setting is not a `Command` — it is this browser's own
and crosses no seam. `ship-layout.mjs headup` and `... mapup` are the pictures.

## The ship view is about the crew member you steer

`Camera::focus` is the point, in the camera's units about the ship's
centre of mass, that sits in the middle of the window at a pan of nought
and that the pan is clamped about. `Game::follow_player` sets it to
`crew_on_screen(PLAYER)` every frame from `Session::render`, so the view
follows James off the ship and through the airlock, and cannot be dragged
until he is off the edge — it used to clamp about the *ship*, which is why
he could not be zoomed in on across the station. Everything is still
**drawn** about the ship — the painter, `stations`, the pointer — and only
`offset_x`/`offset_y` know about the focus, so nothing else had to change;
`Camera::zoom` is written in terms of `to_view` for the same reason. The
map's focus stays nought, the ship.
`the_camera_follows_the_crew_member_the_player_steers` pins it, including
a zoom about a corner and a pan to the limit.

**`Game::follow` is the player's exception, and `Camera::set_loose` is
how.** The game *opens* free — the whole ship in the middle, dragged
anywhere — and Follow or `F` tethers it; `C` recentres on the steered crew
member either way (`Game::centre_on_player`). The Follow / Free camera buttons and `F` (`Game::set_follow`, beside
`Game::head_up`) let the ship view go: `follow_player` then sets
nothing, and the camera is *loose* — a pan moves the **focus** rather than
the clamped pan, and nothing clamps it, so the view goes wherever it is
dragged. Letting go folds the pan into the focus first (`absorb_pan`) so
nothing on screen moves for the flip of a switch; tethering again zeroes
nothing, the next `follow_player` sets the focus and the view snaps back.
One camera and one transform either way — do not add a second camera for
the free one. The same test pins both halves and `ship-check.mjs` has a
section on the buttons and the key.

## The ship is drawn in its own frame, and the exhaust is read off the plan

`paint_ship` builds the whole ship — rim, exhaust, tiles, the hull's
pictures, the lights, the hover ring — into a ship-space `DrawList` in
design units and turns it once with `DrawList::append_turned`; the room's
buffer goes through the same call after it. A picture made of many shapes
only has to be right the once, and `crates/ship/src/hull.rs`, where the
pictures of the plating, the engines, the thrusters, the airlock and the
array live, has never heard of a heading. Anything new drawn *on* the ship
goes into that list; anything drawn because it is out there does not.

**What fires is `hull::Firing`, and it comes from `flight::effort_at`** —
the plan read a second way, beside `state_at`, and pinned against it by
`the_effort_is_the_derivative_of_the_state`. Nothing in the picture keeps
its own idea of whether the engines are on, because a flame that lagged the
ship at 24x or after a catch-up would say the ship was in two places. Four
rules that fall out of it:

- **The flame is blue.** There is no fuel; the exhaust is plasma off the
  reactor, and `hull.rs`'s `FLAME_*` and `paint.rs`'s `FLAME` (the designer's
  exhaust marks) are a white-blue core, an electric blue body and a violet
  tail. The thrusters' `PUFF` was already pale blue. And the **reactor
  glows with its load**: `fittings::reactor_glow(list, part, load, frame)`
  is drawn over every `supplies()` part after the tiles — in the game at
  `World::power().load()` (day-long draw plus what the engines draw now,
  over the supply), so a burn lights the reactor as well as the stern; in
  the designer at the static `draw / supply` of `power_budget`, frame 0 —
  a halo over the housing and a brighter core, amber for the reactor and
  plasma blue for the large one, breathing a little off the frame.
- **A forward engine burns through the burn *and* through a flip brake**;
  a backward one through a brake without a flip; a sideways one never — the
  autopilot does not fly it. `Firing::of` is the arithmetic (the nose
  against the plan's line, times the sign of the acceleration), and
  `the_exhaust_follows_the_plan` in `crates/ship/src/tests.rs` drives a
  real trip through every phase and checks it.
- **A thruster's nozzle is every side of it that faces open space, and the
  one that fires is worked out from where the thruster is** — exhaust
  pushes the ship the other way, that push turns it about the centre of
  mass, and the nozzle whose turn matches the plan's is lit. The dynamics
  never look at placement; the picture does, because a corner thruster
  puffing into the hull is a picture of a broken ship.
- **The exhaust is drawn under the hull, and a plume starts where it
  clears the skin.** (An engine can no longer be inside the hull — see the
  exhaust rule — but the walk aft costs nothing and keeps the picture right
  for a design that predates it.) The playtest ship's engine sits inside the hull, and a
  flame from its bell was a dim smudge at the stern with the bright end
  under the deck; `plume` walks the tiles aft until one holds nothing and
  begins there. An engine flush with the stern is unchanged.
- **The flicker and the running lights run off `Game::frame`**, a picture
  clock counted in `Session::render` and read by nothing that decides anything
  — never the RNG, which is the simulation's, and never `world.steps`,
  which stops at a pause. Hash it; do not draw from the stream.
- **The starfield streams on the world's clock.** `Starfield::advance`
  (`Game::stream_sky`, from `Session::render`) moves each layer on by the
  log-mapped speed times the minutes the clock moved since the last frame,
  wrapped to the tile — so a steady speed is a steady stream, a pause holds
  the sky, and 24x is twenty-four times the stream. It used to be a
  displacement off the speed, which at any constant speed is a still
  picture. `the_sky_streams_on_the_world_s_clock_and_only_under_way` pins it.

`ship-layout.mjs turn` and `... burn` are the pictures, both head up so a
puff into the hull or a flame over the deck is obvious. The map marker is
`hull::marker` — three rectangles, and `FIN_LEAN` is pinned by
`the_map_is_north_up_whatever_the_ship_is_doing`, which knows the marker is
the only thing on the map that turns.

**A station's people are drawn over the ship, since they can be on it.**
`stations` is painted before the ship — the residents' room with it —
and their room's deck holds the ship (`crates/world/CLAUDE.md`, "The
residents' room holds the ship"), so one who has followed the crew
through the passage stood *under* the hull and the room aboard: a name
walking about over an empty tile. The room hands its frame over in two
pieces (`bims::game::Game::shapes_split`: the deck, then the bodies and
everything drawn over them — the dropped weapons, the Bims, the bedding,
the shots), `stations` paints the first and returns the second placed
and turned, and `paint_ship` appends it after the room aboard.
`a_resident_on_the_ship_s_deck_is_drawn_over_it` pins it, and fails with
the bodies painted in `stations`.

## The map says whose a station is, and the rule is the world's

`World::stance` is the one answer — home is friendly, the world's
`hostile` list is hostile, everywhere else is neutral — and the painter
draws it twice without deciding anything. On the map (`paint_map`) every
discovered station's icon is ringed by stance: `ENEMY` red for hostile,
`FRIEND` blue for anything else somebody lives on — home and a stranger's
alike — and nothing for a derelict, which is nobody's. It was green for
home and nothing for neutral; blue for every station that is not an
enemy's went in with the generator putting the enemy's stations in one
corner of a system (`crates/worldgen/CLAUDE.md`), since the point of the
corner is to be seen on the map. The ring is `STANCE_RING`
times the icon size, outside the aim ring, so pointing the helm at an
enemy's station draws two rings that read apart; the blue is not the aim
ring's cyan for the same reason. Every discovered belt gets a **pickaxe**
at its top-right shoulder (`paint_pickaxe`: a haft and a bent head, four
rounded rectangles and a disc), because a belt is a mining site — hold
station at it and the rocks are laid out — and the map should say so
before the crew fly there. Nothing else stands at a belt
(`worldgen::data::parent_suits`), so the pickaxe has the belt to itself.
Out of the window
(`stations`) a hostile station's far plate — the black `HULL_UNKNOWN` fog
plate every stranger's is — gets a wash of the same red at `ENEMY_TINT`
under its icon; a neutral stranger's stays black. `ENEMY` is the lobby's
`lobby::draw::ENEMY` by value, not by import (`ship` does not depend on
`lobby`): the player learnt the colour on the system diagram before they
launched, and it has to be the same colour here. Change one, change both.

**Where you are is a reticle, drawn last.** `world_paint::here_reticle`
rings the ship on the map in `GLOW` — `HERE_RING` pixels across, wider
than a station's stance ring so a docked ship's mark stands out from the
station icon it sits on, with a tick at each compass point and a wash
inside that breathes on `game.frame` over `HERE_PULSE` frames — after
`turn_from`, so it is the screen's and its ticks stay square to the
window whatever the camera did, and `hull::marker` goes on top of it. It
is sized in the camera's units off the scale, like every icon, so it is
the same size at any zoom. The words are the app's: `screens/game.rs`
writes `You · ` and `whereabouts(session)` — the trip strip's first words,
*Docked · the station*, *Alongside the belt* or *Open space* — over it in
`theme::YOURS` with `name_over`, `HERE_LIFT` above `view.to_canvas(ZERO)`,
which is the ship wherever the map has been panned. The galaxy chart tags
the ship's star the same way (`· here`). Before this the marker was
eighteen pixels of hull under a twenty-pixel station icon, and a player
who had panned the map could not find themselves on it.

## "Is it finished" is one export, not three

`ship_phase()` and nothing else. "Is it finished", "may I still edit" and
"which phase is it" are the same question, and three exports answering it are
three things that can disagree — there were three for about an hour, and the
boundary check in `scratchpad/ship-check.mjs` is what said so. `PHASE_DESIGN`
in `crates/app/src/screens/game.rs` is the host's half of the pair.

That check is worth keeping in mind generally: it reads `crates/app/src/screens/game.rs` with
`readFileSync`, collects every `wasm.ship_*` it calls, and compares both ways
against the real exports. An export nothing calls fails it unless it is named
in `FOR_THE_HARNESS`. It is deliberately **not** built on the shell's `grep`,
which here is `ugrep --ignore-files` and returns nothing at all for files under
`web/` — a boundary check built on that comes back clean because it never read
the file.

## A drag is geometry; the edits go out one at a time

`ship_drag_*` works out which tiles a drag covers and, for a clearing drag,
which parts it would take off. The host reads that list and sends **each tile
as its own Edit through `net`**. There is deliberately no bulk operation, so a
transport has nothing extra to learn later.

Two things in that order matter:

- **Read the whole list before applying any of it.** The parts a clearing drag
  names are looked up in the design it was drawn over; applying as you go has
  the list shifting under itself.
- **Objects come off before deck, and only the top of each tile comes off
  at all.** The other way round, every floor tile with something standing
  on it is refused as `FloorUnderObject` and a right-drag over the galley
  leaves the deck behind and looks half broken. That ordering — and the
  one-layer peel — is in `Editor::drag_parts`, not in the host.

A failing Edit inside a drag is **skipped and counted, never fatal**: a
rectangle of deck over a half-floored room is meant to fill the gaps.

## A drag reports its *first* refusal, not its last

A removing drag goes from the top of the stack down, so the first thing to
refuse is the thing the player was pointing at — and everything underneath it
then refuses too, because it is holding that up. Reporting the last one
answers a question nobody asked: "take what is standing on it off first" about
the frame, when what actually said no was the shelf with a hundred units of
ore in it.

## `?random=1` is the simulation somewhere else

`nix run .#test` is `ship.html?mode=1&random=1`. The page rolls a seed and
a pick with `Math.random` — nothing about them has to agree with anybody —
and `ship_pick_dock(seed, galaxy, roll)` turns the pick into a station
somebody lives on, the `roll`-th across the whole galaxy
(`world::spawn_anywhere`, which generates every system, as the lobby does).
`session::pick_dock` is the other half of the same answer. A `seedHi`/
`seedLo`/`roll` on the query pins it, which is how `simulation-check.mjs`
looks at it: the same roll is the same place, a different roll another,
and roll 0 is the simulation's own dock. Never a derelict.

## `ship.html` has three ways in, and no spawn of its own

`web/ship.html` reads `mode`, `star` and `station` off its query with
everything else. `mode=1` is the simulation: `Session::simulate` settles
`shipdesign::playtest_ship()` and opens the world at once — the default seed,
a two-arm spiral, `world::spawn`'s dock and `SIMULATION_MONEY`, each
overridden by the query when it says. Anything else is the game, and the
game **must be told where to start**: `Session::design` takes the star and the
station, `Session::spawn_ok` is asked once at boot, and a page with no spawn or
a wrong one shows the `lost` screen with the link back to `builder.html` —
before a design phase, never after an hour of laying one out, and never a
different dock. `World::start` takes the pair for the same reason and
returns `StartError::NoSuchStation` rather than choosing.

Three things that follow, and bit on the way:

- **Every harness that wants a design phase has to bring a spawn.** There is
  no lobby in front of it, so `scratchpad/spawn.mjs` boots a bare page once
  and reads `Session::simulation_spawn`/`_station` — the simulation's dock,
  exported for exactly this — and `ship-check.mjs`'s `session()` appends it
  unless the query names its own. A session opened with `spawn: false` gets
  the error screen, and is the check that it exists.
- **`world::spawn` is the simulation's and the fixtures', and nothing
  else's.** `world::fixture::simulation_world` is how every fixture world
  starts, so the reference checksum did not move when `World::start` stopped
  choosing.
- **"Nearest discovered node" at the spawn is the dock's own parent body**,
  which the planner rightly calls `AlreadyThere`; nothing else is in sight.
  `the_playtest_ship_can_fly_somewhere_from_the_simulation_spawn` therefore
  reveals the next node out through `discover_for_probe` and plans to that.

`scratchpad/flow-check.mjs` walks the seam the two page harnesses cannot:
lobby → station → Start → the designer opened with that query → build →
Accept → docked at the chosen star and station. `simulation-check.mjs` is the
other command. `flyer.mjs` is the flyable build both it and `ship-layout.mjs`
use; `ship-check.mjs` keeps its own because the checks between the parts are
the point there.

## The designer opens on the playtest ship, as a gift

`Session::design` takes a `preset`: `PRESET_PLAYTEST` (the default, and what a
page with no `preset=` on its query gets) lays `playtest_ship_on(area)` in
the middle of the build area; `PRESET_EMPTY` is a bare grid. The ship is
**given**: `Budget::with_gift` records its price as `given`, so `remaining`
starts at the whole pool and the readout says the crew have spent nothing.
Taking a given part off refunds its price like any removal — a gift is a
gift, and "for now" it is fine that a player can sell the ship they were
handed. A build area under twenty tiles gets an empty grid rather than half
a ship.

Two knock-ons for harnesses: `ship-check.mjs`'s `session()` appends
`preset=0` unless the query names one, because everything in it builds its
own; and `flow-check.mjs` accepts the preset as it stands, because that is
now the shortest path a player has to the world. `ship-layout.mjs given` is
the picture.

## Fittings are the pictures the room has none of

`crates/ship/src/fittings.rs` draws what `hull` and the room between them do
not: the plain wall and the diagonal wall, the door (its leaves parted along
the part's long side, in its own frame), the conduit, the helm, the
shelf and the shower, in each part's own frame through `hull::Local` so a
turned part is drawn turned. `world_paint::hull_tiles` asks `hull::part`
first and `fittings::part` second and draws a block for whatever both
refuse. Every part has a picture now — the reactor, life support,
the battery, and the workshop: the smelter, the workbench, the suit locker,
the armoury and, last, the **drug lab** (`fittings::drug_lab`, the
`PartKind::DrugLab` arm of `fittings::part`) — so a block on the deck is
a new part somebody forgot to draw. The drug lab is the workbench's
footprint and stance, and its picture says so: the same frame and drawer
on the near side where the Bim stands, but the bench top in the palette
swatch's clinical green-white (`LAB`), a rail of five capped vials along
the far edge, a round-bottomed flask on a ring stand in the middle with
the dressing's green in it, a small still at the right-hand end — a
flame, a pot, a riser, a coil and the receiver it drips into — and the
steriliser's lamp lit at the left-hand end, since the bench is on. It
was looked at as SVG at `R0` and `R90` beside the
workbench through a throwaway test since removed; the designer's palette
swatch stays a colour, not a picture, so nothing changed there.
**The design phase draws the same pictures** (`paint::objects` asks
the same two, in the same order), and the room's own fixtures on top of
them through `paint::fixtures`: the room is laid out from the design as it
stands with `bims::aboard::layout_of` and asked, through
`Room::draw_fixtures` and a `room::Fixtures` of which parts the design has,
for the pictures of those and no others — a layout puts every fixture it
has not got on the worktop, and drawn whole it would be a heap of galley on
one tile. The picture is cached on the `Editor` (`Editor::fixtures`) and
redone in `refresh` with the issues, since laying the room out is a walk of
the whole design. `bims design` is how to look at it.

## Three sessions, and the two windows that read the world before a command is sent

`Session::simulate` is `simulate_on(playtest_ship(), 1, ..)`;
`simulate_on(design, crew, ..)` opens `Game::start_with_crew` — `crew`
aboard, one of them the player — and the `test` command uses it on the
combat ship with one crew member so a mercenary hired at the dock has a
bunk; `Session::combat` is the fight (`crates/world/CLAUDE.md`'s arena),
`Session::mercenary_for_probe` the `test` command's hired hand.
`Session::at_the_desk(slot)`/`walk_to_desk(slot)` and
`Session::mercenary_fee(who)` are the app's reads for the trade window,
the desk's row and the `?` over a mercenary's name
(`theme::badge_over`). The Hire window in `crates/app/src/crew.rs` is
the Loot window's shape: opened off the `HIT_VISITOR` menu on a body on
its feet (`Open::Hire`), the walk over left to the screen (`walk`), and
`CrewPanels::terms` handed in every frame off `World::hire_offer` so the
button is greyed with the reason — out of reach, no bunk, not the money
— before `GearOrder::Hire` goes through the seam as `Command::Hire`.
`fittings::trading_desk` is the desk's picture: a counter with a ledge
and a lit terminal along the far edge, a ledger and a coin tray on the
near side.

## The rocks are part of the ship's picture, and the pick is the pointer's

`world_paint::rocks` draws the mining site's tiles into the **ship-space**
list, before `hull::shadow`, so they turn with the hull: they were laid
out on the ship's tile grid (`world::mining`) and that is the whole reason
a click on one is `Game::tile_at`. `ROCK_COLORS` is indexed by
`Rock::code` — stone, iron ore silver, galvum purple — a marked tile is
tinted and ringed in `MARK`, and the hover ring rings a rock as well as a
deck tile while `Game::marking` (the Actions tab's Mine tool, this
window's own) is on. `local_node` draws nothing for a belt the ship has a
site at: the handful of icon-rocks under the hull would sit among the
real ones. `BIMS_AT_BELT=1 bims simulation` is the picture, and
`BIMS_POINTER`'s `wheel` verb zooms it out far enough to see the field.

## The conduit is drawn linked, and only in the electricity view

`fittings::conduit` is **not** reached through `fittings::part`: it takes
`links` — which of the four neighbours is also conduit, from
`fittings::conduit_links`, the same four-neighbour rule `shipdesign::power`
joins a network by — and draws a pad with an arm towards each linked side,
so a run reads as a line, a bend as a corner and a lone tile as a stub, the
way RimWorld draws its. A part standing over the run is joined *through*
the tile and does not get an arm. The design phase draws it always
(`paint::conduit`), since laying it is what the designer is for; the game
draws it only under `Game::overlay == Overlay::Electricity`, the tray's
View tab (`crew.rs`, `Tab::View`, through the `Actions` bundle), and
`world_paint::hull_tiles` skips the utility layer altogether.
`world_paint::electricity` is the overlay: every part that supplies, draws
or stores power washed and rung — `LIVE` on a network with a reactor,
`DEAD` otherwise, asked of `shipdesign::networks` once — with the conduit
on top, appended **after** the room's picture so a powered galley fixture
is rung over its own picture. `Overlay` is a view setting like `head_up`:
this window's own, read by the painter and by nothing that decides
anything. The **numbers** over the drainers are the app's words, off
`Session::power_labels` → `world_paint::power_labels`: one `PowerLabel` a
consumer or engine — the camera-frame point over the top of its footprint
(the same `on_screen` the crew's names use), `now` and `full`, and `live`
— which `screens/game.rs` writes in `theme::DRAW` yellow with
`name_over` while the overlay is up, `now` alone when the two agree and
`now / full` for an engine idle or throttled, muted for a dark part. An
engine's `now` is its `thrust_power × throttle` while `Firing` lights its
facing and it is on a live network, nought otherwise, so the numbers
agree with the exhaust.

## The blueprint is the part's own picture, and its answer is asked once a tile

`Game::placing` is the Build tab's tool — a kind and a rotation, this
window's own like `marking` — and `world_paint::blueprint` draws it over
the tile under the pointer as the part's own picture faded
(`faded_part`: `hull::part`, then `fittings::part`, then the deck's tile
for plating or the colour block, through `DrawList::append_faded`), rung
in `LIVE` or `DEAD` by `Game::ghost_ok`, with the use spots marked as the
designer marks them. `world_paint::sites` draws every `BuildSite` the
same way in `BLUEPRINT` blue with a bar along its foot for what has
arrived. Both go into the ship-space list after `hull_tiles`, so they turn
with the hull.

**The answer is `World::can_place_site`, and it validates the whole
ship**, so `Game::ghost_check` asks once per `(kind, tile, rotation)` and
keeps it (`ghost_check`), `Session::render` asks before painting, and
`Game::step` forgets it only when the design hash, the site count or the
ship's state moved — not every step, or 24x would validate the ship
twenty-four times a frame under a still pointer. `ghost_answer` is the
read-only view for the readout, a frame behind at most. `set_placing`
forgets it with the tool; `rotate_placing` is `R`. `site_at(tile)` is
what the readout names a blueprint by.
`the_blueprint_asks_the_world_once_a_tile_and_finds_a_site_under_the_pointer`
pins it. The app side — `Tab::Build`, `Tool::Build(kind)`, `BUILD_GROUPS`
and the search — is `crates/app/src/crew.rs` and `names.rs`; `R` there
turns the blueprint when one is in hand and recruits otherwise.

## A container's grid is the app's window over the hold, and the seam is a gear order

The ship crate draws no grid and moves no piece. A click on the deck
reaches `Game::hit_at` through `Session::room_point` as it always did,
and `HIT_BENCH`, `HIT_SHELF` and `HIT_FRIDGE` come back to the app, where
`CrewPanels::open_menu` (`crates/app/src/crew.rs`) decides whether the
fixture is a **container**: a bench whose part keeps a class of goods
(`PartKind::def().capacity` — the armoury and the drug lab open the
lockers; the smelter and the workbench open nothing, and have no menu
either), any shelf, and the cold store through the "Open" row its menu
gained. Opening one walks the Bim shown to `Game::container_spot`
through `send_to` — the click is the natural place to start it walking,
since nothing moves until it is within `world::data::REACH` — and puts
the container window (`container_window`) up with the inventory pop-up
beside it (`fixed_pos`, since the two are read together). `Esc` shuts
the innermost thing first: a cell's pop-up, then a fixture menu, then
the window (`CrewPanels::escape`). Nothing is a container in the test
room, which has no hold: `CrewPanels::hold` is `None` there and a bench
click is a menu as before.

**The window is a class, not the fixture** — the app's reading of the
world's rule that the class is the one truth about capacity. The lockers
are fifteen by fifteen, the shelves twenty by twenty, the cold store ten
by ten (`grid::grid`, one egui response for the whole grid, since four
hundred widgets a frame would show nothing for the cost): one cell per
piece of armour in the hold with a sliver of its health under it, then
one stack a resource with its count, showing `World::free` rather than
raw cargo. So the drug lab's window is the armoury's, and a piece stowed
across a shelf appears in the lockers' window and not the shelves' —
showing it in both would be two of it. The tooltips are `names::item_tip`
and the icons `icons::icon`, one per `ResourceId` (a compile error to
leave out) and a piece's cracked when it is broken.

The seam is `crew::GearOrder` — Stow, Fetch, Equip, Unequip, Discard, the
world's five commands with the sender left off — pushed onto
`CrewPanels::orders` by a row or a ctrl-click and drained by the screen:
on the ship `screens/game.rs` wraps each as `Order::Gear` and
`screens/designer.rs` maps it onto `Command::Stow…` stamped with the
slot and the step like every other order, so the hold's every change is
a command every player's ship applies; in the room `screens/room.rs`
applies Equip, Unequip and Discard straight to the `Game`, there being
no hold to stow into or fetch from. The rows say why before the world
does: `hold_of` in `screens/game.rs` snapshots the hold once a frame —
`World::free` a resource, the pieces at `Where::Hold`, each class's fill
off `Session::storage_used`/`storage_capacity`, and `World::in_reach`
for the Bim shown — so Store and Take are greyed with "walk over
first", "the pack is full" or "no room left in the lockers", a broken
piece's Store says to discard it instead, and a ctrl-click that cannot
go opens the pop-up whose row says so, rather than sending a command
the world refuses. Equip needs no reach and is greyed only for a dead
Bim; a broken piece goes on and does nothing, as the room allows. The
orders act on **the Bim
whose inventory is shown** — the selected crew member, else the
player's — not always the player's, which is what lets a crewmate be
dressed from its own tab.

Two things that bit:

- **An egui `Area` claims its bounding box for the pointer.** The
  `game-left` stack's first row — Bims, the clock, "Recruited" — was as
  wide as the three together and sat over a stack as tall as the panel,
  so egui counted the whole rectangle as its own and a canvas click on
  the ship's left half (the cold store at `(3, 7)`, the shelves) never
  reached the room. The row is its own `game-top-left` Area now; the
  stack under it starts below the row. A click that lands on nothing
  is this before it is anything else.
- **The health bars are two-tone on purpose**, not two bars:
  `theme::two_tone_bar`/`thin_two_tone_bar` draw the green with the
  armour's blue appended in proportion, and the text reads `100 hp +
  15 hp`, so a worn piece reads as more bar rather than a second one to
  look for. `theme::popup` is the one pop-up — the fixture menus, the
  cell rows and the Unequip row all go through it — so a change to the
  look of one is a change to all.

`BIMS_FIGHT=1 bims combat` with a `BIMS_POINTER` script that clicks the
armoury at tile `(14, 7)` is how it is looked at: the window reads
"Armoury · 13 of 16 in the lockers" with the three pieces, the four
weapons, the suit and
the bandages, and at 10× the Bim walks over and the rows come alive.
The fight itself is looked at the same way: `BIMS_WEAPON=schword` (or
`pistol|shotgun|rifle|sniper`) in the crew member's hand,
`BIMS_ENEMY_WEAPON=…` in every resident's, and `BIMS_ARMOURED=1` for a
fresh helm, kevlar and leg guards on the crew member, all through the
public `Game::gear`/`Game::issue` from the app's `open` (`dev.rs`),
so a swing, a burst, a tracer in flight or the "locked in melee" tag
over a name can be read off a PNG.

## The research desk and the fusion reactor have pictures, and a key lights its desk

`fittings::research_desk` is a console on a table's footprint — two lit
screens along the far edge, the key's slot at the far right as a tall dark
well with a brass rim, a keyboard's ledge on the near side — and
`fittings::fusion_reactor` is the fission reactor's vessel writ large across
three tiles, eight field coils round a blue-white plasma, so the two read as
the same kind of machine. Both are rows in `PART_COLORS` (40 now).

`world_paint::key_lights` is what "highlighted" is on the deck: for every
research desk of a station's design, while `World::station_has_key(id)`,
a gold wash over the desk and a ring of small lights a little way out from
its footprint, pulsing together on `game.frame` (`KEY_PULSE` frames a
pulse). Drawn in `stations` after `hull::lights`, in the station's frame,
so it turns with the picture; the ship's own desk gets none — a key in it
is the container window's to show.

## Two lights, and the fog is a texture now

`fittings::wall_light` and `fittings::standing_light` (September 2026):
a bracket lamp drawn **flush against the wall it hangs from** — the top
edge of its tile unturned, which is `shipdesign::wall_light_back` of its
rotation (`crates/shipdesign/CLAUDE.md`) — and a lamp on a pole. Neither
draws a halo: what the light *does*, and what it looks like on the deck,
is `bims::sight`'s light map (`crates/game/CLAUDE.md`, "The dark"). The
crew's own semi fog is no longer in the shape buffer: `Game::light_map`
is a two-bytes-a-pixel picture (darkness and lamplight) the app draws as
one texture (`crates/app/src/fogmap.rs`), and
`world_paint::light_map_on_screen` / `Session::light_map` hand over its
four corners through the ship's camera and heading — the `on_screen` the
crew's names use — so it lands on the deck at any zoom and heading.

**A lamp's glass shows what the fight did to it.** `fittings::lamp_face`
is drawn over each light part after the hull (`world_paint::lamp_faces`,
in `paint_ship` for the ship's and in `stations` for a station's) from
`World::lamp_look(station, tile)` — the share of its health and how
bright it is shown this frame: nothing while whole and steady, a veil
the darker the dimmer while it flickers, and out (`bims::sight::Lamp`,
`crates/game/CLAUDE.md`) the glass dark with a crack across it. The
designer's lamps are always whole.

**The designer turns a wall light to its wall.** `Editor::turn_at(tile)`
is the turn the tool goes down at: the ghost's, except a wall light whose
ghost side has no wall is turned by `shipdesign::wall_light_rotation` to
one beside the tile, so a lamp dropped along a bulkhead hangs from it
without a press of `R`; `ghost_turn()` is the same at the hover, and
`ghost_ok`, the ghost painter (a bar of lamplight along the edge it would
hang from) and the designer's drag (`screens/designer.rs`, each tile's
own turn) all read it. The refusal stays the design's
(`EditError::NoWallAtBack`): a lamp with no wall on any side is red.

## The comforts have pictures, and the picture's ghost turns to its wall

`fittings::small_plant`, `big_plant` and `picture` (September 2026): the
two plants seen from above — a pot or a tub as concentric discs, the
soil, and `foliage`, a ring of round leaves in the two greens with a
light one on top, at a size each — and a framed canvas (brass round a
sky over a hill) as a strip flush against the wall its rotation names,
the wall light's top edge unturned. Three rows in `PART_COLORS` (45).
Nothing in this crate reads what a comfort *does*; that is the room's
(`crates/game/CLAUDE.md`, "Surroundings").

`Editor::turn_at` asks `shipdesign::hangs_on_wall` rather than naming the
wall light, so a picture dropped along a bulkhead hangs from it like a
lamp, and the ghost painter's edge bar is lamplight for the lamp and the
frame's brass (`fittings::FRAME_BRASS`) for the picture.
