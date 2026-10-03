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
`game` flow has had none since feature 102, `screens::designer::build_run`
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
Since task 119 the multiplier is one or nought — 1× or paused, nothing
else — so `MAX_STEPS_PER_FRAME` is only how far a long frame may catch up.

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
a **plan** (`station::Plan`: generated from `map_seed` since feature
112, or one of six drawn plans it falls back on; the section "A station
is generated from its seed" below), sized by plan and kind (32 to 72
tiles) and dressed by `map_seed`, with the port
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
`World::days_gone` stays — the crisis spreads by it — and is
`clock_minutes` floored to whole days rather than the crew's calendar
`World::day()`, so two clients that have travelled the same trips count
the same day; `World::hours_gone` is the same in hours, and the
machines' waves grow a step every `data::ENEMIES_HOURS` of it
(`droid::time_steps`, feature 105 — it was `ENEMIES_DAYS`, thirty, and
`day_steps` until then; "The machines hold a station" below).
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
  `a_sniper_rifle_reaches_from_twenty_tiles_and_a_shotgun_does_as_much_at_nine_as_at_three`)
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
  sides. `the_crew_s_shotgun_does_as_much_at_nine_tiles_as_at_three` pins
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

> **Deleted by the trader (task 114)**: nothing is bought or sold across a
> desk any more — see "The trader" at the end. The desk stays as a
> fixture.

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
went into the sum. Both are what `mercenary::how_many` is scaled
against — and the droid waves were until feature 105, and the human
garrison (`station::enemies_of`) and the raiders' boarders until feature
104. The machines read the world clock and the players alone now.

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

## A station is generated from its seed, and the spawn is the hub whatever it rolled

Feature 112 (September 2026). **Every station but the spawn is a building
generated from its kind and its seed** (`crate::stationgen`), so two docks
of one kind are two buildings rather than one of six drawn plans dressed
differently. `Plan::rolled` answers `Plan::Generated` for every seed;
`station::resolve(kind, plan, seed)` builds it and hands back the plan it
really built — the generated one, or, when `stationgen::ATTEMPTS` (24)
candidates all fail the contract, the drawn plan the seed rolled before
feature 112 (`Plan::hand_rolled`, off `PLAN_SALT` as it always was), which
`Station::plan` then carries. Over a thousand seeds a kind none has fallen
back. `layout(kind, plan, seed)` is `resolve`'s design, cached by what was
asked for; `forget_built` empties the cache for the determinism test.

**What stays drawn.** The spawn is a hub whatever it rolled (`World::start`
calls `Station::replan(Plan::Hub)`), the arena is `station::arena`, the
Machine Heart's fortress is `Plan::Fortress`: none moved, and neither did
their design hashes or `bims --self-check`. The six drawn plans — `Hub`,
`Pod`, `Cross`, `Spine`, `Ring`, `Comb`, `Plan::HAND` — are what the
generator falls back on and what `BIMS_STATION_PLAN=Ring` (the app's dev
dial, `station::set_plan_override`) forces; `BIMS_STATION_PLAN=legacy`
(`set_legacy_layouts`) puts every station back on the drawn plan its seed
rolled and every town on the old template, which is what the moved pins
were checked against. `Plan::ALL` is `Generated` and the six; each drawn
plan still has its size (`Plan::side`: the hub by kind, the others pod
34, cross 44, ring 48, comb 52, spine 60, plus 8 for an orbital, 4 a
refinery, 2 an outpost or derelict), its corridors and its residents as
before.

**The shape: a ladder.** Two or three long corridors (the **rails**, two
to five wide) and two to four short ones (the **rungs**) crossing every
rail. The rectangles between two rails and two rungs are **cells**, cut
into rooms seven to twelve wide that share their walls — a deep cell into
two rooms, one a rail — or, one in five, left as **void**: a courtyard
with the hull's skin round it. Along each rail's outer side a **band** of
rooms against the skin, each as deep as it rolled, so the silhouette
steps; a band may be missing and a room of one may be. A rung may run on
through a band as a docking **arm**; the rails stop at a closing rung or
run on to the skin (an **open end**, with cells between them). The ladder
lies east–west (**end on**, the reactor room's door opening into the
first rung) or, two times in five, north–south (**side on**, into the
first rail). A relay is kept full — no void, no missing room — since it
is small enough that a void is the room a role wanted. Every dial is
`stationgen::dials(kind)`; the build area is the farthest tile plus the
margin and must fall in `stationgen::side_range(kind)` — relay 32–46,
outpost and derelict 38–54, refinery 42–58, orbital 50–66, so a relay is
never an orbital's size and nothing is bigger than the drawn spine's 68.
Cheap failures — the wrong size, no room for a role — are drawn again off
the same stream up to `SKETCHES` (12) times before an attempt is spent.

**Wings** (task 131, the stations' half). The ladder is the **core**, and
two times in three a **compact** one — the fewest rails and rungs the
kind has, the cells at most halfway up their range, an arm one time in
two — so the size the kind allows is left over for wings.
`stationgen::grow_wings` takes every **stub** the ladder left against the
skin (an arm's end, a rail run on to it) and, three times in five (a
relay one in three), runs a corridor on out past it (`Rect::extended`, a
**boom**), one time in two turning once (`Rect::turned`, two or three
wide, flush with the end: a **dog-leg**), lays rooms along either side
of each segment where they fit (seven to twelve along, four to nine
deep, a side skipped one time in four), and puts a **module** across the
far end — wider than the corridor by two to five a side, five to ten
deep — one time in two, and always when no room fitted along it, so a
boom leads somewhere. Every block goes through `fits`: beside every
block drawn as `Rect::beside` asks — sharing nothing but a ring line, or
not touching, but never touching only corner to corner — bar the parent
a corridor grows off, and inside `Bounds` (the kind's largest side,
never west of the port's column; a wing may wrap round above or below
the reactor room). A block that does not fit is shortened, or made
shallower, and left out when nothing does. The sizes now run to the top
of each kind's range (an orbital's median 64 of 66), and none fell back
over fifty seeds a kind.

**The reactor room** is the drawn plans' — `x` 1 to `LOBBY_EAST` (10), 13
or 15 tall, the port in its west skin on rows `py`, `py + 1` rolled so the
desk and the reactor along its north wall and the batteries and life
support along its south are clear of them, and its door on the same rows —
so the straight run in from the port (`MIN_RUN`, nine) and
`stage_droid_fight_for_probe`'s and the ashore spot's tiles are open deck,
as on every plan. The big plant is two rows above the port's.

**Roles** (`deal`): quarters, mess, research, heads and a store must be
dealt, the lab and the rec room where a room is left, and every room left
over is another store; each goes to the smallest room its minimum fits.
The minimums are `furnish`'s own offsets, pinned as constants:
`LOBBY_MIN` (8, 11), `MESS_MIN` (8, 6), `QUARTERS_MIN` (7, 7) with
`bunks_in(w, h)` ≥ the kind's residents and two, `HEADS_MIN` (5, 4),
`RESEARCH_MIN` (7, 6), `LAB_MIN` (7, 5), `REC_MIN` (6, 5), `STORE_MIN`
(5, 4). Residents are `residents_of(kind)` — two, one on a relay, none on
a derelict (whose skin is whole since October 2026: the holes it had
were drawn as black squares in the wall).

**Doors are chosen after a trial furnishing.** The floor is furnished once
with every room shut; a door goes in a wall facing corridor deck where its
two tiles, the two inside and the two outside are free of whatever the
furnisher stood there (`door_sites`), and one room in four gets a second
on another wall — a room to go through. Then the real floor is furnished.
Walls are only rooms' rings (`enclose`); a corridor block has none of its
own, so crossings open into each other and a room's wall *is* the
corridor's edge — no one-tile strip is ever left between them.

