# Bims

A 2D top-down game written in Rust, on Bevy. It opens as a window on your
desk.

Two characters — Bims — pottering about a compartment on a ship. Either will
cook a meal from start to finish and clear up after it, take itself to bed, or
go and use the heads, entirely on its own account. A clock runs the whole time,
and the deck goes dark at night.

**You steer one of them.** James takes orders; Kate does not. See
[The crew](#the-crew).

Beside it there is a ship to design and a system to fly it round. The lobby
starts a [design phase](#the-ship-designer); accepting the design starts
[the game](#the-game) — one star system, the ship docked at a station in it,
and one clock everything runs on. The crew are aboard it from the first
step — one per player, each at their own bunk — and they are the same Bims
as in the room, living the same life on the ship you designed.

## Running it

```sh
nix run .
```

That builds the game and opens it. There are five things to run, and each is
a name rather than a flag:

| command | `cargo run` | opens |
| --- | --- | --- |
| `nix run .` or `nix run .#game` | `cargo run -- game` | the whole game, in order: the start menu, setup or a lobby, the world and a station to start at, the ship design, then the world docked where you said |
| `nix run .#simulation` | `cargo run -- simulation` | straight into the world on a prebuilt playtest ship, docked at a station |
| `nix run .#design` | `cargo run -- design` | straight into the ship design, the playtest ship given, docked where the simulation docks |
| `nix run .#room` | `cargo run -- room` | the behaviour test room — the Bims on a deck |
| `nix run .#test` | `cargo run -- test` | the simulation somewhere else each time: docked at a random station somebody lives on, in a random galaxy, with a mercenary for hire at the dock and bunks to spare for one |
| `nix run .#test_planet` | `cargo run -- test_planet` | `test` set down on a planet: the same random galaxy, landed at the settlement of a planet whose people are friendly |
| `nix run .#combat` | `cargo run -- combat` | the fight: the combat ship — fourteen crew, a gun in every hand — docked at the spawn rebuilt as the arena and made hostile, its people enemies, fifteen of them; a recruited crew member shoots at any it can see |

The `cargo run` forms build from the working tree, which is what you want
while editing — `cargo run --release -- test` is the same thing on the
release profile, which is the build to play on; `nix run` builds from the
git tree. The window wants a
display and a GPU: winit and wgpu open the window system's libraries and the
Vulkan loader at run time, and `shell.nix` puts them on `LD_LIBRARY_PATH`,
so a bare `cargo run` outside the shell may open nothing — `nix develop` or
`nix-shell` first, or let `./check` do it.

The rest:

```sh
nix develop            # a shell with the toolchain and the runtime libraries
nix build              # the binary lands in result/bin/bims
nix flake check        # builds it, runs every crate's tests, checks formatting
./check                # everything above and a smoke run of each window
```

`bims --self-check` prints whether this build agrees with the constants the
fixtures pin — the design hash, the world checksum, the money arithmetic —
and exits non-zero if it does not.

## The builder

`nix run .` opens here: what comes *before* the ship and the world — a start
menu, a game setup screen, and a lobby. It is `crates/app/src/screens/builder.rs`,
a screen of its own — the room's screen knows nothing about menus. Only the
World tab touches the galaxy, through `crates/lobby`: the galaxy is generated
and drawn there, and the rest of the screen runs without it.

**Play** goes straight to game setup. **Create lobby** opens a room other
people will one day be able to walk into, and a code field beside the two of
them is the other half of that: type a code, press Join, and it refuses out
loud. That refusal is the honest state of things — there is no transport yet.

The lobby is the settings screen with company. Down the left, four slots: you
in the first one as host, the rest open and waiting. Across the top, the room
code, set like something you read out to somebody. On the right, the same
tabbed tool the setup screen uses:

- **Game setup** — the money each Bim brings at €50 000, €100 000 or
  €200 000, and the ship you start with at 30 × 30, 40 × 40 or 60 × 60 tiles.
  €100 000 and 40 × 40 unless you say otherwise. Nobody starts with stores:
  everybody's money goes into **one pool** and the designer spends that.
- **World** — which galaxy, and where in it the game starts. A **seed** —
  any whole number up to a `u64`, typed in decimal, with **New seed** to
  draw one — and a **galaxy type**: two-arm spiral, spiral, elliptical or
  round. Under them the galaxy itself, a thousand stars on a canvas: drag to
  pan, scroll to zoom, hover to read a star's name and class, click one to
  open its system on the right — the star at the middle, its planets and
  belts on their orbits, its stations as squares, each named with its kind
  and the body it hangs off. A system with a station usually has more —
  up to six, two of a kind allowed — and about three in ten of the ones
  somebody lives on are **hostile**: the people there are enemies, the
  diagram rings their square in red, the station list says so, and a
  crew cannot start at one. Stars with no station are dimmed, since the
  game cannot start there; they can still be looked at. **Start here** on
  a station makes it the pending start, marked on the map and named in
  the tab's header; **Random start** picks one anywhere that is not an
  enemy's. A new seed or a new type is a new galaxy and forgets the start.

  No distances, no travel times, nothing about what a station is like: the
  lobby is where a start is chosen, not where a system is explored.

One settings object sits behind both copies of the tool, so what you pick on
the setup screen is what the lobby shows and the other way about.

**Start** needs a station picked — until there is one the button is disabled
and says so — and then hands the game to the ship designer, which is a page
of its own; see below. What crosses is numbers in a query string and nothing
else: the money each Bim brings, the build area in tiles, how many players
there are, which slot you are, the seed in its two halves, the galaxy type,
and the star and the station the game starts at. `"large"` and `"full"` exist for the buttons;
what a game is *started with* is what crosses into the simulation, and no
strings do — which goes for the euro sign as well, so what is written down is
a bare count of euros.

### The multiplayer seam

`Net` in `crates/app/src/screens/builder.rs` is the whole of it: `create`,
`join`, `push`, `leave`, and what the screens read back from it. It is a local stand-in with the
shape a transport will have, and **nothing in the screens reaches past it** —
the slot list is drawn from what `net` says the players are, not from what the
lobby knows about itself. Giving it a socket is meant to be a change to that
object and to nothing else.

Two things it already does properly, because they are easy to get wrong later:
the settings tool knows how to be read-only, for a guest in somebody else's
lobby — a guest sees the host's world, can pan, zoom and inspect it, and can
**Suggest** a station, which rings that star on everybody's map, but cannot
change a thing — and the settings themselves are plain numbers: a sum of
money, a tile count, a seed, a type and two ids. Nothing but numbers can
cross into the simulation anyway.

## The ship designer

The lobby's **Start** goes here: the whole crew laying out **one ship**
together, on a tile grid, before anybody is aboard. It is not a command of its
own any more — a design phase with no lobby in front of it has no station to
start at, and the page says so rather than picking one: opened without a star
and a station in its query, or with ones the galaxy has not got, it shows
**Nowhere to start** and a link back to the lobby. It never falls back to
another spawn.
It is `crates/app/src/screens/designer.rs` over `crates/ship`.

**No Bims exist during this phase.** Placing a part and taking it off again
are both instant and free: nothing has been welded yet, and the money is only
being promised. That stops the moment everybody accepts — after that, every
change is a Bim's work.

A tile is 52 world units and holds at most one part per **layer**, of which
there are four:

- **Structure** is the frame the ship is built on. It needs nothing under it,
  and it is the first thing anybody lays: nothing else goes down without it.
- **Floor** is the deck plating you walk on. It needs structure.
- **Object** is the one thing standing in the tile — a wall, a bunk, an
  engine. Most need deck; the hull parts stand straight on the frame, which
  is what lets a ship be skinned before it is floored.
- **Utility** runs *through* a tile without filling it: power conduit, which a
  body walks over and a hob can stand on.

A wall is on the object layer like everything else, because a wall and a bunk
in one tile is equally nonsense. What each part needs under it is one column
of the part table, and removal reads the same column backwards: nothing comes
out from under something that is standing on it.

### Laying one out

The designer **opens on a ship**: the playtest ship, laid out in the middle
of whatever build area the lobby chose, whole and flyable, as a gift — the
pool is what the crew brought and none of it has been spent. Everything on
it can be moved, taken off or added to like anything you placed yourself,
and taking a given part off puts its price in hand as any removal does.
`?preset=0` opens an empty grid instead, which is what the harnesses use;
a grid too small for the ship (under twenty tiles) opens empty too.

The palette down the left is grouped the way a ship is thought about — hull,
systems, crew, galley, heads, storage, bay — rather than the way the enum is
numbered. The rows are built from what `shipdesign` says exists, so a part added
and forgotten in the grouping turns up under **Anything else** instead of
quietly not existing.

- **Click** to place. **Drag a rectangle** for the things you fill an area
  with — deck plating, conduit — **drag a line** for both kinds of wall,
  and **drag a diagonal** for the two corner pieces: a **diagonal wall**
  and a **diagonal outside wall** fill half their tile, cut at forty-five
  degrees, and a drag lays a staircase of them one tile a step, every
  piece turned the ghost's way. That is how a hull gets a pointed bow or a
  chamfered corner, the way spacecraft hulls in games of this kind do. To
  the rules a corner piece is a whole tile — one object, a body cannot
  pass, and the hull one seals its tile against radiation exactly as a
  straight plate does, since the exposure fill is four-neighbour and a
  staircase touching corner to corner is tight; only the picture is a
  triangle, and which half is solid is the piece's turn: **R** walks it
  round the four corners of the tile.
  **Deck plating lays its own frame**: there is no separate structure tool,
  because to anybody but the connectivity check the frame and the deck are
  one thing. A plated tile has both; the hull stands on the frame, deck or
  no deck.
- **Right-click peels the top part off a tile** and only that — a hob comes
  off and the deck stays, a second click takes the deck, a third the frame.
  A right-dragged rectangle peels every tile in it by one. Across the tiles
  the parts come off top down, or a tile's deck would be refused because the
  next tile's hob, in the same drag, was still standing on it.
- **R** turns the ghost a quarter clockwise. The footprint and the use spots
  turn with it, and the palette shows the turned size.
- **Middle-drag** or **WASD** pans; the **wheel** zooms. The view is clamped
  to the build area and a margin, and starts showing all of it.

A drag is applied as a run of single edits, and **a failing one is skipped and
counted rather than fatal**: a rectangle of deck over a half-floored room is
meant to fill the gaps and pass over the rest.

The ghost is green where the tool would go down and red where it would not,
and the readout by the pointer says the same thing in the same colour before
the click rather than after it. Resting on a placed part rings it and shows
its **use spots** — the tiles a Bim will stand in to use it. Those are only
ever shown for the part under the pointer; drawn permanently they would fill
the deck with markers.

### What it costs, and what it checks

Across the top is what is left of the crew's money, beside what there was to
start with. There is **one pool**: every Bim's purse goes into it, a lone
player gets a fixed bonus on top because a ship for one costs what a ship for
four does, and every part anybody places comes out of the same figure. Each
part has a price in euros. A removal hands back the **whole** price, and what
is left over at the end is money rather than cargo — it is not aboard, and it
does not count towards what the ship weighs.

What is left is always **worked out from the design**, never decremented as
parts go down: a refused edit, a removal and a replayed run of edits cannot
drift apart from what is actually on the ship. The sum itself —
`money_per_bim × players`, plus the solo bonus — is `crates/economy`, so the
game and the native server that will one day be authoritative arrive at the
pool the same way, in whole euros, with overflow an error rather than a wrap.

### Buying what the ship will live on

Under **Station** on the right is what there is to buy: ore, metal,
components, vegetables and tofu, at the spawn station's own two prices a
unit — what one *costs* here and what the desk *pays* for one, the
station's ask and bid (*Trading*, under the game, has the sum). Buttons
move one, ten or a hundred, and putting a thing back hands back what it
cost — nothing has left the dock, so there is nothing to lose on the
deal; the bid is shown, and is what a sale will fetch once the game has
started, but nothing is sold in the yard, only put back.

Two things bound a purchase, and the station's shelf is neither of them.
Supply is unlimited; what refuses an order is **the
pool** — goods come out of the same money the hull does, so a player who
spends everything on plating has nothing to load it with — or **the ship**.
Goods are stowed: food in a cold store, gear in a locker, everything else on
a shelf. The readout under the rows is how full each class is, and a ship
with no cold store cannot take food at all however much money there is.
Each of those is not a count but a **grid**, ten cells across and as many
rows as the parts aboard add up to — a shelf or a cold store is ten by
ten — and every thing kept there covers its footprint of it: a pistol a
row of two, a rifle seven, a sniper rifle the whole width; a vest four
by four, a helm two by four; a crate of vegetables one by two, a block
of tofu four by four. Goods that stack take one footprint a stack — ten
ore to a cell, twenty components — so what a shelf holds is its cells
times the stacks. A container's window lays its grid out as it is: drag
a thing to move it, press `R` on the way to turn it, and a thing goes in
only where there is a run of cells for it — so a full hold is tidied,
not counted.

What is bought is aboard from the moment it is bought. It is in the design
hash, so a purchase clears everybody's Accept the way a wall does; it is in
the ship's mass, so the acceleration on the handoff screen already accounts
for it; and a shelf with something on it cannot be taken off until it is sold.

### Everything is made of something, and it weighs what it is made of

Every part has a **recipe** — so many units of metal, so many of components,
and never ore or food, which are mined and eaten rather than built with. A part's mass is that recipe added up and there is no other
number: a wall is two metal, and two metal is what a wall weighs.

That is what makes construction a **move** rather than a purchase. Take two
metal out of the hold, put a wall on the frame, and the ship weighs exactly
what it weighed a moment ago — the materials have changed where they are and
nothing else. Take the wall off again and all two units come back; there is no
wastage, no scrap and no scrapping penalty. A ship's mass changes only by
trading at a station, by food being eaten or grown, and by crew coming
aboard or leaving — nothing is burnt in flight.

None of that is visible yet, because in the design phase there is a station
outside and everything is bought with money. **Money only works docked** —
that is where euros and materials swap for each other — and the design phase
happens docked at the spawn station, which is the whole reason a part can go
down instantly. Out between stations there is nobody to buy from, and what
gets built comes out of the hold or does not get built. A part's price in
euros and its recipe are deliberately unrelated: they are two different
transactions that happen to end in the same wall.

The rule is written down and tested now, against every part in the table, in
`crates/shipdesign/src/materials.rs`. Nothing calls it — the construction step
will, with a Bim doing the work.

### What it checks

Down the right is what is wrong with it. An **error** blocks Accept; a
**warning** is the design saying what it will be like to live with. Resting on
a row rings the tiles it names — a highlight, not a tooltip: nothing is said,
and it goes the moment the pointer moves.

The errors are:

- the ship is in more than one piece;
- fewer bunks or chairs than there are players;
- no table, cold store, worktop, hob, dishwasher, toilet or basin;
- somewhere a Bim has to stand is off the ship, has no deck, or is blocked;
- parts nobody could walk between — over deck, through doors, which count as a
  way through;
- an engine firing into the ship: the tiles straight behind its bell have to
  be open space, so an engine stands in the skin with its bell over the
  edge. The designer washes every engine's exhaust onto the deck as you
  build — flame-coloured out into space, warning-red with a cross on every
  tile of the ship it would cook — for the ghost as well as for what is
  placed, so you see it before the checks panel says it.

The warnings are no engine, no engine on some axis, no hydroponic bay, no
broom locker, no helm, nothing to eat aboard, a consumer nothing powers, a
conduit run drawing more than its reactor makes — and **radiation**.

**Having no engine is never an error**: a ship that cannot fly is still a
ship you can live on, and refusing to let a player accept one would be the
design phase having an opinion about how to play. An engine is worked on
from whichever side a body can get at — it has hull on three sides more
often than not — so it needs one free tile round it, not a particular one.

### Radiation, which is a warning and is louder than the errors

Hull keeps it out. The **outside wall**, the **airlock**, the **sensor array**
and the **engine** block shield; a plain internal wall does not, and neither
does a door — so a ship skinned in ordinary walls is a ship whose crew are
being cooked.

It is worked out by flooding in from outside the build area, four ways only,
through everything that does not shield. Every tile the flood reaches and
finds a part in is **exposed**, and the deck is tinted over every one of them
— not when you rest on the row, but always, because a player who has not
looked at the checks panel is exactly the one about to accept a ship with a
hole in it. The row sits first in the list and is styled louder than any error.

It does not block Accept. That is deliberate: it is a decision about how to
play, and the design phase does not take those. It only makes sure nobody
takes it by accident. Two hull parts meeting at a corner seal that corner —
the flood is four-way, so a hull drawn as a staircase does not leak at every
step of it.

That required list is a **mirror of what the room's chains walk to today** — a
meal is a cold store, a worktop, a hob, a table with a chair and a dishwasher;
a night is a bunk; a trip to the heads is a toilet and then a basin. It is not
a design. If the chains change, the list changes with them, and
`crates/shipdesign/src/validate.rs` says so at the top.

### Power

The reactor makes it, the battery holds it, and everything that draws — life
support, the helm, the sensor array, the cold store, the bay, every
door, every bench, and **every lamp** — has to be **wired**: a tile of it
carries conduit, and that conduit runs to a reactor. Conduit is on its own
layer and runs *through* a tile, under whatever is standing in it, so there
is no adjacency rule to learn: drag a run of it under the things that need
it, as you would drag deck, and end the run under the reactor. A conduit
run is a **network**; two runs that both end under the same reactor are
one.

Two warnings come of it, and neither blocks Accept, for the same reason the
flight warnings do not — a ship that cannot run its cold store is still a
ship you can live on, for a while. **Nothing powers this** rings every
consumer with no live conduit under it — a lamp among them, and a lamp
with nothing powering it is a lamp that gives no light. **This run draws
more than its reactor makes** rings the run: a reactor is two and a half
thousand a minute, an engine burning flat out takes a thousand of that,
and the lamps are the biggest of the day-long draws — a wall light 25, a
standing light 40, so the playtest ship's six wall lights and standing
light are 190 of the 327 it draws all day, more than its four benches
together. What the reactor has over after that is what the engines get, so
a ship lit from end to end on one reactor pushes a little less hard; and a
ship whose day-long draw goes over what its reactors make — a reactor
taken off for a rebuild, a run cut, or a big hull hung with lamps and
benches on one basic reactor — runs on its batteries until they are
flat, and then it is *the brownout*, below.

### Accepting

Each player has an Accept, disabled while anything is an error. An Accept is
recorded **against a hash** of the design — the parts sorted by position and
kind, with the ids left out, so the same layout gives the same number whatever
order it was built in. Any successful edit by anybody changes that hash and
clears every Accept, so there is no way to be holding one for a ship that is
no longer on screen.

When everybody's Accept matches the current hash the phase ends: editing locks,
and **the game starts**. Solo, one Accept settles it.

### The two crates behind it

- **`crates/shipdesign`** is the rules and nothing else: the part table, one
  `apply` that is the only way a design ever changes, `validate`, and
  `design_hash`. It renders nothing and knows nothing about a window — the
  native server that will one day be authoritative has to agree with the
  game about what a legal ship is, and `design_hash` has to come out
  **identical on both**. That is why nothing in its data or its hash is a
  `usize` or a float. Its unit tests are plain `cargo test`; the same
  constants are checked all at once by `bims --self-check`.
- **`crates/ship`** is the screen's half: the camera, the pointer, the ghost,
  the draw buffer, and `Session`, which is the design phase and the game it
  turns into. It decides nothing about what may be placed — it asks.
- **`crates/economy`** is the money: whole euros, the shared pool, what a
  station charges for a unit of anything, and which class of hold it goes in.
  Every sum in it is checked — overflow is an error, never a wrap.

Its multiplayer seam is the same idea as the builder's. `Net` in
`crates/app/src/screens/designer.rs` has a transport's shape, every Edit and
every Accept goes through it carrying the design hash it was made against,
and the host end applies messages in arrival order and reports a refusal back
to whoever sent it. No click handler touches the editor directly.

## The game

The design phase is the only phase before it. The moment the last Accept lands,
the ship is real: it is **docked at the station the lobby picked** with
whatever was left of the pool in the crew's hands, and from then on there is
one world, one clock and one loop.

That is worth saying plainly because it is the decision everything else hangs
off. The star system, the ship in it, and everything aboard it all advance
together in `World::step`, which moves the world by a sixtieth of a game minute
and nothing else. The crew are in it, and they are **the room** — the same
simulation as [the behaviour test room](#the-crew), laid out on your ship:
one Bim per player, spawned at their own bunk the moment the world opens —
Bim *i* at bunk *i*, in id order — and from then on hungry, tired, in need
of the heads, cooking at the hob you placed, eating at your table, sleeping
in your bunk, sweeping your deck and tending your bay, on the world's clock,
inside that step. The cold store opens holding what you bought. Construction
is in the same step — [the crew build what you lay out](#building-aboard),
out of the hold — and health is still to come there; a second clock would
be two simulations that disagree, and the failure would read as a ship in
two places.

The room's fixtures are drawn with the room's own pictures — the fridge, the
hob and its pot, the pan, the bunk with its rails — turned with the ship, and
the Bims are named over their heads by the app (`CREW_NAMES` in
`crates/app/src/names.rs`, the room's two names first, so the pair you met on
the deck are the pair aboard).

Three limits, honestly stated: the room has two bunks' and two seats' worth
of identity, so at most two of a crew are simulated; every fixture is used
from the south — a hob with a wall below it is a hob nobody can reach; and
the room's navigation cannot walk a one-tile corridor, so a ship built with
them is a ship whose crew freeze in them. The playtest ship has none of
those problems, which is not an accident.

### Flying it

The ship is flown **from the helm**, and the strip across the top of the
screen is where it is flown from. Open the map with **M** — it keeps
whatever zoom it was left at — click a planet, a station or a bare point in
space, and the strip quotes the trip before anybody commits to it: how
long, what the engines will draw off the reactor and at what throttle, and
whether it ends docked or holding alongside.
Aiming is only looking, from anywhere. **Confirm** is the order, and an
order wants somebody at the seat: the crew member you steer walks to the
helm — the strip says so, with a Cancel beside it — and the order goes
through the moment they get there; then they are let go, to eat and sleep
as the day demands, and under way the helm is a job the crew hand round
among themselves. Brake and Abort are on the strip too and walk the same
walk. With the map closed the strip is the trip instead: where the ship is
going, a bar of how far it has got, and how long is left — or the berth,
while it is at one. A new click on the map under way is a new target, and
Confirm flies it.

From a berth, Confirm is first of all a **departure**. The crew come back
aboard — walking, nobody teleported — and the ship waits at the berth until
they have (or for half an hour, and then leaves without them). Then it
**pushes off**: straight out of the station's door by its own length,
heading untouched, and only then is the trip planned, from where it has got
to, so the turn towards the target is the trip's own first phase. **Abort**
while it is still casting off and it stays tied up; abort during the push-off
and it holds where the push-off ends. That is all Abort is for: a departure
that has not become a trip yet.

A trip is one straight line and four phases:

1. **Align** — turn to face the arrival point, thrusters flat out for half the
   turn and flat out the other way for the rest, so it starts and ends still.
2. **Burn** — the engines, all the way to the changeover.
3. **Brake** — whichever is quicker: flip end over end and burn on the same
   engines, or push on the backward engines without turning at all. A ship with
   no backward engine always flips; a ship with strong ones never does.
4. **Arrive** — docked if it was aimed at a station and has an airlock,
   holding beside it otherwise. Docking is the push-off backwards: the trip
   is aimed at the point in front of the station's door and ends a station
   radius short of it, the ship slides the rest of the way, turning onto the
   berth's heading, and then straight in along the door's line until the
   collars meet. Five minutes, read off the clock like everything else.

There is no speed limit and no coasting in the middle. It is flat out to the
changeover and braking from there, which is the same shape the world generator
laid every system out against.

**Brake** brings it to rest along the line it is already on — it never
reverses — and holds there; a ship already stopping has nothing left to
brake with, and the button says so. A Confirm while it is under way is a
redirect (Change target, click, Confirm), which is the same thing followed
by a fresh departure: it stops first, and the moment it has stopped it sets
off again on its own. Only the latest confirmed
target is kept, and the route line on the map is drawn in the colour of
whoever set it.

### There is no fuel: the engines run on the reactor

A ship flies on its **fusion reactor**. A main engine is wired like anything
else that draws — conduit under it from a reactor — and while it burns it
draws a thousand a minute (the heavy engine five times that, in proportion
to its push); a basic reactor makes two and a half thousand, so one feeds
two engines flat out with five hundred over for the ship's systems. What
the reactor has over after the day-long draw is what the engines get: more
engine than that and they run **throttled**, sharing the spare, and the
ship is slower — the designer says so (`Engines throttled`) and the helm's
quote says the throttle. An engine on no live conduit pushes nothing at all.
Nothing comes out of the hold for a trip; what a burn costs is that the
batteries charge at the surplus less the engines' draw while they are lit,
and nothing during an align or a flip, because thrusters draw nothing. The
exhaust is blue — plasma, not a flame — and the reactor glows brighter the
harder it works, so an engine lighting can be seen at the reactor as well
as at the stern; the **Electricity** view writes over every drainer what
it draws, the engines with what they draw at that moment against what they
would flat out.

### Light, and the dark

A deck no light reaches is **dark**, and in the dark the crew see ten
tiles. Two lights are built like any part: a **wall light** hangs from a
bulkhead or the hull (the designer says so if it has no wall at its back)
and reaches seven tiles, a **standing light** stands anywhere and reaches
nine; both **draw power** — 25 and 40 a minute — and want conduit under
them like the cold store does. A lamp on no live network is dark, and every
lamp on the ship goes out in a brownout (*The reactor, the batteries and
the brownout*), which is how a brownout is noticed: the deck goes dark
round the crew and they see their ten tiles. The lamp is whole — it comes
straight back with the power — where one shot out is glass. A station's
lamps are its own and stay lit whatever the ship is doing. A wall stops light the way it
stops the eye, so a room behind a bulkhead is dark for all the lamps on
the other side, and the shadow a lamp casts round a table is a shade —
light, but there. What the crew see is drawn as it is: a smooth cone from
each of them with the walls' straight edges, the dark shaded where they
see it, the fog where they do not. The default ship and every station
come lit; a ship of your own wants lamps, or its corridors are ten tiles
long to whoever walks them.

### Comforts

Three parts do nothing but make a deck nicer to stand on — a **small
plant**, a **big plant** and a **picture**, under *Comforts* in the
designer and *Furniture* on the Build tab — and what they do is lift the
crew's **Surroundings** bar: every tile within reach of one scores higher
to a Bim standing on it, by 2, 5 and 3, so a spill beside a plant is
bearable where the same spill on a bare deck is not. The picture hangs
from a wall the way a wall light does. The numbers, the cap and where a
station puts its own are under [Surroundings](#surroundings).

### Locking doors, and what the enemy do about it

Every airlock is a door now: right-click one for the same four words a
bulkhead door has — open, close, lock, unlock — and a locked airlock is a
wall, to a walk and to the eye. An enemy with nobody it can get to goes
for the doors: it walks to the nearest locked door it can reach and
**smashes** it, fifteen seconds for a bulkhead door and thirty for an
airlock, one body to a door, with a bar over the door saying how long the
lock has left and a heave heard every couple of seconds; then the lock
gives and the door opens for it. A dying enemy runs, and it locks the
door behind it: sealed in, it binds its wounds every ten seconds, and
once nothing bleeds it unlocks its own door and comes out to fight. The
crew never smash a door and never seal themselves in — they are yours to
send. Docked, the station's doors are the same doors on both sides of the
passage: a lock you set stops the station's people, and a lock they set
you can lift from the panel.

### The hyperdrive, and the galaxy behind the map

A **hyperdrive** jumps the ship to another star. It is a part like an
engine — a two-by-two block on deck, wired like anything that draws — and
it has to be **bolted to a main engine**, block against block, or the
designer says so and it never fires. It is behind a tier-one research key,
after fusion power. On the map, **Galaxy view** swaps the system for the
galaxy the lobby showed: the star the ship is at is ringed in green, and
the strip lists what its system holds — every planet and station, the
hostile ones in red. Click another star and the strip lists what *that*
one holds, and **Jump** charges the drive for it: twenty seconds, holding
still, away from any berth (docked, the station's people are aboard; under
way, the ship is flying), and then the ship is in that system, in empty
space, pointing the way it was, with only what its own sensors reach on
the chart. Nothing of the old system comes along — its stations, its
people, its mining site — and everything of the ship's does. Abort during
the charge leaves the ship where it was. **System view** puts the map back.

### Landing on a planet

A **rocky planet** or an **ice world** can be landed on; a gas giant and
a belt cannot. Fly to it as to anything, and once the ship is holding in
its frame the strip offers **Land** (from the helm, like a trip): the
ship slides to the point straight over the planet and comes down from
there — the planet growing under it until it fills the window, then
black for a moment while the ground is laid out — and is set down on a
**landing pad**. There is no space any more: the planet is the whole of
the surroundings. Beside the pad stands a **town** of ten to fifty
people, on one of three **biomes** — **desert**, **temperate** or
**arctic** (an ice world is always arctic; a rocky planet is one of the
other two) — and no two towns are laid out the same. Its **houses**
stand along two or three streets, a few bunks each with a door onto the
street; the **gathering hall** is its mess, a galley along the north
wall and tables with a chair for everyone; a **bathhouse** holds a
toilet, a basin and a shower for every twelve people; the **trading
house** is where trade is done across the desk as at any station — the
ship's airlock opens onto the ground through the town's gate, and the
Station button is the same — and the **watch house** by the pad is
where the town's **guard** stands at its post behind two sandbags,
looking out towards the pad. A town feeds itself. A desert or temperate
town has **fields** in a belt beyond the houses: strips of soil that
work like hydroponic bays — planted, tended and lifted the same way, and
a field under the pointer opens the same menu, headed Field — but grow
at **half the pace** under the sky and have nothing to plug in, so a
brownout never touches them and a town has twice the trays a ship
would. An arctic town grows nothing in its ground and has
**greenhouses** instead, buildings full of hydroponic bays. Standing
lights line the streets, and by day the whole ground is lit. Round the
town is the **wild**, and it is what stops a Bim rather than an edge:
dense forest, a lake or a river and a few boulders in temperate country;
cliffs of rock, cactus scrub and one oasis in a desert; outcrops, a
frozen lake and firs in the arctic. A Bim off the ship can walk any way
it likes until a tree, a cliff, the water or a wall is in the way —
there is always a way from the pad to every door and every field, and
never a pocket it cannot get to.

The town is not the whole of the ground. It stands on a **plain** ten
thousand tiles across, and the ship is set down on open ground with the
plain on every side of it: walk round the hull, out through the gaps in
the wild ring, and keep going. What is out there is the planet's —
cliffs, water, forest too dense to push through, the odd tree and rock —
and it is what confines a crew member, not any edge; the plain's own
edge is a rim of cliff further off than anybody will walk. The ground is
made as it is walked and seen, never all at once, and the view reaches
**sixty tiles**: the camera cannot be pulled out further than that on a
planet, the ground is drawn that far from the middle of the window and
no further, and a crew member sees that far over open country. What the
crew have not seen of the plain is black; what they have seen and do not
see now is grey; and a walk out into it is a walk like any other — right-
click the ground, however far, and the crew member goes leg by leg, and
comes back the same way when it is hungry. A town is
**hostile or friendly** like a station — the map rings its planet in red
or blue — and at a hostile one its people are enemies and the guard is
the first of them. A Confirm from the pad is the cast-off, then the ship
lifts straight off to where a trip to the planet would have ended, and
flies from there.

### The reactor, the batteries and the brownout

Every step, what the wired reactors made less what the wired consumers drew
— and less what the engines are drawing, under a burn — goes into the
batteries, and the **Power** line on the ship panel says so: so much drawn,
so much to the engines, of so much made, and what the batteries have of
what they hold. A battery on the run arrives empty and fills at the surplus; one
taken off takes what was in it. A ship drawing more than it makes drains
its batteries at the difference, and when they are flat it **browns out**:
everything optional stops, and the essentials, life support and the doors,
run on off the reactor's own output. The world opens with the batteries
full, since the ship has been sitting at a dock. The log says *Brownout*
the step it starts and *Power restored* the step it ends, and the ship
panel's Power line carries *— brownout* in between.

What stops, all of it recoverable and none of it lethal:

- **The lamps go out.** Every lamp on the ship, at once, and the deck is
  dark round the crew: they see ten tiles, and so does anybody looking for
  them. That is the alarm — nothing is drawn for a brownout but the dark.
  The lamps are whole, and light again the moment the power is back.
- **The hydroponic bay stops**, in the same state as one whose target is
  met: nothing grows, nothing is planted or lifted, the trays hold what
  they hold, and the bay's menu says *no power*. Growth picks up where it
  stopped.
- **The cold store stops and the food spoils.** Every hour the cold store
  is without power — browned out, or on no conduit at all — an eighth of
  the vegetables, the tofu and the stew goes, rounded up, off the shelf
  and out of the hold, and the log says how much. The hour is a clock on
  the world that only runs while the cold store is unpowered and goes back
  to nought when it is powered again, so an overdraw the batteries cover
  costs nothing, and what is left when the power comes back stays. That
  is what a battery is for.
- **The benches, the research desk and the hyperdrive stop**, as before:
  no order is placed, the AI thinks about nothing, a charge with no drive
  to fire fails.

Nothing about the air: life support is essential and runs on, and nobody
dies of a brownout. At forty-eight times, a game day is thirty real seconds
and an hour is one and a quarter, so a brownout left alone while you looked
away is a dark ship, a stopped bay and a larder mostly gone — a day to work
back from, not a game over. There is no brake on the speed for it; the cost
is the brake.

### Making things

The crew make ten things, at four benches, and one mechanism does all of
it. A **recipe** is a bench, what goes in, what comes out and how long it
takes; the **smelter** turns two ore into one metal in half an hour, and the
**workbench** turns one metal into four components in twenty minutes, or
one metal, two components and one **galvum** into an **emitter** in an
hour — and, out of metal alone or metal and a galvum, the three pieces of
**armour** (see *Armour* under *What the ship will make*). Galvum is the rare one — only a mining outpost sells it, and only one
asteroid in ten has it in its core — and the emitter is what the interesting parts will be made of: a turret, a shield,
a mining laser. Nobody sells an emitter. The **armoury** makes the handgun,
the three other guns and the schword (below) and the vest, and the
**drug lab** turns two **fibre** into one **bandage** in a quarter of an
hour — fibre being the one crop the bay grows that nobody eats, and a
bandage the one thing made aboard that the crew already use, on each other
— and two vegetables and a component into a **medkit**. Medicine is made
from the first day; most of the rest has to be **researched** first, by
the ship's AI — see [Research](#research) — and a bench aboard a ship
whose crew have not researched what it does stands idle.

What turns a recipe into an errand is a **target**: on the items panel,
every row for something the benches can make carries a *keep so many*
number, stepped up and down, and while the hold has fewer of that thing
than the number — and the inputs for one, and room for it, and a bench of
the right kind with power — the work list offers *Making things*. A Bim
walks to the bench, stands at it for the recipe's length, and the ore comes
out of the hold and the metal goes in. The log says what was made. If the
ore was sold while the Bim stood there, nothing is made and the log says
that instead. The target is a command like a deal, so every player's ship
is making the same thing.

The smelter loses mass — two ore at ten is twenty, one metal is eight, and
the slag is vented — and it is the only recipe that may. Everything the
other three benches make weighs exactly what went into it — a bandage is
two fibre's worth. Every bench draws power, and every one stops in a
brownout.

### Research

**Research is done by the ship's AI**, at the **research desk** — a
console on a table's footprint, worked from the tile below, drawing ten —
because the humans aboard have stopped being able to. The **Research** tab
at the bottom left is the tree: the nodes as boxes, what is known on the
left and what waits on it to the right, a line from each to what it
needs. A crew sets out knowing everything a crew needs to live and to fly
— the hull, the galley, the heads, the bunks, the hydroponic bay, the
fission reactor, the helm and the engines — and knowing how to **mine**
(the suit locker and the suit) and how to make **medicine** (the drug lab,
its bandage and its medkit), so all three are possible from the first day.
The rest is researched: **smelting** (the smelter), the **workshop** (the
workbench and its components), **fusion power** (a **fusion reactor** that
makes 3 500 a minute in a three-by-three block, thirty times the fission
one), and then, behind a **lock**, the **armoury** (the bench, every weapon
and the three pieces of armour) and **emitters**. Click a node for what it
opens and to put the AI onto it; it works through the node on the clock,
hours for the workshop nodes and a day for the reactor, as long as the
desk has power, and stops if the power goes. A part the crew do not know
is not on the Build tab and not in the designer's palette, and a recipe
they do not know is greyed on the Management tab, so a playtest ship's
smelter smelts nothing until smelting is known.

The lock is opened with a **research key**: an artifact, sold nowhere and
made nowhere, that sits on the research desk of **four friendly stations
in five** — the one you set out from always has one — **lit up**, a ring
of lights round the desk that pulse while the key is there, so it can be
seen from the door. Right-click the station's desk and **Take the
research key** walks the crew member you steer over and takes it into
their pack, where it is **two cells tall**: a pack with no two free cells
one over the other cannot take it. Carry it home, store it from the pack
into the ship's own research desk (a click on the desk opens its
window, one slot the key's exact size) and on the Research tab **Consume
the key**: the key is gone and tier one's lock is open for good — both
locked nodes at once, since the key opens the tier, not a node. There
are three tiers to come; the second's key will be bigger, want a bigger
desk, and want a tier-one key as well, which is where the progression
goes. A key a station buys back for five thousand euros if you have no
use for it, and a key taken is a key gone: the desk stays bare.

### Mining, on foot

Ore is dug out of asteroids by a Bim in a **pressure suit** with a pick,
and the outside is a place. Hold station at an asteroid belt — fly to it,
and the trip ends short of it, alongside — and the belt
becomes a **mining site**: a field of eight to twelve asteroids laid out
round the ship on the ship's own tile grid, close in — the nearest a few
tiles off the hull — so that a walk to the rock is minutes. Every asteroid
is **stone on the outside and ore in the middle**: the skin is bare rock,
worth next to nothing, three tiles deep, and only what lies deeper is the
ore — silver-grey **iron ore** on most of them, and purple **galvum** on
about one asteroid in ten. Which one is the rare one shows through its
skin, so you can see what you are digging for before you dig.

Nothing is mined that you have not **marked**. The **Actions** tab at the
bottom left has the one action there is, **Mine**: pick it and the pointer
becomes a pick, a click on a rock tile marks it to be mined and a second
click unmarks it, and the tab says how many rocks are marked, how many of
those a walk could actually get to, and clears the lot. Escape or a
right-click puts the pointer down again. A mark is a command like a deal,
so every player's ship is digging the same rocks; the marks come off when
the ship leaves.

With rocks marked, a suit in the **suit locker** and room on the shelves,
the work list offers *Mining outside*, and a Bim with it high enough on
the list takes the suit from the locker, walks to the deck inside the
airlock, goes out — and **walks**: the outside has a navigation grid of
its own, a hundred tiles every way about the Bim, one cell a tile, with
the hull and every rock on it, rebuilt as the Bim moves and as rocks come
out. It goes to the nearest marked rock it can get to, stands on the tile
beside it — straight on, never from a corner — and swings the pick for a
dozen minutes until the tile is gone, then the next, until there is none
left it can reach; then back to the port, in, and the suit hung up. A dig
is therefore a **tunnel**: mark a line of tiles from the skin in to the
core and the Bim takes them from the outside in, standing in each mined
tile to reach the one behind it. A rock with rock on every side waits
until one in front of it is mined, and the tab says how many are waiting
like that.

What comes back is on the shelf as each tile goes — two **rock** for a
skin tile, two ore for an iron one, one galvum for a galvum one, as much
as fits — and the log says what the walk brought when the Bim comes in.
Rock is cargo like anything else, heavy and worth two euros a unit at any
station, and nobody sells it. The site remembers: a tile mined is gone
for good, and coming back to the belt finds the field as it was left.
**Nothing stands at a belt**: no station hangs off one, so a belt is the
ship's alone when it gets there — the mining outposts, which used to be
bolted to the belts, are dug into rocky planets and ice worlds instead,
and are still the one place that sells galvum.

There is no air gauge. What bounds a walk is **radiation**: the suit lets a
quarter of the open dose through, and the dose comes off at half a minute
a minute under cover. A Bim whose dose is past half the critical line is
not sent out again until it has come down, and one out there when it
crosses the line finishes the rock it is at and comes in. The **Dose**
line on the ship panel is each crew member's, in minutes-in-the-open, and
the log says when a body crosses a line — a dose picked up, a dose
cleared. One body outside at a time: the airlock is one Bim's while a walk
is on.

A walk interrupted — the Bim gets hungry out there — brings the body back
in through the door, and the walk starts again from the gangway when the
Bim is done eating, picking its rock afresh. A right-click on the deck
does nothing to a Bim outside: the walk is what brings it in.

### Building, aboard

The ship goes on being built after the design phase — by the crew, out of
what is on the shelves, and nothing is instant. The **Build** tab at the
bottom left is the palette: the parts by category — *Structure* for the
deck, the walls and the hull and the ways through it, *Furniture*,
*Production* for the benches, the armoury and the bay, *Galley*,
*Hygiene*, *Power*, *Ship systems*, *Propulsion* — each row with what the
part is made of, dimmed to a warning where the shelves have not got it,
and a search box over the lot for when you know the word and not the
heading. Pick a part and it is in your hand: a **blueprint** of it follows
the pointer over the deck, the part's own picture shown through, green
where it would go and red where it would not, with the reason at the top
left — something standing there, no deck under it, or the one the designer
would have given: it would go, but a Bim could then not get at the hob.
`R` turns it, a click lays it out, a right-click or Escape puts it down.
The rules are the designer's, asked of the ship as it will be once
everything already laid out is built, so a wall on deck that is itself
still a blueprint goes — the crew take the sites in the order they were
laid out, and the deck is there by the time they come to the wall.

A site laid out is a **construction site**, in blueprint blue, and the
crew work it as a job — two rows on the work list, *Hauling* and
*Building*. Whoever is free walks to a shelf, takes a load of what the
site is made of, carries it over — a crate in both arms — and puts it down
there, a load of twenty units at a time until everything is there; the
bar along the foot of the site fills as it arrives, and the tab's **Laid
out** list says the same in numbers, with a way to call each site off.
Then a Bim stands beside it and puts it together, a few minutes for a
wall and a couple of hours for a heavy engine, and the part is on the ship: the
room the crew live in is laid out again under them with the new wall a
solid in it, the new bunk a bed, the new shelf somewhere to fetch from,
and nobody's errand is lost for it. What it cost is exactly the recipe,
out of the hold in one go the moment the part goes down. The materials
never leave the shelf before that: what has been carried to a site is
*spoken for* — it cannot be sold or smelted from under the site, and the
Build tab counts only what is free — so the ship weighs the same
throughout and a site called off costs nothing.

A site can be **outside the hull**. Plating laid out against the skin, an
outside wall on it, a thruster in the void beside the ship: any site with
no tile beside it that a body can stand on from the deck is reached from
outside, and the crew do what the miners do — take the suit from the
locker, go out through the airlock, walk round the hull on the outside's
grid to the tile beside the site, carry the load there or build there,
and come back in. The same dose rules apply, and the same one-at-a-time
airlock. No tools are needed for any of it, only the materials.

Two rules hold the ship and the building apart. **Nothing is built on a
ship that is moving**: sites can only be laid out, and are only worked,
while the ship is docked or holding station; under way the tab says so
and the crew leave the sites alone. And **the ship stays put while it is
built on**: a Confirm is refused while any site has a load carried to it
or a Bim on the way to one. A bare blueprint with nothing done at it
holds nothing — it is a plan, and the ship may fly with a plan on the
deck — and cancelling a site frees the ship at once.

### Seeing where you are

The system you start in is **charted**: every planet, belt and station the
lobby's chart showed is on the map from the first step, drawn as what it is —
a rocky world with its continents, a gas giant with its bands and ring, an
ice world under glare, a belt of rocks; an orbital wheel, a refinery's tanks
and stack, a mining rig in its rubble, a broken derelict, a relay's dish — on
faint rings that show their orbits. **You are the reticle**: the ship is
the map's origin, drawn as a little hull pointing where it points, inside
a breathing cyan ring with a tick at each compass point that sits over
every icon — a docked ship is on top of its station's, and the ring is
wider than the station's — with *You · Docked · the station*, *You ·
Alongside the belt* or *You · Open space* written over it in your own
colour, the way the galaxy chart tags your star. Click one and the helm
quotes the trip; Confirm sends the ship. A planet you are alongside is
drawn under the hull in the ship view, as the ground.

A **station is a place**, not an icon: every one in the system is a hull on
a grid of its own — thirty-four to seventy-two tiles across, laid out
from the blueprint's seed with the same parts and the same rules as a
ship. The one you start at is a **hub and four arms**: a square hub of
open deck in the middle, a corridor five tiles wide running out of each
side of it to a docking lobby with an airlock in its far wall — the west
one is the port you dock at, and its lobby is the reactor room too, with
the trading desk by the door — and the rooms hung off the north and south
arms two deep a side: the mess and the crew's quarters, the heads and the
laboratory with its bays, the rec room and the research room, the storage
and the cargo shelves, each behind a bulkhead with a two-tile doorway, the
outer rooms opening through the inner. A barricade of **sandbags** stands
across three of each corridor's five tiles a few tiles out from the hub —
cover to crouch behind, and low enough to walk and shoot over. **Every
other station is one of six plans**, rolled off its seed so the same dock
is the same building every visit and the next dock is likely another: the
hub; the **pod**, the smallest, a squat bar with one corridor two wide
and a single resident; the **cross**, two fat bands meeting in a hall,
narrow corridors up the arms, three living there; the **spine**, long and
thin with a corridor three wide the length of it and four aboard; the
**ring**, a square ring of corridor round a void with the rooms outside it
and six aboard; and the **comb**, three arms off a spine to a docking bay
each, five aboard. They differ in size, in how narrow the corridors are
— a barricade fits only in a corridor three wide or more — and in how
many people live there; they all have the same rooms, the reactor room
with the trading desk by the port, and a research desk. The seed decides
how many bays, shelves, tables and batteries; a bigger station gets more
of each. Bulkheads and doors, the helm, the shelves and the shower
are drawn as themselves rather than as coloured blocks, in the room's
palette, so a station reads as a building and a ship as a ship — and so
are the reactor, the battery and life support.

**A door in a bulkhead is a powered door**, two tiles along the bulkhead
and one deep — the whole of a doorway the crew can walk — and turned with
`R` to stand in a bulkhead running either way. It opens by itself for
whoever walks up to it and shuts a moment after the doorway is clear, so the crew
and the residents come and go through them without being told, and the
navigation plans straight through an unlocked one. The four things the
bathroom door offers are on a right-click: **hold open**, **close** (let it
look after itself again), **lock** and **unlock** — each an errand that
walks James to the panel, as the bathroom door's are. A locked door is a
wall until it is unlocked: no route is planned through it, the readout
says `Door · locked`, and the leaves wait for anyone standing in the
opening before they shut. The bay is a **run of six trays**, one a tile,
worked from the row along one side of it — a bay standing north–south is
six trays down. In the ship view it is drawn where it is and as big as it
is, so it *approaches*: from far off a plate with its icon on it, nearer its
hull tile by tile, and within fifty tiles of its hull the people who live
there — two on most stations, one on a relay, nobody on a derelict, whose room
opens all the same so its fixtures are drawn — are the room's Bims again, up and about between the room's own pictures of its
fixtures, with names over their heads. Leave and they are forgotten; come
back and they are at their bunks. The ship **docks beside it, airlock to
airlock**: the trip ends at the berth, the ship is turned so its airlock
faces the station's, the two collars — each stands half a tile out of
its skin — meet as a tube between the hulls, and both doors are drawn
parted. The game opens docked at the first station somebody lives on, never
at a derelict. Escape opens a settings sheet with every key on it. An airlock has to be in the skin for that — one on the deck is a door
to nowhere, and the checks say so. And docked, **the two are one deck**: the
ship's and the station's on one navigation grid with the passage between
them, so a right-click on the station's deck sends James through the
airlocks. The station's people stay on the station: they live in a room of
their own, with their own galley, heads, bunks and bay, on their own
timetable and to their own manager's goals — a hundred vegetables, fifty
blocks of tofu and two pots of stew a head in the cold store, and the bay
planted to keep it so. The Management tab is the crew's own and reaches
nobody ashore. Who is who is on their backs: the crew wear the ship's blue
coverall and the station's people the station's orange one. Leaving takes
the deck apart again, once the crew have all walked back aboard — see
*Flying it*.

Beyond the chart, the ship sees `VISION_RANGE` with the crew's own eyes,
which out here is almost nothing, and a great deal further with a **sensor
array**. Anything that comes within range of the stretch the ship travelled
— the stretch, not the endpoints, because at 24x a step is a long way — is
discovered, shared by the whole crew and never forgotten. Undiscovered things
are not drawn on the map at all, and there is no way to plot a trip to one.
That is what will find the next system, when there is a way there.

Most systems have a station now — about three in five, and often more than
one — so a start is rarely far from somewhere to go.

### Speed, and who decides

Pause, 1×, 3×, 10×, 24× and 48×, beside the day and the clock at the top left.
**Every player has a request and the slowest one wins**; a pause by anybody
is a pause. That is not a compromise, it is the point: the player who needs
it slow is the player something is going wrong for, and nobody is ever
carried past something they wanted to look at. The button held down is
what you asked for and the one coloured is what is actually happening, and
with more than one player each one's request is listed under, so being
held at 1× is never a mystery.

The rest of the screen: the **Inventory** down the left, the crew's money
at its head and what is aboard under it by where it is kept; the readout
under that, and the agendas; the crew member picked on the right; the tray
at the bottom with its tabs — the room's three, then View, Actions, Build
and **Ship**, which is the helm and the ship's facts — and the Station
button beside them while there is a station; and what just happened, at
the bottom right.

### Trading

**Money only works while docked**, because a station is where there is somebody
to buy from. Holding station beside one is not docked — that wants an airlock —
and out between them the pool buys nothing at all. Supply is unlimited; what
bounds a purchase is the money and the hold.

**Every station charges its own prices, and two numbers a thing.** There
are two ideas of what a unit is worth, and they are kept apart on
purpose. The **book value** (`economy::trade_price`) is what a thing *is
worth* — the same everywhere, and used only to value a hold: what the
crew set out with, what the ship is worth now, what an enemy's garrison
is scaled against. Nothing is ever bought or sold at it. What a station's
desk actually charges is its **market price** (`economy::market`): a
**quote** of two numbers, the **ask** — what one costs bought here, the
*Costs* column — and the **bid** — what the desk pays for one, the
*Pays* column. The bid is always under the ask, so a thing bought and
sold straight back at one desk always loses money, and the whole of the
trading game so far is that two desks lean different ways.

The sum, in whole euros and rounding down at every division:

    mid  = book * (100 + kind_bias + local_bias) / 100
    half = max(1, mid * SPREAD_BP / 20 000)
    ask  = mid + half
    bid  = max(1, mid - half)

`kind_bias` is what the *kind* of station does to the price, per cent —
a hand-written table, starting values to be tuned: a mining outpost sells
ore and galvum cheap and pays well for food, metal and components; a
refinery sells metal cheap and pays well for ore; an orbital sells food
and fibre cheap; a relay is dear on everything and pays well for food and
bandages; a planet's settlement sells food cheap and pays well for metal;
a derelict has no market at all — nothing to buy, and nobody to sell to.
`local_bias` is the station's own lean, one small whole number per cent
a resource in `−15..=15`, rolled by the generator off the station's seed
for every resource whether it stocks the thing or not, and in the galaxy
checksum beside its shelf — so two outposts in one system are two
different outposts, and two players on one seed see the same numbers.
`SPREAD_BP` is the desk's cut, a thousand basis points: five per cent
either side of the mid, and never less than a euro.

**The station you start at leans only the way its kind does**: the
generator rolls it a local bias like any other, and the world sets it to
nothing (`World::start`, and the yard's Station panel with it), so an
opening pool buys the same at a kind of station whatever the seed
rolled, and what the crew set out with is worth its book value.

The shelf is a window — **Station** on the tray, docked, opens it in the
middle of the screen and the cross, Escape or casting off shuts it — and
what is *on* it is two rules deep. The kind's is the ceiling: galvum only
at a mining outpost, an emitter nowhere, rock nowhere, nothing at a
derelict — there is nobody aboard to sell it. Under that each station keeps
a shelf of its own, rolled off its seed: ore, metal and both foods are on
every one, because a station where the crew can buy nothing to build with
and nothing to eat is a trap, and each of the rest — components, suits,
medkits, an outpost's galvum, an orbital's fibre, a bandage at an orbital
or a refinery — is there or not, so two refineries stock
different things and there is a reason to fly to the other one. A row the
station does not sell is greyed with its buy buttons off, and stays,
because what is aboard can still be sold there, at the bid. Every station
somebody lives on buys anything; a derelict buys nothing, since there is
nobody at its desk. A station is not yet *for* anything beyond the way
its kind leans — a theme would replace the roll, not the ceiling.

### Mercenaries

At a station whose people are not your enemies there may be a **mercenary**
living among them: a body in an olive coverall rather than the station's
orange, with a **?** floating over its name, which is how you know it can
be spoken to. How many depends on what your ship and its hold are worth
against what you set out with — at the start it is one at some stations
and none at the rest, and a crew that has got richer finds more, up to
four. Right-click one and the row is **Hire — see the terms**: it walks
your Bim over and opens the **Hire** window, which says what they carry
— the gun, and every piece of armour worn — and **what a month of them
costs**. The fee is the kit: a pistol and nothing else is about two
thousand a month, a sniper rifle in full armour about twenty thousand,
and every mercenary asks a little more or a little less than the kit is
worth, up to fifteen percent either way. **Hire** wants your Bim within
two tiles, the first month's fee in hand, and a free bunk aboard — the
window greys the button and says which is missing. Hired, they walk out
of the station's room and onto your deck, a crew member from then on:
they eat, sleep and work like the rest, and take orders like a crewmate
(only the Bim you steer takes yours). **They are paid by the month**,
every thirty days out of the crew's money, and the log says so. A month
the money will not cover is a month owed: the log says they are unpaid,
and at the next berth they walk off — back into the station's room, for
hire again when you can afford them. The `test` command always has one
for hire at its dock, and its ship has bunks to spare.

### The trading desk

Every station keeps a **trading desk** just inside its port — a wooden
counter with a ledger and a terminal on it, against the corridor's north
wall. The station is traded with across it: **Station** on the tray still
opens the shelf, but the rows are live only while the Bim you steer
stands within two tiles of the desk, and until then the window says so
and offers **Walk over**. Right-clicking the desk itself gives the same
in one row — **Trade** walks your Bim over and opens the window. What you
buy still goes straight into the hold, wherever aboard it is kept; the
desk is where the deal is struck, not where the goods land. A buy or a
sell sent from across the room is refused with *nobody of yours is at
the trading desk*.

### What the crew can see

The crew see **all the way round** — there is no cone — and what stops
their eyes is what stands in the way: a wall, a bulkhead, the hull, a
shelf or a reactor, a door with its leaves shut. Not the low things: a
Bim sees over a table, a bunk, the worktop, a tray of crops. It is
**traced**, tile by tile, from every crew member's eyes, and it is
**shared**: what one of them sees, all of them see, and the screen shows
the crew's view.

A Bim standing **against a wall peeks round it**: as well as from where it
stands, it looks from the tile either side of it along the wall, and what
that adds is whatever lies past the wall's line. So a Bim pressed to the
corner of a room sees the whole of the room round the corner, where one
standing a tile back sees only the wedge the corner leaves — the wall's
edge cuts the view. A peeking Bim shoots from the peek.

What nobody sees is under a fog, and whose the structure is decides what
the fog looks like. **Your own ship** is under a light one: the deck stays
readable — you know where your own walls are — but whatever is standing
there is not drawn. **Somebody else's station** — neutral or hostile — is
**black** where nobody has looked, and **grey** in a ring a few tiles wide
round what is seen: the walls and the fixtures show through the grey, and
nobody standing among them does. The grey stays once earned; what has been
looked at is known. Its people appear only in line of sight, and stay
drawn for two seconds after they were last in it, so somebody stepping
behind a bulkhead is a moment fading rather than winking out. From outside,
a stranger's station is a black shape and your home station is its hull
under the light fog. Home is the station you set out from; every other
station is neutral, and `combat` makes the dock hostile.

### The two views

**Ship** is the live ship at tile scale, drawn turned to its heading, with the
starfield streaming the other way behind it — faster the faster the ship is
going, on the world's clock, so it holds at a pause and races at 24x. The hull is plated, with running
lights to port and starboard and a strobe at the bow; the engines burn while
the ship is under them — through the burn, and again through the brake once
it has flipped — and the thrusters puff on the corners that turn it the way
it is turning. What fires is read off the same plan the ship's position is,
so the flame is where the ship is at any speed. **System map** is the star,
what the crew have found, the ring the scanner reaches to, the route, and a
little hull pointing where the ship is pointing.

The camera is **north-up in both by default**. It is the ship that turns on
screen. A camera that followed the heading would make a flip legible and every
other moment unreadable — you could not tell which way you were going, because
"which way" would always look the same. **Head up** (the View tab, or `N`)
is the other choice: the ship held square to the window and the sky and the
map turned round it instead.

The ship view **follows the crew member you steer**: James is what sits in
the middle, on the deck or across a station, and a drag can shove the view
only so far before he would be off the edge. **Free camera** (the View
tab, or `F`) lets it go — the view stays where it is and a drag or the
keys take it anywhere, for looking at the far end of a station while the
crew are busy at this one — and **Follow** snaps it back to him.

### Saving, and picking it up again

Esc opens the settings, and **Save** and **Load** are on it. A save is the
whole game as it stands — the clock, the ship and where it is, the room
aboard with everybody in it mid-errand, the stations and what is on their
shelves, the hold, the money — written as text to
`~/.local/share/bims/saves/<name>.ron` (`$XDG_DATA_HOME`, or wherever
`BIMS_SAVES_DIR` points). A name is letters, digits, `-` and `_`; a name
already used is written over, and the page lists what is there to pick
from. Load lists the same files, newest first, and puts the game back
exactly where it was: the crew carry on from the step they were at, and
the checksum of the world read back is the checksum of the world written.
What is *not* saved is the window's — which panel is open, where the
camera is, what you were aiming at — so a loaded game opens on the whole
ship the way a new one does.

The menu at the start has a **Load** button too, beside Play, so a game is
picked up without walking through the setup and the yard. There is nothing
to save in the yard: the game starts when the ship is accepted, and the
button says so until then. A file written by another version of the game
is refused by its version rather than read wrong.

### The two crates behind that

- **`crates/flight`** is what a design does when you push it — mass, centre of
  mass, inertia, acceleration, how fast it turns — and the trip that takes it
  somewhere. A plan is worked out **once** and then read at a time: nothing
  integrates, so a window at 24×, a window at 1× and a server catching up on
  an hour of somebody's disconnection all put the ship in the same place.
- **`crates/world`** is the star system, the ship in it, the clock, and the
  order things happen in. It renders nothing and knows nothing about a window.

What the steps after this one are promised is written down at the top of
`crates/world/src/lib.rs`: Bims live in ship-design tile coordinates and the
ship's rotation does not reach them, every ship change goes through
`on_ship_changed`, construction spends what is aboard and asks
`can_modify_part` first, money is only for docks, and the design's exposure map
is the radiation input.

Which world you land in is a seed and a galaxy shape, and for now they come off
the settings the lobby hands over, with a fixed default behind them. The
lobby's World tab will pick them instead; nothing else about them changes.

## What the ship will make

None of this exists yet. It is the contract the next few steps are built to,
written down first so that each of them is measured against something, and
each heading below is replaced by a description of the real thing as it
lands. The order is the order of the headings: power, the resources, the
workstations, the walk outside and the armoury — all built, and described
under the designer and the game — then the fight, of which the boarding
half is built and the rest is not.

### One mechanism, the tree, and the mass

Built. *Making things* under the game is the mechanism and the table as
they stand; *Trading* has what each kind of station sells. The tree as
planned had a locker class for suits and weapons — that arrives with the
suit, below.

### Power

Built. It is *Power* under the designer and *The reactor, the batteries and
the brownout* under the game, below.

### The walk outside

Built, and the outside is a place now: *Mining, on foot* under the game.

### The armoury

Built, as far as the making goes. The **armoury** is a bench and a locker in
one: two components and an emitter make a **laser handgun** in three
quarters of an hour, four metal and two components a **vest** — and,
since the fight grew four more weapons, metal and components with or without an emitter make the
**shotgun**, the **auto rifle**, the **sniper rifle** and the **schword**
(*Combat mode* under the game says what each does) — and it holds eight
of them beside the suits. Set a target for a handgun with galvum aboard and the benches
run the whole chain in order without anybody sequencing them — a handgun
is not on offer until there is an emitter, and an emitter is not until
there are components. Nobody sells a handgun or a vest; a medkit is on
every lived-in station's shelf. Beside it stands the **drug lab**, the
fourth bench: two **fibre** — a crop, grown in the bay — make a
**bandage** in a quarter of an hour, two vegetables and a component a
**medkit** (the armoury's until research came in, since medicine is made
from the first day and the armoury is researched), and a bandage is the one thing made
aboard that a Bim already *does* something with: it closes the wounds on
one part of a body (see *Getting hurt* under the game), and a medkit is
the other — it is what gets a Bim out of a dying state. What a Bim does
with a handgun is the fight, below — the boarding half of it; what it
does with a vest is still nothing.

### Armour

Built. Three pieces, all made at the **workbench**: a **helm** out of two
metal (half an hour; +15 health, 2 protection), **kevlar** out of three
metal and a galvum (three quarters; +20, 2), and **leg guards** out of one
metal (twenty minutes; +10, 1). The playtest ship carries one of each. A
piece is two things at once, on purpose. **In a container it is a
resource** — `Helm`, `Kevlar`, `LegGuard`, locker class beside the medkits
— so buying, selling, crafting, mass and the shelves work on it with no
new mechanism; **anywhere else it is an instance**, with an id that only
climbs and a health it keeps wherever it goes. The world keeps every piece
there is (`World::pieces`: id, kind, health left, and where — the hold, a
pack cell, or worn by somebody) and holds one invariant against the hold:
the count of each armour resource is always the number of pieces in the
hold of that kind. A bench or a purchase pushes a whole piece; a sale takes
the most damaged one first. What is worn and what is in the pack are the
room's (`bims::combat::Gear`), since the room's health reads them, and the
world reads them back every step for the checksum.

Five commands move a piece about, the way a craft target is a command —
the hold is the world's, and every player's ship has to agree what is in
it: **Fetch** takes a piece (or a unit of anything) out of a container into
the crew member's 3×3 pack, **Stow** puts one back, **Equip** puts on what
is in a pack cell and swaps what was worn into it, **Unequip** takes a
piece off into the pack, and **Discard** throws one away — and a sixth,
**Loot**, takes one thing off a body that is down (see [Combat
mode](#combat-mode-and-the-inventory)). A fetch or a
stow wants the crew member within two tiles (`REACH`) of a container that
takes the thing — the armoury or the drug lab for locker goods, a shelf
for shelf goods *and* for armour and weapons, the cold store for food —
and is refused *out of reach* otherwise; a full pack refuses a fetch, a
full class refuses a stow, and a **broken** piece — at nought — cannot be
stowed or sold at all, only discarded. Equipping wants no container. What
a worn piece does to a hit is [Getting hurt](#getting-hurt): the
protection comes off the damage first, what is left drains the piece, and
only what the piece could not take reaches the body.

**Every weapon and every piece has a tier**, one to three, and the
workbench is where a tier is made: two of a kind at the same tier go into
its two slots, **Upgrade** in its window starts a day of work on them,
and one of the next tier comes out in the third slot — a quarter more
damage and accuracy for a weapon, half again the health and protection
for armour, and at tier three a little more range or a chance to dodge.
A tier-two thing lies on a blue cell wherever it is drawn — a locker, a
pack, a body being looted, the bench — and a tier-three on gold, so an
inventory says at a glance what is worth taking. A crew member carries
a thing to the bench by hand (Ctrl-click it in the pack with the bench's
window up) and takes the result off the same way; with **Combine
matching gear** ticked on the Management tab the crew do it themselves,
one thing in the arms at a time, from the lockers to the bench and back.

### The fight

- **Between ships**, turrets fire on a plan-shaped clock rather than an
  integrated one, damage lands on a tile, and a **destroyed part leaves
  scrap, not its recipe**. That is the one place the materials contract's
  "no loss" is broken, and it is broken on purpose and named. Outside walls
  have hit points; a shield trades power for damage. Shields, turrets or a
  full burn off one battery is the decision the power system was built for.
- **Salvage** is docking at a derelict and taking its parts apart with a Bim
  in the middle — the caller `materials.rs` says the deconstruction rule is
  waiting for. What a derelict has left in it is generated already.
- **Boarding** is docking to somebody hostile. The rooms join into one, one
  nav grid, and the fight is Bims with handguns in corridors the station
  layout already promises are walkable. That is in now, both ways:
  `combat` docks at a hostile station, a recruited Bim shoots what it can
  see, the station's people take the hits — and they shoot back, from
  cover, knowing where the crew are, with whatever each was issued — a
  pistol for most, a shotgun, a rifle, a sniper rifle, or a schword, whose
  bearer charges and locks a gunner in a melee — and a hit is a wound on
  a part of the body that bleeds until somebody dresses it with a bandage
  — see [Combat mode](#combat-mode-and-the-inventory). Stations are hostile
  from the generator (about three in ten of the lived-on ones; the lobby
  rings them red and never starts a crew at one), and `combat` makes the
  dock hostile whatever it rolled. Armour is in too — the section above.

## The simulation

`nix run .#simulation` is the game without the front of it: it opens the
world at once, for one player, on the **playtest ship** —
`shipdesign::playtest_ship()`, a twenty-tile hull with one of everything a
crew of one needs to live and to fly, a full tank, metal and components on
the shelf and a few days' food in the cold store. It is laid out the way a
small ship would be: a bow cut back to a point in diagonal hull, with the
bridge in it — the helm on the centreline, life support and a battery in
the corners the cut leaves; the main deck amidships, the galley along the
bridge bulkhead to port with the table under it, the bay in the middle,
the bunk against the starboard skin and the airlock behind it; and
engineering aft — the tank and the reactor down the port side, a shelf of
stores, the heads, and the main engine set into the stern so its bell is
the stern. Three compartments, two bulkheads with a two-tile doorway each,
a run of conduit from the reactor to the helm, and thrusters in the skins
fore and aft. Every doorway and every gangway is two tiles wide on purpose:
the room's navigation cannot walk a one-tile gap. It has `SIMULATION_MONEY`
(a placeholder €50 000) in hand. It is for playtesting the world quickly, and
it is the one place the old "lowest star with a station" spawn survives:
the world is the fixed default seed's, a two-arm spiral, docked at that
system's first station. `nix run .#test` is the same ship somewhere else
each time: a random seed, and a dock somebody lives on picked at random
across that galaxy.

The ship's part count and `design_hash` are pinned — `PLAYTEST_PARTS` and
`PLAYTEST_HASH` — and checked like the reference design's, so the simulation
opens on the same ship on every machine.

## The crew

Two Bims live aboard: **James** and **Kate**. Their names are written over
their heads on the deck, each has an agenda down the left, and the one you have
selected has its bars and its crew sheet down the right. The same panels, the
same keys and the same menus are on the ship's screen once the world is open
— `1` or a drag to select, right-click the deck to move, `r` to recruit, and
the tray at the bottom left — because the crew aboard are these Bims and the
panels are one module, `crates/app/src/crew.rs`, shared by both screens. In
the simulation there is one of them, James.

They are not two kinds of thing. Both run the same needs on the same clock,
both take themselves to bed and to the galley and to the heads for the same
reasons, and nothing in the simulation distinguishes them except an index. The
one asymmetry is the player: **every order goes to James**. Selection, the
right-click move order, recruiting, and every item on every fixture menu act on
him and only him. Kate takes no instruction from anybody and lives her whole
day on her own account — which is the point of her being there.

A Bim's name, coverall and hair are the app's business and the drawing's; the
simulation knows crew member 0 and crew member 1. No strings come out of the
room, so the names are written over the deck by the app after the shape
buffer has been replayed, not carried across in it.

### Picking one, and the crew sheet

**Clicking a Bim selects it, and selecting is what puts its panels on the
right-hand side**: the four bars, health, and a character sheet under them.
One at a time, and nothing at all when nothing is picked — the right-hand side
answers *who am I looking at*, not *what is everybody up to*. Kate's bars are
hers until you click on her, and her panels carry her own colour so a glance
says whose sheet is open without reading the name.

**Selecting is looking at, not taking charge of.** Any of them can be picked;
only James takes orders. Click Kate, right-click the floor, and nothing
happens — which is the same answer as before, arrived at more visibly.

The sheet is tabbed the way the tray at the bottom left is, because it is the
same kind of thing: pages of detail you open when you want them rather than a
readout to be watched.

- **About** — name, age, and the date they were born. Everyone aboard was born
  between **2350** and **2380**, rolled at the start of the game and fixed
  thereafter; the game opens on the first of January **2400**, so the crew are
  somewhere between twenty and fifty. The ship keeps a 365-day calendar with
  twelve months of the usual lengths and **no leap years** — a leap day buys
  nothing here and costs a special case in every piece of date arithmetic,
  including working out whether this year's birthday has been had yet.
- **Memory** — the Bim's own account of its days.

### What a Bim remembers

**Only what went wrong.** Newest day at the top, in the order it happened
within a day, and a crew who spend the week eating, sleeping and getting on
with their work leave the page **blank** — it says *"Nothing has gone wrong."*
and that is the good outcome rather than a panel that failed to load.

It used to keep the day's work too — *"Woke up after 6 hours"*, *"Went to the
heads"*, *"Swept 5 patches of the deck"* — three or four times a day each,
which meant scrolling past a wall of chores to find the one line that mattered.
Now the chores are not written down at all.

What is left: an accident, being sick, dropping off standing up, each stage
further into hunger or sleeplessness as it arrives — once, as it happens, not
once a frame for as long as it lasts — the low moments of a Bim nobody has
spoken to, sitting down on the deck, hurting itself. Coming back out of one is
not an entry; the bars say so.

And **what it saw**. When something happens to one of the crew, any of the
others near enough to see it and awake to notice remembers that too: *"Saw Kate
have an accident."* A Bim asleep in its bunk on the far side of the compartment
witnessed nothing, and a diary that claims otherwise is one nobody can trust.

**A death is the exception to that.** It goes into every surviving diary
wherever they happened to be standing, because it is the one thing aboard
nobody could fail to notice — and it is the thing the whole "only what matters"
rule exists to make findable.

An entry with no words for it is **dropped from the page** rather than padded
with a placeholder. A line that says "Something happened." reads as the Bim
having had a mysterious experience when in truth the table is simply short an
entry; a missing row is at least honest.

No strings come out of the room, here as everywhere. An entry is a day, a
time, a code and one number; `memory_line` in `crates/app/src/names.rs` is
where the sentences live. A Bim that remembers being sick remembers `(day 4, 18:22,
WasSick, 0)` and "on the deck" is the host's wording of nothing at all. The
book is bounded — a few hundred entries, oldest falling off the front — so a
game left running does not grow without end. Memory is finite; so is this.

### What is one each, and what is shared

Each has **a berth of its own** and **a seat of its own**. There are two bunks —
one against the left wall where the only bunk aboard always stood, one in the
top-right corner, mirrored so it is got into from the room — and two chairs, one
each side of the table, with a place laid in front of each. A Bim goes to its
own bed and sits in its own chair; neither is ever contested.

**A bunk is one crew member's, and you say whose.** Each starts with the
bunk of its own number, as far as the bunks go; click any bunk for a menu
that says whose it is and offers **Assign to** the crew member shown (the
one you have selected, else your own) — whoever had it loses it — or **Give
up this bunk**. Aboard a ship a hire takes the first spare bunk, a dead
crew member's comes free, and a station's bunks are not yours to give. A
crew member **with no bunk sleeps on the deck** where it stands — the
`combat` command's fourteen on a ship with five bunks, or anybody you took
one from — and only **three hours in every six**: the six-hour window opens
the minute it lies down, and until it closes there is no more sleep to be
had on the deck, though a tired enough Bim nods off on its feet as ever.
It gets up **sore** — *Slept on the deck* on its panel, for twelve hours —
and its rest runs out half as fast again the while. The panel says **No
bunk** under its bars until you give it one.

Everything else is shared, and two of the shared things are one pair of hands'
worth:

- **The galley.** One cold store, one board, one knife, one pot, one hob, one
  dishwasher. While one Bim is on a galley errand the other's simply does not
  start — the menu item says *"Kate is in the galley"* and is greyed out, and
  Kate's own hunger waits and tries again. Tending the hydroponic bay counts as
  a galley errand, because that chain ends by putting the harvest in the
  fridge.
- **The heads.** One pan, one basin, one door.

Nobody queues for either: the errand is not begun, `consider_errand` moves on
to whatever else that Bim could be doing, and it comes round again a moment
later. That is the same shape as every other "can't do that yet" in the game —
an empty cold store, a locked door — rather than a new mechanism.

The **deck** is shared too, which means a mess one of them makes is a mess the
other has to stand in. How far gone each is for standing in it is still its
own: the mess is the room's, the two hours spent beside it are the Bim's.

### They walk through each other

**Bodies do not collide.** Two Bims that meet pass straight through one
another, and both slow to **70% of their pace** for as long as they are within
a body's width. Close quarters are slow; that is the whole of the rule.

It is the one resolution that cannot leave anybody stuck, and being stuck is
the real hazard here. **A route is planned once and never replanned**, so a Bim
whose line goes through the other has no second plan to fall back on. Anything
that stops it getting where that line goes — pushing it aside, holding it up —
risks two of them standing nose to nose for ever with errands on both agendas.
Letting them overlap gives that failure nowhere to happen.

An earlier version did try to be clever about it: push them apart, pick one at
random to stand aside for three seconds, shove that one sideways out of the
other's lane. It worked, eventually, after two rounds of fixing what the fix
broke — but every part of it existed to stop bodies overlapping, and once
overlapping is allowed the whole apparatus has nothing left to do.

A Bim sitting at the table or asleep in its bunk slows nobody: it is tucked
into the furniture rather than standing in the gangway, and walking past the
foot of a bed should cost nothing.

## Controls

| | |
| --- | --- |
| Click the fridge | Menu: what is left, **Make a stew**, **Make a bowl**, the door |
| Click the stove | Menu: turn on/off — James walks over and flips it |
| Click a bunk | Menu: whose it is, **Assign to** the Bim shown or **Give up this bunk**, and on James's own bunk **Nap** (30 min) or **Sleep** (6 hours) |
| Click the dishwasher | Menu: what is stowed, and **Run now** |
| Click the hydroponic bay | Menu: the trays, **Automate**, and what to plant |
| Click the broom locker | Menu: **Sweep up** — and they get round to it themselves |
| Click the toilet | Menu: **Use** — and a wash at the basin after |
| Click the bathroom door | Menu: open/close, lock/unlock |
| Click the armoury, a shelf, or **Open** on the cold store (ship only) | Its grid — the lockers, the shelves, the food — and the inventory of the Bim shown beside it; the Bim walks over |
| Right-click a cell | **Take** · **Store** · **Equip** · **Discard**, or **Unequip** on a worn slot — greyed with the reason when it cannot go |
| Ctrl-click a cell | The quick move: container to pack, pack to the open container |
| Right-click a fixture | The same menu, on the other button |
| `1` | Select James (control group 1) — the one you steer |
| `r` | Recruit James, or let him go — see below |
| Drag a box over one | Select it — either of them. Selecting shows its crew sheet |
| Click one | Select it — a click is just a box of no size. Only James takes orders |
| Click empty floor / `Esc` | Deselect — the right-hand panels go with it. `Esc` first shuts whatever is up, innermost first: a cell's rows, a fixture's menu, a grid window |
| Right-click the floor | Send the selection there — opens a door on the way if it must |
| Tray, bottom left | **Schedule** — paint the day and set the thresholds; **Management** — autonomy, food to keep, what is aboard; on the ship, **Build** — lay parts out for the crew to build |
| Action thresholds, under the strip | Rest and Food: how low each may get before the Bim acts, and a tick box to stop it acting at all |
| Point at anything | Top left says what it is, and what is lying on it |
| Point at a management row | The place it names is ringed on the deck |
| Let the Bim decide | Whether the crew start errands on their own when idle |
| Speed slider, top right | Run the simulation from 1x up to 24x |
| Rest on an underlined word or a ? | It explains itself, after a third of a second |

Nothing on the page explains itself in prose any more. The explanations are in
tooltips, and a tooltip is always *asked for* rather than stumbled into. Two
rules, and nothing else pops anything up:

- **An underlined word.** Where the thing already has a name — each need, Health,
  "Let the Bim decide", "Speed" — the name carries it, dotted-underlined and
  with the help cursor. Hovering the row, the bar or the panel does nothing.
- **A "?".** Where there is no word of its own to underline, a small question
  mark sits beside the controls: the schedule's is at the end of the brush row,
  past **Sleep** and **Everything**, and **Action threshold** has one of its
  own — it names a block of two rows rather than a single control, so there is
  no one word the explanation belongs on.

Either opens after the pointer has rested on it for 300ms. The delay is the
point: crossing a panel on the way somewhere else sets nothing off. The ring a
management row draws round its fixture is not one of these and needs no
affordance — nothing pops up, nothing is said, and it is gone the instant the
pointer moves on. The one number inside a tooltip that could go stale — the
rested-enough level in the schedule's — is read off the room rather than
written into the words.

Either button opens a fixture's menu, and doing so leaves the selection alone;
a *sweep* across one is still a marquee. Right-clicking bare floor is still a
move order — the hit test decides which of the two a right-click meant. Nothing
is greyed out for being busy any more: a new errand takes over and the old one
goes on the agenda (below). Items are only ever greyed out for a reason of their
own — a locked door, a dishwasher with nothing in it.

## Nothing is remote

Every menu item is an errand, including the ones that look like switches. Asking
to open the fridge, work the hob, open or lock the bathroom door, or start the
dishwasher sends the Bim walking to that thing; the state changes at the moment
its hand arrives and not before. There is no way to reach into the room from
outside it.

That falls out of one shared pair of steps — walk to it, put a hand on it —
carrying which switch on the task rather than in the step, so a switch errand
queues, resumes and shows up on the agenda exactly like a meal does. It also
means a toggle reads the state at the moment the Bim touches it rather than when
the player asked, which is the honest reading: ask for "turn on" and then change
your mind twice while it walks, and what you get is what the hob was actually
like when it got there.

Where a fixture has two sides — the bathroom door — the Bim goes to whichever
panel it is nearest, so it can let itself out as readily as in.

Produce is held to the same rule. A plant lifted from the hydroponic bay is in
the Bim's hands until it has carried it up the room and put it in the cold
store — see [The harvest is carried](#the-harvest-is-carried).

## Interrupting, and getting back to it

Send a selected Bim somewhere, or start it on something else, and the new thing
wins immediately. What it was doing is not thrown away: it goes on a queue with
its progress intact, and the Bim picks it up again as soon as it is free — after
walking to wherever it was sent, or after the errand that displaced it.

The queue is a stack. Each interruption goes on the *front*, so a chain always
resumes directly after whatever displaced it: interrupt a meal with a nap and
the nap with a trip to the heads, and the order back out is heads, nap, meal.

Resuming is the part with a trap in it. Standing steps assume the Bim is already
in the right place — `Chop` chops whatever is in front of it — so dropping the
Bim back on the step it was interrupted at would have it chopping thin air in
the middle of the floor. Instead, resuming rewinds to the most recent *walking*
step of that chain and lets the Bim walk back to the bench first, then drops
into the interrupted step with the elapsed time and the count of knife strokes
it had already done. The steps skipped on the way in are exactly the ones whose
work the room is already holding: a chopped vegetable stays chopped, a full pot
stays full.

What the room cannot hold is what was in the Bim's hands, and whether it was
sitting on something — `set_scripted(false)` throws both away. So those are
snapshotted too, and put back after the walk. A Bim interrupted mid-meal walks
back to the table, sits down again with its fork, and carries on eating.

Two cases need the world tidied on the way out, or the Bim would be stranded:
being lifted out of the bunk, and being let out of the heads. The second matters
because the Bim locks the door behind itself, and a locked door is a wall to the
pathfinder — being called out of a locked room unlocks it first, or the order
would have nowhere to go.

### The agenda

The panel over the top-left of the deck is the checklist for all of this. One
row per chain — the one running first, then the queue in the order they come
back — each with a progress bar at the left-hand end, a tick box that is filled
for the chain actually running and empty for one that is waiting, and the name.
A queued row keeps the progress it had when it was put down, so you can see the
meal sitting at 32% while the nap it was interrupted by runs at the top. The
panel hides itself when there is nothing on.

## Make food

The whole chain, about fifty seconds of it, runs off one menu item. Every beat
is animated rather than implied: the Bim walks to the fridge and opens it,
takes out a vegetable and shuts the door, carries it to the board, fetches a
knife from the counter drawer, chops — the vegetable visibly shrinking as
slices pile up beside it — puts the knife down, gathers the slices into the
pot, turns the hob on, waits while it bubbles and the contents turn from raw
green to stew, fetches a plate and spoon from the drawer, spoons three helpings
across onto the plate (the pot keeps the rest), turns the hob off, carries the
plate to the table, sits down, eats it with cutlery until the plate is empty,
sits a moment, gets up, gathers the plate and the cutlery, carries them to the
dishwasher, opens it, stacks them, shuts it — and, if that was the tenth plate,
presses the button — then goes back to wandering.

## Two recipes, and a cold store that runs out

There are two things the Bim can make, and both start the same way — fridge,
board, knife — before parting company:

- **A stew.** Two vegetables, fetched one at a time, chopped one after the
  other, tipped in the pot and cooked. The second trip skips the drawer,
  because the knife is already in hand. **A pot holds two helpings**: the Bim
  has a plate of it now and the rest of it later.
- **A bowl.** One block of tofu chopped into cubes, with the salad that came
  out of the fridge alongside it, tipped into a bowl and eaten cold. No pot, no
  heat, and about two thirds the time of a stew.

The loop and the fork in the chain are the same one mechanism: `Step::next` is
still a straight line, and `Task::next_step` overrides it in the three places
where the recipe matters — round again for the second vegetable, skip the
drawer on that second trip, and turn off towards the bowl instead of the pot.

### The pot keeps

A stew is cooked once and eaten twice. Tipping it in fills the pot with two
helpings; serving a plate takes one. When the Bim is hungry again and there is
still something in the pot it goes back to it — a plate out of the drawer, the
rest of the stew, and the same sit-down and clearing-up — which is half the
time of a fresh meal and costs the store nothing. After the second plate the
pot is empty and the next meal is cooked from scratch.

It is the same chain as a meal, started part-way along: at the drawer rather
than the fridge, and skipping the hob on the way past, since nothing was lit.
The hob menu offers it by hand as **Eat from the pot**, and a Bim deciding for
itself always prefers it — cooking a second pot on top of the first would throw
the first away.

The helping comes off the pot when the serving *finishes* rather than spoonful
by spoonful, so a serve that was interrupted and started again costs the pot
nothing. What the spoonfuls move is the picture.

### The cold store

The cold store is **two counts, not one**: vegetables and blocks of tofu, and
they are not interchangeable. A stew is two vegetables; a bowl is one block of
tofu with one vegetable as the salad. So either can be the thing that runs out,
and a store of nothing but tofu feeds nobody. It starts at twenty and ten —
two to one, the ratio the Bim eats at — and the fridge menu says what is left
of each.

The only thing that puts any back is the hydroponic bay, below. Left to itself
the store still runs down: two meals a day against ten days of greens, and
after that the Bim cannot eat at all, which is what the next part is about.

## Going hungry

Staying hungry costs the Bim, in three stages. What is measured is *time spent
with nothing in it* rather than how empty the bar is — being hungry for an hour
is just being hungry — so the stages are thresholds on a clock that runs while
the stomach is empty and winds back three times as fast once it is eating again.

| stage | after | walks at | tires | health |
| --- | --- | --- | --- | --- |
| Mild malnutrition | 8 h empty | ×0.85 | — | — |
| Malnutrition | 16 h | ×0.70 | ×2 | — |
| Extreme malnutrition | 24 h | ×0.55 | ×3 | 100 → 0 over a day |

Only the last stage costs health; the first two are a warning, and feeding the
Bim at any point walks the whole thing back. Health regrows over two days of
eating properly. At zero the Bim dies: whatever it was doing is dropped, the
queue is emptied, it stops where it stands and is drawn cold and flat on the
deck, and nothing will take an order from it again.

The stages compound on purpose. Tiring three times as fast means a starving Bim
spends more of its day asleep, which is less of it spent doing anything about
being starving. Unlike the needs, that clock keeps running while it sleeps —
you do not stop starving because you are asleep.

Health sits under the needs on the right, with the stage named underneath it,
and the state of its sleep named under that.

## Going without sleep

The same shape as going hungry, and measured the same way: time spent on
nothing, not how low the bar is.

| stage | after | errands take | and |
| --- | --- | --- | --- |
| Sleepy | 4 h on empty | ×1.25 | — |
| Sleep deprived | 10 h | ×1.50 | — |
| Past it | 18 h | ×2.00 | drops off where it stands |

The slowdown is not a multiplier on a timer. A step that finishes has a chance
of having to be done again — the Bim stands there for as long as that step
takes and then starts it over — and the chance is set from the target rather
than guessed at. A step repeated with probability *p* takes `1 / (1 − p)` times
as long on average, so the three stages use *p* = 0.2, 0.333 and 0.5. Measured
over a dozen runs of the same meal, that comes out at ×1.29, ×1.59 and ×1.93.
The fumbling is random; what it costs over a whole errand is not.

The status line says *Lost the thread of it…* while it is stalled, because
otherwise a chain that has silently doubled in length reads as the game having
hung.

At the worst stage the Bim also drops off on its feet, for fifteen minutes at a
time, roughly once every three quarters of an hour upright. It recovers rest at
the proper sleeping rate while it is out — which is worth about four per cent —
and it keeps hold of whatever it was carrying and whatever route it was on, so
it carries straight on when it comes round.

What a nod-off does **not** do is clear the state. That lifts only when the Bim
has properly slept and is back above 80% rested, which four per cent at a time
will never reach. The only way out is a night in bed, which means the timetable:
wipe the strip and a Bim will run itself into the ground and stay there.

## The hob turns itself off

A hob left lit with nothing coming up to heat shuts itself off after fifteen
game minutes. The menu says so while it is counting — *left on — cuts out in
12 min* — because a ring going out on its own with no explanation reads as a
bug rather than as a safety feature.

The word doing the work is *cooking*, and it means something in the pot that has
not finished heating — not merely that the ring is lit. The distinction matters,
and the margin is thinner than it looks. A meal keeps the hob on for about
fifteen game minutes in total, which a naive "fifteen minutes lit" rule would
cut short at the worst possible moment. But the stew is only *coming up to heat*
for the first five of those: the rest is the Bim fetching a plate, spooning out
a helping and reaching for the knob. Counting only the idle time leaves the Bim
turning the hob off with five minutes still on the clock, which is what it does.

## The dishwasher

A unit in the galley counter front, between the drawer and the hob. It stows ten
plates, one per meal, and the tenth one in is what starts a cycle. The cycle
takes two hours on the world clock and runs entirely on its own: the Bim walks
away the moment it has pressed the button and is free for all of it.

Plates are counted in two places, which is the only part of it worth explaining.
`loaded` is the dirty stack waiting to go round; `washing` is what is going
round now. Starting a cycle moves one to the other. Without the split, a plate
cleared away *during* a cycle would be washed and then thrown out with the load
it was never part of; with it, that plate simply waits for the next one. The
pips across the front show both — bright for a plate waiting, dim for one going
round — so a running machine looks full rather than looking empty.

Everything else falls out of that. A cycle that cannot start (nothing in it, or
one already running) is simply refused, and the next Bim to finish a meal tries
again. The rack goes back into the galley stores when the cycle ends, which is
the one bit of hand-waving in here: nobody unloads it.

## Sweeping up

The broom lives in a locker set into the port bulkhead, between the foot of the
first bunk and the hydroponic bay. It is the **only thing aboard that undoes a
mess**: everything else either makes one or gets out of its way.

A Bim with nothing else on fetches it and sweeps. That is the whole trigger —
sweeping sits at the very bottom of `consider_errand`, below every need, below
a scheduled night and below the bay, so it is what a Bim does with time it has
nothing better to spend. Anything arriving interrupts it exactly like any other
errand, and the half-swept deck goes on the queue to be picked up after. You can
also ask for it: the locker has a **Sweep up** item, greyed out when the deck is
already clean.

The chain is fetch, sweep, fetch again: **broom out → walk to the worst tile →
sweep it → walk to the next → …** up to five tiles, then the broom goes back.
Five rather than "until it is done" so that hunger and the heads get a look in
between armfuls; if the deck still wants it and nothing else has come up, the
Bim goes straight back for the broom.

Which tile is next is worst-first with distance counting against it, so the Bim
works outwards from where it is standing rather than crossing the compartment
for the single filthiest tile every time. A tile comes all the way clean in one
go — it is swept or it is not — and the **time** is in the chain rather than in
chipping away at the score.

Two things worth knowing about the corners:

- **It sweeps the tile it set out for, not the one under its boots.** They
  differ whenever the dirt is somewhere a body cannot quite stand, and a broom
  has the reach for that. Sweeping underfoot instead would leave those tiles
  filthy for ever *and* send the Bim back to the same one every time, because
  it would still be the worst on the deck.
- **A tile nobody can get to is never chosen.** Some of the deck is deck and
  still unreachable — the corner past the end of the counter, hemmed in by the
  bunk. A mess there would otherwise be picked as the worst tile for ever, with
  the Bim fetching the broom, failing the walk, giving up and starting again.

There is one broom, so one Bim sweeps at a time; the other's errand simply does
not start, the same way the galley and the heads work. And the locker door is
drawn from *whose hands the broom is in* rather than from a flag of its own, so
a chain given up mid-sweep cannot leave the cupboard claiming to hold a broom
that is somewhere else.

## The hydroponic bay

Six trays along the bottom-left wall, and the only thing aboard that puts food
*back* into the cold store. Right-click it for its two controls.

**Automate** is on out of the box and puts the bay on the manager's target
(below) — a bay that has to be switched on is a bay that is off whenever the
player has not noticed it, and at dawn the store already holds what the target
asks for, so it sits quietly until the first meal dips below the mark. While
the store is under it the bay has work, and the Bim walks over and does it a tray at a time:
lift anything ripe, then plant whatever is missing. Greens come up in **one
day**, soy in **a day and a half** and is pressed into tofu.

Six trays is one bay per Bim, near enough. At the two-to-one ratio that is four
trays of greens and two of soy, which comes out at about four vegetables and one
and a third blocks of tofu a day. A Bim eats two meals a day and a pot covers
both of them, so it gets through two or three vegetables and a block of tofu —
the bay keeps up, with a little to spare for the days it cooks twice.

What it plants is decided by which of the two the store is furthest behind on,
as a *share* of what was asked for. The share matters: measured in plain
numbers the bigger target would always be the shorter one and the bay would
fill all five trays with greens, so the tofu it is just as far behind on never
went in. Measured proportionally the trays come out at roughly the ratio that
was asked for, which is the point of a food unit having two halves.

**Hibernation** is what happens when both marks are met: nothing grows, nothing
is planted, nothing is lifted, and the trays hold exactly what they hold. The
lights over them go out, which is how it reads across the room. Drop below the
mark again and it picks up where it stopped, part-grown plants and all. A bay
**without power** — on no conduit, or aboard a ship that has browned out —
is in the same state, whatever the store holds and whatever was ordered,
and its menu says *no power* rather than *at target*; see *The reactor, the
batteries and the brownout*.

**Plant greens / soy / fibre in every tray** is the override: a standing
order that ignores both the target and hibernation, for when you want a bay
full of soy whatever the store says. Click it again to lift the order and go
back to the target.

**Fibre** is the third crop, and the one nobody eats: a paler, taller
stalk, a day in the tray, carried to the cold store like the greens, and
what the drug lab makes bandages out of, two to one. It has a target of
its own on the Management tab — a fourth row under the food — and it
starts at **nought**, which means never: the share arithmetic that
decides what to plant reads a target of nought as nothing wanted, so a
bay nobody has asked for fibre goes on growing food exactly as it did.
Set the target and the bay grows fibre beside the greens and the soy,
furthest-behind first, like everything else.

Both controls are *settings* rather than errands — the same kind of thing as
the timetable or letting the Bim decide. The deciding is the player's; every
bit of the doing is still the Bim's, on foot, one tray at a time.

### The harvest is carried

A plant lifted out of a tray is **in the Bim's hands**, not in the store. The
bay is at the bottom-left wall and the cold store is at the top of the room, so
the tend errand carries on past the tray: up the room with the vegetable,
fridge open, plant in, fridge shut. Only then does the count in the management
tab go up.

That is [Nothing is remote](#nothing-is-remote) applied to the one thing that
was still cheating. The trays used to empty and the store used to fill in the
same instant, with the Bim standing twenty feet away — which is the exact
pattern the rest of the game exists to avoid, and it showed: a bay running flat
out looked like it was posting produce through a wall.

There is a real consequence, not just a nicer animation. The walk is most of
the errand now, so a bay working hard costs the Bim a noticeable part of its
day, and a harvest interrupted half way is a Bim standing about holding a
carrot until it gets back to it. The crop rides along in the saved chain, so
being pulled off it loses nothing; the only case where produce is banked
without a hand on it is a chain given up for good because a door shut across
the walk, and putting it in the store then is the lesser of the two wrongs.

A **planting** has nothing to carry and ends at the tray, which is why a
planting's row on the agenda vanishes at about half a bar. That is deliberate:
weighting the chain by what the Bim turns out to be holding would make the bar
run *backwards* the moment a harvest came up, because nothing is in its hands
until the tray is already worked.

## The manager

**Management** in the tray is a table of **targets**: one row per thing the
place is told to keep up, with what there is and a *keep at least this
many* beside it. Four rows — **vegetables**, **tofu**, **stew** and
**fibre**, all in the cold store — and each is a standing order: below the
mark the work goes on the crew's list by itself (a tray planted and the
harvest carried for the first two and the last, a pot cooked for the
shelf for the third), and at the mark it comes off. **0 means never**, and
stew and fibre start there on purpose. The food-units dial with its
two-to-one split, which this used to be, is gone: a target is a count of
the thing itself.

The same rows are **what is aboard**: what it is, how many there are, and
where they are — the four in the cold store, and under them, aboard the
ship, whatever the benches have made. It is built from a list rather than
written into the markup, so a new thing to keep track of is a new line rather
than a new table.

**Resting on a row rings the place on the deck.** "Cold store" and "Pot on the
hob" are words, and the room is full of grey rectangles standing against the
same wall; pointing at the row draws a cyan ring round the actual fixture, so
the two do not have to be matched up by eye. The food target does the same for
the hydroponic bay, which is what works to it.

It is not a tooltip and does not go through the tooltip machinery: nothing pops
up, nothing is said, and a pointer crossing the panel on its way somewhere else
lights a fixture for a moment and leaves nothing behind. That is why it can
hang off a whole row rather than needing a word of its own to underline — every
row is about exactly one place. The ring is drawn *over* the night wash, since
a highlight that dims at three in the morning is no highlight, and in the
ship's own cyan rather than the green that means "selected" or the warm colours
that mean trouble.

The speed slider used to sit here too. It is in the header now, beside the
clock it is speeding up.

## Work priorities

The **Work** tab is the third in the tray, and it is the answer to "what should
they do first". One row per job, a number from **1 to 5** in a box beside each,
and **1 is done first**. Click a box and it steps one less important, from 5
back round to 1. Everything starts at **3**, all equal, so out of the box this
changes nothing at all — the ship behaves exactly as it did before the panel
existed, which is deliberate: a default that is already an opinion is a default
you have to undo before you can use anything.

| | |
| --- | --- |
| **Cleaning** | sweeping the deck |
| **Planting** | sowing an empty tray in the bay |
| **Plant cutting** | lifting a ripe one out of it |
| **Hauling** | carrying what was lifted to the cold store |
| **Cooking** | making a meal, and stew for the shelf |
| **Controlling the ship** | standing at the helm while the ship is under way |
| **Making things** | working a bench the manager has an order for |
| **Mining outside** | a walk out to the belt in a suit |
| **Building** | putting a laid-out part together once its materials are there |
| **Medical** | treating a crewmate's dying state with a medkit, and dressing a wound — its own, or a crewmate's — with a bandage |

The colour of the box says what the number means without anybody having to
remember which end is which: warm at the top of the list, cold at the bottom,
five steps across the palette. At the top of the panel are three ways to
reorder the rows — **Priority 1→5**, **Priority 5→1** and **Name A–Z**.

Sorting is something you *press*, not a rule that stays on. If the list
re-sorted itself live, the row you just clicked would jump out from under the
pointer — and at the wrap from 5 back to 1 it would jump the whole length of
the list. So the button reorders the rows there and then, and the mark showing
which order was applied is cleared the moment a box is clicked, because the
list may no longer be in it.

Resting on a row rings the place the job happens, the same as a management row:
the locker for cleaning, the bay for both bay jobs, the cold store for hauling,
the hob for cooking — **every one of that kind**, so a ship with two galleys
sees both hobs rung. Building and medical ring nothing: a site is wherever it
was laid out and a patient wherever it fell. Nothing pops up and nothing is
said — see [A highlight is not a tooltip](#what-the-pointer-is-over).

**Medical is the one row that can interrupt.** It is on offer while somebody
aboard is dying and there is a medkit to hand — the nearest crewmate in a
dying state, its worst trauma first; never the Bim's own, since nobody
treats their own — or bleeding and there is a bandage to hand — the Bim's
own wounds first, else the crewmate with the most open, and that one's
worst part — and at
any number from 2 to 5 it waits its turn like the rest, though among equals a
wound is dressed before the blood under it is swept up. Set it to **1** and it
is urgent: a Bim drops whatever it is on the moment there is a wound to dress,
the errand going onto the queue the way a meal would push it, and picks it back
up when the hands come off. Set it to **never** and nobody doctors of their own
accord; your own bandage orders from the inventory or the menu on a body still
work. A patient nobody can walk to, or one somebody is already walking over to
dress, is not offered. A patient somebody is walking over to holds still
for them.

### What it actually changes

Only what a Bim takes on **of its own accord**. A Bim with nothing pressing
looks at whatever work is going — a tray asking, a deck wanting the broom, its
own hunger — and does the one nearest the top of the list. Whether it then gets
on with it is a separate question: there is one galley and one broom, so the
other Bim may have the thing it needs, in which case it moves down the list
rather than waiting in line.

Two things the list deliberately does **not** touch:

- **Sleep and the heads are not work.** There is no row for either and no
  number to set. A timetable and an action threshold are how those are steered.
- **Nobody starves for it.** Cooking at the bottom with a deck that never comes
  clean is exactly the arrangement a player will try, and a Bim is allowed to
  put the meal off — but once going without has actually begun to tell on it,
  the meal jumps the queue whatever the cook row says. The list is a statement
  about what to do next, not about whether to eat at all.

**Hauling is two things under one row.** A harvest is carried in the same
chain that lifted it, so that half of hauling is the back half of a cutting,
and a cutting waits on whichever of **Plant cutting** and **Hauling** is set
later; put hauling at the bottom and the bay stops being emptied, which is
the truthful answer: there is nobody to carry it. The other half is an errand
in its own right — a load off a shelf, walked to a construction site and put
down there, one trip at a time, while a site still wants something (see
*Building, aboard* under the game) — and that one is offered and taken at
hauling's own number.

## The timetable

The tray at the bottom left folds away and has two tabs. **Management** holds
the controls that used to sit in the page header — whether the Bim decides for
itself, the speed, and what to keep in stock. **Schedule** is a strip of
twenty-four hours you paint
with one of two brushes: blue for sleep, grey for everything else. The hour the
clock is on is ringed.

It comes with a night already painted in: **22:00 through to 04:00**, six hours,
the same length as a night actually is. Wipe it with the grey brush if you want
the Bim left entirely to its own devices.

Only sleep is timetabled so far, and **the timetable is what sends the Bim to
bed on an ordinary night**. Running low on rest is not a reason in itself for a
scheduled night: the level decides whether that block is worth taking, not
whether to have one. Underneath the timetable there is a floor — the **Rest
threshold**, below — which catches the Bim when the timetable has not. Wipe the
strip entirely and the threshold is all that is left: the Bim goes to bed when
rest runs past it, whatever the hour. Untick that too and it never sleeps at
all.

The timetable is still not an order. When the clock walks into a painted block
the Bim adds a sleep to the **back** of its agenda — behind whatever it is doing
and behind anything already queued, unlike an interruption, which jumps the
front. A standing instruction waits its turn.

Two rules keep it from being silly:

- **Already more than 80% rested, and it ignores that block.** Being sent to
  bed nearly rested means a walk to the bunk, a climb and a walk back for a few
  minutes' lie-down. It declines that particular block rather than the
  timetable as a whole.
- **Fully rested, and it gets up** — whatever the hour says. So a scheduled
  early night taken at three quarters rested runs about ninety minutes rather
  than the full six hours, and the Bim is up and about again.
- **Hungry, and it eats first.** A sleep waiting at the front of the agenda
  stands aside while food is past its threshold and there is something to cook:
  turning in starving costs the Bim six hours of losing health and it wakes no
  better off, where the meal costs it three quarters of an hour and puts the
  need away entirely. The sleep is held, not dropped, and goes ahead the moment
  the plate is cleared. It only stands aside while a meal is actually to be
  had, and while the Bim is free to go and have it — with the cold store empty,
  the galley behind a locked door, or autonomy switched off, bedtime goes ahead
  as it is. Waiting on a meal nobody is going to cook would be a Bim that never
  sleeps at all.
- **Under 80% on the restroom need, and it goes first.** You go before bed.
  Unlike the meal, this one does not wait for the 10% threshold: the restroom need
  is the only one that keeps draining through the night, and a night is six
  hours — turn in at four fifths and the Bim wakes at nothing. So anything under
  four fifths is worth emptying out first, and the sleep waits the twenty
  minutes it takes. The same guards as the meal: only while the Bim is free to
  go, and only while the pan is reachable — a locked door is not a reason to
  keep a tired Bim up. This is half a rule on its own: holding the sleep back
  does nothing unless something *starts* the trip, and the need is nowhere near
  the trigger the errand loop watches, so `consider_errand` starts it by name.

A block fires once however long it is: painting the whole day sends the Bim to
bed once, not twenty-four times. Painting sleep onto the hour it already is
takes effect immediately rather than waiting for the next hour to tick over.

A block fires on the way *in*, which has one edge worth knowing: painting the
whole day makes a single block with no ordinary hour to re-arm it, so it asks
once and never again. Blocks the clock walks into are the ones that work.

The default night is what keeps the Bim's drifting body clock (below) anchored,
and it does it the way daylight does for an animal. Six nights running it went
to bed at 22:01, 22:00, 22:00, 22:05, 22:03, 22:00, arriving at ten o'clock
between 8% and 14% rested every time and getting up around a quarter to four,
fully rested and a little short of the six hours. A 25-hour rhythm pulled onto a
24-hour day is exactly what entrainment looks like.

## Action thresholds

Under the hour strip, in the same tab, sit the two levels that say **how low a
need may get before the Bim does something about it**: one for Rest, one for
Food. Each is a tick box and a slider, and each starts ticked at 10%.

(In the code these are `needs::Trigger` — a level and a switch. "Threshold" is
the player's word for the setting; "trigger" is what the mechanism does.)

They are the other half of the timetable's question. The strip says when the Bim
*may* sleep; the Rest threshold says when it should go anyway. A Bim whose night
has been wiped off the strip, or who has been kept out of bed by a long errand,
falls past the threshold and turns in on its own account — which is why wiping
the timetable no longer means a Bim that never sleeps.

A threshold fires the same errand a need has always fired, so everything that
already governed those errands still governs these:

- **A meal comes first.** The Rest threshold stands aside for hunger exactly as a
  scheduled night does, and for the same reason.
- **So does a trip to the heads.** You go before bed, and the Rest threshold waits
  the twenty minutes it takes. The two rules have to be paired: the threshold
  refusing to start the sleep is what makes something else start the trip.
- **Autonomy and orders still outrank it.** With *Let the Bim decide* off, or
  the Bim recruited, no threshold starts anything.

**Unticking one is not the same as setting it to nothing.** The need carries on
draining and everything going without does to the Bim still bites — a Bim with
the Rest threshold off still gets sleepy, sleep-deprived, and finally drops off on
its feet. All that stops is the Bim going and doing something about it on its
own account. Three days of that leaves it hovering just under the level it would
have acted on, clawing back a few minutes at a time from nodding off where it
stands.

Restroom and Surroundings have no threshold on the page. The first is not something
a player should be able to talk the Bim out of; the second has no errand behind
it to start.

One thing the numbers assume, and worth knowing before you move one: the drain
rates are derived from a tenth. Each need is written as the drop from full to
10%, divided by how long that drop is supposed to take, so two meals and three
visits a day come out of the arithmetic rather than being tuned by eye. Tell the
Bim to eat at half full and it eats more often than twice a day — which is the
point of being able to say so, but it is no longer the day the rates were built
for.

## What the pointer is over

Top left, above the agenda, one line says **what is under the pointer**. Deck
plating, a bulkhead, the worktop, the chopping board, the cold store, the hob,
the dishwasher, the table, the chair, the bunk, the hydroponic bay, the toilet,
the basin, the bathroom door — and *outside the hull* if the pointer is off the
ship altogether, which it can be, because the room is a fixed size letterboxed
into whatever window you have.

Where there is something worth saying about the state of it, it says that too:
the hob **lit**, the cold store **open**, the door **locked** or **shut**, the
dishwasher **running**, the bay with **two ready to lift**.

And on deck, a second line says what is lying there: **Grime**, **Wet**,
**Soiled**, **Vomit** or **Blood**, with how far down that tile has been taken. The simulation itself has
never distinguished one stain from another — a tile is one number, and the
average around the Bim is all anything reads — so the kind is remembered
alongside the score purely for this readout. It keeps the worst of what has
happened rather than the latest: being sick on a tile already wet reads as sick.

The readout is a different question from a click, and answers accordingly. A
click asks *what would this act on*, which is why only the few things with a
menu behind them are hit-testable and why each of those is given a few pixels of
slack. The readout asks *what is this*, so everything aboard has a name and
nothing is expanded: a pixel beside the pan is deck, not toilet.

## Recruiting

Press **r** and the Bim is under direct orders. It stops doing anything of its
own accord: no errand from a need, no sleep from the timetable, nothing picked
back up off the queue, and none of the pottering about it does between jobs. It
stands where it is put until told otherwise. Press **r** again to let it go.

What recruiting does *not* touch is the going-without. The needs drain exactly
as before and every stage of hunger and drowsiness bites exactly as hard — a
recruited Bim left alone for two days still ends up at the worst stage of both,
and walks and fumbles accordingly. Being under orders is not being looked after.

Everything the player asks still works: move orders, fixture menus, all of it.
Whatever it was in the middle of when recruited is left to finish rather than
cancelled, since throwing away a half-cooked meal for tidiness would be worse,
and a right click interrupts it anyway. The queue is kept rather than emptied,
so letting the Bim go picks up where it left off.

It shows in two places, because one of them is easy to miss: a badge in the
header, and a broken amber ring around the Bim itself, in a colour used for
nothing else and drawn whether or not the Bim happens to be selected.

### Combat mode, and the inventory

**The rest of the crew take arms on their own.** Whenever an enemy is
within thirty tiles of anybody or in anybody's sight — a compartment or
two away, not merely somewhere on the station you are docked at — or a
crew member has been hit in the last half minute, the
header says **To arms — an enemy is near** and every crew member but the
one you steer is in combat mode of their own accord: they take a weapon
out of their pack if their hand is empty, draw it, and **gather round
you** — a slot each beside and behind the Bim you steer, which they
keep to as you move, shooting whatever they can see from there. Only
when one of them **sees an enemy for itself** does it go and fight the
way an enemy's people do — walking to cover within range, peeking round
it — so a squad does not charge in headfirst after something only the
log knows about. **And while the alarm is up you can order them**: click
a crewmate to select it and right-click the deck, the way you send your
own; it goes and holds that spot, shooting from it, until the alarm is
over. In peace a crewmate takes no orders, as before. The alarm lasts
until nobody is near, nobody has seen an enemy and nobody has been hit
for half a minute, when they go back to their errands — and to bed —
however many of the station's people are still alive somewhere on it.
(A crew member that bled past the line in the fight is not asleep but
**out cold** on the deck until somebody bandages it; see the medical
notes.) A hired mercenary does the same. The Bim you
steer is yours alone: recruit it yourself or leave it to its work; the
alarm never touches it.

A recruited Bim is in **combat mode**: it draws its weapon — held in both
hands, out in front — and, whenever an enemy is in range and in its own
line of sight — the peek round a wall included — it fires. Everybody
carries a **hand laser pistol** from the start; the armoury makes four
more, and a Bim swaps to one out of its pack. It does nothing else about
the enemy: it stands where it was put, as a recruited Bim does, and shoots
from there; walking it somewhere is still yours to order, and it holds its
fire while it walks. Let it go and the weapon is holstered.

Every shot is a **bolt** that flies — at the weapon's speed, until it
reaches a body, a wall or the end of its range — and is always drawn,
whatever it flies through, each weapon's own: the pistol's dash, the
shotgun's fan of orange pellets, the rifle's short yellow tracer, the
sniper's long white-blue streak. Friendly fire is **blue**, an enemy's
**red**. A weapon's numbers are two points on a line: an **accuracy** and
a **damage** that hold out to its sweet distance and fall off in a
straight line to what they are at its range. A shot's odds of landing are
the accuracy at the distance to the target, rolled when it is fired; the
damage is read where the bolt *lands*, at the distance it flew, and comes
off the body it lands on; a miss flies visibly wide. The **pistol**
reaches twelve tiles, 95% falling to 65%, six a hit, one and a half a
second. The **shotgun** reaches ten: 90% and fifty a hit out to four
tiles, 60% and thirty at ten, one shot every four seconds. The **auto
rifle** reaches sixteen, 85% to eight tiles and half that at the end, five
a hit — but a pull of the trigger is a **burst** of eight in two seconds,
then two seconds to recharge, and every shot of the burst is rolled and
aimed afresh. The **sniper rifle** reaches thirty-five: a certain hit and
forty-five out to twenty tiles, 70% and twenty-five at the end, a shot every four
seconds, and its bolt is the fastest thing in the game. The **schword** is
no gun: a blade with a laser edge, thirty-five a swing every two seconds —
the swing takes half a second and the blow lands as it ends, so a body
that steps back in time is missed — on whoever is within arm's reach —
a tile and a bit — and a cut is a
**three-unit wound** that bleeds three times what a shot does and throws
blood over the tiles round the body. Who is an enemy is the station's
business: at a hostile station the people living there are enemies,
ringed in red, and `nix run .#combat` opens the game docked at one.
**They shoot back**, each with what it was issued off the station's own
seed — half of them the pistol, a fifth a shotgun, a few a rifle, fewer a
sniper rifle, one in ten a schword. The moment the crew are aboard a
hostile station its people are at war: they know where the crew are
without seeing them, and each picks where to shoot from — against a wall
with a peek round it for choice, otherwise as far off as its weapon
reaches — walks there, and fires at whoever it can see from the spot; one
with a schword charges instead, to the free tile nearest its target. Their
shots fly in the crew's room, red, and land on the crew's own bodies: a
hit is on the head, the body or the legs, and every hit opens a wound
that bleeds.

Two things a corridor fight turns on. **Peeking exposes the peek.** A Bim
that aims from the peek beside a wall leans out to it, and that is where
the enemy shoots at — but it is in cover, and a bolt reaching a body that
is peeking is **dodged half the time** and flies on. The same for the
enemy peeking at the crew. A line of **sandbags** is **low cover**: a
body can walk over it and see over it, but standing close behind it —
the tile next to it, on the far side from the shooter — half the bolts
coming over it miss, the same as a peek; a bot picks such a spot to
shoot from, and every station's corridors have a barricade of them; a
ship can build them too, under Structure. **Walking halves your aim.**
Every gun shoots on the move at half its odds, so a bot with a shot from
where it stands stays put and takes it, and walks only for cover or
when it has no shot at all; a Bim you send across a room fires as it
goes, at half. **A blade at your throat is a melee.** A Bim
with a gun that has an enemy with a schword within arm's reach is
**locked**: it cannot fire, and brawls with its fists instead — twenty
every two seconds — while the blade lands its thirty-five; a Bim with a
schword of its own is locked with anything in reach and swings. Walking
out of reach ends it. The log says *locked in melee* the step it happens.
A station's people are hostile or not as the galaxy was generated (the
system map rings a hostile station red, and a crew is never started at
one); `combat` makes the dock hostile regardless. **How many of them
there are is up to you**: an enemy station arms two people plus one for
every crew member, and doubles that every time the worth of your ship
and everything in its hold has grown by another half of what you set out
with — so a crew that has been trading and building well finds every
enemy's dock a bigger fight than a poor one does, up to sixteen. The
money in hand is not counted, only the ship and its cargo, and a station
keeps the crowd you reached it with until you have left and come back.
A crew member dying, treated
or dead is said in the log, and so is every hit. What a hit does to a
body, and how it is dressed, is [Getting hurt](#getting-hurt); the
enemy's people bleed and die the same way — one shot to a dying state
runs from you — and nobody dresses theirs until the fight is over, when
the station's own bandages and its one medkit come out.

Tab opens a Bim's **inventory** in a window of its own (a recruit does
not — a fight is not the moment for a window over the deck), and the tray's
**Inventory** tab shows the same for whoever is selected, recruited or not:
three armour slots down the left — head, body, legs — each with the icon
of the piece worn there, a bar of the health it has left and its numbers
(`+15 hp · 2 prot`); the weapon slot on the right with its icon and its
numbers beside it, read off the two-point curve: **Range**, **Accuracy**
("90% to 4 tiles, 60% at 10"; the pistol, with no sweet distance, reads
"95% up close, 65% at 12 tiles"), **Damage** ("100 to 4 tiles, 60 at
10", or "6 a shot"), **Shot speed**, **Fire rate** ("1.5 a second") or
**Burst** ("8 in 2 s, then 2 s") and **DPS**, which is the burst times
the fire rate times the damage — what it could do a second with every
shot landing; a schword reads "Melee — 70 a swing every 2 s", its
**Reach** and its DPS instead. While the Bim is locked in a melee the
header says so — *combat mode — locked in melee*, with a `?` that
explains the fists — and *locked in melee* sits over its name on the
deck; and under the weapon the **pack** on the
Bim's back, a three-by-three grid of cells that each hold one thing — a
piece of armour, a weapon, or one unit of anything else. Beside each
armour slot is the count of open wounds on that part of the body and a
**Bandage** button — see the next section — and under the lot, how many
bandages there are to hand.

**Inventories are grids**, and every container aboard is looked at the
same way. Click the **armoury** (or the drug lab — either opens the
lockers), a **shelf** or, from its menu, **Open** on the cold store, and a
window opens showing what the hold keeps in that class: the lockers
fifteen by fifteen, with each piece of armour in a cell of its own and a
sliver of its health under it and everything else the lockers keep — the
suit, the bandages, the handguns, vests and medkits the armoury makes,
and the shotguns, auto rifles, sniper rifles and schwords it makes now —
as a stack with its count; the shelves twenty by
twenty, with the ore, the metal and the components stacked; the cold
store ten by ten, the food. The window is the class, not the cupboard: a
second shelf is the same twenty by twenty, and a helm put away across a
shelf turns up in the lockers' window, because the lockers are where the
hold counts it. Clicking a container also opens the inventory of the Bim
shown beside it and walks that Bim over, since nothing can be moved until
it stands within **two tiles**. **Right-click** a cell for the rows —
**Take** from a container into the pack, **Store** from the pack into a
container, **Equip** what is in a pack cell (whatever was worn comes off
into that cell; a weapon swaps with the one in hand), **Discard** — and
right-click a worn slot to **Unequip** it into the pack. **Ctrl-click** is
the quick move: a cell in a container goes straight into the pack, a cell
in the pack straight into the open container. A row that cannot go says
why in grey — *walk over first*, *the pack is full*, *no room left in the
lockers* — and a ctrl-click that cannot go opens the rows instead of
doing nothing. Every move is a command, so on every player's ship the
hold agrees. Hover a cell and it names the thing and its numbers; what a
worn piece does for the body wearing it is the next section.

**A body that is down can be looted** — a crewmate dead or out cold, or
one of a hostile station's people lying where the fight left them; nobody
on their feet can be. Right-click the body and the row is **Loot** (beside
the bandage rows for a crewmate who is only out cold). It walks your Bim
over and opens the **Loot** window: the body's pack, three by three, and
under it a row of four — head, body, legs, and the weapon in its hand —
drawn like any container's grid, a worn piece with the sliver of health it
has left. **Take** on a right-click, or ctrl-click, moves one thing into
your Bim's pack, once it stands within two tiles and has a free cell; a
piece comes off the body as it is, broken or not, a weapon goes into the
pack to be equipped from there, and the stripped body draws bare. Nothing
can be put onto a body. Down and reach are checked when the command lands,
not when the window opened: a crewmate who comes round meanwhile is no
longer a body — the window shuts on them — and the command is refused
*not down*. Undock, and a station's body goes with its room.

## Getting hurt

A Bim's health is one bar of a hundred, and it always was; what is new is
that the bar is **three parts added up** — the **head** (5), the **body**
(75) and the **legs** (20) — and the panel shows the three under it, a thin
bar each, with a fourth for **blood**. Hunger still drains and mending
still regrows the bar as a whole, over all three in proportion, so nothing
about going hungry has changed. A shot is what tells them apart.

A shot lands on one part — one in twenty the head, three in four the body,
one in five the legs — and takes the weapon's damage *at the distance it
flew* off that part alone; a blow in a melee, a fist's twenty or a
schword's thirty-five, lands the same way, on a part rolled where it lands.
A part at nothing is not death any more: it is a **dying state**, rolled
for the part the moment it goes, and the panel says it in red under the
bars — *Dying · skull fracture* — with what it is doing under that. The
head's three are a **heavy concussion** (a quarter slower walking and
working), a **skull fracture** (losing ten blood every quarter hour) and
**cranial trauma** (half as fast, and five blood a quarter hour); the
body's an **internal bleeding** (ten a quarter hour, and nothing to see),
**broken ribs** (a quarter slower) and a **severe chest trauma** (five a
quarter hour, and half pace); the legs' a **fractured femur** (ten a
quarter hour), a **shattered knee** (it can barely move — a quarter of its
pace) and, one roll in twenty each, a **crushed right or left leg**: the
leg is **lost, for ever**, a fifth off its pace for good, and the stump
bleeds ten a quarter hour. The part stays at nothing and does not mend;
**only another crew member with a medkit** gets it out (below), and what
the trauma leaves behind stays a while after: the concussion and the
ribs a quarter slower for two days, the cranial trauma half as fast for
a day, the chest trauma at half pace for a day, the knee a quarter slower
for two. A hit on a part already at nothing opens a wound and nothing
more. The panel counts the lasting effects down under the bars, and the
log says *is dying — skull fracture* the moment it happens and *was
treated* when it is over.

A Bim that is dying **runs from the fight** — the enemy are one place,
the middle of wherever they all are, and it runs the other way, round a
wall if it has to, and neither aims nor shoots while it does. Your own
does it too, whatever you told it, and so does one of the station's
people you shot to that state; with the enemy gone it stops where it is
and waits for the medkit. **A crewmate that is merely hurt runs too** —
bleeding from a wound, or down to half its blood — rather than walking
up to the guns for a firing spot the way a whole one does: it gets out
of the enemy's sight, binds its own wound there if there is a bandage
to hand, and comes back into the fight dressed. It stops for anybody
you send over with a bandage or a kit, once they are nearly at it. Your
own Bim goes where you send it, hurt or not, and the station's people
fight on wounded: only dying makes those two run. And a body **out cold
is nobody's target**:
nobody aims at one, and a bolt already flying passes over it.

Every hit, wherever it lands, opens a **wound**, and a wound bleeds until
somebody dresses it: **ten points of blood an hour each**, out of a
hundred, so ten open wounds bleed a Bim out in an hour and one takes ten.
A schword's cut counts as **three** — it bleeds three times what a shot
does, and throws blood over the tiles round the body besides — and a
bandage still closes the lot on a part at once.
Under half its blood the Bim walks at half pace; under **four tenths** it is
**out cold** — lying where it fell, breathing, doing nothing, its errand
put back on the queue for when it comes round — until the blood comes back;
at nothing it is dead, which is how a Bim dies now: bled out, through
wounds nobody dressed or a dying state nobody treated. **Going out cold
drops the gun**: it lies on the deck beside the fallen figure, and the Bim
has to pick it back up when it comes round. A crewmate or one of the
station's people does that on its own — the walk over and a moment bending
for it, *Picking a weapon up* on the agenda; your own waits to be told:
right-click the gun and the one row is **Pick up**, into the hand if it is
empty, else into the pack. A body that dies with its gun on the deck takes
it back, so looting the body finds it. Blood comes back on its own, over two days, once
nothing is open. It shows: a dark blotch on the head, the middle of the
coverall or the boots while that part has a wound open, **blood on the deck**
under a Bim that is bleeding — a drop every second or so a wound, and a drop is
a stain like any other: the tile reads **Blood** under the pointer, the broom
takes it up, and it counts against the deck the way a mess does (see
[Mess](#mess)) — "Bleeding · n open wounds" under the bars,
the blood bar in red once it is low enough to slow it, and every hit and
every crew member down written in the log.

**Armour is worn over all of that**, one piece a part — a helm, kevlar,
leg guards (*Armour* under *What the ship will make* says what each is
made of) — and it is drawn on the Bim: a steel-blue cap over the hair, a
dark plate over the coverall with the yoke still showing, darker boots
with a band across the shin. A piece has a **health** of its own that is
added to the part's — the body at 75 in a 20-health kevlar reads 95, and
the Bim's bar 100 → 120 — and it is drawn as a **blue bar on the end of
the green one**, the number reading `100 hp + 20 hp`, on the crew sheet's
big bar and on each part's thin one alike. A hit on that part goes to
the piece **first**: its **protection** comes off the damage before
anything else, so a shot that does no more than the protection does
nothing at all — no wound — and what is left drains the piece; only what
the piece cannot take reaches the body, and only that opens a wound. A
15-damage shot into a 20-health kevlar leaves the Bim untouched and the
kevlar at 5. At nothing the piece is **broken**: still worn, drawn with a
crack across it, doing nothing — its protection is lifted with its
health — and worth nothing put away, so it cannot be stored or sold, only
discarded and another made. The log says when a piece breaks. What is
worn and what is in the pack go with the Bim and keep their damage
wherever they go: take a dented helm off and put it away, and it is a
dented helm in the lockers, with its health under it in the grid. Taking
"a helm" out of the lockers takes the least damaged one there; selling
one sells the most damaged.

A **bandage** closes every wound on one part. There are two ways to order
one, and both go through the crew member you steer:

- **From the inventory** — the tab, or the pop-up that opens on
  recruiting — for whoever is shown: each armour slot's row says how many
  wounds are open on that part, and its **Bandage** button lights when
  there is one and a bandage to hand.
- **Right-click a Bim** on the deck, yourself or a crewmate, and the menu
  is the three parts — *Bandage the head · 2 wounds* — each greyed with a
  reason when there is nothing open there, no bandage aboard, or your Bim
  is in no state to walk over. A crewmate out cold gets a **Loot** row
  under them as well; a dead one, or a station's person down in the
  corridor, has only that — see [Combat mode](#combat-mode-and-the-inventory).

Either way your Bim walks to the patient (a tile off, or nowhere if it is
dressing itself) and spends **ten minutes** with its hands on the part;
*Dressing a wound* is on its agenda, and the bandage comes off the count
when the ten minutes are up — provided the patient is still alive and
within reach, since a patient that walked off is ten minutes lost. A
bandage on a part with nothing open is refused outright as a waste. The
bandages are the hold's: the playtest ship carries five, an orbital or a
refinery sells them, and the **drug lab** makes one out of two fibre. The
test room starts with three of its own so `bims room` can try it. The crew
dress each other **on their own** as well — the **Medical** row on the Work tab
is that, and at priority 1 it interrupts whatever they are doing; see [Work
priorities](#work-priorities).

A **medkit** is the other thing, and the only way out of a dying state.
It is ordered the same two ways — a **Treat** button under the part's
Bandage one on the inventory, *Treat the head · skull fracture* on the
menu on a body — and the hands are your Bim's for a crewmate; for your
own, since **nobody treats their own**, the nearest crewmate that is free
walks over instead, and the row says who. Twenty minutes with hands on
the part, *Treating a trauma* on the agenda, the medkit off the hold when
they come off, and the part starts again from half. The patient holds
still while the helper walks over. The medkits are the hold's: the
playtest ship carries two, every lived-in station sells them, and the
**armoury** makes one out of vegetables and a component; the test room
starts with two. The crew treat each other on their own too — a dying
crewmate comes before any wound on the Medical row.

## Needs, and the day they make

Four levels run a Bim's day, shown down the right-hand side of the deck for
whichever of the crew is selected: **Rest**, **Food**, **Restroom** and
**Surroundings**. Kate's are worth going and looking at precisely because you
cannot order her about — her bars are the only warning you get. Each sits
at 1 when the Bim is comfortable and falls as the day goes on. One dropping past
its **threshold** — a tenth, until you move it — is what sends the Bim to bed,
to the fridge or to the heads; doing the thing fills it back up, and a meal fills
hunger completely however empty it was. Rest and Food have theirs on the page,
under the timetable; see [Action thresholds](#action-thresholds).

The restroom need covers both ends of the business deliberately, rather than
being two numbers. They come up together, they are dealt with in one trip, and
splitting them would only mean two bars that always move as one.

Surroundings is the odd one out and is described under [Mess](#mess): it has no
clock of its own and no errand behind it, and follows the state of the deck the
Bim is standing on.

### Where the rates come from

Nothing here is eyeballed. The day is 1440 game minutes and a night is 360, so
the Bim is awake for 1080. Each need should come round a set number of times in
that, which fixes everything else:

```
slot  = waking minutes / times per day − what the errand itself costs
drain = (1.00 − 0.10) / slot        full, down to the trigger
```

The subtraction is the part that is easy to miss. A cycle is the draining *plus*
the doing, so if the whole slot goes on draining there is no room left for the
errand and the day comes up short — two meals an hour apart in the making are
two hours not spent getting hungry. The costs are measured off the chains rather
than guessed, and measured to the moment the need is *full* rather than to the
end of the chain: a meal carries on for another ten minutes stacking the
dishwasher, and the Bim is getting hungry again through all of it.

| need | per day | slot | errand costs | drain per minute |
| --- | --- | --- | --- | --- |
| Rest | 1 | 1140 | 6 | 0.9 / 1134 |
| Food | 2 | 540 | 48 | 0.9 / 492 |
| Restroom | 3 | 480 | 14 | 0.9 / 466 |

The first two drain per *waking* minute and the last one per minute of the day,
because the restroom need is the one that keeps going while the Bim sleeps —
hence a slot of 1440/3 rather than 1080/3. See below.

Recovery is spread across the act itself, so a bar fills while the Bim is doing
the thing rather than jumping at the end: six hours in bed carries Rest from the
trigger back to full exactly, and a meal or a visit covers the whole range.

### The body clock runs slow

Rest is the one need whose slot is not the ship's waking day. It uses 1140
minutes rather than 1080, which makes the whole sleep cycle **25 hours** — six
hours in bed and nineteen out of it — against a 24-hour day.

That one-hour difference is the point. Drained at exactly the rate that empties
it in a day, bedtime lands on the same hour for ever: the Bim has no rhythm of
its own, only the clock's. An hour slow and its body clock free-runs, so bedtime
walks about an hour later every day and the Bim keeps resettling — roughly what
an animal left in the dark does. It is also what makes the timetable worth
having, since a schedule is now something to pull a drifting Bim back onto
rather than a restatement of what it was going to do anyway.

Only rest drifts. Food and the restroom need still drain per waking minute, so
the Bim still eats twice and visits three times in a waking day whatever hour it
starts.
The one figure that moves is how much of a *calendar* day is spent asleep: 360
minutes in every 1502 is 5.75 hours a day rather than a flat six.

Hunger and rest stop while the Bim is asleep — six hours in bed cost it nothing
to put right afterwards. **The restroom need is the exception: it runs all
night.** A body does not stop making water because its owner is unconscious.

Which is why its slot is the whole 1440 rather than the waking 1080. Derive it
off the waking day and the night quietly adds a fourth visit; derive it off the
calendar day and three a day stays three, with one of the three falling not long
after the Bim gets up. The other half of keeping that honest is the rule that
sends the Bim to the heads before bed, so the night starts from full rather than
from wherever the evening left it.

What *is* still frozen is the accidents that hang off the need — the one-in-ten
an hour would otherwise roll six times a night and wet the bed about half the
time. The level falls; nothing happens because of it until the Bim is up.

Run it and the Bim eats twice, visits the heads three times, and — because the
default night is painted in — sleeps once, at ten.

### What it does not do

A need arising while the Bim is mid-errand waits for that errand to finish
rather than interrupting it, and a walk you ordered counts as having something
on. So a bar can sit at zero for a while — cross the threshold as a meal starts
and the Bim finishes its dinner before going to the heads.

## Mess

A bar at zero is not the end of anything, it is the start. Two of them have
something waiting past the bottom, and both are reached by a clock rather than
by the level: the level running out starts the clock, and each stage is an hour
further into it. The needs panel shows the bars; the lines under the health bar
name the stage.

### Needing the heads

The restroom need has three urges under it, and they only ever come up when the
Bim cannot go — shut in, under orders, or with autonomy switched off. Left to
itself it sets off at 10% and none of this happens at all.

| below | urge | what it does |
| --- | --- | --- |
| 25% | mild | hops about on the spot now and then |
| 10% | medium | hops about more, and a **one in ten chance an hour** of wetting itself |
| 0% | extreme | holds on for **one hour**, and then does not |

Wetting itself takes the edge off — the need goes back to 45% — and leaves the
Bim and the tile it is standing on in a state. The hour at the extreme urge ends
in the worst of it: the need goes back to *full*, because relief is relief
however it comes, the Bim is covered, and the tile under it goes straight to the
bottom of the scale. The hopping is the only warning the player gets, which is
why it is there.

### The deck, tile by tile

The deck is scored tile by tile on the same 52-pixel grid it is drawn on. A tile
starts at **10** and goes down as things happen on it: an accident takes one
straight to **−100**, and so does being sick on one; wetting one costs 35.
**A broom puts it back** — see [Sweeping up](#sweeping-up) — and it is the only
thing that does. Being sick is the one a Bim does over and over, so a Bim at
the worst stage leaves a trail of ruined tiles behind it as it moves away from
each in turn, and somebody has to go round after it. The Bim carries its own
share around separately, 0 to 1, because a Bim that soils itself takes the mess
with it when it walks away; no broom reaches that. A wash at the basin gets half
of it off, and nothing else does anything.

The score is the whole of what the simulation reads — the average around the
Bim, how fast that grinds it down, which way it walks to get clear. Alongside it
each tile also remembers **what** was spilt on it, which nothing in the
simulation looks at: it is there so that pointing at a tile gets an answer a
player can use. "−65" is no answer to *what is that*.

### Dirty work, and dirt that walks

Not every mess is an accident. **The dirty jobs mark the deck around them**: a
knife going through a vegetable, a pot tipped and served, a pair of hands in a
hydroponic tray, a crop going into the cold store. Each of those steps has
about a **one in three** chance of flicking something onto one of the nine
tiles around the Bim, worth 14 off that tile — a stain, not a ruined tile, but
well past the threshold where the broom is worth getting out. So the galley and
the bay go grubby on their own, and the crew have standing work even on a week
where nobody has a bad day.

It goes on a tile *near* the Bim rather than always underfoot, so a week of
cooking spreads a patch across the galley instead of wearing one hole in the
deck in front of the stove. Nothing is ever flicked somewhere a body cannot
walk to, because that is a stain the broom would never reach and the deck would
keep for good — the same question `worst_tile` asks, asked here too.

And **dirt travels on boots**. Each time a Bim steps from one tile to the next,
there is a **25% chance** it carries **25% of what was on the tile behind** onto
the tile ahead. Both numbers are about the tile it is leaving, not a fixed
amount: a boot out of a ruined tile leaves a real smear, a boot out of a faint
one leaves almost nothing. The mess it left behind is now a source in its own
turn, so a Bim pacing the same route lays a trail that thins as it lengthens — a
quarter, then a sixteenth — and falls under the sweeping threshold after three
or four steps. That is what stops one accident eventually reaching every tile
aboard.

The dirt **moves rather than multiplies**: the tile behind loses exactly what
the tile ahead gains. Copying it would let a Bim walking back and forth across
the galley make filth out of nothing, and the deck would lose to a pair of feet.
Nothing is drawn from the dice at all unless the step crossed a tile boundary
*and* the tile behind had something on it — a Bim crossing clean deck costs
nothing, which matters because every roll aboard comes off one stream.

The smear carries the *name* of what it came off, too: a boot out of a tile
somebody was sick on leaves a fainter patch of the same thing, not a new kind of
mess. Only what a dirty job leaves has its own name — **grime** — and it is the
least bad of them, so grime tracked across a ruined tile never talks the readout
back down.

**Blood is the fifth kind**, and the one that is not an accident or a job: a
Bim with a wound open drips it where it stands, sixty off the tile a drop —
between a wetting and a ruined tile, so a Bim standing still bleeding fouls the
tile under it in a few drops and leaves a trail as it walks. It is drawn in its
own dark red where the rest are brown, and it stays blood wherever a boot
carries it, whatever else is on the tile it lands on: a pool of it is what you
are looking for after a fight. Otherwise it is filth like the rest — the broom
takes it up, the cleaning row comes round for it, and the crew's surroundings
follows it.

### Surroundings

The fourth bar has no clock of its own. It follows the average of the tiles
within three of where the Bim is standing — a seven by seven block — lifted
by whatever **comforts** are in reach, and how filthy the Bim itself is:

```
average of 49 tiles + lift ≤ 0   →  falls, a full bar in an hour at exactly 0
                                    and 11× that on a fouled tile (1 + |average|/10)
the Bim itself filthy            →  falls, up to 5× the base rate at fully covered
both clean                       →  fills, four hours to full
```

The **comforts** are the three parts that do nothing but make a deck nicer:
a **small plant** (€150) lifts every tile within two of it by 2, a
**picture** on a wall (€250) every tile within three by 3, and a **big
plant** (€450) every tile within three by 5. The default ship comes with a
picture beside the bridge door and a small plant by the bunk. The lifts add up where they
overlap and stop at 10 — a clean tile's worth, so a corner full of plants
is at best twice as good as a clean deck. The lift sits *on top of* the
mess rather than in its place: a plant beside a spill makes the spill
bearable, and a fouled tile is a fouled tile whatever stands next to it.
The picture hangs from a wall exactly as a wall light does — turned to
the wall beside it, refused with none, and warned about if the wall comes
down later. Every station lays a few of its own: a big plant in the middle
of the hub, a small one in the mess and the rec room, a picture in the
quarters and the rec room.

The radius cuts both ways, and it is worth being clear about which. A wider
block reaches further — a mess three tiles off now counts for something — but it
also *dilutes*, because what the bar follows is the average: one ruined tile
among forty-nine drags it less than one among twenty-five, and it takes five
of them nearby before the room alone pulls the average under zero. The Bim's
own filth is the other half of the sum, and that is what makes a single
accident bite immediately whatever the radius is.

So one accident is worth minutes rather than hours: a Bim standing in it is at
nothing within a quarter of an hour, and walking away does not clear it while it
is still covered. What that leads to, an hour a stage:

| stage | what it does |
| --- | --- |
| mildly uncomfortable | walks 10% slower, picking its way around it |
| uncomfortable | moves away from the mess whenever it is otherwise idle |
| extremely uncomfortable | **is sick every half hour**, which takes **50% off the food bar** — 92% becomes 42%, and at or below half full it goes to nothing — and puts more on the deck |

None of it interrupts a chain: these are things that happen *to* the Bim, so one
that wets itself on the way to the galley carries on to the galley. Moving away
from the mess is a walk rather than an errand, and anything the Bim is actually
doing — or has waiting on the queue — outranks it.

**This is a spiral, by design.** A Bim at the worst stage brings up half a meal
every half hour, which is faster than it can cook, so with nothing aboard able
to clean the deck the only way out is the player moving it somewhere clean
before it gets there. That is what the cleaning job this is built for will be
for.

Everything it starts for itself is an ordinary chain, so it can be interrupted
and queued like any other. Uncheck **Let the Bim decide** in the header to drive
it entirely by hand; the levels carry on moving, they just stop giving orders.

## Socializing

The fifth bar, and the only one that wants **another Bim** rather than a
fixture. Past its trigger the Bim goes and finds the other one — which happens
**about four times in a waking day**, roughly thirty conversations a week.

Two numbers make it that talkative, and both are unlike every other need here:

- **The trigger is half a bar**, not the tenth everything else uses. Company is
  filled by the *other* Bim being free at the same moment, so waiting until the
  bar is nearly empty would mean waiting until the one thing that fixes it is
  least likely to be available. Asking early is how two people sharing one
  compartment actually behave.
- **A conversation only puts three tenths of the bar back.** It is a word on
  the deck, not a meal. So the next one is never far off, and the bar hovers
  around its trigger instead of swinging the whole way down and back — a Bim
  aboard a working ship should never see this one anywhere near empty.

The drain is written against those two rather than against `URGENT`, which is
the one place in `needs.rs` that departs from the house rule: the span it
travels between one chat and the next is what a chat restores, over the time
that is meant to take.

### Talking

Both crew are handed the errand in the same frame, each walking to **its own
spot** either side of a meeting point worked out from where the two of them are
standing. That is not a flourish. A route here is planned once and never
replanned, so a Bim sent to *where the other one is* would be walking at a
target that is itself walking: it would converge on empty deck and stand there
for ever with `arrived()` never coming true. Two fixed points is the only shape
of this that terminates.

They stand **side by side** rather than either side of the line they happened to
approach along. A pair who met walking north–south would end up one above the
other, and the host paints each name a body's height above its head — so the
lower one's label lands squarely on the upper one. Left and right, and nothing
is written over anything.

A **speech bubble** goes up over whichever of them has the floor. They take
turns, and whose turn it is comes off the ship's clock rather than off either
one's own errand: they arrive a moment apart, and two bubbles over two Bims
standing together would sit on top of each other. What they talk about is
picked out of the speaker's own recent diary — a harvest, a bad night, being
sick on the deck — so a conversation is about the week they have actually had.
The code crosses the boundary; the words are entirely the host's, in
`CHAT_TOPICS`.

A conversation is the one errand that is **dropped rather than put down** when
something interrupts it. Half of one is worth nothing and the other half will
have walked off by the time it is picked up again.

The other Bim gets interrupted to have it, if it is merely working — sweeping,
or at the bay. Not if it is asleep, in the heads, or sitting on the deck. The
alternative is two Bims who are never both free at the same moment and so never
speak, and with the deck always finding something to be swept that is not a
hypothetical.

### Going without

The bar is the comfortable end of it. What matters is a second clock, in days,
that **starts once the bar is empty** and that only a conversation resets —
the same shape as malnutrition and as standing in the mess, where reaching
nothing is the start of it and not the end. The clock stands while there is
anything on the bar, and the first stage is six days of nothing, so a crew
parted for a few days — one outside in a suit, one left on a station — does
not come back brooding.

| bar empty for | stage | what it does |
| --- | --- | --- |
| 6 days | **desocialized** | writes low entries in its diary every four hours, and everything it does takes **a tenth longer** |
| 8 days | **badly desocialized** | that, and **sits down on the deck for ten minutes**, roughly every five hours, wherever it happens to be |
| 10 days | **isolated** | that, and **hurts itself every four hours** — ten points of health a time |
| 13 days | — | **3% an hour of giving up altogether**, and three points more for every further day |

The stages do not replace each other: an isolated Bim is still brooding and
still sitting down, because each one is the one before it and worse.

The self-harm rate is set against the mending, not picked by eye. A well-fed Bim
recovers half its health a day, so six bouts of ten is ten points a day of *net*
damage — the three days between the isolated stage and the despair that follows
it leave it worn down and alive, which is the shape the escalation wants. Make
it much faster and nothing ever reaches the thirteenth day to give up on it.

Two things that are easy to get wrong here and were:

- **Health mends every frame, so a bar taken to nothing by anything sudden is
  back above nothing before the game has looked at it.** Hunger works on health
  over hours and reaching zero from it is checked in the same frame it happens;
  a Bim hurting itself, or giving up, sets the bar to zero in a *later* part of
  the frame and the recovery on the next one undid it. The result was a Bim
  whose health read nought and who was still walking about. `Health::update`
  now returns at once from zero: nothing comes back from nothing.
- **The clock belongs to a body.** It is per Bim and never on anything shared —
  the same lesson `filth::Ordeal` is there to record. One Bim's solitude must
  never be reachable from the other's frame.

None of this can be switched off in the work list, and there is deliberately no
row for it: the list is for jobs, and a player who could put *company* at the
bottom could set a Bim to die of loneliness without meaning to.

## The heads

Called the bathroom, the toilet and the basin everywhere the player can see —
the agenda says *Using the toilet*. "The heads" is the code's own word for the
compartment and stays in the source and in this file; nothing on screen uses it.

The compartment in the bottom-right corner is a room in its own right, walled
off with its own bulkheads and shut by a sliding door. The toilet's menu has one
item on it, and that one item is a whole errand: the Bim walks to the door,
opens it, steps through, shuts it behind itself **and locks it**, crosses to the
pan, sits, waits, gets up, flushes, moves to the basin, washes its hands under a
running tap, comes back to the door, unlocks and opens it, steps out onto the
deck and shuts it again. About half a minute, all of it animated.

The door is the interesting part, because it is the one thing aboard that
changes the shape of the room. Locking it shuts it as well — a locked door
standing open is not locked — and while it is locked the toilet's **Use** item is
greyed out with the reason, rather than the Bim walking into a sealed door. The
Bim locking up behind itself is why the lock is worth having: the state the
player left it in is restored on the way out.

The door has a panel on each side of the bulkhead, and asking for it sends the
Bim to whichever one it is nearest — including from the inside, so a Bim that
has shut itself in can always let itself out again.

These two panels are the only switches aboard that carry what was asked for
rather than reading the state they find. Every other switch is a toggle worked
at the moment the Bim's hand arrives, which is the honest thing for a hob or a
fridge. The door is different because *asking* changes it: the ask drops
whatever the Bim was doing, and dropping a trip to the heads takes off the lock
the Bim set behind itself. A toggle then found the door already unlocked and
locked it straight back — press **Unlock** on a Bim sitting on the pan and it
got up, walked to the panel, and locked itself back in.

It also shuts itself. Five seconds after the Bim has walked through, the panels
slide to — the chains that shut the door by hand simply get there first — so the
door is never left standing open by an errand that had no reason to close it.
The count waits while the Bim is still in the opening, and waits again while the
Bim is walking a route that goes through the doorway: a route is planned once
and never replanned, so a door that shut across one would leave the Bim walking
into the panels for good.

### Starting from inside

The chain is written for a Bim out on the deck, and a Bim already in there would
be sent round to a handle on the wrong side of the bulkhead. So from inside it
starts at the pan instead, and a locked door is no obstacle: a lock only stops a
Bim on the wrong side of one.

Skipping the four steps that let it in also skips the four that let it out —
they exist to undo each other, and a Bim that never opened the door has no
business unlocking it on the way past. So a Bim shut in stays shut in, finishes
at the basin, and the lock it never set stays set.

## Bed time

There are two bunks, one per crew member: one against the left wall, one in the
top-right corner. A Bim only ever goes to its own. Either bunk's menu offers a
nap of thirty minutes or a sleep of six hours; both run the same chain — walk
over, in, under the covers, out cold, a stretch, and back out — and
differ only in how long the Bim stays put. The menu says what time it will be
up, and the status line counts the rest down while James sleeps.

The second bed is the first one mirrored. A `Berth` carries which side of it
the deck is on, and the spot the Bim stands on to get in and the way it turns
to do so are read off that rather than written into the drawing twice.
Everything else — pillow at the top of the room, head towards it — is the same
in both, so a sleeper lies the same way up whichever bed it is in.

It is a single bed, seen from above: a frame with a headboard standing proud
at the head end and a footboard at the other, a mattress, a pillow, and a
duvet with a turned-down cuff. The duvet and the shape of whoever is breathing
under it are drawn *after* the character, which is what actually puts a Bim
under the covers rather than on top of them. It used to be a bunk bed — an
upper deck with the lower one showing along two sides — and the footprint is
still the bunk's, a little larger than the bed drawn in it: the pathfinder,
and so every route and every seed the probes pin, is built on that footprint,
and the bed's station and lying position are the numbers the bunk had.

## Time

`crates/game/src/clock.rs` keeps one clock for the whole game: minutes since midnight, and
which day it is. One real second is one game minute at 1x, so a day takes
twenty-four minutes of real time and a six-hour sleep takes six — and because
the clock runs off the same `dt` as everything else, the speed slider carries it
along. At 24x a whole day goes by in one minute.

Nothing about time comes out of the room as a string: the app reads the
minute count and formats it. The light level comes from the same clock, eased in
at dawn and out at dusk, and is laid over the finished frame as one tinted
rectangle — so nothing in `room.rs` has to know what time it is.

## The look of it

Everything is drawn from the two primitives in `crates/game/src/draw.rs`, so "futuristic"
here is a matter of palette and of what gets a light on it rather than of any
new drawing machinery. The deck is dark blue-grey, the fittings are composite
panel in three shades, and one cyan running light is picked up by every powered
surface: the counter fascia, the induction coils etched into the hob, the rim of
the table, the underside of the top bunk, the jambs of the bathroom door. Warm
colours are held back for heat and for trouble, which is why a live hob and a
locked door are the only two warm things in the room and both read instantly.

The two crew are told apart by their hair and the yoke on their coverall, not
by where they happen to be standing: James has cropped hair and a pale yoke,
Kate a mauve one with hers worn long, which from directly above is a second
ellipse behind the head. The coverall itself is the ship's blue on both —
aboard the ship game the station's people wear the station's orange, so a
joined deck says at a glance who is going ashore when it casts off. Their names are written over them — James's in the
green that everything steerable uses, Kate's in plain ink, so which one takes
orders reads without being explained.

The page around it is arranged the same way: nothing is framed. The deck runs to
the edge of the window, the canvas carries no border of its own, and every panel
sits flush in a corner of it — the readout and the agendas top left, the
selected Bim's bars and crew sheet top right, the tray in the bottom-left
corner. A rounded box drawn
round the whole interface reads as a window frame inside a window, which is one
frame too many; only the header, with the name and the clock, sits above the
deck rather than on it.

The tray is the exception to everything being small. It is set a size larger
than the rest — wider, bigger type, taller hour cells — because the timetable
and the action thresholds under it are where the day is actually decided, and they are
read and edited rather than glanced at.

## How it fits together

The simulation is entirely Rust, and so is the window. Each frame the app
steps whichever screen is up and asks it for a flat buffer of shapes; the
buffer is tessellated into one mesh (`crates/app/src/shapes.rs`) and drawn by
egui with the panels round it, and the words — names over heads, labels on
the map, the readouts — go on after. The rules crates hand over numbers and
shapes and nothing else: no string leaves them, and no sound either — the
room says a door slid or a shot was fired as a *cue*, and the app owns the
recording — so the native server that will one day run the same crates has
nothing to say and nothing to disagree about. `cargo build` is the whole
pipeline.

### Eleven crates

The repository is a cargo workspace and everything is under `crates/`. The
split is not tidiness — each line of it is something that has to give the same
answer in two places at once:

| Crate | What it is | Who else needs it |
| --- | --- | --- |
| `app` | The window: Bevy and egui, the four screens, the pointer, the words. The one binary | — |
| `game` | The room: the simulation, and the shapes it draws itself as | `world`, which runs it aboard the ship; `app`, on its own as the test room |
| `ship` | The design phase and the game it starts: `Session`, the editor, the two cameras, the painters | `app` |
| `lobby` | The World tab's galaxy: a camera over it, a pick, and the system diagram | `app` |
| `world` | One star system, the ship in it, and the one clock they both run on | `ship`; the native server, which has to run the identical loop |
| `flight` | What a design does when you push it, and the closed-form plan that flies a trip | `world`; the native server, which has to put the ship in the same place |
| `shipdesign` | What a ship is made of and the rules for putting one together | `ship`, `flight` and `world`; the native server, which has to admit the same ships |
| `worldgen` | The galaxy, what is in each system, and station blueprints | The native server that will one day be authoritative, which has to generate the identical world from the same seed |
| `physics` | Ship mass, engine thrust, travel time. Pure arithmetic | `worldgen`, to check its layouts; `shipdesign` and `flight`, to weigh and fly a real ship |
| `economy` | Money: whole euros, the shared pool, what a station charges and which hold it goes in | `shipdesign`, `world` and `ship`; anything that ever charges for anything later |
| `health` | One body's health: conditions with stages, mending, death. Radiation dose, sickness and cancer are the first two | The crew step, which holds one per Bim; the native server, which has to agree about who survived |
| `time` | How long a minute, an hour and a day are | All of them — a day that is two lengths is two games |

`cargo test --workspace` runs every crate's tests, and `nix flake check`
builds the app and runs them. `game` carries no unit tests of its own and is
checked by the native probes in `scratchpad/` instead; `./check smoke` opens
each of the four windows for sixty frames and keeps a picture of two of them.

### Inside the room

Relative to `crates/game/src/`:

| File | What lives there |
| --- | --- |
| `lib.rs` | The module list, and why `time` is reached as `crate::time` |
| `game.rs` | Ties the room, the Bim and the running task together; input |
| `room.rs` | The room: layout, fixture state, and how it is drawn |
| `bath.rs` | The heads: its bulkheads, its door, and its fittings |
| `dish.rs` | The dishwasher: what is in it, and the cycle it runs |
| `needs.rs` | What the Bim wants, and the rates that shape its day |
| `health.rs` | Going hungry and going without sleep, the three stages of each; the body's three parts, the blood, wounds and bandages |
| `filth.rs` | The state of the deck, and what a mess does to the Bim |
| `hydro.rs` | The hydroponic bay: six trays, and the three crops that go in them |
| `sight.rs` | What each Bim can see, traced on the tile grid, and the peek round a wall |
| `combat.rs` | Weapons, bolts, hits and shots; the enemy's choice of where to stand |
| `manager.rs` | What the place is told to keep in stock |
| `schedule.rs` | The day's timetable, and when it is worth obeying |
| `task.rs` | The scripted chains — a meal, a sleep, a trip to the heads |
| `clock.rs` | The time of day, and how much light there is — restated from the `time` crate as `f32`, not defined again |
| `character.rs` | The Bim — how it decides where to go, and how it is drawn |
| `draw.rs` | The shape buffer and the local frame used for sprites |
| `math.rs`, `rng.rs` | Vectors, rectangles, angles, and a PCG32 generator |

And in `crates/app/src/`:

| File | What lives there |
| --- | --- |
| `main.rs` | The four things to run, and the screen state machine |
| `screens/room.rs` | The behaviour test room: the deck, the pointer, the speed, the status line |
| `screens/builder.rs` | The menu, the setup screen and the lobby, with the World tab |
| `screens/designer.rs` | The design phase, and the `Net` seam every edit goes through |
| `screens/game.rs` | The game: the helm, the map, the station, the ship's facts |
| `crew.rs` | The crew's panels, shared by the room and the game: needs, the sheet, the agendas, the tray, the fixture menus |
| `names.rs` | Every word on the screen, indexed by the codes the crates hand over |
| `shapes.rs` | The shape buffer, turned into a mesh |
| `sound.rs` | Every sound, the way `names.rs` is every word: the room's cues and the world's events played as clips cut from `Sounds/` |
| `settings.rs` | The Esc sheet: the UI scale, the audio volumes, and the keys |
| `canvas.rs`, `theme.rs`, `format.rs`, `dev.rs` | The pointer, the palette and widgets, numbers as words, and the smoke run |

### Getting about

Every walk — a right-click order, and every leg of a scripted job — is planned
on a grid in `crates/game/src/nav.rs`. Obstacles are inflated by the body radius before the
search, so a route that exists on the grid is one the Bim can physically walk
without clipping a corner. A* returns a staircase of cells, which is then pulled
straight by dropping every waypoint that can be skipped with a clear line of
sight; what is left is the two or three straight legs a person would actually
walk. A destination that is not standable — inside the table, behind the counter
— snaps to the nearest spot that is, so an order there means "the floor beside
it" rather than nothing at all.

The steering and the collision push-out are still there, but only for the
wander and as a backstop. While following a path the Bim trusts the plan.

### Ordering the Bim through a door

A shut door is a wall to the pathfinder, so a route is worked out twice: once
with the door as it stands, and — only if that comes back with nothing — once
with it open. A destination that only the second finds is one the Bim can reach
by letting itself through, so it goes and works the door panel and then carries
straight on. The order waits on that errand rather than joining the queue
behind it: the player asked for it now, not after whatever the door displaced.

There is only ever one of those errands. Clicking again while the Bim is on its
way to the panel replaces the destination waiting on it and nothing else —
before, each click put the door errand on the queue and started another exactly
like it, and a few impatient clicks left the agenda with ten *Bathroom door*
jobs all opening the same door. An order the Bim can reach without the door
cancels the waiting one outright, and the errand that was opening the door goes
with it: the two exist only to serve each other.

Three things can come back instead:

- **Blocked by a locked door.** The second route exists but the door is locked,
  so nothing is attempted at all — the Bim does not walk over and fail, and it
  does not drop what it was doing either. The one exception is a Bim part-way
  through its own trip to the heads, which locked that door behind itself:
  letting go of that errand unlocks it, so calling the Bim out works.
- **Nowhere to go.** No route with every door in the place wide open. There is
  nowhere in the current layout that this can happen, but the case is handled
  rather than assumed away.
- **Ignored**, if nothing is selected or the Bim is dead.

Both refusals are said out loud, because an order that quietly does nothing
reads as a broken click: the status line says which it was, in the warm colour
the room keeps for things worth noticing, and a cross is drawn where the order
landed.

### A chain never starts somewhere it cannot walk to

An empty route reports itself *arrived* the instant it is handed over — the
alternative, waiting on a walk that will never finish, hangs the chain for good.
That is fine for one step and quietly disastrous for a whole chain: every
remaining step finds itself already arrived and runs in the same frame, until
one of them sets a position outright and flings the Bim across the deck. Shut a
Bim in the heads, start it on a meal, and it appeared at the dining table
without having walked a step of the way.

Two things stop it now. A walk with nowhere to go gives the chain up rather than
taking it for an arrival, putting the world back in a state the Bim can be left
in. And no chain is begun at all unless the Bim can reach the place it starts
at, so a Bim behind a shut door does not set off for the galley: the need stays
unmet until the way opens. Queued chains are held back the same way rather than
dropped — they wait on the queue for the door.

Waiting is only right when the Bim can do nothing about it. A shut door is not
that: the panel is on the inside too, so a Bim that finds the galley out of
reach behind one goes and opens it, and the need comes round again with the way
clear. Only a *locked* door is a real wait. Without that, unlocking the door on
a Bim shut in the heads left it exactly where it was — unlocking leaves a door
shut — and it stood there at nothing per cent food until it starved.

Reachability is worked out afresh every frame rather than cached, so opening or
unlocking a door needs no nudge: the moment the way is clear the Bim takes up
whatever it could not reach a moment ago, queued work included. Queued work it
cannot reach does not count as having something on, either, or it would hold up
every need there is while it waited — and a Bim standing over an unreachable
meal would starve beside a queue it was never going to get to.

### Two grids, because of one door

A grid built once is only right while the room never changes shape, and the
bathroom door changes it. Rebuilding the grid when the door moves does not work:
a task opens the door and asks for a route through it *in the same step*, so any
rebuild in the frame loop is a frame late and the route comes back empty — the
Bim would stand at a door it had just opened and give up.

So `nav.rs` builds two grids at startup, one with the door in the way and one
without, and the choice between them is made at the moment a path is asked for.
That also gives the closed door its proper behaviour for free: order the Bim
through one and the route is genuinely empty, so it stays where it is instead of
walking through it. Shut the door on a Bim that is inside and it really is shut
in until someone opens it.

Physics is allowed to be less careful. The push-out that keeps the body out of
the furniture takes the door a frame late, which nobody can see.

### Simulation speed

The speed control multiplies how many fixed 1/60 steps run per frame, never the
size of a step. At 24x a single stretched step would move the Bim four times
its own body in one frame and it would pass straight through the counter;
twenty-four normal steps give exactly the result 1x would, just sooner.
`MAX_STEPS_PER_FRAME` caps the catch-up so a backgrounded tab cannot come back
and lock the page up — it has to stay above the top speed, or the top of the
slider would quietly be slower than it says.

### The wander

A plain per-frame random direction looks like vibration, not walking. Instead
the Bim commits to a leg of a journey at a time: a direction and a speed held
for a second or three, then either a pause or a fresh leg. Course changes are
usually gentle and occasionally a complete change of mind, and the body eases
towards each new heading rather than snapping to it.

Near the walls and the furniture it steers rather than bounces, and the steering
bends the Bim's *intent* as well as its current heading — otherwise it turns
back into the wall the moment it comes clear, and spends its life hugging the
edge. A push-out pass after each move is the backstop that keeps it out of the
table when steering is not enough.

### The room is a fixed size

The room is 860×580 world units whatever the window is; `Game::resize` works
out a scale and offset to centre it in the canvas, and the app applies that
transform when drawing and inverts it for pointer positions. Furniture at
honest proportions is worth more than filling every pixel, and it means the
layout constants in `room.rs` can be trusted.

One consequence worth knowing when editing `room.rs`: the Bim stands
`STAND_OFF` from the counter front, and `STAND_OFF` has to clear `BODY_MARGIN`
or the collision push-out fights the station it is walking to. Everything on
the worktop also has to sit within arm's reach of that line, or the Bim ends up
chopping thin air.

### The draw buffer

Twelve floats per shape: `kind, x, y, w, h, rot, radius, line, r, g, b, a`.
`kind` is 0 for a rectangle and 1 for an ellipse; everything on screen is built
from those two. Rotation is about the shape's own centre, and a non-zero `line`
strokes the outline instead of filling it; 2 is a right-angled triangle, the
bottom-left half of its box, which the diagonal hull plates are. `draw::STRIDE`
is the stride, and `crates/app/src/shapes.rs` asserts its own copy equals it.

If you change the layout, note that the replay in `shapes.rs` assumes the
field *order* in `crates/game/src/draw.rs`.

One trap worth knowing: `f32::clamp` panics when its bounds are crossed. The
room uses the branchless `clamp` in `crates/game/src/math.rs` instead, which
dates from when the panic path cost 19 KB of a wasm binary; it is still the
one to use, because a panic in a step is a stall with no message.
