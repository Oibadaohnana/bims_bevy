# The world

Notes on `crates/world` — one star system, the ship in it, the stations, the
crew aboard and the one clock. The room it steps is `crates/game/CLAUDE.md`;
the painter and the page, `crates/ship/CLAUDE.md`.

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

`shipdesign::recipes::RECIPES` is the table: a station `PartKind`, inputs,
one output, minutes, and `vents` — the smelter may lose mass and nothing
may gain it; `every_recipe_holds_together` pins the arithmetic against
the resource table, which is why `Emitter` weighs 16 and `Components` 2.
`recipes::at(kind)` is how anything asks "is this a bench"; `aboard.rs`
uses it to build `Layout::benches` (a `room::Bench`: the part's code, its
frame, its use spot) — the **only** `shipdesign` use, still in `aboard.rs`.

The chain is `Kind::Craft { recipe, bench }`, two steps, `GoToBench` and
`Work`, with the length riding in `rest_minutes` the way a doze's does,
because the room has no recipe table. **The room moves no cargo.** `Work`'s
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
`Command::SetCraftTarget` (`net.keep` → `Command::SetCraftTarget`) and clamped to
the class's capacity. Nought at the start, for the same reason as the
stew target. The items panel's `.keep` box is the control and
`recipeLines` is what reads the `ship_recipe_*` exports — all seven of
them, or the boundary check names the unused one. `JOB_CRAFT = 18`,
`SPOT_BENCH = 19` (ringing only; the readout names the part), and
`WORK_NAMES`/`WORK_SPOTS` in `crates/app/src/crew.rs` grew a row.

`PartKind::Armoury = 34` is the third bench, and the one that is also a
container (`Storage::Locker`, eight since the four weapons); `Handgun =
9`, `Vest = 10` and `Medkit = 11` are what it makes, and `Shotgun = 18`,
`AutoRifle = 19`, `SniperRifle = 20` and `Schword = 21` since
(`RECIPES[10..=13]`), all in the locker class. **A held item
is a resource in the locker class** — a count, no per-item state — until
something needs a charge or wear; the suit was the first and these are the
next three. Armour was the first thing that *did* need wear, and the
answer was not to make it something else: it stays a count in the hold
and becomes a `Piece` the moment it leaves it — "A piece of armour is a
resource in the hold and an instance everywhere else" below.
`a_target_for_a_handgun_runs_the_whole_chain_from_the_hold` is
the user's original example run end to end.

`PartKind::DrugLab = 35` is the fourth, and the one whose product the
room *uses*: `RECIPES[6]`, two `Fibre` (13) to one `Bandage` (14) in a
quarter of an hour. The hold is still the world's — the room never sees
the recipe — but the count of bandages to hand and the fibre on the shelf
are handed to the room every step and read back after it; see "The enemy
shoots back" below for that seam, and `crates/game/CLAUDE.md` for what a
bandage does.

The playtest ship has a smelter, a workbench and a drug lab aft, all
`R180` so they are worked from the row forward of them (the row aft is
the stern), an armoury forward by the bunk with a second reactor to pay
for it, a second shelf, 40 ore, five bandages, six fibre, a piece of
each armour and one of each of the four weapons — thirteen of the
sixteen locker slots, which is why the armoury's cabinet grew to eight.
`a_target_for_metal_has_a_bim_smelt_ore_at_the_bench` runs
the whole seam natively and `simulation-check.mjs`'s keep section from a
click; both craft tests count `benches()` as four now, the drug lab and
the armoury being benches through `recipes::at`.

## The outside is a place, and the ship's tile grid is its grid

