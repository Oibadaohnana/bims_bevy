# The ship's rules

Notes on `crates/shipdesign` — parts, layers, power, cargo, money, the hash and
the validator. The designer that draws it is `crates/ship/CLAUDE.md`; the world
that flies it, `crates/world/CLAUDE.md`.

> **Since the port to Bevy (September 2026):** the browser host is gone.
> Every name table this file points at in `web/*.js` is now
> `crates/app/src/names.rs`; the pages are the screens in
> `crates/app/src/screens/`; the `bims_*` and `ship_*` exports are methods on
> `bims::game::Game` and `ship::Session`. The node harnesses
> (`scratchpad/*.mjs`, `smoke.mjs`, `stub.mjs`) are gone with the host — a
> check this file says lives in one of them lives nowhere now unless it says
> so in `crates/app`'s tests or `./check smoke`, and is worth restoring as a
> unit test if the thing it guarded moves again.

## A new part needs six edits, and the compiler catches three

`PartKind` for the variant, `PARTS` in `crates/shipdesign/src/parts.rs` for
the row, `PART_COLORS` in `crates/ship/src/paint.rs` for the colour,
`PART_NAMES` in `crates/app/src/names.rs` for the word, `PART_GROUPS` beside it for
which heading it lives under in the designer, and `BUILD_GROUPS` for which
category of the game's Build tab (`every_buildable_part_is_in_one_build_group`
pins that one). A part with a recipe made at it is a seventh:
a row in `recipes::RECIPES`, and `aboard.rs` makes it a bench off that. The three arrays are fixed-length, so those
three are compile errors; the two tables in the host are not, and
`scratchpad/ship-check.mjs` is what catches them. Same shape as `memory.rs`'s `What` and
`work.rs`'s `Job`, and for the same reason: no strings cross the boundary, so
the ship knows `PartKind::Hob` and only the host knows "Hob".

The row itself is seven decisions, and five of them are easy to get wrong by
leaving them at the default:

- **`layer` and `requires`.** What the part *is* and what has to be there
  already. `defs_are_sound` refuses a part that needs its own layer and one
  that needs nothing unless it is the frame; nothing else can catch a hull
  part that quietly asks for deck.
- **`recipe`.** What it is made of, which is also **what it weighs** — there
  is no mass column. See the next section.
- **`shields`.** Whether it keeps the radiation out. Defaulting to `false` is
  safe; defaulting to `true` on something that is not hull puts a hole in
  every exposure check that will never be noticed.
- **`capacity`.** A class of storage and how much, or `None`. A container
  with no capacity holds nothing and refuses every purchase.
- **`thrust` and `torque_thrust`.** What it pushes with and what it turns
  with, and `defs_are_sound` insists they are **exclusive**: thrust on
  `Engine` and nowhere else, turning force on `Thruster` and nowhere else. A
  part that did both would make "which engines are burning" — and therefore
  the power bill — a different question for every design.
- **`power`, `thrust_power` and `charge`.** Whether it draws, and how much.
  Leaving a workstation at nought is a machine that runs in a brownout and
  never needs the conduit under it; see "Power is a column" below.
  `thrust_power` is the engines' draw **while they burn** — `ENGINE_POWER`
  on the small one, in proportion to thrust on the heavy one, and
  `defs_are_sound` insists on the one ratio — kept apart from `power`
  because it is not paid all day; see "There is no fuel" below.

The first three are fixed-size arrays, so leaving one out is a compile error.
The last two are not, and both fail quietly — which is what
`scratchpad/ship-check.mjs` is for. It counts palette buttons against
`ship_part_count()` and fails a name that is missing or still reads `Part 7`,
and the page builds its rows off that count rather than off `PART_NAMES`, so a
part left out of `PART_GROUPS` turns up under **Anything else** instead of
disappearing. Same arrangement as the room's work panel.

An `IssueCode` is the same trap with worse consequences: an issue whose code
has no line in `ISSUE_LINES` is **dropped from the page**, exactly as a diary
entry with no `MEMORY_LINES` is. What catches it is the row count against
`ship_issue_count()`, and nothing else would.

The game half has four more tables with the same rule: `PLAN_ERRORS`,
`REFUSALS`, `PHASE_NAMES` and `EVENT_LINES`, plus `BODY_KIND_NAMES` and
`STATION_KIND_NAMES` for what is on the map. A `WorldEvent` whose code has no
line in `EVENT_LINES` is dropped from the log the same way.

## Power is a column, a flood and one number

`PartDef::power` is signed — `REACTOR_OUTPUT` on the reactor, negative on
what draws, nought elsewhere — and `PartDef::charge` is `BATTERY_CHARGE` on
the battery and nothing else. `supplies()`, `draws()` and `stores()` are
the questions; `defs_are_sound` keeps each on exactly its kind the way it
keeps thrust on engines, and insists everything `parts::essential` names
(life support, doors) draws. A new consumer is one row: set `power`.

`shipdesign::power::networks` is the rule. A part is on a network when a
tile of its footprint carries conduit on the **utility** layer — no
adjacency, under it or not at all — and networks are conduit runs joined
four-neighbour, plus one more join: two conduit tiles under one
*electrical* part are one network (the reactor is the wire between them; a
table is not). Only a network with a reactor is `live()`. `validate` warns
`Unpowered = 33` (every dark consumer in one issue, tiles = footprints) and
`PowerShort = 34` (one per live run drawing more than it makes, tiles = the
run); neither is an error, for the flight warnings' reason.

