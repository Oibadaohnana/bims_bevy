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

**Head up is the player's choice, and it is one number.** `Game::head_up`
(the View buttons and `N` in `crates/app/src/screens/game.rs`, `Game::head_up` at the
boundary) holds the ship square to the window in both views and turns the sky,
the station alongside and the map round it instead. It is **on by default**
since feature 65 — both of `Game`'s constructors set it, and the tests'
`game()` fixture in `tests.rs` turns it off again, since those are written
north up and the head-up ones say so by name. It is done without a
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
map's focus is nought, the ship, while it follows; let go, it is
`Game::map_anchor` — a **place in the system**, put back into the focus
every frame and read out of it after every pan and zoom (`Game::pan`,
`Game::zoom`, `hold_map`) — because the map's camera is measured from
the ship, and a focus left alone would have flown with it: a planet
zoomed in on slid off as the ship set out, and before that the clamp kept
the ship on the canvas so a planet could not be zoomed in on at all.
`the_camera_follows_the_crew_member_the_player_steers` pins the ship view,
including a zoom about a corner and a pan to the limit, and
`the_map_let_go_holds_a_place_in_the_system_still` the map.

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
**Somewhere the crew have already been gets a tick** at its *upper-left*
shoulder — the upper-right one is the pickaxe's and the settlement's
pad's, and under the icon is where the app writes its name
(feature 85, `paint_tick`, `TICK_SHOULDER`, `VISITED` — a pale grey,
and two strokes of `DrawList::line`, a shape nothing else on the map
draws). What has been visited is the world's, not the painter's:
`World::visited`, marked off `settle_frame` at every node the ship has
stopped at and filed with the system when it jumps out
(`crates/world/CLAUDE.md`, "The dead lie where they fell"). The galaxy
chart rings a visited *star* in the same grey (`lobby::preview`), so the
two maps agree about the colour the way they agree about the enemy's red.
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

**The bunks' tags are the same shape** (feature 61): `Session::bunk_labels`
→ `world_paint::bunk_labels`, one `BunkLabel` per bunk of the ship's off
`Game::bunk_tags` — the bunk's middle taken to the design (`Aboard::to_design`)
and through `on_screen` like a Bim, and whose it is — which
`screens/game.rs` writes across the bed with `theme::bunk_tag` every
frame, under the crew's names, the owner's name or *Unassigned*. A
station's bunks on a joined deck are not in it.
`a_bunk_s_tag_lands_on_the_bunk_and_turns_with_the_ship` pins it.

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

**The plain's fog is the same texture, a chunk at a time** (feature 67,
September 2026). On a planet the fog beyond the room's box used to be
`world_paint::plain`'s own rectangles, a run of tiles each, and looked
like the tile fog the deck had before its light map; now the room
pictures the plain the way it pictures the deck
(`bims::terrain::Plane::picture`, `crates/game/CLAUDE.md`), a
`LightMap` per 32-tile chunk of the room, and `plain` draws the ground
alone. Two things are the ship's: `world_paint::plain_window` is the
room tiles the plain is drawn over this frame — the camera's reach,
capped at `PLAIN_DRAWN` — and `ship::Game::picture_the_plain` hands it
to the room once a frame in `Session::render`, after
`hold_view_to_the_ground` and before `world_paint::paint`, since the
room does not know the camera; and `world_paint::plain_fog_on_screen`
/ `Session::plain_fog` hand the app every chunk's map with its
`FogPiece`s — the chunk less the room's box, at most four rectangles,
each a UV rectangle of the texture and four corners through
`on_screen` — so the plain's fog and the light map never lie over one
another and a picture's apron (`PICTURE_APRON`, composed for the app's
blur to read across the seam) is never drawn. The app keeps a
`fogmap::FogTexture` per chunk (`GameScreen::plain_fog`, dropped when
the room lets the chunk go) and `FogTexture::paint_pieces` draws a
list of pieces of one texture; `paint` is one whole piece.

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

## On a planet there is no space, and the planet grows under a landing