**Airlocks, the array, the cover.** The port first; the array on a
straight run of north skin clear of it; then a **trial furnishing** with
every room shut and the port the one way in; then the kind's extras —
one or two on a relay, two or three on an outpost, derelict or refinery,
three or four on an orbital (task 131; the check's least is unchanged) —
on a straight run of skin with space beyond both tiles and deck two deep
inside that the trial left clear: **a corridor's or a room's**
(`Raster::room`, the rooms' insides bar the reactor room's), never on
the array or beside it. Each is drawn among the six sites farthest from
every airlock already chosen, ten tiles apart at the least, a room's and
a corridor's in turn while both are left (which goes first is a roll),
so the ways in are spread round the station and lead into different
parts of it. `droid::arrival_airlock_at` takes them all in turn. Sandbags
only across a corridor three wide or more, a line from one wall leaving
two, straight across that corridor alone (not in a crossing), none
within three tiles of a doorway, the reactor room's or an airlock's.

**The check** (`stationgen::check`, on the furnished candidate): the port
first and in the west skin, the array facing north, the reactor room big
enough, the straight run, every door two deep clear both sides, no
diagonal pinch between two pieces of structure, no hull that meets itself
only at a corner, the flood, beds, airlocks, the two desks, and at least
one **loop** — a piece of structure with deck all round it, a cell or a
courtyard a crew can go round either way. **The flood** (`reach`): from
the window inside the port, a body is a two-by-two window of free tiles
moved a tile at a time, sandbags counted as solid, and every doorway and
airlock must be reached so; from there, a tile at a time four ways, every
use spot and every open tile of deck. The furnisher itself leaves one-tile
gaps — past the end of a run of trays, between bunks — which the drawn
plans have too and the room's grid walks (`a_one_tile_corridor_can_be_
walked`); a four-way step never takes a diagonal one. So the flood is the
stricter of the two, and a layout it passes that `Nav` fails is a bug in
the flood. `shipdesign::validate` is not in the loop — it was 85% of the
cost and is redundant with the flood; the tests ask it.

**How the drawn plans are built** (the generated one is above; both end
in the same `furnish`). `build_layout` is `match plan` to a floor
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
shelves, lamps, comforts (and a derelict's holes, until October 2026). The hub's floor reproduces the old
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
that every seed rolls the generated plan, that a station fallen back is
its drawn plan at its size, that no two drawn plans share (size,
corridor, residents), and the spawn rule.

**Tests**: `tests_layoutgen.rs` — the determinism (twice, and after
`forget_built`), `generated_stations_keep_their_invariants` (fifty seeds a
kind, a thousand under `BIMS_SWEEP`, printing the spread of the size, the
hull, the rooms, the corridor deck, the loops and the airlocks and failing
above two in a hundred fallen back), `twenty_docks_of_a_kind_are_twenty_
buildings` (no two share hull, walls and doors),
`the_default_galaxy_s_stations_are_generated`; the walkability contract in
`tests.rs` walks the generated plan on every kind (eight seeds each under
`BIMS_SWEEP`). `print_candidate` (ignored) prints a candidate and where
the flood stopped, `why_candidates_fail` why attempts are thrown away.

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

## Research was the world's state, and went with the relics (feature 106)

`World::research` (a `shipdesign::research::Research`), the station keys
(`World::station_keys`, `SystemMemory::station_keys`, `Station::key`,
`station::key_tier`/`key_rolled`/`KEY_CHANCE`), the five commands
(`TakeKey`, `Unlock`, `Research`, `CancelResearch`, `Dequeue`), their
events (44–47, 65–66), their refusals (22–25, 39, 40), `run_research`
and every gate research held — `craft_orders`' recipe check,
`can_place_site`'s `SiteRefusal::NotResearched`, and the workbench's
`Node::Upgrades` (`can_upgrade`, `upgrade_pair`, `bench_wants`) — were
**deleted** in feature 106. `git show 90e81c0^:crates/world/CLAUDE.md`
has the section as it was. `shipdesign::research` itself and the
designer's palette are untouched; the key resources are still
`physics::ResourceId`s, and nothing makes or finds one.

What stayed: every station's **research desk**, a solid on every layout
and a container class (`Storage::Research`), `World::station_desk`, and
the desk's spot and reach (`research_desk_spot`, `research_desk_in_reach`)
— at a site the machines hold, the desk is where a **relic cache** lies
("Relics, and research gone" at the end). The gold ring of lights that
showed a key on a desk (`world_paint::cache_lights`) shows the cache now.

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

> **Deleted with the electricity (October 2026).** `World::{run_power,
> run_brownout, sync_lamp_power, power, powered}`, `Power`, the power budget,
> `Ship::charge` (and its line in `world_checksum`), `throttle_reactors_for_probe`,
> `WorldEvent::{Brownout, PowerRestored}` (56, 57, left free) and the room's
> `Lamp::powered` / `set_lamp_powered` are gone: a lamp is dark only when it
> is shot out. A station's reactor room keeps its name and lost its reactor
> and batteries; the batteries' roll is still drawn (`let _ = rng.below(3)`)
> so every station is the building it was. `SAVE_VERSION` 81,
> `wire::PROTOCOL` 84. What follows is the history.

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
and `visited`, since feature 83 the machines' `infested`, and since
feature 111 a town's `defenses` and `held_towns`. The
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

**The plan is a town** (feature 54; its streets drawn from its seed since
feature 112, "A town's streets are drawn from its seed" below, and what
this paragraph describes the fixed **template** it falls back on,
`surface::template_floor(side, biome, population, seed)`): the whole build area bar its rim is ground — `Floor::open`,
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

## A town's streets are drawn from its seed (feature 112)

`surface::build_town(seed, biome, population)` draws a town with
`surface::towngen` — up to `towngen::ATTEMPTS` (16) drawings off
`seed ^ TOWNGEN_SALT`, each furnished and checked, the template when all
fail (none has, over 540 drawings) — and hands back the placer, the gates
and which attempt it was; `station::town` is its design and gates,
`Surface::station` builds the town's `Station` from them with
`Station::gates` set.

**Fixed**, since the rest of the world is measured from it: the side, the
fort's wall, the pad in the middle of the west wall, the yard, the watch
house with the dish and `GUARD_POST` south of the pad, and the trading
house north of it — the trading hall, the research room opening onto the
first cross street and the store onto the main street — exactly where the
template has them. **Drawn**: the main street's width (six to eight), east
from the pad to the east wall; two or three cross streets, the first
beside the trading house and out to the north wall, each running to a wall
or to the side street it meets and across the main street; a side street
north of the main street and one south of it, wall to wall, or not; the
hall's lot and its place in it, on the main street's north side, still a
chair each; and two or three **gates**, on as many walls as there are
candidates, where a street meets the north, south or east wall. Every
street ends at a wall or at another street. Then along the east–west
streets' frontages — edges, nearest the middle first — the food, far from
the pad first (four-strip field blocks, or greenhouses on an arctic
world), then the quarters, the heads, the other bathhouses and the houses
(`Town::pick_house`, the same rule) until there is a bunk each and two
over. A building goes where it overlaps no street, no other building, no
reserved ground (the yard, the trading house, the watch house and the
post, a gate's apron) and comes no nearer anything than two tiles
(`Taken`). Standing lights down every street and beside every gate; the
streets, the yard, the field blocks and every gate's apron are `clear`,
so the wild keeps off them.

**A gate** (`surface::Gate`: which wall, from which tile, how wide) is an
opening in the wall a street's width, a pier `GATE_PIER` deep inside either
side and a standing light beyond each pier (`Gate::piers`, `lights`,
`apron`). Gates are listed north, east, south; a wave takes them in turn
(`gate_for_wave`), its lander beyond the one it came by (`Gate::beyond`),
the wave a tile inside it (`Gate::spot`), and the guard's round stops a
couple of tiles inside each (`Gate::round_point`).

**The checks.** `towngen::check_trial`, on the town furnished without its
wild: every doorway and every gate's opening reached by a body two tiles
wide from the pad, every use spot and the post from there
(`stationgen::reach`), a chair and a bed each and two beds over, the food.
`check_built`, with the wild: every free tile reached four ways from the
pad, every gate's opening, and nothing wild on or beside a use spot, a
standing light, the post or a doorway's approach — the wild's own
keep-outs, which its pocket fill could otherwise break. The town test in
`tests_surface.rs` walks every biome at five, seventeen and thirty on two
seeds (five under `BIMS_SWEEP`) through the room's `Nav`;
`tests_layoutgen::a_generated_town_has_its_gates_its_hall_and_its_beds`
counts the fallbacks and asks that each gate opens onto open ground on the
plain. `print_town` and `why_towns_fail` (ignored) look at one.

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
box is foreign (`Game::set_foreign_outside`) — which is the world's word
for whose it is and no longer the fog's: the ground beside the ship is
under the one fog like the town and the ship wherever nobody sees it
(task 128).

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
(not saved), and nothing is remembered (task 128); `veil_at_room` is
what the painter draws — nothing, or the one fog.

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

> **Changed by task 120** ("One bar of hit points" at the end of this
> file): the beam puts back hit points (`class::HEAL_BEAM_HP` an hour), a medic may beam itself, the surge charges while a patient is short of its bar, and the medic's kit, `Doctoring`, `Beamed`, the field surgery and the closing surge are gone. The rest of this section is the history.

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

The app's half: `keys::Action::{Attack (X), Retreat (Y)}` — F and T until the
attack-move took F (`Action::AttackMove`, the player's own Bim), which is
what moved the camera's Follow onto V — `screens::game::orders_key` and
`screens::game::attack_key` (which is what makes X over a banner a
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
(`droid_tier`), the reinforcement clock (`droid_reinforce`) and the
forced wave size (`droid_wave_forced`, nought unforced — it was the wave
cap, `droid_wave_max`, until task 132), all three of which the probes
move. It is the
size of the fight.

- **The wave count is fixed at the crew's first dock and never worked
  out again** (`Infestation::settle`, called from `droid_waves` the first
  step the rooms are joined): **the tier's** — one wave at tier one, two
  at tier two, four at tier three (`data::DROID_TIER_WAVES`,
  `droid::wave_count(tier)`, the tier `World::droid_tier` answers; the
  scaling's `tier_waves` dial over it) — or whatever
  `World::set_droid_waves_for_probe` says — the `droids` commands set
  **three** (`screens::game::DROID_WAVES_IN_PROBE`, `BIMS_DROID_WAVES=n`
  over it), since a tier-one site's one wave is one landing and then a
  cleared station, and what those commands are for is the wave after the
  first. (Until the player asked for a count by tier it was
  `DROID_WAVES_BASE` + a wave every second step of the world clock,
  feature 105.) That dial is neither saved nor hashed, unlike the
  other three: it is read once and what it decides —
  `Infestation::waves_left` — is both. Wave one is aboard then,
  stood about the station's rooms (`droid::spots_about`, free deck tiles
  spread across the design).
- **The wave size is worked out as each wave appears**:
  `DROID_WAVE_BASE` + the **player** Bims (`World::players`) + a step
  every `ENEMIES_HOURS` of the world clock (`droid::wave_size`), with
  **no cap** since task 132. **Nothing the crew own, learn or hire is read**
  (feature 105): not the bots, the mercenaries or the townsfolk who
  joined, not the worth and not the levels — it was all of those until
  then, and growing stronger made the machines stronger. Nothing in a
  mission moves the world clock, so every wave of one fight is the same
  size. **Integers only, and nothing doubles.** The cap it had,
  `DROID_WAVE_MAX` (sixteen, a performance limit), went in task 132, to
  be balanced another way.
- **The run's difficulty** (`droid::Difficulty`, `World::set_difficulty`):
  the game setup's base, per player, scaling (per step), early ease and
  early days, laid over the tuning file's `WaveScaling` by
  `World::scaling`, which every wave size and count reads;
  `wave_scaling()` stays the file's own, so the app's live reload
  compares against that. `None` is the file's five. **Saved, not
  hashed** (SAVE 67, PROTOCOL 67 — `SettingsWire` carries it to the
  guests at Start; the early ease's two at SAVE 82, PROTOCOL 85): what
  it decides, the machines laid, is hashed.
- **The mix is `bims::droid::mix_of`**: Wardens `n / 6`, Husks `n / 3`,
  Troopers the rest, and `wave_kinds` orders them Wardens, Husks,
  Troopers so a Trooper's arm is dealt by its place *among the Troopers*
  — pistol, rifle, pistol, rifle.
- **No reinforcement while a machine lives.** `droid_waves`, a stage of
  the step right after `settle_residents`: with any of them standing the
  clock is held at `None`; with none standing and waves left it is set
  `DROID_REINFORCE_STEPS` (fifteen seconds) of the **mission clock** on — a
  minute in the probes — and the wave lands when it runs out (feature
  103; it was the world clock's minutes before). The clock is the
  world's and not the room's, so a room built afresh resets nothing.
- **Where a wave arrives.** At a station, `droid::arrival_airlock_at` —
  **every airlock but the port in turn** (task 131): wave one at the one
  farthest from the port (`droid::arrival_airlock`; the first airlock is
  where the crew dock, and ties go to the lower index), wave two at the
  next farthest, and round again; a station with only the port has them
  come in by it, and the Machine Heart keeps its own turn by index. It was
  always the farthest until task 131, which is also what put airlocks in
  rooms' outer walls. The machines are posted
  `data::ASHORE_TILES` inside it, spread round the spot in rings so a
  wave does not land on one tile, the way `Residents::post_boarders`
  posted a raider's boarders until feature 104. On a surface, just inside the **gate** its lander set
  down beyond: the town's gates in turn, the first for wave one
  (`surface::gate_for_wave`, `Gate::spot`) — on the template's two, north
  for an odd wave and south for an even one, as before feature 112. The lander itself is drawn on the plain, which is
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

> **Changed by task 120** ("One bar of hit points" at the end of this
> file): there are no dressings. This section is the history.

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

> **Changed by task 120** ("One bar of hit points" at the end of this
> file): the medkit and the bandage are no charges any more — `class::Charge` is the sandbag, the sentry and the grenade — and nobody carries medicine. This section is the history.

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
  medicine. **`charge_cooldown`** is `MEDKIT_COOLDOWN` (40 s of the
  clock) — `MEDIC_MEDKIT_COOLDOWN` (30 s) for a medic of either kind —
  and `BANDAGE_COOLDOWN` (30 s), the same for a medic. One medkit
  treats **every** trauma on the body (`Game::apply_treatments`), so a
  body dying of two is one kit, not two.
  `a_medic_s_medkit_comes_back_quicker_and_a_bandage_no_quicker` pins
  the three.
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
  pack out, so spending a bandage does not thin the hands for hire
  scaled on it (the enemies were, until feature 105).
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

**Threatened is derived** — and since task 111 it is **every defence
site's, from the first day**, not the front's (the next section has the
whole rule; `World::site_threatened` replaced `town_threatened`). Until
then it was a friendly town on a planet's surface in a system the
infection is **one hop** from (`World::front(star_id) == Some(1)`), not
already the machines', and not one the crew had already held. Nothing is
saved for it either way. The system map tags every site with its kind
now, `DEFEND` in amber, and a site the crew held `DEFEND · held`.

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
  held station has one: the first lands `data::DEFENSE_DELAY_STEPS`
  after the crew set down — twenty seconds of the mission clock since
  task 111, time to get the crew where they mean to hold (an hour of it
  until then, time to walk the town, trade and hire) — and the rest `DROID_REINFORCE_STEPS` after the
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

**`defenses` and `held_towns` are the system's own**, like `infested`
(feature 111's first fix): a town's id is `SURFACE_BASE | body`, and body
ids are a system's own, so while the two lists were global a town held in
one system held the next system's town of the same id — never
threatened, never taken. They go onto `SystemMemory` in
`remember_system`, come back in `recall_system`, are cleared by a jump to
a system never visited, are kept by `SystemMemory::overrun`, and are
hashed in the checksum's memory block **only where a memory has any**
(so no number moved). A trip's quote for another system reads that
system's memory for *held* and *cleared*. `SAVE_VERSION` 40,
`wire::PROTOCOL` 32 (the relay wants redeploying);
`a_town_held_in_one_system_is_not_held_in_the_next` is the test.

**And some of its people go with the crew.** `defense::joiners` is
`DEFENSE_JOINERS` — **two**, however big the town (it was a fifth of the
survivors, `DEFENSE_JOIN_PERCENT`, until October 2026) — never more than
the survivors other than the guard, so a town with nobody left, or only
the guard, sends none. Who: the lowest indices,
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
orbital stations (task 111 made every station and derelict a defence);
droids raiding the ship; a second ship following the first.

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
rolled, a mercenary a civilian), the town's gates added to the guard's
ways in (`Station::gates`, each `Gate::round_point`). A body taken
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
minutes for the clock and **never under `data::MIN_TRAVEL_HOURS`** (a
day; `TravelQuote::minimum` says when the floor is what it is — feature
105, since the machines scale on the world clock alone and a trip that
left it where it was would be a fresh fight at the same strength); the
site the crew are at is `Refusal::AlreadyHere` (92), first of all; one
lane at most (`TooFar`), `jammed_step` still
`Jammed`, and the state on arrival read off the arrival day: infested, the
tier (the distance rule unless the probe's override), the jammer (lowest
orbital, else derived), whether a defence starts on arrival and the
site's kind (task 111, `TravelQuote::{threatened, kind}` — a town on the
front worked out at that day, until then). No hyperdrive part is asked for: the default ship has none, and a
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
`Defense`'s `won`, and otherwise `!site_threatened(id)` (task 111; it was
`!town_threatened`) — a trader, or a site under the tests' quiet dial,
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
press by one already returning clears a `Declined` departure. **Every
press walks the player's own Bim home as well** (`walk_the_player_home`:
`Game::walk_to` the `gangway`, no post, so the player's next order takes
over), unless it is already `inside_ship` or down; the bots' `Retreat`
never reaches a Bim a player steers.
`settle_departure` waits for every **waited-for** player (`waited_for`:
connected, not out, alive, not `is_down`) to be returning and
`inside_ship` (`Aboard::on_ship` against the ship's own design), and at
least one to exist; then `left_behind()` (every living crew member outside
the ship, bar those `comes_home`) empty is `leave_mission`, and otherwise `Departure::Asking`. An
`Asking` whose list changed is asked afresh; one declined is not asked
about the same list again. **After a fight won** — `Run::fought` and
`mission_cleared()` — `comes_home(who)` is every crew member alive with
`bleeding() == 0` and not `is_dying`, down or not: never listed, and
`leave_mission` first stands them at the `gangway` (`bring_home`,
`Game::stand_at`), so they unjoin aboard. The bleeding and the dying are
left behind as before. `leave_mission`: the stable brought home, the left behind `kill_now`,
`casualties` (the deaths paid for), the bounty settled or dropped,
`unjoin_rooms` and `close_residents`, then a town under attack `infest`ed
(`TownFell`) or the snapshot restored (`restore_site`), the fallen
stripped, the dead bots dropped (`bury_the_bots` → `drop_crew_member`,
which removes a crew index from every list the world keeps one in), the
ship `Holding` at the site's place, `undocked_once`, and `Phase::Map`.

**Dying.** `casualties` no longer resets a dead crew member's `Progress`
(it is kept for the buyback) and calls `fall`: a player onto `run.fallen`
in death order, a bot `BotLost` and — since the bot-deaths task, which
deleted `BOT_DEATH_PENALTY` and `BotLost::paid` — nothing off the pool:
only a player's Bim costs money (its buyback). `check_run_lost`: every player slot's Bim dead — out cold is alive,
and the bots do not count — or nobody of the whole crew standing
(every player and bot down or dead: nobody left to revive, so the
countdowns are not waited out; `the_run_is_lost_at_once_when_every_player_and_bot_is_down`)
**and no defender of the site on its feet either** (`World::defender_standing`:
a defender alive and not downed in the residents' room, the rooms joined —
while one stands the machines are still held off;
`the_run_is_not_lost_at_once_while_a_defender_stands`, `tests_defense.rs`).

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
  `ARENA_GARRISON`; `ENEMIES_DAYS` stayed for the waves, until feature
  105 made it `ENEMIES_HOURS`), `BOARDERS_MAX`
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

- **The wave.** *Since October 2026 no plain wave has a Guardian*
  (`wave_kinds(n)`, the player's word: Guardians spawn only at elite
  sites, `elite::with_guardian`; and a Guardian near a Bim drops a
  grenade, `crates/game/CLAUDE.md`). What follows is the history:
  `build_wave` asked `bims::droid::wave_kinds(n, tier)` at
  the world's tier, so a Guardian stood in a wave only at tier three —
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

## Relics, and research gone (feature 106)

> **Since October 2026 the relics are rebuilt** — "The relics rebuilt" at
> the end of this file: twelve relics, the crew's, each a boon with a
> price, offered only off an elite's clear and voted on; no cache, no
> trader's relic, no dice, no hooks. What this section, "Relics drop by
> the day", "Five patches of relics", "The relic rebalance" and "A relic
> for every player off an elite" say is the history.
>
> **Since task 117** a relic is drawn by the world clock's day and not the
> site's tier, and an offer takes nothing out of the pool — "Relics drop
> by the day" at the end of this file. What this section says about
> `offer`, the pool and `RelicChoice::tier` is the history.

`crate::relic` is the types and the rules that are not the world's to
walk: `Relic` (twelve, codes in list order), `RELICS` (a row a relic —
tier, whether a new profile has it, and its `Effect`), `offer` (a draw a
slot, the site's tier while it has any left, then the next lower, never
upward), `stat_percent`, `hooks`, `overcharge`, the site rolls
(`cache_rolled`, `tier_two_rolled`, both off `site_roll`: the galaxy's
seed, the star, the station and a salt — no stream a fight draws from),
`Relics` (the run's state), `RelicChoice`/`RelicProposal`/`Source`, and
`Profile` with `record_run`. `relics.rs` is the world's side, a child of
`world` as `mission.rs` is.

**The state is `Run::relics`**, saved and hashed whole at the end of the
run's block: the **pool** (what is still to be offered — a relic leaves
it the moment it is offered, taken or not, which is what "once a run"
means), what each **player slot** holds, what is **pending** off a cache,
the **choice** on the table, what has **fired** this mission, who is
waiting to **get up**, and the count of offers (a draw's seed). Beside it
on `Run`: `fought` (the site had something to clear when the mission met
it — set in `open_the_mission`), `cleared_here` (the clear said, once),
`won` and the probes' `win_on_clear`. A player's Bim is its slot's crew
index, so `relics_of(who)` is nothing for any `who >= players()`: a bot
never holds one, and `ProposeRelic` to a bot is `Refusal::NotAPlayer`.

**What a relic does is read where the thing it moves is worked out**:

- `World::skill_of` ends in `lift_by_relics` — `Skill::damage` and
  `melee` (*Focusing Lens*, *Last Stand* while another player's Bim is
  down), `fire_rate` (*Steady Grip*; it was `accuracy`), `walk` (*Servo Braces*),
  `armour_protection` (*Field Plating*), `healing` (*Trauma Kit*, which
  the room multiplies into a medkit's `treated_to` for the patient) and
  `overcharge` (*Overcharge Cell*, `Skill::for_shot` off `Bim::shots`,
  which the room counts at both of a crew member's trigger pulls).
- `charge_cooldown` (the class's charges, never `Charge::everybody`),
  `taunt_cooldown` (new; `taunt_cooldown_left` reads it) and
  `rally_cooldown` multiply by `Stat::Cooldowns` (*Coolant Loop*).
- The beam's `Beamed` is raised per patient in
  `hand_the_room_the_medics` (*Trauma Kit*).
- `visit` keeps each machine destroyed with the residents' `last_hit_by`
  and hands the list to `machine_kills`: *Salvage Beacon*'s share on the
  holder's own kills' bounty (pending like any), and `Trigger::Kill` —
  *Kill Relay* moves the start of every class cooldown running back by
  its seconds (`cooldowns_less`: the charge timers, a tank's
  `last_taunt`, a commander's `last_rally`).
- `settle_relics` is a stage right after `casualties`, before the tanks
  take their hits' experience out of the count: a player's Bim **down**
  (alive and out cold) is `Trigger::Downed` — *Second Wind* starts its
  wait (`down_since`) and `get_up_if_due` brings it round
  (`Game::bring_round`, `Health::brought_round`: parts at the share at
  least, traumas over, wounds shut, blood to `SLOWED_AT`) once the wait
  is up, marking it fired; a **hit taken** is `Game::hits_taken` gone up
  since `hits_before_the_step` read it before the rooms stepped, and is
  `Trigger::HitTaken` — *Phase Harness*, under its share of health
  (`health_share`, the parts over `MAX_HEALTH`), is the room's own surge
  for its seconds (`Game::set_surge`, halo and all).
- `Trigger::MissionStart` is said in `begin_mission`
  (`relics_at_mission_start`, which also clears `fired` and
  `down_since`), and `Trigger::AbilityUse` at the end of `apply` for a
  class key that went through unrefused (`Deploy`, `Throw`, `Surge`,
  `Taunt`, `Rally`, `Squad`). No relic of the twelve uses either yet;
  they are the hooks a later one is written against.

**Where relics come from.** `leave_mission`, while the ship is still
tied up, calls `relics_on_leaving`: a cache's choice not made is dropped;
the site left **uncleared** loses what is pending (`RelicLost`) — the
snapshot then puts the site back with its cache; the site left
**cleared** keeps what is pending (`settle_clear`) and, when `fought`,
draws `RELIC_OFFER` relics at `droid_tier()` (`offer_reward`) and the
phase is `Phase::Reward` rather than `Map`. The **cache** is
`Infestation::cache`, rolled in `infest` (`cache_rolled`,
`RELIC_CACHE_CHANCE`), hashed, and in the snapshot with the rest of the
infestation; `OpenCache` wants the cache here, no choice already made
and the Bim within reach of the station's research desk, and draws one
relic. A carried cache relic is **given** at once on a site already
cleared and **pending** otherwise; `settle_clear`, in `settle_run` the
step the site is cleared, gives what is pending and — with
`win_on_clear` — calls `run_won`.

**The vote** is the map's: `ProposeRelic { relic (u32::MAX none), to }`
replaces the proposal and every yes but the proposer's; `AcceptRelic`;
`relic_if_carried` on the last connected yes (`PlayerGone` needs no hook
— the next yes finds the gone player not waited for). Both apply at once
(`applies_at_once`) and are heard in `Phase::Map` and `Phase::Reward`,
where the step does nothing else.

**The pool** opens as a new profile's (`Run::new`) and is replaced once
by `set_relic_pool` — the host's, from `Session::set_relic_pool` in the
app's `build_run` — before the world's first step. `give_relic_for_probe`
(`BIMS_RELICS`) takes the relic out of the pool too.

**Tier two** is `site_tier` (the root `CLAUDE.md` has the rule), read by
`droid_tier` for the site alongside and by the quote at the arrival
minute. It is in `world_checksum` through `droid_tier()` as before.

`tests_relic.rs` is the feature's tests; `relic::tests` the rules'
own; `tests::a_workbench_upgrade_wants_no_research` the bench's.

## The Machine Heart (feature 108)

The end boss, and the one way to win a run. `crate::heart` is the types
and the rules; `fortress.rs` (a child of `world`, like `mission.rs` and
`relics.rs`) is where they meet the world's fields.

- **The fortress is a derived station at the origin**: `heart::blueprint`
  rolls it off the origin star's own stream (the jammer's `Purpose`, a
  branch of its own, so nothing about the jammer moved), its id
  `heart::heart_id(star)` = `HEART_BASE | star` (`0x2000_0000`, clear of
  the generator's, `JAMMER_BASE` and `SURFACE_BASE`), its point a
  `jump::clear_point` past every ring a derived jammer is rolled on and
  worked out off the system **without** any derived station in it — so the
  map's quote from next door and the station laid in the system agree.
  `World::settle_heart` strips and lays it; `settle_jammer` takes the
  fortress out first, settles the derived jammer (the fortress is not a
  station of the system's own for that rule, and never the jammer —
  `jammer_station` and the quote's "lowest orbital" skip it), then calls
  it. Never saved: what is saved is its `Infestation`.
- **Its layout is `Plan::Fortress`** (`station::fortress`): the hub
  floor at `data::ARENA_SIDE`, the big plant's tile put on a partition so
  the comforts' rule leaves it out (the plant is opaque, and the core
  stands in the middle), and two **standing lights** inside the hub on the
  diagonal the fabricators are not on — the hub's own four lamps hang in
  its corners a little over seven tiles from its middle, and the middle
  tile is out of reach of all four (on a hub the plant stands there, so
  nobody had noticed). `station::fortress_rooms` hands `heart::places` the
  hub and the conduits' rooms in the order they are filled: the four outer
  rooms (one a corner), the three lobbies the waves come in by, the four
  inner rooms. `Station::build` builds a heart id as a fortress whatever
  the seed rolls.
- **Always tier three**: `World::droid_tier` answers `Three` at a fortress
  before the probes' dial.
- **The fight is on the `Infestation`**: `Infestation::heart:
  Option<HeartFight>` (phase, how many conduits and how much core health it
  was built with, whether it is laid, the next build and how many were
  built), set beside the waves' count at the first dock
  (`settle_heart_fight`, off `World::players`), hashed by `eat_heart` only
  where it is `Some`. The machines are laid by `settle_droids` ahead of the
  wave (`heart_machines_to_lay`), so they keep the front of the droid list;
  a wave arriving clears the rest (`Game::clear_wave_droids`) and cuts the
  residents' per-body lists back to the Bims **and** the Heart's machines.
  `droids_standing` never counts them, so a conduit standing never holds a
  wave off, and a fortress is cleared by its core alone (`droid_waves`
  leaves `cleared` to `heart_step`).
- **`World::heart_step`**, after the rooms step and before the loss is
  checked: the last conduit down is `HeartExposed` and the first build
  due an interval on; under `HEART_OVERLOAD_FRACTION` of its health
  `HeartOverload`, the next build on the faster clock; builds at every
  fabricator still standing on their interval (`fabricate`: a Trooper, a
  Husk, a Trooper, a Warden and round, by the count built, out of the bay
  it faces with, the wave aboard); the core at nothing `HeartDestroyed`,
  `DroidStationCleared`, the station cleared and **`run_won`**. Then
  `tell_the_heart` puts the phase on the core's and the fabricators'
  `HeartState` for the room's next step. `check_run_lost` does nothing once
  the run is won.
- **No waves; every conduit shot down brings its Guardians** (October
  2026, the player's word: "first 1 guardian, then 2 all the way up to 5
  when the last link is destroyed"; before it a conduit brought a wave of
  `droid_wave_size` and four waves came by the clock). The fortress's
  wave count is one whatever the dial (`droid_wave_count`), its first
  wave lays nothing but the Heart's machines (`settle_droids`) and
  `droid_waves` brings none by the clock at a site with a `heart` — an
  older save's `waves_left` too; `data::HEART_WAVES` and
  `World::heart_wave_count` went. (`heart_step`, `conduit_guardians`):
  the k-th conduit a wreck past `HeartFight::links_down` (saved, hashed,
  serde default) is `heart::guardians_for_link(k)` Guardians
  (`data::HEART_GUARDIANS_PER_LINK` × k) at tier three in by the next
  airlock in turn (`arrival_spots`), looking for the crew like a reinforcement —
  **added** to the deck, never clearing it, whatever still stands —
  counted on `Infestation::wave` (the waves still to come by the clock,
  `waves_left`, untouched) and said as `DroidReinforcements`. A conduit
  laid a wreck (a room built afresh past the seal) and the probe's
  `set_heart_phase_for_probe` count as answered.
  `every_conduit_shot_down_brings_its_guardians`. **`SAVE_VERSION` 70,
  `wire::PROTOCOL` 72; the Guardians `wire::PROTOCOL` 132.**
- **Waves come in by every airlock but the crew's in turn**
  (`droid::arrival_airlock_at`), and the lander is drawn at the one they
  used.
- **Leaving** puts it back through the mission's `SiteSnapshot`, which
  photographs the `Infestation` before the first dock settles anything.
- **The map**: a heart site is listed at the origin (`sites_in`), placed by
  `heart::blueprint` (`site_position`), and its quote carries
  `TravelQuote::heart` — `World::heart_preview`: the conduits, the core
  and `HeartPreview::guardians` (`heart::guardians_for`, every conduit's
  lot), none of it the clock's.
  `World::origin_seen` (a visited star within a hop of the origin, off the
  kept hop table) is what the chart's diamond waits on.
- **The run's summary**: `Run::{machines_destroyed, sites_cleared,
  systems_liberated}` (counted in `visit` and `settle_clear`; a system is
  liberated when the site cleared is its jammer), in the checksum, read by
  `World::run_summary`.
- **The probes**: `heart_dock_for_probe` (the origin put at the crew's
  star, everything held, the ship re-docked at the fortress as
  `arena_dock_for_probe` does), `set_heart_phase_for_probe` (every conduit
  down, and for the overload the core's health just under the fraction)
  and `residents_point_on_deck_for_probe`.

`tests_heart.rs` is the rule: one fortress a galaxy at the origin, the
same for the same seed, tier three, the conduits a player count asks for,
none sharing a room; the seal; one beam and the builds on their interval
and a fabricator down building nothing; two beams at two targets, faster,
and faster builds; the win once with a player dead and kept; the fortress
put back whole; the preview matching the fight on arrival; and two worlds
alike step for step through it. `heart.rs`'s own tests pin the id and the
placement. **`SAVE_VERSION` 38, `wire::PROTOCOL` 30**, `REFERENCE_CHECKSUM`
re-pinned for the run's counters; `SURVIVORS` unmoved.

## The Manufacturers (feature 109)

A human faction the crew fight from the first day of a run: the people who
built the machines. `crate::manufacturer` is the rules — which sites, who
stands in a garrison, what each carries — and `garrison.rs` (a child of
`world`, like `mission.rs` and `fortress.rs`) is where they meet the
world's fields. The whole schedule is `data::MANUFACTURER_*`.

- **Which sites is stateless, like the crisis.** `manufacturer::holds`: an
  orbital station (`eligible` — never a settlement, a derived jammer or the
  fortress) outside the home system, either `rolled` off the galaxy's seed
  (`MANUFACTURER_SITE_CHANCE`, ten in a hundred) or one of
  `near_sites(galaxy, home)` — what the start makes up so there are at
  least `MANUFACTURER_NEAR_SITES` (2) within `MANUFACTURER_NEAR_HOPS` (2)
  lanes of home. `World::manufacturer_near` is that list, **derived** at the
  start and in `settle_crisis` (every load) like `home_hops`, never saved.
  `World::is_manufacturer_station(id)` asks it of this system,
  `is_manufacturer_site(star, blueprint)` of any.
- **A site of theirs is held the way the machines hold one**: an
  `Infestation` flagged `manufacturers` (serde default, hashed by
  `eat_manufacturers` only where set, so no machines' site's hash moved).
  `settle_manufacturers` lays one for every site of theirs in the system,
  at the end of `settle_jammer` — the start, a jump (after the memory is
  recalled), the spread and every load — never over one already there, so
  a cleared site stays cleared. So `is_droid_held` is **"held by an
  enemy"**: the stance is Hostile, `people_of` and `mercenaries_of` are
  nought, no desk quotes, and the waves' clock, the clear, the pending
  bounty, the relic reward and cache and the `SiteSnapshot` put-back all
  work unchanged. `is_manufacturer_held(id)` is the flag.
- **Not infested.** `infest` refuses a site of theirs (the spread passes it
  by); `jammer_station`, `settle_derived_jammer`, the quote's jammer and
  `sites_in`'s derived jammer skip them — a system whose only orbitals are
  theirs gets the machines' derived jammer. `TravelQuote::manufacturers`
  says so, `infested` and `jammer` false, and `tier` the tier of what they
  will carry on arrival.
- **Who stands there** — `lay_manufacturers`, the branch `settle_droids`
  takes at a site of theirs. Wave one is the **garrison**: `droid_wave_size`
  bodies about the station's rooms (`spots_about`), each rolled a Trooper
  at `trooper_percent(day)` while `has_droids(day)` (before
  `MANUFACTURER_DROIDS_LOST_DAY`, ten) and one of their people otherwise;
  a later wave is their people alone at `arrival_spots` (split out of
  `arriving_wave`). **Every arriving wave's spots go through the room's
  `Game::spread_wave`** (`Nav::spread_from`): the ring round the spot
  inside the airlock or the gate is kept only where it is free deck in
  the same walkable patch as that spot, and the rest filled outward from
  it by walking, a tile apart — a ring laid round a generated station's
  corridor was half in its walls, and a machine snapped out of one could
  land on the far side, shut in, and never move. The seed is the galaxy, the star, the station, the wave
  and the **world clock** (`garrison_seed`), so a visit's garrison is fixed
  and the next visit's is rolled afresh. Their people are enlisted
  (`Game::enlist_manufacturer`) **before** the Troopers, since a body index
  past the Bims is a machine's; reinforcements are their people alone, so
  no Bim is ever added while machines stand. `Residents::manufacturers_laid`
  says which wave the room holds (a fresh room is nought; the room can hold
  graves as Bims, so "any Bims" would not do), and nothing is laid on a
  cleared site. The room's shelf of medkits goes to nought.
- **What they carry** is `manufacturer::gear(day, droid_tier, seed, ids)`
  → `Gear::manufacturer`: the pistol alone before day three, a tier-one
  gun (never the schword) from it, tier-one armour besides from day six,
  and gun and armour at `droid_tier()` from day ten.
  `World::manufacturer_tier` is that tier (one before day ten) — what a
  clear's relics are drawn at (`site_tier_code`); the bounty is
  `bounty_for(gear_tier)`, paid through `experience`'s Bim branch at the
  first down or death, pending until the clear.
- **The waves**: `wave_count_here` is one while they have the machines and
  `droid_wave_count` after; `reinforce_steps_here` is
  `MANUFACTURER_REINFORCE_STEPS` (fifteen seconds at 1×, the machines' own) unless a
  probe moved `droid_reinforce`. `droids_standing` adds
  `manufacturers_standing` — alive and **not out cold** — so one down and
  bleeding holds no wave and no clear up. `droid_ship` draws their ship at
  the airlock while their wave stands.
- **Nothing of theirs is taken**: `visit`'s click list is false for one
  (no Loot window), a loot of any resident is refused as ever, and
  `close_residents` counts no loss and lays no grave for one — they are not
  the station's people. The room keeps their gun with the body and nobody
  doctors, carries or binds for them (`crates/game/CLAUDE.md`).
- **Experience, for every enemy**: `XP_ENEMY_DOWN` (ten) once, at the first
  down or death — out cold, or dead without being down first (every
  machine); a death after a down is nothing. `XP_ENEMY_DEAD` went. That
  moved `SURVIVORS` (the constant's note says how it was checked).

`tests_manufacturer.rs` is the feature: sites near home and none at home,
the map's tag, day nought's pistols and its clear on the last down (out
cold, not dead) with the bounty and ten a head, a Manufacturer down
bleeding out with nothing taken and nothing more for its death, day
eight's Troopers, day ten's waves thirty seconds apart, the crisis passing a site
by and the jammer elsewhere, two worlds meeting the same garrison, a site
left uncleared met afresh with no graves, and the two sides shooting each
other. `manufacturer::tests` pins the schedule and the one-in-ten.
`World::manufacturer_dock_for_probe(day)` (the nearest site by
`nearest_manufacturer_site`, a trip a lane, the clock put at the day —
rewound, rooms and all, when the trips took longer) is the tests' and the
`manufacturers` command's. **`SAVE_VERSION` 39, `wire::PROTOCOL` 31** (the
relay wants redeploying); `REFERENCE_CHECKSUM` and the ship's `PINNED` and
`PICTURES` did not move.

**Not done**: the threat chip along the top still says *machines* at a
site of theirs (the spec kept the HUD to the nameplate); a relic that reads
kills (*Salvage Beacon*, *Kill Relay*) counts machines only, as before.

## Nothing is stored: the holdings and the loadouts (task 113)

> Every section above about the hold's gear, the pieces' `Where`, the
> guns' list, the grids, the pack, a fetch or a stow, a loot, the
> workbench's slots, upgrades and repairs, the drug lab's medkit and the
> engineer's *armourer* describes what **task 113 deleted**. They are
> kept as the history of how it worked; this section is what is there
> now.

`crate::holdings` is the one new module: `Holdings { armory, keys,
next_id, offers }` on `World::holdings` (saved, and in `world_checksum`
whole), `Stored { id, item }` a thing in the armory, `GearSlot` (weapon,
head, body, legs — codes 0–3, with `read` and `write` on a
`bims::combat::Gear`), `GearSource` (the armory by id, or a Bim's slot)
and `Offer { from, slot, to }`. Its tests pin the slot rule and the
armory's numbering.

- **The loadout is the room's `Gear`** and nothing of it is copied on
  the world: the checksum reads every crew member's gear off the room
  (weapon, pieces with their health to a hundredth, the charges).
  `World::{pieces, next_piece, guns, grids, bench, craft_targets,
  auto_upgrade}` are gone with `crate::grid`, `armour::{Where, Piece,
  FetchKind}` and the keys' pack items; `armour` is the resource table and
  `LootSource`, which still names a body for a hire and the commander.
- **`stock_the_armory`** at `World::start`: the design's gear cargo
  becomes things in the armory (tier one, whole) and its research keys
  the holdings' count, and those counts go to nought — so the ship's mass
  no longer carries gear. `set_out_empty` empties the armory and the keys
  too. `World::held(resource)` is what a sale takes from (the armory, for
  gear) and what the trade window shows.
- **The four commands** (`Equip`, `Unequip`, `Offer`, `AnswerOffer`) are
  allowed between missions and refused `GearLocked` (98) in one — bar an
  equip or unequip on a Bim alive and `inside_ship` (and, for a thing
  taken off another Bim, that one too), which is how a crew arriving at
  a site kits out from the armory aboard (`gear_refusal`,
  `may_change_now`; `a_crew_arriving_at_a_site_kits_out_from_the_armory_aboard`);
  `stock_every_thing_for_probe` fills the armory with every kind at every
  tier it is made at for the app's combat-ship runs;
  `may_change(slot, who)` is the player's own Bim or a bot
  (`NotYours`, 99); `NoSuchGear` (100) and `NoOffer` (101) are the rest.
  `set_slot` is the one door a slot changes by, and it withdraws every
  offer the slot was in (`OfferWithdrawn`). An accepted offer moves the
  thing and puts the recipient's old one in the armory (`OfferTaken`);
  `begin_mission` withdraws every offer standing. Events 120–125:
  `GearChanged`, `GearOffered`, `OfferTaken`, `OfferWithdrawn`,
  `KeyFound`, `Respawned`.
- **A mission's start** mends every piece (`mend_all_armour`, the armory's
  and every loadout's) and **sets** every charge to its start amount
  (`fill_charges` — it used to top up). There is no buyback at the start.
- **A mission's end** (`leave_mission`): dead bots' loadouts go into the
  armory (`store_loadout`, mended) before `drop_crew_member`, and every
  fallen player is revived with its gear (`Game::revive` keeps it now,
  mended), the pool paying `BUYBACK_COST` saturating at nought
  (`respawn_the_fallen`). A player left behind dies and comes back the
  same way; `Run::fallen` empties.
- **Charges are counts**: `charges_of` reads `Gear::units_of`,
  `restock_charges` adds one with `Game::give_stack`, a deploy and a
  throw spend one with `take_stack`, a pack-up gives one back — there is
  no pack to be full, so `PackFull` went. `set_charges_for_probe` sets
  the count outright.
- **Trading**: a gun or a piece bought goes into the armory at its tier
  (`new_piece` numbers a piece off the holdings); a sale gives up the
  lowest tiers first (`gear_leaving`). `NoRoomAboard` went: nothing is
  ever short of room. A resident joining — a hire, a townsperson — keeps
  what it brought as its loadout, its pieces renumbered off the holdings.
- **Keys**: `pick_up_key` is the one door a key comes in by, counted the
  moment it is (`KeyFound`). Nothing in a run lays one yet.
- **The engineer**: `pick_at(Engineer, 6)` is `None` and
  `fixed_at(Engineer, 6)` is *Higher quality armour*; `Talent::Armourer`
  keeps its code (a talent's code is its place in `Talent::ALL`) and is
  never offered. `class::ARMOUR_REPAIR_*`, `deploy::{ARMOUR_REPAIR_COST,
  REPAIR_ORDER}`, `UPGRADE_ORDER`, `data::UPGRADE_SESSION*` went.
- **Refusals gone**: 3, 15–18, 34–36, 51 and 81. **Events gone**: 14, 15,
  34, 35, 38, 48, 49, 74, 102 and 103.

`tests_holdings.rs` is the task: the commands refused in a mission and
taken on the map and the reward screen; every way one player could reach
another's rifle refused, an offer declined, withdrawn by a slot changing,
accepted and withdrawn by a mission; a helm worn to nothing staying worn
and whole at the next mission, an armory piece mended too; a dead
player back at the mission's end with its sniper and helm, the pool
charged and never below nought; a dead and a left-behind bot's kit in the
armory; a key counted at once; the holdings in the checksum; and the
design's gear in the armory and not the hold.

## The trader (task 114)

> Every section above about a desk, a buy, a sell, the cart or a
> station's gear trades describes what **task 114 deleted**. Gear is
> bought at a trader, on the map; nothing is sold.

`crate::trader` is the rules — which sites are traders, the shelf, the
relic's price, what two things combine into — and `trading.rs` the
world's side, a child of `world` like `mission.rs`.

- **Which sites** (`trader::eligible`, `rolled`, `pick`, `near_sites`,
  `holds`): **a system** has a trader where its star is rolled at
  `data::TRADER_SYSTEM_CHANCE` (10) off the galaxy's seed (its own salt),
  or is one of `TRADER_NEAR_SITES` made up within `TRADER_NEAR_HOPS` lanes
  of home, its own system counted; its trader is `trader::pick`, the
  lowest station blueprint with a desk (never a derelict), not home, not
  the Manufacturers', not derived (`World::trader_of`). **The trader is
  picked first and the jammer after it**: `World::jammer_site_among` —
  `trader::jammer_candidate`, the lowest station not derived, not the
  Manufacturers' and not the trader — is the one rule
  `jammer_station`, `settle_derived_jammer`, `sites_in` and the quote
  ask, so a system whose one station is its trader gets a derived jammer
  and a trader is never a site to clear. (Until the galaxy of worldgen's
  `GENERATOR_VERSION` 8 it was twenty in a hundred a *station*, and the
  jammer's station was never eligible; with a single station in two
  systems in five that left a trader in three systems in a hundred.)
  **A trader every five hops** (October 2026): `near_sites` also makes
  up `trader::cover`'s — greedily, the star that can take one (its
  `pick` is `Some`) and would serve the most stars short of one, ties by
  an order off the seed — until from every star, a trader's own
  included, **another** star's trader is within `data::TRADER_EVERY_HOPS`
  (5) lanes; a star hemmed in by systems that can have none is the only
  one left without. 4 to 11 are added a galaxy (18–28 traders of 240
  over the four galaxy types, two seeds each);
  `from_every_star_another_trader_is_at_most_five_hops_away` is the rule.
  Every system is generated for it at the start and every load (about
  1 ms). `wire::PROTOCOL` 124.
  `World::trader_stars(galaxy)` is every star with one, for the chart.
  `World::trader_near` is derived at the start and in `settle_crisis`,
  behind `manufacturer_near` which it reads, and never saved.
  `World::is_trader(site)`, `trader_in(galaxy, site)` for a list,
  `is_trader_here(id)`, `trader_sites()` for the map.
- **Closed**: `World::liberated(star)` is every non-Manufacturer
  `Infestation` of the system cleared, at least one — off `infested` for
  this system and the system's `SystemMemory` for any other, so a system
  never visited since it fell is not. `trader_closed_on(star, day)` is
  the crisis on that day and not liberated. `quote_in` refuses a trader
  `TraderClosed` (102) closed today and `ClosedOnArrival` (103) closed on
  the arrival day, and quotes `trader: true` otherwise (`infested` false:
  it is never theirs). `spread_crisis` skips a trader; so does
  `infest_here_for_probe`, bar the dock.
- **The visit**: `travel` calls `arrive_at_trader` for a trader quote and
  `arrive_at` + `begin_mission` for anything else. `arrive_at_trader`:
  the ship `Holding` off the station in its frame, `mark_visited`,
  `Phase::Trade`, `run.site` the station, and nothing a mission's start
  does. The first arrival meets the trader: `Trader::new` rolls the
  shelf (`trader::roll_shelf`, off the galaxy's seed and the site on a
  salt of its own — `TRADER_WEAPONS` weapons then `TRADER_ARMOUR` pieces,
  any kind of `worldgen::data::{WEAPONS, ARMOUR}` at any tier) and the
  relic is `draw_relics(site_tier, 1, station)` — out of the pool for
  good. Kept on `Run::traders`, sorted by site; a shelf slot bought is
  `None`, so a slot's index never moves. `Run::trade_relic` is the relic's
  vote while there; `travel` clears it.
- **The step** does nothing in `Phase::Trade` but the commands, as on the
  map. `propose` and `accept_proposal` take `Map | Trade`; `player_gone`
  carries both votes there.
- **`Command::BuyShelf { index, to }`** (`buy_shelf`): at a trader, the
  slot not sold (`SoldOut`, 105 — the first command has it), `to` a Bim
  the player `may_change` (`NotYours`) or `None` for the armory, the pool
  paying `shelf_price` (`quote_at`'s ask at the item's tier — the station's
  lean and the front premium included — else the book at the tier;
  `Unaffordable`). Onto a Bim through `set_slot`, the old thing into the
  armory. `WorldEvent::ShelfBought` (126).
- **The relic**: `ProposeRelic`/`AcceptRelic` in `Phase::Trade` are
  `propose_trader_relic`/`accept_trader_relic` — the trader's relic only
  (`NotOnOffer`), a player's Bim (`NotAPlayer`), `relic: None` withdraws.
  Carried, `trade_relic_if_carried` pays `trader::relic_price` (by the
  relic's own tier, `data::RELIC_PRICE`) or refuses the proposer
  `Unaffordable` and drops the vote; paid, the Bim holds it and the
  trader's relic is `None`. `WorldEvent::RelicBought` (128).
- **`Command::Combine { a, b }`** (`combine`): at a trader, two distinct
  sources, each the armory or a Bim the player `may_change`;
  `trader::combined` — `NotAPair` (107), `TopTier` (106); the fee
  `COMBINE_FEE` out of the pool; a piece made is numbered off the
  holdings. A worn input takes the result in its place (the first worn of
  the two), the other is taken away; two from the armory make one in it.
  `WorldEvent::Combined` (127).
- **The checksum** eats `Run::traders` and `trade_relic` only where there
  are any, which is why neither `REFERENCE_CHECKSUM` nor `SURVIVORS`
  moved. The ship's `PINNED` moved for `jammer` alone, by
  `infest_here_for_probe` passing a trader by.

`tests_trader.rs` is the task: arriving enters `Phase::Trade` with no
room and neither clock moving; the same seed the same shelf and relic; a
bought thing never back and a revisit the same trader; onto a Bim or the
armory, another player's refused, the pool short refused; two buyers of
one thing; the relic's vote, a bot refused, a new proposal clearing the
yes, the pool short, paid once; combining, tier three, not a pair,
another player's Bim, worn in place; closed and liberated; closed on
arrival against a site beside it; and the checksum.
`a_trader_in_one_system_in_ten_and_the_chart_marks_them` pins the share
(6–14 % of systems over two galaxies, never two in one, and
`trader_stars` agreeing).

## The minigun and the rail lance (task 115)

Two kinds made only from a tier up (`bims::combat::WeaponKind::min_tier`;
`crates/game/CLAUDE.md` has the weapons). The world's part is every place
a weapon is *made*:

- **The shelf** draws one thing a slot from `trader::shelf_candidates` —
  every kind of the list at every tier it is made at, so a tier-one
  minigun or a lance below three is never a candidate at all (not drawn
  and moved up). `worldgen::data::WEAPONS` is seven long.
- **Combining** is unchanged: two tier-two miniguns make a tier-three
  one; a lance is tier three already and is `TopTier`.
- **`outfit_for_probe`** puts a kind never made that low at its own
  lowest tier; `armour::weapon_at` answers `None` below it.
- The issue, hire and garrison rolls and every machine's arm never name
  either (`combat::tests`).

`tests_trader.rs` is unchanged; `trader::tests` pin the shelf over many
seeds and the two combines, and
`tests_droid::a_rail_lance_fights_the_same_fight_on_two_worlds` two
worlds alike to the checksum through a lance's fight.
`tests_survivors::arm` deals the five kinds the reading was taken with,
so `SURVIVORS` did not move; `REFERENCE_CHECKSUM` moved for the ship's
design hash alone (two more empty cargo slots).

## The arc greaves and the Reflective plate (task 116)

Two kinds of armour made only from a tier up (`bims::combat::ArmourKind::
min_tier`; `crates/game/CLAUDE.md` has what they do). The world's part is
every place a piece is *made*, and they are the weapons' of task 115:

- **The shelf** draws from `trader::shelf_candidates`, which now asks
  `ArmourKind::made_at` as well — never greaves at tier one or a plate
  below three. `worldgen::data::ARMOUR` is five long, so an armour slot is
  one of twelve candidates where it was one of nine: shelves are other
  shelves, which nothing pinned meets.
- **Combining**: two tier-two pairs of greaves make a tier-three pair; a
  plate is tier three already and is `TopTier`.
- **Nothing the world dresses a body in names either**: the tank's start,
  `outfit_for_probe` and the Outfitter's hire go by `ArmourKind::BASIC`,
  and `stock_the_armory` makes a design's piece at its kind's own lowest
  tier.
- `ResourceId::{ArcGreaves = 20, ReflectivePlate = 21}`, booked at 1 500
  and 2 500 (`economy::trade_price`, times `TIER_PRICE`).

`trader::tests` pin the shelf over many seeds and the combines,
`tests_droid::the_plate_and_the_greaves_fight_the_same_fight_on_two_worlds`
two worlds alike to the checksum with both worn, and
`no_outfit_or_start_wears_the_greaves_or_the_plate` the outfits.
`REFERENCE_CHECKSUM` moved for the design hash alone (two more empty cargo
slots); `SURVIVORS` did not.

## Every site is an attack, a defence or a trader (task 111)

Every site the map lists is **exactly one** of three
(`crate::run::SiteKind`, `code()` 0–2): **Attack** — an enemy holds it,
the machines (`is_droid_held`, cleared or not), the Manufacturers or the
Machine Heart, or a derived jammer; **Trader** — a trader site of task
114 (`is_trader_here`); **Defend** — every other site, derelicts
included. `World::site_kind(station)` is the rule for this system and
`TravelQuote::kind` the map's (off the arrival day, and for another system
off its `SystemMemory`). Derived, never saved.

- **Threatened from the first day.** `World::site_threatened(station)`
  replaced `town_threatened`: a Defend site with no `Defense` over (won or
  lost) and not a held town — no front rule, no surface rule. The quote's
  `threatened` is the same rule on the arrival day, reading held towns and
  finished defences from that system's memory for a jump; `cleared` covers
  a won defence. `site_cleared` falls back to `!site_threatened`.
- **A defence anywhere.** `defense_here` and `defense_waves` lost their
  surface guards: a station's or a derelict's defence is a town's, the
  waves in at `arrival_airlock_at` (every airlock but the crew's in turn,
  the farthest first, since task 131) through
  the station branch `arriving_wave` already had. The step it starts
  (`stand_the_crew_ashore`) every living crew member on its feet is put
  just inside the site's own airlock (`droid::inside_of(port,
  ASHORE_TILES + 1)`, the arrival's rings, `Aboard::from_station`), each
  snapped by the new **`Game::stand_at`** (`put_for_probe` is it now).
  `data::DEFENSE_DELAY_STEPS` is **1 200** (twenty seconds at 1×); the
  first wave's arrival puts everybody back to 1× as any wave's does.
  The gap after a wave is down is `data::DEFENSE_REINFORCE_STEPS`,
  **600** (ten seconds), not an attack's `DROID_REINFORCE_STEPS`
  (thirty): `defense_waves` uses it while `droid_reinforce` is at the
  game's own, so a probe's dial still wins.
- **Defenders** (`Residents::defender`, serde default, kept in step with
  `fee`, `medic` and `grave` at every resize, remove and truncate):
  `Residents::open` takes `defenders` after the mercenaries, **not** cut
  to the bunks, in `Uniform::Station`, `Gear::hired_for(defense::
  defender_seed(seed, n), ..)`, the residents' bandages, no fee (never
  hailed or hired), dealt `Role::Civilian`. `open_residents` asks
  `World::defenders_of(station)`: nought unless threatened or a defence
  is still running, else `defense::defenders(days_gone)` —
  `min(DEFENDERS_BASE + days / DEFENDER_DAYS, DEFENDERS_MAX)` (2, a
  spread's five days, 8; placeholders). They never shelter (`visit`'s list
  keeps a **town's** guard, the mercenaries and the defenders out of it).
  `reopen_residents` counts them with the people and the mercenaries.
- **The wave** at a defence is `droid::wave_size(players + defenders
  fielded, ..)` (`World::defenders_fielded`, the flags standing or not;
  `droid_wave_size` → `wave_size_with`). Feature 105 made the players the
  crew term, so the defenders are counted as players.
- **No bookkeeping.** `close_residents` counts no defender's death (its
  grave stays, `hired: false`); `town_is_dead` counts the site's own
  (`Residents::is_own`: not a mercenary, a defender or a grave), and a
  site with none of its own is never dead.
- **Won**: the bounty through the ordinary path — **and held until every
  wreck is counted**: `visit` counts a machine down only while the
  defence runs, and a win declared the step the last one fell left it
  uncounted and its bounty unpaid (towns too, before). `TownHeld` for
  every site; a **town** onto `held_towns`, immune, the joiners; a station
  or a derelict cleared and nothing more — `infest` may take it later
  (it marks a defence lost only when not already over), an Attack site
  then.
- **Leaving before the last wave** is `leave_mission`'s `falls` as it was
  — any site now: `infest`, `TownFell`, the pending bounty dropped. The
  words in `names.rs` (`TOWN_HELD`, `TOWN_FELL`) say *site*, not *town*.
- **A trader is never the machines'**: `infest` refuses one (beside the
  crisis's own skip), so it is never threatened, held or the jammer
  (`trader::jammer_candidate` already kept it out of the jammer's rule).

**The tests' dial.** `World::set_quiet_sites_for_probe(bool)` (saved,
serde default, not hashed) makes every non-trader, non-held site a
peaceful stop and reopens the open room without its defenders.
`fixture::simulation_world` and `fixture::crewed_world` set it — the
shared fixtures of every test whose subject is not the fight — and
`fixture::open_simulation_world` / `open_crewed_world` are the game as it
plays: `reference_world` (and so `reference_run_world`), `SURVIVORS`
and `tests_defense.rs` use those. Two tests that build a world by hand
for a hire set the dial themselves:
`tests::a_mercenary_is_hired_from_the_station_and_paid_by_the_month` and
`tests_medic::a_medic_earns_five_on_a_mercenary_and_beams_are_cleared_by_a_hire_and_a_bot_lost`.
The ship's `PINNED`, `PICTURES` and `self_check` use no dial. A scripted
run with nobody at the keyboard now has to walk the players back aboard
before *Back to ship* can carry (`fixture::walk_the_players_aboard`,
`CrewOrder::SendTo` to the gangway) — the reference run and `SURVIVORS`
do, and answer the departure check.

**Mining sites are gone**: the generator never builds a
`StationKind::MiningOutpost` (`worldgen::data::parent_suits`, the variant
kept for its code), and the system map draws no pickaxe on a belt
(`crates/ship/CLAUDE.md`).

**The trader measurement.** The first draft of this task made a trader
"a desk with a weapon or armour trade": 54.9–57.9 % of all sites over ten
seeds (median 56.5 %), past the 5–40 % the spec allowed, so it waited;
"both trades" was 14 %, stations-only-either 20 %. Task 114 then made
traders their own stateless roll (about one station in ten), which is the
Trader kind here.

**What moved.** `SAVE_VERSION` **46**, `wire::PROTOCOL` **38** (the
relay wants redeploying); `REFERENCE_CHECKSUM`, `SURVIVORS`, the ship's
`PINNED` (all eleven) and `PICTURES`' `simulation_deck` — each note says
why, and `simulation_deck` came back bit for bit with the sites quiet;
the old-layout checks (`REFERENCE_BEFORE_112`, `SURVIVORS_BEFORE_112`,
`PINNED_BEFORE_112`) taken again; and `worldgen`'s `REFERENCE_CHECKSUMS`
for the mining outposts. `tests_defense.rs` (a station defended end to
end, the wave with its defenders, a derelict's defenders dying for
nothing, leaving early) and `tests_trader.rs` (every site one kind over
three seeds; a trader never infested or the jammer) are the rule.

## Relics drop by the day (task 117)

How a relic enters a run, replacing the site-tier offer. A relic's
**tier** is data and nothing the player sees: it decides the odds of
drawing it and its price at a trader (`data::RELIC_PRICE`, 1 500, 3 000,
5 000), and `names::relic_tier` went with every "Tier n" on the screen.
`RelicChoice::tier` went too (out of the save and the hash).

- **One roll for every source** — a reward's three, a cache's one, a
  trader's one: `relic::offer(pool, day, n, seed)`. Each draw rolls a
  tier by `relic::tier_odds(day)` — `data::RELIC_ODDS_START` [70, 25, 5]
  on day nought, a straight line to `RELIC_ODDS_END` [40, 35, 25] on
  `RELIC_ODDS_FULL_DAY` (30), then flat, integers rounded down and a roll
  taken against their sum — then a relic of that tier; an empty tier is
  rolled again by the same odds among the tiers with any left; nothing
  left is nothing drawn. None twice in one offer. The day is
  `World::days_gone()` at the draw; the seed is the galaxy, the site and
  `Relics::offers`, as before. The site's enemy tier is not read:
  `site_tier_code` went.
- **The pool loses a relic when a Bim gets it**, never when it is
  offered: `relic_if_carried` takes the relic out (`Relics::take_from_pool`)
  whether it is given or goes pending, `trade_relic_if_carried` when it is
  bought, and `relics_on_leaving` puts a lost pending relic back
  (`Relics::return_to_pool`, in list order, never a held or pending one).
  What is **on offer, pending or held** is kept out of a draw by
  `Relics::in_play`; what is **on the table of an open trader** by
  `World::draw_relics` itself, which also takes the relic off the table of
  a trader closed today (`release_closed_traders_relics`) — it never left
  the pool, so from then on it can be drawn elsewhere. There is no
  restock that returns a trader's relic: a trader keeps its first until
  it is bought or closes.
- **The starting pool is 23** (`RelicDef::first`), and
  `relic::Profile::pool` counts every starting relic in whatever the file
  holds (`Profile::unlocked`), so a profile written before a starting relic
  was added has it. `record_run` unlocks the first two not unlocked in
  `Relic::ALL`'s order — seven wins for the fourteen. The code order of
  the relics 12–36 is chosen so the locked ones fall in the unlock order
  the task gives.

`relic::tests` pins the odds (the line, the day-fifteen midpoint, the
shares over ten thousand seeds at days 0, 15, 30), no duplicates, the
re-roll of an empty tier (tier two against three at 25 to 5 with no tier
one), the pool's two doors and an old profile. `tests_relic.rs` pins the
reward left in the pool and the one taken out of it, any tier at any
site, nothing at all, the day read at the draw, and a cache's relic out
and back in; `tests_trader.rs` a trader's relic kept out of other draws
while on the table, out of the pool once bought, and back in the running
when the trader closes.

## Five patches of relics (task 118)

Twenty-five relics (`Relic` 12–36), each a row of `relic::RELICS`, and
**`relic_hooks.rs`**, a child of `world` like `relics.rs`, which is where
all of them are read. The rule the file keeps: **nothing there draws from
a stream, and nothing there runs for a crew holding no relic**
(`World::any_relics`), so a run without them is the run it was.

- **A crew hit on a machine** goes through `land_on_machines`, called in
  `visit` before the residents are borrowed (hostile or defending only);
  the hits on the residents' Bims are handed back to `visit`'s own loop.
  With relics held, `relic_hit_on_machine` reads the part off the roll,
  then: *Marksman's Habit* moves its holder's first hit on each machine
  (`Relics::limb_aimed`, slot and body) to a limb it still has;
  `Stat::MachineDamage` under a `Situation` — on a limb it still has
  (*Servo Cutter*), missing its arms or legs (*Crippler's Mark*), from
  outside the front arc (*Blind Spot*: `Droid::front` against the
  shooter's bearing, `GUARDIAN_SHIELD_COS`, or *Wide Angle Optics*'
  narrower `front_cos`); *Crossfire* for the holder and for a crewmate
  opposite it (bearings over a hundred and twenty degrees apart, both
  within its tiles, positions `crew_ashore` in the residents' units);
  *Spotter*'s mark (`Relics::spotted`: slot, body, until) for anybody's
  hit; and *Total Teardown* multiplied on a hit on a limb already gone.
  Whether a player's last hit came from the side is kept
  (`Relics::flanked`, by body) for the kill.
- **A kill** is a `MachineKill` (by, bounty, crippled, flanked) off
  `visit`'s down loop, which also forgets the body's marks — a wave's
  machines take the last wave's indices. `machine_kills_noted`: the
  bounty's `Stat::Bounty` under `Situation::crippled` (*Parts Broker*),
  `Trigger::Kill`, then `relics_on_a_kill`: *Scrap Collector*'s money
  into what is pending, `Trigger::FlankKill` (*Signal Scrambler*) and
  `Trigger::CrewKill` — its holder's kill or any bot's (*Squad Morale*,
  stacking with *Kill Relay*). `machine_kills` (the tuples) is the tests'.
- **Timed effects** are `Relics::buffs` (`Buff { who, relic, until }`, a
  crew index and a mission minute), put by `Action::{Sprint, Unseen,
  Tether}` and read by `relic_buff_on`: *Sprint Coil*'s pace and
  *Tether Field*'s share of a hit in `lift_by_relic_hooks`, *Signal
  Scrambler*'s cloak in `visit` (the crew member `None` on the machines'
  list, `unseen_by_machines`). A hook's `cooldown` is
  `Relics::ready_at`, checked in `relic_trigger_on`.
- **The skill**: `lift_by_relic_hooks` after `lift_by_relics` — *War
  Chest* (`war_chest_percent`: the pool over the players, a step a
  thousand, capped), the buffs, and the auras of other players fit to
  act within their tiles (`Effect::Aura`: *Field Radio*'s fire rate for
  anybody, *Cover Formation*'s `DamageTaken` for bots). `DamageTaken`
  goes to `bims::combat::Skill::damage_taken`, which the room multiplies
  into every hit on the body before the armour (`strike_stripping`).
  `hand_the_room_the_shield_fronts` gives the room each shooter's front
  (`Game::set_shield_fronts`), which a Guardian's shield is asked with.
- **The healing is hit points, never the blood** (the health system is
  going to lose its blood): `relics_mend`, a stage right after
  `settle_relic_downs`, puts back *Pressure Seal*'s `Rule::Regen` every
  step a holder is alive, and *Clot Booster*'s `Rule::MendWhileDown`
  while it is down and its seconds since `Relics::downed_at` run —
  through `Game::heal` (`Health::heal`: shared over the parts by what
  each is short of, none to a leg gone or a part a trauma holds).
  `settle_medics` says every dressing a player finished
  (`relics_on_a_dressing`): `Trigger::Bandaged` on its own or a
  crewmate's (*Quick Wrap*, `Action::Heal` on the patient) and
  `Trigger::BandagedCrewmate` on a crewmate's (*Tether Field*).
- **A crewmate down**: `settle_relic_downs`, after `settle_relics`, off
  `downs_before_the_step`: `downed_at` kept for the players, and
  `Trigger::CrewmateDowned` to every other player for a crew member
  newly down (*Lifeline*: `Action::Shelter`, both surged, within its
  tiles, once a mission). *Rally Point* is `Trigger::AbilityUse` →
  `Action::RallyUp` (`bring_round` every crewmate down within its tiles),
  and fires — so is spent — only when it got somebody up.
- **Pay and the trader**: *Hazard Pay* in `settle_clear`
  (`relics_pay_the_clear`, into the pool, `RelicFired`); *Trade License*
  is `trader_discount` inside `shelf_price` and `trader_relic_price`
  (the best discount among the players); *Restock Codes* is
  `Command::Restock` (`restock`, `can_restock`).

**In `world_checksum`** after the offers, and only where any of it is
set: the buffs, the cooldowns, the marks, `limb_aimed`, `flanked`,
`downed_at` and `restocked`. `tests_relic_patches.rs` is every relic
where its hook is read, and two worlds holding all twenty-five alike step
for step; `relic::tests` pin the rows.

## One speed, and experience alike for every class (task 119)

> Every section above that speaks of 3×, 10×, 24× or the top speed, of a
> wave putting everybody back to 1×, or of `XP_BUILT`, `XP_HEALED`,
> `XP_HIRE`, `TANK_HITS_PER_XP` and `settle_tanks`, describes what **task
> 119 deleted**.

- **`Speed` is `Paused` (0) and `Real` (1)** and nothing else;
  `data::DAY_SPEED` and `TOP_SPEED` went. `speed::effective` is still the
  slowest request, which with two speeds is the one rule: **a pause by any
  player pauses the world**. A wave landing no longer resets anybody's
  request — a paused world takes no step, so no wave can land in one.
  `the_world_runs_at_one_times_or_not_at_all` and
  `any_player_s_pause_pauses_everyone` in `tests.rs`, and `speed::tests`,
  are the rule. The reference run's two requests are `Real`.
- **Experience is two things and every class's alike**: an enemy going
  down within `VICINITY_TILES` (fifty) is `XP_ENEMY_DOWN` (10), once, and
  its death `XP_ENEMY_DEAD` (5, back from feature 109), once — whether it
  died the step it went down (every machine) or bled out later — to every
  classed crew member in range (`experience`). Nothing else gives any:
  `award_engineers_near`, the hire's, the medic's and `settle_tanks` went.
  `Bim::hits_taken` stays, a count that only climbs, for the relics' *hit
  taken*. `finish_build` lost its `who`.
  `each_class_gets_identical_experience_for_the_same_kills`
  (`tests_engineer.rs`) is the rule; the class tests that asserted a
  class's own experience now assert none.
- **What moved**: `SAVE_VERSION` **49**, `wire::PROTOCOL` **41** (the
  relay wants redeploying), and whatever pins read the crew's experience
  (see the root `CLAUDE.md`).

## One bar of hit points, downed and revived (task 120)

> Every section above that speaks of blood, a wound, a trauma, a dying
> state, a bandage, a medkit, the drug lab, a treatment, *out cold* for a
> crew member or the medicine's charges describes what **task 120
> deleted**. The room's half is `crates/game/CLAUDE.md` ("One bar of hit
> points"); this is the world's.

- **Downed and revived are said as events**: `WorldEvent::CrewDowned {
  who }` (42, off `Game::take_downs` in `casualties`) and `CrewRevived {
  who, by }` (43, `who + 100 * by`, off `take_revives` in the step, which
  also fires the relics' `Trigger::Revived`). They took the codes of
  `CrewDying` and `CrewTreated`, which went. `CrewDown` is still a death.
- **The revive time is the world's**: `World::revive_seconds(who)` —
  `class::revive_time(medic, quicker)`: `health::REVIVE_SECONDS` (10), or
  `class::MEDIC_REVIVE_SECONDS` (4) for a medic of either kind, less
  *Trauma Kit*'s `data::TRAUMA_KIT_REVIVE_SECONDS` (2, `Rule::QuickRevive`),
  never under `data::REVIVE_FLOOR_SECONDS` (1) — handed to the room as
  `Skill::revive` in `skill_of`. The crew's room revives of its own
  accord (`set_revivers(true)` every step); a station's or a town's never
  (`Residents::open`, `replace_room`), so a Manufacturer downed stays
  down and dies, and so does a townsperson nobody of the crew picks up
  ("A townsperson is picked up with the medkit" at the end).
- **The medic** (`hand_the_room_the_medics`): the beam heals hit points
  through `Game::heal` at `beam_rate` an hour of the clock
  (`class::HEAL_BEAM_HP`, 30, marked for tuning; *strong beam* times
  `STRONG_BEAM_RATE`); `beam_reaches` lets the medic be its own patient;
  the surge charges while a patient is up and short of its bar. The
  surge still absorbs every hit.
- **A mission's end**: a body downed is **left behind** as ever
  (`comes_home` asks it is up), and every body's slow from a down is
  forgotten (`Game::forget_downed`) after the fallen respawn. A mission's
  start restores the whole bar (`restore_health`).
- **Gone from the world**: `Charge::{Medkit, Bandage}` (and
  `Charge::MEDICINE`, `everybody`), every medicine constant and its
  cooldowns, `hand_the_room_the_medicine`, `take_the_room_s_medicine`,
  `bank_medicine`, the residents' dressings and medkits
  (`RESIDENT_BANDAGES`/`MEDKITS`), `Medic::field_surgery_used`, the
  aura's nerve and bleed, the rally's no-running, `Stat::HealingReceived`,
  `Trigger::{Bandaged, BandagedCrewmate}`. The `ResourceId::{Medkit,
  Bandage}` resources stay (a design's cargo slots), stocked nowhere and
  made nowhere; `shipdesign`'s `DrugLab` part and its recipe went.
- **Relics reinterpreted**: *Trauma Kit* is the two seconds off a revive;
  *Quick Wrap* and *Tether Field* fire on `Trigger::Revived` (the reviver
  holding it); *Clot Booster* heals its holder for its seconds after a
  down once it is up again (a downed body is healed by nothing).
- **No-op talents** (kept in their slots, doing nothing, the app saying
  so): the soldier's *Iron Nerve*; the medic's *Field Dressing*,
  *Surgeon*, *Clean Hands*, *Steady Hands*, *Self-care*, *Closing Surge*,
  *Field Surgeon* and the fixed *Mender*; the tank's *Unmovable* and
  *Hold Fast*; the commander's *Steady Ranks* and *Grit*.

**Checksum**: a body's `down_left` and `was_downed` are hashed where the
blood and the parts were, and `field_surgery_used`, the surge's closing
flag and `fear` are out. **What moved**: `SAVE_VERSION` **50**,
`wire::PROTOCOL` **42** (the relay wants redeploying),
`REFERENCE_CHECKSUM`, `SURVIVORS`, the ship's `PINNED` (all eleven) and
`PICTURES`' two decks, `shipdesign`'s hashes (the drug lab off the part
list and the playtest ship) and `worldgen`'s `REFERENCE_CHECKSUMS` (no
shelf stocks medicine). `tests_medic.rs` is rewritten for the beam, the
self-beam and the revive times; `tests_relic.rs` has
`trauma_kit_takes_two_seconds_off_a_revive_and_never_under_one`,
`tests_manufacturer.rs`
`a_manufacturer_downed_is_never_revived_and_nothing_of_it_is_taken`, and
`tests_mission.rs` `the_slow_a_downing_leaves_is_cleared_at_the_mission_s_end`.

## The soldier's ranked kit (task 124)

> "The soldier: the brace, the skills and the grenades (feature 75)"
> above describes the ten levels of talents **task 124 deleted**, and the
> soldier's half of "A grenade is a charge" its old numbers. This is what
> the soldier is now.

**Levels are a class's own.** `class::level_xp(class)` is the table —
`RANKED_LEVEL_XP` (sixteen levels, the top at 3 200, where the others'
tenth is) for a class with a ranked kit, `LEVEL_XP` for the rest — and
`class::levels`, `class::level_of(class, xp)` and `Progress::{level,
to_next, gain}` all take the class. `World::level_of(who)` is what every
gate asks. `class::ranked(class)` says which classes have a kit: the
soldier (the engineer's is task 127).

**A rank is a point.** `Progress::ranks: [u8; 4]` (serde default) is the
ranks bought, Q C E R (`class::SLOT_Q` …); `Progress::points(class)` is
the level less the ranks bought, nought for a class of talents.
`Progress::can_rank_up`/`rank_up` refuse, in order, `NoRankedKit` (110:
no kit, or a slot past R), `NoSkillPoint` (111), `TopRank` (112, at
`MAX_RANK`) and `RankLocked` (113: below `class::rank_level` — Q, C and
E rank `n` at `2n − 1`, R at `ULTIMATE_LEVELS`). `Command::RankUp { slot,
ability_slot }` is heard between missions as in one (`World::rank_up`),
says `WorldEvent::RankedUp { who, class, ability_slot, rank }` (130), and
a Frag Grenade rank that lifts the charges puts the new ones in hand at
once (`grant_charges`). `World::{rank_of, points_of, can_rank_up}` are
the readings; `set_ranks_for_probe(who, ranks)` sets them outright,
capped by the level's gates (`BIMS_RANKS`). The ranks are hashed with
the progress only where any is bought, and kept through a death as the
picks are. `PickTalent` for a soldier is `NotAPickLevel`: `pick_at` has
no soldier rows, and `Talent` 14–27 are deleted, their codes free
(`Talent::from_code` searches `ALL`, 55 long).

- **Q, Frag Grenade**: `World::charges(who, Charge::Grenade)` and
  `charge_cooldown` read `GRENADE_CHARGES`/`GRENADE_COOLDOWN` of the rank
  (nought charges at rank nought, so nothing restocks), `grenade_damage`
  and `grenade_radius` its `GRENADE_DAMAGE`/`GRENADE_RADIUS`; range and
  fuse are fixed. `can_throw` is `NoGrenadesYet` at rank nought. The
  soldier's start gives the rank's grenades (none at rank nought).
- **C, Weak Spot**: `Skill::crit_chance` (`WEAK_SPOT_CHANCE`). The roll is
  the room's, where a hit lands — a friendly bolt on a target, a Bim's
  blow — off **`World::crit_rng`**, a stream of its own off the galaxy's
  seed (`CRIT_SALT`), lent to the crew's room around its step
  (`Game::lend_crit_rng`/`take_crit_rng`) and hashed only once drawn on
  (`fresh_crit_rng`). A body with no chance draws nothing, so no other
  roll of a fight moves. The hit carries `crit` and `flat` (the weapon's
  damage at the distance, before any factor); the world adds
  `flat × (WEAK_SPOT_DAMAGE − 1)` (`World::crit_extra`) **after** every
  relic's factor — in `land_on_machines` after the machine hooks, in
  `visit` for a hit on a Bim — and before the armour, which the room's
  `strike` takes after. A grenade's burst is never critical.
- **E, Brace**: `NotLearnt` (114) at rank nought. Braced, `soldier_skill`
  sets `Skill::miss_cut` (`BRACE_MISS_CUT`: the miss chance times one
  less it, near and far, in `Skill::stats_at`, the hit chance never past
  one, and with no cut the odds to the bit as before),
  `damage_taken` × `BRACE_DAMAGE_TAKEN`, and `deadeye` at the fourth.
- **R, Rampage**: `crate::soldier::Soldier { began, until, extended }` a
  crew member on `World::soldiers` (serde default), mission minutes.
  `Command::Rampage` — `can_rampage`: `NotASoldier`, `OutOfReach` (not fit
  to act, or downed), `NotLearnt`, `AlreadyActive` (115), `CoolingDown` —
  sets `began` and `until`. While it runs `soldier_skill` multiplies the
  fire rate and the damage taken and sets `walking` to one; it stacks with
  the brace. `rampage_kill`, from `machine_kills_noted` for a kill
  credited the way *Kill Relay*'s is, adds `RAMPAGE_EXTEND_SECONDS` at
  the fourth rank up to `RAMPAGE_EXTEND_MAX`. The cooldown runs from
  `began`, times *Coolant Loop*'s factor, and `cooldowns_less` moves
  `began` (never `until`); `make_whole` clears every soldier, so a
  Rampage is ready at every mission's start. Hashed only where one has
  been gone on.

The old *rampage* stacks (`settle_rampage`, `Bim::rampage`) and
`enemy_standing`, which only they read, are gone; the checksum eats a
nought where the stacks were. `tests_soldier.rs` is the task's tests and
`class::tests::a_ranked_kit_climbs_sixteen_levels_and_buys_a_rank_a_point`
the table's; the room's are `combat::tests::weak_spot_rolls_off_its_own_stream_and_every_other_roll_is_as_it_was`
and `a_miss_cut_takes_its_share_of_the_misses_and_never_passes_one`.

## The engineer's ranked kit, and charges without kits (task 127)

> "The engineer: a class, its levels and its deployables", "Charges, not
> crafting" and "A grenade is a charge" above describe what **task 127
> replaced**: the engineer's ten levels of talents, the kits in a pack and
> the sentry charge. Kept as history; this is what is there now.

- **The engineer is a ranked kit** beside the soldier's (task 124):
  `class::ranked` answers for both, sixteen levels on `RANKED_LEVEL_XP`,
  a skill point a level, the same gates (`rank_level`). Its slots are **Q
  EMP, C Healing Sentry, E Sandbags, R Sentry** (the ultimate); every
  number is a table of four in `class.rs` (`EMP_*`, `HEALING_SENTRY_*`,
  `SANDBAG_*`, `SENTRY_*`), read with `by_rank`. The engineer's talents
  (codes 0–13) are gone and their codes left free; `pick_at` and
  `fixed_at` have no engineer rows, so `PickTalent` is `NotAPickLevel`.
- **A charge is a counter** (`World::charges_held`, a `[u32;
  Charge::CODES]` a crew member, saved and hashed): `Charge` is
  `Sandbag` 0, `Grenade` 2, `HealingSentry` 3, `Emp` 4 — the sentry's (1)
  gone, its code free — `Charge::slot` the ability slot whose rank says
  how many (`World::charges`) and how fast (`charge_cooldown`).
  `restock_charges` raises the counter; a deployable laid, an EMP or a
  grenade thrown lowers it; `grant_charges` puts in hand what a rank
  lifted (`rank_up`, `set_ranks_for_probe`); `fill_charges` sets every
  counter at a mission's start. **No charge is an item**:
  `ResourceId::{SandbagKit, SentryKit, Grenade}` (15–17) are gone from
  every crate, their codes left free (`ResourceId::CODES` is 22 and
  `ALL` 19 — index by `ResourceId::from_code`, never `ALL[code]`).
  `reused_kits` went: **laying is nobody's experience** (task 119's rule,
  kept at the user's word), so a reused charge needs no counting.
- **Deployables** (`crate::deploy`): `DeployKind::{Sandbags, Sentry,
  HealingSentry}` (0, 1, 2), a `Deployable::expires` for the ultimate's.
  `Command::Deploy { kind }` lays sandbags or a Healing Sentry,
  `Command::Sentry { slot, tile }` the ultimate, `Command::Emp { slot, x,
  y }` throws; `PackUp` gives a charge back capped at its charges and
  refuses the sentry (`NoSuchDeployable`). The laying is the room's
  `Kind::Deploy { kind, steady }` — the ultimate's `steady`, so a hit
  does not drop it. `laid_health`, `deploy_minutes`, `laid_of`.
- **The Healing Sentry** is a room `Sentry` with `heals` set: a target of
  the enemy's like the gun sentry, never firing. `World::healing_links`
  is the rule — crew on their feet (not downed, not outside), short of
  `MAX_HEALTH`, within the rank's radius and `Game::line_clear` of it —
  and `heal_by_sentries`, in stage 5 after the medics, heals the best
  rate only (`Game::heal`, never past full). Its charges are the
  standing limit (`finish_deploy` destroys the oldest).
- **The ultimate** (`crate::engineer::Engineer::sentry_laid`, saved and
  hashed where set): `sentry_cooldown(_left)`, `sentry_seconds`,
  `sentry_left`; the cooldown runs from the laying on the mission clock,
  is moved by the relics' cooldown cut (`cooldowns_less`), and every
  mission's start forgets it and takes a standing sentry off
  (`make_whole`). `expire_sentries` removes it when its time is up;
  `drop_station_deployables` takes it with the station's at an unjoin.
  One stands at a time; `sentry_weapon` is the minigun at the rank's
  tier, `sentry_skill` its fire rate.
- **The EMP** is the room's grenade with `stun` set (`Game::throw_emp`):
  its burst lands no hit and notes every target within its radius
  (`Game::take_stuns`); `settle_stuns`, right after the crew's room steps
  and before the residents' does, stuns each machine past the residents'
  Bims (`Game::stun_droid` → `Droid::stun`, which refuses the Heart's
  machines). A stunned droid is `Droid::stunned`/`exposed` (saved; hashed
  here only where one is). Rank four's `EMP_EXPOSE_PERCENT` goes on the
  `Stat::MachineDamage` sum in `land_on_machines` while the machine is
  stunned.

**What moved.** `SAVE_VERSION` **52**, `wire::PROTOCOL` **45** (the relay
wants redeploying), `worldgen`'s `REFERENCE_CHECKSUMS` (three price leans
fewer — no bump), and `REFERENCE_CHECKSUM`, `SURVIVORS` and the ship's
`PINNED` (with task 124's soldier; each note says why). `shipdesign`'s
hashes did not move: the cargo is still twenty-two slots.
`tests_engineer.rs` is the task's tests.

## The commander's ranked kit (task 129)

> "The commander: the aura, the squad and the rally (feature 78)" above
> describes the ten levels of talents **task 129 replaced**. Kept as
> history; this is what is there now.

- **Ranked**: `class::ranked` answers for the commander; sixteen levels
  on `RANKED_LEVEL_XP`, the soldier's gates. Every number is a table of
  four in `class.rs` (`BATTLE_CRY_*`, `AURA_*`, `RALLY_*`,
  `REINFORCEMENT*`), read with `by_rank`. `pick_at` has no commander rows.
- **Base traits**: `squad_range` is `SQUAD_RANGE` at every level;
  `hire_fee` takes `HIRE_DISCOUNT_PERCENT` off; `SquadKind::Attack
  { enemy }` is one mark, ended when it is down (no *relentless*, no
  *pincer*, no *focus fire*), hashed as a list of one so no number moved.
- **Q Battle Cry, E Rally**: `Command::BattleCry { slot }` and
  `Command::Rally`, refused `NotACommander`, `OutOfReach` (unfit or
  downed), `NotLearnt` (rank nought) and `CoolingDown`.
  `Commander::{last_battle_cry, cried, last_rally, rallied}`: the mission
  minute and whom it reached (`World::crew_within`, himself included) —
  fixed at the call. `battle_cry_reaching` / `rally_reaching` answer the
  commander covering a Bim (the strongest of two). `lift_by_commanders`
  (in `skill_of`) multiplies the fire rate, the damage taken and `walk`
  (the always-on pace, not *runner*'s `pace`). Both are cleared at every
  mission's start (`make_whole`); `cooldowns_less` (*Kill Relay*) moves
  their start back only once the shout is over, since one timestamp says
  both the run and the cooldown. `WorldEvent::BattleCried` (134).
- **C Command Aura**: `aura_radius`/`aura_cast_by` by rank, `Aura
  { damage }` alone; `in_aura_of` takes the commander himself, fit and
  not downed; the higher of two holds. Multiplied into `Skill::damage`
  and `melee`. A sentry's skill is its own and is lifted by nothing.
  `crit_extra` asks for a **soldier** now, since slot C is the aura on a
  commander.
- **R Reinforcements**: `World::reinforcements: Vec<Reinforcement { who,
  by }>` (saved, hashed where any). `bring_reinforcements` at the end of
  `begin_mission` (and `reinforce_for_probe`): per player commander with
  a rank, `REINFORCEMENTS` Bims on `Game::free_tiles_near` his position
  within `REINFORCEMENT_REACH_TILES` (deck, reachable, nobody standing
  there), `Game::enlist_reinforcement` with the rank's auto rifle, face
  off `REINFORCEMENT_SALT` — `WorldEvent::Reinforced { who, count }`
  (135). A dead one is `Game::vanish`ed by `settle_reinforcements` (end of
  `casualties`) and `fall` pays nothing for it; `left_behind` and
  `worth` skip them; `leave_mission` drops them all first
  (`send_reinforcements_home` → `drop_crew_member`, which now remaps the
  reinforcements and every commander's reach lists).

`tests_commander.rs` is the task's tests.

## The medic's ranked kit (task 130)

> **Reworked by task 153** ("The medic reworked" at the end of this
> file): the Nanite Burst, the Healing Aura and the Cloak are gone, and the
> beam fires at full rate. What follows is the history.

> "The medic: the heal beam and the surge (feature 76)" above describes
> the ten levels of talents and the surge **task 130 replaced**. Kept as
> history; this is what is there now.

- **Ranked**: `class::ranked` answers for the medic; sixteen levels on
  `RANKED_LEVEL_XP`, the soldier's gates. Every number is a table of four
  in `class.rs` (`NANITE_BURST_*`, `HEALING_AURA_*`, `HEAL_BEAM_RATE`,
  `HEAL_BEAM_RANGES`, `HEAL_BEAM_PATIENTS`, `CLOAK_*`), read with
  `by_rank`. His talents (codes 28–41) are gone, their codes free;
  `pick_at` has no medic rows, `Talent::ALL` is the tank's thirteen.
  `Ability::Surge` went; `Ability::{NaniteBurst, Cloak}` are new.
- **Base traits**: `revive_time` as before (four seconds for a medic of
  either kind); `medic_skill` sets `Skill::revived_to` to
  `MEDIC_REVIVED_TO` for a medic of the class, which the room reads off
  the helper at the revive's end.
- **Q Nanite Burst**: `Command::NaniteBurst { slot }` → `WorldEvent::
  NaniteBurst { who, healed }` (136). `can_nanite_burst`: `NotAMedic`,
  `OutOfReach` (unfit — downed among it), `NotLearnt`, `CoolingDown`.
  `nanite_burst_reaching` is every crew member on the deck, on its feet,
  within the radius and — bar the medic — `room.sees` him, the beam's
  sight; each is healed the rank's points through `heal_crew`.
  `Medic::last_burst` (a mission minute) is the cooldown's start.
- **C Healing Aura**: `healing_aura_reaching(who)` — the medic, fit and
  not downed, whose radius the Bim stands in, the higher factor of two —
  and **`heal_factor(who)`**, one where none reaches. `heal_crew(who,
  points)` multiplies it in and is what every heal of the world's goes
  through: the beam (`hand_the_room_the_medics`), the Healing Sentry
  (`heal_by_sentries`), the burst, *Pressure Seal* and *Clot Booster*
  (`relics_mend`) and *Quick Wrap* (`Action::Heal`). A revive is the
  room's and never asks.
- **E Heal Beam**: `can_beam` wants a rank (`NotLearnt`) after
  `OutOfReach`; `beam_range`, `beam_rate` and `beam_patients` read the
  rank (the first's before one). `HEAL_BEAM_HP` is 120. From
  `HEAL_BEAM_ITEM_RANK` (4) `beam_item_rate` adds the medic's own item
  regeneration (`item_regen_now`, ×60 an hour) to each patient's rate in
  `hand_the_room_the_medics`, never to himself. `medic_skill`
  holds the fire while linked below `HEAL_BEAM_FIRE_RANK` (3) and halves
  the fire rate from it. A cloaked medic's beam is let go in
  `hand_the_room_the_medics`.
- **R Cloak**: `Command::Cloak { slot, target }` → `WorldEvent::Cloaked
  { who, target }` (137). The app sends the crew member under the pointer
  or the medic's own slot. `can_cloak`: `NotAMedic`, `OutOfReach`,
  `NotLearnt`, `CoolingDown`, `NotACrewmate` (not a living crew member —
  downed is fine), then for another `OutOfCloakRange` (116, past
  `CLOAK_RANGE` or outside) and `NoSightOfTarget` (117). The cloak is
  `World::cloaks[target]` (`medic::Cloak { until, pace, seconds }`): the
  later of the two ends and the faster of the two paces when one is
  already on, `seconds` its whole length for the ring. `Medic::last_cloak`
  is the cooldown's start. **While it lasts**: `hidden_from_enemies`
  (Signal Scrambler's `unseen_by_machines` or a cloak) makes it `None` on
  the residents' list in `visit` — every enemy's, the machines' own list
  in a defence included — and tells the residents' room
  `set_targets_withheld`; `lift_by_cloak` in `skill_of` holds its fire
  and multiplies `walk`; `apply` refuses every class command
  (`Refusal::Cloaked`, 118) before dispatch — a brace or a wall put down,
  a beam let go, a pack-up and a carry go through. `hand_the_room_the_cloaks`
  (before the medics) forgets one run out and tells the room
  `Game::set_cloaked` for its picture.
- **Timers**: the burst and the cloak run on the mission clock, times
  *Coolant Loop*; `cooldowns_less` (*Kill Relay*) moves both starts back;
  `make_whole` clears both and every cloak; `casualties` clears a dead
  crew member's cloak; `drop_crew_member` removes its index; a hire
  resizes the list.
- **Checksum**: a nought where `Medic::charge` was (so a world with no
  medic hashes what it did), then each medic's `last_burst`/`last_cloak`
  and every cloak's `until` and `pace`, only where any is set.
  `Cloak::seconds` is the picture's and not hashed.

`tests_medic.rs` is the task's tests; `class::tests` the tables; the
ship's `a_game_saved_and_read_back…` the save's round trip with a burst
set off.

## The Manufacturers attack a defence before day ten (task 131)

**While they still have the machines** (`manufacturer::has_droids`, before
`MANUFACTURER_DROIDS_LOST_DAY`) every wave that lands on a site the crew
defend is the Manufacturers': `World::defense_by_manufacturers` says so,
and `defense_waves` lays `World::lay_defense_manufacturers` where it laid
`settle_defense_droids` — `n` bodies at the wave's `arrival_spots`, each a
Trooper at `manufacturer::trooper_percent(day)` (the garrison's own roll,
`manufacturer::garrison`, off `garrison_seed ^ DEFENSE_SALT`) and one of
their people otherwise, armed by `manufacturer::gear`. The garrison and the
defence share `stand_manufacturers`: their people first, then the Troopers.
From day ten the machines as before. `set_defense_by_machines_for_probe`
(saved, not hashed, like `droid_kinds_forced`) keeps a defence's waves the
machines' whatever the day, and `tests_defense.rs`' `basic()` takes it, since
those tests are the machines' fight at day nought; `manufacturers_attack` in
the same file is the Manufacturers'.

What had to change for Bims of the enemy's in a **friendly** room — the
room's half is `crates/game/CLAUDE.md` ("An intruder"):

- **The site's targets are by body index now**, in `visit`: every Bim
  `None` but a Manufacturer on its feet, then the machines — so a town
  person's hit comes back as a body index, a machine's past the Bims
  (`strike_droid`) and a Manufacturer's among them (`strike`/`blast`). For a
  wave of machines alone the list is the machines behind a run of `None`s:
  the same aims in the same order.
- **The crew's targets** keep the whole index space and now leave a
  Manufacturer among the site's Bims on it; **the machines' list** leaves
  them off; **nobody of theirs shelters**.
- **The put-back** of a wave on a room built afresh reads
  `Residents::manufacturers_laid` for their waves (a room can hold them
  and the site's dead, so "no machines" is no test), and the machines'
  `droid_count() == 0` otherwise.
- **The win** waits for every Manufacturer to be counted at its first down
  (`xp_down`) as well as every machine; `experience` pays a Manufacturer of
  a defence its experience and bounty as it does at their own sites (it
  walks the Bims now, not only past `first_enemy_body`), and
  `townsfolk_join` never takes one.

## The ready check

A mission with a fight in it — the site alongside **not**
`site_cleared` (an Attack not yet cleared, a Defend threatened) — opens
**held** while `Run::ready_check` is on: `Run::briefing`, and `World::step`
takes stage 0's path (the commands heard, the step counted, nothing
else), so the mission clock waits at nought and nobody moves. Every
connected player presses *Ready* (`Command::Ready { slot, yes }`, applied
at once; `WorldEvent::Readied`); the last yes — or a `PlayerGone` that
leaves only yeses — is `WorldEvent::AllReady` and the mission starts.
While held, `apply` hears the speed, `Ready`, `PlayerGone`, the crew's
orders and the loadouts (`Equip`, `Unequip`, `Offer`, `AnswerOffer`,
`RankUp`) and refuses the rest `Refusal::AwaitingReady` (119); *Ready*
with nothing held is `NoReadyCheck` (120).

`begin_mission` opens the hold (`open_briefing`), and so does
`World::set_ready_check(true)` at the top of a mission — how the `game`
run's first mission gets it (`screens::designer::build_run`,
`BIMS_READY=0` to switch it off there, `=1` to switch it on for a
command's own run). **Off in `World::start`**: every test, the staged
commands and the pins step at once, and the checksum eats the three
fields only while the switch is on, so `REFERENCE_CHECKSUM` did not move.
`tests_ready.rs` is the rule. `SAVE_VERSION` 56, `wire::PROTOCOL` 50.

**The app shows nothing of the site while it is held** (October 2026):
the game screen's `veiled` draws the station's backdrop and no deck, no
bodies, no names or bars, and takes no pointer on the deck; the panels
(the loadout, the skill points) stay. `worldmap::ready_window` is Dota's
ready check in the middle of the screen: a card a player in its colour,
ticked and ringed green once ready, breathing while waited for, crossed
once gone; a segmented bar of `World::ready_count`; the Ready / Not
ready button.

## A fight won is a frozen deck (task 133)

> "The loop" and "One bar of hit points" above say a downed body is left
> behind and that *Back to ship* waits for the players to be aboard;
> **after a fight won** neither holds any more.

`World::fight_over()` is `Run::fought` and `mission_cleared()` in a
mission. From the step after the clear, `World::step` takes a path of its
own beside the ready check's: the commands, the step counted and
`settle_run` — **the room is not stepped**, so nobody moves, a downed
body's countdown stands and nobody bleeds out, and the mission clock
stops. `apply` hears the speed, `Return`, `LeaveBehind`, `PlayerGone`,
`Ready`, the loadouts and `RankUp`, and refuses the rest
`Refusal::FightOver` (121). `comes_home` is every crew member alive,
downed or not, so `left_behind` is empty and the departure never asks;
`waited_for` takes a downed player too (it has the button like the
rest); and a press counts as home wherever the Bim lies
(`home_for_departure`), so the last player's press is the ship leaving —
`press_return` walks nobody. `bring_home` stands them all at the gangway,
and the next mission's `make_whole` gets the downed up. A site with no
fight (`fought` false) is as before: walk home or be left.
`after_a_fight_won_the_deck_is_frozen_and_everybody_alive_comes_home`
(`tests_mission.rs`) is the rule. The app's button reads *Fight won —
Back to ship*. `wire::PROTOCOL` 52; nothing saved changed.

## Attack and defence evenly, and a defence pays nothing (task 136)

> "Every site is an attack, a defence or a trader (task 111)" above says
> every site not an enemy's is a defence; since task 136 half of them are
> the machines' **from the first day**.

- **Outposts** (`outposts.rs`, a child of `world`): `outposts::held`
  takes a system's candidate sites in id order — the fights it offers
  (`World::offered_fights`, task 135: its station and its town) — and
  gives every other one to the machines, a coin off the galaxy's seed and
  the star (`OUTPOST_SALT`) choosing whether the first is; **the crew's
  home is never one** (in the home system the coin is the one that leaves
  it a defence). A system whose station is the Manufacturers' has none —
  that station is its attack. `World::outposts_of(star, system)` is the
  rule for any system; `settle_outposts`, at the end of `settle_jammer`
  (the start, a jump, the spread and every load), lays each through
  `World::infest`, never over a site with a `Defense` or a held town. So
  an outpost is an Attack site with everything a held station has, and
  the quote of a system never visited asks `outposts_of` (`outpost` in
  `quote_in`).
- **Only attack in a system the machines have**: `site_kind` answers
  Attack for every non-trader site once `infested(star_id)`, a held town
  and a site the flip has not reached yet included (the quote already
  did, off `infested`).
- **A defence pays no money**: `earn_bounty` returns at once where the
  site alongside is a Defend site — nothing paid, nothing pending, no
  `Bounty` said. The people who live through it (and a town's joiners)
  are the reward. *Hazard Pay* (a relic's own pay on a clear) is left.
- **The dials**: the quiet dial (`set_quiet_sites_for_probe(true)`) makes
  `outposts_of` empty and gives back every outpost not yet fought over
  (`give_back_outposts`); `false` lays them again. `land_for_probe` gives
  its town back the same way, so `test_planet`, `defense`, `BIMS_AFIELD`
  and the town defence tests still stand in a friendly town.

**What moved**: `wire::PROTOCOL` 53 (the relay wants redeploying), no
`SAVE_VERSION` (nothing saved changed shape); `REFERENCE_CHECKSUM`,
`SURVIVORS` and the ship's `PINNED`/`PICTURES` where a run meets an
outpost (each note says so). `tests_defense.rs`'s two money assertions
say a defence pays nothing.

## One station and one town a system, and one fight a system (task 135)

> Every section above that walks a system's many stations — the spawn
> system's other orbitals, a trip to "another site here", the trader
> picked among any of them — describes the world **under the tests'
> dial** below. In a run a system offers two sites.

`offered.rs` (a child of `world`, like `outposts.rs`) is the rule.

- **What a system offers**: its **primary** station — the lowest id the
  generator made (`offered::primary`; never a derived jammer or the
  fortress) — or, in the crew's home system, the **home** station
  (`World::offered_station`); the **town** of its lowest landable body
  (`offered::town_body`); its **trader** where it has one; and whatever
  derived jammer or fortress is laid beside them. Every other station and
  settlement is gone: `settle_offered`, the first line of
  `settle_jammer` (the start, a jump, the spread, every load), trims
  `system.stations`, `stations`, `surfaces` and `discovered`; another
  star's system is trimmed the same way (`trim_system`,
  `offered_surfaces`) before `sites_in` lists it and `quote_in` quotes
  it. The bodies stay — a planet with no town is scenery.
- **The trader is never the primary and never at home**
  (`trader_eligible`), so a system with a trader keeps two stations and a
  system whose one station would have been its trader has none — the
  share over the galaxy falls to about one system in twenty (the
  measurement in `tests_trader.rs` says so).
- **The Manufacturers' made-up sites near home are primaries**
  (`manufacturer::near_sites(.., primaries_only)`), so the two a run is
  promised are sites it can go to.
- **The machines' outposts** (task 136) are dealt among the two fights
  (`World::offered_fights`: the offered station unless it is the trader,
  and the town), so a system is one attack and one defence; a system
  whose station is the Manufacturers' has no outpost — that is its
  attack, the town its defence.
- **One fight a system**: `begin_mission` at one of a system's two fights
  records it on `Run::chosen` (saved; hashed only where any, so no pin
  moved for it), and from then on the other is `Refusal::OtherSiteChosen`
  (122) for the rest of the run — the quote, the list (greyed with the
  reason) and a vote. A trader and the Heart are never refused so.
  `World::passed_over(station)` is the system map's word for it.

**The tests' dial**: `World::set_whole_systems_for_probe(true)` (saved,
serde default, not hashed) is task 135 off — every station and town the
generator made, the trader and the Manufacturers' made-up sites picked
among all of them, the outposts' old candidates and no one-fight rule;
turned on after the start it puts back what the start trimmed, keeping
the stations the world holds (the spawn as the hub). `fixture::
open_simulation_world` and `open_crewed_world` set it, so every test
written against whole systems — and `REFERENCE_CHECKSUM` and
`SURVIVORS` — runs on them; `tests_offered.rs` opens its worlds without
it. The ship's sessions (`PINNED`, `PICTURES`) use no dial.

**What moved**: `SAVE_VERSION` 58, `wire::PROTOCOL` 54 (the relay wants
redeploying), `Refusal::OtherSiteChosen` = 122.

## Elites: one system in ten, a Guardian, and the only relics

`crate::elite` is the rule, `offered.rs` the world's side (an elite is
part of what a system offers).

- **Which**: a star's system holds an elite where the galaxy's seed rolls
  it (`data::ELITE_SYSTEM_CHANCE`, ten in a hundred, a salt of its own)
  and it is not the crew's own (`elite::holds`). The elite is the station
  the system offers (`World::elite_station`: the primary of task 135).
  Stateless, never saved; `World::holds_elite`, `is_elite`,
  `is_elite_here`, `elite_stars` (the chart's), `nearest_elite_site`.
  None under the tests' quiet dial.
- **Always an attack**: `settle_elite` (in `settle_jammer`, after the
  Manufacturers, before the outposts) lays it as the machines' through
  `infest` from the first day. Never the Manufacturers' (`manufacturer::
  holds` and `near_sites` skip an elite's system), never an outpost's
  coin (no outposts in an elite's system; under the whole-systems dial
  the elite is taken out of the candidates). `TravelQuote::elite`, and
  the quote's kind is Attack.
- **The fight**: `wave_count_here` is at least `data::ELITE_WAVES` (2),
  bar a count the probes forced; `build_wave` puts Guardians in wave
  `data::ELITE_GUARDIAN_WAVE` (2): at least `data::ELITE_GUARDIANS`
  for the site's tier (`droid_tier`, what the map says) — one at tier
  one, two at tier two, three at tier three (`elite::with_guardian`:
  each the tier's own wave did not bring in the last Trooper's place,
  after the Wardens, never more than the wave's machines) — and those are the only Guardians a run meets, a plain
  wave having none (`wave_kinds(n)`, October 2026). `wire::PROTOCOL` 113,
  121 for the plain waves going without and the Guardian's grenade.
- **Only an elite drops relics**: `relics_on_leaving` offers the reward
  only at an elite, and `infest` rolls a relic cache only there (a site of
  the Manufacturers' has none). Traders still sell theirs.
- **Probes**: `World::set_elite_for_probe(station)` (saved, not hashed)
  makes a site an elite — `tests_relic.rs`'s arena is one — and
  `elite_dock_for_probe` takes the crew to the nearest elite, a mission
  begun (`BIMS_ELITE=1` in the app).
- **The pictures**: a crown and a second ring in magenta
  (`ship::world_paint::{ELITE, paint_crown}`) on the system map, the same
  crown over the star on the galaxy chart (`lobby::preview`'s `ELITE`,
  `Marks::elites`), `ELITE` under the site and "elite · Guardian in wave 2
  · relics" in the list.

`tests_elite.rs` is the rule. **`SAVE_VERSION` 59, `wire::PROTOCOL` 55**.

## A jump is a day, a trip within a system nothing (the map rework)

> "The loop" and "A jump is another system" above say a trip is the
> hyperdrive's charge plus the leg flown, never under a day
> (`MIN_TRAVEL_HOURS`); that is **gone**.

`quote_in` puts the world clock on by `data::JUMP_MINUTES` — one day —
for a trip to another system, however far the site lies from where the
jump lands, and by **nought** for a trip between two sites of one
system. `MIN_TRAVEL_HOURS`, `JUMP_CHARGE_MINUTES`,
`TravelQuote::minimum` and `World::here` went. `CannotTravel` is still
said for a ship whose engines could not stop it anywhere. So the jumps
are the whole of what moves the clock, and the machines grow a step
every `ENEMIES_HOURS / 24` jumps. The one-fight-a-system rule (task 135)
and `AlreadyHere` keep a free trip from being a free fight.

`World::system_look(star)` is any star's system as the map draws it — the
system it offers and a `SiteLook` a site (where it lies, and
`quote_with(.., looking: true)`: the quote with every question about
whether the crew may go there left unasked) — so the map can show a
system across the galaxy with its sites' kinds. Nothing saved or hashed
changed; `REFERENCE_CHECKSUM`, `SURVIVORS` and the ship's `PINNED` move
wherever a run takes an in-system trip or a jump (they were a leg's
length before). `a_jump_costs_a_day_and_a_trip_in_the_system_nothing`
and `every_quote_is_a_day_for_a_jump_and_nothing_in_the_system` are the
rule.

## Twenty for an enemy down, and nothing for its death

> "One speed, and experience alike for every class (task 119)" and the
> engineer's "Experience is given in the step" above say an enemy's death
> is `XP_ENEMY_DEAD` (5) on top of its down; that is **gone** again, as
> feature 109 once had it.

`experience` pays `class::XP_ENEMY_DOWN` — **twenty** now, where it was
ten — once, at an enemy's first down or death, and nothing else: a
machine destroyed is twenty, a Manufacturer downed is twenty, and a
downed one bleeding out or finished afterwards is nothing more.
`XP_ENEMY_DEAD` and `Residents::xp_dead` went (`xp_down` is the one
flag). **`SAVE_VERSION` 62, `wire::PROTOCOL` 61**. `SURVIVORS` moved
(its runs' crew earn experience, and the progress is hashed; taken on a
clean tree with this change alone); `REFERENCE_CHECKSUM` and the ship's
`PINNED` did not.

## A townsperson is picked up with the medkit

A station's person downed — a townsperson or a defender fallen fighting
the machines, never a Manufacturer or a machine — can be revived by the
crew exactly as a crewmate is (the medkit's right-click, or G held
beside it), though its body is in the residents' room and the crew's
hands are in theirs. Three handovers, all in `world.rs`, and nothing
saved or hashed:

- **`revivable_residents`**, in `visit` after the visitors: which of
  the residents' bodies are a Bim, not a Manufacturer, alive and
  downed, on the deck and in nobody's arms, at a station not Hostile —
  told the crew's room as `Game::set_visitors_revivable`. The crew's
  room names one as the patient `bims::game::GUEST + i`
  (`CrewOrder::Revive`), walks to it and kneels as for a crewmate
  (`Room::guests`, `Room::patient_at`), and on the hands coming off
  hands the world a `GuestRevived` (`take_guest_revives`).
- **`tended_residents`**, before the residents' room steps: which of
  them the crew's room has hands on (`revive_share(GUEST + i)`), told as
  `Game::set_tended`, so their countdown stands like a crewmate's
  (task 138).
- **`settle_medics`** brings each round in its own room
  (`Game::bring_round` at the helper's `revived_to`) and says
  `WorldEvent::ResidentRevived { station, who, by }` (140). No relic
  fires for it and it is nobody's experience. The crew's room hears the
  patient is up the next step, with the visitors.

The bots never revive one of their own accord (`revive_on_offer` is the
crew's bodies alone). `a_townsperson_downed_is_picked_up_with_the_medkit`
(`tests_defense.rs`) is the rule; `BIMS_DOWN_RESIDENT=1` downs a town's
guard beside the player's Bim for a look. `wire::PROTOCOL` 60.

## A trip crosses two lanes at most (the second map rework)

> "Jumping along lanes" above says a trip is one hop; since the second map rework it
> is **two lanes at most** (`data::MAX_TRIP_HOPS`).

`World::destinations` lists this system, then the stars a lane off, then
the stars two lanes off (`World::two_lanes_off(galaxy)`, id order), so a
seeded run that takes the first quotable site of a star next door is the
run it was. `World::trip_route(star)` / `trip_route_in(galaxy, star)` is
the way a trip goes — `None` past two lanes (`TooFar`) — and, two lanes
off, goes through the first star between (id order) whose two steps no
jammer shuts (`jammed_step`), else the first: a trip is `Jammed` only
when every way is. `TravelQuote::hops` says how many lanes;
`minutes = JUMP_MINUTES × hops`, and `travel` jumps straight to the far
star — the star between is passed, not visited, and keeps no memory. A
look (`quote_with(.., looking: true)`) at a star further off reads its
arrival day off the plain shortest route. The app passes the chart's own
galaxy to the `_in` forms, since `World::galaxy()` generates one a call.
`tests_jammer::a_trip_wants_two_lanes_or_fewer_out_of_the_star_the_ship_is_at`
and the jammer test's two-lane half are the rule. **`wire::PROTOCOL` 59**
(what a trip may be is what both ends must agree on); nothing saved
changed, and no pin moved.

## A step every five days, and the two tuning files

> "The machines hold a station" above says a wave grows a step every
> `ENEMIES_HOURS`, three weeks; it is **five days** now, and "Attack and
> defence evenly" says a defence pays nothing — it pays like an attack
> now.

Once a jump became a day the three-week step left a crew meeting the
waves of day nought for three weeks of jumps — two machines on day
twelve against a crew of five. `data::ENEMIES_HOURS` is `5 * 24`: a solo
crew meets a wave of five on day twelve, three waves of it.

**Two sets of dials, neither saved nor hashed**, handed over by the app
every frame the world's differ (`crates/app/src/wavecfg.rs`, which
watches both files):

- `droid::WaveScaling` (`World::set_wave_scaling`), the app's
  `scaling.ron` — `step_days` where it was `step_hours`;
- `rewards::Rewards` (`World::set_rewards`), the app's `rewards.ron`:
  `xp_per_down` (read in `experience`), `bounty` by tier (`visit`'s
  machines and `experience`'s Bims; the free `world::bounty_for` is the
  untuned table, for the tests), `defense_bounty_percent`
  (`data::DEFENSE_BOUNTY_PERCENT`, a hundred — the player asked for money
  for every enemy down, so task 136's "a defence pays nothing" is gone
  unless the dial says nought), `bounty_waits_for_clear` (`earn_bounty`:
  false pays at once), `buyback` (`respawn_the_fallen`), `relic_price`
  (`trader_relic_price`), `combine_fee` (`combine`) and
  `shelf_price_percent` (`shelf_price`, before *Trade License*). The app
  reads `World::rewards()` wherever it shows one of those prices.

`Rewards::DEFAULT` is the constants; the committed files are the
constants (`wavecfg`'s test). No `SAVE_VERSION` or `wire::PROTOCOL`: the
dials cross no wire, and in a two-player run each end reads its own files.
`tests_defense.rs`' two money assertions say a defence pays. The ship's
`PINNED` moved for `jammer` alone (its clock is past five days).

## The sentry stands until it is destroyed

> "The engineer's ranked kit" above says the ultimate's sentry is
> removed when `SENTRY_SECONDS` have run (`expire_sentries`,
> `Deployable::expires`, `WorldEvent::SentryDone`); that is **gone**.

The engineer's sentry stands until it is shot to nothing, another is
laid (one at a time, as before), the mission ends (`make_whole`) or the
rooms unjoin. `class::SENTRY_SECONDS`, `World::{sentry_seconds,
sentry_left, expire_sentries}`, `Deployable::expires` (and its checksum
line) and `WorldEvent::SentryDone` (133, left free) went;
`World::sentry_standing(who)` is the app's reading. **Its reach**:
`class::SENTRY_RANGE` — five tiles a rank, from the first — goes on
`sentry_skill`'s `Skill::range`, the tiles the room adds to the
minigun's range. Which way it turns to fire is the room's
(`crates/game/CLAUDE.md`). `SAVE_VERSION` 64, `wire::PROTOCOL` 63; no
survivor pin moved (no seeded run has an R rank).
`the_sentry_is_laid_through_a_hit_stands_until_destroyed_and_is_never_packed_up`
and `the_sentry_s_weapon_rate_health_and_range_are_its_rank_s` are the rule.

## A reinforcement comes looking for the crew

> "The machines hold a station" and "The enemy shoots back" above say
> the machines know only what they have seen; that holds for a **first
> wave** and no longer for the waves after it.

`settle_droids` marks every machine of an **arriving** wave at a held
site (`wave > 1`, the reinforcements of an attack) `Droid::seeking`
(saved, serde default). While any such machine stands in the residents'
room, `Game::set_hostiles` hands `believe` `told = true`: every target
the world names is believed where it stands **now**, seen or not — still
stale out of sight, so it is walked towards (`Tactics::charge`, the
hunter's rule in `plan_droid_stand`) and never fired at. So the wave is
at war from its landing and walks from its airlock to the crew instead
of waiting there. The first wave, a defence's waves (the machines' own
list, `set_machine_hostiles`) and the Heart's machines are untouched.
`a_reinforcement_wave_hunts_the_crew_and_the_first_wave_waits`
(`tests_droid.rs`) is the rule. `SAVE_VERSION` 63, `wire::PROTOCOL` 62
(both carried on by later bumps); no pin moved (`SURVIVORS`, `REFERENCE_CHECKSUM`
and the ship's `PINNED` read the same with the rule on and off).

**And so does one of the Manufacturers'.** A reinforcement at a site of
theirs is their people alone — Bims, no machine to carry `seeking` — so
it stood behind the airlock it came in by ("the attackers just stand
there"), for two reasons: the room was not told, and a body enlisted
while the room was already at war was never mustered (the room's half,
`crates/game/CLAUDE.md`, "A Manufacturer is a hostile Bim"). The world
now says `Game::set_told(residents.manufacturers_laid > 1)` in `visit`'s
hostile branch, right before `set_hostiles` — their garrison is wave
one, and a wave of theirs lands only once the one before is down — which
`set_hostiles` reads beside the machines' `seeking`. The room's flag is
`serde(skip)`, said again every step, so nothing saved or hashed moved.
`a_reinforcement_of_theirs_hunts_the_crew_from_its_airlock` (the room
still at war when it lands) and
`a_reinforcement_of_theirs_is_told_where_the_crew_are` (landing after the
room forgot the crew) in `tests_manufacturer.rs` are the rule.

## A site's waves are its tier's

> "The machines hold a station" above says a site's wave count grows a
> wave every second step of the world clock; that is **gone**.

How many waves a site has — fixed at the crew's first dock, as ever — is
the tier its machines come at (`World::droid_tier`): **one at tier one,
two at tier two, four at tier three** (`data::DROID_TIER_WAVES`,
`droid::wave_count(tier)`). The clock and the players still make each
wave bigger; they no longer make more of them. `droid::WaveScaling`'s
`tier_waves: [u32; 3]` replaced `waves_base` and `steps_per_wave`
(`scaling.ron`: `tier_waves: (1, 2, 4)`), `World::wave_count_at(hours)`
is `wave_count_for(tier)`, and the Heart's preview asks tier three's.
An elite's floor of two (`ELITE_WAVES`), the Manufacturers' one while
they have the machines and the probes' `BIMS_DROID_WAVES` stand over it
as before. `wire::PROTOCOL` 64; nothing saved changed.
`a_site_has_one_wave_at_tier_one_two_at_tier_two_and_four_at_tier_three`
(`tests_droid.rs`) and `droid::tests` are the rule.

## The tank's ranked kit, and the talents gone (task 139)

> "The tank: the wall, the taunt and the hits (feature 77)" above
> describes the ten levels of talents **task 139 replaced**, and "What a
> class's keys have left" its `key_level`, which went with them. Kept as
> history; this is what the tank is now. The soldier's section's
> `RANKED_LEVEL_XP` is `LEVEL_XP`.

- **Every class is ranked, and the talents are gone whole.**
  `class::{Side, Talent}` (codes 0 to 68, never to be reused),
  `pick_at`, `fixed_at`, `is_pick_level`, `talent_of`, `key_level`,
  `levels`, `level_xp`, `Progress::{picks, has, pick, picked_at,
  pending_pick, talents}`, `World::{has_talent, pick_talent}`,
  `Command::PickTalent`, `WorldEvent::TalentPicked` (69) and the pick's
  refusals (48 to 50) went. `LEVEL_XP` is the sixteen-level table that
  was `RANKED_LEVEL_XP` and `LEVELS` is 16; `level_of(xp)`,
  `Progress::{level, to_next}` and `gain(xp)` take no class.
  `class::ranked` is every class but `None`; `Progress::can_rank_up`
  refuses a classless crew member `NoClass` and a slot past R
  `NoRankedKit`.
- **Base traits**: `armour_drain` is `TANK_DRAIN`, times `FORTRESS_DRAIN`
  from Plated's `FORTRESS_RANK`; `give_tank_kit` as before. *Rallying
  wall*, *breacher*, *iron frame* and *plated*'s protection went —
  `Skill::{iron_frame, smash_rate}` with them (the room forces a lock at
  everybody's rate).
- **Q Taunt**: `can_taunt` — `NotATank`, `OutOfReach` (unfit or downed),
  `NotLearnt`, `CoolingDown` — and `taunt` notes `Tank::last_taunt` (the
  cooldown's start, moved by *Kill Relay*) and `Tank::taunt`, a
  `tank::Window { began, until }` that never moves.
  `taunt_radius`/`taunt_seconds`/`taunt_cooldown` read `TAUNT_RADIUS`,
  `TAUNT_SECONDS`, `TAUNT_COOLDOWN` by rank; `taunt_left` is **seconds**
  now.
- **C Plated**: `tank_skill` multiplies `Skill::damage_taken` by
  `PLATED_DAMAGE_TAKEN` — the room takes it off the hit before the armour
  (`strike_stripping`), with Rally's (`lift_by_commanders`) and a
  Juggernaut's.
- **E Bulwark**: `can_bulwark` wants a rank (`NotLearnt`);
  `bulwark_reach`/`bulwark_pace` by rank, `GUARDED_DODGE` from
  `GUARDED_RANK`, the wall's `interpose` from `INTERPOSE_RANK`.
- **R Juggernaut**: `Command::Juggernaut { slot }` → `WorldEvent::
  Juggernaut` (141); `can_juggernaut` — `NotATank`, `OutOfReach`,
  `NotLearnt`, `AlreadyActive`, `CoolingDown` — and `juggernaut` notes
  `Tank::last_juggernaut` and `Tank::juggernaut`. While it runs
  `tank_skill` multiplies `damage_taken` by `JUGGERNAUT_DAMAGE_TAKEN`;
  the pace is his own, or the wall's.
- **What the enemy is forced to**: `taunts_for_the_enemy` is a
  `bims::combat::Taunt { radius, magnet, order }` a crew member — the
  Taunt's radius in room units, `f32::INFINITY` for a Juggernaut, the
  magnet from `TAUNT_MAGNET_RANK`, and `order` the recency
  (`Tank::forcing_since`: one more than every forcing that began before).
  `visit` hands it to the residents' room (`set_hostiles_taunting`) and,
  in a defence, to the machines' own list too
  (`set_machine_hostiles_taunting`), which never had the taunt before.
  The room's rule is now **him and nobody else** (`crates/game/CLAUDE.md`).
  A cloaked tank is `None` on the enemy's list, so it forces nothing.
- **Timers**: mission clock; `make_whole` resets every `Tank`;
  `cooldowns_less` moves `last_taunt` and `last_juggernaut`, never a
  window.
- **Checksum**: a nought where `progress.picks` was counted, and the
  windows and `last_juggernaut` only where one was ever set — so
  `REFERENCE_CHECKSUM`, `SURVIVORS` (still the sweep's
  `0x_c91e_9026_9f7f_3d90`) and the ship's `PINNED` did not move.

**What moved.** `SAVE_VERSION` 65, `wire::PROTOCOL` 65. `tests_tank.rs`
is the task's tests, `class::tests` the tables and the rank gates of
every class.

## The commander's squad orders removed

> "The commander: the aura, the squad and the rally (feature 78)", "Every
> player has two orders for the bots" (the squad's handed first) and the
> commander's ranked kit's "base traits" above describe what this
> removed.

The commander's squad orders — attack (B), fall back (T), stand ground
(Z) — are gone: `SquadAsk`, `SquadKind`, `SquadOrder`, `World::squad`,
`Command::Squad`, `can_squad`, `squad_members`, `squad_range`,
`class::SQUAD_RANGE`, `Ability::SquadOrder`, `WorldEvent::Squadded` (81)
and the refusals `NoSquadInRange` (72) and `NoEnemyThere` (73), their
codes left free. His one base trait is the cheaper hire (`hire_fee` asks
`is_commander`). `size_the_commanders`, where the squad's hand-off was,
keeps `World::commanders` as long as the crew, since the checksum eats
its length; the checksum eats a nought where the squad order was, so
`REFERENCE_CHECKSUM`, `SURVIVORS` and the ship's `PINNED` did not move.
The everybody's standing orders (the X banner, the retreat) are
untouched. **`SAVE_VERSION` 66, `wire::PROTOCOL` 66.**

## `World::would_travel`: the trip asked about before it is taken

`World::would_travel(&command)` (in `mission.rs`) answers whether a
`Propose`, an `Accept { yes: true }` or a `PlayerGone`, applied now,
would carry the vote and take the crew on the trip — `go_if_carried`'s
question, changing nothing. A trip builds the site it lands at (two
seconds for the run's first station, eight or more for a planet's town,
release build), so the app asks first and applies such a command on a
thread behind a loading screen (`crates/app/src/screens/loading.rs`);
the world itself is untouched and nothing saved, hashed or on the wire
moved. `would_travel_foretells_the_trip` (`tests_mission.rs`) is the rule.

## The relic rebalance (task 142)

> "Five patches of relics (task 118)" above says *Wide Angle Optics*
> narrows a machine's front, *Scrap Collector* pays for kills, *Lifeline*
> surges a crewmate going down, *Cover Formation* covers bots alone and
> *Squad Morale* counts its holder's and the bots' kills: this is what
> each is now. The numbers are `data.rs`'s, their notes say what they were.

- **Field Radio** 7% within 5 tiles; **Crippler's Mark** +35%; **Tether
  Field** 40% for 10 s — constants only.
- **Cover Formation**: `Effect::Aura`'s flag is `own` now (the holder's
  own too), not `bots_only`; `aura_percent` counts every holder fit to act
  — the holder itself where `own`, everybody else within its tiles. 15%
  within 5 tiles.
- **Squad Morale**: `Trigger::CrewKill` is said to every player for
  every machine destroyed, whoever hit it last (`relics_on_a_kill`).
- **Wide Angle Optics** is `Rule::StillRange { tiles }`: 7 tiles on
  `Skill::still_range`, which the room adds to the range while the Bim
  stands still (`Game::shot_skill`). A machine's front is the Guardian's
  for everybody again, and `hand_the_room_the_shield_fronts` hands an
  empty list.
- **Strong Will** took *Scrap Collector*'s code (31):
  `Rule::LongerAbilities { percent }`, read by
  `World::ability_length_factor` in the seven ability lengths —
  `rampage_seconds`, `emp_stun`, `cloak_seconds`, `taunt_seconds`,
  `juggernaut_seconds`, `rally_seconds`, `battle_cry_seconds`. `KillPay`
  and `SCRAP_COLLECTOR_PAY` went; `relics_on_a_kill` pays nothing.
- **Lifeline**: `Trigger::PlayerLow` (was `CrewmateDowned`) and
  `Action::Shield { tiles, hp, seconds }` (was `Shelter`).
  `downs_before_the_step` keeps each body's health share beside its down,
  and `settle_relic_downs` says the trigger to every player for a
  player's Bim (never a bot) that crossed `LIFELINE_BELOW_PERCENT` this
  step; `relic_shield` shields the holder and that Bim (the holder alone
  when it was itself) with `LIFELINE_SHIELD_HP` for `LIFELINE_SECONDS`,
  within `LIFELINE_TILES`, once a mission. The shield is the room's
  (`Game::set_shield`, `crates/game/CLAUDE.md`) and in `world_checksum`
  only where one stands.
- **Hazard Pay** and **Trade License** ride on the per-player wallets:
  the holder is `credit`ed `HAZARD_PAY / players` a clear
  (`relics_pay_the_clear`), and a trader asks every player
  `TRADE_LICENSE_PERCENT` less while anybody holds it and the holder
  `TRADE_LICENSE_HOLDER_PERCENT` less (`trader_discount_for(slot,
  price)`): `shelf_price(slot, item)` and `trader_relic_price(slot,
  relic)` take the buyer, and the relic's price is the players' share
  (`trader_share`) like the shelf's.

`tests_relic_patches.rs` has a test a relic.

## Every player's own money (no task number)

> "Money, the bounty and what a crew are worth", "The trader" and the
> buyback in "Nothing is stored" above speak of **the pool**; since this
> change it is split.

- **`World::wallets`** (saved, hashed) is a player slot's own money, and
  `World::money` is only the **takings** not yet shared out: a bounty and
  anything else earned goes there, and `share_out` (mission.rs) divides it
  evenly into every wallet at `World::start` and at every mission's end
  (`leave_mission`, before the buyback), what does not divide left for
  the next. `wallet(slot)`, `credit(slot, amount)`, `pay_from(slot,
  price)` (all or nothing), `share_of(slot)` (the wallet and its share of
  the takings — what the screens show), `crew_money()` (everything, what
  a build site draws on through `spend_shared`) and `set_money_for_probe`
  (the tests' dial: the amount shared out as the opening shares it).
- **A trader a player** (`Trader::owner`): `arrive_at_trader` meets one
  for every player at a trader site, each its own shelf
  (`trader::roll_shelf_for`, the first player's `roll_shelf`) and its own
  relic (drawn one at a time, so no two alike); `trader_here(slot)`. A
  buy, a combine and the relic are paid out of the buyer's wallet; the
  relic is **bought outright** for the buyer's own Bim — `ProposeRelic` at
  a trader is `buy_trader_relic`, and the trader-relic vote
  (`Run::trade_relic`) went. Prices are the crew's share:
  `trader_share(price)` is the price over the players, rounded up, on
  `shelf_price`, `trader_relic_price` and `combine_fee()`. *Restock
  Codes* rolls the presser's own shelf.
- **The buyback** (`World::buy_back`): the fallen's own wallet when it
  holds `Rewards::buyback`; otherwise every wallet pooled, the buyback
  (or what there is) taken, and the rest handed back to the **others** in
  proportion to what each put in, the fallen's spent, the rounding into
  the takings. `a_buyback_the_fallen_cannot_pay_pools_every_player_s_money`
  is the user's own example.
- A hire and its wages are the signer's (`Hired::by`); *War Chest* reads
  the holder's wallet; *Hazard Pay* is `credit`ed to the holder.
- **An enemy's worth by its strength**: `droid_bounty_percent` (a Husk
  90, a Trooper 100, a Warden or a Guardian 110) and
  `manufacturer_bounty_percent` (a pistol and nothing worn 90, a better
  gun or armour a tenth more each), `data::BOUNTY_SPREAD_PERCENT`, through
  `bounty_share`. `WorldEvent::EnemyRewarded { station, who, xp, money }`
  (142) says each enemy's pay the step it is counted, and
  `WorldEvent::Hit { resident, who, damage, crit }` (143) every hit
  landed on either side — pictures' events, for the numbers the app
  floats (`World::shown_hits` carries the relics' hits on the machines
  out of `land_on_machines`).

**`SAVE_VERSION` 68, `wire::PROTOCOL` 69**; `REFERENCE_CHECKSUM`,
`SURVIVORS` and the ship's `PINNED` re-pinned (each note says why).

## The `end` command's classed bots (no task number)

> **Since the conduit waves** the bots are plain classless Bims:
> `World::end_crew_for_probe(tier)` in place of `classed_crew_for_probe`
> — a gun of its own dealt down them (`WeaponKind::ALL`), no class, no
> level, no ranks, no Reinforcements, everybody's kit at `tier`; the
> players at the top level with every point to spend.
> `tests_heart::the_end_command_s_bots_are_plain_bims_in_tier_three_kit`.
> What follows is the history.

`World::classed_crew_for_probe(tier)` is the app's `end` command's crew:
`classes` made **as long as the crew**, every crew member past the
players dealt a class (`Class::ALL`'s five in turn — ten bots are two of
each), its kit (`change_class_kit`, `set_class`'s swap), the top level
and every rank (`set_ranks_for_probe`); the players' own Bims the top
level with every point to spend; everybody's kit at `tier`; and the
commanders' Reinforcements. `class_of` reads `classes` by crew index, so
that is all a bot needs to be classed — its passives, charges, outfit
and experience follow. Nothing else ever makes `classes` longer than the
players, so the two places that now walk it rather than the players —
`bring_reinforcements` (every classed commander) and `drop_crew_member`
(a dropped bot's class, never a player's) — do what they did in every
other run, and no pin moved. **Bots never use an ability**: nothing in
the room casts one for a bot, so a classed bot has its passives (the
tank's Plated, the commander's aura, the soldier's Weak Spot, the
engineer's charges held and unused…) and not its keys. The app's
`Session::end_for_probe` docks at the Heart first
(`heart_dock_for_probe`); `tests_heart::the_end_command_s_bots_are_every_class_at_the_top_in_tier_three_kit`
is the rule. `wire::PROTOCOL` 70 (the lobby's settings carry `end`, and
the relay's `ClientCtl::CreateAt` opens a room at a code asked for).
`Session::end_for_probe(day)` also puts the world clock at `day`
(`set_day_for_probe`; the app's `dev::end_day`, sixty unless
`BIMS_END_DAY`, dealt to guests in `SettingsWire::end_day`) and sets
`World::first_mission_uneased` (`set_first_mission_uneased_for_probe`:
saved, not hashed), which `droid_wave_size` asks before taking
`first_mission_ease` off — the Heart being that run's first mission.
`tests_heart::the_end_command_s_waves_are_day_sixty_s_and_not_eased`.
**`SAVE_VERSION` 69, `wire::PROTOCOL` 71.**

## The commander's Reinforcements are pressed (no task number)

> "The commander's ranked kit (task 129)" above says R brings his Bims
> at every mission's start; that is **gone**.

R is `Command::Reinforce { slot }` (`class::Ability::Reinforce`):
`World::can_reinforce` refuses `NotACommander`, `OutOfReach` (not in a
mission, not fit to act, downed), `NotLearnt`, `CoolingDown` within
`World::reinforcement_cooldown` — `class::REINFORCEMENT_COOLDOWN` (140 s
of the mission clock) at every rank, times the cooldown relics — and
`CantDeployThere` with no free deck round him. A call (`reinforce` →
`bring_reinforcements_of`) stands the rank's `REINFORCEMENTS` beside him
as before and notes `Commander::last_reinforcement` (saved; hashed only
where set, so no pin moved); those called before stay, each still for
the mission alone. `make_whole` makes it ready at every mission's start,
*Kill Relay* moves its start back. `reinforce_for_probe` is every
commander's call with no cooldown asked, for the tests. The app's R box
shows the cooldown and the count standing. `SAVE_VERSION` 73,
`wire::PROTOCOL` 75. `reinforcements_are_on_a_hundred_and_forty_second_cooldown`
and `reinforcements_are_called_in_with_r_by_rank` are the rule.

## The commander's C is the Medivac (no task number)

> "The commander's ranked kit (task 129)" above says C is a Command Aura
> (`aura_radius`, `aura_reaching`, `commander::Aura`, `AURA_*`); that is
> **gone**.

C is `Command::Medivac { slot }` (`class::Ability::Medivac`):
`World::can_medivac` refuses as `can_reinforce` does (`NotACommander`,
`OutOfReach`, `NotLearnt` — a rank of C —, `CoolingDown`,
`CantDeployThere`), on `class::MEDIVAC_COOLDOWN` (140/130/120/110 s by
rank, times the cooldown relics; `Commander::last_medivac`, saved, hashed
only where set, moved back by *Kill Relay*, forgotten at every mission's
start). A call (`medivac`) stands **one** classless Bim beside him through
the reinforcements' own door (`enlist_republic`,
`size_for_the_reinforcements`), marked `Reinforcement::medic` (saved,
hashed only where true): the pistol, and `MEDIVAC_VEST` — none, a tier-one
vest, a tier-three vest, and from `MEDIVAC_FULL_ARMOUR_RANK` (4) helm, vest
and leg guards at tier three — pieces off the holdings.
`WorldEvent::Medivac { who, medic }` (144). Being a reinforcement it is
gone when it dies and at the mission's end, and nobody's kit to change
(`may_change`, `NotYours`). `reinforcements_of` is the R's soldiers alone,
`medivacs_of` the medics; `is_medivac` makes it a medic to `skill_of`
(`Skill::medic`, the medic's revive time) and to the room
(`Game::set_medivac`, every step in `hand_the_room_the_field_medics`),
which runs it to a downed player (`crates/game/CLAUDE.md`). Drawn
`Outfit::RepublicMedic` (`set_republic`'s third argument).
`SAVE_VERSION` 75, `wire::PROTOCOL` 77. No pin moved (`SURVIVORS`,
`REFERENCE_CHECKSUM`, the ship's). `the_medivac_calls_a_medic_in_armoured_by_rank`,
`the_medivac_is_on_its_rank_s_cooldown` and
`a_medivac_medic_s_kit_is_nobody_s_to_change` are the rule.

## A relic for every player off an elite, a clash by the dice (task 146)

> "The vote" in "Relics, and research gone (feature 106)" above is the
> **cache's** alone since task 146; the reward has none.

- **The offer** (`offer_reward`) is `data::RELIC_OFFER` or one more than
  the players, whichever is more — three players see four — so the last
  to pick still has a choice.
- **Every player picks its own** (`pick_reward_relic`, off
  `Command::ProposeRelic` while the choice is a `Source::Reward`; `to` is
  not read and there is no taking none): a relic still to be won
  (`RelicChoice::left`, `NotOnOffer` otherwise), by a player that has won
  none off it (`NoRelicChoice`), a player's slot (`NotAPlayer`). It
  replaces that player's last pick (`RelicChoice::picks`,
  `WorldEvent::RelicPicked` 145).
- **The round settles** (`settle_reward_picks`) once every connected
  player still without a relic off it has picked — also on a
  `PlayerGone`, so a player gone is never waited for. A relic one player
  picked is given at once (`RelicGiven`, out of the pool). A relic picked
  by more goes by `relic::dice`: each throws two dice in slot order, the
  highest sum wins, those tied for the top throw again after the rest —
  off a stream of its own (the galaxy, `Relics::offers`, the round and
  the relic), each throw said as `WorldEvent::RelicDice { slot, relic, a,
  b }` (146) in the order thrown. The winners go on `RelicChoice::won`;
  the picks are cleared and `round` counts on; who lost picks again out
  of what is left. Once every connected player has won one, or nothing
  is left, the choice is gone and the map up.
- **The app** plays the throws back (`screens::dice`): two dice a throw,
  two seconds each, one after another, then the clash's winner; the
  log's lines for the throws and the relics given wait for it, and the
  reward window (`worldmap::reward_window`) stands aside meanwhile.
  `BIMS_DICE=1` stages a clash to look at.

`picks`, `won` and `round` are saved (serde default) and hashed with the
choice. `tests_relic.rs`'s
`every_player_picks_its_own_and_a_clash_goes_by_the_dice`,
`a_reward_does_not_wait_on_a_player_gone` and
`a_reward_offers_one_more_than_the_players`, and `relic::tests`'
`the_dice_go_in_turn_and_a_tie_at_the_top_throws_again` are the rule.
**`SAVE_VERSION` 76, `wire::PROTOCOL` 78.**

## One mission a system, and the map is the galaxy chart (the galaxy-only map)

> "One station and one town a system, and one fight a system (task 135)"
> above offers a station **and** a town, one of them the machines'; that
> is **cut to one**. The rule is still `offered.rs`.

- **What a system offers**: `World::offered_fight(star, system)` — its
  station (the primary, home in the crew's own) **or** its town (the
  lowest landable body's), a coin off the galaxy's seed and the star
  (`offered::town_fight`, its own salt) choosing, the station wherever
  there is no town and always at home, at an elite (`elite::holds`,
  stateless — never the quiet dial) and where the Manufacturers hold it.
  Attack or defence is the outposts' coin over that one candidate (`held`
  unchanged: one candidate, a coin; home's is a defence).
- **A trader's system offers the trader alone**: `trader_eligible` now
  takes the system's **primary** (never at home, never an elite's), and
  `offered_fight` is `None` where `trader_of` is something. About one
  system in ten, `TRADER_SYSTEM_CHANCE`; the near-home made-up trader is
  a whole system next door.
- **Every rule answers the same trimmed or whole** — the primary is kept
  wherever it is the answer, the town's body stays — so `trim_system`
  twice changes nothing (`tests_offered.rs` checks every star).
- **No exceptions: one site a system, always** (the user's follow-up).
  `World::mission_site(star, system)` is it — the Heart's fortress at the
  machines' origin (`offered_fight` is `None` there, no trader and no
  elite either), else the mission, else the trader. **The jammer is that
  site** (`World::jammer_site_of`; `jammer_station` asks it): a town, a
  trader or the Heart can be it, an attack whatever it was, and no
  derived jammer is laid in a run (`settle_derived_jammer` and `sites_of`
  only under the whole-systems dial, which keeps the old lowest-orbital
  rule). **A fallen trader is fought for** (`World::traders_fall`, true
  outside the dial): `infest` and `spread_crisis` take it, the quote no
  longer refuses it `TraderClosed`/`ClosedOnArrival` but reads it Attack
  (`infested`, `jammer`), `site_kind` says Attack until it is cleared,
  `travel` begins a mission there (the visit is for `kind == Trader`),
  and `leave_mission` with it cleared calls `arrive_at_trader` — the
  purchase order is up the moment the crew are back aboard. The app's
  `WorldMap::star_site` is the star's one listed site.
- **Home offers its station alone**, so `land_for_probe` (and
  `reseed_ground_for_probe`) lay the home system's town back first
  (`lay_ground_for_probe`) for `test_planet`, `defense`, `BIMS_AFIELD`.
- **The chart's marks**: `World::star_missions(galaxy)` — every star's
  one site with its quote's kind and `cleared` (`run::StarMission`), each
  system generated once (`quote_given` takes it; `sites_of` is
  `sites_in`'s half over a system in hand). 600 stars in ~11 ms. The app
  asks it with the list (`WorldMap::refresh`, on its key) and the chart
  draws crossed blades or a shield at each star's left shoulder
  (`lobby::preview::Mission`, `Marks::missions`). The system view is gone
  from the app's map; the ship's system painter is untouched and unused.
- `Run::chosen` / `OtherSiteChosen` stay, and refuse nothing while a
  system offers one fight.

No saved shape changed; **`wire::PROTOCOL` 80**, **81** with the
follow-up (what both ends generate a system to). The whole-systems dial is untouched, so `REFERENCE_CHECKSUM`
and `SURVIVORS` (on it) do not move for this; the ship's `PINNED` and
`PICTURES` (no dial) do. `tests_offered.rs` is the rule;
`tests_run::no_human_is_ever_hostile_in_a_generated_galaxy` opens whole
systems now.

## One armour slot (October 2026)

> "A piece of armour is a resource in the hold", "The arc greaves and the
> Reflective plate (task 116)" and every line above about a helm, kevlar,
> leg guards or a hit's part describe what this change took away. The
> room's half is `crates/game/CLAUDE.md` ("One armour").

- **`ResourceId::Armour = 7`** (the kevlar's code) is the one armour
  resource; 6, 8, 20 and 21 are free (`ResourceId::ALL` is fifteen,
  `CODES` still 22). Booked at 4 500 (a hundred a point of its 45),
  52 mass, a 4×4 footprint; `worldgen::data::ARMOUR` is one long, so a
  trader's three armour slots are the armour at three tiers drawn from
  three candidates.
- **`GearSlot` is `Weapon = 0` and `Armour = 1`**; `of_part`/`part` went.
  The Tab panel shows those two a Bim.
- `WorldEvent::CrewHit { who }` carries no part (value: the crew
  member). `mercenary::ARMOUR_FEE` is one row (5 000, the three pieces'
  together). The tank's start, `outfit_for_probe`, the Outfitter, the
  medic's Medivac (`MEDIVAC_VEST`'s tier; `MEDIVAC_FULL_ARMOUR_RANK` went)
  and a Manufacturer's kit put the one armour on.
- **What moved**: `SAVE_VERSION` 79, `wire::PROTOCOL` 82;
  `shipdesign::fixture::PLAYTEST_HASH` (the playtest cargo's three pieces
  are one), `worldgen`'s `REFERENCE_CHECKSUMS` (four leans shorter, no
  bump), and every survivor pin a fight with armour in it reads.

## The base is guaranteed, and the machines are never outnumbered (October 2026)

> "The machines hold a station" above says a defence counts its defenders
> as players and a wave is the formula's alone; both moved.

`World::droid_wave_size` (through `wave_size_with`) is now, unforced:

- `WaveScaling::size_eased(players, hours, ease)` — the formula less the
  early ease and (`ease`) the first mission's, **never under `base`**:
  the base is what every wave is sure of, whatever the eases take off
  (`size` is the same with no extra ease);
- **plus one machine a defender** the site fielded, at a defence (they
  were that many more *players*, so `per_player` each);
- **never fewer than `World::bims_fighting`**: every crew member alive
  (downed too; bots, hands and joiners all) but a commander's
  reinforcements, plus a defence's defenders still alive. Only the
  commander's R and C can tip the count — nothing else adds a crew Bim
  mid-mission. `wave_size_at` (the Heart's preview) takes the same floor
  off the crew as it stands.

A forced wave (`BIMS_DROID_WAVE`, `droid_kinds_forced`) is as forced.
No `SAVE_VERSION` or `wire::PROTOCOL`: nothing saved or carried changed
shape, and both ends work the size out alike. **The survivor pins
(`SURVIVORS`, the ship's `PINNED`) and `REFERENCE_CHECKSUM` move** where
a run's crew outnumber the formula or a town's joiners differ from two —
not re-run here. `no_ease_takes_a_wave_under_its_base`,
`a_wave_is_never_fewer_than_the_bims_it_meets` (`tests_droid.rs`) and
`the_wave_at_a_defence_is_the_wave_with_a_machine_for_each_defender`
(`tests_defense.rs`) are the rule.

## How the enemies scale: the run day and eight dials (task 147)

> "A step every five days", "A site's waves are its tier's", "The base is
> guaranteed, and the machines are never outnumbered", the tier by
> distance in "Jumping along lanes" and "Relics" (tier two on time and
> distance), the first mission's ease and the Manufacturers' kit by the
> day in "The Manufacturers" describe what **task 147 deleted**. This is
> the whole of how the enemies scale now.

`droid::WaveScaling` (`scaling.ron`; the setup's Difficulty, which is
the same type — `pub type Difficulty = WaveScaling` — stands **in
place of** it for a run) is eight dials, every one read off
**`World::run_day`** (`days_gone + 1`, the day the top bar shows):

- **a wave** (`size(players, bots, day)`): `(enemies_per_player +
  day_scaling × ⌊day / scaling_days⌋) × players`, plus at a defence
  `⌈enemies_per_defender × defenders fielded⌉` (an `f32`, worked in
  hundredths) — renamed `enemies_per_bot` since (the old name still
  reads, SAVE 84 / PROTOCOL 87): every **crew bot** alive, downed too
  (bots, hired hands, joiners; never a commander's reinforcements —
  `World::crew_bots`), plus at a defence the defenders fielded, so a
  site attacked with two bots is two machines more. No base, no first-mission or early ease, no floor at the
  Bims fighting (`bims_fighting` went). `scaling_days` nought never grows.
- **a site's waves** (`waves(day)`): `1 + ⌊day / wave_days⌋` (nought: one),
  fixed at the first dock as ever; the Machine Heart's one
  that lays nothing (its Guardians come for its conduits), an elite at least `ELITE_WAVES`, the
  Manufacturers' garrison one while they have the machines, the probes'
  dial over all.
- **each enemy's tier**, in whole enemies, rounded down
  (`share_of`): tier two at the least for `n × day / tier2_days` of a
  wave, tier three for `n × day / tier3_days` (counted first), all of
  them from that day; a timing of nought is the tier from day one.
  `machine_tiers(n, day)` deals the machines in wave order (the first
  tier three, then two); `World::machine_tiers` adds the Heart (all
  three) and `BIMS_DROID_TIER` (all at it). `build_wave` builds each
  machine at its own and asks `wave_kinds` at the wave's highest (so a
  Guardian comes with the first tier-three machine of a wave of four or
  more). **The Manufacturers' gear** (`gear_tiers`,
  `World::manufacturer_gear_tiers`) is the same shares plus
  `tier1_days`: `None` — the laser pistol and no armour — for the share
  not yet geared, a gun (never the schword) and armour at the tier for
  the rest (`manufacturer::gear(tier, seed, ids)`); the pistol-day and
  armour-day steps went. Their Troopers take the machines' tiers.
  **A site's defenders take them too** (October 2026):
  `World::defender_tiers(n)` is `machine_tiers(n)` — the same shares, the
  Heart's and the probes' dial included — and `Residents::open` takes a
  tier a defender (`&[Tier]`, where it took a count), arming each with
  `crew::defender_gear`: the hired hand's kit off its seed, its gun and
  any armour it wears lifted to the tier, never lowered (a minigun stays
  two), and at tier one the kit exactly. The tiers are
  read when the room opens; a running defence keeps its room.
  `the_defenders_are_armed_at_the_day_s_tier` (`tests_defense.rs`);
  `wire::PROTOCOL` 93.
- **a site's tier** said on the map, the chart and in the checksum is
  `usual_tier(day)`, the tier at least half come at (`World::droid_tier`,
  `tier_on(clock)`, `system_tiers` — the same at every star); the
  Heart's is three. The distance rule (`site_tier`,
  `DROID_TIER_THREE_HOPS`, `ENEMY_TIER2_*`, `relic::tier_two_rolled`)
  and `manufacturer_tier`/`gear_tier` went.

Defaults (`data`): `ENEMIES_PER_PLAYER` 2, `DAY_SCALING` 1,
`SCALING_DAYS` 5, `ENEMIES_PER_DEFENDER` 1.0, `WAVE_DAYS` 10,
`TIER1_DAYS` 5, `TIER2_DAYS` 20, `TIER3_DAYS` 40. `DROID_WAVE_BASE`,
`FIRST_MISSION_WAVE_EASE`, `DROID_TIER_WAVES`, `ENEMIES_HOURS`,
`MANUFACTURER_{ANY_GUN,ARMOUR}_DAY`, `wave_size`, `wave_count`,
`time_steps`, `World::{wave_count_for, bims_fighting,
set_first_mission_uneased_for_probe}` and the saved
`first_mission_uneased` went; `World::heart_wave_count` was the Heart's
preview (gone with its waves, October 2026). **`SAVE_VERSION` 83, `wire::PROTOCOL` 86** (the relay wants
redeploying). `SURVIVORS`, `REFERENCE_CHECKSUM` and the ship's `PINNED`
move (every wave's size and tiers did) — not re-pinned. `droid::tests`,
`tests_droid.rs` (`a_wave_is_the_players_and_the_day_and_nothing_else`,
`a_site_has_a_wave_more_every_wave_days`,
`a_wave_s_machines_come_at_the_tiers_the_day_deals`, …),
`tests_defense.rs`' per-defender test and `tests_jammer.rs`'
`the_machines_come_at_the_day_s_tier_however_near_their_origin` are the
rule.

## Items: four slots a player's Bim (October 2026, step one)

> "The trader (task 114)" above says the shelf is four weapons and three
> pieces rolled once a run and never restocked; it is **one weapon and
> one armour at the day's tier, rolled every visit** now.

Dota 2's items. The rules no room needs are `crate::items` (the price,
`shop_tier`, `shop`, `combined`, `ItemClocks`); what they do in a mission
is `item_use.rs` (a child of `world`, like `relics.rs`); the numbers are
`bims::module` (`crates/game/CLAUDE.md`, "Items").

- **Where they live**: `Gear::items` (four `Option<Module>`), so they ride
  every loadout path there is — `GearSlot::Item1..Item4` (codes 2–5,
  `GearSlot::ITEMS`, `of_item` gives `Item1` for "an item slot",
  `takes(item)` any of the four), the armory (`Item::Module`), `Equip`
  (the first free slot, `ItemsFull`), `Command::EquipAt` (a slot named;
  between a Bim's own two slots a swap), `Unequip`, `Offer`, `Combine`
  (`trader::combined` → `items::combined`; the *Override Core* never).
  **A bot carries none**: `item_slot_for` refuses `BotsCarryNoItems`.
  Hashed only where one is carried (`eat_gear`), so no pin moved for an
  empty loadout.
- **The trader**: `World::shop_tier` (tier one, then the scaling's
  `tier2_days`, `tier3_days` off `run_day`) is the tier of the shelf's one
  gun and one piece (`trader::roll_shelf(.., owner, visit, tier)`,
  `Trader::restock` at every `arrive_at_trader`, the visit the world
  clock's minute) and of the item shelf (`item_shelf`: every kind,
  one of each a visit: `Trader::items_sold`, cleared by `restock`, a
  second `SoldOut`, `World::item_sold`). `Command::BuyItem { kind, to }` (`buy_item`):
  `NotForSale`, the player's own Bim's first free slot or the armory,
  `item_price` through the dials, *Trade License* and the players'
  share. `WorldEvent::ItemBought` (148).
  **A second of a kind combines at once**: a `BuyItem` or a `BuyShelf`
  of a thing the player already has at that tier under three
  (`World::buy_partner`: on the Bim it is for, then the others it may
  change, then the armory) pays the price **and** the combine fee and
  makes the next tier where the first one is (`combine_bought`,
  `WorldEvent::Combined` beside the buy's event) — it wants no free item
  slot. The trader's window reads `buy_partner` to draw the line
  outlined in the next tier with *Buy & Combine*. `wire::PROTOCOL` 102;
  `a_second_of_a_kind_bought_is_combined_with_the_first`.
- **Blink Drive**: `Command::UseItem { item, x, y }` (room units) —
  `can_use_item`: `NotAPlayer`, `NoSuchItem` (empty or passive),
  `OutOfReach` (not in a mission or not fit), `CoolingDown`,
  `BlinkLocked` within `BLINK_HIT_LOCK_SECONDS` of a hit — then
  `Game::blink_spot` (the room's: the point or as far towards it as the
  reach goes, stepped back half a tile at a time to free ground the body
  `sees_from` and can reach) and `Game::blink`; `NowhereToBlink`
  otherwise. `Run::items` (`ItemClocks`, saved, not hashed — the
  positions are) keeps `ready_at` and `hurt_at`, cleared at every
  mission's start. `WorldEvent::Blinked` (147).
- **Executioner**: `World::crit_of(who)` is the soldier's Weak Spot and
  every Executioner carried as one (`bims::module::combine_crits`: either
  comes up, the biggest multiple counts) — `skill_of` hands its chance to
  the room, `crit_extra` takes its multiple. No new draw on the crit
  stream: a body rolls once a hit, as ever.
- **Reactor Heart**: the bar is the room's (`Gear::max_health`); the
  regeneration is `settle_items` after `relics_mend` — `hurt_at` off
  `hits_before_the_step`, then the plain or the quiet rate through
  `heal_crew` (a medic's aura lifts it). Nothing runs for a crew carrying
  no item.
- **Override Core**: `rank_of(who, SLOT_R)` is `class::ultimate_rank`
  (bought + 1 with a Core and a rank bought, up to `OVERRIDE_RANK`, five);
  `bought_rank_of` is the points' own. Every ultimate's table is five
  long (`by_rank` is generic over the length); the fifth row's twists are
  `SENTRY_STANDING` (two), the Rampage refilling the grenades,
  `CLOAK_SPREAD_TILES`, `Skill::unyielding` on a Juggernaut (the room
  keeps a hit point), `REINFORCEMENT_VEST`.

`tests_items.rs` is the rule; `tests_trader.rs`' revisit test says the
shelf is rolled again. **`SAVE_VERSION` 85, `wire::PROTOCOL` 88.**
`REFERENCE_CHECKSUM` and `SURVIVORS` were not re-pinned (they fail in this
tree with another change's march speed in it; a world with no item and no
trader visit hashes what it did). Step two (the user's): more items, and
*Coolant Loop*, *Pressure Seal* and *Steady Grip* moved from relics into
item form.

## Items, step two: nine more, three of them relics once (October 2026)

`bims::module::ModuleKind` is thirteen long (codes 4–12 new); every
number is `bims::module`'s, every price `data::ITEM_PRICE` (×4.5 the
first placeholders, then ×3 again in October 2026: ×13.5).

- **Three relics moved into items**: *Coolant Loop*, *Pressure Seal* and
  *Steady Grip* are `Relic::retired()` — kept in `Relic::ALL` (a relic's
  code is its place there) with their effects, so an old save holding one
  keeps it, but out of `starting_pool`, `Profile::pool`, `record_run`'s
  unlocks and `set_relic_pool` (`Relic::in_play`). As items: the cooldown
  cut is folded into `relic_percent(Stat::Cooldowns)` (so every class
  cooldown honours it, several adding to `COOLDOWN_CUT_MOST`), the
  regeneration is `Module::regen` beside the Reactor Heart's, the fire
  rate is `lift_by_items`.
- **`World::lift_by_items`** (in `skill_of`, after the relic hooks): fire
  rate (*Steady Grip*), range in tiles (*Long Barrel*), and while an
  *Ablative Shell* runs `damage_taken × SHELL_DAMAGE_TAKEN` and
  `Skill::unstrippable` (the room's `strike_stripping` strips nothing
  then — a Warden's lance).
- **`items_on_machine_hits`**, off `land_on_machines` with every crew hit
  that landed on a machine (`(by, droid, damage)`): *Leech Capacitor*
  heals the share through `heal_crew`; *Arc Coil* counts the holder's
  hits (`ItemClocks::arc_hits`) and every `ARC_EVERY`th strikes the
  nearest N other machines standing within `ARC_REACH_TILES` (distance,
  then index — no draw), on the chassis, `last_hit_by` and `shown_hits`
  kept as for any hit.
- **The actives** go through `Command::UseItem` as the blink does:
  *Field Mender* heals every crewmate up within `MENDER_TILES` (the user
  included), *Reset Capacitor* readies every class cooldown
  (`cooldowns_less`), fills every charge and readies the other items,
  *Ablative Shell* puts `ItemClocks::shell_until` on. Each says
  `WorldEvent::ItemUsed { who, kind }` (149). `World::shell_left` is the
  app's reading.

`tests_items.rs`' three new tests are the rule. **`SAVE_VERSION` 87,
`wire::PROTOCOL` 90.** No pin moved for an item nobody carries.

## A mission opens with the crew round the gangway (October 2026)

> "The loop" above says the crew arrive where they stood when the ship
> left; since this change `begin_mission` stands them aboard.

`World::stand_the_crew_aboard` (`mission.rs`), at the end of
`begin_mission`'s `make_whole`: every living crew member but a
reinforcement on a free deck tile **of the ship itself** round
`Aboard::gangway` (`Game::free_tiles_near`, the reach widening until
there is a tile apiece), players first by slot, nearest first, and
`Game::stand_still_at` — the walk and the post dropped. Before it, a
body off the ship when the rooms came apart was snapped to whatever
corner of the deck lay nearest (the bridge, beside life support), and
the next mission opened there. The run's first mission is at home, a
defence, whose own start stands the crew ashore as before; `World::start`
is untouched. Nothing saved or hashed changed shape; `SURVIVORS` and
`REFERENCE_CHECKSUM` move (their runs travel) and were already off their
pins, so they are not re-pinned here.
`a_mission_opens_with_the_crew_aboard_round_the_gangway`
(`tests_mission.rs`) is the rule.

## Ten hit points a level (October 2026)

A player's level is on its bar: `class::level_health(level)` —
`LEVEL_HEALTH` (10) times the level, counted from the first, so 110 at
level one and 260 at sixteen — said to the crew's room every step by
`hand_the_room_the_levels` (`Game::set_level_health`, beside the field
medics' hand-over), nought for anybody with no class (a bot, a hand, a
reinforcement), who never levels. The room keeps it on
`Bim::level_health` and adds it to the items' `Gear::max_health`
whenever the bar's whole is worked out, so a level reached keeps the
bar's share as an item put on does. `healing_links` and
`health_share` read `Game::max_health` now, not `MAX_HEALTH`.
`SAVE_VERSION` 89. `every_level_is_ten_hit_points_on_a_player_s_bar`
(`tests_items.rs`) is the rule. A seeded run's classed players have
more hit points, so `SURVIVORS` and `REFERENCE_CHECKSUM` move (both
already off their pins; not re-pinned here).

## Light and dark: fewer lamps, night at a town, a dark station (task 152)

The room's rule is `crates/game/CLAUDE.md` ("The dark"); the world's part:

- **Fewer wall lights**: `furnish` hangs two opposite inner corners of a
  lit block and one every `LAMP_SPACING` (13) tiles along its long walls,
  first one wall and then the other — it was all four corners and every
  six tiles of both walls, and a station was lit throughout. A town's
  houses have fewer too (the town test asks sixteen, not forty); its
  street lights are as they were.
- **Night at a town**: `Residents::night`, dealt in `open_residents` by
  `crew::is_night(map_seed, run_day)` (one visit in two) or
  `World::set_night_for_probe` (`BIMS_NIGHT`); `Residents::set_night`
  takes the town room's daylight away and tells its plain, `join` and
  `unjoin` leave the sky off, and `Aboard::daylight_over_station(night)`
  does the crew's deck (`Game::set_night`: the plain's reach and the
  picture's deeper dark). Only the lamps light the town then.
- **A dark station**: `Residents::dark`, dealt in `open_residents` for a
  station an enemy holds — never a town, never the Heart's fortress —
  by `crew::is_dark_station(map_seed, run_day)` (one visit in five) or
  `set_dark_for_probe` (`BIMS_DARK`): `Residents::set_dark(dark, design)`
  switches off its room's lamps (`Game::set_lamps_off` over the
  station's ground), `join`/`unjoin` keep them off, and
  `Aboard::lamps_off_over_station` the crew's deck's in the station box
  (at `join_rooms` and a reopen while joined), the ship's left lit.
- `fixture::simulation_world` and `crewed_world` land by day and keep
  every station lit (both dials `Some(false)`); the `open_*` worlds roll.
- `tests_surface::a_town_at_night_is_lit_by_its_lamps_alone` and
  `tests_droid::an_enemy_s_station_is_dark_one_visit_in_five` are the
  rule. **`SAVE_VERSION` 90, `wire::PROTOCOL` 94.** `SURVIVORS` and
  `REFERENCE_CHECKSUM` move (they were already off their pins).

## The relics rebuilt (October 2026)

> Every section above about a relic held by one player's Bim, the pool,
> the caches, the dice, the trader's relic, *Restock Codes*, the hooks
> (`relic_hooks.rs`, triggers, timed buffs, auras) and the 37 relics
> describes what this change **deleted**. The player found relics
> redundant with the items; a relic is now the crew's, and it changes how
> the run plays rather than adding a number to one Bim.

- **Twelve relics** (`crate::relic::RELICS`, codes 0–11): *Glass
  Cannon*, *Heavy Plating*, *Hair Trigger*, *Overclocked Cores*, *Bounty
  Contract*, *Hunter's Pact*, *Drill Sergeant*, *Lone Wolves*, *Black
  Market*, *Adrenaline*, *Salvage Burn*, *Nanite Mesh*. A row is a list of
  `Modifier { who, stat, amount }` — `Who::{Everyone, Players, Bots}`
  (a bot is every crew member no player steers: bots, hands, joiners,
  reinforcements) and `Stat::{Damage, FireRate, MoveSpeed, DamageTaken,
  Cooldowns, MachineDamage, Bounty, Experience, WaveSize, TraderPrices,
  Regen, EliteWave}` — and **every row helps and costs** (`Modifier::helps`, pinned
  by `relic::tests::every_relic_helps_and_costs`). **No relic touches a
  revive or cuts the healing** (the player: it would make the medic and
  the commander's Medivac useless). Every number is `data.rs`'s
  (`GLASS_CANNON_DAMAGE` …), placeholders.
- **Where each is read**: `lift_by_relics` in `skill_of` (damage, melee,
  fire rate, walk, damage taken, by `relic_percent(who, stat)`);
  `relic_percent(.., Cooldowns)` in every class cooldown, as before (with
  the *Coolant Loop* item's cut); `land_on_machines` (in `relics.rs` now,
  the EMP's exposure on the same sum, then the crit) for `MachineDamage`;
  `bounty_here` (`bounty_by_relics`) for `Bounty`, so the money shown and
  the money paid agree; `xp_per_down()` in `experience`;
  `wave_size_with` (`wave_by_relics`, rounded up; a forced wave is as
  forced); `trader_price_by_relics` in `shelf_price`, `item_price` and
  `combine_fee`; `relics_mend` (every crew member on its feet, through
  `heal_crew`). And `EliteWave` (*Black Market*'s price, the player's
  follow-up): `wave_count_here` gives an elite one more wave, forced
  count or not; `is_relic_wave` is that last one (past the first, none
  left), which `settle_droids` lays `elite_wave_extra() × players` bigger
  and `build_wave` lays with a Trooper in every Guardian's place and no
  `with_guardian`. A crew holding none plays exactly as it did.
- **State**: `Relics { held: Vec<Relic>, choice, offers }` — no pool, no
  per-slot lists. `drawable()` is every relic not held; `offer(drawable,
  n, seed)` draws evenly, none twice. `RelicChoice { options, proposal }`,
  `RelicProposal { relic, by, accepted }`.
- **Only an elite's clear** offers `data::RELIC_OFFER` (three)
  (`relics_on_leaving` → `offer_reward`, `Phase::Reward`); **the crew
  vote**: `Command::ProposeRelic { slot, relic }` (`u32::MAX` takes none)
  and `AcceptRelic`, the map's vote — the last connected yes gives it to
  the crew (`RelicGiven { relic }`) or takes none (`RelicsDeclined`), and
  the map comes up; a `PlayerGone` is not waited for.
- **Gone**: `relic_hooks.rs`, `tests_relic_patches.rs`, the cache
  (`Infestation::cache`, `Command::OpenCache`, `cache_rolled`,
  `world_paint::cache_lights`), the trader's relic (`Trader::relic`,
  `RelicBought`, `Rewards::relic_price`, `RELIC_PRICE`), *Restock Codes*
  (`Command::Restock`), the dice (`relic::dice`, the app's
  `screens/dice.rs`, `BIMS_DICE`), the tiers and their odds, the unlocks
  (`Profile` keeps classes and wins; an old file's `relics` list is read
  past), `World::ability_length_factor`, the Trauma Kit's quicker revive
  (`class::revive_time(medic)`, `REVIVE_FLOOR_SECONDS`). Events 111,
  113–115, 128, 129, 145 and 146 and refusals 96, 108 and 109 are free.
- **The `relics` command's arena is an elite** now
  (`Session::relics`), or it would offer nothing.

`tests_relic.rs` is the rule. **`SAVE_VERSION` 91, `wire::PROTOCOL` 95** (96 for Black Market's wave)
(the relay wants redeploying). `REFERENCE_CHECKSUM` and `SURVIVORS` move
(the relics' block of the checksum is smaller, the cache is out of an
`Infestation`'s) — both already off their pins, not re-pinned.

## The soldier's E is a Stun Shot (October 2026)

> "The soldier's ranked kit (task 124)" and feature 75's section above
> say E is **Brace** (`Command::Brace`, `WorldEvent::Braced`,
> `BRACE_*`, `can_brace`, `is_braced`, the miss cut and the damage cut
> while braced): all of it went.

- **`Command::StunShot { slot, x, y }`** (the room tile under the
  pointer) — `can_stun_shot`: `NotASoldier`, `OutOfReach` (unfit or
  downed), `NotLearnt`, `AlreadyActive` while one charges, `CoolingDown`
  within `stun_shot_cooldown` (`class::STUN_SHOT_COOLDOWN`, times the
  cooldown relics and items; *Kill Relay* moves `last_shot` back) and
  `Refusal::NoWeaponInHand` (129). The tile is never refused.
  `WorldEvent::ShotCharging` (150) — 75 is left free.
- **The charge** is `Soldier::charging` (`soldier::Charging { until,
  tile }`, a mission minute) and the room's **brace flag**, which is the
  plant now (`Game::set_braced`: the walk dropped, no errand, the gun up
  and nothing fired; the room ends it on an order that moves the body, a
  walk key pressed afresh, a roll or a down). `settle_stun_shots`, right
  before `hand_the_room_the_soldiers`, calls a charge off when the room
  let the plant go or the soldier is unfit (no cooldown), says the room
  the share for the glow (`Game::set_shot_charge`), and at
  `class::STUN_SHOT_CHARGE` (2 s) fires: along the steer's aim when a
  player steers the soldier, else at the tile, as far as the tile was,
  never past `stun_shot_range` (the weapon in hand through `skill_of`)
  and stopped short of a wall (`Game::reach_along`) —
  `Game::fire_stun_shot`, `WorldEvent::StunShotFired` (151),
  `Soldier::last_shot` noted.
- **The burst** is the room's (`Grenade::shot`): every target in the
  radius (`STUN_SHOT_RADIUS`, the grenade's) with a clear line takes
  `STUN_SHOT_DAMAGE` (15/20/25/30) as a blast hit by the soldier — the
  relics' machine factor on it like any hit, never a crit — and is
  stunned through `settle_stuns` (`STUN_SHOT_STUN`, 3 s; the Heart's
  machines and the Manufacturers' Bims never). None of the crew, no
  sentry, no sandbag.
- Hashed: the room's plant as the brace was, and `charging`/`last_shot`
  only where any is set. **`SAVE_VERSION` 92, `wire::PROTOCOL` 98.**
  `tests_soldier.rs`' section B is the rule.

### Every enemy stunned, and he walks while it charges (October 2026)

> The bullets above say the room's plant ends on an order, a walk key or
> a roll, and that the Heart's machines and the Manufacturers' Bims are
> never stunned; both went at the player's word.

- **The charge is no plant**: `Game::set_braced` holds the fire and
  nothing else, and only going down lets it go, so `settle_stun_shots`
  calls a charge off for a down alone. Fired, it bursts along the steer's
  aim as far as the first tile is from where he then stands.
- **`settle_stuns` stuns every target the burst reached**: past the
  residents' Bims a machine (`Droid::stun`, the Heart's too — a stunned
  core's beams cool, a stunned fabricator skips its build in
  `fabricate`), among them one of their people (`Game::stun_bim`: a
  Manufacturer, a hostile site's; a defended site's own people are no
  target). A stunned Bim is hashed beside the stunned machines, only
  where one is.
- **`SAVE_VERSION` 98, `wire::PROTOCOL` 114.**
  `tests_soldier::a_stun_shot_is_refused_goes_on_as_he_walks_and_is_called_off_by_a_down`,
  `tests_manufacturer::a_stun_shot_stuns_a_manufacturer_where_it_stands`
  and `tests_engineer::the_heart_s_machines_are_stunned_too` are the rule.

### Aimed like a grenade, and it hits what is in its way (October 2026)

> The bullets above say it fires along the steer's aim when a player
> steers the soldier, and bursts only where it lands; both went at the
> player's word ("it just flies max distance every time").

- **It fires at the tile aimed at** (`World::stun_shot_landing`): from
  where he stands when it fires, towards the tile, as far as the tile,
  never past the weapon's reach, short of the first wall — the steer's
  aim plays no part. The app draws the burst's ring there
  (`stun_shot_landing`, and `stun_shot_aim` through the charge).
- **It bursts on the first enemy it passes** (`Combat::tick_grenades`):
  each step a shot's flight from where it was to where it is is swept
  for a target (not a stale one) within `HIT_RADIUS`; the nearest along
  it moves the burst there and ends the fuse.
- **The app arms the pointer for it** as for the grenade
  (`screens/game.rs`'s `Throw::StunShot`): `E` held draws the reach and
  the ring, let go (or a left click) sends `Command::StunShot` at the
  tile under the pointer.
- **He faces the spot while it charges** (the player's word, after):
  `settle_stun_shots` says the room the tile's middle every charging
  step (`Game::set_shot_at` → `Character::shot_at`, `serde(skip)`, None
  when it fires or is called off), and `Character::update` turns the
  body to it at once before the steer's aim or a turn towards the walk,
  the feet going the keys' or the route's way (stepping backwards when
  they part). `tests_soldier::a_soldier_faces_where_his_stun_shot_goes_while_it_charges`;
  `wire::PROTOCOL` 125.
- **`wire::PROTOCOL` 123.**
  `tests_soldier::a_stun_shot_bursts_on_the_first_enemy_in_its_way` is
  the rule.

## The medic reworked (task 153)

> "The medic's ranked kit (task 130)" above describes the Nanite Burst,
> the Healing Aura and the Cloak **task 153 replaced**, and a beam that
> holds the medic's fire. Kept as history; this is the medic now.

The class's tables are `class.rs`'s (`HEAL_DRONE_*`, `TRIAGE`,
`OVERRIDE_HEAL`, `HEALING_CIRCLE_*`); `crate::medic::Medic` keeps
`patients`, `last_drone`, `drone: Option<Drone { x, y, patient, until }>`,
`circle` and `last_burn`. `class::Ability::{NaniteBurst, Cloak}` are
`{HealDrone, HealingCircle}`.

- **Every heal of a medic's goes through `World::medic_heal`**: times
  `medic_heal_factor(medic, who)` — his **Triage**, `1 + TRIAGE[C rank]
  × the share of its bar `who` is missing`, times `override_heal`
  (`OVERRIDE_HEAL`, 1.5, while he carries an *Override Core* — its gift
  to a medic, the circle's fifth rank being the fourth's). Everything
  else heals through `heal_crew`, plain now (no aura): a Healing Sentry,
  an item, a relic. A revive is no heal.
- **E, the link**: `medic_skill` holds nothing (full fire at every rank);
  `hand_the_room_the_medics` heals each patient **and the medic himself**
  at `beam_rate` — the strongest heal reaching a body, once, so two
  patients or himself as patient is one heal on him.
- **Q, Heal Drone**: `Command::HealDrone { slot }` → `WorldEvent::
  DroneLaunched` (152); `can_heal_drone`: `NotAMedic`, `OutOfReach`,
  `NotLearnt`, `CoolingDown` (`heal_drone_cooldown`, the relics' cut,
  *Kill Relay* moving `last_drone`). `fly_the_drones`, before the rooms
  step: each drone moves `HEAL_DRONE_SPEED` tiles a second straight at its
  patient (over walls), `drone_patient` keeping one still on its feet, on
  the deck and short of its bar, else the lowest by share — the medic
  included, the lower index on a tie — or keeping by its medic with
  nobody hurt; within `HEAL_DRONE_REACH` it heals `HEAL_DRONE_HEAL` a
  second through `medic_heal`. Gone at `until` or with its medic dead.
- **R, Healing Circle**: `Command::HealingCircle { slot, on }` →
  `WorldEvent::Circled { who, on }` (153); `can_healing_circle`: on wants
  `NotAMedic`, `OutOfReach`, `NotLearnt`; off is never refused a medic.
  `hand_the_room_the_circles`, after the beams: a circle whose medic is
  unfit or off the deck goes off (`Circled { on: false }` said); else every
  crew member on its feet within `HEALING_CIRCLE_RADIUS` with a clear line
  (`healing_circle_reaching`, himself left out) is healed `beam_rate`
  through `medic_heal`, the medic is drained `healing_circle_rate`
  (the beam's rate times the core's) through `Game::drain` — **which can
  down him** — and every `HEALING_CIRCLE_PULSE` seconds `Game::scorch`
  burns every target in it for `HEALING_CIRCLE_BURN` of the rate.
- **Timers**: `make_whole` clears the drone, its cooldown and the
  circle; `casualties` the dead medic's whole state; `clear_beams`
  forgets a drone's patient with the beams (crew indices moved).
- **A site's defenders are patients too** (the beam and the drone, not
  the circle): a patient is a crew index or `medic::GUEST + i`, body `i`
  of the residents' room where `Residents::is_defender` and the rooms are
  joined (`defender_patient`). `patient_pos` (the crew's room's units; a
  defender's off `body_position`), `patient_bar`, `patient_standing` and
  `medic_heal` read and heal it in whichever room it lives in;
  `patient_at` is the beam's pick — a crewmate under the pointer first;
  the app's E asks `beam_patient_near`, which off everybody takes the
  nearest friendly the beam reaches (range and sight) within
  `class::HEAL_BEAM_PICK_REACH` (three) tiles of the pointer, never the
  medic or one already held. **A link is kept out of sight**:
  `beam_reaches(.., sight)` asks sight only when a link is made
  (`can_beam`), and `hand_the_room_the_medics` keeps one on range, life
  and the room alone (`wire::PROTOCOL` 109;
  `a_link_holds_out_of_sight_and_breaks_out_of_range`).
  `drone_patient` keeps a hurt crewmate, else the lowest crewmate, and
  only with none hurt a defender. A link is checked every step
  (`beam_reaches`), so one on a defender breaks when its room closes. The app names one by
  `resident_name` and draws it by `Session::patient_on_screen`.
  `tests_medic.rs` section F. **`wire::PROTOCOL` 107.**
- **Gone**: `World::cloaks`, `hand_the_room_the_cloaks`, `lift_by_cloak`,
  `hidden_from_enemies`, the cloaked-ability gate, `heal_factor`,
  `healing_aura_*`, `nanite_burst_*`, `cloak_*`; `Refusal::{OutOfCloakRange,
  NoSightOfTarget, Cloaked}` (116–118) and `WorldEvent::{NaniteBurst,
  Cloaked}` (136, 137), codes left free. The room's cloak picture
  (`Game::set_cloaked`, `set_targets_withheld`) stays, set by nothing.
- **Checksum**: per medic, only where any is set, `last_drone`, the drone
  (where on the fine grid, its patient, `until`) and the circle with its
  last burn. **`SAVE_VERSION` 93, `wire::PROTOCOL` 99.** `tests_medic.rs`
  is the rule (sections B, C and E new); the app's words are
  `names::{heal_drone_line, healing_circle_line, heal_drone_refused,
  healing_circle_refused}`, the pictures `theme::{healing_circle,
  heal_drone}` and `ship::sprays::circles_and_drones`, and the clips the
  burst's and the cloak's borrowed (`sound.rs`).

## The engineer reworked: mines and satchel charges (task 154)

> "The engineer's ranked kit, and charges without kits (task 127)" above
> says Q is an EMP and E sandbags; both went. The room's half is
> `crates/game/CLAUDE.md` ("The engineer's mines and satchel charges").

- **Q, Mine** (`Charge::Mine`, code 4 — the EMP's; `DeployKind::Mine`,
  3): laid through `Command::Deploy` like the Healing Sentry, in
  `class::MINE_MINUTES`; `MINE_STANDING` of one engineer's lie at once
  and one more takes the oldest up (`DeployableLost`). `settle_mines`,
  right before the crew's room steps, sets off every mine with a target
  of the crew's room standing within `MINE_TRIGGER` (one tile,
  `Game::enemy_near`): off the deck, `Game::detonate` at the owner's
  `mine_blast` (`MINE_DAMAGE`, `MINE_RADIUS`), `WorldEvent::MineTriggered`
  (154). It packs up (`DeployKind::packs_up`).
- **E, Satchel Charge** (`Charge::Satchel`, code 0 — the sandbags';
  `DeployKind::Satchel`, 4; `class::Ability::Satchel` where
  `Ability::Emp` was): thrown, never laid — `Command::ThrowAt { satchel:
  true }` (`PendingThrow::satchel` where `emp` was), `can_throw_satchel`
  the EMP's refusals, `throw_satchel` spending the charge and the room
  lobbing it; `WorldEvent::SatchelThrown` (155). `settle_satchels_landed`,
  after the crew's room steps, lays each that came down on its tile
  **whatever is there** — satchels stack. `Command::Detonate { slot }`
  (`can_detonate`: `NotAnEngineer`, `OutOfReach`, `NotLearnt`,
  `Refusal::NoSatchels` 130) sets off every one of his in the crew's room
  (`satchels_out`), each its own `Game::detonate` at `satchel_blast`
  (`SATCHEL_DAMAGE`, `SATCHEL_RADIUS`), `WorldEvent::SatchelsBlown { who,
  count }` (156). `Command::Deploy` of a satchel is `CantDeployThere`, a
  pack-up `NoSuchDeployable`. `SATCHEL_CHARGES` is two at every rank.
- **Gone**: `Command::Emp`, `WorldEvent::EmpThrown` (132, free),
  `World::{can_throw_emp, throw_emp, emp_radius, emp_stun,
  sync_deployed_cover, settle_bursts}`, `class::{EMP_*, SANDBAG_*}`,
  `Charge::{Sandbag, Emp}`, `DeployKind::Sandbags` (0, free), the double
  bag. Nothing laid is cover; `settle_deployables` drains the room's
  cover hits and forgets them. A machine is stunned by the Stun Shot
  alone now, and nothing exposes one (`land_on_machines` adds no
  `EMP_EXPOSE_PERCENT`).
- **The sentries' health doubled**: `SENTRY_HEALTH` 400 to 1 000,
  `HEALING_SENTRY_HEALTH` 120 to 240.
- **The codes reused** keep `Charge::CODES` at five, so a crew with no
  charges hashes as it did; `REFERENCE_CHECKSUM` and the survivor pins
  move only where an engineer's charges or deployables are in a run.
- **The app**: Q lays a mine at the pointer, E is a quick throw like the
  soldier's grenade (`screens::game::throw_action`), **Space** is
  `keys::Action::Detonate` — the pause it was is the Esc sheet's
  Pause/Resume button (`settings::Allowed::paused`,
  `save::Request::Pause`). `Glyph::{Mine, Satchel}`, `fittings::{mine,
  satchels}` (the satchels on a tile drawn as one stack).
- **`SAVE_VERSION` 94, `wire::PROTOCOL` 100.** `tests_engineer.rs` is
  the rule.

## The tank reworked (task 155)

> "The tank: the wall, the taunt and the hits (feature 77)" and "The
> tank's ranked kit, and the talents gone (task 139)" above describe the
> Taunt, the Bulwark and the Juggernaut **task 155 replaced**. Kept as
> history; this is the tank now.

The tables are `class.rs`'s (`RIOT_SHIELD_*`, `PLATED_REGEN`,
`REFLECT_*`, `BASTION_*`); `crate::tank::Tank` keeps `shield_up`,
`shield_spent` (nought is whole, so a fresh tank's shield is), the last
hit on it (`shield_struck`), when it broke (`shield_broke`), `last_reflect` and the
`reflect` window, `last_bastion`, and `hasted` — the one field anybody's
entry carries, a tank or not. `class::Ability::{RiotShield, Reflect,
Bastion}` are where `{Taunt, Bulwark, Juggernaut}` were.

- **Q, Riot Shield**: `Command::RiotShield { slot, on }` →
  `WorldEvent::ShieldRaised { who, on }` (157), said only when it
  changed. `can_riot_shield(slot, on)`: down is never refused a tank;
  up wants `NotATank`, `OutOfReach` (unfit — downed among it),
  `NotLearnt`, and `Refusal::ShieldRecharging` (131) while a broken
  shield's cooldown runs (`is_shield_recharging`,
  `riot_shield_cooldown_left`: `RIOT_SHIELD_BROKEN_COOLDOWN`, ten
  seconds from the break, times the cooldown relics and items, moved
  back by *Kill Relay* like any class cooldown). `hand_the_room_the_tanks`,
  before the rooms step, puts down a shield whose tank is unfit or has no
  rank and hands the rest to the crew's room
  (`Game::set_riot_shields(&[(who, left, whole)])`), which stops and
  bounces the bolts (`crates/game/CLAUDE.md`). **`settle_tanks`**, after
  `casualties`: every `Game::take_plate_blocks` comes off `shield_spent`
  and notes the minute; at the whole it breaks — down, `shield_broke`
  noted, `WorldEvent::ShieldBroken` (158); then every tank's shield
  restores its rank's `RIOT_SHIELD_REGEN` (0.5, 1, 1.5, 2) a second
  (`riot_shield_regen`), stowed at once — through the cooldown too — up
  only `RIOT_SHIELD_REGEN_DELAY` after the last hit, and a cooldown run
  out is forgotten (`shield_broke` back to `None`). `SAVE_VERSION` 96,
  `wire::PROTOCOL` 103 for the cooldown.
- **C, Plated**: the damage taken as before, and `plated_regen` hit
  points a second through `heal_crew` in `settle_tanks`, for a tank fit
  to act.
- **E, Reflect Barrier**: `Command::Reflect { slot }` →
  `WorldEvent::Reflecting` (159); `can_reflect`: `NotATank`,
  `OutOfReach`, `NotLearnt`, `AlreadyActive`, `CoolingDown`. While it
  runs `tank_skill` sets `Skill::reflect` to `REFLECT_SHARE`, and the
  room sends that share of every enemy hit on him back on the shooter.
  **The shooter is known across the seam now**: a hostile room signs
  every recorded `Shot` with its body index (`Combat::sign_last_shot`,
  Bims first then the machines — the index the crew's room has it at on
  its targets), and `visit` fires it through `Game::enemy_fire_by`,
  `enemy_strike_by` and `enemy_sweep_by`; the hit back is an ordinary
  crew hit by the tank, carried to the machine with the next step's
  hits.
- **R, Bastion**: `Command::Bastion { slot }` → `WorldEvent::Bastion
  { who, reached }` (160); `can_bastion`: `NotATank`, `OutOfReach`,
  `NotLearnt`, `CoolingDown` (`BASTION_COOLDOWN`, the relics' cut).
  `bastion_reaching` is `crew_within` his radius (the Override Core's
  fifth row a tile wider) on their feet; each gets
  `Game::set_draining_shield(who, BASTION_HP, BASTION_SECONDS,
  BASTION_DRAIN)` — the relic shield's own slot, so it takes a hit
  before the armour and is drawn as that bubble. At `OVERRIDE_RANK`
  each also gets `Tank::hasted` for the seconds, which
  `lift_by_bastion` (in `skill_of`, after the commanders) reads as
  `walk × BASTION_HASTE`.
- **Timers**: `make_whole` resets every `Tank` (the shield whole and
  down, both cooldowns ready); `cooldowns_less` moves `last_reflect` and
  `last_bastion`; `casualties` clears a dead tank's.
- **Gone**: `Command::{Bulwark, Taunt, Juggernaut}`,
  `WorldEvent::{Bulwarked, Taunted, Juggernaut}` (79, 80, 141, left
  free), `taunts_for_the_enemy` (the enemy's rooms are handed no taunt —
  the room keeps the rule, set by nothing), `TAUNT_*`, `BULWARK_*`,
  `GUARDED_*`, `INTERPOSE_RANK`, `JUGGERNAUT_*`.
- **Checksum**: per tank `last_reflect` where `last_taunt` was, the
  rest only where any is set, and every crew member's `shield_up` where
  the room's bulwark flag was — so a world with no tank hashes what it
  did. **`SAVE_VERSION` 95, `wire::PROTOCOL` 101.** `tests_tank.rs` is
  the rule (sections C to G new).

## The research keys gone (October 2026)

> "Nothing is stored" above says the holdings count the research keys
> (`pick_up_key`, `KeyFound`) and the money rework's sections price them;
> all of it went.

`Holdings::keys`, `World::pick_up_key`, `WorldEvent::KeyFound` (124, left
free), the key branch of `stock_the_armory`, the keys in `worth` and their
line in `world_checksum` are gone, and so are `physics::ResourceId::
{ResearchKey, ResearchKeyTwo}` (13 and 14, left free: `ALL` is thirteen,
`CODES` still 22, so no design hash moved) with every row of theirs in
`economy`, `worldgen` and the app (the armory line on the Tab panel shows
the money alone). `Storage::Research` stays as the desk's class, holding
nothing. `shipdesign::research` has no keys or locks (`NodeDef::locked`,
`Research::{unlocked, unlock, is_unlocked, needs_key, key_wanted}`,
`KEY_CELLS`). **`SAVE_VERSION` 97, `wire::PROTOCOL` 106**; `worldgen`'s
`REFERENCE_CHECKSUMS` re-pinned (two leans shorter, no bump) and
`REFERENCE_CHECKSUM` moves (one `eat` fewer; already off its pin).

## A defender wears the militia's kit (October 2026)

`hand_the_room_the_outfits` tells the residents' room `Outfit::Defender`
for every body `Residents::is_defender` says is one, every step — drawing
only (`crates/game/CLAUDE.md`, "A body is cloth, lit, and the crew wear a
lamp"). An outfit is left out of a save, so **`World::dress_the_rooms`**
hands them over outside the step and `ship::Game::resume` calls it on
every load, or a world read back draws its first frame in plain coveralls.
`stand_defenders_for_probe` (**`BIMS_DEFENDERS=1`** in the app) stands the
site's first person and its defenders in a row by crew member 0, the three
looks in one picture. `a_site_s_defenders_wear_the_militia_s_kit_and_its_own_people_do_not`
(`tests_defense.rs`) is the rule. No `SAVE_VERSION` or `wire::PROTOCOL`.

## Twenty levels, the last four weapon damage (October 2026)

> "The soldier's ranked kit (task 124)" and "The tank's ranked kit"
> above say sixteen levels; it is **twenty** now.

`class::LEVELS` is 20 and `LEVEL_XP` runs on past the sixteenth's 3 200
(3 570, 3 980, 4 430, 4 920, the step growing by forty).
`Progress::points` counts a point a level only up to
`class::SKILL_LEVELS` (16, every rank of the four slots), so the four
levels past it give none. Each is ten hit points like any level
(`level_health`, said to the room as before: 300 at the twentieth) and
`class::LEVEL_DAMAGE` (five per cent) more weapon damage:
`class::level_damage(level)` multiplies `Skill::damage` in `skill_of`
for a classed crew member, right after the class's half — a bolt's and a
blade's damage, never a grenade's, a mine's or any ability's, and not a
sentry's (its skill is its own). Weak Spot's crit reads the flat damage
and is unchanged. `combat_droids_<class>` opens at the top (twentieth)
with its sixteen points to spend. Nothing saved changed shape;
**`wire::PROTOCOL` 112**. `SURVIVORS`/`REFERENCE_CHECKSUM` can move only
for a run whose classed crew pass 3 200 experience (both were already
off their pins; not re-run here). `class::tests`' twenty-level table and
`tests_items::the_levels_past_sixteen_are_hit_points_and_weapon_damage`
are the rule.

## Nothing combined: an item upgraded, and a thing sold (October 2026)

> "Items: four slots a player's Bim", "The trader (task 114)", "The
> minigun and the rail lance" and the arc greaves' sections above say two
> of a kind combine a tier up (`Command::Combine`, `buy_partner`, *Buy &
> Combine*, `combine_fee`); all of it went.

- **An item is bought onto the player's own Bim, never into the
  armory** (`Command::BuyItem { slot, kind }`, `to` gone). What the line
  sells is `World::item_offer(slot, kind)` → `items::ItemOffer`: `Buy`
  (the day's tier off the shelf, onto the first free item slot,
  `ItemsFull` otherwise), `Upgrade { at, from, to }` where the Bim
  carries the kind — `items::upgraded`, the next tier **whatever the
  day**, made in slot `at`, every slot full or not — or `Top` past tier
  three (and the *Override Core*, made at one tier), refused `TopTier`.
  The price is `item_offer_price`: the tier bought at's whole price, an
  upgrade the next tier's. One of a kind a visit as before
  (`items_sold`), so an item bought is upgraded at the traders after.
  `ItemBought::upgrade` says which.
- **`Module::paid`** (serde default, hashed only where non-zero) is what
  its owner paid at the traders, every upgrade added on.
- **A sale**: `Command::Sell { slot, from }` at a trader (`NotAtATrader`
  otherwise) — the armory, or a slot of a Bim the player `may_change` —
  for `World::sell_value`: `data::SELL_BACK_PERCENT` (50) of an item's
  `paid` (of its tier's price today for one never bought), of a weapon's
  or a piece's `shelf_price` at its tier today; a charge is not sold.
  `sellable(slot, from)` is the app's question; the slot is left empty,
  the value `credit`ed to the seller's wallet, `WorldEvent::Sold` (161).
- **Gone**: `trader::{combined, CombineError, next_tier}`,
  `items::combined`, `World::{combine, combine_fee, buy_partner,
  combine_bought, number_made}`, `Rewards::combine_fee` (and its line in
  `rewards.ron`), `data::COMBINE_FEE`, `WorldEvent::Combined` (127) and
  `Refusal::NotAPair` (107), codes left free. A gun bought with one like
  it already owned is simply a second gun.
- **The app**: the trader's window has a **Buy** and a **Sell** tab; an
  item line the Bim carries reads *Upgrade*, washed and outlined in the
  next tier's colour where *Buy & Combine* was; the Sell tab lists the
  player's own Bim's, each bot's and the armory's things with their
  price. **`SAVE_VERSION` 100, `wire::PROTOCOL` 122.**
  `tests_items.rs` (`a_trader_sells_items_at_the_day_s_tier_onto_the_buyer_s_own_bim`,
  `an_item_carried_is_upgraded_a_tier_at_every_later_trader`,
  `a_thing_sold_fetches_half_of_what_was_paid`) and `tests_trader.rs`
  (`a_second_gun_bought_is_a_second_gun_and_another_player_s_kit_is_not_sold`)
  are the rule.

## Two guns, a shelf a tier past what was bought, and the pistol kept (October 2026)

> "Items: four slots a player's Bim" and the section above say a shelf
> is one gun and one piece at the day's tier; that moved.

- **The shelf** (`trader::roll_shelf`, `shelf_off`) is
  `data::TRADER_WEAPONS` (**two**) guns, drawn without putting back so
  never two alike, then the piece. `trader::shelf_candidates` never
  holds the **laser pistol**: every Bim sets out with one.
- **A shelf a tier past what was bought**: `Run::shelf_bought` (saved,
  serde default; hashed in the traders' block only where any is set) is
  each player slot's best tier code of gun and of armour bought off a
  shelf this run. `World::shelf_tiers(slot)` is the tiers the slot's
  shelf sells its guns and its armour at: the day's (`shop_tier`), or
  one past the best bought where that is higher, three at most.
  `arrive_at_trader` restocks each owner's trader at its own tiers;
  `buy_shelf` records the tier and `Trader::lift`s what is left of that
  kind on the shelf at once (the kind kept — a kind made at a tier is
  made at every tier above). Another player's shelves are untouched.
- **The pistol is never sold**: `sellable` refuses a laser pistol
  wherever it is — a player's hand, a bot's, the armory —
  `Refusal::NotSellable` (132). The trader's Sell tab lists only what
  `sellable` allows, so it is not shown.
- **`SAVE_VERSION` 101, `wire::PROTOCOL` 126.**
  `tests_trader.rs`
  (`a_thing_bought_puts_its_kind_a_tier_up_and_no_shelf_sells_a_pistol`,
  `the_pistols_the_crew_set_out_with_are_never_sold`) and
  `trader::tests` are the rule.

## A bot's kill pays 5%, a player's 110% (October 2026)

The player's words: "if a bot kills an enemy you should only be rewarded
50% of the gold, that does not count for the commander units (medic and
reinforcements)", then 20%, then "If a bot kills an enemy -> 5% money, if
Player kills +10%", the experience the same. `world::kill_bounty` (a free
function, so it reads while a room is borrowed) takes an enemy's bounty
and who last hit it (`Residents::last_hit_by`, a crew index): a crew
member past the players that is not on `World::reinforcements` earns
`Rewards::bot_bounty_percent` (`data::BOT_BOUNTY_PERCENT`, **5**); a
player, and a commander's R soldiers and Medivac medic as his own,
`player_bounty_percent` (`data::PLAYER_BOUNTY_PERCENT`, **110**); a kill
no crew hand landed last (a sentry's bolt, `by` `None`) the whole. Both
are `rewards.ron` lines. Applied in `visit`'s machine kills and in
`experience`'s Bims (a Manufacturer), before the relics' and the
defence's shares, so `EnemyRewarded`'s money is the share paid. The
experience is untouched. A defender's hit never writes `last_hit_by`, so
a machine a bot hit and a defender finished counts as the bot's.
`wire::PROTOCOL` 129.
`tests_mission::a_bot_s_kill_pays_five_per_cent_and_a_player_s_a_tenth_more`
is the rule.

## Every item and ability on every enemy (October 2026)

> The player's word: "all items and spells should work on all enemies,
> not just on machines or manufacturers". A sweep of every ability and
> item found four that touched the machines alone; the rest (grenades,
> mines, the satchel, sentries, the Healing Circle's burn, the Stun Shot,
> the Reflect Barrier, the crits) already went through the room's
> targets, which are both kinds.

- **`land_on_enemies`** (`relics.rs`, was `land_on_machines`) lands
  every crew hit on a machine **or a Manufacturer** (`Game::is_manufacturer`)
  of the residents' room: the relics' `Stat::MachineDamage` share (now
  "damage to enemies"), the crit, the strike — `strike_droid` past the
  Bims, `blast`/`strike` on one of their people — the number shown and
  `last_hit_by`. Only the residents' other Bims are handed back to
  `visit` (which adds their crit, as before), so no hit is crit twice.
- **`items_on_enemy_hits`** (`item_use.rs`, was `items_on_machine_hits`)
  takes `(by, body index, damage)`: the *Leech Capacitor* gives back its
  share of a hit on either, and the *Arc Coil* counts both and arcs from
  the one struck (`Game::body_pos`) to the nearest enemies within reach
  — machines standing and Manufacturers on their feet, by distance then
  body index (for machines alone the old droid order, so a machines-only
  run arcs as before) — `strike_droid` on a machine's chassis, `strike`
  on one of theirs.
- **A Rampage at rank four** is lengthened by a Manufacturer down too:
  `experience` calls `rampage_kill` for each of theirs it counts
  (machines still through `machine_kills_noted`).
- No `SAVE_VERSION` (nothing saved changed shape); `wire::PROTOCOL` 128
  (both ends must land the hits alike).
  `tests_manufacturer::a_leech_and_an_arc_work_on_their_people` and
  `a_manufacturer_downed_lengthens_a_rampage` are the rule.