`World::run_power` is stage 6: `charge += (supply − draw − engines) · STEP_MINUTES`,
clamped to the wired batteries' storage, closed form — `engines` being what
the lit set draws this step, off the plan's effort (see "There is no fuel"). `Power::brownout()`
is `draw > supply && charge == 0`, and `World::powered(kind)` is what a
chain will ask: some part of that kind wired, and not browned out unless
the kind is essential. **Nothing aboard reads it yet** — the smelter will.
`Ship::charge` opens full, is clamped in `on_ship_changed` (a battery taken
off takes its charge), and is in `world_checksum`.

Both fixtures are wired — `REFERENCE_CONDUIT`, `PLAYTEST_BRANCHES` off the
spine in column 8 — which moved `REFERENCE_HASH`, `REFERENCE_PARTS`,
`PLAYTEST_HASH`, `PLAYTEST_PARTS` and `REFERENCE_CHECKSUM`;
`the_fixtures_are_wired` pins that neither warns and the playtest ship
draws **137 of 5 000** — six systems, the smelter, the workbench, the drug
lab, the armoury and the research desk — off **two reactors**, plus its
engine's 1 000 while it burns. The two reactors are from when one made
120: the first made 120 and had 3
to spare with the drug lab drawing 5 rather than the workbench's 15;
this note used to say a fourth bench aboard it was a second reactor or
a brownout, and the armoury (10) was that bench, so it came aboard with
a reactor of its own at `(13, 11)` and four more conduit tiles in
`PLAYTEST_BRANCHES`. Still one network, no warnings: `networks(&design)`
is one long and the supply is `2 × REACTOR_OUTPUT`, both in the same
test. `ship-check.mjs`'s `buildShip` lays the same run by
dragging conduit, which is an area tool like deck. **The station layout is
not wired**: nothing reads a station's power and it is only checked for
errors, so its reactors and batteries are still furniture.

## A new resource needs six edits, and the compiler catches three