`world_paint` (September 2026, feature 52; the rules are
`crates/world/CLAUDE.md`, "A planet's surface is a settlement"). While
`World::landed()` says the ship is down, `paint_ship`'s backdrop is the
**ground** — `ground_color` by the planet's kind, `GROUND_ROCKY` or
`GROUND_ICE`, darker than the map's disc so the deck and the hull read on
it — with `ground` scattering darker patches over it at fixed places in
the camera's units (the ship does not move on the ground, so they need no
world position); no starfield and no `local_node`; a paved **pad** under
the hull's whole box in the ship's frame (`pad`, before the rocks and the
rim, `PAD_MARGIN` past the hull); and `stations` draws the settlement
alone — chained onto `World::stations`, since it is not in it, through
`World::station(id)` — and nothing in orbit, because from the ground
nothing in orbit is in the picture. The settlement is a station to every
other line of `stations`: its residents' room, its mated gate, its fog by
stance.

**The descent is the planet's disc growing under the ship.** During a
landing (`World::landing()`, the body and nought to one) `local_node`
draws the planet `LANDING_GROWTH` (40) to the power of the progress
bigger than it draws it held beside — geometric, so the growth reads the
same all the way down — and during a lift-off (`World::lifting()`) the
same shrinking; the ship's own position does the rest, since the world
slides it over the planet and down. `blackout` fades the window to black
over the last `LANDING_BLACK` of a landing and from black over the first
`LIFT_BLACK` of a lift-off, a rect over the whole list; the app holds the
black a beat after `Landed` while the ground is laid out
(`screens/game.rs`, `BLACKOUT_HOLD`). `BIMS_LANDED=1 bims simulation` is the
ground; `BIMS_LANDING=0.5` and `=0.85` are the planet come up under the
ship and the black beginning.

