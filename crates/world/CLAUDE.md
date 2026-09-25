# The world

Notes on `crates/world` — one star system, the ship in it, the stations, the
crew aboard, the run and its two clocks. The room it steps is
`crates/game/CLAUDE.md`; the painter and the page, `crates/ship/CLAUDE.md`.
Feature 104 deleted the old game (the flown trip, the needs, radiation,
the human enemies); where a section below names one of those, it says so
in the past tense, and the section at the end is the list.

> **Since the port to Bevy (September 2026):** the browser host is gone.
> Every name table this file points at in `web/*.js` is now
> `crates/app/src/names.rs`; the pages are the screens in
> `crates/app/src/screens/`; the `bims_*` and `ship_*` exports are methods on
> `bims::game::Game` and `ship::Session`. The node harnesses
> (`scratchpad/*.mjs`, `smoke.mjs`, `stub.mjs`) are gone with the host — a
> check this file says lives in one of them lives nowhere now unless it says
> so in `crates/app`'s tests or `./check smoke`, and is worth restoring as a
> unit test if the thing it guarded moves again.

## A recipe is a row, a bench is a solid, and the hold is the world's

`shipdesign::recipes::RECIPES` is the table, and since the money rework
(feature 95) it is **one row**: two `Vegetable` into one `Medkit` at the
drug lab, a quarter of an hour. `every_recipe_holds_together` pins the
arithmetic against the resource table, which is why a medkit weighs two
vegetables — crafting conserves mass and there is no exception left, the
smelter that vented slag being gone.

**"Is this a bench" is `shipdesign::recipes::is_workstation(kind)` now**,
not `recipes::at(kind).next().is_some()`: the **workbench** is worked at
for the upgrades and the armour repair and the **armoury** is the cabinet
a crew fetch a gun out of, and neither has a recipe. `aboard.rs` builds
`Layout::benches` off it (a `room::Bench`: the part's code, its frame, its
use spot) and it is still the **only** `shipdesign` use in there. Without
it `World::workbench()` and `World::store_bench()` answer `NoWorkbench` on
a ship that plainly has both.

The chain is `Kind::Craft { recipe, bench }`, two steps, `GoToBench` and
`Work`, with the length riding on the task in `rest_minutes`, because the
room has no recipe table. **The room moves no cargo.** `Work`'s
`leave` pushes the recipe onto `Room::crafted`; `World::step` drains it
with `Game::take_crafted` after the room steps and moves the hold —
`finish_craft`, which re-checks `can_make` and emits `Crafted` or
`CraftLost`. Going in, the world hands the room `Vec<Order>` every step
(`Game::set_craft_orders`, from `World::craft_orders`: target unmet, inputs
aboard, room for the output net of the inputs, `powered(station)`, one
order per bench of that kind). `Job::Craft` is offered while any order's
bench is free — `Exclusive::Bench(index)` — and `craft_on_offer` takes the
first, so recipe order is preference order.

Targets are `World::craft_targets`, in `world_checksum`, set by
`Command::SetCraftTarget` and clamped to the class's capacity. Nought at
the start. The Management tab's crafts table is the control
(`Actions::crafts`, handed back as `Order::Keep`). `JOB_CRAFT`, `SPOT_BENCH` and
`WORK_NAMES`/`WORK_SPOTS` in `crates/app/src/crew.rs` are the app's half —
and `Job::Mine` came **out** of that list with the mining, so every job
code after it moved down one.

`PartKind::Armoury` is a bench that makes nothing and holds a great deal:
`Storage::Locker`, eighty cells, the cabinet the guns and the armour live
in. `PartKind::DrugLab` is the other, and the one whose product the room
*uses*: the medkit that gets a body out of a dying state. The hold is
still the world's — the room never sees the recipe — but the medkits to
hand are handed to the room every step and read back after it; see "The
enemy shoots back" below for that seam, and `crates/game/CLAUDE.md` for
what a medkit and a bandage do.

The playtest ship has a workbench and a drug lab aft, both `R180` so they
are worked from the row forward of them, an armoury forward by the bunk
with a second reactor to pay for it, a second shelf, five bandages, two
medkits, a suit, a piece of each armour and one of each of the four
weapons — 83 of the lockers' 360 cells, the shelf being locker class too
since the money rework. `benches()` is three there: the workbench, the
drug lab and the armoury.

## The outside is a place, and it is a walk to a site and nothing else

A walk outside is `Kind::Eva`, from `GoToSuitLocker` to `PutSuitBack`.
There **was** a reason of its own to go: a belt held at laid a
`MiningSite` out in the ship's design frame, rocks were marked with
`Command::MarkRock`, and a suited Bim walked the outside on a grid of its
own and dug ore and galvum out of the asteroids' cores. The money rework
(feature 95) took the whole of it away with the materials it fed:
`crates/world/src/mining.rs`, `MiningSite`, `World::sites`,
`World::site_here`, `World::eva_offer`, `bims::game::Eva`, the marks and
their commands, `site_version`, `Job::Mine`, `WorldEvent::Mined`, the
asteroid painter and the mining UI are all gone.

What is left is the **run of steps**, because a construction site beyond
the hull is still reached through the airlock: `Kind::fork` turns off at
`StepOut` and back at `WalkToPort`, so a build outside is the suit, the
gangway, the walk on `Nav::outside`, the work, and the suit hung up
again. `Room::rocks_version` is kept and moves for nothing: it is what
`Game::refresh_outside` reads, and a planet's ground or another reason to
rebuild the outside grid would set it.

Who may go out is `World::suit_ok`: everybody, while there is a suit
aboard to wear. There is no dose any more — the radiation, stage 8 and
`World::health` went in feature 104 (see the section at the end) — so a
suit is the whole of the check, the same answer it always gave, since
nothing ever dosed a body in a run. `World::hold_at_belt_for_probe` is
still there — undock, put the ship at
the spawn system's first belt, settle the frame — because a few tests
want the ship holding somewhere that is not a berth; it lays nothing out.

## The design phase, then the game, and one clock in it

The yard (`bims design`) is the one way into the design phase now — the
`game` flow has had none since feature 102, `screens::designer::start_run`
standing `Session::run` up on the default ship instead. There the last
Accept settles the ship **and** opens the world — one event, in
`Session::accept`, because two calls for it would be two things that
could disagree about which ship got handed over. `Session::designing` and
`Session::playing` are how a caller asks which half it is in.

After that there is exactly one loop: `World::step`, which advances the
**mission clock** by `STEP_MINUTES` and nothing else. Its order is the
contract and it is written out in the function:

0. between missions (feature 103), the commands and nothing else — the
   map is up and nothing moves;
1. the commands stamped for this step, in arrival order;
2. the clocks — the mission clock, and never the world clock, which only
   travel moves (`mission.rs`);
3. the ship, which does not move — nothing is flown since feature 104,
   and where it is, is what is in range;
4. discovery and the local frame, the station's room opened or closed,
   the crisis spread and the machines' waves laid (features 83, 92, 94);
5. **crew** — the room aboard and the station's, stepped, with everything
   the classes, the medicine and the benches hand them before and read
   back after;
6. **power** — the reactors against the wired consumers, into the
   batteries — with the brownout and the research after it;
7. **construction** — what the crew did at the sites, moved through the
   hold, and the workbench's carries and the kits laid;
8. **the run** — the bounty paid the step a site is cleared, and the
   departure check (feature 103).

Stage 8 was health and radiation until feature 104 took the dose away;
the run is stage 8 now. Whatever joins the step goes in at one of these
places, on *this* clock. A second clock or a second loop is two
simulations that will disagree, and the failure reads as a ship in two
places.

`crates/app/src/screens/game.rs` turns real time into steps with an
accumulator — `dt * ship_steps_per_second() * multiplier` — and never
into bigger steps. Same rule as the room, same reason: every timer in a
mission — a wave, a cooldown, a bandage — counts whole steps, and a
bigger step would be a different fight rather than a faster one.
`MAX_STEPS_PER_FRAME` has to stay at or above `TOP_SPEED * 60 / 30` or
the top of the range quietly stops being reachable.

## The anchor is stored and the position is derived

`Ship::anchor` is where design tile (0, 0) sits in the system;
`Ship::position()` is the **centre of mass**, worked out from the anchor and
the heading through `flight::angle::rotate_design`.

That way round on purpose. `on_ship_changed` has to promise that welding a
shelf to the stern does not move the *hull*: if the position were stored and
the anchor derived, every wall anybody built would shove the whole ship
sideways through space. Nothing flies the ship any more (feature 104):
it is *put* where it goes — at a berth, off a site between missions,
inside the next system after a jump — and `set_position` does each.

`rotate_design` is **its own inverse** — it is a rotation composed with the
flip between the grid's y-down and the system's y-up, and a rotation composed
with a reflection is a reflection. `unrotate_design` exists so call sites read
the way they mean and calls straight through. Do not write a second one.

## A speed request is the one command that does not wait for a step

Everything a player asks for is queued and applied at the top of the next step
— which is what makes the stamp `crates/app/src/screens/game.rs` puts on it mean anything.
**Except the speed.** At a pause no steps are taken at all, so a queued speed
change would never be applied and the pause could never be lifted.
`World::request_speed` is the door, `Command::SetSpeed` goes through the same
door, and it is safe to be the exception because it changes nothing a step
would have changed — only how fast the caller is expected to turn the crank.

## A station is a place, and the ship docks beside it

`crates/world/src/station.rs` turns every `StationBlueprint` of the system
into a `ShipDesign` through `apply` — the same parts, the same rules — on
one of six **plans** (`station::Plan`, rolled off `map_seed` on a stream
of its own; the section "A station is one of six plans" below), sized
by plan and kind (34 to 72 tiles) and dressed by `map_seed`, with the port
in the west skin and the array in the north. `World::stations` holds them
from `World::start`; `Station::all_of` is the only place they are built
(`Station::build_as` for a plan of your own, `Station::replan` to lay one
out again where it stands), and `station::layout` **caches by (kind,
plan, seed)** because a layout is two thousand `apply`s and the world
test suite went from two seconds to a minute before it did.

Five things that hang off that:

- **Docking is airlock to airlock, outside the hull.** `shipdesign::dock::port`
  is a design's first airlock and the side of it with nothing beyond, and
  `Station::berth` turns the ship so its port faces the station's and puts
  the two outer faces on one point. `World::dock_at` is the one place the
  ship is set down at a station (with `World::start`), at
  `World::berth_at`, so a trip ends outside the station rather than at
  its middle. `the_ship_docks_airlock_to_airlock_outside_the_station` checks
  every ship tile is clear of the station's frame.
- **An airlock in the deck is a door to nowhere.** `Dynamics::has_airlock`
  is now "has a port", and `IssueCode::AirlockSealedIn = 31` warns about an
  airlock with hull on every side. The flyer fixture's airlock moved from
  `(16, 8)` on the deck into the starboard skin at `(18, 11)` for exactly
  this, and so did the harness builds in `flyer.mjs` and `ship-check.mjs`.
  A ship without a port gets a berth anyway — held off the door by its own
  size, heading north — because a world opens docked whether or not the
  ship can go aboard.
- **A station's room opens within fifty tiles and closes beyond.**
  `World::residents` is one `crew::Residents` — the room again, `Aboard::new`
  on the station's design, seeded by `map_seed`, its clock wound on to the
  world's with `Game::wind_clock` — for the nearest station within
  `RESIDENTS_RANGE` of its *hull* (`Station::clearance`), kept out to
  `LOCAL_HYSTERESIS` further, dropped past that. **Every** station gets one,
  a derelict's with nobody in it (`residents_of` is 0), because the room is
  what has the pictures of the fixtures — without it a station is coloured
  blocks. `Game::with_layout` therefore allows an **empty** crew now; the
  ship's room never is (players are `max(1)`), and `Game::simulate` guards
  its tie-break remainder. Dropped means the *room* is forgotten: come
  back and they start at their bunks — the living ones; the dead are
  counted as the room closes and stay dead (`World::close_residents`,
  `World::losses`; "A system has a memory" below). It is in
  `world_checksum` after the crew.
- **The spawn is the first orbital in a system with a belt whose
  blueprint was not rolled hostile**, not the first station: `World::spawn`
  skips derelicts, because a crew that opens docked at a wreck sees nobody
  and blocks, and skips a system with no belt, which was the mining site
  and is now only what keeps the simulation on the dock every pinned
  number stands on. It still skips a blueprint rolled hostile
  (`StationBlueprint::hostile`) though no human has been an enemy since
  feature 104 — the roll picks the dock every command opens at, and
  dropping the condition would move the spawn and re-pin the lot for
  nothing (`spawn_anywhere` and `spawn_with_ground` skip them the same
  way). An orbital rather than any lived-on kind because it is the
  ordinary case, and because the fixture tests lean on what one has:
  four bunks and a shelf with the staples. It was "the first lived-on
  station" until the generator went to 6 and that landed on a three-bunk
  refinery in a system with no belt.
  `the_spawn_is_the_first_lived_in_dock_in_the_galaxy` states the rule;
  `residents_are_there_within_fifty_tiles_and_not_beyond` reads
  `world.home` rather than the first lived-on station, since the spawn
  system has another orbital, rolled hostile, which was an enemy's with a
  garrison until feature 104.
  `the_local_frame_has_a_hysteresis_and_changing_it_touches_neither_the_clock_nor_the_speed`
  drifts *away from the nearest other node* rather than along `+x` for
  the same reason — in the new spawn system `+x` walked into the parent
  body's frame.
- **The airlock has a collar.** `dock::PROTRUSION` is half a tile, the
  `Port::face` is the end of the collar, and `hull::airlock` draws it that
  long out of the open side — so two docked airlocks meet collar to collar,
  hulls a tile apart, with the doors parted and the deck showing through.
  The picture and the berth read the same constant; move one and you move
  both. Airlocks stay 1×2 — two tiles along the skin, one deep.