`ResourceId` in `crates/physics/src/data.rs` — appended, `ALL` and
`RESOURCES` grown with it — then `trade_price` and `storage` in `economy`
(both `match`es, so a missing arm is a compile error), `CARGO_SLOTS` in
`crates/shipdesign/src/design.rs` (an array length; `cargo_is_the_right_length`
pins it, and **every design hash moves** because the cargo is hashed at
fixed length — re-pin `REFERENCE_HASH`, `PLAYTEST_HASH` and
`REFERENCE_CHECKSUM` — and `worldgen::fixture::REFERENCE_CHECKSUMS`
too, because a station's shelf is in the galaxy checksum and every kind
that sells the new thing rolls it), `RESOURCE_NAMES` in
`crates/app/src/names.rs` (the length is pinned against `ResourceId::ALL`
by its tests, and `ITEM_TIPS` beside it the same), and a picture in
`crates/app/src/icons.rs` — `icons::resource` is a `match` on
`ResourceId`, so a resource with no icon is a compile error, and
`every_resource_and_every_item_has_an_icon` draws each into a windowless
painter. Then decide **who sells
it**: `StationKind::sells` in `crates/worldgen/src/data.rs` is the only
place that is written down, and its test enumerates every pair. Galvum is
the outposts' alone, an emitter is nobody's, a derelict sells nothing;
fibre is the orbitals' and a bandage the orbitals' and the refineries'.
Shotgun (18), AutoRifle (19), SniperRifle (20) and Schword (21) are the
last four added — locker class, sold nowhere, weighing their armoury
recipes; see "Armour is three recipes" below — and Helm (15), Kevlar
(16) and LegGuard (17) before them
are the worked example: locker class like the medkit, at 16, 28 and 8 a
unit — two metal, three metal and a galvum, one metal, so the three
workbench recipes conserve mass — priced 300, 900 and 150 (the metal in
each, since a station only ever buys one), and **sold nowhere**. That
last is the edit that is easy to miss: `sells` falls through to `_ =>
true`, so a resource left out of its "made, never sold" arm is on every
lived-in station's shelf and every galaxy checksum moves. The arm and
its mirror in `what_each_kind_of_station_sells` both name the three.
(Fibre, 13, and Bandage, 14, were the pair before: a crop in the
cold-store class at 1.0 and a dressing in the locker class at 2.0, so
the drug lab's recipe conserves mass too.) `Refusal::NotSoldHere = 9` is the world's answer and
`EditError::NotSoldHere = 17` the design phase's — the editor asks
`Editor::market`, the spawn station's kind, before it asks `apply`, since
`shipdesign` knows no stations. `Session::sold_here` is the one export both
phases read.

## A part weighs its recipe, and there is no mass column

`PartDef::recipe` is what a part is made of — metal and components, never ore
and never food — and `parts::part_mass` adds it up. **That is the only place a
mass comes from.** A separate `mass` field existed and is gone, because two
numbers that are meant to agree are two numbers that will not: a part heavier
than what went into it is mass appearing out of nothing every time one is
built.

What that buys is in `crates/shipdesign/src/materials.rs`, and it is a
contract rather than a feature. Building moves the recipe out of the hold and
into the part; deconstructing moves **all** of it back. Total ship mass is
unchanged either way, and changes only through trading while docked, food
eaten or grown, and crew joining or leaving — nothing is burnt in flight. `build_from_cargo`
and `deconstruct_to_cargo` are that rule written down and tested against every
part in the table. `world`'s construction step calls the first — see
`crates/world/CLAUDE.md` — with a Bim and a site in the middle: what is
"carried to a site" is a reservation on the hold, and the whole recipe
leaves it in one `build_from_cargo`, which takes `Edit::Plate` as well now
(`recipe_for` says what plating costs: the deck, and the frame where the
tile has none). Nothing calls the second yet.

Three things that go with it:

- **Money only works at a station.** The design phase is instant and paid in
  euros because it is docked at the spawn station. Away from one, `price` means
  nothing and a part comes out of the hold or is not built. `price` and
  `recipe` are deliberately **unrelated** — nothing derives one from the other.
- **The centre of mass is not conserved, only the total.** Parts sit at their
  tile centres and cargo and crew at the centre of mass, so welding the hold
  into an engine at the stern moves it. Nothing reads that yet; `mass.rs` is
  one figure.
- **Deconstruction can be refused for want of a shelf.** The materials have to
  go somewhere, and a ship whose only shelf is the part coming off has nowhere
  — `NoRoomAboard`, measured against the design *after* the removal. Losing
  them quietly would be the one thing the contract forbids.

## Four layers, and the stack is a chain of `requires`

A tile holds at most one part per `Layer`: `Structure` under everything,
`Floor` on it, `Object` standing on that, and `Utility` running through the
tile alongside whatever is standing in it. `PartDef::requires` is the whole of
the rule — `Some(Layer::Structure)` for the frame's own plating and for the
hull parts that stand straight on it, `Some(Layer::Floor)` for everything
else, `None` for `Structure` itself.

Three consequences that are not obvious:

- **Removal is the same rule read backwards.** `remove` asks every layer of
  every tile whether the part there `requires` this one's layer. That is why
  there is no list of "what holds up what" anywhere: there is one column of
  the table and both directions read it.
- **A removing drag peels one layer.** `Editor::drag_parts` hands back the
  top part of each tile in the drag and nothing under it — object, then
  utility, then floor, then structure — so a right-click on a hob takes the
  hob and leaves the deck, and the next click takes the deck. Across the
  tiles of one drag the parts are ordered top down, or a deck would be
  refused because the neighbouring tile's object was still standing on it.
  Clearing a plated tile to nothing is therefore two clicks, and a harness
  that expects one to do it is wrong.
- **Deck plating lays its own frame.** `Edit::Plate` is structure-if-missing
  then floor, one edit, both prices; the plating tool sends it (`placing`
  in `crates/ship/src/editor.rs`), and the ghost asks the same edit. There
  is no frame button in the palette — `NOT_A_TOOL` in `crates/app/src/screens/game.rs`, and
  `ship-check.mjs` counts one fewer button than parts. Bare frame is still
  a part: plate and peel the deck, which is how the fixtures and the ghost
  view get it.
- **Connectivity is asked of the structure layer alone.** A ship is its
  frame; everything else stands on it. Walking every occupied tile instead
  would let a wall touching nothing but another wall count as holding the
  ship together.

## An engine fires into space, and is worked on from any side

Two rules on the main engines, both in `validate.rs`, both with the
painter behind them so they are seen before the checks panel says so:

- **`IssueCode::ExhaustBlocked = 32` is an error.** `exhaust_tiles(part)`
  is the row of tiles straight behind the bell — aft is grid-down at `R0`,
  the way the engine does *not* push, and turns with it — and every one of
  them has to hold no frame (`exhaust_blocked`). An engine inside the hull
  with deck behind it is firing into a room; the fix is to stand it in the
  skin, its last row on the stern ring (decked, since an engine stands on
  deck; sealed, since an engine shields), the way the fixtures now do —
  `reference` has `REFERENCE_ENGINE_STERN`, the harness ships peel two
  stern tiles and put the engine at `(7, 16)`. That moved `REFERENCE_HASH`,
  `REFERENCE_CHECKSUM` and the harness builds.
- **`parts::any_side_will_do(kind)` — every engine — is used from the ring
  round its footprint**, corners left out, and the validator wants **one**
  ring tile to be standable rather than all of them (`use_spots` and
  `reachability` both branch on it). Any other part still stands where its
  table row says; the rotation test that pinned the engine's single spot by
  hand pins the bay's instead.

`paint::exhausts` washes the exhaust tiles of every placed engine in the
flame's colour with a tongue pointing aft, and in the warning colour with
a cross on every tile of the ship it would cook; the ghost gets the same
whether or not it is placeable, and marks only the ring tiles a body could
stand on. `ship-layout.mjs given | spots | ghost` are the pictures.

## Radiation is a warning that outranks the errors

`validate::exposure` floods in from a **one-tile ring outside the build area**,
4-neighbour, through every tile whose object-layer part does not shield. A
reached tile holding any part is exposed.

- **The ring is outside the area**, or a ship built flush to the edge would be
  sealed by a wall that does not exist.
- **Never 8-neighbour.** Two hull parts meeting at a corner seal it; eight-way
  would leak through every diagonal join and no hand-drawn hull would pass.
- **A shielding part is never exposed itself**, because the fill cannot enter
  it. That falls out of the rule rather than being a special case.
- **A door does not shield, and neither does a plain wall.** `OutsideWall`,
  `Airlock`, `SensorArray` and `Engine` do, and that list is in
  `shielding_and_storage_are_where_they_are_meant_to_be`.

It is `IssueCode::RadiationExposure = 24`, it is **first in the issue list**,
and the host styles it `grave` — louder than an error, which is a deliberate
exception to "an error blocks Accept and a warning does not". It does not
block; it shouts, because it is the only fault on the list that kills people
and a player who never opens the checks panel is exactly the one about to
accept it. The deck carries the tint whether or not anybody is pointing at
the row, which is why `paint::faults` skips outlining that one issue — a wash
and a grid of markers saying the same thing makes neither legible.

`scratchpad/ship-layout.mjs exposure` is the only way to look at it.

## Cargo is part of the design, not something beside it

`ShipDesign::cargo` is units per `ResourceId`, bought and sold through `apply`
like any other edit. Four things hang off that and all four are load-bearing:

- **It is in `design_hash`**, after the parts. Two players accepting are
  accepting the same ship *and* the same manifest, so a Buy clears every
  Accept exactly as a wall does.
- **It is in `Budget::spent`.** Parts and goods come out of one pool, which is
  the whole of the decision the design phase asks anybody to make.
- **It is in `Session::mass`.** A ship with full tanks is heavier and the
  acceleration on the handoff screen says so before Accept, not after.
- **It is bounded by the ship, never by the station.** Supply is unlimited;
  what refuses a purchase is the pool or the hold. `economy::storage` says
  which class a resource goes in and `PartDef::capacity` says what provides
  it — a shelf and a second shelf are two hundred units of one class, not two
  holds.

A storage part with something in it cannot be removed. Sell first; the error
is `StorageInUse` and the sentence says so.

## The designer's rules are in `shipdesign`, and `apply` is the only door

`crates/shipdesign` renders nothing and exports nothing to wasm. `crates/ship`
draws and takes input. The split is not tidiness: a native server has to be
able to say whether a ship is legal without a canvas, and two implementations
of that would be two different games.

Three rules the crate is built around:

- **`apply(&design, &budget, edit) -> Result<ShipDesign, EditError>` is the
  only way a design ever changes.** It hands back a new design, so a refused
  edit cannot leave a half-changed one behind. Nothing in the UI mutates
  `parts` directly, and neither should anything else.
- **Remaining money is derived, never decremented.** `Budget` holds the pool
  and nothing else; what is left is worked out from the design every time it
  is asked. A counter kept alongside would drift from the ship the first time
  an edit was refused or replayed.
- **The occupancy grid is rebuilt from `parts` on demand.** A cached one is a
  second source of truth about what is where.

## Money is one pool, in whole euros, and every sum is checked

Nobody starts with stores. Each Bim brings money, all of it goes into one
pool, and every part has a price in euros paid out of that pool —
`crates/economy` is the arithmetic, `PartDef::price` is the table, and
`shipdesign::Budget` is what spends it.

Four things that are load-bearing rather than tidy:

- **`Money` is a `u64` of whole euros and never a float.** Two players have
  to end up with the same pool down to the last euro, and a rounding rule is
  something two machines can disagree about. The euro sign and the digit
  grouping exist **only in the host** — `euros()` in `crates/app/src/screens/builder.rs` and in
  `crates/app/src/screens/game.rs`, one copy each, and nothing else makes either.
- **Overflow is an error, never a wrap and never a saturation.** A wrap hands
  somebody a fortune; a saturation quietly makes two different lobbies agree.
  `economy::starting_pool` refuses both, and a crew of nobody as well.
- **The solo bonus is a lump, not a multiplier.** A ship for one costs what a
  ship for four does — the hull, the galley and the heads are the same — so a
  lone player gets `SOLO_BONUS` on top of their own purse. Scaling it would
  miss the point, which is that the *fixed* part of a ship does not scale.
- **Money crosses the wasm boundary in two `u32` halves**, `_hi` and `_lo`,
  the same way `design_hash` does. That goes for `Session::design` as well: what
  goes *in* is what one Bim brings, and wasm does the multiplying, so the pool
  is worked out in exactly one place for the browser and for a native server
  both.

## An Accept is for a hash, not for "the design"

`design_hash` sorts the parts by `(y, x, kind)` and leaves the **ids out**, so
the same layout built in two orders gives the same number. That is what makes
an Accept comparable between players. Any successful edit by anybody clears
every Accept, which is the whole protocol — there is no way to be holding one
for a ship that is no longer on screen.

Three things this depends on and that are easy to undo:

- **Part ids only ever climb**, and a removed one is never reissued. An Edit
  in flight that names it is then refused rather than landing on whatever took
  its place. Stage 5 also wants that order: Bim *i* spawns at bunk *i*, bunks
  in id order.
- **The hash is written out by hand** — FNV-1a over little-endian `u32`s. Not
  a `Hash` derive and not `DefaultHasher`: those are explicitly allowed to
  differ between builds, and this number crosses between machines.
- **The build area is in the hash.** The same parts in a bigger square are a
  different ship, and an Accept must not carry across a resize.

## What a trip needs is five warnings, not an error

`validate` now warns about a missing thruster, airlock, sensor array and
forward engine. All four are warnings, like the engine warnings before them: a
ship that cannot fly is still a ship you can live on, and refusing to let a
player accept one would be the design phase having an opinion about how to
play. `NoEngineOnAxis` **became** `NoForwardEngine` and kept its code — the
codes cross the wasm boundary — because the autopilot flies the start–arrival
line and a sideways engine is dead weight.

`shipdesign::fixture` therefore has **two** ships. `reference` is one you can
live on and deliberately cannot fly, which is what those warnings are tested
against; `flyer` is `reference` plus four thrusters, an airlock and an array, and it
is what `flight` and `world` measure their scenarios against; its engine
runs on the reference's reactor, wired down column 8. Only `reference`'s hash is pinned, because only that
one is about two targets agreeing.

## Two engines, and "is it an engine" is asked of the table

`PartKind::Engine` and `PartKind::HeavyEngine` are both main engines: five
times the push for three and a half times the weight, so the heavy one is the
better engine per tonne and the worse one to carry, and a 3×4 for €75 000
against a 2×3 for €20 000. **Nothing lists the two kinds.** `PartDef::pushes()`
(`thrust > 0.0`) is the question, and `turns()` is its partner for the
thruster; the validator, `mass::engines`, `flight::dynamics`,
`World::can_modify_part` and the painter's `exhaust` and `part` all ask it,
so a third size is one row in the table. `defs_are_sound` is the one place
the kinds are named, because it is the thing checking the column.

**Power goes as thrust, not as a count of engines.** `PartDef::thrust_power`
is `ENGINE_POWER` (1 000) on the small engine and five times it on the heavy
one, and `defs_are_sound` pins the ratio to `thrust` as one figure across
the table. A flat rate per engine would have made the heavy engine the only
engine worth having; as it is, a heavy engine flat out wants twice a basic
reactor, so on one it is throttled to under half and still out-pushes the
small one. `the_heavy_engine_is_faster_and_dearer_over_the_same_hop` in
`crates/flight/src/tests.rs` pins the trade: swapped into the flyer it
crosses the longest hop sooner, draws the whole of the reactor's spare doing
it, and a unit of push costs the same a minute on either.

## There is no fuel: the engines run on the reactor (September 2026)

`ResourceId::Fuel`, `PartKind::FuelTank` and `Storage::FuelTank` are gone,
and — there being no save format yet — the discriminants after each were
closed up rather than left as holes: `Components` is `2`, `LifeSupport` is
`21` and everything after it one less than it was, `Storage` has four
classes. That moved every pinned hash (`REFERENCE_HASH`, `PLAYTEST_HASH`,
`REFERENCE_CHECKSUM`, the galaxy checksums with a `GENERATOR_VERSION` bump
to 5, since every station's shelf bits shifted) and the fixed-length tables
in the app (`PART_NAMES` 39, `RESOURCE_NAMES` and `ITEM_TIPS` 22,
`STORAGE_NAMES` 4, the `[u32; 4]` fill readouts). `IssueCode` 30
(`NoFuelAboard`) and `PlanError` 4 and 7 are holes: those codes cross to
`names.rs` and are not reissued.

What replaced it is in `power.rs`. **An engine is on the network** like a
consumer — `electrical` includes `pushes()`, `unpowered()` lists a dark
engine, and a dark engine pushes nothing — but its draw is
`Network::engine_draw`/`Budget::engine_draw`, kept apart from `draw` because
it is only paid while the engine burns. `power::thrust(design)` is the rule
the flight is built on: per **facing** (only one set burns at a time), the
reactors' `spare()` — supply less the day-long draw — over the set's full
draw is its throttle, capped at one; `Thrust::engines` is the wired engines
with thrust × throttle, which `flight::dynamics` accelerates by in place of
`mass::engines`, and `forward_power`/`backward_power` is what the lit set
draws a minute, carried on every burning `Segment` for the world's power
stage. `REACTOR_OUTPUT` is **2 500** — a basic reactor is a fusion reactor
now, named so in the app — so one feeds two engines flat out with five
hundred over for the systems, and `FUSION_OUTPUT` is four of those.
`BATTERY_CHARGE` is still half an hour of a reactor. `EnginesThrottled = 35`
is the warning when a set cannot be fed flat out (parts = the throttled
engines); `PowerShort` is now practically unreachable with the consumers in
the table, and its test builds a hundred and twenty-six life supports to
reach it. `the_engines_push_what_the_reactor_can_feed` in `tests.rs` is the
rule end to end. The playtest ship's engine was on row 16's conduit already;
the reference's got three more tiles of column 8 (`REFERENCE_CONDUIT` is
23), and its tank's deck is bare.

The `yard` in `crates/shipdesign/src/tests.rs` stocks **exactly** the heavy
engine's recipe, because it is the heaviest recipe there is and the
"one unit short" case builds one and is then refused a wall. A part with a
bigger recipe than 150 metal and 100 components wants that fixture moved with
it, and a fourth shelf.

## The required-fixture list is a mirror of the chains

`REQUIRED` in `crates/shipdesign/src/validate.rs` is table, cold store,
worktop, hob, dishwasher, toilet, basin, with bunks and chairs counted against
the crew. That is **exactly what the room's chains walk to** and nothing else
— and it is now also exactly what `crates/game/src/aboard.rs` maps onto the
room's fixtures, plus the locker and the bay. It is not a design decision
about what a ship should have.

If a chain changes what it walks to, this list and `aboard.rs` change with
it. A ship validated against a stale list is a ship whose crew starve standing
in front of the fixture nobody required; `aboard.rs` puts a fixture that is
missing on the worktop rather than panicking, which is a room that stands up
and a crew that cannot use it.

## A corner piece is a whole tile, and only the picture is a triangle

`PartKind::DiagonalWall = 29` and `DiagonalOutsideWall = 30` are the wall and
the outside wall cut across their tile at forty-five degrees. **Every rule
treats them as the straight kind**: one object a tile, `blocks_movement`,
`requires: Some(Layer::Structure)`, and the hull one `shields` — the
exposure fill is four-neighbour, so a staircase of them touching corner to
corner is as tight as a straight run, and nothing in `validate` or the room's
nav knows the other half of the tile is empty. Do not "improve" that by
letting the fill or a body through the open half; the fill would then leak
through every chamfer and the nav would plan through a wall.

Which half is solid is the part's `Rotation`, through one function:
`parts::solid_corner` — `R0` is the **south-west** corner, then clockwise
with `R`. The draw format grew its one new shape for this,
`KIND_TRIANGLE = 2`: the bottom-left half of the box at `rot` nought, which
is `R0`'s corner, so the triangle's turn *is* the part's turn. Both
`draw.rs` files, both replay loops (`crates/app/src/screens/game.rs`, `crates/app/src/names.rs`) and
`ship-layout.mjs`'s SVG dumper know the kind; `hull::corner` is the one
place the turn, the outward normal and the hypotenuse's angle are worked
out, and every painter reads it — the designer's frame and object and ghost,
the hull's diagonal plate and the shadow along the hypotenuse, `fittings`'
diagonal bulkhead, and the frame triangle under a corner piece in both
views. A second copy of that arithmetic is a chamfer filled on one side by
one painter and bevelled on the other by another.

Two things that follow:

- **A chamfer's end tiles replace straight hull.** A cut of `c` from a
  corner at `(x0, y0)` is void where `(x - x0) + (y - y0) < c`, corner pieces
  where it equals `c`, and the ship beyond — so the run's two ends sit *on*
  the skin rows, in place of the plating there. `ship-check.mjs`'s chamfer
  section peels those two before laying the run, and the frame out of the
  void corner as well, or the tiles left behind are exposed and the count
  never reaches nought. `playtest_outline` in `fixture.rs` is that rule
  written down (the station plan has no chamfers any more).
- **A diagonal drag is a staircase**, `editor::diagonal_line`: one tile a
  step along the shorter of the two distances, every tile at the ghost's
  turn. A removing drag is still a rectangle — it would take the deck beside
  the run too — so the harness peels a run one tile at a time.

## The playtest ship is three compartments, and its numbers are pinned

`playtest_ship()` is a chamfered bow with the bridge in it, the main deck,
and engineering aft of a second bulkhead, with a two-tile doorway in each —
the ASCII on `playtest_ship()`'s doc comment is the quickest way to see it,
and it is hand-copied from a dump, so redraw it when the ship moves. The airlock is in the **starboard** skin at
`(17, 11)`, so it docks at heading nought and the station is to starboard,
which `simulation-check.mjs` relies on; the engine is at `(9, 16)` with the
two stern ring tiles decked so its bell *is* the stern and nothing of the
ship is aft of it. The stern row holds the workshop — the workbench at
`(11, 17)` and the smelter at `(14, 16)` to starboard of the engine, the
drug lab at `(6, 17)` to port of it, all `R180` so they are worked from
the row forward of them, the row aft being the stern. The **armoury** is
not in the workshop: it stands on the main deck at `(14, 7)`, `R0`,
along the bridge bulkhead to starboard and forward of the bunk, worked
from `(14, 8)` the way the galley is worked from the row below it, and
its **second reactor** is at `(13, 11)`, under the bay by the airlock —
both placed clear of `(9..=11, 11)`, the tiles the world's tests lay
their own conduit and armoury on. The cargo carries **5 bandages and 6
fibre** (`PLAYTEST_CARGO`) so the dressing seam can be tried from the
first minute, **one helm, one kevlar and one pair of leg guards** so
the armoury's grid opens with something in it to equip, and **one of
each of the four weapons** — a shotgun, an auto rifle, a sniper rifle
and a schword — so each can be put in a hand and looked at. The locker
class is what those had to fit in — the suit locker's two, the drug
lab's six and the armoury's **eight** are sixteen, with a suit, five
bandages, three pieces and four weapons in them, which is the "13 of 16
in the lockers" the armoury's window says. The armoury's cabinet was
four until the weapons came: twelve slots with nine used would have had
one weapon quietly refused by `apply` rather than fail anything, and
the armoury is the cabinet weapons live in, so it grew rather than the
bandages shrinking — the same way, before the armoury, the lockers
were eight with six used and the third piece would have been the one
refused. The drug lab went to port rather than beside
the workbench because the one tile left there is one wide, and it draws
5 rather than the workbench's 15 because the first reactor had 8 to
spare before the armoury (see "Power is a column"). Moving anything moves `PLAYTEST_HASH`
and `PLAYTEST_PARTS` in `fixture.rs`, the hob's tile in `ship-check.mjs`
and `ship-layout.mjs given` (`(6, 7)` on the twenty grid, `(16, 17)` on
the lobby's forty), and `world::fixture::REFERENCE_CHECKSUM` whenever the
world's tests say so. `parts_are_in_id_order` takes the first part *from*
the middle that will come off rather than the one exactly there, because
which tile is exactly in the middle moves every time the ship does, and
one build it was a frame tile with something standing on it.

**The combat ship is the playtest ship with a crew of five aboard.**
`fixture::combat_ship()` is `playtest_ship()` plus four bunks at
`COMBAT_BUNKS` and four chairs at `COMBAT_CHAIRS`, put down through
`apply` after the cargo; `COMBAT_CREW` is 5 and it validates clean for
that many (`the_combat_ship_sleeps_a_crew_of_five`). The bunks are on
the **bridge** — one lying along the bow to port at `(6, 2)` `R270`,
two standing against the life support at `(13, 2)` and `(13, 4)`, one
beside the battery at `(5, 4)` `R180` — because the main deck's aft
rows are two deep above the bulkhead and a row of bunks there seals
the deck between them into pockets (`UseSpotsCutOff`), and a bunk in
engineering cuts the compartment in two. Two things to know before
moving one: the room does **not** read a bunk's use spot — `bed_side`
in `room.rs` gets in from whichever side faces the middle of the room,
and a lying bed in the east half is got into from the *north* — so a
bunk's get-in side has to be deck by that rule as well as the
validator's; and a chair is a seat anywhere, blocking nothing, so the
three extra chairs sit along the aft bulkhead rather than at a second
table there is no room for. Its hash is not pinned; `PLAYTEST_HASH` and
`PLAYTEST_PARTS` do not move for it. What it is for is
`crates/world/CLAUDE.md`'s arena.

`ship-layout.mjs simulation` and `... station` are the pictures — `mode=1`,
no ship built by hand, the playtest ship docked at its spawn an hour in.
They are the only moments that show the fittings and the station's rooms at
all, since every other game moment builds the flyer.

## The trading desk is a station's part, and the ship never needs one

`PartKind::TradingDesk = 36` is the thirty-seventh row: a table's
footprint `(2, 1)` worked from `(0, 1)` and `(1, 1)`, three metal, €600,
unpowered, `blocks_movement` and — it is a counter — in the
`blocks_sight` exceptions beside the table. Nothing in this crate reads
it; `world::station::build_layout` lays one in every station just inside
the port and `world::World::at_the_desk` is the rule that a buy or a sell
wants a crew member within reach of one. It is offered in the palette
and the Build tab (Crew / Furniture) like any part, so a player who
builds one on a ship has a desk nobody trades across — the rule reads
the joined deck's desks, and a ship's own is on it too while docked,
which is harmless: it is still the station's shelf that is bought from.
The six edits were the six: the enum and `ALL`, `PARTS` (37),
`PART_COLORS` (a wooden counter), `PART_NAMES`, `PART_GROUPS`,
`BUILD_GROUPS`, and a picture in `fittings::trading_desk`.

`PartKind::Sandbags = 37` is the row after it: a tile, one metal, €150,
half a body's height — **walked over and seen over**, so neither the nav
grid nor a line of sight stops at it, and `parts::is_cover` marks it the
one part that is **low cover**: the room's `Sight::covered` has a body
close behind it dodge half the shots from across it. No use spots, since
nothing is worked at it. Under Structure in the palette and the
Build tab, and `fittings::sandbags` is the picture: three staggered
courses of bags over a dark ground. The stations lay a barricade of them
in every corridor arm (`crates/world/CLAUDE.md`).

## `blocks_sight` is the movement rule minus the low furniture

`PartDef::blocks_sight()` in `parts.rs` is a method, not a column: what a
body cannot walk through stops its eyes too, except the furniture it sees
over — bunk, worktop, hob, dishwasher, table, toilet, basin, hydro bay, the
helm's console, a battery — and a door counts as opaque here because that
is what it is shut; open or shut is the room's to know. `defs_are_sound`
pins that nothing walked through blocks sight (bar the door) and every
wall does. The room reads it in `aboard::layout_of` and nowhere else.

## The drug lab is the fourth bench, and a bandage is the one recipe that heals

`PartKind::DrugLab = 35` is the workbench's footprint and use spot — `(2, 1)`,
worked from `(0, 1)` — with a lighter draw (5: a press and a steriliser,
not a lathe), a cabinet of its own (`Storage::Locker`, six, the armoury's
precedent for a bench that is also a container) and a recipe of five metal
and six components. `recipes::RECIPES[6]` is what it makes: two **fibre**
to one **bandage** in a quarter of an hour, `vents: false`, and the bandage
weighs the fibre, which `every_recipe_holds_together` pins along with
`at(DrugLab)` being one recipe long. Nothing here knows what a bandage
*does* — that is `bims::health`, where a bandage closes every wound on one
part of a body — any more than it knows what a handgun shoots; the table
is what goes in, what comes out and how long. The cabinet is the reason
the playtest ship can carry its five bandages: until the armoury came
aboard its only other locker was the suit locker, two slots with a suit
in one, and the armoury's eight are what the three pieces of armour and
the four weapons sit in now (see "The playtest ship is three
compartments").

Two edits the six-edit rule did not name, because they are outside the
crate and cannot be left out: `PART_COLORS` in `crates/ship/src/paint.rs`
is indexed by `kind as usize` in every painter, so a part the table is one
short for panics the designer on its first frame; and `trade_price` and
`storage` in `economy` are exhaustive matches on `ResourceId`, so the
resource half is a compile error there. The drug lab has a picture now
— `fittings::drug_lab`, its `PART_COLORS` green-white as the bench top
with a vial rack, a flask and a small still on it (`crates/ship/CLAUDE.md`)
— so a block on the deck is once again a part somebody forgot to draw.

## Armour is three recipes at the workbench, and the vest is not the kevlar

`recipes::RECIPES[7..=9]` are a **helm** (two metal, half an hour),
**kevlar** (three metal and a galvum, three quarters) and **leg guards**
(one metal, twenty minutes), all at `PartKind::Workbench`, none venting,
each weighing exactly its metal — which is why the three masses in
`physics::RESOURCES` are 16, 28 and 8 and not round numbers. They went to
the workbench because that is where the user put them, and the armoury's
own vest (`RECIPES[4]`, four metal and two components) **stays**: a vest
is a `Vest`, a count in a locker with no state, and the kevlar is a piece
with a health of its own. `every_recipe_holds_together` pins all of it —
the table fourteen long, `at(Workbench)` five (components, the emitter
and the three pieces), the vest still at index 4, and the mass of every
piece equal to its inputs — and it is the test to extend for a fourth
piece, which is one more row appended (the index crosses the seam) and
one more `ResourceId` in the locker class.

The four weapons were exactly that, appended after the leg guards, back
at the armoury. `RECIPES[10..=13]` are the **shotgun** (four metal, two
components, three quarters of an hour), the **auto rifle** (three metal,
three components, an emitter, an hour), the **sniper rifle** (four
metal, two components, two emitters, an hour and a quarter) and the
**schword** (one metal, one component, two emitters, an hour), none
venting; `every_recipe_holds_together` pins each row's inputs, output,
minutes, station and mass. The masses in `physics::RESOURCES` — 36, 46,
68 and 42 — are what the inputs weigh (metal 8, components 2, an
emitter 16), and not the 12/10/14/6 the spec first said, because the
table conserves mass and the spec said to move the masses rather than
the inputs; heavy, because an emitter is. What each weapon *does* is
`bims::combat::WeaponKind::stats`, keyed on the resource code the same
way (`WeaponKind::resource()`: 9, 18, 19, 20, 21).

What a piece *does* — its health, its protection, the slot it is cut for
— is `bims::combat::ArmourKind::stats`, keyed on the resource code (15,
16, 17), and the rule that a piece is a resource in a container and an
instance everywhere else is the world's (`World::pieces`,
`crates/world/CLAUDE.md`). This crate knows the three only as goods:
`CARGO_SLOTS` was 18 for them and is 22 for the weapons
(`cargo_is_the_right_length`), the hold
counts them like bandages, `apply(Buy)` refuses a fourth when the
lockers are full exactly as it refuses a medkit, and a station only ever
buys them since none sells. **Adding them moved `REFERENCE_HASH` even
though `reference` did not change**, because the cargo is hashed at
fixed length — and the weapons moved it again, along with
`PLAYTEST_HASH` (the cargo grew, the parts did not: `PLAYTEST_PARTS` is
still 647) and `world::fixture::REFERENCE_CHECKSUM`; expect that of the
next resource too, and re-pin by running rather than assuming the
reference ship is untouched. `worldgen`'s `REFERENCE_CHECKSUMS` did not
move for the weapons, since a resource in the "made, never sold" arm is
never rolled onto a shelf — and the weapons are priced (1 000, 2 000,
3 000, 2 500) only so a station can buy them off the crew.

## Research is a tree in `research.rs`, and a key opens one node

`crates/shipdesign/src/research.rs` is the rules crate's half of what the
crew know: `Node` (eight, codes 0–7, appended never renumbered), `RESEARCH`
(a `NodeDef` a node — what it `requires`, its `tier`, whether it is
`locked` — wanting a key of its tier consumed **for it**; a key opens one
node, never a tier — and `minutes` of the AI's time, nought for
what is known at the start), `node_of_part`/`node_of_recipe` (the gates:
everything not named waits on `Survival`, so a new part is buildable
unless the table says otherwise), and `Research`, the state the world keeps
and hashes — `done` and `unlocked` as a boolean a node, `current` and
`progress`. `Research::unlock(node)` refuses a node with no lock, one open
already or one researched, so no key is spent for nothing.
`tree_is_sound` pins the table's shape; `the_research_tree_is_sound_and_the_crew_know_how_to_live`
and `research_runs_in_order_and_a_key_opens_a_node` in `tests.rs` pin what
a fresh crew know (every part but the four benches and the fusion reactor;
the bandage and the medkit and nothing else the benches make) and the
order. **`Research::begin` on a different node loses the progress**, and
so does `cancel`: the AI thinks about one thing at a time.

Two parts came with it, both the six edits. `PartKind::ResearchDesk = 38`
is a table's footprint worked from the tile below, seen over like the
trading desk, drawing 10, with a capacity of **one in a class of its own**
— `Storage::Research = 4`, the fifth class, which moved `Storage::ALL`,
`STORAGE_NAMES` and the two `[u32; 5]` fill readouts in the app — and
`PartKind::FusionReactor = 39` is a three-by-three that `supplies()`
`FUSION_OUTPUT` (3 500): `defs_are_sound`'s "supplies only the reactor"
became `matches!(kind, Reactor | FusionReactor)` and `power_is_made_held_and_drawn…`
names both. Its recipe is metal and components alone on purpose — an
emitter in it would put a keyless node behind the key. The playtest ship
carries a research desk at `(8, 14)` `R0`, on the spine in engineering's
forward row between the shelves and the heads, which was the one two-tile
spot on the ship whose use spot has deck on its far side — the aft
bulkhead, the combat chairs and the galley's row rule out every other —
and that moved `PLAYTEST_HASH`, `PLAYTEST_PARTS` (648) and the draw the
wiring test pins (137).

`ResourceId::ResearchKey = 22` is the key: mass 2, €5 000 (a station buys
one), `Storage::Research`, sold nowhere (`StationKind::sells`'s
"made, never sold" arm, so no galaxy checksum moved), which grew
`CARGO_SLOTS` to 23 and re-pinned both `REFERENCE_HASH`es. What it does is
the world's — `crates/world/CLAUDE.md` — and what it is in a pack is the
room's (`bims::combat::Item::Key`, two cells tall; `crates/game/CLAUDE.md`).
`KEY_CELLS` here is the pair the desk's slot and the pack's cells agree on.

**The medkit moved to the drug lab.** `RECIPES[5]` keeps its index and its
inputs and its `station` is `DrugLab` now: medicine is made from the first
day and the armoury is a locked node, so a medkit at the armoury would have
been no medkit until the key. `every_recipe_holds_together` counts the
armoury at six and the drug lab at two.

## The hyperdrive is bolted to an engine, and `hyperdrive.rs` is the rule

`PartKind::Hyperdrive = 39` (September 2026): a two-by-two on deck that
draws 50 all day like a system, €90 000, metal and components alone for
the fusion reactor's reason. `research::Node::Hyperdrive = 8` is its node —
after `FusionPower`, **locked**, so it wants a tier-one key like the
armoury and the emitters (`the_research_tree_is_sound…` names all three).
`hyperdrive::connected(design, part)` is the one rule: a tile of the
drive's footprint four-neighbour to a tile of a part that `pushes()`. Not
the conduit — the drive wants power too, and a dark one is `Unpowered`
like any consumer — but block against block. `unconnected(design)` is
what `validate` warns with `IssueCode::HyperdriveUnconnected = 36`
(parts = the loose drives), and `ready(design)` — connected **and**
`power::is_powered` — is what `world` asks before a charge, so the two
cannot disagree. `a_hyperdrive_has_to_touch_an_engine_and_be_wired` pins
it. What a jump is — the charge, the empty space — is
`crates/world/CLAUDE.md`. The app's part groups (`PART_GROUPS`,
`BUILD_GROUPS`) are written as `PartKind::X as u32` now rather than
numbers, after the fuel tank's retirement renumbered half of them.