A walk outside — `Kind::Eva`, from `GoToSuitLocker` to `PutSuitBack` —
used to hold the body at a spot beyond the collar for an hour and a half
and read `worldgen::belt_yield`. Now the belt is a **mining site**,
`crates/world/src/mining.rs`: hold station at a belt and `settle_site`
(stage 4's last word in `World::step`) lays `MiningSite::generate` out
**in the ship's design frame** — every rock a `RockTile { x, y, kind }`
on the same integer tile grid the hull is on, negative and past the build
area both — from `Purpose::MiningSite` and the belt's id, clear of
`hull_box()` by `CLEARANCE`. Once per belt: `World::sites` keeps every
site the ship has held at, mined tiles and all, in belt order, and it is
in `world_checksum` whole. That frame is the whole trick: a suited Bim
walks the outside in room units like the deck, the painter turns the
rocks with the hull (`world_paint::rocks`, and `local_node` skips the
belt's icon-rocks while a site is laid out), and a click on a rock is
`Game::tile_at` like a click on the deck.

Skin and core: `depths` floods each blob from the outside in and a tile
`CORE_DEPTH` (3) or deeper is the ore — `Rock::Iron` on most, `Galvum` on
`GALVUM_SHARE` (a tenth: the whole tenths for certain, the remainder as a
chance, so a site of twelve has one galvum asteroid for certain). What a
tile yields is `mining::yield_of`: two `ResourceId::Rock` (the new
resource, id 12, €2, shelf, sold nowhere — that moved `CARGO_SLOTS` to
13 and re-pinned `PLAYTEST_HASH`, both `REFERENCE_HASH`es and
`REFERENCE_CHECKSUM`), two ore, or one galvum.

Nothing is mined that is not **marked**: `Command::MarkRock { x, y }`
toggles, `Command::ClearMarks` clears, `set_off` clears through
`leave_site` (by the *frame*, not the state — the ship is `Travelling`
by then) and recalls whoever is out (`Game::recall_outside`, which
abandons the walk for good and drops a queued one). The room is handed
the site every step as `bims::game::Eva { allowed, targets, rocks,
version, tile_minutes }` — the marked tiles' middles, every rock as a
`Rect`, and `site_version`, which moves on every mark and every mined
tile so the room rebuilds its outside grid then and only then. `Mine`'s
`leave` puts the rock's middle on `Room::mined`; the step drains it with
`Game::take_mined` **before** `take_walks`, `finish_tile` takes the tile
out, adds what fits to the shelf and tallies it, and `finish_walk` says
the tally as one `Mined { rock, ore, galvum }` when the Bim is in (the
walk is counted at `StepIn`'s *enter*, so the same step sees it).

`hold_at_belt_for_probe` is how a test — and `BIMS_AT_BELT=1` in the
app — gets there without flying: undock, put at the belt, settle the
frame, lay the site. `marked_rocks_are_mined_on_foot_and_what_they_yield_lands_on_the_shelf`
marks a straight dig from a core tile out to the skin and runs it;
`a_mining_site_is_laid_out_about_the_ship_at_a_belt` pins the clearance,
the depth rule, determinism and the galvum share over two hundred belts.

The room's half — the outside grid, `next_rock`, why a rock is mined
straight on and never from a corner — is in `crates/game/CLAUDE.md`.
Stage 8 is as it was: `World::health` doses a body at `SUIT_INTENSITY`
while `Game::is_outside(who)`, `EVA_DOSE_LIMIT` keeps a dosed Bim in and
sends one out there home from the next rock (`eva_allowed` on the room).

## The design phase, then the game, and one clock in it

`web/ship.html` is two screens and one wasm. The last Accept settles the ship
**and** opens the world — one event, in `Session::accept`, because two exports for
it would be two things that could disagree about which ship got handed over.
`ship_phase()` is still the only way to ask which half you are in.

After that there is exactly one loop: `World::step`, which advances
`STEP_MINUTES` and nothing else. Its order is the contract and it is written
out in the function:

1. the commands stamped for this step, in arrival order;
2. the clock;
3. flight;
4. discovery and the local frame;
5. **crew** — the room aboard, stepped;
6. **power** — the reactors against the wired consumers, into the batteries;
7. **construction** — what the crew did at the sites, moved through the hold;
8. **health and radiation** — each body dosed or sheltered.

The last two were extension points, and both are filled now; whatever
joins them goes in *there*, on *that* clock. A second clock or a second loop is two
simulations that will disagree, and the failure reads as a ship in two places.

`crates/app/src/screens/game.rs` turns real time into steps with an accumulator — `dt *
ship_steps_per_second() * multiplier` — and never into bigger steps. Same rule
as the room, same reason: a 24x step would move the ship several times its own
length and skip straight past its own braking phase. `MAX_STEPS_PER_FRAME` has
to stay at or above `TOP_SPEED * 60 / 30` or the top of the range quietly stops
being reachable.

## The anchor is stored and the position is derived

`Ship::anchor` is where design tile (0, 0) sits in the system;
`Ship::position()` is the **centre of mass**, worked out from the anchor and
the heading through `flight::angle::rotate_design`.

That way round on purpose. `on_ship_changed` has to promise that welding a
shelf to the stern does not move the *hull*: if the position were stored and
the anchor derived, every wall anybody built would shove the whole ship
sideways through space. Docking is the one place the ship is *put* somewhere
rather than flown there, and it is `set_position` doing it.

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

## A redirect is a stop and then a trip, and it needs no mechanism of its own

A Confirm while the ship is under way aborts the current plan to rest, and the
moment that abort *ends* the pending target is picked up and a fresh plan is
made from where the ship stopped. Two plans, one after the other, and the ship
is genuinely at rest in between. Only the **latest** confirmed target is kept.

The consequence that surprised a test: a redirect one step after departure has
no speed and no spin to take out, so the stop is of no length and the second
trip is already under way in the same step. That is the redirect working, not
a shortcut round it.

## A station is a place, and the ship docks beside it

`crates/world/src/station.rs` turns every `StationBlueprint` of the system
into a `ShipDesign` through `apply` — the same parts, the same rules — sized
by kind (26 to 40 tiles) and dressed by `map_seed`, with the port in the
west skin and the array in the north. `World::stations` holds them from
`World::start`; `Station::all_of` is the only place they are built, and
`station::layout` **caches by (kind, seed)** because a layout is two
thousand `apply`s and the world test suite went from two seconds to a
minute before it did.

Five things that hang off that:

- **Docking is airlock to airlock, outside the hull.** `shipdesign::dock::port`
  is a design's first airlock and the side of it with nothing beyond, and
  `Station::berth` turns the ship so its port faces the station's and puts
  the two outer faces on one point. `World::dock_at` is the one place the
  ship is set down (with `World::start`); `target_position` of a station is
  the *berth*, so a trip ends outside the station rather than at its
  middle. `the_ship_docks_airlock_to_airlock_outside_the_station` checks
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
  its tie-break remainder. Dropped means *forgotten*: come back and they
  start at their bunks. It is in `world_checksum` after the crew.
- **The spawn is the first station somebody lives on**, not the first
  station: `World::spawn` skips derelicts, because a crew that opens docked
  at a wreck sees nobody and blocks. That moved the simulation's dock and
  `REFERENCE_CHECKSUM`. `the_local_frame_has_a_hysteresis_and_uses_it`
  drifts *away from the nearest other node* rather than along `+x` for the
  same reason — in the new spawn system `+x` walked into the parent body's
  frame.
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
opened on the station's design with the station's galley, heads, bunks,
bay and benches for fixtures — and go on living there while the ship is
tied up: cooking at their own hob, eating at their own table, sleeping in
their own bunks, on their own timetable and under their own manager
(`Residents::open` sets their goals, `data::RESIDENT_*_EACH` a head, and
stocks their larder to them). They never come aboard, and the crew's
Management tab reaches nobody ashore — it is the crew's, and one day
another player's crew on the same ship. `set_off` takes the deck apart
again (`Aboard::unjoined`): the crew back into a room of the ship alone;
the residents' room goes on as it was until the ship is out of range.
`docked_the_station_s_people_keep_to_the_station_and_their_own_agenda`
pins a day of it.

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
- **The room's berths and seats are `Vec`s now**, as many as the layout has
  bunks and chairs, ship's first, so everybody has their own bed;
  `BERTHS`/`SEATS` are the classic room's two. `Game::with_layout` caps the
  crew at the beds there are, and a room may have **none** (a derelict's).
  `plate_on_table` goes through `plate_at`/`set_plate_at`, clamped, and
  `solids()` lists the beds where the classic room always did — between
  the table and the bay — because the push-out walks solids in order and
  a different order re-rolls every probe seed.
- **Moving a Bim between rooms drops its errand.** `Game::take_crew`
  abandons every chain (`Task::abandon`: what was carried goes back in the
  store, whoever was in bed is stood up) and `Game::adopt` stands each
  one where they were, snapped to the nearest free nav cell — a spot
  beside a bunk sits on the edge of the bunk's inflated footprint and
  reads free or not by how the grid happened to fall — or at their bunk
  if that is more than a body's margin away, which is what a crew member
  left on the station at departure gets. `take_crew` takes `&mut self`
  and leaves the old room standing for a reason: giving up an errand is
  what puts a sheaf of fibre in somebody's hands into the *old* room's
  store, so `join_rooms` and `unjoin_rooms` take the crew out first and
  then `bank_medicine` off that room — `take_harvested_fibre`,
  `take_bandages_used` — into the hold before it is dropped. Before
  that, a harvest in hand at the moment of docking was lost.
- **One galley, the ship's.** The joined room maps the first of each
  fixture kind by id — the ship's — and `Aboard::leave_the_station_s`
  drops every further fixture standing in the station's box from
  `Layout::more` and `extras`, so the station's galley, heads, bays and
  lockers are furniture to walk round on the joined deck and never a
  chain's pick. The
  residents' own room, with them in it, is what draws the station's
  fixtures and the residents; the joined room draws the ship's fixtures,
  the crew and every door, over it. The cold store is restocked from the
  ship's cargo at every join and unjoin, like at world start.
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

## A station has a stance, and the fight crosses between two rooms

`World::stance(id)` is Friendly for `home` — the spawn station — Hostile
for any id on `World::hostile` (sorted), Neutral for the rest. The list
starts as **what the generator rolled**: `World::start` puts every
station whose `StationBlueprint::hostile` is set on it, bar `home` —
the spawn is home whatever it rolled, since a world handed an enemy's
dock should open at home rather than under fire — and `Station::hostile`
carries the roll for anyone who wants the blueprint's word rather than
the rule. `set_hostile` puts a station on or takes it off after that
(`combat`, through `Session::make_dock_hostile`, is the caller). **Both
`home` and `hostile` are in `world_checksum`**, after the crew; a stance
change is a different fight, and
`the_checksum_notices_a_stance_change` says so. `spawn` and
`spawn_anywhere` skip a hostile blueprint the way they skip a derelict
(`the_galaxy_has_hostile_stations_and_the_spawn_is_never_one`), and the
lobby's own pick does the same. `apply_stances` — at every `join_rooms`,
every `settle_residents` open and every `set_hostile` — tells the rooms:
the residents' room its own stance (`Game::set_stance`, its fog under
`Fog::All`) and whether its people are enemies (`set_hostile_bodies`,
which is the ring under each *and* the switch that makes them fight —
see the next section), and the joined deck the station's box and stance
(`Game::set_foreign`, the black-and-grey fog over the station's half).
The painter asks it too: a stranger's station is its far plate
(`HULL_UNKNOWN`) until its room is open, so approaching one reveals
nothing at fifty tiles, and the map rings every station by it
(`world_paint::paint_map`: enemy red, home green, neutral nothing).

**How many enemies a station puts up is the crew's worth.** A room is
opened with `World::people_of(station)`, never `residents()` straight:
the residents at a friendly or neutral station, nobody on a derelict,
and at a hostile one `station::enemies_of(crew, worth, start_worth)` —
`data::ENEMIES_BASE` (two) plus one a crewmate, doubled for every half
of `World::start_worth` that `World::worth()` has grown by since, capped
at `data::ENEMIES_MAX` (sixteen). Worth is `shipdesign::Budget::spent`
of the ship's design — every part at its price and every unit in the
hold at its trade value, the money in hand *not* counted — and
`start_worth` is that sum at step nought, fixed for the game and left
out of `world_checksum` because two clients on the same design already
agree on it. Whole euros in and a whole number out, so a server counts
the same crowd. The count is asked when the room opens (`join_rooms`,
`settle_residents`), so a station keeps the garrison it was reached
with until the ship has left and come back; `set_hostile` is the one
exception — it reopens the residents' room at the new stance's count
if that differs, because `combat` turns the dock hostile with its two
residents' room already open, and two residents are not a garrison.
`starts` in the room puts anyone past the last bunk on the first deck
tile, so a garrison bigger than the bunks stands stacked there until
its first errand. `enemies_of_grows_with_the_crew_s_worth_and_caps` pins
the formula and `a_hostile_dock_opens_with_a_garrison_not_its_residents`
the reopen. **Mind that "stands stacked there" is not what happens**:
`Game::with_layout` caps the crew at the beds there are, so a garrison
bigger than the station's bunks — an orbital has four — is cut to the
bunks. That is what the arena is for.

**The `combat` command's dock is the arena.** `station::arena(kind,
seed)` is `build_layout` at `data::ARENA_SIDE` (72) with the quarters'
bunks in `ARENA_BUNK_COLUMNS` (4) columns three tiles apart — twenty
bunks, more than `ENEMIES_MAX` — and `World::arena_dock_for_probe`
rebuilds the docked station as that, standing where it stood (the
anchor recomputed from the old centre, since the build area grew),
sets `World::reinforcements` to `ARENA_REINFORCEMENTS` (6) and docks
again from scratch — `undock_for_probe`, the residents' room dropped,
`dock_at` — so the joined deck and the residents' room are laid out on
the new design. `reinforcements` is added to `enemies_of` in `people_of`
(capped at `ENEMIES_MAX`) and is **in `world_checksum`** after the
hostile list: it is the size of the fight. The crew are five on the
combat ship through `World::start_with_crew(design, money, players,
crew, ..)` — `start` is that with `crew = players` — which sizes the
room, `health`, `crew_down`, `crew_locked` and `Ship::crew_count` by
`crew` and `speed_requests` by `players`, so four crew nobody steers do
not hold the speed at 1x (`speed::effective` is the *slowest* request).
`Session::combat` in `crates/ship` does the whole thing and issues
`WeaponKind::ALL` down the crew, one each.
`the_combat_dock_is_the_arena_with_five_crew_and_a_garrison_of_thirteen`
and `the_arena_and_the_combat_ship_can_be_walked` (the walkability
contract again, by `Nav::can_reach` rather than a search per tile)
are the tests.

`World::visit` is where the fight crosses. While the station is hostile
the residents' positions go to the joined room as **targets**
(`Aboard::hostiles`, `None` for one that is down — each at its exposed
position, `Aboard::exposed`, and paired with its weapon kind) beside the
visitors the doors read, and the hits the joined room's bolts landed and
the blows its fists and blades struck (`take_hits`, a `combat::Hit` —
who, which part, how hard, and whether it is a cut) are delivered to the
residents' room one by one (`Game::strike(who, part, damage, cut)`). At a
friendly or neutral station both target lists are empty and nobody
shoots. The residents never come aboard and their room is a step behind
on `seen`, as before; `SEEN_FOR` in the room is what keeps one drawn for
two seconds after the crew lost sight of it.
`a_stranger_s_deck_is_black_beyond_a_grey_ring_and_the_crew_s_own_is_dim`
and `a_recruited_bim_shoots_the_enemies_it_can_see_and_they_are_hurt`
pin both halves; `stage_fight_for_probe` (`BIMS_FIGHT=1` in the app)
stands the two a few tiles apart inside the station's door.

## The enemy shoots back, and a bolt flies in one room only

The other half of `visit`. While the station is hostile the residents'
room is handed the crew as **its** targets every step —
`Aboard::crew_ashore`, the crew's positions in the station's own units
through `Aboard::to_station` (the inverse of `station_frame`: two
projections onto its unit axes), `None` for one dead or outside in a
suit, out cold still a body, **and `None` for one still on the ship's
own deck** — outside `station_box`, off the station's grid, where its
people can neither see it nor find a spot to shoot it from — each paired
with **what it carries** (`Game::weapon`, the pistol if somehow nothing),
since the room reads the weapon to know which targets lock a gunner in a
melee (`bims::combat`) — and that
is what puts its people **at war** (`bims::game::tick_combat`: hostile
bodies and a target that is `Some`): recruited, armed, and walking to
wherever `Tactics::stand` says — or charging, with a blade. The box
matters: a crew that docks at a
hostile station and stays aboard used to put every resident at war with
nobody — recruited, every errand dropped, standing still for as long as
the ship was docked, neither eating nor sleeping. They know where the crew are without seeing them; what they need
line of sight for is the shot. A room whose bodies are hostile does not
fly bolts — it records `combat::Shot`s, and `visit` reads them back
(`take_shots`), puts `from` and `at` through `station_frame` onto the
joined deck and fires each there as a hostile bolt
(`Game::enemy_fire`). So every bolt of the fight, blue or red, flies in
the **crew's** room, which is the one room both sides' bodies are in;
a red bolt looks for the joined room's own bodies and the wound goes
on at once, in the room (`Game::strike`, off `Combat::wounds_taken`),
its damage the weapon's at the distance the bolt flew — the same
fall-off for both sides, since both sides' bolts fly in the one room.
The world only *says* so: `casualties`, right after `visit`, drains
`take_wounds_taken` into `WorldEvent::CrewHit { who, part }` (code 32,
value `who + 10 * part`) and says `WorldEvent::CrewDown { who }` (code
33) the step a crew member is dead or at nought — once, off
`World::crew_down`, whatever did it: a shot, blood lost to a wound
nobody dressed, hunger. `EnemyDown` is said the same way now, off
`Residents::down`, because a shot to the head kills at the top of the
body's *next* tick with the health total still well above nought —
reading it off the hit missed every head shot.

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
doorway found by scanning the deck — so that he actually peeks — and
pins `crew_ashore`/`crew_peeking` against `exposed_at`/`peek`.

**A blow is carried, not flown.** A resident in a melee — a schword
within reach of a crew member, or a crew member's blade within reach of
it — records a melee `Shot` (`shot.melee`, with `damage` and `cut`
carried, since a fist's damage is not the gun's in the hand) and
nothing flies: `visit` finds the crew member it was aimed at as the
handed-over position nearest `shot.at` (a `Shot` carries no target
index) and delivers it with `Game::enemy_strike(on_deck(shot.from), who,
shot.damage, shot.cut)`, which re-checks the reach in the receiving room
— the lock was read a step ago in the other room, and a body walks —
rolls the part there, applies the wound and records it for `CrewHit`.
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
`a_station_s_people_are_armed_off_its_seed` pins it.

Things that bit or would:

- **The order inside `visit` matters.** The residents' targets are set
  *before* their shots are read, so a room that has just gone to war
  has something to aim at from its first step; and the targets are
  cleared — on both rooms — the moment the station is not hostile, and
  on the residents' room whenever the rooms are not joined, or a room
  left at war with nobody there keeps its people under arms after the
  ship has gone. Peeking is set *after* the targets, every time, since
  `set_hostiles` resets it.
- **The order of the two rooms' steps matters too, and it is against
  the crew.** `World::step` steps the crew's room, then the residents',
  then `visit`. Each room's `melee_with` reads the other's positions as
  handed over *last* step, so the step a charging blade arrives the crew
  member still sees it a tile off and does not lock, while the blade —
  stepped after — sees the crew member in reach and its first blow
  lands that same step through `visit`; the crew member's lock, and
  `Locked`, come a step later. A crew member with no armour (a body of
  75) is dead to the first ninety before either, which is why
  `a_blade_charges_and_locks_the_crew_member_who_fights_with_its_fists`
  puts the kevlar on James first, and why every melee's first cut is
  unanswered. Not fixed here; noted for whoever reorders the step.
- **A sniper enemy walks to the open at its range and fires from there.**
  It used to stand in the doorway at the corridor's crossing and never
  fire: `Tactics::stand` scored the doorway as cover with its door shut,
  and the body's own arrival opened the door — see "A doorway is never a
  stand" in `crates/game/CLAUDE.md`. `plan_stand` now hands the tactics
  the room's doorways and none is a stand, so the rifle's resident walks
  the corridor out of the pistol's reach and lands its shot from there —
  the cover at the corridor's crossing, some seventeen tiles off, on the
  default seed, since cover near it beats the open further on;
  `a_sniper_rifle_reaches_from_twenty_tiles` pins that — the resident's
  hit said as a `CrewHit` from a stand past the pistol's twelve tiles,
  James patched up every step since an enemy shoots on the move too —
  before James's own long shot at a resident held twenty-one tiles down
  the corridor.
- **Two against one is not a fight the crew member wins.** An enemy
  station arms `enemies_of` the crew — four against `basic()`'s two —
  and they all come; at pistol range the crew member is down in seven
  seconds. `one_on_one` in the tests shoots every resident but one in
  the head first so a run is one on one, and `Game::patch_up_for_probe`
  makes James good as new before every step of a run that has to end
  with the *other* body down — a one-in-twenty head shot would
  otherwise decide it. `a_recruited_bim_shoots…` does both, and
  `the_residents_shoot_back_and_a_crew_member_hit_bleeds` and
  `a_hostile_station_s_people_take_arms_and_move_to_cover` pin the
  other side off `stage_fight_for_probe`.
  `the_crew_s_shotgun_does_more_at_three_tiles_than_at_nine` pins the
  fall-off through the seam — the curve's fifty at three tiles against
  about thirty-three at nine — with both bodies
  `put_for_probe` each step so nobody walks off the mark.
- **The hold's medicine is handed to the room every step, and read
  back after it.** Stage 5 sets the bandages to hand and the fibre on
  the cold store's shelf from the hold (`hand_the_room_the_hold_s_medicine`
  — `Game::set_bandages`, `set_stock` with the hold's `Fibre`) before
  the room steps, and after it takes what was used off and puts what
  was grown in (`take_the_room_s_medicine`: `take_bandages_used`,
  `take_harvested_fibre`) — fibre the cold store cannot take is
  **dropped without an event**, since the room's count is set again
  from the hold next step and an event a sheaf for a full larder would
  be noise. A dressing is the room's chain (`Game::bandage`; see
  `crates/game/CLAUDE.md`), and
  `a_crew_member_dresses_a_wound_with_a_bandage_from_the_hold` runs it
  on the playtest ship, which carries five. The residents' room gets
  `data::RESIDENT_BANDAGES` at open and no hold behind it — a count its
  people spend on each other of their own accord now that the room has
  a Medical job (`Game::medical_on_offer`, `crates/game/CLAUDE.md`), but
  only once the fight is over: a recruited Bim takes no errand of its
  own, and at war every resident is recruited. A bandage is
  `RECIPES[6]` at the drug lab out of two fibre — the playtest ship has
  the lab, so `benches()` is three there and the two craft tests say
  so.

## A piece of armour is a resource in the hold and an instance everywhere else

`crates/world/src/armour.rs`. `ResourceId::Helm = 15`, `Kevlar = 16`,
`LegGuard = 17` are locker class like a medkit, made at the workbench
(`RECIPES[7..=9]`), sold nowhere, and the playtest ship carries one each
— so buying, selling, crafting, mass and the shelves needed nothing new.
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
that moves gear, so `world_checksum` (pieces after the hostile list,
health to a hundredth, `next_piece` too) sees what the room sees.

The five commands are `Stow { who, cell }`, `Fetch { who, kind:
FetchKind::Piece(id) | Resource(code) }`, `Equip { who, cell }`,
`Unequip { who, part }`, `Discard { who, cell }`, server-shaped like
`SetCraftTarget`. A stow or a fetch wants the Bim within `data::REACH`
(two tiles) of a container that takes the thing — `container_takes`: a
bench whose part is a locker-class cabinet (the armoury, the drug lab;
the smelter holds nothing), a shelf for shelf goods *and* for armour and
weapons, a cold store for food; `Aboard::containers` lists them by the
room's indices and `Game::within_reach` measures — and is refused
`OutOfReach` (14) otherwise; `PackFull` (15), `NoRoom` (16, the class
full; `NoRoomAboard` is the same wall met buying) and `Broken` (17) are
the rest. Equipping wants no container. A fetch by resource of an armour
kind takes the *least* damaged piece; a fetch of a weapon resource is
`Item::Weapon(kind)` — `armour::weapon_resource` is `WeaponKind::
resource()` looked up in `ResourceId::ALL`, the way `resource_of` reads
`ArmourKind::resource()`, so the room's table is the one table: the
handgun being the laser pistol and the four after it their own (18–21),
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
  `REACH`, so a test of "out of reach" stands the Bim at the helm
  (`helm_spot() + aboard.offset`, through `put_for_probe`) rather than
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
  the row is a courtesy, not the gate. `World::free` is what the
  container windows show, not raw `cargo`, since a fetch can only take
  what is not spoken for.

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
reads `9 + bandages` for the suit, the three pieces, the four weapons
and the handgun made.

## A loot is a command across two rooms, and a resident's piece is new to the world

`Command::Loot { slot, who, source: LootSource, cell }` takes one thing
off a body into `who`'s pack: `cell` a `bims::combat::LootCell` code
(the body's pack 0–8, then head, body, legs, weapon 9–12), `source`
either `LootSource::Crew(i)` — an index into the crew's room — or
`Resident(i)` — an index into the *station's* room, since the two keep
separate rooms while docked (`armour.rs`, `code()` 0 and 1, `who()`,
`from_code`). `World::loot` asks, in this order: the body *down* — dead
or out cold, `World::is_down(source)` off whichever room it lies in
(`Game::is_down`) — else `Refusal::NotDown` (18); the looter alive, awake,
aboard and within `data::REACH` of the body — `in_reach_of_body(who,
source)`, else `OutOfReach`; a free pack cell, else `PackFull`. Then the
body's room does the stripping (`Game::take_from_body`) and the crew's
room the taking in (`Game::give`), `mirror_pieces` runs, and
`WorldEvent::Looted { who, source_kind }` (38, `who + 10 * kind`) is
said. Nothing goes *onto* a body. Two things to keep straight:

- **Down and reach are asked when the command lands, not when the
  window opened.** A crewmate who came round while the Loot window was
  up is refused `NotDown`; the app's `Body { down, reach }` snapshot is
  a courtesy for the rows, read off `is_down`, `loot_cells` and
  `in_reach_of_body` every frame, the way `hold_of` reads `in_reach`.
- **A piece off one of the station's people gets a fresh id.** The
  residents' room numbered it as it liked (`Piece::new(1, …)` in the
  probe), and that id would collide with the ship's; so `loot` pushes a
  new `Piece` under `next_piece`, `at: Pack { who, cell }`, with the
  health the fight left it, and hands the looter `piece.item()` — the
  *world's* id. A piece off a crewmate is already on the list and
  `mirror_pieces` finds it in the new pack. A weapon or a stack is a
  plain item either way; a looted weapon goes into the pack and is
  equipped from there, as now.

Where a body lies is `World::body_position(source)` in the crew's room's
units — a resident's through `Aboard::from_station`, the inverse of
`to_station` — so the app can `send_to` the looter beside it; `None` for
a resident once the rooms have parted, which shuts the window. `visit`
tells the joined room which residents are down every step
(`Aboard::visit(&positions, &down)` → `Game::set_visitors_down`, after
`set_visitors`, which clears it), so a body among them is `HIT_VISITOR`
under a right-click; the fresh `Aboard` an undock builds knows no
visitors at all.

`an_unconscious_crewmate_is_looted_and_an_awake_one_is_refused` (Kate bled
to under the line, the helm off her head with its 9 health, the checksum
of two worlds parting and meeting again) and
`a_resident_down_in_the_fight_is_looted_of_its_weapon_and_its_helm` (the
fight run to `EnemyDown`, James walked over with `send_to`, the pistol
and a renumbered helm in his pack) are the tests.

## A mercenary is an extra body in a friendly station's room, and hired it is crew that costs money

`crates/world/src/mercenary.rs`. A station whose people are not enemies
may have hired hands living among them: **extra** bodies past
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
`mercenary::how_many(worth, start_worth, map_seed)`: one for every half
of the start worth grown by, plus one on `MERCENARY_CHANCE` (0.4) off
the seed, at most `MERCENARIES_MAX` (4) — so at the start worth a
station has one or none, and a richer crew finds more. Nothing new is
hashed for it: it is a function of the seed and the worth when the room
opens, like the gear. Derelicts and enemies' stations have none; the
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
with `LootSource::Resident`, the loot's own check — `NoBunk` 20,
`Unaffordable`) takes the body out of the station's room (`take_crew`,
the one removed, `adopt` the rest back — every errand ashore dropped
once, the way a docking drops the crew's; `down` and `fee` shrink with
it) and adopts it into the crew's at the same spot on the joined deck
(`body_position` → `stand_at` → `adopt`), coverall kept; its worn pieces
become pieces of the world's under fresh ids (`Where::Worn`), re-issued
through `Game::issue`; `health`, `crew_down`, `crew_locked`,
`Aboard::crew` and `Ship::crew_count` grow by one (`on_ship_changed`
for the dynamics); the first month comes off `money` and a
`mercenary::Hired { who, fee, due, owed }` goes on `World::hired`, in
`world_checksum` after `reinforcements`. `WorldEvent::Hired` (39).
`World::hire_offer(who, resident) -> Option<Offer>` is what the app's
Hire window reads every frame — fee, in reach, affordable, a bunk,
docked — so it can say "walk over first" before the command is refused.

**Wages are stage 2's second half**: `pay_wages` right after the clock,
every step: a `Hired` whose `due` has passed is paid (`MercenaryPaid`
40, `due += MONTH`, thirty days) if the money covers it; else it is
`owed` (`MercenaryLeft` 41, said once) and, at a berth with the rooms
joined, `dismiss`ed at once — out of the crew's room the way it came in
(indices after it move down one: `hired`, the pieces' `who`), its pieces
off the list, and back into the station's room as a mercenary for hire
at the same fee if that room has a bunk, else gone. Away from a berth it
sails on owed until it is paid or the ship docks. A mercenary is never a
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
stays; `stand_down` lifts it). The app: the desk's menu row (`Trade`)
walks the Bim over and opens the trade window
(`CrewPanels::trade_requested`); the window greys its rows and offers
"Walk over" while nobody of yours is at it (`Session::at_the_desk`,
`walk_to_desk`). `trading_wants_somebody_at_the_station_s_desk` pins it.
The desk moved `REFERENCE_CHECKSUM` (every station's layout has one)
and the walkability contract covers it.

## The world is bounded by the ship, and money by the dock

Two rules carried straight over from the design phase into the game, and both
are easy to lose:

- **Money only works while docked.** `Buy` and `Sell` are refused anywhere
  else, with `Refusal::NotDocked` — and *holding station beside* a station is
  not docked either, which wants an airlock. The trade window — and the
  Station button on the tray that opens it — is hidden rather than
  disabled, because a panel full of dead buttons is a panel nobody can
  tell is dead on purpose.
- **What is on the shelf is the station's, not its kind's.**
  `worldgen::Stock` is a bit a resource on the `StationBlueprint`, carried
  onto `world::Station::stock` and asked by `buy` (`Refusal::NotSoldHere`),
  `Session::sold_here` and the editor's `market`. `StationKind::sells` is
  still the ceiling; under it `Stock::roll` puts the `STAPLES` on every
  shelf and rolls the rest at `STOCKED_CHANCE` off the station's own branch
  of the contents stream. A test that buys something at the spawn buys a
  staple, or reads the shelf first the way
  `a_station_only_sells_what_its_kind_sells` reads the kind.
- **Reserved fuel is not the crew's to sell.** It has been promised to a trip
  already under way and there is nowhere out there to buy more.
  `World::can_modify_part` says the same thing about the tank it is sitting
  in, and about engines and thrusters while a trip is in the air: those three
  would change a trip that has already been quoted.

## The room is aboard the ship, and it is the same room

`crates/game` is a library as well as the room's cdylib, and `world` runs
it: `crates/world/src/crew.rs` holds a `bims::game::Game` laid out from the
accepted design by `bims::aboard` and steps it in stage 5 of `World::step`
— `Game::simulate` once per world step, at one sixtieth of a real second,
which is the room's own frame at 1x. The Bims aboard are the room's Bims:
needs, errands, the galley, the heads, the bay, the diary, all of it, with
the cold store stocked from the cargo when the world opens. Nothing was
copied; `world` imports `bims`, and `ship` imports it too, for the
painter.

Five things that hang off that and will bite:

- **`Game::render` is split from `Game::simulate`.** The room's own page
  calls `update`, which is both; the world calls `simulate` per step and
  the ship painter calls `aboard.render()` once per frame. At 24x that is
  one picture a frame rather than twenty-four.
- **The room draws its fixtures and the ship painter draws the rest.**
  `bims::aboard::drawn_by_room` names the parts the room has pictures for —
  the galley, the heads, the table and seats, the bunks, the bay, the locker
  — and `world_paint` skips those tiles and re-emits the room's whole draw
  buffer turned with the ship (`room_aboard`). The room's night wash and
  its deck plate are off aboard (`Room::shell`); the ship owns the sky.
- **Every fixture is used from the south.** The room's stations stand the
  Bim *below* the counter, the pan and the basin, and above the bay, as the
  classic layout had them; a part turned to face another way is used from
  the wrong side. `aboard.rs` says so at the top. Fixing it is the room's
  stations learning a direction each.
- **At most as many of a crew as the layout has bunks are simulated.** A
  Bim's index is its berth and its seat; the room has a berth per bunk and
  a seat per chair of the design, and the classic room its `BERTHS` and
  `SEATS` of two. A layout with none of one gets a single stand-in.
- **The RNG order in `Game::new` is pinned by every probe.** The classic
  room draws each Bim's start position *between* the Bims, from the one
  stream; `with_room` takes a closure for exactly that reason. Drawing them
  all first reshuffled every seed and failed `probe.rs` on
  "somewhere that counts as deck" — an hour's diagnosis for a two-line
  reorder. Any change to `Game::new` wants `probe`, `diary`, `sweep`,
  `crew` and `neglect` run against it.

The heads aboard have no compartment — `Bath::aboard` is a pan and a basin
with the walls and the door zero-sized off the map, `closed_door` never
anything, and both used from the deck side. The room's nav grid takes every
other blocking part as a solid (`Layout::others`), and every tile inside
the deck's bounding box that is not deck, so an L-shaped ship does not get
a room that thinks the missing corner is floor. `the_crew_live_aboard` in
`crates/world/src/tests.rs` runs the playtest ship six game hours and
asserts the Bim went somewhere and never left the deck.

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
thing that went wrong. The map also rings whatever the helm is aimed at,
off `Game::aimed`, which is set with the preview and read by nothing else.

`STATION_SHARE` went from a quarter to three fifths and a system rolls for
a second and a third station (`MORE_STATIONS`); the rolls are drawn whether
or not they take so the stream stays in step. That is a re-pin of the four
`worldgen::fixture` checksums and of `world::fixture::REFERENCE_CHECKSUM`
and not a `GENERATOR_VERSION` bump — the shares are deliberately off the
bump list, since no layout changes shape.

## The ship is flown from the helm, and a post is not an order

`World::can_command(slot)` is true only while that player's crew member is
within `HELM_REACH` (a tile) of the first helm's use spot — `helm_spot()`,
`at_the_helm(slot)` — and Confirm, Brake and Abort ask it; speed and
trading do not. **Brake and Abort are one command** (`Command::Abort`,
`net.stop()`) behind two buttons that are never both live: Brake for a
ship `Travelling` and not already stopping (`Plan::aborting`), Abort
for one `CastingOff` or `Undocking`. **Change target** is host-only — it
clears the aim and opens the map, which is what a click on the map did
already — and crosses no seam. Slot *i* is Bim *i*, the same pairing as the bunks. Two consequences:

- **Every test and harness that confirms a trip has to put a Bim there
  first.** `World::man_the_helm_for_probe(slot)` /
  `ship_man_helm_for_probe(slot)` stand one at the seat without the walk;
  `set_off` in `crates/world/src/tests.rs` does that and runs the departure
  through. And it has to be done **again before a later Confirm**: the Bim
  goes off about its errands — at 24x a short trip is an afternoon — and
  `ship-check.mjs` and `simulation-check.mjs` both re-man the helm before
  the second trip for exactly that reason. The screen's own way is the
  walk: a Confirm, Brake or Abort on the strip at the top of
  `crates/app/src/screens/game.rs` is a `HelmOrder` held in `pending`
  while `World::order_to_helm(slot)` walks the Bim there, sent through the
  seam the frame `at_the_helm(slot)` says so, and followed a step later by
  `World::stand_down(slot)` (`Game::stand_down`: the post off, nothing
  else), so the Bim goes back to its errands and the seat is the job's.
  There is no Take the helm button any more, and aiming on the map wants
  nobody anywhere — it is only a preview.
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

## Leaving a station is three states, and arriving is one

`ShipState` has `CastingOff`, `Undocking` and `Docking` beside the three it
had, codes 3, 4 and 5; `world::data` has `UNDOCK_MINUTES`, `DOCK_MINUTES`
and `CASTING_OFF_LIMIT`. A Confirm at a berth is refused straight away if
`plan_from_here` fails, and otherwise begins `CastingOff` with the target in
`Ship::pending`: every step `Aboard::send_everybody_home` posts the
station's people ashore (`ashore`, the corridor inside the station's port)
and walks the crew back (`gangway`, the deck inside the ship's), and
`everybody_home` is asked against the **ship's** design, not the joined one.
Then `unjoin_rooms`, and `Undocking` pushes the ship straight out along
`way_out` — the station's `face()` — by `undock_distance()` (its own span),
eased, heading untouched; `set_off` plans the trip from where that ends. A
trip to a station is aimed at `hold_point`, the same spot the push-off ends
at, and `finish` hands a docking plan to `Docking`: half of `DOCK_MINUTES`
sliding and turning onto the berth's heading to the hold point, half
straight in, then `dock_at`. Two things that bit:

- **"Docked" is two questions now.** The painter and the airlock ask
  `ShipState::alongside()` — docked *or* casting off, the rooms joined —
  and trade, `Session::docked_at` and the station panel ask `Docked` alone.
  `settle_residents` skips both. Anything new that matches `Docked` has to
  pick one.
- **A Confirm at a berth is not `Travelling` on the next step, and a trip to
  a station is not `Docked` the step the plan ends.** `until_stopped` in the
  world tests runs on through `Docking`; the harnesses wait on
  `ship_world_state() === 2` before reading a plan and on `!== 5` before
  reading a dock. A test that reads either the step after is reading the
  wrong state, not finding a bug. `REFERENCE_CHECKSUM` moved with all of
  this and with the second player standing at the helm in `reference_run`.
- **Nobody is aboard at the start**, so a probe of the walk ashore has to
  bring a resident onto the ship first — `bring_a_resident_aboard` in the
  world tests — or the ship casts off in the same step and the probe
  measures nothing.

`ship-layout.mjs undocking | docking` are the pictures. The map keeps its
zoom between visits (`Game::set_mode` no longer refits), and the crew and
the station's people are told apart by `character::Uniform` — the coverall
is the room's, the yoke and the hair are the person's; `Residents::open` is
the one place the station's is put on.

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
every use spot and every open deck tile, for every kind at three seeds. Run
it after moving anything in the layout; `validate` will not tell you.

Knock-ons: the layout is one plan sized by kind, and the seed decides only
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
middle, so `stage_fight_for_probe`'s spots and the ashore spot are on
open deck. Every tile in any block is frame and deck, and one with any
of its eight neighbours outside them all is outside wall (`skin`) —
which is why a room keeps `ROOM_GAP` (two) of void from the lobby and
the hub beside it: a room touching the hub's corner would open into it.
Partitions are explicit `Wall` runs down the shared hull column with a
two-tile `Door` (`column`): the inner rooms' onto the arm, the outer
rooms' through the partition, the west rooms' doors towards the hub and
the east rooms' towards the lobby so none faces another across the
corridor. A barricade of sandbags stands `BARRICADE_OUT` (four) tiles
out from the hub's skin in each arm, three of the five tiles from
alternate walls. Sizes: Relay 48 (the smallest the rooms fit at),
Outpost 52, Derelict 54, Refinery 56, Orbital 64, the arena 72 with
`ARENA_BUNK_COLUMNS` (4) columns of bunks in the quarters, since a
column holds five there and the garrison is sixteen. `CASTING_OFF_LIMIT`
is an hour, for the walk back from the far lobby. The tank stands a row
up from the wall — it is filled from below its left-hand column, and a
tank against the wall has its spot in the skin (`UseSpotBlocked`).
There are no chamfers: a diagonal piece where two blocks meet would be
a pinch the navigation cannot walk, and the picture's round hub is a
square one here. `nav_map_of_a_station` (ignored) prints the plan as
digits; run it after moving anything, and the walkability contract
after that.

## Construction is stage 7, and the materials never leave the hold until the part goes down

`crates/world/src/build.rs` is a `BuildSite` — kind, origin, rotation,
the same three numbers a design-phase placement is — and what has been
carried to it (`delivered`) or is in somebody's arms on the way
(`carrying`). `Command::PlaceSite`/`CancelSite` are the seam; `builds`
and `next_site` are in `world_checksum`; `WorldEvent` codes 27–30 are
`SitePlaced`, `SiteCancelled`, `Built`, `BuildLost`; `Refusal` 10–13 are
`UnderWay`, `WontFit`, `NoSuchSite`, `UnderConstruction`. Every step the
world hands the room one `bims::game::Build` **per site**
(`build_orders`): its tiles in room units (the offset added, like the
helm), `haul: Some((resource code, units))` for the first material short
of the recipe that the hold has any free of — a `HAUL_LOAD` at most — and
`minutes > 0` when everything is there and the part would go down now.
Stage 7 drains `take_picked`/`take_dropped`/`take_returned`/`take_built`
and moves the count. Things that bit, or would:

- **A "delivered" load is a reservation, not a move.** `World::free(id)`
  is what is aboard less every site's claim, and `sell`, `can_make` and
  the haul offers all ask it; `finish_build` then calls
  `build_from_cargo` — which takes `Edit::Plate` now, for plating that
  lays its own frame — and the whole recipe leaves the hold in one go.
  So mass is conserved at every step, a cancelled site frees everything,
  and a room taken apart at dock or undock (`drop_loads`) drops what was
  in the arms back onto the count without the room saying anything.
- **Every site is on the room's list, wanting nothing or not.** The
  first cut only listed sites with something to do, and a Bim carrying a
  load found its site gone from the list at `CarryToSite` — `site_stand`
  had nothing to stand beside — and gave the load up, every trip.
- **The room's copy is a step behind the world.** `Construct`'s `leave`
  takes the site off `Room::builds` and `DropMaterials`'s clears its
  `haul`, or the room re-offers the very site it just finished within the
  same step — `consider_errand` runs after the chain ends — and the
  world's `Built` arrives with a fresh chain already on the way to
  nothing. `building_under_way` skips a chain that `is_done()` for the
  same reason.
- **`can_place_site` is `apply` on `design_with_sites()`** — every
  pending site laid on first, in order — and then `validate`: a site is
  refused (`SiteRefusal::Fault(code)`) when the ship *would then* raise
  an error it does not raise now. A wall on the hob's use spot is the
  case; the app says the issue line. The room works the sites in order,
  so a wall on plating that is itself a site waits for the deck
  (`minutes` stays 0 until `apply` on the real design goes).
- **The ship and the building keep off each other.** Sites are placed
  and worked only while `at_rest()` (`Docked | Holding`); `confirm`
  refuses `UnderConstruction` while any site `begun()` or the room says
  `building_under_way()`. A bare blueprint holds nothing. Note that with
  one crew already aboard a Confirm at a berth is `Undocking` in the same
  step — `the_ship_does_not_move_while_built_on…` learnt that — so "under
  way" is checked there rather than at `CastingOff`.
- **A part built relays the room under the crew** — `relayout_room` →
  `Aboard::relayout` → `Game::relayout` → `Room::relayout` — keeping
  every errand, the dirt, the crops and the doors' locks; docking is the
  only thing that still takes the room apart. Joined, the joined design
  is recomputed through `docking::join` and the offset does not move (a
  join's shift is the station's corners against the ship's *build area*).
- `BUILD_MINUTES_BASE`/`_PER_UNIT` and `HAUL_LOAD` are in `data.rs`.
  Adding sites to the checksum moved `REFERENCE_CHECKSUM`.

The room's half — the two chains, the stand spot, the suit — is in
`crates/game/CLAUDE.md`; the blueprint and the Build tab in
`crates/ship/CLAUDE.md`. `a_site_on_the_deck_is_hauled_to_and_built_by_the_crew`,
`a_site_beyond_the_hull_is_built_in_a_suit`,
`the_ship_does_not_move_while_built_on_and_is_not_built_on_while_moving`
and `a_site_is_refused_where_the_designer_would_have_refused_it` are the
tests.

## Two things worked out once, not once a step or once a frame

- **The power budget** is `World::power_budget`, set at `start` and in
  `on_ship_changed`, which is the only path a part joins or leaves the
  design by (`shipdesign::apply`, at the one `self.ship.design = next`).
  `run_power` and `power()` read it. It used to be
  `shipdesign::power_budget(&design)` every step — a union-find over
  every tile of the grid — and was two thirds of a docked step.
- **`ShipDesign::part(id)` is a binary search**, because `parts` is in
  ascending id order (`parts_are_in_id_order` in the shipdesign tests).
  The painters ask it per tile per hull per frame (`hull::diagonal_at`,
  `shadow`, `hull_tiles`), and the linear scan it was made a station's
  picture 2.4 ms a frame. What the ship painter still spends per frame
  is drawing the station's hull from scratch — a few hundred µs — and
  that picture never changes; it is the next thing to cache if a frame
  is short again.