**A town is drawn as ground, not as a hull** (feature 54; the town
itself is `crates/world/CLAUDE.md`, "A planet's surface is a
settlement"). In `stations`, a `Plan::Surface` station gets a
`Terrain` — its biome off `World::surface(surface_body(id)).biome`,
and which of its tiles are **out of doors**: a four-neighbour flood
from the tile inside the port (`port.centre` in tiles, one step back
along `port.outward`, as `world::crew` finds it) over tiles whose
object layer is empty or holds only the wild — tree, shrub, boulder,
water, field — a standing light, sandbags or a plant
(`passable_outdoors`); a door, a wall or anything else stops it, so a
house's floor is inside and the street outside its door is not. It is
rebuilt every frame, a walk of the parts and the tiles. `hull_tiles`
takes it as `Option<&Terrain>` (`None` for the ship and every station
in orbit) and with it **skips the structure layer** — there is no frame
under a town — and draws no `hull::shadow`, since the ground goes on
past the deck's edge. The floor is `ground_floor`: **one rect a run**
of floor tiles along each row, the outdoor runs in the biome's ground
colour and the indoor runs in `FLOORBOARD`, rather than a rect a tile
(a 96-tile town is nine thousand tiles, and most of them are ground);
then a decoration on roughly one outdoor tile in `DECORATED_ONE_IN`
(6) with nothing standing on it, chosen by `tile_hash(x, y)` so it
holds still — a tuft of two or three darker strokes and now and then
a flower, a ripple in the sand or a pebble, a drift of snow. The
objects go through `hull::part`, then **`fittings::part_in(..,
Some(biome))`**, then the block, as ever.

**The backdrop is the town's floor colour.** `ground_color` takes the
biome now, not the body's kind (`SAND`, `GRASS`, `SNOW`), and returns
the very colour the outdoor floor runs are drawn in, so where the
settlement's deck ends is invisible and the wild ring is what marks
the ground's edge; `ground`'s scattered patches are the biome's darker
tone (`ground_dark`). All three are dark enough that a landed ship's
deck and the bodies read on them.

**`fittings::part_in` is biome-aware, and `part` is it with `None`.**
With a biome a `Wall` is the ground's — temperate: two courses of
stone blocks let into a mortar seam; desert: adobe with a lighter
face; arctic: timber with a plank line — and the ship's bulkhead stays
for `None`. The five wild parts have pictures in the part's own frame
through `hull::Local`, each with a shadow to its south-east: `Tree` a
round canopy of three ellipses in the two greens, a palm (a trunk disc
and six fronds) in a desert, a fir (three tiers stacked up the tile,
snow on the top one) in the arctic — the tiers are the format's
right-angled triangle turned three-eighths so its right angle is the
apex, and `turn_of` reads the part's turn back off the `Local` for
that one shape; `Shrub` a small bush, a cactus with an arm each side,
or a tussock of dry blades; `Boulder` as `world_paint::list_rock` — the
biome's rock, a seam, a lit edge — with a second smaller lump against
it; `Water` the whole tile in the biome's water so a lake is one sheet,
with two wavelets on one tile in three, or, arctic, ice with a crack;
`Field` the soil with an edge and three furrows, the far view of what
the room draws live. With `None` the temperate look. `salt` (a hash of
the origin) varies size and lean so a row of trees is not one tree,
and picks which water tiles carry a mark. `SAVE_VERSION` is 2: the
feature grew `Surface`, `Station`, the bay, the layout and the sight.

**On the map a landable planet says so three ways.** Its icon is ringed by
its settlement's stance like a station's (`World::surface`,
`World::stance`); a **landing pad** stands at its top-right shoulder
(`paint_pad`: a plate in the side's colour with an arrow coming down onto
it, `PAD_SHOULDER` out — past the stance ring *and* the reticle round a
ship docked at the planet's own station, which sits on the planet at map
scale), where a belt has its pickaxe, so a planet with ground is told
from a gas giant at a glance; and the app writes its name and `· land`
**under** the icon (`screens/game.rs`, `LAND_TAG`, `LAND_DROP`; the
ship's own `You · …` goes over, and a ship docked at the planet's station
would otherwise have the two on top of each other) in `theme::LAND` —
`FRIEND` by value — or `theme::BAD` for a hostile one. The words come off
`Session::landing_sites`, which reads `World::surface` and `World::stance`
and places each with `Game::map_spot` — the map's arithmetic read back,
shared with `pick`, so a name lands where the icon was drawn at any
heading and head-up. `BIMS_KEYS=40:M bims simulation` with a screenshot
is the picture; the simulation docks at a station in orbit of a landable
ice world, so the docked case is the first one seen.

## A save is the world as text, and the session is stood up again round it

`save.rs` (September 2026, feature 53). `encode(&session)` is the game's
`World` written as RON — compact, one value — behind four numbers: the
players, the local slot, the galaxy type's code and the spawn, which is
everything `Session` holds that a world does not. `decode` reads it back,
and `Session::restore` stands a session up round it the way `simulate_on`
does: `Editor::settled` on the ship's design, `Game::resume` round the
world — the cameras fitted, nothing aimed, no tool in hand, the sky rolled
off the world's own seed. Nothing of the window is saved; a load opens on
the whole ship like a new game.

Three things about it that are easy to get wrong:

- **The version is read off the front of the text by hand**, not through
  a struct that ignores the rest. `encode` writes `(version:N,` first, and
  `version_of` reads the digits. The obvious way — deserialize a `Head {
  version }` and let serde ignore `world` — took *fourteen minutes* on a
  save: RON tells a struct from a tuple by scanning ahead to the matching
  bracket, so ignoring a value is linear in its size and ignoring a nested
  one is quadratic. Two ship test binaries sat at 99% on it before it was
  found. Keep the version first in `Written`, and keep the read by hand.
- **What is left out is what is worked out again.** `serde(skip)` on the
  room's draw buffer, the sight's pixel caches (the light map, the light
  fields, the lamps' boxes, the views) and a surface's lazily built
  settlement, all of which `render`, `light_map` and `Surface::station`
  rebuild from what *is* saved. Those pictures were 25 MB of a 39 MB
  file. The two big flag grids that are state — the nav's blocked cells
  and the sight's explored pixels — go as a string of noughts and ones
  (`bims::math::bools`), a fifth of `true,false,`. A save is ~7 MB now
  and reads back in a quarter of a second.
- **`a_game_saved_and_read_back_is_the_same_game`** is the test, and it
  compares three things: `world.checksum()`, every crew member's position
  in the room, and the *picture* both sessions draw — as read back, and
  six hundred steps on. A cache skipped that was not rebuilt, or a field
  that should have been saved and was not, shows up there as a pixel or a
  Bim somewhere else. A new field in the world is in the save by
  deriving, so the test is what says whether it round-trips.

`SAVE_VERSION` is bumped when a saved type changes shape; an old file is
refused as `LoadError::Version` rather than read wrong, and one that is
not a save at all as `LoadError::Syntax` with the parser's words. Where
the file goes is the app's (`crates/app/src/save.rs`).

**The same text is the host's world on the wire** (feature 67). A guest
whose checksum has parted from the host's, or every guest when the host
loads a game, is handed the host's `Session::save()` whole
(`net::Packet::World`, `crates/app/src/net.rs`) and stands a session up
round it with **`Session::restore_as(text, local, w, h)`** — `restore`
with the caller's slot in place of the file's, so the guest comes up as
*its* crew member and not the host's (`restore` is `restore_as` with the
file's own `local`; both go through `stood_up`, and a slot the crew has
not got is clamped to the last as `Game::resume` clamps it). Before the
host loads with company it asks **`save::players_of(text)`** — the player
count read off the front the way `version_of` reads the version, since
`players` is written right behind it — and refuses a file saved for a
different number than are in the room. The file's `local` is still what
a load *without* company comes up as.
`the_host_s_save_read_back_as_a_guest_is_the_same_world_steered_from_its_own_slot`
is the test: a two-player game saved on one end and `restore_as` slot 1
on the other agree by checksum, as read back and six hundred steps on.

**`Session::crew_names` is the one string a session carries** (feature
60, version 9): what the players called their crew, in slot order, empty
at a slot for the app's own name. It is nothing the rules read — the
world knows crew member 1 and nothing else — and the app puts it where
the words are (`names::set_crew_names`) at every open and load; it is
here so a save keeps it. Beside it, `Session::design_point` and
`design_point_on_screen` are one player's pointer as the others see it:
a canvas point read back to a point on the ship's grid — the yard's view
before the world opens, `Game::design_point_at` after — and that point
put forward again through the ship's turn (`world_paint::design_on_screen`,
the crew's names' arithmetic), so a pointer lands on the tile it is over
whatever each window has zoomed and turned.