- **Three pictures by distance**, all in `world_paint::stations`: out to
  `STATION_VISIBLE` a plate with the icon on it, inside `LOCAL_RADIUS_STATION`
  the hull tile by tile with parts as their colours, and while the room is
  open the room's own pictures with the residents walking between them. A
  station is never turned — heading nought — so its picture goes through
  `camera_turn` alone, via `DrawList::append_turned_at`. `local_node` draws
  bodies only now; the ring a docked ship used to sit inside is gone. The
  mated airlocks are drawn **open** (`hull::part`'s `mated`) and they *are*
  a way through — see the next section.

`ship-layout.mjs docked | residents | approach | crossing` are the pictures.

## Docked, the ship and the station are one room

`crates/world/src/docking.rs` lays the ship's design and the station's
down again as **one** `ShipDesign` — the ship first, at its own coordinates
plus a whole-tile shift; the station turned into the ship's frame by the
quarter turns the berth put between them; and the one tile between the two
hulls, where the collars meet, decked — and `World::dock_at` opens the
room on that (`Aboard::joined`). One deck, one nav grid, so a right-click
on the station's deck walks James through the airlocks. **The station's
people are not in it.** They keep their own room — `World::residents`,
opened on the station's design with the station's own fixtures — and go
on living there while the ship is tied up, each keeping to the round of
its role (`bims::routine`, dealt by `Residents::deal_roles`, feature 102;
"A run" below). The crew's Management tab reaches nobody ashore — it is
the crew's, and one day another player's crew on the same ship.
`unjoin_rooms` takes the deck apart again (`Aboard::unjoined`) when the
ship leaves the site at a mission's end (`World::leave_mission`, which
closes the residents' room straight after) or a test calls
`undock_for_probe`: the crew back into a room of the ship alone.
`every_role_has_a_round_on_a_sample_of_stations_and_towns` and
`two_runs_on_one_seed_walk_the_same_rounds` (`tests_run.rs`) pin the
rounds. Until feature 104 they cooked, ate and slept there on their own
timetable under their own manager; the needs went, and the round is
what is left.

What that rests on, and what will bite:

- **`Aboard` has an `offset`, a `crew` count and a `station_frame`.**
  `offset` is where the ship's origin sits in the room's grid (nought
  alone); `position(who)` is in the **ship's** frame whatever the room, so
  the checksum and the crew names do not care. The painter adds the offset
  to the room's centre (`room_centre` in `paint_ship`) and
  `Session::room_point`/`_y` add it to the pointer — those two are the only
  places it is added. `crew` is how many of the room's Bims are the ship's
  — all of them now, kept as a count for a second crew one day.
  `station_frame` is where a station point lands on the joined deck, for
  `Aboard::visit`: every step the residents' positions are put on the deck
  as *visitors* (`Game::set_visitors`), bodies the joined room's doors open
  for and nothing else reads, since the joined room draws every door —
  the station's own room draws none while docked — and a resident walking
  through a shut-looking door would be a door lying.
- **The room's berths and seats are `Vec`s**, as many as the layout has
  bunks and chairs, ship's first. A station's room is cut to its bunks by
  `Residents::open` (the room itself takes what it is given; see the
  arena, below), and a room may have **none** (a derelict's). Nobody
  sleeps in a bunk since the needs went in feature 104, but a bunk's frame
  is still a solid, a shelter (`nearest_shelter`) and a place a round
  stops at, and `solids()` lists the beds where the classic room always
  did — between the table and the bay — because the push-out walks solids
  in order and a different order re-rolls every probe seed.
- **Moving a Bim between rooms drops its errand.** `Game::take_crew`
  abandons every chain (`Task::abandon`: what was carried goes back in the
  store) and `Game::adopt` stands each one where they were, snapped to the
  nearest free nav cell — a spot beside a bunk sits on the edge of the
  bunk's inflated footprint and reads free or not by how the grid happened
  to fall — or at their bunk if that is more than a body's margin away.
  `take_crew` takes `&mut self` and leaves the old room standing for a
  reason: giving up an errand is what puts a medkit in somebody's hands
  back into the *old* room's store, so `join_rooms` and `unjoin_rooms`
  take the crew out first and then `bank_medicine` off that room before
  it is dropped. Before that, a kit in hand at the moment of docking was
  lost.
- **One set of fixtures, the ship's.** The joined room maps the first of
  each fixture kind by id — the ship's — and `Aboard::leave_the_station_s`
  drops every further fixture standing in the station's box from
  `Layout::more` and `extras`, so the station's galley, heads, bays and
  lockers are furniture to walk round on the joined deck and never a
  chain's pick. The residents' own room, with them in it, is what draws
  the station's fixtures and the residents; the joined room draws the
  ship's fixtures, the crew and every door, over it.
- **The crew panels rebuild when the room's crew changes**
  (`CrewPanels::rebuild_crew` in `crates/app/src/crew.rs`, from the game
  screen when the count moves). With the residents in their own room that
  is a second crew one day, not a docking; the residents are named over
  their heads on the canvas (`resident_name`) and have no panels.
- **Joining costs ~3000 `apply`s** — a second in a debug test, tens of
  milliseconds in wasm — once per dock. Not cached: the ship's design
  changes.
- **A ship docked side on has the station turned**, so the station's
  fixtures are used from the wrong side (see "Every fixture is used from
  the south"). The playtest ship's airlock is to starboard and every
  station's port is in its west skin, so that ship docks at heading 0.
  `a_station_is_turned_into_the_frame_of_a_ship_docked_side_on` covers the
  arithmetic with a bow airlock; the `debug_assert` in `docking::join`
  checks every turned part's tiles against `covered`.

`docked_the_ship_and_the_station_are_one_room_and_the_crew_can_cross` walks
James over and back; `simulation-check.mjs` does it from a right-click.

## The residents' room holds the ship, the other way round

Since September 2026 the station's people walk the same two hulls as the
crew, but in **their** frame: `docking::join_mirror` is `join` with the
roles swapped — `join_frames(first, second, to_first)` is the one
function, the station laid first at its own coordinates plus a shift and
the ship turned in through `Station::from_system` and the berth, the
passage decked beyond the *station's* port — and `Residents::join`
(called from `join_rooms`, and from the reopen path while docked)
rebuilds the residents' room on it through `Aboard::mirrored`: everybody
out of the old room with `take_crew` (errands dropped, as the crew's are
at a dock), adopted at the shift, what the old room held that is not a
body's carried over (`replace_room`: its medicine, its fog, whether its
doors are drawn). The `Joined` a mirror returns reads with
the names swapped: `ship_at` is the station's shift and `station_*` the
ship's frame; the residents' `Aboard` keeps that as its `station_frame`
and the ship's box as its `station_box`, so `leave_the_station_s` drops
the ship's fixtures from their room as it drops the station's from the
crew's. That is what lets a body of the station's room follow a crew
member through the passage onto the ship — a machine of a held station,
since feature 104; the test of it was a human garrison's
(`an_enemy_follows_the_crew_member_it_saw_onto_the_ship_…`) and went
with the human enemies.
`Residents::unjoin` at `unjoin_rooms` builds the station's own room
again, adopting back at `-offset`: one still on the ship has no floor
under it and goes to its bunk.

Three things about the offset, which was always nought for a residents'
room before and is not now:

- **`Aboard::position`/`exposed` subtract it**, so everything that reads
  a resident's place in station design units — `visit`, `body_position`,
  the hire, `resident_on_screen` — is right as it was.
- **What goes *into* their room adds it**: `visit` hands the residents
  their targets at `crew_ashore() + offset` and takes their shots' `from`
  and `at` back through `- offset` before `station_frame`
  (`Aboard::to_room`/`to_design` are the two helpers, and the tests'
  `put_for_probe` on a resident go through `to_room`).
- **The painter turns their picture about `middle + offset`**
  (`world_paint`), since the station's middle is that far into the room.

`crew_ashore` no longer hides a crew member on the ship's deck: the
residents' grid reaches it now, and what keeps them from a war with
nobody is the last-seen rule instead ("The enemy shoots back" below).

## A station has a stance, and the fight crosses between two rooms

`World::stance(id)` is Hostile for a station the machines hold
(`is_droid_held`), Friendly for `home` — the spawn station, while the
ship is at the home star — and Neutral for the rest. **No human is ever
the crew's enemy** (feature 104): whatever the generator rolled a
station's people, they are a friend's at home and a stranger's anywhere
else. The roll itself stays — `StationBlueprint::hostile`, carried as
`Station::hostile` — because three things still read it: `spawn`,
`spawn_anywhere` and `spawn_with_ground` skip a blueprint rolled hostile
the way they skip a derelict, which is what picks the dock every command
opens at (`the_galaxy_has_hostile_stations_and_the_spawn_is_never_one`);
`station::key_tier` puts the tier-two research keys on it; and the
machines' derived jammer is built with it set. Nothing reads it as a
stance. The world's own `hostile` list, `rolled_hostile` and
`set_hostile` went with the human enemies, and `home` is in
`world_checksum` after the crew; the one stance that changes is a
station taken by the machines (`World::infest`), and
`the_checksum_notices_a_stance_change` (a block of the checksum test in
`tests.rs`) says so. `apply_stances` — at every `join_rooms`, every
`settle_residents` open, every reopen (`reopen_residents`,
`mercenary_for_probe`) and every `infest` — tells the rooms: the
residents' room its own stance (`Game::set_stance`, its fog under
`Fog::All`) and whether its people are enemies (`set_hostile_bodies`,
which is the ring under each *and* the switch that makes them fight —
see the next section, and true only for a held station, whose Bims are
at most its dead), and the joined deck the station's box and stance
(`Game::set_foreign`, the black-and-grey fog over the station's half).
The painter asks it too: a stranger's station is its far plate
(`HULL_UNKNOWN`) until its room is open, so approaching one reveals
nothing at fifty tiles, and the map rings every station by it
(`world_paint::paint_map`: the enemy's red for a held station, blue for
any other somebody lives on, nothing for a derelict;
`crates/ship/CLAUDE.md`).

**A station's room opens with its survivors.** A room is opened with
`World::people_of(station)`, never `residents()` straight: the people
who live there **less its dead** (`World::losses`), nobody on a
derelict, and nobody at a station the machines hold — what that room
is opened with is a wave (`settle_droids`). The mercenaries for hire are
`World::mercenaries_of` on top. Both are asked when the room opens
(`join_rooms`, `settle_residents`), so a station keeps the crowd it was
reached with until the ship has left and come back. Until feature 104 a
hostile station opened with a **garrison** instead — `station::enemies_of`,
a base by the calendar plus one a crewmate, doubled with the crew's
worth and capped at sixteen, with the `combat` arena's `reinforcements`
on top — and the whole formula went with the human enemies.
`data::ENEMIES_DAYS` (thirty) and `World::days_gone` stay: the machines'
wave counts a step every thirty days the world has run
(`droid::day_steps`, "The machines hold a station" below), and
`days_gone` is `clock_minutes` floored to whole days rather than the
crew's calendar `World::day()`, so two clients that have travelled the
same trips count the same day.
**A station's room is cut to its bunks, and the cut is
`Residents::open`'s** (since September 2026): people and mercenaries
past the station's bunks — an orbital has four — are not opened, the
mercenaries cut first. The room itself no longer caps —
`Game::with_layout` and `Game::adopt` take every Bim they are given,
since the ship's crew is the world's count (`crew_down`,
`Ship::crew_count` are sized by it) and a room that quietly held fewer
was a world at odds with itself; `berth` and `seat` clamp, so a crew
past the bunks shares the last, and `bims::aboard::starts` stands them
on clear deck tiles of their own to begin with. That is what lets
`Session::combat` put sixteen on a ship with five bunks. That is also
what the arena is for.

**The fight's dock is the arena.** `station::arena(kind, seed)` is
`build_layout` at `data::ARENA_SIDE` (72) with the quarters' bunks in
`ARENA_BUNK_COLUMNS` (4) columns three tiles apart, and
`World::arena_dock_for_probe` rebuilds the docked station as that,
standing where it stood (the anchor recomputed from the old centre,
since the build area grew), and docks again from scratch —
`undock_for_probe`, the residents' room dropped, `dock_at` — so the
joined deck and the residents' room are laid out on the new design. The
crew are sixteen (`shipdesign::fixture::COMBAT_CREW`) on the combat ship
through `World::start_with_crew(design, money, players, crew, ..)` —
`start` is that with `crew = players` — which sizes the room,
`crew_down`, `crew_locked` and `Ship::crew_count` by `crew` and
`speed_requests` by `players`, so the crew nobody steers do not hold the
speed at 1x (`speed::effective` is the *slowest* request).
`Session::combat` in `crates/ship` does the whole thing and issues
`WeaponKind::ALL` down the crew and round again, one each; it is no
command of its own since feature 102, only what `Session::droids` builds
on before it hands the arena to the machines.
`the_arena_is_the_combat_dock_and_it_and_the_combat_ship_can_be_walked`
(the walkability contract again, by `Nav::can_reach` rather than a
search per tile) is the test. **`tier2_test` and `tier3_test` are that
fight with everybody's kit at one tier** (feature 70):
`Session::droids_at_tier` is `Session::droids` with the machines at the
tier and then `World::outfit_for_probe(tier)`, which puts every crew
member's gun at the tier (its kind kept) and a fresh helm, kevlar and leg
guards at it on — pieces of the world's, `next_piece` ids at
`Where::Worn`, so `mirror_pieces` finds them. It dressed the residents'
garrison too until feature 104; a held station has nobody in it to
dress. `the_tier_tests_are_the_fight_with_everybody_s_kit_at_that_tier`
in `crates/ship` pins it.

`World::visit` is where the fight crosses. While the station is held
(Hostile) — or is a town the crew are defending (feature 94) — the
residents' room's bodies go to the joined room as **targets**
(`Aboard::hostiles` for its Bims, `None` for one that is down — each at
its exposed position, `Aboard::exposed`, and paired with its weapon kind
— and the machines beside them; "Every place that assumed a hostile body
is a `Bim`" below) beside the visitors the doors read, and the hits the
joined room's bolts landed and the blows its fists and blades struck
(`take_hits`, a `combat::Hit` — who, which part, how hard, and whether it
is a cut) are delivered to the residents' room one by one
(`Game::strike(who, part, damage, cut)`, or `strike_droid` past the
room's Bims). At a friendly or neutral station both target lists are
empty and nobody shoots. The residents' room is a step behind on `seen`,
as before; `SEEN_FOR` in the room is what keeps one drawn for two
seconds after the crew lost sight of it.
`a_stranger_s_deck_is_black_where_nobody_has_looked_and_grey_where_they_have`
and `a_recruited_bim_shoots_the_machines_it_can_see_and_they_are_hurt`
pin both halves; `World::stage_droid_fight_for_probe(kind, weapon)`
(feature 104, in place of the human `stage_fight_for_probe` and
`BIMS_FIGHT`) infests the station the ship is tied to, replaces its
first wave with **one** machine of that kind four tiles down the
corridor from the port, armed with `weapon` or, with `None`, posing —
held where it is put, firing nothing — and puts crew member 0 inside the
door, recruited. `Game::droid_mut_for_probe` is the room's half, for a
test to mend, hold or arm the machine.

## The enemy shoots back, and a bolt flies in one room only

The other half of `visit`. While the station is hostile — which since
feature 104 is only a station the machines hold, whose Bims are at most
its dead — the residents' room is handed the crew as **its** targets
every step — `Aboard::crew_ashore`, the crew's positions in the
station's own units through `Aboard::to_station` (the inverse of
`station_frame`: two projections onto its unit axes), `None` for one
dead, **out cold** — a body down is nobody's target, on either side:
`visit`'s `alive` for the residents asks `is_unconscious` too — or
outside in a suit — **on the ship's own deck too, since September 2026**
(see "The residents' room holds the ship" above), where it used to be
`None` outside `station_box` — each paired with **what it carries**
(`Game::weapon`, the pistol if somehow nothing), since the room reads
the weapon to know which targets lock a gunner in a melee
(`bims::combat`) — and that is what puts its bodies **at war**: its
machines, and — the room's combat code being kept as it was, though no
room in a run has any since feature 104 — hostile Bims
(`bims::game::tick_combat`: hostile bodies and a target that is `Some`:
recruited, armed, and walking to wherever `Tactics::stand` says — or
charging, with a blade). **They know only what they have seen**: the
room keeps a last-seen belief a target (`Game::set_hostiles` in a
hostile room, with the machines' eyes among its own;
`crates/game/CLAUDE.md`), walks to where it last saw one and gives the
hunt up after `FORGET_AFTER`, so a crew that docks at a held station and
stays aboard unseen leaves it at peace rather than at war with nobody.
**But the airlock is watched** (since September 2026):
`Residents::join` hands the fresh room the tile just inside the
station's own door — `station.port()`, a tile back from `outward`,
through `to_room` — as `Game::set_watched`, and a crew member within
`bims::game::AIRLOCK_WATCH` (2.5) tiles of it is seen with nobody
looking, so a boarding is noticed at the door and is what starts the
fight; the ship's own airlock is beyond the watch, so a crew still
aboard is not. `unjoin`'s fresh room has none. The world's test of it
boarded a human station with every resident dead and went with the
human enemies in feature 104; the room's
`a_hostile_room_s_airlock_is_watched_and_a_boarder_at_it_is_seen_by_nobody_in_particular`
pins the rule. What they need line of sight for is the shot. A room
whose bodies are hostile does not fly bolts — it records
`combat::Shot`s, and `visit` reads them back (`take_shots`), puts `from`
and `at` through `station_frame` onto the joined deck and fires each
there as a hostile bolt (`Game::enemy_fire`). So every bolt of the
fight, blue or red, flies in the **crew's** room, which is the one room
both sides' bodies are in; a red bolt looks for the joined room's own
bodies and the wound goes on at once, in the room (`Game::strike`, off
`Combat::wounds_taken`), its damage the weapon's at the distance the
bolt flew — the same fall-off for both sides, since both sides' bolts
fly in the one room. The world only *says* so: `casualties`, right
after `visit`, drains `take_wounds_taken` into `WorldEvent::CrewHit {
who, part }` (code 32, value `who + 10 * part`), `take_traumas` into
`WorldEvent::CrewDying { who, trauma }` (42, `who + 10 * trauma`) and
`take_treated` into `CrewTreated { who, trauma }` (43, the same) — a
part shot to nothing is a **dying state** now, not a death
(`crates/game/CLAUDE.md`, "A part at nothing is a dying state") — and
says `WorldEvent::CrewDown { who }` (code 33) the step a crew member is
**dead** — once, off `World::crew_down`, whatever did it: bled out
through a wound nobody dressed or a trauma nobody treated. `EnemyDown`
is said the same way, off `Residents::down`, for a hostile room's
*Bim*, and means dead too — which no room has had since feature 104, so
it is never said now; a machine destroyed is `DroidDown`.

**The positions handed over are the exposed ones.** A Bim aiming from
the peek beside a wall leans out to it, and that is where a shot at it
is aimed: `crew_ashore` reads `Game::exposed_at` — the peek while it
peeks, else where it stands — and `Aboard::exposed(who)` is the same for
a resident; and each room is told which of its targets are peeking, by
a **second call** right after the targets (`Aboard::crew_peeking` →
`Game::set_hostiles_peeking`, and `room.peek(who).is_some()` the other
way), because a bolt reaching a body peeking from cover is dodged half
the time (`bims::combat::DODGE_IN_COVER`) and the room has to know which.
`the_crew_are_handed_over_at_the_peek_while_peeking` stands James in a
doorway found by scanning the deck — so that he actually peeks — with a
posing machine held down the corridor, and pins
`crew_ashore`/`crew_peeking` against `exposed_at`/`peek`.

**A blow is carried, not flown.** A body of the held room in a melee —
a claw or a schword within reach of a crew member, or a crew member's
blade within reach of it — records a melee `Shot` (`shot.melee`, with
`damage` and `cut` carried, since a fist's damage is not the gun's in
the hand) and nothing flies: `visit` finds the crew member it was aimed
at as the handed-over position nearest `shot.at` (a `Shot` carries no
target index) and delivers it with `Game::enemy_strike(on_deck(shot.from),
who, shot.damage, shot.cut)`, which re-checks the reach in the receiving
room — the lock was read a step ago in the other room, and a body walks
— rolls the part there, applies the wound and records it for `CrewHit`.
The crew's own blows go the other way as ordinary `Hit`s off
`take_hits`. `melee_locks`, after `casualties`, says
`WorldEvent::Locked { who }` (code 37, value `who`) the step
`Game::is_locked(who)` turns `Some`, once, off `World::crew_locked`
beside `crew_down`; walking out of reach breaks it and the next lock is
said again. The app reads the state, not the event, for its header.

**What a station's people carry is rolled off the seed.**
`Residents::open` issues every resident `Gear::issued_for(map_seed ^
who)` — the `seed` its callers already pass is `station.map_seed` —
half of them the pistol, a fifth a shotgun, a few a rifle, fewer a
sniper rifle, one in ten a schword; a function of what `world_checksum`
already holds, so nothing new is hashed and `REFERENCE_CHECKSUM` did not
move for it (it moved for the playtest cargo — the four weapons B3 put
aboard — to `0x_4fc1_5639_a265_137d`, and again for `reinforcements`, the hired hands and the trading desk in every station, to `0x_288f_814e_d375_027d`; and for the hub plan and the bigger stations, to `0x_3b1f_707d_8489_5c09`; and for the hub-and-arms plan, to `0x_80a6_d0b3_b7d9_5b08`).
Nobody turns those guns on the crew since feature 104; a town's guard
and its mercenaries turn them on the machines when the town is
defended. `a_station_s_people_are_armed_off_its_seed` pins it.

Things that bit or would:

- **The order inside `visit` matters.** The residents' targets are set
  *before* their shots are read, so a room that has just gone to war
  has something to aim at from its first step; and the targets are
  cleared — on both rooms — the moment the station is not hostile, and
  on the residents' room whenever the rooms are not joined, or a room
  left at war with nobody there keeps its bodies under arms after the
  ship has gone. Peeking is set *after* the targets, every time, since
  `set_hostiles` resets it.
- **The order of the two rooms' steps matters too, and it is against
  the crew.** `World::step` steps the crew's room, then the residents',
  then `visit`. Each room's `melee_with` reads the other's positions as
  handed over *last* step, so the step a charging blade arrives the crew
  member still sees it a tile off and does not lock, while the blade —
  stepped after — sees the crew member in reach and its first blow
  lands that same step through `visit`; the crew member's lock, and
  `Locked`, come a step later. That is why
  `a_claw_charges_and_locks_the_crew_member_who_fights_with_its_fists`
  puts the kevlar on James first, and why every melee's first blow is
  unanswered. Not fixed here; noted for whoever reorders the step.
- **A sniper walks to the open at its range and fires from there.**
  It used to stand in the doorway at the corridor's crossing and never
  fire: `Tactics::stand` scored the doorway as cover with its door shut,
  and the body's own arrival opened the door — see "A doorway is never a
  stand" in `crates/game/CLAUDE.md`. `plan_stand` now hands the tactics
  the room's doorways and none is a stand, so a rifle walks the corridor
  out of the pistol's reach and lands its shot from there.
  `a_sniper_rifle_reaches_from_twenty_tiles` (a block of
  `a_sniper_rifle_reaches_from_twenty_tiles_and_a_shotgun_does_more_at_three_than_at_nine`)
  pins that with a Warden carrying the rifle — its hit said as a
  `CrewHit` from a stand past the pistol's twelve tiles, James patched up
  every step since an enemy shoots on the move too — before James's own
  long shot at a Warden held twenty-odd tiles down the corridor.
- **Two against one is not a fight the crew member wins**, so a test
  of the seam is one against one: `stage_droid_fight_for_probe` lays a
  single machine, and `Game::patch_up_for_probe` makes James good as new
  before every step of a run that has to end with the *other* body down
  — a one-in-twenty head shot would otherwise decide it.
  `a_recruited_bim_shoots_the_machines_it_can_see_and_they_are_hurt` and
  `the_machines_shoot_back_and_a_crew_member_hit_bleeds` pin the two
  sides. `the_crew_s_shotgun_does_more_at_three_tiles_than_at_nine` pins
  the fall-off through the seam off what comes off a Trooper's chassis
  at three tiles and at nine, with both bodies `put_for_probe` each step
  so nobody walks off the mark. These were human fights, staged by
  `stage_fight_for_probe` and `one_on_one`, until feature 104 ported
  them to machines.
- **The medicine is handed to the room every step, and read back after
  it.** Stage 5 tells the room each crew member's medkits
  (`hand_the_room_the_hold_s_medicine`: `set_pack_kits` off
  `charges_of(who, Charge::Medkit)`, the shelf at nought) before the room
  steps, and `bank_medicine` reads what was used after it; a dressing is
  spent out of the pack by the room itself and no count of those
  crosses. See "The medicine is everybody's charges" below. The
  residents' room deals `data::RESIDENT_BANDAGES` into **each of its
  living people's packs** and gets `RESIDENT_MEDKITS` on its shelf at
  open, with no hold behind either — what its people spend on each other
  of their own accord now that the room has a Medical job
  (`Game::medical_on_offer`, `crates/game/CLAUDE.md`), but only once a
  fight is over: a recruited Bim takes no errand of its own. A bandage
  is **bought** since the money rework, not made; the one recipe left is
  the medkit at the drug lab.

## A piece of armour is a resource in the hold and an instance everywhere else

`crates/world/src/armour.rs`. `ResourceId::Helm = 6`, `Kevlar = 7`,
`LegGuard = 8` are locker class like a medkit, **bought** at any place
with the armour trade since the money rework (feature 95) and made
nowhere, and the playtest ship carries one each
— so buying, selling, mass and the shelves needed nothing new.
Beside the count the world keeps `World::pieces: Vec<Piece>` — `id`
(`next_piece`, only climbs), `kind: bims::combat::ArmourKind`, `health`,
`at: Where::Hold | Pack { who, cell } | Worn { who }` — because a piece
has a health it keeps wherever it goes and a count cannot say that.
**The invariant: the hold's count of each armour resource is the number
of pieces `at == Hold` of that kind.** `settle_pieces` holds it, and it
runs inside `on_ship_changed` — the one door every cargo change goes
through — so a count that grew (a bench, a purchase, a test poking
`cargo[]`) gets whole pieces pushed and one that shrank (a sale) loses
its most damaged first, which *is* the sell rule. `pieces_agree_with_the_hold`
pins it. A piece in a pack or on a body is the room's (`Gear`, since the
room's `Health` reads it) and `mirror_pieces` reads it back — where and
what is left — after the room steps in stage 5 and after every command
that moves gear, so `world_checksum` (pieces after the hired hands,
health to a hundredth, `next_piece` too) sees what the room sees.

The five commands are `Stow { who, cell }`, `Fetch { who, kind:
FetchKind::Piece(id) | Resource(code) }`, `Equip { who, cell }`,
`Unequip { who, part }`, `Discard { who, cell }`, server-shaped like
`SetCraftTarget`. A stow or a fetch wants the Bim within `data::REACH`
(two tiles) of a container that takes the thing — `container_takes`: a
bench whose part is a locker-class cabinet (the armoury, the drug lab),
a **shelf**, which is locker class itself since the money rework took
the shelf class away, and a cold store for food; `Aboard::containers` lists them by the
room's indices and `Game::within_reach` measures — and is refused
`OutOfReach` (14) otherwise; `PackFull` (15), `NoRoom` (16, the class
full; `NoRoomAboard` is the same wall met buying) and `Broken` (17) are
the rest. Equipping wants no container. A fetch by resource of an armour
kind takes the *least* damaged piece; a fetch of a weapon resource is
the gun off `World::guns` (below), the highest tier of the kind, the way
the piece is the least damaged — `armour::weapon_resource` is `WeaponKind::
resource()` looked up in `ResourceId::ALL`, the way `resource_of` reads
`ArmourKind::resource()`, so the room's table is the one table: the
handgun being the laser pistol and the four after it their own (9–12
since the money rework closed the deleted resources up),
with `weapon_of` its inverse, `item_of` the item a resource makes and
`is_gear` what the shelves take beside the lockers
(`every_weapon_is_a_locker_resource_and_the_two_tables_agree` pins the
five, both ways, locker class and sold nowhere); the room's pieces and
packs come through `Item::Weapon` unchanged — and anything else a
`Stack(code)` of one. Events `Equipped { who, kind }` (34, `who + 10 *
kind`), `Stowed { who }` (35), `PieceBroke { who, kind }` (36, off
`Game::take_pieces_broken`, since the room wounds its own). Three things
that bit or would:

- **Move the piece before the count.** A fetch sets `at = Pack` and then
  takes one off `cargo`; a stow puts `at = Hold` and then adds one. The
  other way round, `settle_pieces` inside `on_ship_changed` would push a
  fresh piece or remove the worst one to make the count agree, and the
  fetched piece would be a duplicate or the stowed one gone.
- **No piece at `Hold` is ever broken**, and nothing relies on that by
  accident: `Game::take` refuses a broken piece, so `stow` cannot move
  one, and a sale only ever finds whole or dented ones. `Discard` is the
  only way out for a broken piece and takes it off `pieces`.
- **The bunk on the playtest ship is beside the armoury**, within
  `REACH`, so a test of "out of reach" stands the Bim at the helm's
  seat (`helm_seat` in the tests, the helm part's first use spot plus
  `aboard.offset`, through `put_for_probe` — the helm is a part and
  nothing more since feature 104 took the helm job away) rather than
  leaving it where it woke up. `at_the_armoury` in the tests stands it at
  the armoury's use spot *before every command that wants reach* — the
  Bim goes off about its errands between steps, and a command lands at
  stage 1, before the room moves.
- **The app asks the same rule, it does not copy it.** `container_takes`
  and `in_reach(who, resource)` are public so that `hold_of` in
  `crates/app/src/screens/game.rs` can ask once a resource a frame
  whether the Bim shown stands within reach of something that takes it,
  and a Store or Take row can read "walk over first" *before* the
  command is sent and refused. The command checks again when it lands;
  the row is a courtesy, not the gate.

`a_shot_on_the_head_is_taken_by_the_helm_first`,
`a_stowed_piece_keeps_its_health_and_a_sale_takes_the_worst`,
`a_fetch_or_a_stow_wants_the_bim_in_reach_and_room_to_put_it` and
`the_checksum_notices_a_worn_piece` are the tests; the room's half — what
a worn piece does to a hit, the pack, `equip`/`unequip` — is in
`crates/game/CLAUDE.md`. The second reactor and the armoury B2 put on the
playtest ship made `benches()` four there, moved `REFERENCE_CHECKSUM`
(with the pieces in it), and `an_order_wants_the_inputs_aboard…` now
takes *both* reactors off to darken the workbench. The four weapons B3
put in the playtest cargo moved it again — the cargo is hashed whole —
and the locker count in `a_target_for_a_handgun_runs_the_whole_chain…`
reads `9 + bandages + medkits` for the suit, the three pieces, the four
weapons and the handgun made.

## Two of a kind go onto the workbench, and one comes off a tier up

Feature 38.6 (September 2026). Every weapon and every piece of armour
has a `bims::combat::Tier` — the room's half, what a tier does to the
numbers, is `crates/game/CLAUDE.md` ("A weapon is a value with a tier")
— and the world is where tiers are *made*: two of a kind at the same tier
go onto the workbench and come off as one of the next, a day later.

**The hold's weapons are a list.** A weapon in a pack or a hand is the
room's `Item::Weapon(Weapon)`, tier and all, and the world keeps no copy;
in the hold it is still a count of its resource, and beside the count
`World::guns: Vec<Weapon>` says the tiers, sorted by kind and tier. **The
invariant is the armour's again: the count of each weapon resource is the
number of guns of that kind**, held by `settle_guns` inside
`on_ship_changed` right after `settle_pieces` — a count grown pushes
tier-one guns, one shrunk drops the lowest tier first (the sell rule) —
and moved-before-counted in `fetch` and `stow` like a piece. A fetch by
resource takes the *highest* tier; `FetchKind::Tiered { resource, tier }`
takes exactly that tier, which is what a container window's cells ask
for, since it shows a weapon stack **per tier** (a tier-two pistol is not
a tier-one one). A loot leaves the list alone: the weapon goes into the
pack as the item it is. `armour::Piece` carries `tier` and `Piece::new`
takes one; `weapon_at(resource, tier)` is the gun a resource is at a
tier. `guns_agree_with_the_hold` pins the invariant, the two fetches, the
stow and the sell rule; `the_checksum_notices_a_tier` that the list and
the tick box are hashed (pieces hash their tier too).

**The workbench has three slots, and the pair is carried to it** (feature
56, September 2026 — the user's 55, but 55 is the plain below). The
first cut began an upgrade out of the hold the step there was a pair;
the user then asked for the bench to be a thing: two slots the Bim
*carries* the pair to, a button once both are in, and a third slot the
upgraded one appears in. `World::bench: Workbench { slots: [Option<Item>;
3], carrying, back, work: Option<Upgrade> }` — `Workbench::IN` are 0
and 1, `OUT` is 2, and a slot holds the room's `bims::combat::Item`
(`Weapon` or `Armour`, never a stack) whole: **a piece on the bench is
not in `World::pieces`**, the way one in a pack is not in the hold, and
`fetch_from_bench` pushes it back with the health it had. The bench with
the slots is `World::workbench()`, the first workbench in bench order; a
second workbench is a bench to make things at and nothing more. The
workbench is *not* a hold container — `container_takes` still says no,
so `in_reach` and a plain `Stow` never land on it — and has its own
three seams:

- `Command::StowOnBench { who, cell }` — the thing out of the pack into
  the first free input slot, checked by `Workbench::takes`: `BenchBusy`
  (36) while work is under way, `NoPair` (35) for a stack, a tier-three
  thing (nowhere to go) or one that would not pair with what is in the
  other slot (kind and tier alike), `NoRoom` with both inputs full,
  `Broken`, `NoWorkbench` (34) with none aboard, and `OutOfReach` by
  `in_reach_of_bench` — `REACH` of the workbench's frame.
- `FetchKind::Bench { slot }` — a slot's thing into the pack: an input
  any time the work is not on it (`BenchBusy`), the output any time;
  `NotAboard` for an empty slot, `PackFull`. Nothing in the hold moves
  either way.
- `Command::Upgrade` — the button: `can_upgrade()` is `NoWorkbench`,
  `NoUpgrades` (the upgrades node not yet researched — see "The
  upgrades node gates the bench" under research),
  `BenchBusy` (work under way, or the output slot still full) or
  `Workbench::pair()`'s `NoPair`, and the world reads the same function
  for the window, so the button is greyed with the reason before
  anything is sent. `begin_upgrade` sets `work = Upgrade { resource, to,
  done: 0 }` and says `UpgradeBegun` (48); **the pair stays in its slots
  being worked on** until `finish_upgrade`, every step after the crafts,
  clears the two and puts one of the next tier in `OUT` — `Piece::new`
  with a fresh `next_piece` id, or `kind.at(to)` — and says `Upgraded`
  (49). No room check any more: it is on the bench, not in the hold.

`craft_orders` is as it was — one `UPGRADE_ORDER` session at the first
workbench while `bench.work` is under way and not `complete()`, powered,
nobody at it — and `finish_craft(UPGRADE_ORDER)` is `done += 1`.

**With the tick box on the crew do all of it, one thing in the arms at
a time.** `tend_bench`, stage 5 before the craft orders, is the whole of
the automatic path, and what it hands the room is a `Vec<bims::game::Ferry
{ from, to }>` (`Game::set_ferries`, at most one long), bench indices
both: `from` the cabinet, `to` the workbench, or the other way round.
The cabinet is `World::store_bench()`, the first bench whose part keeps
the locker class — the armoury, the drug lab — because the hold is one
pool and any cabinet of the class is where a gun is; **a ship with no
such bench has nowhere for a Bim to walk to, and the crew carry nothing
of their own accord** (the suit locker is not a bench). In order:
nothing while work is under way; a carry under way keeps its order
posted whichever way it was going (`bench.back` remembers, since the
order cannot once the thing is off its slot) — the room is a step
behind, and a chain whose order vanished gives the thing up; then, box
on: the output in `OUT` is carried back once `has_room` for it (else it
waits there, and the Management line says so); a pair in the inputs has
the button pressed for it; an input free has the next thing carried
over, `bench_wants()` — a match for the other slot, else the first of
`upgrade_pair()` (armour kinds in `ArmourKind::ALL` order then weapons
in `WeaponKind::ALL`, the lowest tier first, tier three never), and for
armour the **most damaged** piece of the kind and tier, so the good
ones stay in circulation and the piece that comes out is fresh whatever
went in. Stage 7 drains the room's three reports: `finish_ferry_pick` —
from the cabinet, `bench_wants()` asked *now* and `hold_gives` moves it
(instance, grid slot, count, `on_ship_changed`) into `carrying`;
nothing there any more (sold since) and the Bim carries nothing; from
the workbench, `OUT` into the arms — `finish_ferry_drop` — into the
first input slot the bench `takes` it in, into the hold if the slots
filled meanwhile; at the cabinet, `hold_takes` — and
`finish_ferry_return` (given up between the two), `hold_takes` either
way. `hold_takes` never asks `has_room`: it was in the hold a moment
ago, and the grid keeps what it cannot lay unplaced rather than losing
it. `drop_loads` (dock, undock) puts a carried thing back the same way.
The room's half — `Kind::Ferry`, `GoToStore → TakeGear → CarryGear →
PutGear`, offered under `Job::Haul` — is `crates/game/CLAUDE.md`.

The checksum eats the three slots and the carried thing (a gun by kind
and tier, a piece by id, kind, tier and health to the hundredth, `u64::MAX`
for none), `back`, and the work as `(resource, to, done)` or `u64::MAX`;
`REFERENCE_CHECKSUM` moved. `two_pistols_or_two_helms_are_combined_at_the_workbench_over_a_day`
runs the automatic path undocked — the carry seen in `carrying` with the
hold one lighter, both on the bench and the work begun, the day, the
output in `OUT`, the carry back, a fetch handing the tier-two pistol over;
the helms with the dented one chosen, the two whole on the bench with
their health, the fresh piece **waiting on the bench** while the lockers
are full of bandages and carried back once they are not, and the pistols
following — and pins the refusal codes;
`a_pair_is_put_on_the_bench_by_hand_and_the_button_pressed` is the
manual path with every refusal, the piece leaving and rejoining
`pieces` with a dent, the button, `BenchBusy`, and the output taken into
the pack with the box off. The Bim is no longer at the bench the step the
work begins — its hands are on the second pistol — so the test waits an
hour for `crafts_under_way`. The app's window is `crates/app/src/crew.rs`
`bench_window` (`BIMS_ARMOURY=workbench`); the Management tab's line
reads `bench.work`, or the thing waiting in `OUT`.

## Every hold but the desk is a grid, and a thing lies on it where it fits

Feature 38.13 (September 2026) made the lockers a grid; the shelves and
the cold store followed when the user asked for a ten-by-ten storage
whose limit is the stacking. `crates/world/src/grid.rs`. **Every class
but the research desk is one grid `GRID_COLS` (10) across**, every part
of the class so many rows of it (`PartDef::capacity` in cells — a shelf
100, a cold store 100, the suit locker 20, the armoury 80, the drug lab
60; `crates/shipdesign/CLAUDE.md`), and every thing kept there covers
its `economy::footprint` — a pistol 1×2, an auto rifle 1×7, a shotgun
2×5, a sniper rifle 1×10, a schword 1×5; a helm 2×4, kevlar 4×4, leg
guards 3×2; a suit 3×3, a medkit 2×2, a box of dressings 2×2 and five to a stack (feature 87); a crate of
vegetables 1×2, a block of tofu 4×4, a grenade 1×1 — the way a
survival game's inventory is laid out. **Goods that stack cover a
footprint a stack** (`economy::stack_size`:
ten vegetables or tofu, five dressings, one of anything worn or held), so
what a shelf holds is its cells times the stacks: the playtest ship's
forty vegetables are four cells. Beside the counts the world keeps
`World::grids: [Grid; 2]` in `World::GRID_CLASSES` order (cold store,
locker — the shelf class went with the materials at the money rework and
`PartKind::Shelf` is locker class now; `World::grid(class)` looks one up) — each `slots:
Vec<Slot>`, `{ id, kept: Kept, count, foot, x, y, turned }` with
`Kept::Piece(id) | Gun(kind, tier) | Stack(resource)`, and `next`, an id
that only climbs. **The invariant is the pieces' again: a class's slots
are exactly what it holds — one per piece at `Hold`, one per gun on the
list, and for each other resource stacks whose counts add up to the
hold's, none over the stack size — as far as they fit**, held by
`settle_grids` inside `on_ship_changed` right after `settle_guns` (a slot
names a piece by id and a gun by tier, so those settle first): a count
grown tops up the stacks with room first (the lowest id first) and lays
new ones by `first_fit` — row by row from the top left, the *whole grid
unturned before any of it turned*, so a rifle lies along a row while any
row has room and stands up only when none has — a count shrunk empties
the last stack first (the highest id) and drops what is empty, a piece
or gun gone loses its slot, and a thing that fits nowhere has the grid
`repack`ed once, biggest first, before it is left **unplaced**: counted,
in no slot, laid the step room is made, never lost and never forced in.
`Grid::covered` is what the slots take, `units_of` what the stacks of a
resource add up to; `lockers_agree` in the tests asks the whole
invariant of the lockers.

**The gate is `World::has_room(resource, units)`**, asked *before* a
count moves — by `buy`, `can_make` (the output onto the grid as it
stands; what the inputs would free is not counted, so a bench whose
locker is full waits a step for the vegetables to be spent), `tend_bench` (the
upgraded thing carried back from the workbench) and `stow` — and it is
the area rule (`ShipDesign::has_room`) **and** a
place on the grid as it stands (`Grid::can_take`, a trial `add` on a
clone: part-full stacks topped up, then new stacks laid). So a stow can
be refused `NoRoom` with cells to spare — the lockers full but for seven
scattered cells refuse a rifle the count would take
(`a_fetch_takes_the_slot_asked_for_and_a_stow_wants_a_run_of_cells`), the
cold store with area for three blocks of tofu and a four-by-four run for
two refuses the third
(`the_shelves_hold_stacks_and_a_fetch_takes_one_off_the_stack_asked_for`)
— and that is the point: tidy, or turn something. `World::room_for(resource,
wanted)` is the most of `wanted` that would go — a harvest banked was
its caller until the bay's growing went with the needs (feature 104). The designer's `Edit::Buy` has only the area rule,
since the yard has no grid; a design bought full in the yard is laid out
at `World::new` and overflows, if it does, the way a poked count does.

**Where a thing lies is a command.** `Command::Arrange { slot, class, id,
x, y, turned }` moves a slot of a class's grid — a drag in a container
window, or `R` over a thing there — refused `NoRoom` off the grid, over
another slot, for no such slot or a class with no grid, and wants nobody
in reach: it is tidying, nothing leaves the hold. The grids are in the
checksum, so it is a command: each grid's `next`, then every slot's id,
`Kept::codes()`, count, x, y and turned, after the guns and before the
tick box; `REFERENCE_CHECKSUM` moved. **A fetch names the slot**:
`FetchKind::Slot { class, id }` is what a container window asks, resolved
in `fetch` to the piece by id, the gun by kind and tier or one unit of
the stack's resource, and *that* slot is taken — or that stack is one
lighter — before the count moves, so two rifles alike, the one clicked is
the one that goes; the older kinds take the first slot holding the
thing, or one off the last stack. `Grid::fits(capacity, foot, x, y,
turned, ignoring)` is public so the app can colour a drag's ghost by the
same rule the command will apply. Four tests:
`the_lockers_lay_the_gear_out_and_a_thing_can_be_moved_and_turned` (the
playtest layout, an Arrange seen by the checksum and the twin, the three
refusals), the fetch-and-stow one, the shelves-and-stacks one, and
`the_grid_turns_a_thing_to_fit_and_keeps_an_overflow_unplaced` on a bare
`Grid` — stacks topping up and emptying included.

**The pack is a grid too** (feature 49): ten across by five down, the room's
(`crates/game/CLAUDE.md`, "The pack is `Gear::pack`"), every thing over
the same footprints — `Item::footprint` is `economy::footprint` said
again by code, and `the_pack_lays_things_by_the_same_footprints_as_the_lockers`
pins the two. A `cell` in `Stow`, `Equip`, `Discard`, `Where::Pack` and
`LootCell::Pack` is the cell a thing's top-left corner is in, and a
command naming any cell a thing reaches over lands on that thing
(`Gear::head_of`). `Command::Repack { slot, who, cell, to, turned }`
moves one — a drag in the inventory window — through the room's
`rearrange`, refused `NoRoom` where it would not lie and `NotAboard` for
an empty cell, and `mirror_pieces` after it, since a piece's cell is in
the checksum. A fetch lands in the first place the thing fits
(`Gear::free_cell_for`), so a test that reads `pack(0)[n]` after a
fetch has to know the footprints of what went before — a pistol lies
along two cells, so the next thing is at 2, not 1. `LOOT_CELLS` is 54
now: the pack's 50, then head, body, legs, weapon.

## A loot is a command, and only a crewmate is looted

`Command::Loot { slot, who, source: LootSource, cell }` takes one thing
off a body into `who`'s pack: `cell` a `bims::combat::LootCell` code
(the body's pack 0–8, then head, body, legs, weapon 9–12), `source`
either `LootSource::Crew(i)` — an index into the crew's room — or
`Resident(i)` — an index into the *station's* room, since the two keep
separate rooms while docked (`armour.rs`, `code()` 0 and 1, `who()`,
`from_code`). **Only a crewmate is looted** (features 102 and 104): a
`Resident` source is refused `NotACrewmate` (62) before anything else is
asked — every human is friendly, a station's dead are left as they lie,
and a machine carries nothing. For a crewmate `World::loot` asks, in
this order: the body *down* — dead or out cold, `World::is_down(source)`
(`Game::is_down`) — else `Refusal::NotDown` (18); the looter alive,
awake, aboard and within `data::REACH` of the body —
`in_reach_of_body(who, source)`, else `OutOfReach`; a free pack cell,
else `PackFull`. Then the crew's room does the stripping
(`Game::take_from_body`) and the taking in (`Game::give_stack`, a box of
dressings being five), `mirror_pieces` runs — a piece of armour off a
crewmate is already on the world's list and is found in the new pack —
and `WorldEvent::Looted { who, source_kind }` (38, `who + 10 * kind`) is
said. Nothing goes *onto* a body. **Down and reach are asked when the
command lands, not when the window opened**: a crewmate who came round
while the Loot window was up is refused `NotDown`; the app's `Body {
down, reach }` snapshot is a courtesy for the rows, read off `is_down`,
`loot_cells` and `in_reach_of_body` every frame, the way `hold_of` reads
`in_reach`.

`LootSource::Resident` stays for everything else that points at one of
the station's bodies — the hire, the commander's attack order,
`body_position`, `in_reach_of_body`. Where a body lies is
`World::body_position(source)` in the crew's room's units — a
resident's through `Aboard::from_station`, the inverse of `to_station`
— so the app can `send_to` somebody beside it; `None` for a resident
once the rooms have parted. `visit` tells the joined room which
residents are down every step (`Aboard::visit(&positions, &down)` →
`Game::set_visitors_down`, after `set_visitors`, which clears it) — a
wreck is never on that list, since a machine carries nothing — so a body
among them is `HIT_VISITOR` under a right-click; the fresh `Aboard` an
undock builds knows no visitors at all.

Until feature 104 a resident down in a fight could be looted too, and a
piece off one was pushed onto the world's list under a fresh
`next_piece` id, since the residents' room numbered its pieces as it
liked; that path, and its test, went with the human enemies.
`an_unconscious_crewmate_is_looted_and_an_awake_one_is_refused` (Kate
bled to under the line, the helm off her head with its 9 health, the
checksum of two worlds parting and meeting again) is the test, and
`no_human_is_ever_hostile_in_a_generated_galaxy` (`tests_run.rs`) pins
the refusal.

## An execution was a command, and went with the human enemies

`Command::Execute` — the Kill row on one of a hostile station's people
lying out cold, which walked a crew member over and shot the body where
it lay (`Game::execute`, `Kind::Execute`) — went in feature 104 with the
human enemies it was for, and `NotHostile` (26), `Unarmed` (27),
`WorldEvent::Executed` (48) and the room's `JOB_EXECUTE` (25) with it;
the codes are left free. `Game::knock_out_for_probe` is still how a
body is put out cold without a wound to bleed out from.

## A mercenary is an extra body in a friendly station's room, and hired it is crew that costs money

`crates/world/src/mercenary.rs`. A station the machines do not hold
may have hired hands living among its people: **extra** bodies past
`people_of`, opened with the room (`Residents::open(.., count,
mercenaries, ..)`), the last `mercenaries` of them — in the olive
`Uniform::Mercenary`, armed off `Gear::hired_for(seed, ids)` against
`MERCENARY_ODDS` (a third the pistol, the rest heavier) and
`MERCENARY_ARMOUR_ODDS`, and priced. `Residents::fee` is
`Vec<Option<Money>>` index for index, `None` for one of the station's
own; `Residents::hailable()` is who may be spoken to (a fee, and not
down), handed to the joined deck every `visit` as
`Game::set_visitors_hailable`, so a right-click on one **on its feet**
is `HIT_VISITOR` and the app's menu offers the Hire row (`visitor_down`
says which row). How many is `World::mercenaries_of(station)` →
`mercenary::how_many(worth, start_worth, map_seed, near_front)`: one for every half
of the start worth grown by, plus one on `MERCENARY_CHANCE` (0.4) off
the seed, at most `MERCENARIES_MAX` (4) — so at the start worth a
station has one or none, and a richer crew finds more. Nothing new is
hashed for it: it is a function of the seed and the worth when the room
opens, like the gear. Derelicts and held stations have none; the
room's bunks cap the crowd, residents first.

**The fee is the kit**: `fee_of(gear)` is `WEAPON_FEE` (pistol 2 000,
schword 3 500, shotgun 6 000, auto rifle 8 000, sniper rifle 15 000) plus
`ARMOUR_FEE` a worn piece (helm 1 000, kevlar 3 000, leg guards 1 000)
— a pistol alone is 2 000 a month, a sniper rifle in full armour
20 000 — and `priced(seed, gear)` moves it by up to `VARIANCE_PERCENT`
(15) either way in whole percent, whole euros. Tiers: the pistol is
one, the shotgun, auto rifle and schword two, the sniper rifle three;
armour has one tier until a heavier one is a row in `ARMOUR_FEE`.

**`Command::Hire { slot, who, resident }`** (`World::hire`, refusals in
order: `NotForHire` 19, `NotDocked`, `OutOfReach` — `in_reach_of_body`
with `LootSource::Resident` — and `Unaffordable`; the bunk check,
`NoBunk` 20, went with the needs in feature 104, a bunk being furniture
and no cap on the crew) takes the body out of the station's room (`take_crew`,
the one removed, `adopt` the rest back — every errand ashore dropped
once, the way a docking drops the crew's; `down` and `fee` shrink with
it) and adopts it into the crew's at the same spot on the joined deck
(`body_position` → `stand_at` → `adopt`), coverall kept; its worn pieces
become pieces of the world's under fresh ids (`Where::Worn`), re-issued
through `Game::issue`; `crew_down`, `crew_locked`, `Aboard::crew` and
`Ship::crew_count` grow by one (`on_ship_changed` for the dynamics); the
first month comes off `money` and a `mercenary::Hired { who, fee, due,
owed, medic }` goes on `World::hired`, in `world_checksum` after the
held towns. `WorldEvent::Hired` (39). `World::hire_offer(who, resident)
-> Option<Offer>` is what the app's Hire window reads every frame — fee,
in reach, affordable, docked, a medic or not — so it can say "walk over
first" before the command is refused.

**Wages are paid by travel**, since only travel moves the world clock
(feature 103): `World::travel` puts the clock on by the trip's minutes
and `pay_wages_due` pays every month the clock has reached, one at a
time, through `pay_wages`: a `Hired` whose `due` has passed is paid
(`MercenaryPaid` 40, `due += MONTH`, thirty days) if the money covers
it; else it is `owed` (`MercenaryLeft` 41, said once) and sails on owed
until it is paid. It used to be paid by the step and, owed at a berth,
`dismiss`ed back into the station's room; the step's clock went with the
free clock in feature 104, and with it the only moment a hand could
walk off, so `dismiss` went too. A mercenary is never a
player: appended after the players, no slot, no speed request. The
`test` command forces one at the dock through
`World::mercenary_for_probe` (`least_mercenaries`, not hashed).
`a_mercenary_is_hired_from_the_station_and_paid_by_the_month` runs the
whole of it on the combat ship; `mercenary::tests` pin the two prices
and the roll.

## A station is traded with across its desk

`PartKind::TradingDesk` (36) — a table's footprint, worked from the tile
below, seen over — stands in every station just inside the port against
the corridor's north wall, at `(5, mid − 1)` in `build_layout`, clear of
the spot the station's people are sent home to. The room reads every
desk on its deck (`Layout::desks`, `Room::desks`, kept on the joined
deck since the station's is the point — `leave_the_station_s` drops
other station fixtures, not these), answers `HIT_DESK` (16) under a
click and `Game::desk_spot(i)` for the walk. **`World::at_the_desk(slot)`
is the rule**: alive, awake, aboard and within `data::REACH` of a desk's
footprint; `buy` and `sell` refuse `NotAtTheDesk` (21) without it —
asked **last**, after the goods and the money, so "walk over first" is
said only about a deal that would otherwise go, and the build test's
sale from under a site is still `NotAboard`. A ship has no desk, so away
from a berth it is never true. `man_the_desk_for_probe(slot)` posts a
Bim at it for the tests that trade (`post_for_probe`: a post, so it
stays; the room's `Game::stand_down` lifts it). The app: the desk's menu row (`Trade`)
walks the Bim over and opens the trade window
(`CrewPanels::trade_requested`); the window offers "Walk over" while
nobody of yours is at it (`Session::at_the_desk`, `walk_to_desk`).
`trading_wants_somebody_at_the_station_s_desk` pins it.

**Nothing is bought or sold until it is confirmed** (September 2026).
The trade rows — `trade_rows` in `crates/app/src/screens/designer.rs`,
shared by the yard's Station panel and the docked window — fill a
`Cart`, the app's own: a signed line a resource, a picture of it from
`icons.rs` beside the word, and under the rows what the lot comes to
(bought, sold, "You pay"/"You earn", the money after, each hold's
count after), greyed a step early where it would not fit
(`Cart::short`: the money, then a hold). The buttons are live from
anywhere; **Confirm** is live at the desk, and sends the lot as
`Command::Sell`s **then** `Command::Buy`s (`Cart::deals`), so what a
sale frees is there for the buys behind it, each judged by the world as
before — the rules crates never see a cart. Confirm empties it; so does
shutting the window or leaving the berth with it up. `BIMS_TRADE=1` opens the
simulation with the window up, which is how it is looked at.
`a_cart_prices_its_lines_and_sells_before_it_buys` pins the arithmetic
and the order.
The desk moved `REFERENCE_CHECKSUM` (every station's layout has one)
and the walkability contract covers it.

## Money, the bounty and what a crew are worth (feature 95)

Three readings and one payment, all in `world.rs`, and none of them new
state beyond `World::money` itself.

**`World::worth()` is everything the crew own**, in whole euros:

- every part of the ship at `PartDef::price`;
- the hold at the **book value** (`economy::trade_price`, the same
  everywhere — a valuation, never what a desk pays), with a gun or a
  piece of armour at its **tier**, `book × economy::TIER_PRICE[t]`;
- every gun and every piece of armour **on** a crew member — in a hand,
  worn or in a pack — at the same book and tier, and every stack in a
  pack (a box of dressings, a medkit, a key) at its book;
- and `World::money`.

The crew's gear is in there on purpose: a thing moved out of the hold
into a pack must be worth the same in both, or a restock would make the
crew poorer and `start_worth != worth()` on the first step. **`start_worth`
is `worth()` taken at `World::start`**, the starting pool included, so
unspent money is never counted as growth — which it was before the pool
went into the sum. Both are what `mercenary::how_many` and the droid
waves are scaled against — and the human garrison (`station::enemies_of`)
and the raiders' boarders were, until feature 104 — so every one of
those got quieter about a crew that is merely carrying its own money
about.

**The Republic pays a bounty** for an enemy taken down:
`data::REPUBLIC_BOUNTY` by the enemy's tier — 500, 1 500, 4 500 —
`World::bounty_for(tier)` indexed safely, nought for no tier. It is
earned **once per enemy**, whoever did it. For a **machine** — every
enemy since feature 102 — it is its own tier, counted in `visit` the step
it is destroyed (feature 103). For a hostile room's *Bim* it is its
**gear tier** (`gear_tier(room, who)`), hooked at `award_classed_near`'s
call site in `experience`, the first down *or* death — and never earned
since feature 104, since no room has hostile Bims. A friend's people, a
neutral's and the crew are worth nothing. Earned is not paid:
`earn_bounty` pays it into `World::money` at once only at a site with
nothing left to clear, and otherwise holds it as `Run::pending_bounty`
(`WorldEvent::BountyPending`, 93) until the site is cleared
(`settle_bounty`, stage 8) — and throws it away if the crew leave the
site uncleared (`leave_mission`; "The loop" below). The payment is
`WorldEvent::Bounty { amount }` (92), and the app's line is "The Republic
pays €X."

**A quote is `World::quote_at(station, resource, tier)`**, which is
`quote` times the tier factor on both the ask and the bid
(`Quote::at_tier`), and `quote` itself for anything that comes at no
tier. `buy` takes a `tier` and the gun or the piece arrives at it;
`sell` takes none — `tiers_leaving(resource, units)` says which tiers
actually leave the hold, **lowest first**, and the desk pays for each at
its own. `gear_tier(room, who)` is what a Bim's bounty was read off.

## The world is bounded by the ship, and money by the dock

Two rules carried straight over from the design phase into the game, and both
are easy to lose:

- **Goods only change hands while docked.** `Buy` and `Sell` are refused
  anywhere
  else, with `Refusal::NotDocked` — and *holding station beside* a station is
  not docked either, which wants an airlock. A **part** is the exception
  since the money rework: a construction site is paid for docked, holding
  or landed, because euros are not a shelf. The trade window — and the
  Station button on the tray that opens it — is hidden rather than
  disabled, because a panel full of dead buttons is a panel nobody can
  tell is dead on purpose.
- **What is on the shelf is the station's, not its kind's.**
  `worldgen::Stock` is a bit a resource on the `StationBlueprint`, carried
  onto `world::Station::stock` and asked by `buy` (`Refusal::NotSoldHere`),
  `Session::sold_here` and the editor's `market`. `StationKind::sells` is
  still the ceiling; under it `Stock::roll` puts the `STAPLES` —
  vegetables, tofu and medkits — on every
  shelf and rolls the rest at `STOCKED_CHANCE` off the station's own branch
  of the contents stream. A test that buys something at the spawn buys a
  staple, or reads the shelf first. **The gear is not on the shelf at
  all**: a gun or a piece of armour is behind one of the two **trades**
  the place rolled off its own seed (`Stock::weapon_trade`,
  `Stock::armour_trade`; `crates/worldgen/CLAUDE.md`), and
  `Stock::sells` answers for it off that flag — so a station with neither
  sells no gear whatever its kind, and that is the ordinary case rather
  than a fault.
- **What a thing costs is the station's desk's, and it is two numbers.**
  Nothing is bought or sold at `economy::trade_price` — that is the
  **book value**, a valuation, and the world reads it only through
  `World::worth` (above). `buy`
  and `sell` quote `Station::market()` — an `economy::market::Market`,
  the desk's kind with the station's own `bias` — and pay its **ask**
  for a buy and its **bid** for a sale, checked sums both, the bid always
  under the ask. `station::market_kind(kind, plan)` is the one map from
  the generator's kinds to the market's: `Plan::Surface` is a
  `MarketKind::Settlement` whatever kind it is laid out as, a derelict
  is `None`, and `ship::Session::design` asks it for the yard's desk
  too. **A derelict keeps no desk**: a sale there is
  `Refusal::NoMarket` (33), a buy `NotSoldHere` as before, since it
  stocks nothing. `Station::bias` is the blueprint's roll carried across
  — bar the **spawn's, which `World::start` sets to `Bias::NONE`** so an
  opening pool buys the same at a kind of station whatever the seed
  rolled and the yard's desk (kind, no lean) and the world's agree; a
  settlement's is rolled beside its shelf in `Surface::all_of`. The bias
  is not in `world_checksum` — it is a function of the galaxy seed, in
  the galaxy checksum, like the shelf.
  **A tier multiplies both**: `World::quote_at` is the quote times
  `economy::TIER_PRICE[t]`, a buy names its tier and a sale gives up the
  lowest tiers in the hold first and is paid for each at its own.
  `buying_costs_money_and_makes_the_ship_heavier` pins a buy at the ask
  and the sale back at the bid losing the spread;
  `a_derelict_has_no_market_and_every_other_station_quotes` the rest.
- **There is no fuel, and nothing burns.** The fuel went in September
  2026 and a trip ran on the reactor after it; since feature 104 nothing
  is flown at all — a trip is resolved (`World::travel`, "The loop"
  below) — so the engines' draw (`Power::engines`), the trip's preview
  and `World::can_modify_part`, which refused an engine, a thruster or a
  reactor while a trip was in the air, went with the flight. `run_power`
  charges the batteries `supply − draw` a minute and `Power::load()` is
  `draw / supply`. An engine still matters: a trip is quoted off
  `Ship::dynamics` by `physics::travel_days`.

## The room is aboard the ship, and it is the same room

`crates/game` is a library as well as the room's cdylib, and `world` runs
it: `crates/world/src/crew.rs` holds a `bims::game::Game` laid out from the
accepted design by `bims::aboard` and steps it in stage 5 of `World::step`
— `Game::simulate` once per world step, at one sixtieth of a real second,
which is the room's own frame at 1x. The Bims aboard are the room's Bims:
their errands, their routes, their wounds and their fight, all of it.
Nothing was copied; `world` imports `bims`, and `ship` imports it too, for
the painter. The room's needs — hunger, sleep, the galley, the heads, the
bay, the diary's every entry but a crewmate's death — were the room's
too, switched off by the world in every run from feature 102 and deleted
in feature 104.

Five things that hang off that and will bite:

- **`Game::render` is split from `Game::simulate`.** The world calls
  `simulate` per step and the ship painter calls `aboard.render()` once
  per frame; at 24x that is one picture a frame rather than twenty-four.
  (The behaviour test room's own screen called `update`, which is both,
  until it went in feature 104.)
- **The room draws its fixtures and the ship painter draws the rest.**
  `bims::aboard::drawn_by_room` names the parts the room has pictures for —
  the galley, the heads, the table and seats, the bunks, the bay, the locker
  — and `world_paint` skips those tiles and re-emits the room's whole draw
  buffer turned with the ship (`room_aboard`). Most of those parts do
  nothing since feature 104 and are pictures only, drawn in the state a
  fresh room draws them. The room's night wash and its deck plate are off
  aboard (`Room::shell`); the ship owns the sky.
- **Every fixture is used from the south.** The room's stations stood the
  Bim *below* the counter, the pan and the basin, and above the bay, as
  the classic layout had them, and a part turned to face another way is
  used from the wrong side. Nobody uses those fixtures since the needs
  went, but a round's stops (`routine::Anchors::of`) are still read off
  the same spots. `aboard.rs` says so at the top. Fixing it is the room's
  stations learning a direction each.
- **A Bim's index is its berth and its seat.** The room has a berth per
  bunk and a seat per chair of the design, and a crew past them shares the
  last (`berth` and `seat` clamp; "A station has a stance" above); a
  layout with none of one gets a single stand-in.
- **The RNG order of a room's making is pinned.** The classic room drew
  each Bim's start position *between* the Bims, from the one stream;
  `with_room` takes a closure for exactly that reason. Drawing them all
  first reshuffled every seed and failed the old `probe.rs` on "somewhere
  that counts as deck" — an hour's diagnosis for a two-line reorder. The
  classic room went as a game mode in feature 104, and what pins a room's
  making now is the survivor tests (`tests_survivors.rs` in `crates/world`
  and `crates/ship`; see the section on feature 104 at the end).

The heads aboard have no compartment: the pan and the basin stand on the
deck (`bims::fixtures::Heads`). The classic bath's three walls,
zero-sized and off the map aboard, went with the classic room in feature
104 — dropped from `Room::solids`, with the survivor tests showing that
no route moved (`crates/game/CLAUDE.md`, "The needs deleted") — and its
door with them. The
room's nav grid takes every other blocking part as a solid
(`Layout::others`), and every tile inside the deck's bounding box that is
not deck, so an L-shaped ship does not get a room that thinks the missing
corner is floor. `the_crew_live_aboard` in `crates/world/src/tests.rs`
runs the playtest ship six game hours and asserts the Bim went somewhere
and never left the deck.

The names are still the host's: `CREW_NAMES` in `crates/app/src/screens/game.rs`, painted by
`paintCrewNames` off `Session::crew_on_screen`/`_y`, which are camera units about the
ship. The crew's positions and the room's clock are in `world_checksum`, so
the room coming aboard moved `REFERENCE_CHECKSUM`; anything that moves a
Bim moves it again, and that is the checksum working.

## The starting system is charted, and the pictures are one drawing at two scales

`World::start` puts **every** node of the spawn system in `discovered`: the
crew picked the dock off the lobby's chart of that very system, and a map
that then hid what they had just looked at had nothing on it to fly to —
which read as "I cannot click on stations". Discovery (`discover_along`)
is untouched and is for what the chart does not show; a probe of it has to
`uncharted_for_probe()` first or there is nothing left to find. Two tests
already do.

`paint_body` and `paint_station` in `crates/ship/src/world_paint.rs` are
the pictures, by `BodyKind` and `StationKind`, drawn from ellipses and
rectangles about a centre and a diameter, and used twice: at icon size on
the map (pixels over the map scale) and, for a body, hull-sized alongside,
drawn **under** the hull as the ground. A station alongside is no longer
its icon — it is a hull of its own, drawn by `world_paint::stations`; see
"A station is a place". Nothing in them
may paint `VOID` to cut a shape — the derelict's broken ring is short
straight pieces, because a void bite painted over the deck was the first
thing that went wrong. The map also rings the site picked on the world
map's list, off `ship::Game::aimed` — set by the app every frame since
feature 103, where it was the helm's aim before — and read by nothing
that decides anything.

`STATION_SHARE` went from a quarter to three fifths and a system rolls for
a second and a third station (`MORE_STATIONS`); the rolls are drawn whether
or not they take so the stream stays in step. That is a re-pin of the four
`worldgen::fixture` checksums and of `world::fixture::REFERENCE_CHECKSUM`
and not a `GENERATOR_VERSION` bump — the shares are deliberately off the
bump list, since no layout changes shape. The later move to eight rolls
and two-to-ten bodies (`crates/worldgen/CLAUDE.md`) *was* a bump, to 4,
and moved the simulation's dock to a station that rolls a mercenary for
hire: the residents' room there is the residents **plus**
`World::mercenaries_of`, which is what the tests that count it compare
against now.

## A post is a standing order, and the helm is a part and nothing more

Nobody flies the ship since feature 104, and the helm is a part like a
bunk: the helm job and every gate in front of it — `World::can_command`,
`helm_spot`, `at_the_helm`, `order_to_helm`, `stand_down`,
`man_the_helm_for_probe`, `HELM_REACH`, `Command::ToHelm`, the room's
`Job::Helm` — went with the flown trip. What stood behind the helm still
stands behind the desk and every class's key: `World::fit_to_act(slot)`,
the crew member alive, awake and aboard (`is_unconscious`,
`is_outside`); before b-next (September 2026) the helm asked only the
distance, and a body that died beside the seat could still fly the ship.
Slot *i* is Bim *i*. Two things that stay:

- **A post is a standing order, not a chain.** `Character::post` is where a
  Bim has been told to stand; `Game::send_to` sets it (interrupting the
  errand and walking there), `Game::walk_to` is the same walk *without* the
  post, `return_to_post` walks back once whatever took the Bim away is
  done, and `order_move` clears it. It holds the Bim still exactly as
  `recruited` does. `return_to_post` only re-routes when the last route has
  been walked to its end — "hands the Bim a fresh route every frame: it
  never moves" is the trap it is written round — and `POST_SLACK` is how far
  off the post counts as on it. `adopt` shifts a post and **the route in
  progress** with the body: before it shifted the route, a Bim carried
  between rooms mid-walk marched off to where its old waypoint used to be.
- **An order to the room is `Command::Crew { slot, order }`, and one
  given with Shift is `Command::CrewLater`** (feature 69): the same
  `bims::order::CrewOrder`, handed to `Game::order_later` instead of
  `Game::order`, so it waits its turn on the crew member's queue rather
  than displacing what it is on — see "An order given with Shift waits
  its turn" in `crates/game/CLAUDE.md`. The walk refusals come back the
  same way (`walk_refusal`); everything else a queued order cannot do
  when its turn comes is dropped in the room without an event. The desk
  (`ToDesk`) has no later form: it is the world's walk, given now.
  `a_shift_order_is_a_command_that_waits_its_turn` in `tests_orders.rs`
  pins the seam.

## A ship is docked or holding, and nothing in between

`ShipState` is `Docked { station }` (code 0) and `Holding` (1) and
nothing else since feature 104: `Travelling`, `CastingOff`, `Undocking`,
`Docking` and a jump's `Charging` went with the flown trip, and with
them the push-off, the docking run, `plan_from_here`, `hold_point`,
`way_out`, `Ship::pending`, `send_everybody_home`/`everybody_home` and
`UNDOCK_MINUTES`, `DOCK_MINUTES`, `CASTING_OFF_LIMIT`. A trip is
resolved rather than flown (`World::travel`, "The loop" below): the
ship arrives `Docked` at its site — a settlement's id for a planet — in
one go and leaves it `Holding`
off the site (`leave_mission`), the rooms taken apart by
`unjoin_rooms`. `ShipState::alongside()` and `station()` are both
`Docked` alone now, where `alongside` used to take in a ship casting
off with the rooms still joined. `undock_for_probe` and
`dock_for_probe` are the tests' way between the two; `STATE_NAMES` in
the app is `["Docked", "Holding"]`.

The map keeps its zoom between visits (`Game::set_mode` no longer
refits), and the crew and the station's people are told apart by
`character::Uniform` — the coverall is the room's, the yoke and the hair
are the person's; `Residents::open` is the one place the station's is
put on.

## The station has rooms, and the layout is a walkability contract

`station::build_layout` was a chamfered square with two three-tile corridors
crossing in the middle and four rooms off them (it is a hub and four arms
now — the bold paragraph at the end of this section — but the rules
below were learnt on the square and still hold). Every room has a **two-tile
doorway** and every fixture stands with **two clear tiles in front of it**, because the room's nav inflates every solid by
`BODY_MARGIN` (23) on a 52-unit tile and a one-tile gap leaves six units,
which it will not walk. That rule is not only about corridors: **two solids
one tile apart corner to corner leave a diagonal gap it will not squeeze
through either**. The reactor at `(x0 + 3, y0)` and the tank at
`(x0, y0 + 3)` did exactly that and cut the whole of engineering off — every
tile in it read as free deck, `validate` was happy, and `send_for_probe`
returned false for all of it. The tank is at `y0 + 4` for that reason.

`a_station_s_rooms_can_all_be_walked_from_its_door` in
`crates/world/src/tests.rs` is the contract: it builds the room's own `Nav`
from the layout and asks it for a route from the deck inside the port to
every use spot and every open deck tile, for every plan on every kind — the
hub at three seeds, the rest at two — **when `BIMS_SWEEP=1` asks for it**,
which `./check full` does. A route search per tile of deck over
sixty-five layouts is longer than every other test in the workspace put
together, so a plain run walks a slice: one seed, one kind a plan,
cycled so every plan and every kind is still walked. Run
it after moving anything in the layout, and run it **swept** — `BIMS_SWEEP=1
cargo test -p world walked` — before saying a layout change is done;
`validate` will not tell you.

Knock-ons: a layout is one plan sized by plan and kind, and the seed decides only
how many bays, shelves, tables and batteries — two seeds are two stations
without being two buildings. Only the **first** bay, cold store and so on
by id is the room's fixture; the rest are furniture the painter draws as
blocks, which is why a second bay is a green square. And the deck just
inside the port is corridor and stays open: `simulation-check.mjs` finds the
station's deck by scanning to starboard from James.

**The plan is a hub and four arms now, after the picture the user gave.**
The hull is the **union of `Block`s** — a thirteen-tile hub (`HUB`),
seven-wide arms (`ARM`) to a nine-by-seven docking lobby (`LOBBY`,
`LOBBY_DEPTH`) at each end with an airlock in its outer skin (the west
one placed first, so it is the port; the array in the north lobby's
skin), and the rooms hung off the north and south arms two deep a side:
the mess and the crew's quarters to the north arm's west, the heads and
the laboratory (the bay) to its east; the rec room and the research room
(more bays) to the south arm's west, the storage and the cargo (the
shelves) to its east. The port's lobby is thirteen tall and eleven deep
(`PORT_LOBBY`, `PORT_LOBBY_DEPTH`) and is the reactor room as well —
the trading desk and the reactor along its north wall, life support,
the batteries and the tank along its south, the corridor through the
middle, so `stage_droid_fight_for_probe`'s spot and the ashore spot are
on open deck. Every tile in any block is frame and deck, and one with any
of its eight neighbours outside them all is outside wall (`skin`) —
which is why a room keeps `ROOM_GAP` (two) of void from the lobby and
the hub beside it: a room touching the hub's corner would open into it.
Partitions are explicit `Wall` runs down the shared hull column with a
two-tile `Door` (`column`): the inner rooms' onto the arm, the outer
rooms' through the partition, the west rooms' doors towards the hub and
the east rooms' towards the lobby so none faces another across the
corridor. A barricade of sandbags stands `BARRICADE_OUT` (four) tiles
out from the hub's skin in each arm, three of the five tiles from
alternate walls — low cover since September 2026, walked and seen over,
ducked behind (`crates/game/CLAUDE.md`, "Sandbags are low cover").
Sizes: Relay 48 (the smallest the rooms fit at),
Outpost 52, Derelict 54, Refinery 56, Orbital 64, the arena 72 with
`ARENA_BUNK_COLUMNS` (4) columns of bunks in the quarters, since a
column holds five there and the garrison was sixteen (the arena is laid
out as it was, though nobody lives in it once the machines have it).
The tank stands a row
up from the wall — it is filled from below its left-hand column, and a
tank against the wall has its spot in the skin (`UseSpotBlocked`).
There are no chamfers: a diagonal piece where two blocks meet would be
a pinch the navigation cannot walk, and the picture's round hub is a
square one here. `nav_map_of_a_station` (ignored) prints the plan as
digits (`BIMS_NAV_MAP=Ring` picks a plan by name); run it after moving
anything, and the walkability contract
after that.

## A station is one of six plans, and the spawn is the hub whatever it rolled

`station::Plan` (September 2026) is which building a station is: `Hub`,
the plan above, and five more — `Pod`, `Cross`, `Spine`, `Ring`, `Comb`
— rolled evenly off `map_seed ^ PLAN_SALT` (`Plan::rolled`; a stream of
its own, so the layout's own rolls for bays, shelves, batteries and holes
are what they were). `Station::plan` carries it, `station::layout(kind,
plan, seed)` builds it and the cache is keyed by all three. The plan
decides three things the seed does not: the **size** (`Plan::side`: the
hub by kind as before, the others a base each — pod 34, cross 44, ring
48, comb 52, spine 60 — plus 8 for an orbital, 4 a refinery, 2 an outpost
or derelict, 0 a relay), the **corridors** (`Plan::corridor`: two wide
on the pod, cross, ring and comb, three on the spine, five on the hub)
and the **residents** (`Plan::residents`: pod 1, hub 2, cross 3, spine
4, comb 5, ring 6 — `residents_of(kind)` caps it, so a relay houses one
and a derelict nobody on any plan, and "does anybody live there" is
still `residents_of(kind) > 0`). Each of the five newer plans has at
least two bunks over its residents, for mercenaries.

**The spawn is a hub whatever it rolled** — `World::start` calls
`Station::replan(Plan::Hub)` on it — the way it is home whatever its
stance rolled: a crew's first dock is the familiar one, every fixture
test and `basic()` walk it, the pinned `REFERENCE_CHECKSUM` stands on its
berth, and the arena (`station::arena`) is it laid out bigger. Every
other station of the system, and every station after a jump, is what it
rolled. So the variety is met by *travelling somewhere else*; `nix run
.#test` still docks at a hub.

**How they are built.** `build_layout` is now `match plan` to a floor
function — `hub`, `pod`, `cross`, `spine`, `ring`, `comb` — each
returning a `Floor` (the hull blocks, the airlocks with the port first,
the array tile, the reactor room's deck, the walls and doors, the cover,
and a block per room role: mess, quarters, heads, research, optional lab
and rec, any number of stores, the bunk columns, the lit blocks, the
big plant's tile) and one `furnish` that fills it in **one order for
every plan**: hull, airlocks and array, the reactor room (desk at
`x0 + 1`, reactor `x0 + 5`, life support `x0 + 6` on the south wall,
batteries `x0 + 4` — so a reactor room is at least eight wide and eleven
tall), walls and doors, sandbags, galley and tables, bunks, heads,
research desk, bays (the lab's then the research room's; the broom locker
in the lab's corner or the research room's without one), rec tables,
shelves, lamps, comforts, holes. The hub's floor reproduces the old
`build_layout` step for step and its designs come out **hash-identical**
(checked when the plans went in; the reference checksum did not move).
The newer plans wall their rooms with `enclose` — a `Wall` on every
ring tile of the room block that is deck (the skin refuses one) less the
doorway's two tiles — so adjacent rooms must **share** their wall
column or row exactly, or a one-tile corridor is left between them that
nothing can walk. Things learnt laying them: a door's two inside tiles
and the two beyond them must be clear of fixtures (the bunk column at
`x0`, the shelves at `x0 + 2, x0 + 4, …`, the mess's chairs at
`x0 + 2..x0 + 3`), which is why the quarters' door is in the wall the
bunks do not stand against and the cargo's is in its far corner; a
sandbag fits only in a corridor three wide or more (one tile from a
wall leaves two), so the two-wide plans keep their cover in a hall or
have none; lamps hang on the *inner skin* of a ring corridor as happily
as on a wall. The port's straight run of deck stays at least nine tiles
deep everywhere for `stage_droid_fight_for_probe` and the ashore spot.
`a_station_s_plan_is_rolled_off_its_seed_and_the_spawn_is_a_hub` pins
the roll, that all six turn up in the default galaxy, that no two plans
share (size, corridor, residents), and the spawn rule;
`a_station_is_a_place_the_room_can_live_in` and the walkability contract
run every plan on every kind.

## Construction is stage 7, and a site is paid for out of the pool

`crates/world/src/build.rs` is a `BuildSite` — an id, a kind, an origin
and a rotation, the same three numbers a design-phase placement is, and
**nothing else**: since the money rework (feature 95) there is nothing
carried to a site, so `delivered` and `carrying` went with the hauling.
`BuildSite::price(design)` is what it costs (`shipdesign::site_price` of
its `edit()`: a plating site is the floor and, where the tile has no
frame, the structure under it) and `build_minutes(price)` is how long it
takes, `BUILD_MINUTES_BASE + BUILD_MINUTES_PER_EURO × price`.

`Command::PlaceSite`/`CancelSite` are the seam; `builds` and `next_site`
are in `world_checksum`; `WorldEvent` codes 27–30 are `SitePlaced`,
`SiteCancelled`, `Built`, `BuildLost`; `Refusal` 11 and 12 are
`WontFit` and `NoSuchSite`, **80 is `NotEnoughMoney`** and 82
`NoShipyard` (10 and 13, `UnderWay` and `UnderConstruction`, went with
the flown trip in feature 104 and are left free). Every step the world
hands the room one `bims::game::Build` per site (`build_orders`): its
tiles in room units (the offset added) and `minutes > 0` when the part
would go down now — which is to say when `shipdesign::apply` of its edit
goes and when **the crew can pay for it**.

- **A site is begun only while the pool covers it.**
  `World::affordable_site(site)` is the rule: the price of every site
  already begun comes off `World::money` first (`free_money`), and what
  is left has to cover this one — so two sites are never both begun on
  one wall's worth of euros, and a site that cannot be paid for **waits**
  rather than failing. `place_site` refuses `NotEnoughMoney` outright
  where the pool could never cover it. `finish_build` takes the price and
  puts the part down in the same step.
- **Deconstruction gives the whole price back** (`shipdesign::refund_for`),
  so building and unbuilding leave `World::worth()` where it was —
  `building_and_deconstructing_leave_worth_unchanged` in
  `tests_money.rs`.
- **A site is paid for anywhere.** Goods want a desk and a desk is a
  place; a part does not, because euros are not a shelf. Docked, holding
  station or landed, the price leaves the pool and the part goes down.
- **Every site is on the room's list, wanting nothing or not.** The
  first cut only listed sites with something to do, and a Bim walking to
  one found its site gone from the list — `site_stand` had nothing to
  stand beside — and gave the errand up, every trip.
- **The room's copy is a step behind the world.** `Construct`'s `leave`
  takes the site off `Room::builds`, or the room re-offers the very site
  it just finished within the same step — `consider_errand` runs after
  the chain ends — and the world's `Built` arrives with a fresh chain
  already on the way to nothing. `building_under_way` skips a chain that
  `is_done()` for the same reason.
- **`can_place_site` is `apply` on `design_with_sites()`** — every
  pending site laid on first, in order — and then `validate`: a site is
  refused (`SiteRefusal::Fault(code)`) when the ship *would then* raise
  an error it does not raise now. A wall on the hob's use spot is the
  case; the app says the issue line. The room works the sites in order,
  so a wall on plating that is itself a site waits for the deck.
- **The ship and the building kept off each other** while the ship
  flew: sites were placed and worked only at rest, and a Confirm was
  refused `UnderConstruction` while any site was begun. The ship is
  always at rest since feature 104, so both checks went with the
  flight; `World::under_construction` (the room's `building_under_way()`:
  a Bim on the way to a site, or at one) is still there to be asked.
  `a_site_is_begun_once_somebody_is_on_the_way_and_a_cancel_frees_its_price`
  is what the old test of it became.
- **A part built relays the room under the crew** — `relayout_room` →
  `Aboard::relayout` → `Game::relayout` → `Room::relayout` — keeping
  every errand, the blood on the deck and the doors' locks; docking is the
  only thing that still takes the room apart. Joined, the joined design
  is recomputed through `docking::join` and the offset does not move.
- `BUILD_MINUTES_BASE`/`_PER_EURO` are in `data.rs`. `HAUL_LOAD`,
  `World::haul_load`, `World::free` and every reservation are gone with
  the hauling, and so are `Room::picked`/`dropped`/`returned` and the
  four `Step`s that filled them.

The room's half — the one chain, the stand spot, the suit — is in
`crates/game/CLAUDE.md`; the blueprint and the Build tab in
`crates/ship/CLAUDE.md`. `a_site_on_the_deck_is_paid_for_and_built_by_the_crew`,
`a_site_beyond_the_hull_is_built_in_a_suit`,
`a_site_is_begun_once_somebody_is_on_the_way_and_a_cancel_frees_its_price`
and `a_site_is_refused_where_the_designer_would_have_refused_it` are the
tests.

## Two things worked out once, not once a step or once a frame

- **The power budget** is `World::power_budget`, set at `start` and in
  `on_ship_changed`, which is the only path a part joins or leaves the
  design by (`shipdesign::apply`, at the one `self.ship.design = next`).
  `run_power` and `power()` read it. It used to be
  `shipdesign::power_budget(&design)` every step — a union-find over
  every tile of the grid — and was two thirds of a docked step.
  `World::powered_parts` (`shipdesign::powered_parts`, the live parts by
  id) is kept beside it for the same reason since the lamps went on the
  bill: `powered(kind)` used to ask `is_powered` — the union-find again —
  once a part of the kind per call, and `sync_lamp_power` asks it of
  every lamp every step.
- **`ShipDesign::part(id)` is a binary search**, because `parts` is in
  ascending id order (`parts_are_in_id_order` in the shipdesign tests).
  The painters ask it per tile per hull per frame (`hull::diagonal_at`,
  `shadow`, `hull_tiles`), and the linear scan it was made a station's
  picture 2.4 ms a frame. What the ship painter still spends per frame
  is drawing the station's hull from scratch — a few hundred µs — and
  that picture never changes; it is the next thing to cache if a frame
  is short again.

## Research is the world's state, and a key is a resource that is found

`World::research` is a `shipdesign::research::Research` — what the crew
know, which locked nodes have had their key, what the AI is on and how
far, and the queue it goes onto next — and it is in `world_checksum`
whole after the construction sites (the queue as its length and then
each code, after `progress`; that moved `REFERENCE_CHECKSUM` when it
went in, feature 64), with `World::station_keys` (a `u8` a station, by
index into `stations`: the tier of key still on its desk, nought for none)
after it. Five commands, all slot-stamped like
the rest: `TakeKey { who }`, `Unlock { node }` (one key, one node, of
the node's own tier),
`Research { node }` (queue it — `Research::enqueue`, so what it needs
goes in ahead of it; a `ResearchQueued` for each), `CancelResearch`
(the AI off what it is on, and off the queue what needed it) and
`Dequeue { node }` (a node off the queue, and what needed it with it).
Events 44–47: `KeyTaken { who }`, `Unlocked { node }`, `ResearchBegun
{ node }`, `Researched { node }`; 65–66: `ResearchQueued { node }`,
`ResearchDropped { node }` (off the queue without being begun, by a
`Dequeue` or in the wake of one, or of a cancel). Refusals 22–25:
`NoKey`, `NoResearchDesk`, `NotResearchable` (cannot be queued: planned
already, or a key wanting somewhere in its chain), `NotResearched`; 39:
`NotQueued`; 40: `NoUpgrades` (the workbench asked to take two of a
kind up a tier before `Node::Upgrades` is known). The AI's step is `run_research`, the second half of stage
6 — it runs on the desk's power (`research_desk_powered`: a
`ResearchDesk` aboard and `powered`): `Research::next` first, so an
idle AI goes onto the head of the queue and says `ResearchBegun` — the
step the queueing command landed in, since commands apply before the
stages — then `advance`, `Researched` the step a node is done, and
`next` again that step, so the next queued node begins with no idle
step between.

What research gates, and where: `craft_orders` skips a recipe
`!research.recipe_allowed(i)` however the bench came aboard — the one
recipe there is waits on `Medicine`, which is known at the start, so
nothing is held back by it in practice since the money rework cut the
tree to five nodes; `know_everything_for_probe` and
`research_for_probe(node)` are still how a test gets ahead of it. And
`can_place_site` answers `SiteRefusal::NotResearched(node code)` before it
asks `apply`, `place_site` refusing `NotResearched`. The design phase is
not gated here (the yard built the ship); the app's palette leaves the
unknown parts out off `Research::new()`.

**Where the keys are.** `Station::key` is a **tier**, nought for none,
and `station::key_tier(kind, hostile, map_seed)` is the rule: a derelict
holds nothing; every station whose blueprint was rolled hostile holds
the **tier-two key** (`ResourceId::ResearchKeyTwo`, 14 since the money
rework closed the deleted resources up), no roll — a stranger's desk
since feature 104 rather than an enemy's, unless the machines have taken
the station; any other holds the tier-one key at
`station::key_rolled(map_seed)`'s odds — `KEY_CHANCE` (80) in a hundred
off a stream of its own, the layout's rolls what they were, and the
tier-two keys going in moved no tier-one key: which friendly desks hold
one is pinned over a seed set across every galaxy type in
`keys_are_on_four_friendly_desks_in_five_and_always_at_the_spawn` (a
count and a hash captured before the change). `World::start` copies it
to `station_keys` with the spawn forced to tier one, so the first key
is always at home — the arena included; a settlement's desk is bare
(`surface.rs` builds with `key: 0`), and so was a raider's until the
raiders went in feature 104. `station_key(id)` is the
tier, `station_has_key(id)` whether it is above nought, `key_at_the_dock()`
the tier at the berth, and the ring of lights (`world_paint::key_lights`)
shows for either tier. `tier_two_keys_lie_only_on_hostile_desks` pins
the rule over the same seed set. Every station's layout has a
`ResearchDesk` against the research room's north wall from the corner,
worked from the row below, and that room's trays start a row lower than
the laboratory's (`first_row` 3 against 2) so the desk's spot has deck on
its far side — a spot between two solids is one the navigation will not
walk. That moved `REFERENCE_CHECKSUM` (every layout has a desk, and the
playtest ship one); the walkability contract covers it.

**A take is a command across the joined deck.** `station_desk()` is the
index into `research_desks()` of the desk standing in `station_box`;
`key_in_reach(who)` is `within_reach` of `Container::Desk(that)`; `take_key`
wants docked, a key there, reach, and `free_cell_for(Item::Key(tier))` —
two cells one over the other, either tier — and then `give`s
`Item::Key(tier)` for whatever lies on the desk and sets the station's
tier to nought. **No new check**: a key is taken at a held dock while
the machines stand, in the middle of the fight
(`a_key_is_taken_at_a_held_dock_while_the_machines_stand`: the
generator-hostile station's tier-two key, the station infested, a wave
standing), and at a stranger's with nobody minding. `key_desk_spot()` is the walk for the app, which sends the
take the frame the Bim is in reach (`CrewPanels::key_requested`). Home,
the key is stowed like anything else: `container_takes(Desk(i))` is the
`Research` class **on the crew's own desks only** (`Some(i) !=
station_desk()`), `armour::item_of` makes a `ResearchKey` an `Item::Key(1)`
and a `ResearchKeyTwo` an `Item::Key(2)` (`key_tier_of`, `key_resource`)
and `resource_of_item` the way back, `fetch` finds the cell with
`free_cell_for`, and `pack_item` reads a tail cell as its key, so a stow by
either cell is the same stow. Both keys are the `Research` class and one
cell of it, so the desk's capacity of one holds one key of either tier,
never two. `unlock` wants a powered desk, then a node with a lock
(`Research::key_wanted`, else `NotResearchable` before the desk is looked
at), then **a key of that tier** free in the desk (`keys_in_desk(tier)`,
which takes a tier now — the other tier's key there is `NoKey`, and it
stays), then `Research::unlock(node)` to take (a node open already is
`NotResearchable`, and no key is spent), and takes one of that resource
off the cargo through `on_ship_changed`
(`unlock_wants_the_nodes_tier`). `research` wants a desk aboard and
`Research::enqueue`; `dequeue` wants the node on the queue.

**The upgrades node gates the bench.** `shipdesign::research::Node::Upgrades
= 9` — tier 2, locked, after the armoury, 1 440 minutes — is
`Research::upgrades_allowed()`, and `can_upgrade` is `NoUpgrades` (40)
without it, before the bench is looked at, so `begin_upgrade` and the
button both refuse; `upgrade_pair()` and `bench_wants()` answer `None`
without it, so Combine matching gear carries nothing to a bench that
would refuse it. The node covers both steps, one to two and two to
three; there is no tier-three node. `no_upgrade_before_the_node` pins
it, and the tests that upgrade gear mark the node known with
`upgrades_known` first.

`a_key_is_taken_ashore_put_in_the_desk_and_consumed_to_open_a_node` runs
the whole loop on the playtest ship at its spawn (two desks on the joined
deck, the station's second);
`the_benches_and_the_build_tab_wait_on_research`,
`the_checksum_notices_research_and_a_key_taken`,
`keys_are_on_four_friendly_desks_in_five_and_always_at_the_spawn`,
`tier_two_keys_lie_only_on_hostile_desks`,
`a_key_is_taken_at_a_held_dock_while_the_machines_stand`,
`unlock_wants_the_nodes_tier` and `no_upgrade_before_the_node` are the
rest; `save_round_trip_keeps_key_tiers` in `crates/ship` is the save
(`SAVE_VERSION` 14). `tier_two_probe` (`#[ignore]`, `--nocapture`) is a
probe rather than a test: how many tier-one keys a start system holds
and how often a tier-two desk is in it (66% of start systems on the
first fifty seeds, 61% of station-bearing systems, whatever the galaxy
type — the systems are the seed's, not the type's), the walk from a
port to the research desk plan by plan (24 tiles on a pod, 27–31 on a
cross or a spine, 33–37 on a ring or a comb, 42–58 on a hub), and that a
crew member left down at that desk is carried home by
`undock_for_probe` with the key still in the pack (`unjoin_rooms` takes
every crew Bim, `adopt` puts one off the deck at its bunk). In a run the
same crew member is **left behind** instead, and dead for it, when the
ship leaves the site (`leave_mission`; "The loop" below).

## The station's doors are in two rooms, and a lock is carried between them

`World::sync_doors` (September 2026), in `visit` after the visitors: for
each door of the residents' room (`Game::door_states`), its middle through
`Aboard::from_station` is a point on the joined deck and
`door_index_at` finds the deck's door there; whichever room's `changed`
flag is up has its lock copied to the other (`mirror_door_lock`, which
puts `Locker::Crew` or a `Body(usize::MAX)` — somebody's in the other
room, nobody's here — and clears the flag), and the residents' smashing
is mirrored onto the deck door as a bar (`mirror_door_smash`). So a lock
the crew set on the deck stops the station's people in their own room —
and is what they smash, `crates/game/CLAUDE.md` — a lock an enemy set
sealing itself in stops the crew on the deck (and the panel can lift it),
and the bar the crew watch is the smash in the room where it happens.
`a_lock_on_a_station_door_is_the_same_lock_in_both_rooms` pins it.

## A brownout is dark, and none of it is lethal

`World::run_brownout` (September 2026) is the second half of stage 6,
right after `run_power`: what `Power::brownout()` — draw over supply with
the batteries flat — does to the ship, now that the lamps draw
(`shipdesign::WALL_LIGHT_POWER` 25, `STANDING_LIGHT_POWER` 40;
`crates/shipdesign/CLAUDE.md` "Power is a column") and a brownout is a
thing a design can reach. **The lamps go dark**, and that is the whole
of it now:

- `bims::sight::Lamp::powered` is a second way for a lamp to give no
  light beside its health — `is_dark()` is out *or* unpowered, and it is
  `is_dark` the tile mask, the fields and the flicker read, while
  `is_out` stays what a bolt asks (an unpowered lamp is still glass to
  shoot) — and `Sight::set_lamp_powered` / `Game::set_lamp_powered` flips
  it, relighting through `lamp_switched` like a hit that puts one out,
  with no flicker. `sync_lamp_power` sets it for **the ship's lamps
  only**: every light part of the design, on the crew's deck
  (`lamp_index(&aboard, false, tile)`) and on the residents' mirror of
  the ship (`foreign = true`), to *on a live network and not browned
  out*. A station's lamps are furniture — nothing reads a station's
  power, its layout is not wired — and stay lit. It runs at the end of
  `restore_lamps`, so every step and every rebuilt room, and again the
  step the brownout turns so the dark lands with the event. The painter
  needs nothing new: `lamp_look` hands back a full health share and a
  level of nought, which `fittings::lamp_face` draws as dark glass,
  uncracked. **This is the alarm** — nothing else is drawn for a
  brownout.

What draws stops besides: `World::powered(kind)` is false for anything
not `essential` while the ship is browned out, so the benches take no
craft order and the research desk does no research. The other two
consequences — the hydroponic bay hibernating (`sync_bay_power`,
`Game::set_hydro_powered`) and the cold store's food spoiling
(`World::cold_store_out`, `spoil`, `SPOIL_STEPS`, `SPOIL_DIVISOR`,
`WorldEvent::FoodSpoiled`) — were the needs', and went with them in
feature 104.

`WorldEvent::Brownout` (56) and `PowerRestored` (57) are said once each
way off `World::browned_out`, the last step's answer, which is derived
and not hashed. Life support and the doors are `essential` and run on;
nothing here kills anybody, and there is deliberately **no veto on the
speed** for a brownout: the answer is that a brownout is expensive, not
fatal. `throttle_reactors_for_probe` **sticks** across
`on_ship_changed` (`probe_supply`), since every cargo change runs
`on_ship_changed` and a one-shot throttle came off with the first
crate. The tests:
`a_brownout_darkens_the_ship_and_stops_the_benches_until_the_power_is_back`
(the playtest ship, the lamps, the benches, the doors and life support,
then the power back), `a_short_overdraw_a_battery_covers_costs_nothing`
and `an_unwired_lamp_is_dark` (a standing light on bare deck at
`(12, 9)`; both blocks of one `#[test]`); `ship_lamps(&world)` beside
them reads the ship's lamps off the room by tile. Wiring the fixtures'
lamps moved `REFERENCE_CHECKSUM`. Out of scope, and next: reactor and
conduit damage, which is what makes a brownout happen *to* a crew rather
than by their own design.

## A lamp shot out is remembered by where it hangs, and is out in both rooms

`World::lamps: Vec<LampDamage { station: Option<u32>, tile, health }>`
(September 2026). A bolt lands on a lamp on the crew's deck — the one
deck bolts fly on — and the room takes it off the lamp
(`bims::sight::Lamp`, `crates/game/CLAUDE.md`); a room is built afresh
at every dock, undock and relayout and starts every lamp whole, so the
damage has to live here. `sync_lamps`, in the step right after `visit`,
drains `Game::take_lamp_changes` off the crew's room and remembers each
by **which design it hangs in and its tile there** — `Aboard::design_of(p)`
says whether a room point is in the foreign box (the station's on the
crew's deck, the ship's on the residents' mirror) and gives the design
point through the frame or the shift; `room_of(foreign, p)` is the way
back — then `restore_lamps` sets every remembered lamp on whichever rooms
it hangs in (`Game::lamp_at` by tile, `set_lamp_health`), which is what
carries a hit to the residents' mirror of the same lamp *and* what puts
it back on a fresh room; it is also called outright at the end of
`join_rooms`, `unjoin_rooms` and `relayout_room` so no frame draws a
fresh room lit. The record is in `world_checksum` after the keys —
length, then station or `u64::MAX`, tile, health to `HEALTH_GRID` —
and `REFERENCE_CHECKSUM` moved (to `0x_01a6_a8a4_d852_c023`). The painter
asks `World::lamp_look(station, tile)` a light part a frame — off the
crew's deck, the residents' room, or the record for a station out of
every room — for `fittings::lamp_face`.
`a_lamp_shot_out_is_out_in_both_rooms_and_stays_out_across_a_dock` shoots
a station lamp and a ship lamp out with `enemy_fire`, undocks
(`undock_for_probe`) and docks again (`dock_for_probe`, which sets the
docked state first, since `dock_at` does not);
`shoot_lamps_for_probe(n)` is `BIMS_LAMPS_OUT` in the app.

## A jump is another system, and `World::jump` is the one list of what a system is

`crates/world/src/jump.rs` (September 2026). A jump is a leg of a
trip, and nothing else since feature 104: `World::travel` calls
`World::jump(star, events)` — `pub(crate)`, and the only caller outside
the tests — when the site proposed is in a system a hyperlane away
(`TravelQuote::jump`), and the trip's length counts
`data::JUMP_CHARGE_MINUTES` (twenty game minutes) for the charge on top
of the leg. The flown jump went with the flown trip: `Command::Jump`,
the helm's charge (`ShipState::Charging`, `begin_jump`, `charge_jump`,
`jump_charge`, `hyperdrive_ready`), `WorldEvent::JumpFailed` and the
refusals `NotHolding`, `NoHyperdrive`, `NoSuchStar`, `SameStar` and
`NoLane`. A trip asks the quote instead (`travel_quote`: `TooFar` for a
star no lane reaches, `Jammed` for a jump inward out of a jammed
system; "Jumping along lanes" below), and no hyperdrive is asked for.
`jump` answers false, and moves nothing, for a star the galaxy has not
got.

`jump(star)` replaces `star_id`, `system`, `stations` (`Station::all_of`),
`surfaces`, `station_keys`, `residents` (closed, so its dead are
counted), and — for a system never visited — the lamps' damage, the
losses, the graves, the places visited, the machines' hold (`infested`,
which names this system's ids) and `discovered` (cleared; the trip
charts the whole system on arrival), and leaves the ship, the crew, the
hold, the sites on the deck, the research, the money and the hired hands
alone. Since feature 71 the system left is filed first and one visited
before is put back as it was left ("A system has a memory" below). The
jammer is settled (`settle_jammer`) before the landing point is picked,
so the two are never the same spot. The ship lands **holding** at
`jump::landing_point(&system)` — rings of sixteen bearings out from the
origin, the first point `data::JUMP_CLEARANCE` (four body radii) from every
node, deterministic off the system — pointing the way it was,
`Frame::Space`, and `WorldEvent::Jumped` is said; the trip then docks
it at the site. `home` is only home while `star_id == home_star`
(`stance`), since a new system reuses station ids; the checksum eats
`home_star` and `star_id`. `a_jump_puts_the_ship_in_another_system` in
`tests.rs` is the rule, calling `jump` straight.

`World::galaxy()` regenerates the `Galaxy` from `galaxy_seed` and
`galaxy_type`; the chart the app shows over the system map is
`lobby::Lobby` made from the same pair (`screens/game.rs`, the strip's
`Galaxy view`), with `here` and `target` marks the lobby draws for the
game — the lobby is the one thing that lists a system before the crew
have been there.

## A system has a memory, and a station's dead stay dead

`crates/world/src/memory.rs` (September 2026, feature 71). Two things
were forgotten before it: a station's room, dropped fifty tiles out
(`settle_residents`), was opened again on the way back with every one
of its people at their bunks, the dead included, so a garrison shot to
the last stood up again the moment the ship had gone a little way off;
and `World::jump` replaced everything of the world that was the
system's, so a jump away and back was the system as the generator
rolled it — the key back on the desk, the rocks back in the belt, the
enemy a stranger again. Both are kept now, as **counts and lists**,
never as rooms. (The garrison went in feature 104; what a station has
lost is its own people now, and the machines' hold on a station is
kept the same way, in `infested`.)

**`World::losses: Vec<Losses>`**, sorted by station id, is what each
station has lost: `dead` of its own people and `mercenaries` gone —
hired onto the crew, or dead. It is added to at **one door**,
`World::close_residents`, which every residents' room goes out by bar a
probe's: `settle_residents` out of range, `join_rooms` when another
station's room was open, a reopen (`reopen_residents`: the old crowd's
dead counted before the new stands, and the new `people_of` is the
fewer for them), the end of a mission (`leave_mission`) and `jump`. A
body is counted once it is not `is_alive` — dead, not out cold, since
one out cold wakes — told from a mercenary by `Residents::fee`. A hire
adds one to `mercenaries` (`memory::amend_losses`; an entry with
nothing lost is dropped, so two worlds that lost the same read the same
whichever rooms opened on the way); a dismissal back ashore took one
off until `dismiss` went with the free clock in feature 104.
**`people_of` and
`mercenaries_of` subtract them**, so a station opens again with its
survivors and an emptied one opens empty (`Residents::open` with nought
is a derelict's room, and `dock_for_probe` there docks at nobody). The
count is the *number* the room opens with, so the survivors are the
first `n` off the seed with their own kit again, not the same bodies —
nothing about where a survivor stood or what it was carrying is kept.
The *dead* are kept, though, since feature 85: see "The dead lie where
they fell" below.

**`World::memories: Vec<SystemMemory>`**, sorted by star, is every
system the ship has jumped out of as it was left — the per-system fields
lifted out as one struct: `station_keys`, the **stations'** `lamps`
(`station.is_some()`; the ship's own stay with the ship, which also
fixed a lamp shot out at one system's station 3 landing on the next
system's station 3), `discovered`, `losses`, since feature 85 `graves`
and `visited`, and since feature 83 the machines' `infested`. The
hostile list, the arena's `reinforcements`, an enemy's shelf
(`plunder`) and the mining `sites` were in it too, and went with what
they were for. `SystemMemory::overrun` clears the losses and the graves
of a system the machines take (feature 92). `jump` calls
`close_residents` (so the dead at a station
within fifty tiles are counted), then `remember_system` (filed under
`star_id`, replacing), then rebuilds `stations` and `surfaces` and asks
`recall_system(star)`: found, the fields are put back — `discovered`
never shrinks, and the trip charts the rest; not found, the fields are
reset the way they always were.
`station_keys` is by index into `stations`, and `Station::all_of` on the
same system gives the same list, so the indices hold.

**Both are in `world_checksum`**, after the lamps and the lost flag —
the losses whole, then every memory whole, each field the way the live
one goes in (`eat_losses`, `eat_graves`, `eat_nodes`; the mining sites
and the plunder's grids went in too, while there were any) — which moved
`REFERENCE_CHECKSUM` to `0x_28ce_5772_6470_76ab`; **`SAVE_VERSION` 15**,
`wire::PROTOCOL` 8. Nothing in the app changed: a memory has no words and
no picture, it is only what the next room opens with. `tests_memory.rs`
is the rule: `a_station_s_dead_stay_dead_when_its_room_is_closed_and_opened_again`
(two dead, then all, then docked at an empty station),
`a_jump_away_and_back_finds_the_system_as_it_was_left` (every field, on
the jumper, and the checksum), `a_mercenary_hired_is_not_there_to_hire_twice`
(on the combat ship, as the hire test is).

## The dead lie where they fell, and the map says where you have been

`crates/world/src/memory.rs` again (September 2026, feature 85). The
count above kept a garrison shot to the last from standing up again, but
the deck it was shot on was swept: the room is built afresh at every
open, so the crew walked back into a station an hour after a fight and
found nothing but fewer people. Now the bodies stay.

**`World::graves: Vec<Grave>`**, sorted by station and, within one
station, in the order the bodies stood in the room, is every body lying
on a station's deck: `x`/`y` in **the station design's own units** (what
`Aboard::position` answers, joined or not — the offset is already off
it), the `gear` still on it, the `look` it had and whether it was
`hired`. The body itself is not kept: a `bims::bim::Bim` is a room's, and
`Grave` is what a room can be built over.

**One door each way.** `close_residents` walks the room it is closing
and files *every* dead body it finds — `memory::set_graves` replaces that
station's whole run, since the room is the whole truth about its deck —
and `Residents::open` takes the station's graves and lays them out on the
end of its people: `count + mercenaries + graves.len()` bodies, the
coverall, the look and the gear put on each of the last of them, and
`Game::lay_out_dead(who, at)` (`crates/game`) to stand it on the spot and
kill it outright — no tick waited for, nothing in anybody's diary, the
bunk given back. `World::open_residents` is the one call, so no open
anywhere can forget them. **The dead are not people**: they are past the
bunk cap, no fee is priced for them, and `Residents::grave: Vec<bool>`
flags them so `close_residents` does not count a body into `losses` a
second time — that flag is maintained beside `down`/`fee` at the places
those lists grow, shrink or shift (`visit`, the hire, the wave
truncation; the dismissal was a fourth until feature 104). A body laid
out is `down`, `xp_down` and `xp_dead` from the first step, so no
`EnemyDown` is said for it and nobody is paid for it twice. A raider's
dead went with the raider and nothing was filed for one, until the
raiders themselves went in feature 104.

What the crew took off a body stays taken — the grave carries the gear
as the room left it, so a looted pack comes back empty — and what is on
a body is in `world_checksum` (`eat_graves`, `eat_gear`: the station,
the position on the thousandth grid, `hired`, then what it wears, the
gun in its hand and its pack cell by cell). The `look` is not, for the
reason no other body's look is.

**`World::visited: Vec<Node>`** is the other half: the nodes of this
system the ship has actually been at, sorted by `node_key` the way
`discovered` is. `mark_visited`, off the end of `settle_frame`, is the
one door, and `arrive_at` asks it again the moment a trip ties the ship
up — the frame already knows what the ship is *there for*. It asked as
well that the ship had stopped, while a trip flew into its target's
frame from the moment it began braking; nothing flies since feature 104
and the check went with it. `World::stars_visited`
is the galaxy's half and keeps no list at all: a star jumped out of has
a memory filed under it, and the one the ship is at is the one it is at.

`ship::world_paint::paint_map` draws a **tick** at the upper-left
shoulder of every visited node (`paint_tick`, `VISITED`,
`TICK_SHOULDER`) — the upper-right one is a belt's pickaxe and a
settlement's pad, and the name is written under the icon — and the
galaxy chart rings every visited star in the same grey
(`lobby::preview`'s `Marks::visited`, set from `stars_visited` in
`screens/game.rs`). A tick is a shape nothing else on either map draws.

`REFERENCE_CHECKSUM` moved to `0x_f766_3dcf_74a2_5cbf`; **`SAVE_VERSION`
23**, `wire::PROTOCOL` 15. `tests_memory.rs`:
`a_station_s_dead_lie_where_they_fell_when_its_room_opens_again` (the
spot, the emptied pack, and counted once over two closes),
`the_dead_stay_on_the_deck_across_a_jump_away_and_back`,
`a_settlement_s_dead_lie_in_its_street_too` (a planet's town is a
station like any other here, and `World::lay_graves_for_probe` —
`BIMS_GRAVES=n` in the app — is how a fight's aftermath is staged
without the fight), and `the_map_marks_where_the_ship_has_already_been`
(docked, the belt held at, filed by the jump, and sorted). A spot goes
through the room's own `f32` on the way out and back, so it returns
within a ten-thousandth rather than exactly — inside the thousandth the
checksum rounds to, and it settles after the first round trip rather
than drifting.

## A station lays a few comforts, after its lamps

`build_layout` puts five comforts down (September 2026): a big plant in
the middle of the hub at `(mid, mid)`, a small plant a tile in from the
far south corner of the mess and of the rec room, and a picture on the
north wall of the quarters and of the rec room, turned to it with
`wall_light_rotation` like a lamp. **After the lamps**, so a comfort
never takes a lamp's tile — `put` skips a taken tile, and the lamp
stays. Only the hub's plant blocks a walk, and it stands in open deck
where the inflated grid passes on every side; the other four are walked
past or under, so `a_station_is_a_place_the_room_can_live_in` and the
walkability test cover them without a change. The layout is not in
`world_checksum`, so `REFERENCE_CHECKSUM` did not move. A comfort lifted
a Bim's surroundings, which were the needs'; since feature 104 it is a
picture and a solid, and nothing more.

## A planet's surface is a settlement, and landing is a docking

`crates/world/src/surface.rs` (September 2026, feature 52). A rocky
planet or an ice world (`surface::landable`) has a **settlement** on it,
and the settlement is a `Station` like any other — `Plan::Surface`, laid
out through the same `furnish` on a `Floor` of its own — found through
`World::station` by `surface_id(body)` (`SURFACE_BASE | body`, well
clear of the generator's ids; `surface_body` reads it back). That is the
whole trick: `dock_at`, `join_rooms`, `Residents::open`, trade at the
desk, `people_of`, `hire`, `apply_stances`, a machines' hold and a
town's defence — all of it works on the surface untouched because to the world the surface
*is* a station. `World::surfaces` holds one `Surface` a landable body,
in body order — the roll only: seed, side, shelf, off a stream of its own
(`Purpose::Settlement`, from the galaxy seed, the star and the body; the
generator draws nothing from it and the galaxy checksum is what it was).
The **layout is built the first time it is asked for** (`Surface::station`,
a `OnceLock`): a surface is `SURFACE_SIDE` (96) tiles of deck and twenty
thousand parts on it, and a world opens every station of its system at
once, so the towns wait until somebody lands. Since feature 54 the roll
also carries a **biome** (`Biome`: `Desert = 0`, `Temperate = 1`,
`Arctic = 2` — an ice world is arctic, a rocky planet rolls desert or
temperate evenly off branch "BIOME") and a **population**
(`SURFACE_POPULATION`, 5 to 30 inclusive since feature 66 — 10 to 50
before — off branch "POPL"), both
serialised on `Surface`; `Station::population` carries the number —
the plan's for a station, the roll for a surface — and
`Station::residents()` answers it. `Plan::residents(Surface)` is the
*most* a town holds, which only `layout` and the tests ask. `a_landable_body_has_a_settlement_…`
pins that a world opens with none built.

Its side is rolled like a station's — `Surface::hostile`, off the
settlement's own stream, at `worldgen::data::HOSTILE_SHARE`, pinned at
`0.18..0.42` over the reference galaxy — and read the way a station's
roll is: never as a stance. Until feature 104 it went on the world's
`hostile` list with the stations' ("planets, like stations, can be
hostile or friendly"); now every town is a stranger's, or the machines'
(`World::stance`), and the map rings a planet by that stance
(`world_paint::paint_map`) as it rings a station. What still reads the
roll is `spawn_with_ground`, below.

**The plan is a town** (`surface::floor(side, biome, population, seed)`,
feature 54): the whole build area bar its rim is ground — `Floor::open`,
**no skin**: every tile deck — and since feature 55 the deck's edge is
not the world's, see "The town stands on a plain" below — and since
feature 66 **built like a fort**: a `Wall` on every outermost tile of
the deck (`FIRST` and `LAST`, through `Floor::walls` like a partition —
not `OutsideWall`, which the test pins at none), the port at the middle
of the west wall, where the **pad** is (the ship docks airlock to gate
exactly as at a station, so `berth`, `join` and `join_mirror` are what
they were; the airlock's two tiles are left out of the wall), and two
**gates** — the west cross street's six columns (`GATE_X0`,
`GATE_WIDTH`) left open in the north wall and in the south, a pier of
wall `GATE_PIER` (2) deep standing inside either side of each and a
standing light beyond each pier — so the cross street runs out through
both onto the plain and nothing else does: every other street ends at
the wall. The gates are openings, not doors; what closes them is later.
By the pad, the same in every town: the **watch house**
south of it, a small square with the sensor dish on its roof
(`Floor::array`), its door towards the pad, two sandbags before it and
the **guard's post** (`GUARD_POST`, `(5, SURFACE_SIDE / 2 + 8)`) between
them; the **trading house** north of it — the trading hall (the reactor
room: desk, generator, life support, batteries) with its door onto the
yard, the research room (the research desk and **one** run of trays; it
is eight rows inside so `furnish` fits no second) onto the west cross
street, the store onto the main street. Three streets run east from the
pad — the **main street** eight wide on the pad's rows, a north and a
south street six wide — crossed by two more, the east one three tiles
past the hall wherever the hall ends; the streets are what a Bim walks
out along, and they run to the wall. Between them the lots:
the **gathering hall** on the main street in the middle (the mess, with
the galley along its north wall and tables in `Floor::mess_columns`
columns four tiles apart — two at the least, since the galley is six
tiles of fittings — three rows deep, a chair for everybody — the
`TooFewChairs` error is asked at the population); **bathhouses** (a
toilet, a basin and a shower; one for every twelve) and **houses** (two,
three, four or — in a big town — six bunks, the quarters' pattern: a
column of bunks at `R180` every three rows, the door two tiles in from
the corner, a picture or a plant in some) poured along the streets'
frontages nearest the middle first with a rolled gap and setback each,
until there is a bunk each and two over for mercenaries; and **food**
beyond the east cross street and along the south — blocks of four
`PartKind::Field` strips (six wide, a row every three so the worked row
and the row behind are clear; a strip for every two people, the rule
from when a field grew at half a bay's pace) or, on an arctic world,
**greenhouses** of three hydroponic runs each with an aisle down the
east side, a bay for every four counting the research room's. What
each lot leaves is a frontage for more houses. Twenty standing lights
along the streets and in the yard, a big plant on the main street
before the hall, and a wall light in every building (`Floor::lit`).
Everything the standard rooms do not lay — the extra bunks, fittings,
bays and fields — goes through `Floor::extra`, placed **after the
comforts and before the standing lights**, so it is laid clear of the
lamps' tiles (the inner corners and every sixth tile along a wall): an
extra on a lamp's tile is dropped, which is why a house's inner height
is `3 × bunks + 1` (the corner under the last bunk stays free).

**The wild** (`surface::wild`, `Floor::wild`, run last of all) is
everything the town is not, up to the wall: a scatter thinning towards
the town by a breadth-first distance from the buildings and fields
(`rates`; the forest ring at the deck's edge with gaps in it went with
the wall, feature 66) — temperate: clumps of `Tree`s, `Shrub`s, a lake
of `Water`, a few `Boulder`s;
desert: `Boulder` lines for cliffs and outcrops, `Shrub`s the painter
draws as cacti, one oasis pool with palms; arctic: rock outcrops, a
frozen lake, firs, hardly a shrub. Nothing grows within one tile
(eight-neighbour) of a use spot, a door's tiles or the two beyond either
face, the post or a standing light, nor in a building, a street, the
yard or a field lot (`Floor::clear`). Then a **four-way flood from the
pad's inside tile** over every tile nothing blocks (a door is open)
finds what can be reached, and every free tile it did not reach is
filled with the nearest wild kind, so there are no pockets: a one-tile
straight gap is walked by the room's navigation and a diagonal-only gap
is not, and a four-way flood says exactly that. Every roll is an
integer or a `Rng` draw in tile order — no floats decide anything — so
a town is the same town on every machine. Building one is
`station::Placer` (below), and `layout_surface(seed, biome, population)`
takes about four milliseconds; `layout(kind, Plan::Surface, seed)` is
the biggest temperate town, for the map and the tests.
`Plan::Surface` is **not on `Plan::ALL`**: it is a planet's, never
rolled, so the tests that walk every plan never see it;
`a_town_is_a_place_the_room_can_live_in_and_can_be_walked_in_every_biome`
(`tests_surface.rs`) runs the checks — `validate`, the port, the desks,
the bunks, the chairs, the food, the wild's keep-outs, and the
walkability contract from the pad to every use spot, the post and every
walkable tile, by `Nav::can_reach` since a route per tile over nine
thousand tiles is minutes, and the fort: a `Wall` on every outermost
tile of the deck but the pad's two and the gates', which are walkable —
on every biome at populations 5, 17 and 30 on two seeds. `BIMS_NAV_MAP=Surface` prints the plan.

**`station::Placer`** is what `furnish` builds through now: the design
with the occupancy of its four layers kept beside it, `put` refusing
exactly what `shipdesign::design::place` refuses (bounds, the layer
taken, what it requires missing, no wall at a hung part's back) and
`take` removing an object-layer part, ids and order as `apply` gives
them — a town is twenty thousand parts, and `apply` rebuilding the grid
per edit was the better part of a minute. It must answer **identically**
to a run of `apply`s so no station's `design_hash` moves;
`furnish_through_the_placer_is_furnish_through_apply` replays a hub, a
pod and the arena through `apply` (`Placer::replay`, off the log of
attempts it keeps under `cfg(test)`) and asks for equality part for
part. The lamps' `wall_light_rotation` is `Placer::hung`, the same rule
on the occupancy. `REFERENCE_CHECKSUM` did not move for any of this.

**Daylight.** A town is under a sky: `Residents::open` on a surface sets
`Game::set_daylight` over the whole design, `Residents::join` over the
station's area at the mirror's shift (not `station_box`, which is the
ship's there and the ship has a roof), `Residents::unjoin` over the
design again, and `join_rooms` over `Aboard::station_box` on the crew's
joined deck (`Aboard::daylight_over_station`); `unjoin_rooms` sets
`None` on the crew's fresh room. `Aboard::leave_the_station_s` drops
`more.fields` in the station's box as it drops the bays, so a town's
fields are furniture on the joined deck. `a_settlement_s_ground_is_lit_by_day` lands,
stands James on the pad and asks `seen_at` of a tile of the main street
beyond every lamp's reach and beyond `DARK_RANGE`, with the sky and
without — after `Game::observe`, since a step alone does not look.

**The guard.** The town's people are its population and the first of
them (`surface::GUARD`) is the guard. It used to be posted at
`GUARD_POST` behind the watch house's sandbags (`Residents::post_guard`,
after every open, join and unjoin of a surface's room) and to go off to
eat and sleep and come back; that was the needs', and the post went with
them in feature 104. Since feature 102 it keeps a round like everybody
else (`Residents::deal_roles`: a town's first walks between its gates
and its pad), and when the crew defend the town it takes arms with the
mercenaries (feature 94). `GUARD_POST` is still the plan's — the watch
house, the sandbags and the wild's keep-out are laid round it.
`Residents::open` cuts the room at the bunks, and a town has its
population's and two over.

**Landing is arriving.** A trip to a planet's site ties the ship up at
its settlement in one go (`World::travel` → `arrive_at`:
`ShipState::Docked { station: surface_id }` and
`Frame::Local(Node::Body(body))`), and `World::landed()` is the body.
`frame_candidate` maps a surface's station id to `Node::Body`, so the
view is the **planet's**. The flown landing — `Command::Land`,
`can_land`, `above`, the descent over `LAND_MINUTES` and the climb over
`LIFT_MINUTES`, `Landing`/`Landed`/`LiftedOff`, `landing()` and
`lifting()`, `landing_for_probe` and `BIMS_LANDING` — went with the
flown trip in feature 104, and so did the hop from an orbital's berth to
the point over its planet that made landing from one possible. What that
hop left and is kept: a **holding** ship stays in the frame it is in
while it is within the exit radius (`within_exit_radius`), rather than
being handed to whatever discovered node is nearest — the orbital is
nearer than the planet's centre from most of its orbit — and
`undock_for_probe` sets `Frame::Space`, since a probe that puts the ship
somewhere else next wants it to start from nowhere.
`settle_residents` never opens a surface's room from range
(`nearest_station` walks `stations` alone); `unjoin_rooms` finds the
residents' station through `station()`, so a surface's room is unjoined
like a station's. `land_for_probe` sets the ship down at the first
landable body's settlement without a trip — `test_planet`,
`droids_planet`, `defense` and `BIMS_AFIELD` open through it — and
`spawn_with_ground` is `spawn_anywhere` kept to the systems whose
**first** landable body rolled friendly, the one `land_for_probe`
picks, so `nix run .#test_planet` (`test` landed:
`ship::session::pick_ground`) opens where the generator put a friendly
town; `a_roll_picks_a_dock_in_a_system_with_friendly_ground` (a block of
`a_roll_picks_a_lived_in_dock_anywhere_and_one_with_friendly_ground_when_asked`)
pins it, reading both dock lists off `every_system()` once because a
roll generates the galaxy every call.

**The picture** is `ship::world_paint`: landed, the backdrop is the
planet's ground (`ground`, coloured by biome) in place of the void, no
stars, no planet in the sky, a paved **pad** under the hull's box
(`pad`), and `stations` draws the settlement — chained onto the list,
since it is not in `World::stations` — and nothing in orbit. The
descent's growing disc (`LANDING_GROWTH`), the blackout over a landing
and the strip's **Land** button went with the flown landing; a
settlement is named for its planet ("Ice world 1 settlement",
`node_name`).

## The town stands on a plain, and the plain is walked on windows

Feature 55 (September 2026). The settlement's deck is no longer the
edge of the ground: a landed ship stands on a **plain**
`bims::terrain::EXTENT` (10 000) tiles a side, centred on the deck's
middle, and the deck is a patch of it. What is beyond the deck is
**terrain** — `bims::terrain::Terrain`, a function of the planet's seed
(`Surface::terrain`: the map seed salted, the biome and
`SURFACE_SIDE`), read at any tile in the *station's* tile frame and
never stored whole: integer value noise (three octaves off a hash, a
smoothstep in fixed point, no floats anywhere) thresholded per biome
into `Ground` — `Open`, `Tree`, `Shrub`, `Boulder`, `Water`, `Cliff`,
`Forest`. Everything but open blocks a body; a cliff and a forest stop a
line of sight, and nothing smaller does — a lone tree throwing a
sixty-tile shadow was a picture of stripes. A `CLEARING` (48) tiles about the deck is
open ground with singles on an even lattice (never two touching, so the
scatter can wall nothing off); the outer `RIM` (48) tiles are cliff with
scree before it, and past the extent everything is cliff. The tests in
`terrain.rs` pin the clearing, the rim, the determinism and that a
four-way flood from the deck reaches most of a thousand-tile square in
every biome, with every kind of ground in it. The thresholds are the
`Rules` tables there; `calibrate::histogram` (ignored) prints the
noise's percentiles for tuning them.

**The room on a plain.** `World::join_rooms` hands `Aboard::joined` the
terrain when the station is a surface, and the layout is
`bims::aboard::layout_of_on(design, Some(plane))`: the room's box is the
joined deck's floor box plus `DECK_MARGIN` (12) tiles of ground every
way — `Room::interior` and `bounds` alike, so the sight mask, the light
map and the filth cover that and no more — a tile in it with nothing
on it is ground to walk rather than void to keep off (the hull's skin,
a structure tile with no floor, is still solid), and what the ground
blocks or stops sight at in the margin goes into `others` and `opaque`
as row runs (`Plane::solids_in`, `opaque_in`). The room keeps the plane
on `Room::plane` — a `bims::terrain::Plane`: the terrain, the station's
frame in the room's tiles (`Joined::station_origin/ex/ey` in whole
tiles, so a room tile is the station's by two projections), a cache of
the chunks looked at (32 tiles square, not saved), and the fog. On a
relayout the room keeps its own plane and takes the new deck box
(`Room::relayout`). The whole box is under daylight
(`Aboard::daylight_over_station`) and everything outside the hull's own
box is foreign (`Game::set_foreign_outside`, `Sight::set_foreign_outside`),
so the ground beside the ship is black until looked at like the town.

**Beyond the box a body is afield**, and walks a window of its own:
`Game::refresh_afield`, at the top of every step, sets
`Character::afield` from where each body stands (outside the room's
box) and keeps a [`Nav::outside`] — `OUTSIDE_RADIUS` (100) tiles every
way, a cell a tile — for every body that is afield *or bound beyond the
box* (`Character::far`), over the ground's runs and whatever of the
deck's solids and locked doors fall in it, built again once the body is
`OUTSIDE_RECENTRE` tiles from its middle (`Maps::afield`, one a body;
`Game::afield_blockers` for the push-out; neither saved). A route is
`Game::plan_route`: on the deck's grid when both ends are in the box,
else on the window, through `Character::walk_to` — the route to the
free cell nearest the target *as the grid clamps it*, and when that
ends more than a tile short of the target, `far` keeps the rest: the
body is not `arrived()` until `far` is `None`, and
`Game::continue_far_walk` (top of `tick_bim`) plans the next leg from
wherever the last ended, on the window that has by then been built
again about the body. A leg that ends where the body already stands is
the walk over — as near as the ground allows. A leg with no route
gives the walk up and puts the chain down, as `unstick` does; `unstick`
and `return_to_post` plan on the body's grid (`Game::nav_for`), and a
post beyond the window is the point itself, found when the walk gets
there (`nearest_stand`). `move_body` holds a body on a window walk to
the window and its solids rather than the box, or the box's edge would
hold it in. A player order onto the plain has no door to work
(`order_move_for`); `Task::enter` plans a need's walk the same way, so
a hungry body a thousand tiles out walks home leg by leg. `is_outside`
is still the suit and the airlock, and nothing else;
`Character::off_deck` is either.

**Seeing the plain** is `Plane::observe`, from the same eyes as the
mask and right after it (`Game::observe_from`): every tile off the
room's box within `VIEW` (60) of an eye whose line from the eye's tile
crosses nothing opaque — the plain's own, and on the deck the mask's
cells (`Sight::opaque_room_tile`), doors and all, so a body indoors
sees nothing of the plain through the walls and the ship's hull throws
a shadow over the ground behind it — and every tile beside one of
those, which fills the pinholes and softens the stepped edge a trace
to tile middles leaves. Traced tile by tile over a window of flags,
only when an eye has crossed a tile. Inside the box the mask and the
light map have the same range on a plain (`Sight::set_range`, set by
`Room::from_layout` and carried over a relayout): a lit tile further
than `VIEW` is not seen, and the picture's rays stop there too — which
is also what keeps them from streaking, with `RAYS` at 4096, across a
box that is bigger than any deck was. Seen is a bitset a chunk
(not saved), explored the same (saved); `veil_at_room` is what the
painter draws — nothing, grey or black.

**The picture** is `world_paint::plain`, drawn after the backdrop and
before the town: every tile without floor over `world_paint::plain_window`
— within `VIEW` of the camera's middle and within the canvas (the
window is the world that is loaded) — in the room's frame turned with
the ship — water, cliff and forest as
runs along a row (a cliff with a lip along its top and a shadow at its
foot, a forest with crowns on one tile in three), a tree, a shrub and a
rock as the town's are (`fittings::part_in`, through a placed part at a
`PLAIN_SHIFT` so the tile is unsigned), the ground's decoration on the
rest. **The fog over it is not in the shapes** (feature 67, September
2026): it is the plain's **picture**, `bims::terrain::Plane::picture`
— the same ray march as the deck's light map (`sight::march_rays`,
eight pixels a tile), a chunk of the room at a time (32 tiles, in the
room's frame, not the station's chunks the ground is cached by), so a
cliff's shadow has the cliff's edge and not the tile grid's steps —
where it used to be black and grey rectangles a run of tiles at a
time, the "old pixelated" look beside the smooth deck. `ship::Game::
picture_the_plain` asks for it once a frame in `Session::render`, after
`hold_view_to_the_ground` and before the paint, over the same window
the plain is drawn on (the room does not know the camera, which is why
it is not in `Game::render`); a chunk the window touches is composed —
wholly if it has no map, else over the box the marches moved under it
— and one the window leaves lets its map go and keeps its memory. The
app draws each as its own `fogmap::FogTexture`, cut into the pieces of
the chunk that lie off the room's box (`world_paint::plain_fog_on_screen`,
`FogPiece`: at most four rectangles), so it never lies over the light
map; every picture carries a `PICTURE_APRON` of the next chunk's
pixels that is composed and never drawn, so the app's blur reads
across the seam and a shadow does not kink there. A body's view holds
until its eye has moved half a pixel or the deck's cells have
(`Sight::cells_version`: doors, tall parts, lights). Every pixel a ray
has reached is remembered, a bit a pixel per chunk, and none of it is
saved: a plane read back starts its pictures from the tile rule's
memory as it stood then (`Plane::seed`, whole tiles), and from nothing
otherwise — the tile rule reaches half a tile past the rays with a
stepped rim, and a picture must not take that on.
`the_picture_is_marched_a_chunk_at_a_time_and_its_aprons_agree` in
`terrain.rs` pins the lot. The fog
over the box is the room's light map as before. The camera on a planet
is held at or above the scale where the canvas's *nearer* edge is `VIEW`
tiles from its middle (`Game::hold_view_to_the_ground`, once a frame;
`Camera::set_floor` — applied in `zoom` as well as `settle`, or a
notch of the wheel past the floor shoved the view sideways every time),
and the plain is drawn out to `PLAIN_DRAWN` (twice `VIEW`) to cover the
corners. `BIMS_ZOOM=0.3` zooms the game view about its middle once it is
fitted, and `BIMS_AFIELD=1` lands the simulation with the crew member
walked out west of the ship and a minute gone by — the two together are
how the plain is looked at (`BIMS_AFIELD=1 BIMS_ZOOM=0.3 ./hidden
target/debug/bims test_planet` is that on a planet chosen at random);
`a_landed_picture_as_svg` (ignored, in `crates/ship`) dumps the same
picture as an SVG, without the fog.
`the_ground_beyond_the_town_is_a_plain_the_crew_walk_out_on_and_back`
(`tests_surface.rs`) walks a crew member two hundred tiles north of the
town — past the margin, past one window — and back, with its errands
off, and asks the fog what it saw. `SAVE_VERSION` went to 3 for
`Room::plane`.

## Raiders and an enemy's shelf went with the human enemies (feature 104)

Two systems were the human enemies' own, and went with them in feature
104. **A raid** (`raid.rs`) was a hostile ship that came alongside a
crew holding in space: a `Station` the world built when the raid was
due, with an id of its own — `RAIDER_BASE | n`, bit 29, clear of the
generator's ids and of `SURFACE_BASE`'s bit 30 — a hull laid by hand
(`Plan::Raider`, `furnish_raider`), its boarders counted like a garrison
to `BOARDERS_MAX`, a warning with a countdown on the strip, and the
ship's locked airlock forced. That is why the station ids made outside
the generator have a gap in their bits: `SURFACE_BASE` (bit 30,
`surface::surface_id`) and `jammer::JAMMER_BASE` (bit 28, the derived
jammer) are still given out, bit 29 no longer is, and
`jammer::jammer_star` masks `SURFACE_BASE` alone now that no id carries
the raider's. **An enemy's shelf** (`plunder.rs`) was a grid laid out off a
hostile station's seed the moment the rooms were joined there, taken a
stack a click (`Command::Plunder`, `WorldEvent::Plundered`) and
remembered with the system. `World::raids`, `World::plunder`, their
events, refusals, constants and checksum lines, `tests_raid.rs`,
`tests_plunder.rs`, and the app's raid warning, plunder window,
`BIMS_RAID` and `BIMS_ARMOURY=plunder` went too; the codes they held are
left free. `git show needs-sim-final:crates/world/src/raid.rs` (or
`plunder.rs`) reads either again.

**The station's shelves are still told from the ship's**, the way its
research desk is: `World::station_shelves()` is every
`Container::Shelf(i)` on the joined deck whose frame stands in
`station_box`. They are **not containers of the hold** —
`container_takes(Shelf(i), _)` is `false` for them, so a crew member
beside a station's shelf reaches nothing of the ship's, and nothing of
the crew's is stowed onto a station's shelf. Before the plunder a
station's shelf on the joined deck was a window onto the ship's own
storage. The app's `Hold::station_shelves` keeps them out of the
container windows and off the Nearby strip; a click on one opens
nothing — a station is traded with across its desk.

## The engineer: a class, its levels and its deployables (feature 74)

`crates/world/src/class.rs` and `deploy.rs`; the world's side is the
"classes, experience and the engineer's deployables" `impl World` at
the foot of `world.rs`, and `tests_engineer.rs` is every test the spec
asked for (A to E and the hotkeys' defaults, which are `keys.rs`'s).

**A class is a slot's, progress is a crew member's.** `World::classes`
(`Class::None`/`Engineer`, one a player) is set before the game opens
— `ship::Session::set_class` in the yard, `World::set_class` as the
world opens — and by `Command::SetClass` until `undocked_once`, which
the step sets the first time the state is not `Docked`; after that it is
`Refusal::ClassLocked`. Choosing Engineer puts its **charges** in the
Bim's pack — `SANDBAG_CHARGES` (three) `SandbagKit`s and
`SENTRY_CHARGES` (one) `SentryKit` — through `Game::give`
(`give_engineer_kit`, `World::ENGINEER_START` being the two counts in
one table), and choosing None takes them out again
(`take_engineer_kit`). Since feature 88 those are not crafted at all:
they are what the cooldowns fill the pack back up to ("Charges, not
crafting" below). A probe or a test that wants more than the class deals
asks `World::give_kits_for_probe(who, kit, n)`.
**The pool is untouched** — since feature 75 a
class owns abilities and never money (`class::contribution` and
`economy::starting_pool_of` are gone). `World::progress` is a
`Progress` a crew member — `xp` and `picks: Vec<(level, Side)>` — and
only a player's own ever gains (`award` returns early for `Class::None`);
`casualties` resets a dead crew member's to default. **A pick is a level
and a side, and which talent that is depends on the class**:
`pick_at(class, level)`, `talent_of(class, level, side)`,
`Progress::{has, pick, talents}(class, ..)` all take it, and
`World::has_talent(who, talent)` answers only for a talent of the
crew member's own class (`Talent::class`). `LEVEL_XP`, `level_of`,
`is_pick_level` (the same shape for every class: fixed at one, three and
seven), `Progress::{level, to_next, gain, picked_at, pending_pick}` are
the rules, with every multiplier a named constant beside them
(`SITE_FOREMAN_EFFORT` … `SENTRY_MARK_THREE_DAMAGE`, then the soldier's).
`pick_talent` is the command: `NoClass` (52) without a class, then
`Progress::pick`'s `NotAPickLevel`/`LevelNotReached`/`AlreadyPicked`. A
`LevelUp` (68, `who`, the class's code and the level) is said once a
level by `award`; `TalentPicked` (69) by the pick. `class::can(class,
Ability)` is the one rule a class gates anything by — `Deploy` the
engineer's, `Brace` and `Throw` the soldier's — and no job, errand, site
or weapon ever asks it.

**Experience is given in the step, after `visit`.** `experience`: while
the residents' room is hostile and the rooms are joined, every body of
it — the machines, every human being friendly since feature 104 — that
is down (dead or `is_unconscious`) or dead and not yet flagged in
`Residents::{xp_down, xp_dead}` is `XP_ENEMY_DOWN`/`XP_ENEMY_DEAD` to
every classed crew member within `VICINITY_TILES` (fifty) of where it
lies on the joined deck (`Aboard::from_station` of its position;
`in_vicinity` is the rule, alive and aboard) — so an enemy down on an
unjoined deck is nobody's, and a crewmate down is never counted. A
machine is destroyed outright, so it is both at once, the same step
(`a_machine_destroyed_is_experience_once_each_to_the_classed_crew_in_range`
and `a_machine_destroyed_on_an_unjoined_deck_…`, ported to machines in
feature 104).
`finish_build(site, who, ..)` — `Room::built` carries the builder now —
gives `XP_BUILT` to every engineer within the vicinity of the builder
(`award_engineers_near`), and `finish_deploy` the same at the layer,
unless the kit was a re-used one: `World::reused_kits` counts, a crew
member each, the kits packed up and not laid again, and a
deploy spends one of those first. `mercenaries` and the crew nobody
steers have `Class::None` and are neither counted nor paid.

**A deployable is a room object.** `Deployable { id, kind, owner_slot,
deck: Ship | Station(id), tile, health }` on `World::deployables`
(id order, `next_deployable` climbing), never in the design.
`Command::Deploy { slot, kit, x, y }` names a **room tile** of the crew's
room — the app's `room_point` under the pointer — and `can_deploy` is
the check the app greys a press with and the command makes: fit to act
(`OutOfReach`), an engineer (`NotAnEngineer`), the kit in the pack
(`NoKit`), a sentry from `SENTRY_LEVEL` (`NoSentryYet`), and a
tile `Game::deploy_tile_ok` takes — walkable floor, not a door, a stand
beside it — with nothing laid on it (`CantDeployThere`). There is no
limit refusal: since feature 88 a sentry over the charges destroys the
oldest instead. The errand is
the room's (`Game::deploy`, `Kind::Deploy`, `crates/game/CLAUDE.md`) for
`deploy_minutes` — the kind's, halved by *sandbagger* or *quick build* —
and the kit leaves the pack only when `Room::deployed` says it was laid:
`finish_deploy` reads the deck and the design tile off `Aboard::design_of`
(the station's while the tile is in the foreign box, `Deck::Station` of
the berth), lays it with `lay` — sandbags at `sandbag_health` (the base
plus *reinforced sand*), a sentry at `sentry_health` for the owner's
talents — and *bulk bags*
a second tile of sandbags on the first free neighbour. `PackUp` wants
the engineer within `REACH` of it (`deployables_in_reach`) and is the
kit back (`PackFull` if not) and one more `reused_kits`.
Events `Deployed` 70, `PackedUp` 71, `DeployableLost` 72.

### Charges, not crafting (feature 88)

An engineer does not make its kits and a sentry does not carry
ammunition. **Nothing in the game does**, yet, which is why *deep
magazine*, *field refit* and *salvage* — three talents about a thing an
engineer no longer does — went with the shots: `Deployable::shots`,
`Sentry::shots`, `Game::take_sentry_shots`, `Command::Refill`,
`WorldEvent::Refilled`, `Refusal::SentryLimit`, `SENTRY_SHOTS` and
`SENTRY_REFILL_METAL` are all gone, and the four talents' **codes are
kept** (`Talent` 0, 5, 7 and 10 are `ReinforcedSand`, `EnhancedOptics`,
`BetterArmour` and `ExtraBags` now) rather than renumbered.

- **`World::kit_charges(who, kit)`** is how many kits of a kind an
  engineer's pack fills back up to: `SANDBAG_CHARGES` (three) plus
  *extra bags*, and `SENTRY_CHARGES` (one) — nought under
  `SENTRY_LEVEL` — or `SECOND_SENTRY_CHARGES` (two) with *second
  sentry*. Nought for anybody who is not an engineer.
- **`World::kit_timers`** is one `[Option<f64>; 2]` a crew member, the
  clock minute each kind's current cooldown began, and it is **in
  `world_checksum`** after the re-used kits. `World::restock_kits`, a
  step before `hand_the_room_the_engineers`, is the whole of it: short
  of its charges, a timer runs; run out, one kit goes into the pack
  (through `Game::give`, so a pack with nowhere to put it keeps the
  timer where it is and the kit lands the step room is made) and the
  timer starts again while it is still short. `SANDBAG_COOLDOWN` is 45
  seconds of the clock and `SENTRY_COOLDOWN` 60, read the grenade
  cooldown's way; `World::kit_cooldown_left(who, kit)` is the reading.
  **It runs in combat as out of it** — an ability's cooldown, not the
  dressings' restock — and conjures nothing out of the hold, a kit being
  the ability itself.
- **The sentry charges are the world limit.** `sentry_limit` *is*
  `kit_charges(.., Sentry)`, and `finish_deploy` destroys that
  engineer's **oldest** standing sentry (lowest id, a `DeployableLost`
  said for it) rather than refusing the laying, so a sentry can be moved
  about the deck freely and never outnumbers its charges. Sandbags have
  no such limit: `sentries_left` is now simply the kits in the pack.
- **The sentry's talents are a `bims::combat::Skill`**, not a weapon:
  `World::sentry_skill(owner)` carries *enhanced optics*'
  `ENHANCED_OPTICS_RANGE` (ten tiles added) and *sentry mark III*'s
  `SENTRY_MARK_THREE_FIRE_RATE` (×2) and `SENTRY_MARK_THREE_DAMAGE`
  (×1.2), handed to the room on the `Sentry` and applied through the one
  `Combat::fire_as` a Bim's skill goes through. `sentry_weapon` is the
  auto rifle at tier one or two as before and a **tier-three sniper
  rifle** with *sentry mark III*.
- **The engineer's own armour** is *higher quality armour*'s, set in
  `skill_of` beside `armour_drain`: `armour_protection_add` is
  `BETTER_ARMOUR_PROTECTION` (a point added after *plated*'s multiplier)
  and the drain is divided by `BETTER_ARMOUR_HEALTH` (1.05), which is
  five per cent more health said the tank's way — a piece's stored
  health never changes meaning as it moves between bodies.
- **`REINFORCED_SAND_HEALTH`** (50) is *added* to `SANDBAG_HEALTH` (200)
  where every other sandbag factor multiplies, and a bag at nothing is
  gone for good as it always was.

`tests_engineer.rs` pins all of it:
`a_spent_charge_comes_back_on_its_cooldown` (the two cooldowns, the
third level gating the sentry's, and the timer in the checksum),
`second_sentry_is_two_charges_and_mark_three_is_a_tier_three_sniper`
(the oldest destroyed, and the sniper's worked numbers),
`a_sentry_fires_at_the_enemy_it_sees_and_never_runs_out`,
`reinforced_sand_and_site_foreman_are_the_level_two_pick`,
`armoured_sentry_and_enhanced_optics_are_the_sentry_s_numbers`,
`higher_quality_armour_adds_protection_and_health_to_what_he_wears` and
`extra_bags_is_a_fourth_charge_and_steady_hands_keep_at_a_deploy`.
**`SAVE_VERSION` 26, `wire::PROTOCOL` 18** (the relay wants
redeploying), `REFERENCE_CHECKSUM` = `0x_e136_a3b4_7e94_cdcd`. The dial
is `World::set_kits_for_probe(n)` → `Session::kits_for_probe` →
**`BIMS_KITS=n`**: exactly `n` of each kit in every pack with both
cooldowns started afresh, since an engineer with nought charges and the
whole wait ahead is the one state a scripted pointer cannot walk a Bim
into.

**Cover is set again every step, on both rooms.** `sync_deployed_cover`
turns every laid sandbag into a tile rect in the crew's room
(`deployable_room_pos`: the ship's through `room_of`, a station's through
`from_station` while docked there) and in the residents' room
(`deployable_residents_pos`: the ship's through *their* `from_station`
— the mirror — a station's own through `to_room`) and hands both lists
to `Game::set_laid_cover`, which is a no-op when unchanged. It runs in
`hand_the_room_the_engineers` before the rooms step and after every
`join_rooms`, `relayout_room` and `unjoin_rooms`, because a fresh
`Sight` has none; `unjoin_rooms` also drops every `Deck::Station`
deployable (`drop_station_deployables`) — the ship's keep across
docking, undocking, joins and unjoins.

**A sentry is the room's shooter and the enemy's target.**
`hand_the_room_the_sentries` gives the crew's room every sentry in it as a
`bims::combat::Sentry` — its room position, `sentry_weapon` (the auto
rifle at tier one; two from `SENTRY_MARK_TWO_LEVEL`; a tier-three sniper
rifle with *sentry mark III*), `sentry_skill` and whether *dug in* — and
the room fires them
after the crew; `visit` appends the same sentries **after the crew** in
the residents' targets (`to_station` of each), so the enemy's
nearest-target rule and their blades find them, and a melee shot nearest
a sentry index goes to `Game::enemy_strike_sentry`. `settle_deployables`,
after `visit`, reads back
`take_sentry_hits` (off `health`) and `take_cover_hits` — the bolts a
body dodged behind laid sandbags, by room tile, off the bags' `health` —
removes what is at nothing (`DeployableLost`) and hands
the sentries again. Nothing comes back off a wreck: its charge returns
on its own cooldown. `hand_the_room_the_engineers` also sets
`Game::set_work_factors` (*site foreman* on a build; the craft factor is
one for everybody since feature 88 took *quick hands* off the tree) and
`Game::set_steady_hands`.

**The armourer's repair is `REPAIR_ORDER`** (1 001), the upgrade's
pattern: `Command::Repair` (`can_repair`: the talent, `NoTalent`; a
workbench; the bench not `busy()` — `Workbench::repair: Option<u32>` is
the engineer, and `busy()` is it or `work` — a damaged piece alone in
the first slot, `NoPair` otherwise; `deploy::ARMOUR_REPAIR_COST` (100 €)
in the pool, taken at once — it was metal out of the hold until the
money rework) puts `bench.repair = Some(slot)`, `craft_orders` offers
one `Order { recipe: REPAIR_ORDER, only: Some(slot) }` at the first
workbench — the room's `craft_on_offer` skips an order with an `only`
that is not the asker — and `finish_repair` puts the piece in the
output slot with `class::ARMOUR_REPAIR_HEALTH` back on it, capped at its
tier's health (`Repaired` 74). `Workbench::takes` lets a damaged piece
of any tier onto an empty bench for it.

**What moved.** `ResourceId::SandbagKit` and `SentryKit` (15 and 16
since the money rework closed the deleted resources up; locker class,
footprints 2×2 and 2×3, sold nowhere, and **made nowhere** since
feature 88 turned them into charges) re-pinned every design hash, the galaxy
checksums and `REFERENCE_CHECKSUM`; the
classes, progress, `undocked_once`, the deployables and `reused_kits`
are hashed after the memories; **`SAVE_VERSION` 16, `wire::PROTOCOL` 9**.
`Refusal` 41–51 are the new ones. `BIMS_CLASS=engineer` in the app puts
the class on slot 0 of any launch, and `Q`/`E` in `BIMS_KEYS` press the
two keys.

## The soldier: the brace, the skills and the grenades (feature 75)

The second class. `crates/world/src/class.rs` holds `Class::Soldier = 2`,
its fourteen talents (`Talent` 14–27, `DugInBraced` for the soldier's
*dug in* beside the engineer's `DugIn`) and every number
(`BRACE_ACCURACY` … `RAMPAGE_STACKS`, `steady_aim_walking()`); the
world's side is the "soldier" `impl World` after the deployables'
(`skill_of`, `can_brace`/`brace`, `can_throw`/`throw`, the grenade
getters, `hand_the_room_the_soldiers`, `settle_rampage`,
`settle_bursts`), and `tests_soldier.rs` is every test the spec asked
for. `set_class` to Soldier is `give_soldier_kit`: a basic auto rifle
given and equipped (the pistol swapping into the pack) and
`SOLDIER_START_GRENADES` grenades; back to None is `take_soldier_kit`.

**The talents are one `bims::combat::Skill` a crew member, handed to the
room every step.** `World::skill_of(who)` works it out fresh —
`Skill::NONE` for anybody but a soldier — from the progress, the brace
(read off the room, `Game::is_braced`) and the *rampage* stacks (the
same): the odds multiplied by `BRACE_ACCURACY` and *marksman*, *point
blank* on the bolt, *runner*'s pace, *steady aim*'s walking odds, *iron
nerve*, *cover master* on `DODGE_IN_COVER`, *drill* from `DRILL_LEVEL`
(the seventh, a fixed level), *bruiser*, *dug in* while braced,
*deadeye*, and *rampage* as `RAMPAGE_FIRE_RATE` to the power of the
stacks. `hand_the_room_the_soldiers` runs after the engineers', before
the rooms step, into `Game::set_skills`; what the room does with it is
`crates/game/CLAUDE.md` ("One shooter, and the soldier's skill on it").

**The brace is the room's flag.** `Command::Brace { slot, on }` —
`can_brace`: `NotASoldier` (53) by `class::can`, `OutOfReach` not fit to
act — is `Game::set_braced`, and the room ends it on its own on any
order that moves the Bim (`interrupt_for_order`) and when it goes down;
the world only reads it (`World::is_braced`). `WorldEvent::Braced { who,
on }` (75). It is hashed off the room like the positions, with the
*rampage* stacks (`Game::rampage`, integers) and — until feature 90 made
it one entry of `World::charge_timers` — `World::last_throw`.

**A grenade is `ResourceId::Grenade`** (17 since the money rework closed
the deleted resources up; mass 10, locker class, 1×1, sold nowhere and
bought anywhere, and **made nowhere** since feature 95 took its recipe
with the materials). `Command::Throw { slot, x, y }` names a
room tile like a deploy; `can_throw` refuses in this order: `OutOfReach`
(not fit), `NotASoldier`, `NoGrenadesYet` (55, under `GRENADE_LEVEL`),
`NoGrenade` (54), `CoolingDown` (56, `grenade_cooldown_left` off
`last_throw` in clock minutes against `GRENADE_COOLDOWN` seconds — a
game minute is a real second at 1×; **gone at feature 90**, where the
charge in the pack is the whole of the gate and the cooldown is what
brings it back), `CantThrowThere` (59, not a deck
tile: `Game::is_deck_tile`), `OutOfThrowRange` (57, past
`grenade_range`), `NoLineToTile` (58, `Game::line_clear` — walls and
shut doors, never sandbags). `throw` takes the grenade out of the pack
at once, notes the clock and calls `Game::throw_grenade` with the fuse,
the radius and `GRENADE_DAMAGE` — *long throw*, *short fuse*, *frag* and
*quick draw* are the four getters. `WorldEvent::Thrown { who }` (76).

**The burst is the room's, in both rooms.** `Game::burst` (on the fuse
running out, in `tick_combat`) hits every own body and every target
within the radius with a line from the burst — the damage falling from
`GRENADE_DAMAGE` to half at the edge, halved again in cover from the
burst's side, on a part off the combat stream — and every sentry and
every laid sandbag in it. An own body is `Game::blast` (a strike, then
`Filth::splash_blood`); a target is a `Hit { blast: true, by }` that
`visit` carries to the residents' room as `Game::blast` there, noting
`Residents::last_hit_by`; the sentries go out through `take_sentry_hits`
as ever, and the bags through `Game::take_bags_blown`, which
`settle_bursts` turns into `DeployableLost` for every sandbag deployable
under a blown tile. `Hit` and `Bolt` carry `by: Option<usize>` (a Bim's
own bolt or blow; `None` for a sentry's and an enemy's) for the
*rampage*: `experience` hands back every enemy newly down with who last
hit it, and `settle_rampage` gives that soldier a stack (up to
`RAMPAGE_STACKS`) — or clears every stack when `enemy_standing` is
false: the rooms unjoined, or no body of the station's up and
conscious. **`enemy_standing` counts the station room's machines as
well as its Bims** — every body at a hostile station, and the machines
alone in a town the crew are defending. Until the fix after feature 104
it counted the Bims alone, so against the machines — every enemy since
feature 102 — it was always false and the stack earned was cleared the
same step; `rampage_is_a_stack_a_machine_downed_until_none_stands` is
the test, a second machine standing out of the grenade's reach while the
soldier's burst downs the first. Enemies neither throw
nor dodge grenades.

**What moved.** `SAVE_VERSION` 17, `wire::PROTOCOL` 10, `Refusal` 52–59,
events 75–76. The app's
`keys::Action::{ClassPrimary, ClassSecondary}` (Q, E) replaced the
engineer's two and dispatch by the steered class (`screens::game::class_key`);
`BIMS_CLASS=soldier` puts the class on slot 0 of any launch.

### A grenade is a charge, and every class's supply is one list (feature 90)

Feature 88 took the crafting out of the engineer's abilities; this one
says the same of every class, and the soldier's grenade was the one
left. **`class::Charge`** is the thing now — `Sandbag = 0`, `Sentry = 1`,
`Grenade = 2`, codes across the seam like everything else, with
`resource()`, `class()`, `level()` (the level its ability is learnt at)
and `of_kit`/`kit` to and from `deploy::Kit` — and the world keeps
**one** list for the lot: `World::charge_timers`, a
`[Option<f64>; Charge::ALL.len()]` a crew member, which replaced
`kit_timers` **and** `last_throw` together.

- **`World::charges(who, charge)`** is how many of it the pack fills
  back up to: nought for a crew member of another class (`Charge::class`)
  and nought under `Charge::level`, else the engineer's two kits as
  feature 88 had them and `class::GRENADE_CHARGES` (**two**) grenades.
  **`World::charge_cooldown(who, charge)`** is what one takes to come
  back — 45 and 60 seconds for the kits, `class::GRENADE_COOLDOWN`
  (**thirty**, five before) for a grenade, halved by *quick draw*, which
  is what that talent now means. `charges_of` counts what is in the
  pack, `charge_cooldown_left` reads the timer, and `kit_charges`,
  `kit_cooldown_left`, `kits_of`, `grenades_of`, `grenade_cooldown` and
  `grenade_cooldown_left` are all thin wrappers so nothing that asked
  the old questions had to change.
- **`World::restock_charges`** (was `restock_kits`) is the one loop, a
  step before `hand_the_room_the_engineers`, over every crew member and
  every charge. A soldier's grenades come back exactly the way an
  engineer's kits do: in combat as out of it, one at a time, nothing out
  of the hold.
- **The charge is the whole of the gate.** `can_throw` no longer refuses
  `CoolingDown` and `throw` notes nothing down — the grenade leaving the
  pack is what starts the cooldown, on the next step, the way a kit laid
  does — so **both charges may be thrown one after the other** and the
  wait is for them coming back. `Refusal::CoolingDown` is the tank's and
  the commander's now.
- The soldier still **sets out with** `GRENADE_CHARGES` grenades
  (`give_soldier_kit`, `take_soldier_kit`), whatever its level, the way
  an engineer sets out with its sentry charge before the third level.
  The `Grenade` recipe, price and footprint are **left alone**, as
  feature 88 left the kits': removing a `ResourceId` would move every
  design hash and galaxy checksum, and a grenade is still craftable and
  lootable even though no soldier needs one made.

**What moved.** `world_checksum` eats `charge_timers` where it ate
`kit_timers` (the soldiers' block is the brace and the *rampage* stacks
alone now) — `REFERENCE_CHECKSUM` = `0x_3a43_a3fe_a913_4b6f` —
**`SAVE_VERSION` 27, `wire::PROTOCOL` 19** (the relay wants
redeploying). The dial is `World::set_charges_for_probe(charge, n)`,
with `set_kits_for_probe(n)` (both kits) and `set_grenades_for_probe(n)`
over it → `Session::grenades_for_probe` → **`BIMS_GRENADES=n`**. The
app: the Q box counts the charges and shows the seconds only with none
left, the way the engineer's two boxes do. `tests_soldier.rs`'s
`a_grenade_wants_a_line_a_range_and_a_cooldown` is the charges and the
refusals, and
`long_throw_short_fuse_frag_and_quick_draw_are_the_grenade_s_numbers`
the halved recharge.

## The medic: the heal beam and the surge (feature 76)

The third class. `crates/world/src/class.rs` holds `Class::Medic = 3`,
its fourteen talents (`Talent` 28–41, `SteadyHandsMedic` for the medic's
*steady hands* beside the engineer's `SteadyHands`) and every number
(`XP_HEALED` … `FIELD_SURGEON_TIME`); `crates/world/src/medic.rs` is the
state — one `Medic { patients, charge, field_surgery_used }` a crew
member on `World::medics`, by index, an empty one for anybody who is not
a medic — and the "medic" `impl World` after the deployables' is the
rules (`can_beam`/`beam`, `can_surge`/`surge`, `hand_the_room_the_medics`,
`settle_medics`, `clear_beams`, `medic_skill` and the getters).
`tests_medic.rs` is every test the spec asked for. `set_class` to Medic
is `give_medic_kit`: `MEDIC_START_MEDKITS` medkits and
`MEDIC_START_BANDAGES` bandages into the pack, the pistol left in the
hand; back to None is `take_medic_kit`. **Those kits are the ones it
treats with**, before ever walking to a cabinet for the hold's — the
medicine bullet under "The enemy shoots back" is the seam, and
`a_helper_treats_with_the_kit_in_its_own_pack_before_fetching_one_off_a_shelf`
in `tests_medic.rs` pins it.

**The beam is a list of crew indices on the medic, checked every step.**
`Command::Beam { slot, patient: Option<u32> }` links or unlinks;
`can_beam` refuses in order: `NotAMedic` (60) by `class::can`,
`OutOfReach` (not fit to act), then `beam_reaches` — `NotACrewmate`
(62) for itself, for nobody, or for a body that is not a living crew
member; `OutOfBeamRange` (63) past `beam_range` tiles or outside;
`NoSightOfPatient` (64) with no line (`Game::sees`, the trace's rule —
walls, shut doors and the dark). A patient past `beam_patients` (one, or
[`class::DOUBLE_LINK_PATIENTS`] with *double link*) takes the oldest's
place. `hand_the_room_the_medics`, stage 5 before the soldiers' skills,
is where a link is **kept or broken**: broken whole where the room
ended it (`Game::is_beaming` — an order to an errand, the medic down) or
the medic is unfit, and patient by patient where `beam_reaches` no
longer holds; a `WorldEvent::Beamed { who, patient: None }` (77) says so
once. It then charges the surge — a step's minutes while some patient is
under `MAX_BLOOD` or bleeding — and hands the room two things: a
`bims::health::Beamed` a body (`Game::set_held`: the blood an hour and
the mend factor, *mender* from `MENDER_LEVEL` at `MENDER_RECOVER`;
*self-care* gives the medic an entry of its own at no blood rate, which
is the "nothing bleeds" half), and a `bims::health::Doctoring` a Bim
(`Game::set_doctoring`: *field dressing* and *surgeon* as effort factors
on the two errands' working steps, *field surgeon* as `bare` while the
fight's one is unused, *clean hands* and *steady hands* on what a
treatment leaves). What a beam does to a body is the room's
(`Health::update_held`: nothing bleeds, the blood comes back at the
rate — or the body's own once nothing is open — and the parts above
nothing mend faster), and the room knows nothing of who holds whom.

**A medic beaming holds its fire.** `skill_of` answers `medic_skill`
for a medic: `Skill::holds_fire` while linked, or `fire_rate ×
GUNNER_MEDIC_FIRE_RATE` with *gunner medic*; `tick_combat` reads
`holds_fire` where it reads the rest.

**The surge is the room's timer on each body.** `Command::Surge { slot }`
— `can_surge`: `NotAMedic`, `OutOfReach`, `NoSurgeYet` (65) under
`SURGE_LEVEL`, `NotLinked` (67) with nobody held, `NotCharged` (66)
under a full charge — empties the charge and sets `Game::set_surge` on
the medic and every patient for `surge_minutes` (the room counts it down
in seconds), a patient's with *closing surge*'s flag, and with *mass
surge* on every other crew member within `MASS_SURGE_TILES` of a
patient. While it runs `Game::strike` absorbs the whole of any hit —
no wound, no armour drained, no trauma — and the room closes the
patient's wounds as it ends when the flag is up. `WorldEvent::Surged`
(78). Unlinking does not end one.

**`settle_medics`, after the experience**, drains `Game::take_healings`
— every dressing and treatment the room finished, with what it used —
gives `XP_HEALED` to a medic that did one on a crewmate, marks the
field surgery used for a bare one, and puts every medic's field surgery
back when `enemy_standing` is false, the way `settle_rampage` clears the
stacks — so once a fight is **while a machine stands**, and the surgery
comes back when the last of a wave is down.
(`field_surgeon_is_once_a_fight_against_the_machines_and_comes_back_when_it_ends`.)
Until the fix after feature 104 `enemy_standing` counted the station
room's *Bims* alone, a held station has none, and the reset was every
step: a bare field surgery was never held to once a fight.

**Crew indices are all a link is**, so `clear_beams` breaks every beam
at a hire and when a dead bot is dropped from the crew at a mission's
end (`drop_crew_member`, which also removes that index's `Medic`; the
dismissal was the other until feature 104);
a dead crew member's beam and charge go with its progress in
`casualties`. In `world_checksum` after `last_throw`: every medic's
patients, charge and flag, then each body's surge off the room. **`SAVE_VERSION`
18, `wire::PROTOCOL` 11**, `REFERENCE_CHECKSUM` = `0x_7167_1507_6824_4a02`.
`BIMS_CLASS=medic` puts the class on slot 0 of any launch; `E` over
another crew member links the beam (`class_key` reads
`Game::crew_at` under the pointer).

**`beam_for_probe` bleeds the patient as well as wounding it** (feature
91): `BEAM_PROBE_BLOOD` (three fifths of full — under
`health::SLOWED_AT` so there is plenty to put back, over `OUT_AT` so the
patient is on its feet). A beam stops the bleeding *dead*, so a patient
wounded and beamed in the same breath is at full blood for ever, the
beam's blood rate does nothing and the app's green numbers over it
(feature 91) count nothing — the dial staged a beam that was doing
visibly no work.

## The tank: the wall, the taunt and the hits (feature 77)

The fourth class. `crates/world/src/class.rs` holds `Class::Tank = 4`,
its fourteen talents (`Talent` 42–55) and every number
(`TANK_HITS_PER_XP` … `RALLYING_WALL_DRAIN`);
`crates/world/src/tank.rs` is the state — one `Tank { last_taunt }` a
crew member on `World::tanks`, by index — and the "tank" `impl World`
after the soldier's is the rules (`can_bulwark`/`bulwark`,
`can_taunt`/`taunt`, `armour_drain`, `tank_skill`, `haul_load`,
`hand_the_room_the_tanks`, `settle_tanks` and the getters).
`tests_tank.rs` is every test the spec asked for. `set_class` to Tank is
`give_tank_kit`: a fresh basic helm, kevlar and leg guards **on** —
pieces of the world's, `next_piece` ids at `Where::Worn`, the way
`outfit_for_probe` dresses a crew, so the hold's counts never move and
the pool is untouched — with the laser pistol already in hand; back to
None is `take_tank_kit`, which takes off the whole tier-one pieces the
world knows are worn and leaves anything looted or fetched.

**There is almost nothing to keep.** `Tank::last_taunt` is one clock
reading and the whole of the struct: Bulwark is a flag on the *Bim*
(`bims::bim::Bim::bulwark`, `Game::set_bulwark`), since the room is what
has to know where the wall stands when a bolt comes through it; the
hits are a count on the Bim too (`Game::hits_taken`), since the room is
where a hit lands; and every talent is read afresh each step into the
room's one `bims::combat::Skill`. All three are in `world_checksum`
after the medics — `last_taunt` on the clock's grid, then, read off the
room like the brace, who stands as a wall and each body's hit count —
which moved `REFERENCE_CHECKSUM` to `0x_73f0_69d0_4be9_0a00`.

**`World::skill_of` is a bag for every class now**, not the soldier's
alone: it picks `medic_skill`, `tank_skill` or `soldier_skill` and then
sets `armour_drain` for **everybody**, since *rallying wall* gives a
tank's drain to the crew round him. `tank_skill` is the rest of the
tank's: `walk` (the always-on pace factor, the bulwark's — `pace` is
*runner*'s, applied only with an enemy in sight), `dodge` (*guarded*,
while the wall is up), `armour_protection` (*plated*), `smash_rate`
(*breacher*), `nerve` and `steady_pace` (*unmovable*) and `iron_frame`
from `IRON_FRAME_LEVEL`. What each does is one place in the room —
see `crates/game/CLAUDE.md`, "The tank's wall, its armour and its hits".

**The wall is handed to the room every step.** `hand_the_room_the_tanks`,
stage 5 before the skills (which read the flag off the room), gives the
crew's room a `bims::combat::Bulwark { who, reach, interpose }` for
every tank with it up **and fit to act** — so a wall goes down with the
tank the step he does — and the room's one shooter does the rest.

**The taunt is a clock reading and nothing else.** `Command::Taunt`
notes `clock_minutes`; `is_taunting`, `taunt_left` and
`taunt_cooldown_left` are read off it against `taunt_minutes(who)` and
`class::TAUNT_COOLDOWN` (the grenade's cooldown arithmetic). What it
*does* is in `visit`: the crew's taunting radii in room units and their
*magnet* flags — worked out before the residents' room is borrowed, like
the sentries, since the talents are the world's — go to
`Game::set_hostiles_taunting` right after `set_hostiles_peeking`, and
`Combat::aim` prefers a taunting target in reach and in sight over any
nearer one. That is the room the enemies aim and charge in whoever they
are, so the machines are taunted by the same code a station's people, a
garrison and a raider's boarders were
(`a_taunting_tank_is_shot_at_before_a_nearer_crewmate_and_two_runs_agree`
is a Trooper's since feature 104). *Hold fast* rides on the medics' seam: it puts
a `bims::health::Beamed { blood_an_hour: 0.0 }` on the tank in
`hand_the_room_the_medics`, exactly as *self-care* does for a medic.

**The experience is `settle_tanks`**, after the medics': every
`TANK_HITS_PER_XP` hits on the Bim's count are one point and the
remainder counts on. The room counts a hit where it *lands*
(`Game::count_hit_taken`), so armour, a surge and the body all count and
a miss or a dodge does not.

***Pack mule* is gone** (feature 95): it was a bigger load carried to a
construction site, and nothing is carried to one any more. `Room::picked`,
`finish_pick`, `World::haul_load` and `World::free` went with it, and the
tank's **second level is a fixed level** — *plated* alone, given outright
rather than chosen at. `Talent::PackMule` is deleted rather than kept as
a dead code, so `Talent::ALL` is 69 and the tank's band starts one
lower; `class::fixed_at(Class::Tank, 2)` is the one row in that table.

**What moved.** `SAVE_VERSION` 19, `wire::PROTOCOL` 12, `Refusal` 68–69
(`NotATank`, `NoTauntYet`; a taunt within its cooldown is the grenade's
`CoolingDown`), events 79–80 (`Bulwarked`, `Taunted`). No new resource,
so no design hash moved. The app: `class_key` dispatches Q and E for the
tank, `crew::TankView` is the panel's rows, `theme::wall_mark` and
`theme::taunt_ring` are the two pictures; `BIMS_CLASS=tank` puts the
class on slot 0 of any launch.

## The commander: the aura, the squad and the rally (feature 78)

The fifth class. `crates/world/src/class.rs` holds `Class::Commander = 5`,
its fourteen talents (`Talent` 56–69) and every number (`XP_HIRE`,
`AURA_*`, `NERVE_HOLD`, `HIRE_DISCOUNT_PERCENT` … `ANCHOR_BONUS`);
`crates/world/src/commander.rs` is the state and
`tests_commander.rs` every test the spec asked for. `set_class` to
Commander gives and takes **nothing**: he sets out with the laser pistol
every Bim is issued.

**Two things are kept and no more.** One `Commander { last_rally }` a
crew member on `World::commanders` — the taunt's arithmetic again —
and **one** `SquadOrder { by_slot, kind, members }` for the whole world
on `World::squad`. The aura is not kept at all: `World::aura_reaching`
works it out every step from where the commanders stand, and it goes to
the room through `skill_of` like every other class's numbers, so it
follows him about with no state to keep in step.

**Whom each half reaches is the rule to hold on to.** The aura and the
rally lift **every friendly Bim** in range — a player's own steered Bim,
the crew's bots and the hired hands alike, never an enemy and never
himself (the rally does cover him). A **squad order commands only the
squad**: `World::squad_members` is every crew member `who >=
players()`, alive and on the deck, within `squad_range` (the whole room
from `LONG_REACH_LEVEL`). `settle_squad`, at the top of
`hand_the_room_the_squad` every step, prunes a member a player has begun
steering and one dead or outside, and drops the whole order when the
commander is not `fit_to_act`, when an attack's marks are all gone
(down or dead) or when nobody is left under it. ***Relentless*** does
two things there. A mark **out cold** is not gone until it is dead — a
machine never is out cold, being destroyed at once — and an attack
whose marks are all dead **moves on** to the enemy standing nearest the
commander (`World::nearest_enemy_standing`, the lowest index on a tie)
rather than ending, so the order ends only when nobody is standing. The
move says nothing (an order ending by itself says nothing either); the
room is simply handed the new mark. The second half is the fix after
feature 104, which had found the talent with nothing to act on against
the machines (`relentless_takes_the_attack_on_to_the_nearest_machine_standing`;
without the talent the order ends with its mark, a machine still
standing, `an_attack_marks_an_enemy_and_ends_when_it_goes_down`). An
unseen mark is walked towards whatever the talent — that is the room's
`Target::stale`. `take_the_ordered_out_of_squad` is the other half: a
`Command::Crew`/`CrewLater` takes the crew member an errand names — or
everybody the player has selected, for a move or a line — out of the
order until the next one. `clear_squad` is called by `unjoin_rooms`, a
hire and `drop_crew_member`, since the members are crew indices and the
marks are residents'.

**What the room is told** is one `bims::game::Squad` a crew member
(`Game::set_squad`), handed over before the skills, which read *focus
fire* and *stand ground* off the order. The room's side — the stand, the
gather ring anchored on a tile, holding fire while falling back, and
`Combat::aim_marked` — is `crates/game/CLAUDE.md`.

**What the aura does** is `lift_by_aura`, at the end of `skill_of` beside
`armour_drain`: `accuracy`, `effort` (a factor on every errand's working
steps, the only one here that is nobody's own class's), `walk` and
`nerve_hold` — seconds a dying body holds its ground before it runs,
nought for everybody and `NERVE_HOLD × aura.nerve` in an aura — and,
with a rally, `accuracy × RALLY_AIM`, `nerve` and *grit*'s `unhurt`.
*Steady ranks* rides on the medics' seam like the tank's *hold fast*: a
`bims::health::Beamed` with `bleed` under one, which slows the bleeding
rather than stopping it (`Beamed::HELD` is the entry that stops it
dead). `class::aura_bonus` is how *strong presence* and *anchor* deepen
each bonus — what it *adds* is multiplied — and two commanders' auras
never stack: `aura_reaching` takes the strongest by `work`.

**Hiring** goes through `World::hire_fee(slot, resident)`: the fee less
`HIRE_DISCOUNT_PERCENT` (two fifths with *haggler*) for a commander fit
to act, rounded down to whole euros, and the plain fee for everybody
else. `hire_offer(who, resident)` reads it for the window, so the app
says the discounted price while a commander is steered; `hire` reads it
for the **sending slot** and that is what goes into `Hired`, which is
that hand's fee for the whole contract. The hire is then `XP_HIRE` to
that commander alone, and *outfitter* puts the lowest basic piece the
hand was missing on it out of nothing (`outfit_the_hire`, a piece of the
world's like the tank's start), charged for at nothing.

**What moved.** `SAVE_VERSION` 20, `wire::PROTOCOL` 13, `Refusal` 70–73
(`NotACommander`, `NoRallyYet`, `NoSquadInRange`, `NoEnemyThere`; a
rally in its cooldown is the grenade's `CoolingDown`), events 81–82
(`Squadded`, `Rallied`). No new resource, so no design hash moved;
`REFERENCE_CHECKSUM` = `0x_942d_d49d_a5af_7b82` (the commanders, the
squad order and each body's `fear` hashed after the tanks). New pub
`World::resident_at(x, y)` is the enemy under a room point, for the
app's Attack key. The app: `class_key` dispatches Q and E for the
commander and `squad_key` the two keys of its own —
`keys::Action::{SquadFallBack (X), SquadStandGround (Z)}` —
`crew::CommanderView` is the panel's rows, and `theme::{aura_ring,
lifted_mark, squad_mark}` the three pictures. `BIMS_CLASS=commander`
puts the class on slot 0 of any launch, and `BIMS_KEYS` knows `X` and
`Z`.

## Every player has two orders for the bots that follow them (feature 84)

`crates/world/src/orders.rs` is the whole of the world's side: one
`Standing` a player slot on `World::standing` — `Follow`, `Attack {
tile }` or `Retreat` — and nothing else. It is **not a class's**: every
player has these two from the first step, whatever they chose in the
yard, and `World::can_order(slot)` asks only that they are `fit_to_act`.

* **`Command::Orders { slot, order }`** is the seam, and the commander's
  squad rule is its rule: the **same order given again** puts that
  player's bots back to following, which is what a second press of the
  key does. An attack wants ground under the banner —
  `Game::is_banner_tile`, a free deck tile of the crew's room or
  anywhere at all on a plain, where the ground beyond the deck's box is
  walked on the body's own window — and is `Refusal::NoGroundThere` (74)
  otherwise. `WorldEvent::Ordered { who, kind }` (86) says it, the kind
  being `Standing::code` and nought the release. **A release is never
  asked for ground** (`World::ground_for`, checked after `same_as` and
  not before it): the ground is a question about a banner being *put
  down*, and a banner on a station's deck is on no tile of the crew's
  room the moment the ship has left — so asking it of a release
  refused the one press that takes a banner up, and the crew stood under
  arms at it for ever.
* **`hand_the_room_the_standing`**, in the step right after the squad's
  hand-off, drops the order of a player no longer fit to act — down,
  asleep, outside — **and any attack whose tile is no longer ground**,
  which is every arrival and departure, since a banner is a
  tile of the deck the crew walk and that deck is built afresh at each
  of them; then hands the room one `bims::game::Standing` a slot with
  the attack's tile turned into room units. The squad's is handed
  first on purpose: a commander's order to the squad is a class's and
  outranks the standing one, which is the order the room reads them in.
* **And it says where the ship is**, `Game::set_home` off
  `Aboard::gangway` — the deck a few tiles inside the **ship's** own
  airlock, in room units, `None` for a ship on its own. A retreat
  goes there, and the room cannot work it out for itself: its own
  `Room::gangway` on a joined deck is the joined design's first *free*
  airlock, and the ship's is mated to the station, so the room answers
  the station's **far** door — which is why a retreat walked the crew
  the length of the building away from the ship until this was said.
  `World::fall_back_point()` is the same spot for the app, which stands
  a blue **defend sign** (`theme::defend_banner`) on it while any player
  has a retreat called, since a retreat has no banner of its own to put
  down and the log's line was the only word for it. Nothing new is
  hashed: it is a handover, like `set_foreign`.
* **In `world_checksum`** after the squad order: the length, then each
  order's code and an attack's tile. An order moves bodies, so two
  clients that disagree about it disagree about where the crew are
  standing. `REFERENCE_CHECKSUM` moved to `0x_0e55_7d58_533d_1c7e`;
  **`SAVE_VERSION` 22, `wire::PROTOCOL` 14**.

**Whose order a bot is under, and what it then does, are the room's** —
`crates/game/CLAUDE.md`, "The bots follow a player": the bot takes the
order of the player whose own Bim it is nearest, the crew are under arms
whenever a player is leading them (`Game::led`, which is a player's own
Bim recruited or a standing order given) as well as at the alarm, and
the ship is a last stand no order takes anybody out of. The world does
not decide any of that, because the room is what knows where everybody
is standing.

The app's half: `keys::Action::{Attack (F), Retreat (T)}` — which is
what moved the camera's Follow onto V — `screens::game::orders_key` and
`screens::game::attack_key` (which is what makes F over a banner a
release rather than a second banner), the
armed red pointer (`attack_cursor`), `theme::attack_banner` and
`theme::defend_banner` on the deck. `tests_standing.rs` is the rule: the
order given, said and released; the two refusals; the checksum and a
twin; the bots under it and the player's own never; the fall back
walking home and **arriving**; that home is the ship's own gangway and
not the station's far door; the two ways it is walked — backwards
with the gun up, or a sprint with nothing to shoot at; and
`a_banner_is_dropped_when_its_ground_goes_and_a_release_never_wants_any`,
the two halves of the banner that could not be taken up.

## What a class's keys have left is the world's, and so is the level they want (feature 80)

The app draws two boxes at the foot of the screen for the class's own
two keys (the root `CLAUDE.md`, "The class's two keys have two boxes"),
and every number on one comes from here rather than being worked out
there. Nothing new is kept and nothing new is hashed: they are all
readings of what the world already holds.

- **`class::key_level(class, primary)`** is which level a key is learnt
  at — every class's **E** from the first and its **Q** from the third
  (`SENTRY_LEVEL`, `GRENADE_LEVEL`, `SURGE_LEVEL`, `TAUNT_LEVEL`,
  `RALLY_LEVEL`, all three) — and `None` for `Class::None`, which has no
  keys at all. `a_class_s_e_is_its_first_level_and_its_q_its_third` pins
  the shape, and the app's own
  `the_two_boxes_say_what_the_keys_do_and_how_many_are_left` pins that a
  class has a box exactly where it has a key.
- **`World::kits_of(who, kit)`** counts a kit in a crew member's pack,
  the way `grenades_of` counts grenades, and **`World::sentries_left`**
  is what an engineer could still lay: those kits held down to the room
  `sentry_limit` less `sentries_of` leaves it — which is why
  `sentries_of` is public now. Three kits at the two-sentry limit is
  two, and one more laid is one.
- The rest were public already: `grenades_of` and
  `grenade_cooldown_left`, `is_braced`, `surge_charge`/`is_surging`,
  `patients_of`/`beam_patients`, `taunt_left`/`taunt_cooldown_left`,
  `is_bulwark`, `rally_left`/`rally_cooldown_left`, `squad_members` and
  `World::squad`.

The box never decides anything — a press still goes through
`can_deploy`, `can_throw` and the rest, which know about the pointer as
well — so a box that looks ready and a key the world refuses are not a
disagreement: the box says what it can see from here, and the log says
the rest.

## A class is worn, and the room is only told (feature 81)

`Class::outfit()` is the one table from a class to the kit the room draws
over the coverall (`bims::character::Outfit`), and
`World::hand_the_room_the_outfits` — in the step, beside the engineers'
handover — walks the crew and calls `Game::set_outfit(who, …)` for each.
It is **drawing only**: nothing reads it back, nothing is hashed, no
`SAVE_VERSION` and no `wire::PROTOCOL` moved for it, and the room's field
is `serde(skip)` because it is a function of `World::classes` — a load
has it back on the first step.

A class is a **player slot's**, so `class_of` answers `Class::None` for
the crew past the players and they are drawn `Outfit::Plain`, which is
what every Bim looked like before; a station's residents are never told
at all. It is said every step rather than at `set_class`, because a class
is also chosen in the yard, a hire shifts the crew and a restore brings a
whole world in — and `set_outfit` writes only when the answer changed.
The pictures themselves are `crates/game/CLAUDE.md`'s.

## The machines hold a station, and they come in waves (feature 83)

`crates/world/src/droid.rs` is the plan; `bims::droid` is the machine
(`crates/game/CLAUDE.md`, "A droid is not a Bim"). **Which** stations are
held is the crisis's job (feature 92, below); `World::infest(id)` is the
one door, which the crisis calls and `Session::droids` and
`droids_planet` call for the probes.

**A held station has no people at all.** `World::people_of` is nought
for one and `mercenaries_of` with it, and `World::stance` is Hostile for
one whatever the generator rolled — so the room is hostile, its bodies
are at war, and every seam the fight already had works unchanged.
`World::infest` reopens a room already open on that station
(`reopen_residents`, factored out of `set_hostile`, which used the same
machinery to arm a garrison where two residents stood until the human
enemies went in feature 104), so the people are gone the moment the
machines have it. `reopen_residents` compares
against the room's **`crew_count`** and not `Aboard::count`, which counts
the machines too.

**`World::infested` is one `Infestation` a held station**, sorted by id
and **in `world_checksum`** whole — how many waves are left, which is
aboard, when the next is due, whether it has been settled and whether it
has been cleared — beside the tier the machines come at
(`droid_tier`), the reinforcement clock (`droid_reinforce`) and the wave
cap (`droid_wave_max`), all three of which the probes move. It is the
size of the fight.

- **The wave count is fixed at the crew's first dock and never worked
  out again** (`Infestation::settle`, called from `droid_waves` the first
  step the rooms are joined): `DROID_WAVES_BASE` + the calendar + half
  the worth steps + a tenth of the levels, or whatever
  `World::set_droid_waves_for_probe` says — the `droids` commands set
  **three** (`screens::game::DROID_WAVES_IN_PROBE`, `BIMS_DROID_WAVES=n`
  over it), since the formula's two at day nought is one wave landing and
  then a cleared station, and what those commands are for is the wave
  after the first. That dial is neither saved nor hashed, unlike the
  other three: it is read once and what it decides —
  `Infestation::waves_left` — is both. Wave one is aboard then,
  stood about the station's rooms (`droid::spots_about`, free deck tiles
  spread across the design).
- **The wave size is worked out as each wave appears**, so a crew that
  has grown richer between waves meets more: `DROID_WAVE_BASE` + the
  crew + the calendar + the worth steps + a third of the levels, capped
  at `DROID_WAVE_MAX`. **Integers only, and nothing doubles** — a crew
  ten times as rich meets eighteen *steps* added, not ten doublings,
  which `station::scaled` would have made it. That is deliberate:
  `DROID_WAVE_MAX` is a **performance limit**, not a balance one, and a
  formula that could reach it in one jump would make the cap the only
  number that mattered.
- **The mix is `bims::droid::mix_of`**: Wardens `n / 6`, Husks `n / 3`,
  Troopers the rest, and `wave_kinds` orders them Wardens, Husks,
  Troopers so a Trooper's arm is dealt by its place *among the Troopers*
  — pistol, rifle, pistol, rifle.
- **No reinforcement while a machine lives.** `droid_waves`, a stage of
  the step right after `settle_residents`: with any of them standing the
  clock is held at `None`; with none standing and waves left it is set
  `DROID_REINFORCE_STEPS` (two hours) of the **mission clock** on — a
  minute in the probes — and the wave lands when it runs out (feature
  103; it was the world clock's minutes before). The clock is the
  world's and not the room's, so a room built afresh resets nothing.
- **Where a wave arrives.** At a station, `droid::arrival_airlock` — the
  airlock **farthest from the port** (the first airlock, where the crew
  dock; ties go to the lower index) — and the machines are posted
  `data::ASHORE_TILES` inside it, spread round the spot in rings so a
  wave does not land on one tile, the way `Residents::post_boarders`
  posted a raider's boarders until feature 104. On a surface, just inside the **gate** its lander set
  down beyond: north for an odd wave and south for an even one
  (`droid::gate_spot`). The lander itself is drawn on the plain, which is
  the crew's room's to draw — **a town's own room is the deck alone and
  has no plain in it**, so the machines are posted at the gate rather
  than beside the lander and walk in from there. That is the one place
  this step falls short of what was asked, and it is the room geometry
  saying so rather than a shortcut.
  **A ring falls where it falls, and `Game::adopt_droids` snaps what
  lands badly.** Four machines of sixteen came down in a bulkhead or out
  in the void at the arena, where a body fits in neither: they could not
  walk, nothing was ever in their sight, they never fired a shot, and
  they held the wave *after* them up as well, since nothing arrives while
  one is still standing — "the second wave does not shoot any more".
  Every machine adopted onto a spot the nav grid says is blocked is put
  on the nearest free cell now, exactly as `Game::adopt` puts a Bim
  carried between rooms. `every_machine_of_a_landing_wave_stands_where_a_body_fits`
  in `tests_droid.rs` lands two waves and asks the room of every one of
  them. `adopt_droids` also **keeps each machine's `plan_wait`** where it
  zeroed it: that is the stagger `build_wave` spread over `PLAN_EVERY`,
  and every wave laid went through the adopt and lost it.
- **A wave cleared takes the room's memory of it with it.**
  `Residents::{down, xp_down, xp_dead, last_hit_by, fee}` are one entry a
  **body** and `visit` only ever *grows* them, since a wave landing makes
  the room bigger. A wave *destroyed* makes it smaller, and the arrival
  cuts all five back to the room's Bims beside `clear_droids` — without
  that every machine of the next wave was born already flagged down: no
  `DroidDown` said when it was destroyed, and no experience paid for it.
  `the_next_wave_s_machines_are_said_down_and_paid_for_like_the_first`.
- **The ship is a picture.** `World::droid_ship(station)` is where it
  stands in the station's own design units and which way it faces, and
  whether it is a lander; `world_paint::droid_ship` draws it into the
  station's frame. It is drawn **while its wave has machines alive** —
  wave one arrived on nothing and gets none — and it is not part of the
  room, cannot be entered and cannot be shot.
- **Arrival raises `WorldEvent::DroidReinforcements { station }`** (code
  83) and puts every player's speed request back to 1× once, as raid
  contact did. The last machine of the last wave destroyed raises
  `WorldEvent::DroidStationCleared { station }` (84) **once**, and
  `World::droid_station_cleared(id)` answers for it afterwards — what the
  crisis step will read.
- **The countdown is two readings and no new state.**
  `World::droid_wave_standing()` is `(which wave is aboard, how many are
  still to come)` at the held station alongside and
  `World::droid_wave_due()` how long until the next lands, in minutes of
  the world's clock — `None` while a machine is still standing, since the
  clock does not run then, and `None` with none left. Both are read off
  the `Infestation` the world already keeps. The app draws them along
  the top in a red frame (`screens::game::droid_warning` in
  `warning_frame`, which was the raid's `raid_frame` until the raid
  went; `names::droids_*`): the
  wave and how many of it are standing while the fight is on, then the
  countdown the moment the last of them is down. That is the one number
  the player had no way of knowing.


### Every place that assumed a hostile body is a `Bim`

The survey feature 83 asked for, done before a line was written, and what
each one turned into. The rule that came out of it: **`Game::crew_count`
stayed the Bims and `Game::body_count` became the index space the world
hands over**, so a `who` inside the room is still a Bim's and only the
handful of methods the *world* asks of the other room had to learn the
difference.

In `bims::game` (`crates/game/src/game.rs`):

| assumed a `Bim` | now |
| --- | --- |
| `is_alive`, `is_down`, `is_unconscious` | dispatch: a machine is up or a wreck, and never out cold |
| `weapon`, `peek`, `exposed_at`, `dodge` | dispatch; a machine's arm is a `Weapon` value and its dodge is nought |
| `loot_cells`, `take_from_body` | empty and refused for a machine |
| `execute_body` | refused: there is no down-and-alive state to finish off (gone with the execution in feature 104) |
| `strike(who, part, ..)` | `strike_droid(i, DroidPart, ..)` beside it, the part off `Hit::roll` |
| `bim_pos(who)` | `body_pos(who)` |
| `set_hostiles`' eyes (`self.bims.iter()`) | the machines are eyes too, or a room of nothing but machines never sees anybody |
| `update_doors`' and `shut_now`'s bodies | the machines are bodies for the doors |
| `render`'s body loop | the machines drawn after the Bims, in body order |
| `take_crew`/`adopt` | `take_droids`/`adopt_droids` beside them |
| `hostile_bodies`, `at_war`, `muster` | untouched: a machine is recruited by nothing and `tick_droids` reads `war` directly |

In `world` (`crates/world/src/{world,crew}.rs`):

| assumed a `Bim` | now |
| --- | --- |
| `Aboard::count`, `Aboard::position` | `body_count`, `body_pos` |
| `visit`'s hits loop | a hit past the Bims goes to `strike_droid` |
| `visit`'s `alive`/`weapons`/`peeking`/`dodge` lists | over `room.body_count()` |
| `visit`'s `down` loop | over the body count, and says `DroidDown` for a machine |
| `visit`'s `down` list for the joined deck | false for a machine: a wreck is not a body to loot |
| `Residents::{down, xp_down, xp_dead, last_hit_by, fee}` | resized to the body count each step, since a wave landing makes the room bigger |
| `Residents::hailable` | `fee.get(who)`: a machine has no entry and no fee |
| `close_residents`' losses | the Bims alone |
| `Residents::join`/`unjoin` | carry the machines across with the same shift |
| `people_of`, `mercenaries_of` | nought at a held station |
| `stance` | Hostile at a held station whatever the list said (there is no list since feature 104) |
| `set_hostile`'s reopen | `reopen_residents`, shared with `infest` (and `infest`'s alone since `set_hostile` went), comparing against `crew_count` |
| `experience`, `enemy_standing_at`, `enemy_dead_at` | unchanged: they ask `is_alive`/`is_unconscious`, which dispatch |
| `take_hits`, `take_shots`, `enemy_fire`, `enemy_strike` | unchanged: a `Shot` and a `Hit` never knew whose body they were |

In `app`: `resident_name(station, who)` over a machine, which now reads
`Session::resident_droid` first and writes the kind (`names::droid_name`)
instead of somebody's name.

**The seam the machines needed taught.** `visit` hands the crew's room
the residents' **bodies** — its Bims and then its machines, one index
space — and reads the hits back against the same: a hit past the Bims is
`room.strike_droid(i, DroidPart::hit_by(hit.roll), hit.damage)`, and
everything else lands as it always did. `Aboard::count` and
`Aboard::position` are `body_count` and `body_pos` now, so the positions,
the exposed positions, the weapons, the peeking and the dodge lists all
grow with a wave; `visit` resizes `down`, `xp_down`, `xp_dead`,
`last_hit_by` and `fee` to the body count each step, since a wave landing
makes the room bigger. `close_residents` counts the **Bims** alone — a
machine destroyed is no loss of the station's people — and the `down`
list the joined deck is told is false for a machine, so a click on a
wreck is a click on the deck and the Loot window never opens on one.
`Residents::join` and `unjoin` carry the machines across with
`Game::take_droids`/`adopt_droids`, by the same shift the Bims take.

Experience falls out unchanged: a wreck is `!is_alive` and never
`is_unconscious`, so `World::experience` pays `XP_ENEMY_DOWN` and
`XP_ENEMY_DEAD` together and once, which is what the feature asked for.

`crates/world/src/tests_droid.rs` is the world's half — a held station
has machines and no people, a hit reaches the one it was aimed at, a
wreck is down and carries nothing, no reinforcement while one lives and
one exactly a clock after the last dies, the farthest airlock, the count
fixed at the first dock, cleared said once, the state surviving a
leaving, the checksum noticing, and two worlds on one seed meeting the
same machines — and `droid::tests` pins the arithmetic.

## A medic carries a body out, and a field medic is hired for it (feature 86)

Two halves of one thing: **a medic can pick a crewmate up**, and **a
mercenary can be hired whose whole trade that is**.

**The carry is the room's, and it is two fields on a body.**
`bims::bim::Bim::carrying` is whom a body has in its arms and
`Bim::field_medic` whether it is a hired one (the first saved and
hashed, the second said by the world every step like the squad's
orders). `Game::{can_take_up, take_up, set_down, carrying, carried_by,
is_carried, needs_rescue}` are the whole of the rule, and
`Game::carry_the_carried` — at the end of every `simulate`, **after**
`separate_under_arms`, or the shove would push the body out of the arms
again — stands each carried body where its carrier stands and lets go of
any carry that can no longer hold (a carrier down, out cold or outside;
a body that died in the arms). A carrier `holds_fire` and walks at
`game::CARRY_PACE` (0.6): both its hands are the carry. A carried body
shoots nothing and walks nowhere of its own.

**The world's side is one command and one getter.**
`Command::Carry { slot, who: Option<u32> }` — `None` sets down —
`World::{can_carry, can_lift, carrying_of, carryable_near}`, and
`WorldEvent::Carried { who, patient }` (87 picked up, 88 set down).
`can_lift` is the gate: **a medic of the class, or a hired field
medic**, and nobody else. The refusals are `NotCarrying` (75, not a
medic of any kind, or a set down with empty arms), `NotHurt` (76, a body
on its feet and whole — a carry is for getting somebody *out*, not for
moving the crew about) and `AlreadyCarried` (77), with `OutOfReach` and
`NotACrewmate` doing their usual work. `clear_carries` goes beside
`clear_beams` at a hire, a mission's start and `drop_crew_member`, since
an index is the whole of what an arm holds.

**A field medic is a mercenary with a trade.** `mercenary::is_medic`
rolls it off the same seed the gear and the price come off
(`MEDIC_CHANCE`, about a third), `Residents::medic` is the derived list
beside `fee` — maintained at the same four places — `priced_as` adds
`MEDIC_FEE` (4 000 a month) before the variance, and `Hired::medic` is
what the contract remembers. A hire of one is `give_field_medic_kit`:
`mercenary::MEDIC_MEDKITS` (two) medkits in its pack, no bandages and no
class — it has **none of the medic's talents**, which is what the user
asked for. `World::is_field_medic` reads the contract and
`hand_the_room_the_field_medics` says it to the room every step.

**What it does under arms is `Game::bot_stand`'s first branch**,
`Game::rescue`, ahead of the last stand and ahead of its player's
standing order:

* carrying somebody — clear of the fight (`Game::out_of_harm`: no target
  up within `RESCUE_CLEAR` tiles **and** nothing a body there could see)
  and it sets them down, and the ordinary medical row takes over, since
  doctoring a crewmate wants it out of harm (or the room calm) and out
  of harm is where it has just been carried — so it is treated there and
  then, fight or no fight (`crates/game/CLAUDE.md`, "A crewmate is
  doctored where it lies out of harm"); not clear, and it runs `Tactics::flee` with the body in its
  arms;
* carrying nobody — the nearest crewmate within `RESCUE_LOOK` (18 tiles)
  that is **out cold or dying**, is not already in somebody's arms, is
  still in the fire and can be walked to (`Game::worth_fetching`), and it
  goes and gets it. A body merely bleeding is left to the medical row:
  it is on its feet and can walk itself out.
* nothing to fetch, and it falls through and fights — **from the far end
  of its reach**. That is `plan_stand` passing
  `combat::KEEP_BACK_WORTH` (6) as the stand's `distance_worth` in place
  of `DISTANCE_WORTH` (1), which is a new last argument on
  `Tactics::{stand_with_cover, stand_scored}` and is the only reason
  either signature moved.

**The restock was `World::restock_field_medics`** — a kit out of the
hold into the pack out of combat — and **it is gone**: a field medic's
medicine is a medic's charges now, four medkits and ten bandages that
come back on the cooldown like anybody's (see "The medicine is
everybody's charges" below). `give_field_medic_kit` is `fill_medicine`,
topping the pack up to them at the hire.

**What moved.** `Refusal` 75–77, events 87–88, `Hired` grew a field and
every body's `carrying` is hashed after `fear` — **`REFERENCE_CHECKSUM`
= `0x_3721_d0ad_d6ef_9c6f`**, **`SAVE_VERSION` 24**, **`wire::PROTOCOL`
16** (the relay wants redeploying). New probes:
`World::{field_medic_for_probe, carry_for_probe}` and
`Session::{field_medics_for_probe, carry_for_probe}`, behind
`BIMS_FIELD_MEDIC=n` and `BIMS_CARRY=1`. The app: `keys::Action::Carry`
(**G**), `screens::game::{carry_key, ability_keys, affected_by}`,
`theme::{carry_mark, fall_back_mark, stand_ground_mark, affected_ring,
rallied_mark}`, and the Hire window naming a field medic
(`names::FIELD_MEDIC`).

## The dressings are carried, and the Management tab says how many (feature 87)

> **Since the medicine became everybody's charges** (next section) the
> restock below, the Management tab's number and the stow of a box are
> gone: a pack is filled by the bandage cooldown, never out of the hold,
> and a box of dressings cannot be stowed. The stacks in the pack are as
> this section says. Feature 104 took the rest of what it leant on: the
> room's manager and its food targets went with the needs (the bandage
> target, `manager::Stock::Bandages`, with them), and the plunder's
> top-up with the human enemies.

A bandage used to be a number the world handed the room off the hold and
read back after the step. It is a **thing in a pack** now — the room's
half is `crates/game/CLAUDE.md`, "A bandage is a thing in a pack" — and
the world's side is three small pieces and no new state at all.

- **`manager::Stock::Bandages = 4`** is the one target that is nobody's
  shelf: how many dressings *each* crew member is to have in its own
  pack. It rides the `CrewOrder::StockTarget` the food's targets ride,
  so the Management tab's box crosses the seam as a command like any
  other, and `Manager` keeps it beside the four — `BANDAGES_CARRIED`
  (3) at dawn rather than nought, unlike the stew and the fibre: a crew
  with nothing to bind a wound with bleeds out the first time anybody is
  shot, and a hold with no bandages in it restocks nobody, so nothing
  moves on a ship that carries none.
- **`World::restock_bandages`**, a step before the hold's medicine is
  handed over, fills every crew member's pack back up to that number,
  **one dressing a step and out of combat only** (`Game::calm`, the
  room's own twenty seconds). Nobody walks for it — it is bookkeeping,
  the way the hold's medicine is handed over without an errand — and it
  conjures nothing: the unit comes off `cargo[Bandage]` and off the
  locker grid together, the way a fetch takes one. The player's own Bim
  is filled like the bots, since the dressing a player winds comes out
  of the player's own pack.
- **The stacks reach the seam in three places.** A `Stow` of a box moves
  the **whole** stack (`Gear::units` of the cell, `has_room` asked for
  that many); a `Fetch`, a `Plunder` and the restock top up a box that
  has room before taking a cell of their own
  (`Gear::stack_with_room`); and a `Loot` of one takes what the cell
  held (`Game::body_units` asked before `take_from_body`, which empties
  it) and puts back on the body whatever would not fit.
  `Game::loot_counts` / `World::loot_counts` are the same numbers for
  the Loot window.

**Nothing new is hashed, and `REFERENCE_CHECKSUM` did not move.**
`world_checksum` never hashed a crew member's pack — a piece is in it by
id and cell, a gun by the hold's list, and a stack not at all — and the
one place `eat_gear` runs is a station's graves, where the cell's count
now goes in beside its `turned`. A grave's box of dressings is part of
what the crew took or left. **`SAVE_VERSION` 25** and
**`wire::PROTOCOL` 17** did move: `Gear` grew `count`, `Room` lost
`bandages`/`bandages_used`, `Manager` grew `bandages`, and `CrewOrder`
grew `BandageAll` (the *Bandage all wounds* row, `Game::bandage_all`).

`the_crew_fill_their_packs_with_dressings_out_of_the_hold` in `tests.rs`
is the restock, the hold running out and the stow of a whole box;
`a_wound_is_dressed_with_a_bandage_from_the_hold_and_a_treatment_fetches_the_kit`
is the dressing through the seam. **`without_dressings(&mut world)`** is
the helper every test that lays a pack out by hand or counts the lockers
says first: the target to nought and every pack emptied, so a box of
dressings is not sitting in the cell the test wants.

## The medicine is everybody's charges

The user's words: get rid of medkits — they have a cooldown and charges
for every Bim now, and so do the bandages; a medkit and five bandages
for everybody, a medic four and ten, the same recharge. So **a medkit
and a bandage are two more `class::Charge`s** — `Medkit = 3`,
`Bandage = 4`, codes across the seam — and they are **everybody's**:
`Charge::everybody` (`Charge::MEDICINE`), whose `class()` answers
`Class::None` meaning *any*. The machinery is the one features 88 and
90 built, so everything that asked about charges asks about these too:

- **`World::charges`** is `class::MEDKIT_CHARGES` (1) and
  `BANDAGE_CHARGES` (5) for anybody, `MEDIC_MEDKIT_CHARGES` (4) and
  `MEDIC_BANDAGE_CHARGES` (10) for a medic **of either kind** —
  `can_lift`, the class or a hired field medic, since the trade is the
  medicine. **`charge_cooldown`** is `MEDKIT_COOLDOWN` (60 s of the
  clock) and `BANDAGE_COOLDOWN` (30 s), the same for a medic.
  **`charges_of` counts by the unit** (`Gear::units_of`), since five
  dressings lie in one box; a kit or a grenade is one a cell as before.
- **`restock_charges`** fills a short pack one charge a cooldown, in
  combat as out of it. **`fill_medicine(who)`** tops a pack up at once
  instead: every crew member at `World::start`, a hand joining
  (`take_resident_aboard`, a hire or a recruit), a medic's class
  (`give_medic_kit`) and contract (`give_field_medic_kit`);
  `take_medic_kit` takes the pack back down to everybody's.
- **Nothing is the hold's.** `restock_bandages` and
  `restock_field_medics` are gone; `hand_the_room_the_hold_s_medicine`
  sets the room's shelf to **nought** and no kit stands, and hands it
  each crew member's own medkits (`set_pack_kits`, which leaves out one
  already in that Bim's hands). A treatment is opened where the helper
  stands, and **the kit leaves the pack when the treatment is done** —
  `settle_medics`, off `Healed { with: Medkit }` — not when it is taken
  up, so a treatment given up for a shot (`care_gives_way`) has put
  nothing anywhere and the kit is still in the pack. `bank_medicine`
  only drains the room's counts now; `take_medkits_used` and
  `take_pack_kits_used` mean nothing to the world.
- **A charge stays in its pack**: `stow` refuses a medkit or a bandage
  with `Refusal::ChargeKept` (81) — one stowed would be one more in the
  hold every cooldown for nothing. A kit fetched out of the hold is
  still a kit, and bought ones still sit there; the recipe, the trade
  and the playtest ship's cargo were left alone, since taking a
  resource off a shelf moves every galaxy checksum.
- **A charge is not worth**: `World::worth` leaves every charge in a
  pack out, so spending a bandage does not shrink the enemies scaled on
  it.
- **`medicine_off_for_probe`** (a serde-skipped flag, never hashed) is a
  test's: no medicine dealt or come back, so `without_dressings` empties
  the packs of both and they stay empty.

The fight's crew (`Session::combat`, under `droids`) sails with
**four** field medics (`session::COMBAT_MEDICS`) in a crew of
**sixteen** (`COMBAT_CREW`), the two the user asked for added rather
than two guns traded for them. (Sixteen crew put up `ENEMIES_MAX` by
the human garrison's formula alone, so `data::ARENA_GARRISON` was that
cap, sixteen, until the garrison went in feature 104.)

**What moved.** `charge_timers` is five a crew member and the packs
hold medicine from the first step — **`REFERENCE_CHECKSUM` =
`0x_da4f_9df9_1693_e8d2`**, **`SAVE_VERSION` 32**, **`wire::PROTOCOL`
24** (the relay wants redeploying). The dials are `BIMS_MEDKITS=n` and
`BIMS_BANDAGES=n` (`set_charges_for_probe`). `tests_medic.rs` pins it:
`everybody_carries_a_medkit_and_five_bandages_that_come_back_on_their_cooldowns`,
`a_helper_treats_with_the_kit_in_its_own_pack_and_the_hold_is_never_touched`,
`a_field_medic_carries_a_medic_s_medicine_and_a_spent_kit_comes_back_on_the_cooldown`;
and `tests.rs`'s
`a_wound_is_dressed_and_a_trauma_treated_out_of_the_helper_s_own_charges`
and `the_packs_fill_on_the_cooldown_and_never_out_of_the_hold`. The
residents are **unchanged**: a station's people still get
`RESIDENT_BANDAGES` in their packs and `RESIDENT_MEDKITS` on their shelf
at open, and nothing of theirs comes back.

## The crisis: an origin, a hop count and a day (feature 92)

> **Since feature 102 the first day is nought**: the crisis is there from
> the start, and `DROID_FIRST_DAY` below, which nothing read after that,
> went in feature 104. See "A run" at the end of this file.

Feature 83 built the machines and left "which stations are held" to a
later step. This is that step, and the thing to hold on to is that the
**rule has no state at all**:

> a star is infested from `crisis_first_day + DROID_SPREAD_DAYS * hops`
> onwards, where `hops` is its lane distance from the origin.

No per-tick roll, nothing accumulated, nothing to keep in step: two
clients that agree about the day, the galaxy and the origin agree about
every star in the galaxy without a word passing between them. `data`'s
numbers are `DROID_SPREAD_DAYS` (5) and `DROID_ORIGIN_MIN_HOPS` (8), and
were `DROID_FIRST_DAY` (10) beside them until feature 104; the graph is
`worldgen::Galaxy::lanes` (`crates/worldgen/CLAUDE.md`). Jumping was
untouched by this step — feature 93 put it on the lanes, next section.

**What is kept, and what is derived.** `World::droid_origin` is one `u32`
rolled once at `World::start` by `droid::origin` — off `Purpose::DroidOrigin`
(12), the galaxy seed and the crew's *starting* star, among every star at
least `DROID_ORIGIN_MIN_HOPS` hops off, or the furthest the galaxy has if
none is — and it is **saved and in `world_checksum`** with
`World::crisis_first_day`, the probes' dial. `World::droid_hops`, the hop
table from the origin, is `serde(skip)` and hashed nowhere: it is a pure
function of the galaxy and the origin, so a save carries two integers
rather than a thousand, and `World::settle_crisis` works it out again —
called from `World::start` off the galaxy already in hand, and from
`ship::Game::resume`, which is every load, every restart and every guest
handed the host's world. `infested_on(star)`, `infested(star)` and
`infested_stars()` are the readings; `start_star_hops_for_probe` is the
`crisis` command's way to a star a stated number of hops off.

**The flip is the state, and it happens on the way in.**
`World::spread_crisis` does nothing unless `infested(star_id)`, and
nothing while the ship is **docked** — the rooms joined, one deck. Since
only travel moves the world clock (feature 103), the day that turns a
system can only come on a trip, and `World::travel` asks
`spread_crisis` at the new day with the ship still holding, **before**
`arrive_at` ties it up — so a system whose day has come is the
machines' before the crew arrive in it. The call in the step, just
before `settle_droids`, is kept for a ship holding and for the probes;
a station's own room open *alongside* a holding ship is opened again
with the machines in it through `reopen_residents`. When it does
go, every station of the system, `stations` and `surfaces` alike, goes
through `World::infest` — feature 83's own door, so `people_of` is
nought, `mercenaries_of` is nought, `stance` is Hostile and the room
opens with a wave — and `WorldEvent::Infested { star }` (89) is said
once. Two things it does not touch: **a station the crew cleared**,
which keeps the `Infestation` it was cleared with so `infest` refuses it
and the crisis never re-arms it; and anything about open space.

**`World::infested` is per-system, like everything else that names a
station id.** It went onto `SystemMemory` with the rest (`crate::memory`)
— a jump that carried the list would hand the next system's station seven
the last one's wave — and the jump's never-been-here branch clears it. A
memory of a system the crisis has taken since is **overrun** on the way
back (`SystemMemory::overrun`, called by `recall_system` when
`infested(star)`): the chart, the rocks and the keys stand, since the
crew saw those and they are still there, but the `losses` and the
`graves` are dropped (and the `hostile` list was, while there was one)
— the people are gone, so what the crew did to them is nothing the crew
will meet. The **infestations stay**, cleared flags and all. The
flip itself clears the live `losses` and `graves` for the same reason.

The app's side: the galaxy chart draws the lanes faintly under the stars
and crosses every infested star in the enemy's red **charted or not** —
the crisis is not a secret — and `screens/game.rs::crisis_line` says under
the star you pick which day it is due or since when it has been theirs.
`tests_crisis.rs` is the rule (the origin's distance and the day
arithmetic, two builds agreeing about every hop, the flip waiting for the
crew and then taking the system, a cleared station staying cleared, a
system overrun while the crew were away remembering none of its people,
and two worlds on one seed falling alike to the checksum), with
`crisis_spread_over_ten_seeds` (`#[ignore]`) printing the measurements the
root `CLAUDE.md` carries. `REFERENCE_CHECKSUM` = `0x_3d19_a6e0_51f2_fdfa`;
**`SAVE_VERSION` 28, `wire::PROTOCOL` 20** (the relay wants redeploying).
The probe is `nix run .#crisis`, `Session::crisis_for_probe`, with
`World::{set_crisis_first_day_for_probe, set_droid_origin_for_probe,
set_day_for_probe}` under it.

**Not in this step**, and deliberately: jumping along lanes rather than
freely, the Machine Heart, the jammer, stopping or reversing the spread,
what an infested system does to prices or to hiring elsewhere, and droids
attacking a friendly town.

## Jumping along lanes, and the jammer (feature 93)

The crisis crawls along the hyperlanes; this is the step that makes the
**crew** crawl along them too, and then shuts one direction of them.

**A jump is one hop, and only down a lane.** A trip's quote
(`World::travel_quote`) refuses a site in a star `World::laned_to` says
no lane reaches from here with `Refusal::TooFar` (87); until feature 104
the helm's charge (`begin_jump`) refused it `Refusal::NoLane` (78), and
that went with the flown jump. Nothing chains: a trip is at most one
jump. `World::route_to(star)` is
`worldgen::Galaxy::route` asked from where the ship is (breadth-first,
each star reached from the **lowest-numbered** of the nearest ones, so
the same route comes out of every build), `World::reachable_stars` is
the lanes out of here, and the app's chart draws both — the reachable
lanes bright over the faint web, the route as a chain. (The helm's
**Jump** button charged for `route[1]` rather than for the star picked,
until the flown jump went in feature 104.)

**The jammer is one station a system, and `World::jammer_station` is the
only place it is decided**: the **orbital station with the lowest id**, a
derived one excluded from the running, and where a system has no orbital
station at all the derived one (`crate::jammer`). A town on a planet's
surface is never it — a system may have no orbit worth the name, and
every infested system has to have exactly one. `None` in a system the
machines have not got.

**While it stands, the lanes inward are shut.** `World::jammed()` is "this
system is infested and its jammer is not cleared";
`World::jammed_step(from, to)` is that plus `to` being **fewer hops from
the origin** than `from` (`World::hops_from_origin`, off the crisis's own
`droid_hops`), and a trip's quote says `Refusal::Jammed` (79) for it
(the helm's `begin_jump` did, until feature 104).
Sideways and outward are accepted, and a jump **into** an infested system
is never refused: getting in is free and getting back out the way you came
is not. `jammed_step` asked about the system the ship is in uses the live
answer, and about any other star assumes the jammer is standing, which is
what the chart bars along a route. Clearing the station lifts the jam for
good — the `Infestation`'s `cleared` flag is saved and hashed like any
station's, so a save, a load and every later spread leave it down.

**A derived jammer is rolled, never saved.** `crate::jammer`: id
`JAMMER_BASE | star` (`0x1000_0000`, clear of the generator's and of
`SURFACE_BASE`, and of the raiders' `RAIDER_BASE` while there were
raiders), kind `Relay` — "deep space, on its
own, listening" is what the machines have made of it — `hostile`, its
plan, map seed, shelf and **position** off `Purpose::Jammer` (13) from
the galaxy seed and the star. The position is `jump::clear_point`, which
is `landing_point` generalised: the same ring search from a stated first
ring and with every bearing turned, so the jammer stands well outside a
landing's ring and half a sub-step off every bearing one would try, and
the two can never come out the same point.

**`World::settle_jammer` is the one door**, and it both lays it and takes
it away: whatever a save or a memory carried is stripped from
`self.system.stations`, `self.stations`, `self.station_keys` (truncated —
the derived station is always last, its id being past everything the
generator numbers) and `self.discovered`, and then it is rolled again if
the system wants one. It is called from `World::start`, from `jump`
**before the landing point is picked** (so the ship never arrives inside
it), from `spread_crisis` before anything is infested, and from
`settle_crisis`, which is the call every load goes through
(`ship::Game::resume`). The station is **charted the moment it is laid**,
the way the crisis is not a secret: a jammer the crew cannot find is a
system they cannot leave.

**What tier the machines come at is a reading now.** `World::droid_tier`
is **tier three within `data::DROID_TIER_THREE_HOPS` (2) hops of the
origin** and tier one everywhere else, until there is a general rule for
what tier an enemy carries; `World::droid_tier` the field became
`Option<Tier>` and holds the probes' override alone
(`set_droid_tier_for_probe(Some(..))`, `BIMS_DROID_TIER`). The checksum
eats the **answer**, as it did before, so it is a function of the origin
and the star and two clients agree without exchanging a word.

**What moved.** `Refusal` 78–79 (78, `NoLane`, went in feature 104),
`Purpose::Jammer = 13`, **`SAVE_VERSION`
29, `wire::PROTOCOL` 21** (the relay wants redeploying). No checksum
moved: at a start the spawn is nowhere near the origin, so `droid_tier()`
is `Tier::One` as the field was, and a system that is not infested has no
jammer in it. `tests_jammer.rs` is the rule — the lane, the route, the
three ways out of a jammed system, a jump into one, the cleared station
staying down through a load and a further spread, exactly one jammer a
system, the derived one standing clear and rolling the same on two
clients, the tier by distance, and two worlds jammed and freed alike to
the checksum — with `jammers_over_ten_seeds` (`#[ignore]`) printing the
measurements the root `CLAUDE.md` carries; the save's half is
`save_round_trip_keeps_a_jammer_down_and_rolls_the_derived_one_again` in
`crates/ship`. The probe is `nix run .#jammer`,
`Session::jammer_for_probe`.

**Not in this step**: the Machine Heart; stopping or reversing the spread;
what infested systems do to prices or hiring elsewhere; droids attacking a
friendly town.

## The front: prices and mercenaries near the infection (feature 94)

The crisis is a disc on the lane graph, and its **edge** is a place: the
systems just outside it are where the machines are coming next, and that
is felt at the desk and in the hiring hall before it is felt at the
airlock. Like the spread itself, **none of it is state** — two readings
off the day and the hop table the crisis already keeps, saved nowhere and
hashed nowhere.

- **`World::crisis_radius()`** is how far the infection has spread by
  today, in hops: the largest `n` with `crisis_first_day +
  DROID_SPREAD_DAYS * n` on or before `days_gone()`, and `None` before
  the first day, when the machines hold nothing at all.
- **`World::front(star)`** is how far a star is *outside* that disc —
  one for a star on the edge, and up. `None` before the first day, for a
  star already theirs, and for one the lanes do not reach.
- **`World::front_at(station)`** is the same for a station's desk, and
  `None` past `data::FRONT_HOPS` (3). The station is an argument rather
  than the system because a station can carry a front of its own.

**The premium is on war goods and nothing else.**
`World::front_bias(station, resource)` is `data::FRONT_BIAS` (5) ×
(`FRONT_HOPS + 1 − d`) — fifteen per cent on the edge of the infection,
ten two hops out, five three hops out, nothing beyond — for a resource
`economy::market::war_goods` says yes to: the five weapons, the four
pieces of armour, the medkit and the bandage. That list is a `match` with
a row a resource in `economy`, so a resource added to `physics` is a
compile error there rather than a thing quietly priced as groceries; the
class charges (a sandbag kit, a sentry kit, a grenade) are **not** on it,
since features 88 and 90 made those abilities on a cooldown rather than
things a desk stocks.

**`World::quote(station, resource)` is the one place a price is worked
out**, and the premium is added to the station's own rolled lean inside
it: `buy`, `sell` and `ship::Session::quote` — which is every panel and
the trade window — all go through it, so nothing can show one price and
charge another. `None` where there is no desk: a derelict, a station
the machines hold (and a raider, while there were raiders). The sum can exceed `market::MAX_BIAS`, which
is why `economy::market::MAX_FRONT_BIAS` is written down there and
`quote`'s own "the bid is under the ask everywhere" test runs out to
`MAX_BIAS + MAX_FRONT_BIAS`; `tests_front.rs` pins that the world's two
numbers stay inside it.

Nobody's shelf stocks a weapon or a piece of armour
(`StationKind::sells`), so the direction the front is *felt* in for those
is the **bid**: a crew selling its kit a hop from the machines is paid
over the odds. Medkits and bandages are stocked, so both sides move.

**One more hand for hire.** `mercenary::how_many` takes a `near_front`
now — `World::mercenaries_of` passes `front_at(station).is_some()` — and
adds one, still capped at `MERCENARIES_MAX`. Nothing new is hashed: how
many a station has was always a function of the seed and the worth when
the room opens, and the day is one more thing it is a function of.

The app's half is one line in the trade window (`names::front_premium`,
`FRONT_PREMIUM_TIP`, through `theme::asks` so the underlined words carry
what it is charged on) off `Session::front_premium()`.
`BIMS_CRISIS_DAY=0 BIMS_TRADE=1 bims crisis` is the picture: the origin
already red two hops off, the line under the desk's own, and the guns,
the armour and the medicine dearer with the food unmoved.
`tests_front.rs` is the rule.

## Defending a town (feature 94)

The one fight in the game that happens **inside one room**. Everything
else is two rooms exchanging targets across the seam — the crew's and a
hostile station's — and here the machines and the town's own people are
in the *same* room, the residents', while the crew are in theirs. So
there are three target lists rather than two, and the hits between the
two sides in that one room never cross anywhere.

**Threatened is derived, and it is the front's.**
`World::town_threatened(station)` is a friendly town on a planet's
surface in a system the infection is **one hop** from
(`World::front(star_id) == Some(1)`), not already the machines', and not
one the crew have already held. Nothing is saved for it; the day and the
hop table say. The system map tags such a town `threatened` in the
enemy's red where it would otherwise say `land`, and a town the crew held
`held`.

**The attack is `World::defenses`**, one `crate::defense::Defense` a town
the crew have ever landed at while it was threatened, saved and in
`world_checksum`. What it holds is the schedule — which wave is on the
ground, how many are still to come, how long until the next, how many
machines are still standing, and whether it was won or lost. Three things
about it that differ from the droid step's own `Infestation`:

- **The clock is steps left, not a moment of the clock.** `next_in`
  counts down in `defense_waves`, and `defense_waves` runs only while the
  ship is on the pad. It was built so that **taking off paused the
  attack** and landing again resumed it where it stood; since feature
  103 leaving the town with waves left is losing it (`leave_mission`
  infests it, `WorldEvent::TownFell`), so the pause is never met in a
  run.
- **The wave on the ground is put back.** A town's room is built afresh
  whenever it is opened, so `Defense::standing` is how many machines were
  still up when the world last looked, and the first step of the room
  lays exactly that many at the gate. Laying the wave again at full
  strength would be a fight that could be won or lost by leaving.
- **Every wave arrives.** There is no wave one already standing the way a
  held station has one: the first lands `data::DEFENSE_DELAY_STEPS` (an
  hour of the mission clock) after the crew set down — time to walk the
  town, trade and hire — and the rest `DROID_REINFORCE_STEPS` after the
  last machine of the one before dies. The count and the size are the
  droid step's own formulas.

**The room is told three things and works the rest out.** In `visit`,
when `World::defense_here()` is `Some`:

- the **crew's** targets are the machines alone — the whole body index
  space is handed over as ever, with every one of the town's Bims marked
  not alive, so the crew never aim at a townsperson and a hit read back
  past the Bims still lands on the machine it was meant for;
- the **town's** targets are the machines in its own room, by droid
  index, and since its room is friendly (`set_hostile_bodies(false)`)
  its bolts **fly there** and its hits come back as ordinary friendly
  `Hit`s, which `visit` delivers to the droid they were aimed at —
  `room.take_hits()` of the *residents'* room, the one place that is
  drained;
- the **machines'** own list (`Game::set_machine_hostiles`, feature 94):
  the crew across the seam first and the town's people after them, with
  `cross` saying where the list turns. A machine shooting at an index
  below `cross` records a `Shot` for the world to fly on the joined deck,
  as it always did; at one above, it fires a hostile bolt **in the
  residents' room** and `Combat::step` finds the townsperson.
- and **who shelters** (`Game::set_sheltering`): everybody of the town's
  own but the guard (`surface::GUARD`) and the mercenaries. A sheltering
  body is posted at the nearest bunk — the nearest house — and left out
  of the alarm's muster, so it holds no weapon and fights nobody.

**Won** is the last machine of the last wave destroyed:
`WorldEvent::TownHeld` (code 90) said once, the station onto
`World::held_towns` (saved and hashed), and **`World::infest` refuses a
held town for ever** — so the crisis taking the system round it leaves it
friendly, trading and hiring. `World::front_at` reads a held town as one
hop out whatever the chart says, since it is then the last friendly desk
inside the infection.

**And some of its people go with the crew.** `defense::joiners` is the
larger of one and `survivors × DEFENSE_JOIN_PERCENT (20) / 100` rounded
down, never more than the survivors other than the guard — so a town with
nobody left, or only the guard, sends none. Who: the lowest indices,
never the guard and never a mercenary. Each goes through
`World::take_resident_aboard(who, for_hire: false)` — the block lifted
out of `World::hire` — which is the hire's own move **without** the fee,
the `Hired` row and the kit (and without the bunk check, while the hire
had one): a classless crew bot like any other, taking no player slot,
keeping what it carries and any wounds it has.
`WorldEvent::TownsfolkJoined { count }` (91) once.

**Lost** is either every one of the town's own people dead
(`World::town_is_dead`, the mercenaries not counted) or the system's day
coming while the crew are away with waves left — or, since feature 103,
the crew leaving it with waves left — and in every case the town falls
through `World::infest` like any other station, which marks the
`Defense` lost on the way past. While the crew are *at* the town the
crisis's flip waits anyway: `spread_crisis` does nothing while the rooms
are joined, and an arrival at a town is a dock.

**What moved.** `Refusal` none, events 90–91, `REFERENCE_CHECKSUM` =
`0x_262b_281e_2a67_eae9`, **`SAVE_VERSION` 30, `wire::PROTOCOL` 22** (the
relay wants redeploying). New `data` constants `DEFENSE_DELAY_MINUTES`
(60; `DEFENSE_DELAY_STEPS` since feature 103) and `DEFENSE_JOIN_PERCENT`
(20) beside the front's. The probe is
`nix run .#defense` — `Session::defense_for_probe`, the origin one hop off
and both clocks a minute — with `BIMS_DEFENSE_DELAY` over the first.
`tests_defense.rs` is the rule and
`save_round_trip_keeps_a_held_town_and_an_attack_under_way` in
`crates/ship` the save.

**Not in this step**: the Machine Heart; stopping the spread; attacks on
orbital stations; droids raiding the ship; a second ship following the
first.

## What the money rework moved (feature 95)

The numbers, in one place, since this file quotes a good many of them
along the way and the money rework moved most at once:

- **`REFERENCE_CHECKSUM` = `0x_00fd_e444_a606_d8d2`**, `SAVE_VERSION`
  **31**, `wire::PROTOCOL` **23** (the relay wants redeploying), and
  `worldgen::GENERATOR_VERSION` **7**, which re-rolled every galaxy: new
  systems, new stations, new shelves — and a new raid schedule, since
  `Raids::stream` mixed the version in (the raids went in feature 104,
  and with them `World::raid_due_for_probe`, which two workbench tests
  called to keep from being boarded halfway through).
- `shipdesign`: `REFERENCE_HASH`, `PLAYTEST_HASH`, `PLAYTEST_PARTS`
  (667) and `COMBAT_PARTS` (675) re-pinned; `CARGO_SLOTS` **18**;
  `ALL`/`PARTS` **49** with `PartKind::Smelter` gone and 31..49 closed up
  to 30..48.
- `worldgen`: `REFERENCE_CHECKSUMS` re-pinned, `Purpose::GearTrade` = 14.
- `economy`: `Storage` is three — `ColdStore`, `Locker`, `Research`.
- `world`: `Refusal::NotEnoughMoney` = 80, `WorldEvent::Bounty` = 92,
  `WorldEvent::Mined` deleted, `Talent::PackMule` deleted (`ALL` 69),
  `World::GRID_CLASSES` two long, `Command::Buy` grew a `tier`.
- `tests_money.rs` is the new file: what a crew are worth, the bounty,
  and where the gear is sold. The construction half of it is in
  `tests.rs` beside the other building tests.

## A run: the switches, the crisis from day nought, and a station's routine (feature 102)

The first step of the roguelike redesign (the root `CLAUDE.md`, "A run")
switched the old game off rather than deleting it: four fields on
`World`, saved and hashed, **all off from `World::start`**, each with a
setter the tests called right after they built their world. Feature 104
deleted three of them with what they switched (the section at the end):

- `needs_enabled` was told to every room every step
  (`hand_the_rooms_the_needs`); off, the food in a dark cold store did
  not spoil, a town's guard was not posted, and a hire was never refused
  for want of a bunk. The world's half of the needs went with it, and
  the fixed tell that stood in for it (`hand_the_rooms_the_run`) went
  with the room's half.
- `human_foes_enabled` put the generator's hostility for the system
  (`rolled_hostile`, home excepted) on the world's `hostile` list; off,
  the raids never fell due, the plunder laid nothing and a loot of a
  resident was refused. The list, the raids and the plunder went with
  it, and the generator's roll stays ("A station has a stance").
- `radiation_enabled` let stage eight dose; the dose, stage eight and
  the `health` crate went with it.
- `shipyard_enabled` **stays**, off in every run: `can_place_site`
  answers `SiteRefusal::NoShipyard` first (`Refusal::NoShipyard`, 82),
  and `World::buyable` is gear alone, so `buy` refuses the rest
  `NotSoldHere` before anything else is asked. The tests of building and
  of the shelf switch it on.

**The crisis is there from day nought**: `crisis_first_day` opens at
nought, so the origin is infested at the start and `front` has a value from
the first step; a system due is the machines' whole the moment the crew are
in it (the flip, the jammer, the waves), exactly as the stateless rule
always said. `DROID_FIRST_DAY` was no longer read, and went in feature
104 — the tests that used it as a probe's dial write their own day — and
the measurements in the root
`CLAUDE.md` (*How fast the crisis crosses a galaxy*) are ten days early
now: the whole galaxy is theirs by day 480 to 715.

**A station's people are dealt a round** as their room opens —
`open_residents` calls `Residents::deal_roles` unless the station is
hostile then (the machines', since feature 104) — off `map_seed` and the body's index (`bims::routine::deal`:
a town's first its guard, a trading station's first its trader, the rest
rolled, a mercenary a civilian), the town's two gates added to the guard's
ways in (`surface::gates`, off the wall's own constants). A body taken
aboard — a hire, a townsperson joining — loses its round
(`take_resident_aboard` → `Game::clear_routine`). The residents' room is
not in the checksum, so the rounds are not either; `tests_run.rs` pins that
two runs on one seed walk them alike, position for position.

## The loop: travel resolved, and a mission's start and end (feature 103)

The second step of the redesign. `crate::run` is the types — `Run`,
`Phase`, `Site`, `Proposal`, `Departure`, `Fallen`, `SiteSnapshot`,
`TravelQuote` — and `mission.rs` is the world's side of them, **a child
module of `world`** (`#[path = "mission.rs"] mod mission;` near the top
of `world.rs`), so it reaches the private fields the rest of the `impl
World` blocks do without growing `world.rs` by a thousand lines.
`World::run` holds all of it, saved and in `world_checksum` at the very
end.

**Two clocks.** `clock_minutes` is the **world clock** — the day, the
crisis, the front, the wages — and in a run it moves **only** in
`World::travel`, by the trip's whole minutes in one go. `Run::mission_steps`
is the **mission clock**, nought on arrival and one a step: the droid
waves (`Infestation::next_wave` is a mission step now, `droid_reinforce`
steps, `data::DROID_REINFORCE_STEPS`), a town's first wave and its
countdown (`Defense::next_in` in steps, `defense_delay`,
`data::DEFENSE_DELAY_STEPS`), and every class timer read through
`World::mission_minutes()` — `charge_timers`, a taunt, a rally. The probes'
dials are still in minutes of it (`set_droid_reinforce_minutes_for_probe`,
`set_defense_delay_for_probe`), turned into steps by `steps_of`. A room's
own clock does not run: `Game::simulate` never advances it and
`Game::wind_clock` is the one thing that moves it, so a room's time of day
stands where it was built — wound to the world clock at every build
(`Aboard::joined`, `mirrored` and `unjoined`, `Residents::open` and
`unjoin`) — and `World::day()` read off the room stays the world's day.
Nothing tells a room so: that is simply what a room does since feature
104 took the room's clock switch away with the needs.

**The old game's clock was a switch**, `Run::free_clock`
(`World::set_free_clock`), off in every run in the pattern feature 102
set: on, the world clock ran with the step, `pay_wages` with it, and the
helm's four orders were taken; off, `Command::{Confirm, Abort, Jump,
Land}` were refused `Refusal::TravelIsResolved` (83). Feature 104 deleted
it with the flown trip, the four commands and the refusal. A test that
wants time to pass steps a mission and reads `mission_minutes()`; wages
are paid by `travel` alone; and the reference run
(`fixture::reference_run_world`) is a mission at the spawn, both players
back to the ship, a resolved trip and a mission at the other end.

**The step's order grew two ends.** Stage 0: in `Phase::Map` the commands
are applied, the step is counted and nothing else happens. Then
`open_the_mission` photographs the site the first step of a mission
(`SiteSnapshot`: the site's `Infestation`, `Defense`, `Losses`, graves
and lamps — and its plunder, until feature 104 — **not** its research
key or a hire, which the crew carried away), before the commands. The
last stage — 9 then, 8 since feature 104 took the radiation's — is
`settle_run`: the pending bounty paid the step the site is cleared, and
the departure check. `check_lost` is `check_run_lost` now.

**The run's commands apply at once** (`applies_at_once`):
`Command::Propose`, `Accept`, `Return`, `LeaveBehind` and `PlayerGone`, so
a vote lands on the map where no step is taken and at a pause. The last
yes of every connected player **is** the trip — `go_if_carried` →
`travel`: the clock on, `pay_wages_due` (a month a pass), `jump` for
another system, the whole system charted, `spread_crisis` **before**
`arrive_at` (so a system whose day came on the way is the machines'
before the ship ties up), `arrive_at` (`Docked`, the frame, `dock_at`,
`settle_residents`, `mark_visited`), `begin_mission`. The host says
`PlayerGone` when the roster loses somebody (`screens/game.rs`), since the
world cannot know who is at a keyboard.

**A quote** (`travel_quote`, `travel_quotes` for the map's whole list off
one generated galaxy) is `physics::travel_days(distance, a_forward,
max(a_forward, a_backward))` of the leg — from `current_site()`'s place,
or `jump::landing_point` for another system — plus
`time::days(JUMP_CHARGE_MINUTES)` for a jump, rounded **up** to whole
minutes for the clock; one lane at most (`TooFar`), `jammed_step` still
`Jammed`, and the state on arrival read off the arrival day: infested, the
tier (the distance rule unless the probe's override), the jammer (lowest
orbital, else derived), a threatened town (the front worked out at that
day). No hyperdrive part is asked for: the default ship has none, and a
trip is resolved. `sites_at(star)` is the stations then the settlements,
and a system with no station gets the derived jammer's id when it is
infested today.

**A mission's start** (`begin_mission`): the run's per-mission state
cleared, `buy_back` (the fallen in death order while the pool holds
`BUYBACK_COST`; `Game::revive` — alive, whole, awake, **no gear** — and
`strip_the_dead` for the world's armour records; `StillOut` for the rest),
and `make_whole` — `Game::restore_health` for every living crew member
(and the world's radiation `HealthState`s fresh, until feature 104),
beams and carries cleared, every charge timer,
taunt and rally reset and every charge filled (`fill_charges`) — and every
player's standing order back to Follow.

**Cleared** is `site_cleared(id)`: an `Infestation`'s `cleared`, a
`Defense`'s `won`, and otherwise `!town_threatened(id)` — a peaceful site
is cleared from the start, which is also why a hostile Bim's bounty was
paid at once there (none is earned since feature 104). `earn_bounty`
pays at once where
`mission_cleared()` (the site tied up at, and "nowhere" counts as clear)
and makes it pending otherwise; `visit` adds `bounty_for(droid.tier)` for
every machine seen destroyed — the Republic pays for machines now, every
enemy being one — and `experience` a hostile Bim's, which no room has
had since feature 104.

**The end of a mission.** `press_return`: `returning` set, `recalled` on —
which `hand_the_room_the_standing` hands every slot as `Retreat` — and a
press by one already returning clears a `Declined` departure.
`settle_departure` waits for every **waited-for** player (`waited_for`:
connected, not out, alive, not `is_down`) to be returning and
`inside_ship` (`Aboard::on_ship` against the ship's own design), and at
least one to exist; then `left_behind()` (every living crew member outside
the ship) empty is `leave_mission`, and otherwise `Departure::Asking`. An
`Asking` whose list changed is asked afresh; one declined is not asked
about the same list again. `leave_mission`: the left behind `kill_now`,
`casualties` (the deaths paid for), the bounty settled or dropped,
`unjoin_rooms` and `close_residents`, then a town under attack `infest`ed
(`TownFell`) or the snapshot restored (`restore_site`), the fallen
stripped, the dead bots dropped (`bury_the_bots` → `drop_crew_member`,
which removes a crew index from every list the world keeps one in), the
ship `Holding` at the site's place, `undocked_once`, and `Phase::Map`.

**Dying.** `casualties` no longer resets a dead crew member's `Progress`
(it is kept for the buyback) and calls `fall`: a player onto `run.fallen`
in death order, a bot `BotLost` and `min(money, BOT_DEATH_PENALTY)` off the
pool. `check_run_lost`: every player slot's Bim dead — out cold is alive,
and the bots do not count.

**What moved.** `Refusal` 83–91, `WorldEvent` 93–106, `data::BUYBACK_COST`,
`BOT_DEATH_PENALTY`, `DROID_REINFORCE_STEPS`, `DEFENSE_DELAY_STEPS` (the two
`_MINUTES` constants gone), `SAVE_VERSION` 34, `wire::PROTOCOL` 26 (the
relay wants redeploying), and `REFERENCE_CHECKSUM` with the reference run's
loop. **Not done, because it does not exist**: the spec's "Commander's
call-in reinforcements, once per mission" and "temporary Republic
soldiers" — the commander has a rally and a squad, and nothing calls a
soldier in. `tests_mission.rs` is the rule, with
`travel_days_over_ten_galaxies` (`#[ignore]`) printing the measurements
the root `CLAUDE.md` carries.

## The old game deleted (feature 104)

The third step of the redesign (the root `CLAUDE.md`, "The old game
deleted"): **everything features 102 and 103 switched off is deleted,
and nothing a run does moved.** The last commit with the old game is
tagged `needs-sim-final` (29d5d54); `git show needs-sim-final:<path>`
reads a deleted file again. What the world lost:

- **The flown trip.** `ShipState` is `Docked { station }` and `Holding`
  and nothing else ("A ship is docked or holding" above).
  `Command::{Confirm, Abort, Jump, Land, ToHelm}`; the helm job and its
  gates (`can_command`, `helm_spot`, `at_the_helm`, `order_to_helm`,
  the world's `stand_down`, `man_the_helm_for_probe`, `HELM_REACH`); `fly`,
  `finish`, `cast_off`, `come_alongside`, `confirm`, `give_up`,
  `begin_abort`, `set_off`, `plan_from_here`, `target_position`,
  `preview`/`Preview`, `hold_point`, `way_out`, the jump's charge
  (`begin_jump`, `charge_jump`, `jump_charge`, `hyperdrive_ready`), the
  landing (`land`, `can_land`, `above`, `landing`, `lifting`,
  `landing_for_probe`), `engine_draw_now`, `Power::engines`,
  `can_modify_part`, `trip_state`, `effort`, `plan`, `trip_progress`,
  `Ship::pending`/`destination_set_by`, `send_everybody_home`/
  `everybody_home`; the flight's events (`Departed`, `Arrived`,
  `Aborted`, `PlanFailed`, `CastingOff`, `Undocking`, `Docking`,
  `Charging`, `JumpFailed`, `Landing`, `Landed`, `LiftedOff`), its
  refusals (`NotAtTheHelm`, `NotTravelling`, `ComingAlongside`,
  `UnderWay`, `UnderConstruction`, `NoHyperdrive`, `NotHolding`,
  `NoSuchStar`, `SameStar`, `NoPlanetHere`, `NoLane`,
  `TravelIsResolved`, `SiteRefusal::UnderWay`) and constants
  (`UNDOCK_MINUTES`, `DOCK_MINUTES`, `CASTING_OFF_LIMIT`,
  `LANDING_HEIGHT`, `LAND_MINUTES`, `LIFT_MINUTES`). **Kept**:
  `World::jump`, `pub(crate)` now and called by `travel` alone, with
  `jump::landing_point`, `WorldEvent::Jumped`, `JUMP_CHARGE_MINUTES`,
  `JUMP_CLEARANCE` and `Refusal::Jammed`; `land_for_probe`,
  `undock_for_probe`, `dock_for_probe`; `SetSpeed`; `laned_to`,
  `route_to`, `reachable_stars`; `under_construction`. The `flight` and
  `physics` crates stay — a trip is quoted off `Ship::dynamics` by
  `physics::travel_days`.
- **The free clock**: `Run::free_clock`, `set_free_clock`, the world
  clock by the step and the wages by the step. Wages are paid by
  `travel` (`pay_wages_due`) with the ship holding, where an unpaid hand
  could never walk off, so `World::dismiss` went too: an unpaid hand
  sails on owed.
- **Radiation**: the whole `crates/health` crate, `World::health` and
  every write to it, `run_health` and stage 8, `WorldEvent::Health`,
  `SUIT_INTENSITY`, `EVA_DOSE_LIMIT` and `radiation_enabled`. The run is
  stage 8 now, and `suit_ok` is the suit aboard — the same answer it
  gave, since nothing ever dosed a body in a run.
- **The world's half of the needs**: `needs_enabled`, the cold store's
  clock and the spoiling (`cold_store_out`, `spoil`, `SPOIL_STEPS`,
  `SPOIL_DIVISOR`, `WorldEvent::FoodSpoiled`), the bay's power
  (`sync_bay_power`), the guard's post (`Residents::post_guard`), the
  hire's bunk check (`Offer::bunk`, `Refusal::NoBunk`) and
  `DROID_FIRST_DAY`. The room's own needs went with the room crate's
  half of the feature (`crates/game/CLAUDE.md`).
- **The human enemies**: `raid.rs` and `plunder.rs` ("Raiders and an
  enemy's shelf" above), the world's `hostile` list,
  `human_foes_enabled`, `rolled_hostile`, `set_hostile`,
  `reinforcements`, the garrison formula (`station::enemies_of`,
  `base_by_day`, `scaled`, `ENEMIES_BASE`, `ENEMIES_MAX`,
  `ARENA_GARRISON`; `ENEMIES_DAYS` stays for the waves), `BOARDERS_MAX`
  and every raid and plunder constant, `Plan::Raider`,
  `Command::{Execute, Plunder}`, the loot of a resident,
  `stage_fight_for_probe` and `outfit_for_probe`'s residents half; events
  `Executed` (48), `RaidContact`–`RaidRepelled` (59–62), `Plundered`
  (64) and `RaidBreached` (67), refusals `NotHostile` (26) and `Unarmed`
  (27). The generator's roll stays (`Station::hostile`: the spawns, the
  tier-two keys, the derived jammer), and so do `World::stance`,
  `apply_stances`, `reopen_residents`, `open_residents` and
  `LootSource::Resident`.

**What stands in their place.** `World::stance` is a held station
Hostile, home Friendly, the rest Neutral. `people_of` is a station's
residents less its dead, nought for a held one. A loot of a resident is
refused `NotACrewmate`. `World::stage_droid_fight_for_probe(kind,
weapon)` stages the one-against-one fight the human
`stage_fight_for_probe` did, with one machine four tiles down the
corridor — armed, or posing with `None` — and crew member 0 recruited
inside the door; `Game::droid_mut_for_probe(i)` is the room's half.
Every test that staged a human fight to test a class or the combat was
ported onto it or onto `World::infest`; the tests of what only a human
enemy did (the garrison's size, the execution, a resident's loot, the
raids, the plunder, an enemy following the crew aboard) were deleted.
`hand_the_rooms_the_needs` became `hand_the_rooms_the_run` for the world's
half of the feature — a fixed tell of `set_needs_enabled(false)` and
`set_clock_runs(false)` to the crew's room and the station's, at
`World::start` and at every step — and **went** with the room's half,
when `crates/game` lost both switches: a room now behaves as it did with
both off, and nothing is told it. The room's side is
`crates/game/CLAUDE.md`, "The needs deleted".

**The codes.** A deleted `WorldEvent` or `Refusal` leaves its code
free rather than closing the rest up — `names.rs` matches by variant,
not by a table indexed by code — and `event.rs` notes which are free.
`STATE_NAMES` is `["Docked", "Holding"]`. `world_checksum` dropped the
switches, the hostile list, the reinforcements, the raids, the plunder
(live and remembered), `health`, `destination_set_by`, the flight
states' payloads, `cold_store_out` and the free clock, so it moved:
`REFERENCE_CHECKSUM` is re-pinned in `world::fixture` with a history
line (`0x_5359_7c7a_29be_b3e2` after the world's deletions, and still
that after the room's, which moved nothing hashed), and
`fixture::reference_run_world` was rewritten without flight — the world
opened, both players at `Speed::Day`, a mission at the spawn, both back
to the ship, the first quotable other site proposed and accepted, a
mission there — with `reference_target` deleted. **`SAVE_VERSION` 35**
and **`wire::PROTOCOL` 27**, and those three are the final numbers: the
room crate's half bumped none of them.

**How it is known that nothing moved.** `world::fixture::Survivors` is
a reading of **only the state that outlived the deletion** — the world
and mission clocks, the run's phase, missions, deaths and pending
bounty, the pool, the star, where the ship is alongside, every crew
member's class, experience and picks, the fallen, every body on the
crew's deck and the site's (where, alive or down, its blood, each
part's health, wounds and trauma, its gear), every machine's place,
kind, tier and body, and every site's state — with an enum hashed **by
its name**, so a variant deleted or a discriminant closed up moves
nothing that did not itself move. `world_checksum` could not do the job,
since the switches and the needs were in it. Two tests pin a reading
taken off `needs-sim-final` before anything was deleted, and **their
constants are never edited** — except by a change *meant* to alter how a
run plays, which says why in the constant's own note (`SURVIVORS` has
moved twice so, for the town defence's experience, below, and for the
Guardian in the town run's tier-three wave, feature 100):

- `crates/world/src/tests_survivors.rs`, `SURVIVORS`
  (`the_run_plays_as_it_did_before_the_old_game_was_deleted`): a seeded
  run, two players and four bots on the combat ship with a gun in every
  hand — the trip to a station of the spawn system handed to the
  machines and the fight there; back to the ship, a jump to a star next
  door and a mission there; and a second world whose own system is on
  the front, the trip to its town and the town's defence. About forty
  seconds: `cargo test -p world --lib tests_survivors`.
- `crates/ship/src/tests_survivors.rs`, `PINNED` and `PICTURES`: every
  command's `Session` stepped and read, and the fixtures' and a run's
  deck drawn bit for bit (`crates/ship/CLAUDE.md`).

**If a survivor number moves, a run plays differently**: find why — a
draw on the room's stream removed, an order of operations changed, a
solid gone from a nav grid, a room told something it was not told
before — and put it back.

**What the deletion found and left alone.** Four things the human enemy
was the only one to reach, which did nothing against the machines; the
deletion left the game as it played, and the three that were game bugs
were **fixed after it**, as a change of their own (each noted where it
lives above):

- ***Rampage*** did nothing: `settle_rampage` clears every stack when
  `enemy_standing()` is false, and `enemy_standing` counted the station
  room's **Bims** alone, so against machines the stack earned was
  cleared the same step. Fixed: `enemy_standing` counts the machines —
  every body at a hostile station, the machines alone in a town the crew
  are defending.
- **The field surgeon's reset** was every step: `settle_medics` puts
  `field_surgery_used` back when `enemy_standing()` is false, for the
  same reason, so a bare field surgery was never held to once a fight.
  Fixed by the same change.
- ***Relentless*** had nothing to bite on: it keeps a mark past an enemy
  out cold until it is dead, and a machine is never out cold — it is
  destroyed at once. Fixed by giving it a second half: an attack whose
  marks are all dead moves on to the enemy standing nearest the
  commander instead of ending.
- **`EnemyDown` is never said**, no room having hostile Bims — and that
  is left as it is: a machine going down is `DroidDown`, which the log
  prints and everything that pays for a machine reads. The variant and
  its log line stay. `Game::recall_outside` lost its only caller
  (`leave_site`, the raid's arrival). `surface::guard_post()` has no
  caller either, the guard's post being gone; `GUARD_POST` is still the
  plan's.

The tests deleted with them — `field_surgeon_is_once_a_fight_and_comes_back_when_it_ends`,
the rampage half of `bruiser_dug_in_deadeye_and_rampage` and the
relentless half of `relentless_and_grit` — came back against the
machines with the fix: `field_surgeon_is_once_a_fight_against_the_machines_and_comes_back_when_it_ends`,
`rampage_is_a_stack_a_machine_downed_until_none_stands` and
`relentless_takes_the_attack_on_to_the_nearest_machine_standing`. No
survivor pin moved for those three: neither survivor run has a crew
member past the first level, so no talent is in either.

**One more gap, found by the same fix and fixed after it**: in a
**town's defence** `experience` paid nothing for a machine downed — it
asked `stance == Hostile`, and a defended town is friendly — so no
`XP_ENEMY_DOWN`, no `XP_ENEMY_DEAD` and no *rampage* stack was earned
there (the Republic's bounty was paid, off `visit`'s own count). Now
`World::first_enemy_body` is the one answer to *which of the residents'
bodies are the crew's enemies* — all of them at a hostile station, the
machines alone past the town's own Bims in a defended town, none
elsewhere — and `experience` and `enemy_standing` both ask it, so a
townsperson going down is still nobody's experience
(`a_machine_downed_in_a_town_s_defence_is_experience_and_a_townsperson_is_not`).
**This moved `SURVIVORS`, on purpose and once**: its town run has an
engineer and a tank, who earn the experience now. The new number is in
the constant with a note saying why; the hash after the first two
worlds is the same with the fix and without it, and with the defence's
half taken out the old number came back, so nothing else moved.
`crates/ship`'s `PINNED` and `PICTURES` did not move.

## The Guardian: a tier-three wave's fourth machine (feature 100)

The machine is the room's (`crates/game/CLAUDE.md`, "The Guardian"); the
world does two things for it and keeps nothing new but a probe's dial.

- **The wave.** `build_wave` asks `bims::droid::wave_kinds(n, tier)` at
  the world's tier, so a Guardian stands in a wave only at tier three —
  a held station, a reinforcement, a town's defence, all within
  `DROID_TIER_THREE_HOPS` of the origin, or wherever the probes force the
  tier — `n / 8` of it, at least one from four, out of the Troopers'
  share. `World::set_droid_kinds_for_probe(kinds)` forces every wave to be
  exactly those machines in that order (`droid_kinds_forced`, saved so a
  restart brings them again, not hashed); `droid_wave_size` answers its
  length. It is the `guardian` command's one Guardian and two Troopers
  (`Session::guardian`).
- **The shield across the seam.** In `visit`, beside the peeking and the
  dodge, every body of the residents' room is asked `Game::shield_of`,
  turned onto the joined deck through `station_frame`'s two unit axes (a
  direction takes no origin and no shift), and handed to the crew's room
  with `set_hostiles_shields` — so a crew bolt is stopped in the crew's
  room, where it would have landed. In a town under defence the town's
  people's targets are the machines in their own room, and their shields
  go with them untouched (`set_hostiles_shields` on the residents' room).

`tests_guardian.rs`: the waves by tier and size, and a staged Guardian
whose shield stops every bolt of a crew member in front of it and none
once it is turned about. **`SAVE_VERSION` 36, `wire::PROTOCOL` 28.**
`SURVIVORS` moved, on purpose: the town run of `tests_survivors.rs` is one
hop from the origin, so its tier-three wave of six has a Guardian in it
(the constant's note says how it was checked that nothing else moved).
`REFERENCE_CHECKSUM` and the ship's `PINNED` and `PICTURES` did not move.

**The beam across the seam.** `visit`'s shot loop lays a recorded
`Shot` with a `sweep` as `Game::enemy_sweep` on the joined deck — its lens
and its two aim points through `on_deck` like any shot — before it asks
whether a shot is a blow. `the_beam_crosses_the_seam_and_two_worlds_agree_through_it`
runs two worlds with an armed Guardian staged side by side, events and
`world_checksum` step for step, until the beam has been laid on the deck
and has hit the crew member.