**`Session::crew_hair` is the players' hair** (feature 62): what each
picked on the setup tab, in slot order, put onto the crew by
`Session::dress_crew` — `Look::of(slot).with_hair(..)` through
`Game::set_look` for as many slots as have said and are players, so a
bot or a hire keeps what its index dealt it — at `start_game` and again
from the app whenever a choice arrives late (`Online::hair_said`). Not in
the save: the look is on the character and the save carries that
(version 10, `Look` a struct).
`the_hair_a_player_chose_is_on_its_crew_member_when_the_world_opens`
pins it, and that the checksum does not move for a hair.

## A class is chosen in the yard, and the deployables are painted with the room (feature 74)

`Session::crew_classes` is the players' classes in slot order, as the
hair is: `Session::set_class(slot, class)` in the design phase keeps it
— and leaves the pool alone: since feature 75 a class owns abilities and
never money, so every Bim brings `money_per_bim` whatever it is, and
`class::contribution`, `economy::starting_pool_of` and `Editor::set_pool`
are gone — and `start_game` puts them onto the world through
`World::set_class` right after `dress_crew` (`class_crew`); playing,
`set_class` does nothing and the change is `Command::SetClass`.
`Session::class_of` reads the world's while there is one.
**`SAVE_VERSION` 16**: the world's classes, progress and deployables are
in the file; **17** (feature 75) the grenade in the cargo, a Bim's brace
and rampage, the room's grenades and the world's `last_throw`; **18**
(feature 76) the world's medics — each beam's patients, the surge's
charge and the field surgery — with a Bim's beam flag and its surge;
**27** (feature 90) the kits' cooldowns and that `last_throw` as one
`World::charge_timers`, a grenade being a charge like a kit.
`a_class_chosen_in_the_yard_leaves_the_pool_and_opens_the_world_and_is_saved`
pins the pool untouched, each class's kit in the pack and the round trip.

`world_paint::deployables` draws every deployable in the crew's room as a
part stood on its room tile — `fittings::sandbags` (now `pub(crate)`) for
laid sandbags, `fittings::sentry` for a turret: three feet, a drum, the
barrel out to the right, the eye it aims with, and a dark ring closing
over the drum as its health goes — appended
turned with the room, under the room's own picture, since a station's
tile is on the joined deck's grid rather than the ship's. The eye was
dull for a sentry with no shots left; feature 88 took the shots away
(nothing in the game carries ammunition) and the eye is lit for as long
as the turret stands.

**`SAVE_VERSION` 26** (feature 88): the world's deployables lost their
`shots` and every crew member's kit cooldowns went in.

## The `crisis` command is two dials on a `test` world (feature 92)

`Session::crisis_for_probe(first_day)` is the whole of `nix run .#crisis`:
a `test` session — a random galaxy, a random dock somebody lives on, a
mercenary at it — with `World::set_crisis_first_day_for_probe(first_day)`,
the machines' origin forced onto a star `session::CRISIS_HOPS` (2) lane
hops from the crew's own (`World::start_star_hops_for_probe` finds it,
`set_droid_origin_for_probe` sets it and works the hop table out again),
and the clock wound to `first_day - 1` (`set_day_for_probe`).

Both halves earn their place. The roll's own floor is
`DROID_ORIGIN_MIN_HOPS` (eight), which is forty days of the clock before
the crisis is anywhere near the crew and one red speck on the far rim of
the chart; two hops puts the crew's own system ten days behind the first
star. And the clock opens on the **eve** of `first_day` whatever
`BIMS_CRISIS_DAY` says it is, so the first flip is a day of the clock
away rather than ten — and `set_day_for_probe` winds the **crew's
calendar** with the world's, or the strip would read Day 1 with the chart
saying day ten.

`the_crisis_is_saved_and_the_hop_table_is_worked_out_again` in `tests.rs`
is the save's half: the origin and the first day are in the file, the hop
table is not — `Game::resume` calls `World::settle_crisis` for every load,
restart and guest — so a world read back has to report the same infested
set and the same checksum. **`SAVE_VERSION` 28.**

## The `jammer` command is the crisis with the crew inside it (feature 93)

`Session::jammer_for_probe(tier, reinforce, waves)` is `nix run .#jammer`:
`crisis_for_probe(DROID_FIRST_DAY)` first — the same random galaxy, the
same random dock, the origin `CRISIS_HOPS` (2) lane hops off — and then
the clock wound **past** the day this system falls
(`World::infested_on(star_id)` plus one) rather than a day short of the
first, so the chart is red *here* and the lanes inward are shut; then
every station of the system into the machines' hands at once
(`World::infest_here_for_probe`), since the crisis's own flip waits for
the crew to be off the berth and the point of this command is the crew
**on** one, with a wave aboard and the reinforcement clock at a minute.

Two hops is also inside `world::data::DROID_TIER_THREE_HOPS`, so the wave
comes at **tier three** unless `BIMS_DROID_TIER` says otherwise — which
is the distance rule (feature 93) doing its work, not a dial.
`Session::droids` and `infest_the_dock_for_probe` take an
`Option<Tier>` for the same reason: `None` is the distance rule, and the
probes' dial is the only thing that overrides it.

**`SAVE_VERSION` 29**: `World::droid_tier` is an `Option<Tier>` holding
the override alone, and the machines' **derived jammer station** is not
in the file at all — `Game::resume`'s `settle_crisis` rolls it again off
the star's own stream (`World::settle_jammer`).
`save_round_trip_keeps_a_jammer_down_and_rolls_the_derived_one_again` in
`tests.rs` pins both: a cleared jammer stays down, and the file never
mentions the derived station's id.

## The `defense` command, and a town on the map (feature 94)

`Session::defense_for_probe(delay, reinforce, waves)` is `nix run
.#defense`: the `test_planet` run — a random galaxy, a system with
friendly ground, the ship set down at the town — with the machines' origin
forced **one** hyperlane hop off and the crisis's first day wound to
nought, so `World::front(star)` is one and the town is *threatened*. Both
clocks are cut to a minute by the command, so the first wave lands a
minute after the landing rather than an hour. It is called **after**
`land_for_probe`, since what starts an attack is the crew being on the pad.

One hop rather than `CRISIS_HOPS`' two: two is a system near the front,
and only one is a town the machines are coming for next.

`Session::landing_sites` answers a `LandingSite` now rather than a tuple —
the node, whose the town is, whether it is **threatened**, whether the
crew **held** it, and where the map draws it — so the app can write
`threatened` in the enemy's red or `held` in the accent where it would
otherwise write `land`. `Session::front_premium()` and
`Session::defending_a_town()` are the other two readings the app takes:
the first for the trade window's line about a desk near the front, the
second for the red warning over a town under attack.

**`SAVE_VERSION` 30**: `World::defenses` and `World::held_towns` are in
the file, so a fight paused by a take-off and a town held both survive a
load — `save_round_trip_keeps_a_held_town_and_an_attack_under_way` in
`tests.rs`.
