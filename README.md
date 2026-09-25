# Bims

A roguelike top-down co-op shooter, written in Rust on Bevy: humans
against the machines. It opens as a window on your desk.

Up to four players share one ship — the **default ship**, the same every
run — and each of them steers one **Bim** aboard it. A crisis of machines
is spreading across the galaxy a hyperlane at a time, and the crew go
where they choose: to a station to trade and to hire, to one the machines
hold to take it back, to a town they are coming for to hold it. Between
places is the **world map**, where the crew choose the next one together;
at every place is a **mission**, which is where the shooting is. **Time
and money are the only resources**: a trip costs days, and the machines
spread a hop every five of them; a machine destroyed pays a bounty, and
the bounty buys guns, armour, hands for hire and the dead back. The run
is over when every player's Bim is dead at once.

**You steer your own Bim**, and the crew nobody steers are bots that
follow the players into a fight. [A run](#a-run) is what a run is made of, [the
loop](#the-loop-world-map-travel-missions) is how it goes from one place
to the next, and [the crew](#the-crew) is who is aboard.

The lobby starts [the game](#the-game) straight away — one star system,
the default ship docked at the station you picked, and one clock everything
runs on. The [ship designer](#the-ship-designer) is still there, behind the
`design` command, but a run does not pass through it.

It got here in three steps. Bims was a life sim — Bims on a ship the crew
laid out and flew themselves, cooking, sleeping and keeping the deck clean
on a clock of their own — and the first step (feature 102) switched off
everything of that the shooter does not use, the second (feature 103)
built the loop of travel and missions, and the third (feature 104)
**deleted** what the first had switched off: the needs and the day they
made, the behaviour test room, radiation, every human enemy — the hostile
stations, the raiders, the plunder — and the flown trip, with the helm and
the landing. The git tag `needs-sim-final` is the last commit that has
them.

## A run

What a run is, from the lobby to the end of it.

- **The start.** Start menu, setup or a lobby, the galaxy and a station to
  start at — and then the run: docked at that station on the **default
  ship** (the playtest ship `nix run .#simulation` opens on), with **5 000 a
  player's Bim** in the one pool the crew share and nothing added for going
  alone. The setup tab's money row defaults to it; there is no ship size to
  pick, since there is no ship to lay out. The run opens in its first
  mission, at that station.
- **The crew, and the bots.** Every player steers their own Bim, and
  everybody else aboard is a **bot**: a mercenary hired at a station, or a
  townsperson who joined the crew once their town was held. A bot follows
  the players, takes arms when an enemy is near, dresses a crewmate's
  wounds and answers the two orders every player has — **F** puts an
  attack banner down for it to fight its way to, **T** calls it back to
  the ship. Each player picks a **class** for their own Bim — engineer,
  soldier, medic, tank or commander, or none — and it levels up in the
  fight; see [Classes and levels](#classes-and-levels).
- **Every enemy is a machine.** No station's or town's people are the
  crew's enemies: a place is the crew's home, neutral, or held by the
  machines. The fights are theirs — Husks, Troopers and Wardens, in waves,
  at tier one, two or three, and a Guardian among them at tier three — and `nix run .#droids` is how one is looked
  at on its own; see [The fight](#the-fight).
- **The crisis is there from day nought.** The machines' origin is theirs
  the moment the run opens, eight hyperlane hops or more from the crew, and
  every system due by then with it — its jammer standing and its stations
  held — and it spreads a hop every five days from there. An infested
  system's **jammer** shuts the lanes back towards the origin while it
  stands; a system a hop outside the infection is on the **front**, and a
  friendly town there is **threatened**: set down at one and the machines
  come for it, and hold it to the last wave and it is the crew's for good.
  *The fight* has the whole of it.
- **Money and gear.** A station's desk sells guns and armour at every
  tier, where the place has the trade, and nothing the ship lives on — no
  food, no suits and no medicine, which are everybody's charges anyway —
  and nothing is built onto the ship but a class's sandbags and sentries.
  The money comes from the Republic's bounty on every machine destroyed,
  and it goes on gear, on hands for hire and on buying the dead back.
  Research, the workbench and the drug lab work as they always did.
- **Nobody eats, sleeps or goes to the heads.** The bunks, the galley,
  the heads, the shower, the hydroponic bay and the cold store are
  **furniture**: drawn as they always were, in the way of a walk as they
  always were, and nothing more — no menu opens on one. And a walk outside
  in a suit doses nobody: there is no radiation.
- **A station's people keep a routine.** With nothing to send them
  anywhere, each is dealt a role when their station's room opens — a
  **guard** walks between the airlocks, or a town's gates and its pad; a
  **trader** stands at the trading desk and steps away from it now and
  then; a **worker** goes between two or three of the benches, research
  desks and shelves; a **civilian** strolls between rooms and stops in
  each — and a round worked out from that station's own fixtures, so every
  station and town the generator makes has one. A role the site has not
  got the fixtures for wanders between spots of open deck instead, or
  open ground on a planet. On an alert they fight or take shelter as they
  always did, and go back to their round when it is over. Every choice is
  off the station's seed and the world's step, so two players' clients
  walk the same rounds.

## The loop: world map, travel, missions

A run is a string of **missions**, one at each place the crew go, with the
**world map** between them: world map, travel, mission, back to the ship,
world map, from the first dock to the end of the run. Nothing is flown:
there is no helm to stand at and no trip to sit through.

- **The world map.** Between missions every player sees it: the system
  map of the stations and settlements round the crew, the galaxy chart
  behind *Galaxy view*, and along the top a list of every place a trip
  can go — every station and settlement in this system, and every one in
  a system a hyperlane joins to this one — each with how long the trip
  is, the day the crew would get there, and what they would find there
  on that day: the machines and at what tier, the system's jammer, a town
  the machines are coming for, a place already cleared. `M` shows it
  during a mission too, read-only.
- **Choosing together.** Pick a place — on the list, or click it on the
  system map — and **Propose**. Every player still in the game has to
  **Accept**; proposing counts as your own yes, and another proposal
  clears every yes there was. A player who has left the game is not
  waited for, and a player whose Bim is dead still has a vote.
- **Travel costs days, and only travel moves the clock.** A trip is the
  hyperdrive's twenty-minute charge for a jump, and the flight inside
  the system at the ship's own accelerations — from where the crew are,
  or from where the jump lands them, to the place. One hyperlane hop at
  most, and the machines' jammer still bars a jump inward out of a
  system they hold. The moment the last player accepts, the world clock
  goes on by the whole trip in one go and everything that runs on days
  is read at the new day: a system whose day came on the way is the
  machines' when the crew get there, towns on the new front are
  threatened, and the waves are sized for the day. The crew arrive
  docked, or landed at a settlement, and a mission begins. **The world
  clock stands still during a mission and on the map**: the day on the
  screen only changes when you travel. **Every trip is at least a
  day**, however close its two ends — the map says *the minimum* beside
  one that would have been shorter — and **the place the crew are at is
  never a destination**: to fight a place again the crew go somewhere
  else first, and a day or more has gone by when they come back.
- **The machines grow with time, and with nothing else you do.** A
  wave is two machines, one more for every **player** — never a bot, a
  mercenary or a townsperson who joined — and one more every **three weeks
  of the world clock**, up to sixteen; a place holds two waves, and one
  more every six weeks. What the crew own, what they carry and how far
  they have levelled make no difference to the machines: getting
  stronger makes the fight easier, and keeping money costs nothing.
- **A mission** begins on arrival anywhere, peaceful or not — a visit to
  a trader is a mission without a fight. Everybody's health is made
  whole, every class charge and cooldown is ready, and dead players are
  bought back (below). Everything inside a mission runs on the **mission
  clock**, which starts at nought on arrival: the machines' next wave two
  minutes after the last of one is destroyed, a town's first wave a
  minute after the landing, the class cooldowns.
- **The bounty waits for the place to be cleared.** The Republic pays
  for every machine destroyed — 500, 1 500 or 4 500 by its tier — but
  it is **pending**, "+€ n on clear" along the top, until the place is
  **cleared**: no machine left there and none still to come. Then it is
  paid into the pool, once. Experience is always yours.
- **Back to ship**, at the bottom right. The first press sends every bot
  back to the ship. When every player still on their feet has pressed
  it and is aboard, the ship leaves — asking first if anybody would be
  left outside: every player gets *Leave them behind?*, and it takes
  everybody's yes; one no keeps the ship where it is, the presses
  standing, and *Ask again* asks again. A player who is down, dead or
  gone is not waited for. **Left behind is dead**, and a body down
  outside the ship is not carried aboard by leaving.
- **What leaving does to the place.** A place the crew cleared stays
  cleared. Any other is put back exactly as the mission found it — the
  machines, the dead, the lamps — and its bounty is lost. A town the
  machines were attacking falls to them when the crew leave it before the
  last wave is down, and is an infested place like any other from then
  on.
- **Dying.** A player's Bim that dies is **out**: its gun, armour and pack
  are lost with the body, but its class, level, experience and talents are
  kept. At the start of each mission the pool buys each dead player's Bim
  back for **5 000** if it can, the longest dead first — it wakes aboard
  the ship carrying nothing — and one the pool cannot pay for stays out
  and is tried again at the next mission. A bot that dies — a hired hand,
  a townsperson who joined — is gone for good and costs the pool **5 000**,
  never taking it below nought. **The run is over when every player's Bim
  is dead at once**, whoever is waiting to be bought back: a screen says
  so, with the day, and its *Start again* puts the run back to where it
  opened.

## Running it

```sh
nix run .
```

That builds the game and opens it. There are eighteen things to run, and each is
a name rather than a flag — `cargo run -- list` (or `bims list`) prints them
all with a line each, and is the build's own answer rather than this table's:

| command | `cargo run` | opens |
| --- | --- | --- |
| `nix run .` or `nix run .#game` | `cargo run -- game` | the whole game, in order: the start menu, setup or a lobby, the world and a station to start at, then [the run](#a-run) — docked where you said on the default ship, 5 000 a Bim in the pool |
| `nix run .#simulation` | `cargo run -- simulation` | straight into the world on a prebuilt playtest ship, docked at a station |
| `nix run .#design` | `cargo run -- design` | straight into the ship design, the playtest ship given, docked where the simulation docks |
| `nix run .#test` | `cargo run -- test` | the simulation somewhere else each time: docked at a random station somebody lives on, in a random galaxy, with a mercenary for hire at the dock |
| `nix run .#test_planet` | `cargo run -- test_planet` | `test` set down on a planet: the same random galaxy, landed at the settlement of a planet whose people are friendly |
| `nix run .#droids` | `cargo run -- droids` | **the fight**: the combat ship — sixteen crew, a gun in every hand, four of them hired field medics — docked at the arena, which the **machines** hold: a wave of Husks, Troopers and Wardens stands about it. They wear nothing, carry nothing and leave nothing to loot; a Husk snaps at arm's length, a Trooper walks into the open with a gun for a forearm, and a Warden's lance **strips the armour off** whatever it hits rather than wounding the body under it. Clear a wave and the next lands at the far airlock a minute later. Every enemy is a machine (see [a run](#a-run)), so the `combat` command that turned the arena's people against the crew — which would be this exactly — is gone |
| `nix run .#combat_droids_engineer` … `#combat_droids_commander` | `cargo run -- combat_droids_medic` | that **same fight with a class in hand**: the crew member you steer starts as an engineer, a soldier, a medic, a tank or a commander — one command a class, the ship, the arena and the wave `droids`' own, so two of these runs differ by the class and nothing else. It starts at the **tenth level** with every one of the class's seven talents still to choose, so the tray opens on the **Skills** tab with seven points to spend; `BIMS_LEVEL=3` opens it at that level instead, and `BIMS_CLASS` still overrides the command |
| `nix run .#tier2_test` | `cargo run -- tier2_test` | `droids` with everybody's kit at **tier two**: every crew member's gun at it and a full set of armour at it on, and the machines at tier two — nothing at tier one on either side |
| `nix run .#tier3_test` | `cargo run -- tier3_test` | the same at **tier three** |
| `nix run .#droids_planet` | `cargo run -- droids_planet` | the same on a planet: a town held by the machines, the ship set down at its pad, and their lander coming down on the plain beyond a gate |
| `nix run .#crisis` | `cargo run -- crisis` | the simulation **a day before the crisis first spreads**: a random galaxy and a random dock as `test` deals them, and the crisis's origin forced two hyperlane hops from the crew's own star — theirs from day nought, as in every run — with the clock wound to the eve of the day the stars next to it turn. Open the galaxy chart: the origin is red, and since only travel moves the clock, the next ring is red once the crew have travelled a day — the world map says which places the machines will hold on the day you would arrive — and the crew's own system follows five days later; the day any star is due is written under its name when it is picked. `BIMS_CRISIS_DAY=n` moves the day the origin turns, and the rest with it |
| `nix run .#jammer` | `cargo run -- jammer` | the crew **inside an infested system**, two hyperlane hops from where the machines began: every station of it in their hands, a wave aboard the one the ship is tied to, and the system's **jammer** standing — so the chart's route inward is barred in red, a jump that way is refused, and the machines come at tier three because of how near the origin they are. `BIMS_DROID_TIER=1` brings them at tier one instead |
| `nix run .#defense` | `cargo run -- defense` | **a town worth defending**: the ship set down at a friendly settlement with the machines one hyperlane hop away, so the town is next. A minute after the landing a wave sets down outside a gate and walks in; the town's guard and whatever mercenaries live there take arms, everybody else goes indoors, and the red line along the top counts the wave the way it counts a held station's. Hold the last wave and the town is yours to keep. `BIMS_DEFENSE_DELAY=n` is the wait before the first wave and `BIMS_DROID_WAVES=1` a fight short enough to finish |
| `nix run .#guardian` | `cargo run -- guardian` | **the Guardian**: the fight at tier three with every wave one Guardian and two Troopers — the largest machine, a walker behind a shield that stops everything from the front. Get round it |
| | `cargo run -- list` | nothing: every one of these printed with a line each, and what the environment adds. `--list`, `--help` and `-h` are it too |

Whichever of them you open, **Esc → Restart → Start again** puts the run back
to the situation it opened in — the fight as it was dealt, the run as the
lobby started it — without leaving the window; the end
screen, when the crew are down, offers the same as *Start again*. Nothing on
disk is touched by it, and a saved game is still there to load.

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
./check                # the quick tier, under a minute: the fast crates' tests, the pinned
                       #   numbers, clippy, and four windows opened and screenshotted
./check full           # the same deepened — every crate's tests, every sweep, every
                       #   window, the probes, the flake
./hidden <command>     # runs it on a headless compositor — nothing opens on the desktop
```

`bims --self-check` prints whether this build agrees with the constants the
fixtures pin — the design hash, the world checksum, the money arithmetic —
and exits non-zero if it does not.

## The builder

`nix run .` opens here: what comes *before* the ship and the world — a start
menu, a game setup screen, and a lobby. It is `crates/app/src/screens/builder.rs`,
a screen of its own, and the game's screen knows nothing about menus. Only the
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

- **Game setup** — the money each Bim brings at €2 500, €5 000 or
  €10 000, €5 000 unless you say otherwise. Nobody starts with stores:
  everybody's money goes into **one pool**, and [the run](#a-run) opens on
  the default ship with it. There is no ship size to pick any more: a run
  is always the default ship.
- **World** — which galaxy, and where in it the game starts. A **seed** —
  any whole number up to a `u64`, typed in decimal, with **New seed** to
  draw one — and a **galaxy type**: two-arm spiral, spiral, elliptical or
  round. Under them the galaxy itself, a thousand stars on a canvas: drag to
  pan, scroll to zoom, hover to read a star's name and class, click one to
  open its system on the right — the star at the middle, its planets and
  belts on their orbits, its stations as squares, each named with its kind
  and the body it hangs off. A system with a station usually has more —
  up to six, two of a kind allowed. No station's people are enemies — every
  human is friendly in a run — so a crew may start at any of them. Stars
  with no station are dimmed, since the game cannot start there; they can
  still be looked at. **Start here** on a station makes it the pending
  start, marked on the map and named in the tab's header; **Random start**
  picks one anywhere. A new seed or a new type is a new galaxy and forgets
  the start.

  No distances, no travel times, nothing about what a station is like: the
  lobby is where a start is chosen, not where a system is explored.

One settings object sits behind both copies of the tool, so what you pick on
the setup screen is what the lobby shows and the other way about.

**Start** needs a station picked — until there is one the button is disabled
and says so — and then opens [the run](#a-run) on every machine in the
lobby. What crosses is numbers and nothing else: the money each Bim brings,
how many players there are, which slot you are, the seed in its two halves,
the galaxy type, and the star and the station the game starts at. What a
game is *started with* is what crosses into the simulation, and no
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

**Your Bim has a name, and so does everybody's pointer** (feature 60). The
Game setup tab has a **Your Bim** field, host's and guest's alike: what
you type there is what your crew member is called — over its head, in
its panel, in the log and the diary — and blank keeps the crew's own
name for the berth (James, Kate, Priya, Tomas). The lobby's crew list
shows each player's Bim's name beside them, in the colour that player's
pointer and route will be drawn in; the names are dealt with the slots
at Start, one said late still lands, and a save keeps them. Over the
ship, every other player's **pointer** is drawn where it is —
a see-through arrow in their colour with their Bim's name beside it, over
the tile they are over on *your* screen whatever they have zoomed or
turned, gone when theirs is off the ship or on the map. It is sent twenty
times a second at most and never through the host: a pointer is a picture,
not an order, and it is in nothing that has to agree.

**No two Bims look quite alike, and yours wears the hair you gave it**
(feature 62). Every body is one of five **builds**, from slight to
sturdy — six per cent of the body scale a step, enough to tell two
apart standing together — and wears one of eight **hairstyles** in one
of six **colours**: cropped, long, bald, bob, bun, mohawk, ponytail or
curly, in dark, brown, black, blond, red or grey, drawn from above on the
crown and again on the deck when the body is down. The first two of
any crew are the pair they always were (a dark crop with the pale yoke,
brown hair down past the collar with the mauve one); everybody after —
a crew of five, a station's people, a mercenary — is dealt a
look off its index, the same on every machine and off no dice, so a seed
that pins a probe is not moved by it. The Game setup tab's **Hair** row,
under the name field, is the chooser: the figure as it will stand on
the deck, a button a style and a swatch a colour, everybody's to pick
like the name; it goes out with the name, is dealt with the slots at
Start and one picked late still lands. A look is a picture and nothing
else — the reach, the hit test and the checksum are the same whatever
the hair — and a save carries it on the body.

**The host's world is the world** (feature 67). With company every copy
of the world is the host's, checked by checksum every couple of seconds;
when a guest's copy parts from it — a bug, a build not quite the host's
— the guest says so in the log (*Your world has drifted…*, *Catching up
with the host…*), asks the host for its world, and the host sends the
whole game as the text a save is, which the guest then plays on from as
its own crew member (*Back on the host's world.*). The same is how a
load works with company: **only the host can load** — a guest's Load is
greyed with the reason — and a game saved for a different number of
players than are in the room is refused (*That game was saved for N
players; M are here.*); a load that goes replaces the world on every
machine, and guests still in the yard are brought into the game with it.
A world in flight is a few megabytes, so it is a hitch on both ends, the
way a load is. A game of one loads as it always did.

## The ship designer

> **Not in a run any more** (feature 102, [A run](#a-run)): the lobby's
> Start opens the run on the default ship, and the yard is the `design`
> command's alone. What follows is the yard as it still is.

The lobby's **Start** used to go here: the whole crew laying out **one ship**
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

Under **Station** on the right is what there is to buy: vegetables, tofu,
medkits, bandages, a suit, and whatever gear the spawn station trades in
— at its own two prices a
unit — what one *costs* here and what the desk *pays* for one, the
station's ask and bid (*Trading*, under the game, has the sum). Buttons
move one, ten or a hundred, and putting a thing back hands back what it
cost — nothing has left the dock, so there is nothing to lose on the
deal; the bid is shown, and is what a sale will fetch once the game has
started, but nothing is sold in the yard, only put back. The yard buys at
**tier one**: the tier chooser on the gear rows is the docked trade
window's, since the design phase's purchase takes no tier.

Two things bound a purchase, and the station's shelf is neither of them.
Supply is unlimited; what refuses an order is **the
pool** — goods come out of the same money the hull does, so a player who
spends everything on plating has nothing to load it with — or **the ship**.
Goods are stowed: food in a cold store, and everything else in a locker —
the armoury, the drug lab, the suit locker, and the **shelf**, which is
locker class too since the money rework took the shelf class away with
the materials that filled it. The readout under the rows is how full each
class is, and a ship with no cold store cannot take food at all however
much money there is.
Each of those is not a count but a **grid**, ten cells across and as many
rows as the parts aboard add up to — a shelf or a cold store is ten by
ten — and every thing kept there covers its footprint of it: a pistol a
row of two, a rifle seven, a sniper rifle the whole width; kevlar four
by four, a helm two by four; a crate of vegetables one by two, a block
of tofu four by four. Goods that stack take one footprint a stack — ten
vegetables to a crate, five dressings to a box — so what a shelf holds is
its cells times the stacks. A container's window lays its grid out as it
is: drag
a thing to move it, press `R` on the way to turn it, and a thing goes in
only where there is a run of cells for it — so a full hold is tidied,
not counted.

What is bought is aboard from the moment it is bought. It is in the design
hash, so a purchase clears everybody's Accept the way a wall does; it is in
the ship's mass, so the acceleration on the handoff screen already accounts
for it; and a shelf with something on it cannot be taken off until it is sold.

### Everything costs money, and weighs what the table says

Every part has a **price** in euros and a **mass** in the same table
(`shipdesign::parts`), and the two are deliberately unrelated: a wall
costs what a wall costs and weighs what a wall weighs. There was a third
column — a **recipe**, so many units of metal and components — and the
mass was that recipe added up; the money rework took the materials away
and wrote each part's mass down as exactly the number its recipe used to
come to, so nothing about how a ship flies moved.

That is what makes construction a **purchase** rather than a move. The
price of a part leaves the crew's pool and the part's mass arrives on the
ship; take the part off again and **the whole price comes back** — there
is no wastage, no scrap and no scrapping penalty, and the crew are
neither richer nor poorer for building and unbuilding. A ship's mass
changes by trading at a station, by building and deconstructing, and by
crew coming aboard or leaving — nothing is burnt in flight.

**Money works anywhere a part is concerned.** Goods are bought at a desk,
so they want a dock; a part does not, because euros are not a shelf: a
site is paid for docked, holding station or landed. The design phase
happens docked at the spawn station, which is why a part can go down
instantly there; out in the world a site is walked to and worked at, and
the price leaves the pool when the work begins (*Building, aboard*).

The rule is written down and tested, against every part in the table, in
`crates/shipdesign/src/materials.rs`: `site_price` says what a site
costs — a plating site is the floor and, where the tile has no frame, the
structure under it — and `refund_for` says what comes back.

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

The warning is the designer's own, and the designer's alone now: nothing in
a run doses anybody, in a suit or out of one, since the radiation went with
the rest of the old game (feature 104). The designer is untouched by that,
and still says what it always said.

That required list was a **mirror of what the room's chains walked to** — a
meal was a cold store, a worktop, a hob, a table with a chair and a
dishwasher; a night a bunk; a trip to the heads a toilet and then a basin.
It was never a design. The chains went with the needs (feature 104) and the
list stayed, since the designer is untouched and a run never passes through
it; `crates/shipdesign/src/validate.rs` still says at the top what it was a
mirror of.

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
light are 190 of the 327 it draws all day, more than its benches
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

Nothing comes before it any more. The lobby's Start opens the run
**docked at the station the lobby picked**, on the default ship, with the
pool in the crew's hands, and from then on there is one world, one clock
and one loop.

That is worth saying plainly because it is the decision everything else
hangs off. The star system, the ship in it, and everything aboard it all
advance together in `World::step`, which moves the world by a sixtieth of
a minute and nothing else — a minute of the **mission clock**, since the
world's own clock stands still until the crew travel (see [the
loop](#the-loop-world-map-travel-missions)). The crew are in it, and they
are **the room**: the Bims' simulation laid out on your ship, one Bim per
player from the first step and whatever bots the crew have taken on,
walking, working the doors, seeing, shooting, bleeding and dressing each
other's wounds inside that step — and the people of whatever station the
ship is tied to are a room of their own beside it. Construction is in the
same step, where it is on ([the crew build what you lay
out](#building-aboard)); a second clock would be two simulations that
disagree, and the failure would read as a ship in two places.

The room's fixtures are drawn with the room's own pictures — the fridge,
the hob and its pot, the pan, the bunk with its rails — turned with the
ship, and the Bims are named over their heads by the app (`CREW_NAMES` in
`crates/app/src/names.rs`, unless a player named theirs). The galley, the
bunks, the heads and the bay are drawn **as a fresh room draws them** and
never change from it: nothing aboard is cooked on, slept in or grown in
any more, and they are furniture a walk goes round.

Two limits, honestly stated: every fixture is used from the south — a
bench with a wall below it is a bench nobody can reach — and the room's
navigation cannot walk a one-tile corridor, so a ship built with them is
a ship whose crew freeze in them. The default ship has neither problem,
which is not an accident.

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

Three parts do nothing but make a deck nicer to look at — a **small
plant**, a **big plant** and a **picture**, under *Comforts* in the
designer and *Furniture* on the Build tab. They used to lift the crew's
**Surroundings**, the need that followed the state of the deck round a
Bim; with the needs gone they are pictures and nothing else. The default
ship carries a picture beside the bridge door and a small plant by the
bunk, and every station sets a few of its own about — a big plant in the
middle of the hub, a small one in the mess and the rec room, a picture in
the quarters and the rec room. The picture hangs from a wall the way a
wall light does.

### Locking doors, and what the enemy do about it

Every airlock is a door now: right-click one for the same four words a
bulkhead door has — open, close, lock, unlock — and a locked airlock is a
wall, to a walk and to the eye. A machine with nobody it can get to goes
for the doors: it walks to the nearest locked door it can reach and
**smashes** it, fifteen seconds for a bulkhead door and thirty for an
airlock, one body to a door, with a bar over the door saying how long the
lock has left and a heave heard every couple of seconds; then the lock
gives and the door opens for it. The crew never smash a door and never
seal themselves in — they are yours to send. Docked, the station's doors
are the same doors on both sides of the passage: a lock you set stops the
station's people, and a lock they set you can lift from the panel.

### The galaxy behind the map

On the map, **Galaxy view** swaps the system for the galaxy the lobby
showed: the star the crew are at is ringed in green, the lanes out of it
are lit, and the stars they reach are ringed — those are where the next
trip can go. **A trip follows the hyperlanes, one hop at most.** Pick any
star, near or far, and the chart draws the **shortest route** to it along
the lanes, but the world map only ever offers the places a hop away, so
getting across a galaxy is a chain of trips with a system to stop in at
every step of it, rather than one from anywhere to anywhere.

A jump puts the ship well inside the new system, and nothing of the old
one comes along — its stations, its people, its construction sites — and
everything of the ship's does. **The system remembers, though**: come back
and it is as the crew left it — a station they cleared still cleared, the
key they took still off its desk, the chart still charted, the dead still
dead — where a system never visited is as the galaxy rolled it. (A place
left *uncleared* is put back as its mission found it; see [the
loop](#the-loop-world-map-travel-missions).) **System view** puts the map
back.

The **hyperdrive** is still a part like an engine — a two-by-two block on
deck, wired like anything that draws, and **bolted to a main engine** or
the designer says so — and still behind a tier-one research key, after
fusion power. A run's jump does not ask for one: every trip across a lane
is quoted with the drive's twenty-minute charge, whatever is aboard.

### A town on a planet

A **rocky planet** or an **ice world** has ground, and a trip to its
**settlement** ends with the ship set down on a **landing pad**; a gas
giant and a belt have none. There is no space any more: the planet is the
whole of the surroundings. Beside the pad stands a **town** of five to
thirty people, on one of three **biomes** — **desert**, **temperate** or
**arctic** (an ice world is always arctic; a rocky planet is one of the
other two) — and no two towns are laid out the same. It is built like a
**fort**: a **wall** runs round the whole of it, the pad is set into the
west wall — the ship docks into the wall as it docks into a station's
hull — and two **gates**, each a street's width with a pier of wall
either side, open in the north wall and the south where the cross street
meets them. Its **houses** stand along two or three streets, a few bunks
each with a door onto the street; the **gathering hall** is its mess, a
galley along the north wall and tables with a chair for everyone; a
**bathhouse** holds a toilet, a basin and a shower for every twelve
people; the **trading house** is where trade is done across the desk as
at any station — the ship's airlock opens onto the ground through the
town's gate, and the Station button is the same — and the **watch house**
by the pad has two sandbags before it and the town's **guard** on its
round. A desert or temperate town has **fields** in a belt beyond the
houses, strips of soil, and an arctic town **greenhouses** full of
hydroponic bays — pictures now, like the bays aboard. Standing lights
line the streets, and by day the whole ground is lit. Over the ground the
town leaves inside its wall is the **wild**, and it is what stops a Bim
rather than a line on the ground: copses, a lake or a river and a few
boulders in temperate country; cliffs of rock, cactus scrub and one oasis
in a desert; outcrops, a frozen lake and firs in the arctic. A Bim off
the ship can walk any way it likes until a tree, a cliff, the water or a
wall is in the way — there is always a way from the pad to every door,
every field and both gates, and never a pocket it cannot get to.

The town is not the whole of the ground. It stands on a **plain** ten
thousand tiles across, and the ship is set down on open ground with the
plain on every side of it: walk round the hull, or out through either
gate, and keep going. What is out there is the planet's —
cliffs, water, forest too dense to push through, the odd tree and rock —
and it is what confines a crew member, not any edge; the plain's own
edge is a rim of cliff further off than anybody will walk. The ground is
made as it is walked and seen, never all at once, and the view reaches
**sixty tiles**: the camera cannot be pulled out further than that on a
planet, the ground is drawn that far from the middle of the window and
no further, and a crew member sees that far over open country. What the
crew have not seen of the plain is black; what they have seen and do not
see now is grey — and both have the same smooth edges as the fog on the
deck: the shadow a cliff or a forest throws is the cliff's edge, not a
stair of tiles; and a walk out into it is a walk like any other — right-
click the ground, however far, and the crew member goes leg by leg. A
town the machines hold is ringed in red on the map and fought through
like a held station, their lander coming down on the plain beyond a
gate. Leaving a planet is *Back to ship*, as it is anywhere.

### The reactor, the batteries and the brownout

Every step, what the wired reactors made less what the wired consumers drew
goes into the batteries, and the **Power** line on the ship panel says so:
so much drawn of so much made, and what the batteries have of what they
hold. A battery on the run arrives empty and fills at the surplus; one
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
- **The benches and the research desk stop**: no order is placed and the
  AI thinks about nothing until the power is back.

Nothing about the air: life support is essential and runs on, and nobody
dies of a brownout. A dark ship and a stopped bench are something to work
back from, not a game over.

### Making things

The crew make **one thing**, at one bench, and one mechanism does all of
it. A **recipe** is a bench, what goes in, what comes out and how long it
takes; the **drug lab** turns two **vegetables** into one **medkit** in a
quarter of an hour, and that is the whole table. Everything else a ship
carries is **bought** — see [Trading](#trading) — because since the money
rework there are no materials: no ore, no metal, no components, no
galvum, no emitters. A part of the ship costs euros and a gun costs
euros, and what a crew do to get richer is travel, fight and trade rather
than run a production line.

The **workbench** still stands, and it is still worked at: two weapons or
two pieces of armour of a kind at one tier go onto it and one of the next
tier comes off, a day later (see *Armour*, and *Two of a kind go onto the
workbench* in `crates/world/CLAUDE.md`). The **armoury** still stands too,
as the cabinet the crew fetch a gun out of and stow one into. Neither
makes anything.

What turns the one recipe into an errand is a **target**: on the
Management tab's crafts table, the medkit's row carries a *keep so many*
number, stepped up and
down, and while the hold has fewer medkits than the number — and two
vegetables to make one with, and room for it, and a powered drug lab
aboard — the work list offers *Making things*. A Bim walks to the bench,
stands at it for a quarter of an hour, and the vegetables come out of the
hold and the medkit goes in. The log says what was made. If the vegetables
were sold while the Bim stood there, nothing is made and the log says that
instead. The target is a command like a deal, so every player's ship is
making the same thing.

Medicine is known from the first day. **Crafting conserves mass**: a
medkit weighs exactly two vegetables, and nothing vents anything — the
smelter that used to is gone. The drug lab draws power, and it stops in a
brownout.

### Research

**Research is done by the ship's AI**, at the **research desk** — a
console on a table's footprint, worked from the tile below, drawing ten —
because the humans aboard have stopped being able to. The **Research** tab
at the bottom left is the tree: the nodes as boxes, what is known on the
left and what waits on it to the right, a line from each to what it
needs. Since the money rework the tree is **five nodes**, because most of
what it used to gate was a production chain and there are no production
chains left. A crew sets out knowing everything a crew needs to live, to
fight and to fly — the hull, the galley, the heads, the bunks, the
hydroponic bay, the fission reactor, the helm, the engines, the suit
locker, the workbench and the armoury — and knowing **medicine** (the
drug lab and the medkit it makes), so all of that is possible from the
first day. Three nodes are left to work for: **fusion power** (a **fusion
reactor** that makes 3 500 a minute in a three-by-three block, thirty
times the fission one), which is the one node with no key on it; the
**hyperdrive**, after fusion power and behind a **lock**; and, in the
second tier and behind a lock of its own, the workbench's **upgrades**.
Click a node for what it
opens and to **queue** it: whatever it needs that is not yet known goes
onto the queue ahead of it — queue the hyperdrive on a fresh crew and
fusion power goes in first — and the AI works through the
queue in order on the clock, going straight onto the next node the step
one is done, as long as the desk has power, and stopping if the power
goes. Research is slow: two days for the reactor, and most of a day to a
day and a quarter for each locked node. Every queued box wears its place
in the line, the line itself is written under the tree, and a node picked
there can be **taken off the queue** — taking with it whatever was queued
behind it that needed it — or, if the AI is on it, **stopped**, which
loses what was put in and sends the AI onto the next. A part the crew do
not know is not on the Build tab and not in the designer's palette, so a
fusion reactor cannot be laid out until fusion power is known.

The lock is opened with a **research key**: an artifact, sold nowhere and
made nowhere, that sits on the research desk of **four stations in five**
of those the generator rolled friendly — the one you set out from always
has one — **lit up**, a ring
of lights round the desk that pulse while the key is there, so it can be
seen from the door. Right-click the station's desk and **Take the
research key** walks the crew member you steer over and takes it into
their pack, where it is **two cells tall**: a pack with no two free cells
one over the other cannot take it. Carry it home, store it from the pack
into the ship's own research desk (a click on the desk opens its
window, one slot the key's exact size) and on the Research tab **Consume
a key** with the node picked: the key is gone and that node's lock is
open for good — that node alone, since **one key opens one node**, not
the tier. There is one tier-one locked node now, the hyperdrive.
**Tier two** is one
node, the workbench's upgrades, and its key is the **tier-two research
key**: the same slab, drawn in the tier-two blue, lying on the research
desk of **every station the generator rolled hostile** — about three in
ten of those somebody lives on, whose people are nobody's enemies in a
run, since every enemy is a machine — lit up the same way and taken the
same way; nobody sells one. A node wants a key of its own tier, so a tier-one key in the
desk does nothing for the upgrades and a tier-two key nothing for the
hyperdrive, and the desk holds one key of either. Tier three is declared
and empty. A key a station buys back — five thousand euros for a
tier-one, ten for a tier-two — if you have no use for it, and a key
taken is a key gone: the desk stays bare.

### Mining, on foot

**There is no mining.** There was: a belt held at laid a field of
asteroids out on the ship's own tile grid, rocks were marked with a pick
pointer, and a Bim in a pressure suit walked out and dug ore and galvum
out of their cores. The money rework took the whole of it away with the
materials it fed — there is no ore, no metal, no galvum and nothing to
smelt them at — so the **Actions** tab, the marks, the *Mining outside*
job and the site itself are all gone.

What is left of it: the **asteroid belts** are still bodies in a system,
still on the map — there is simply nothing to go to one for. The **mining
outposts** are still a kind of station, dug into rocky planets and ice
worlds, and still trade. And the **pressure suit** and the **suit
locker** are still aboard, because a walk outside is still how a
construction site beyond the hull is reached (see *Building, aboard*):
the Bim takes the suit from the locker, walks to the deck inside the
airlock, goes out onto a navigation grid of its own — a hundred tiles
every way about it, rebuilt as it moves — works at the site, and comes
back in and hangs the suit up.

One body outside at a time: the airlock is one Bim's while a walk is on.
A right-click on the deck does nothing to a Bim outside: the walk is what
brings it in. What used to bound a walk outside was **radiation** — the
suit let a quarter of the open dose through — and it went with the rest
of the old game (feature 104): nobody is dosed out there any more.

### Building, aboard

> **Off in a run** (feature 102, [A run](#a-run)): nothing is built onto the ship but a class's sandbags and
> sentries, and the Build tab is not shown.

The ship goes on being built after the design phase — by the crew, paid
for out of the crew's money, and nothing is instant. The **Build** tab at
the bottom left is the palette: the parts by category — *Structure* for
the deck, the walls and the hull and the ways through it, *Furniture*,
*Production* for the benches, the armoury and the bay, *Galley*,
*Hygiene*, *Power*, *Ship systems*, *Propulsion* — each row with what the
part **costs**, dimmed to a warning where the money will not stretch, and
a search box over the lot for when you know the word and not the
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
crew work it as one job on the work list, *Building*. There is no
hauling: since the money rework a part is **paid for** rather than made,
so nothing is carried anywhere and no shelf is emptied. A Bim stands
beside the site and puts it together — a few minutes for a wall and a
couple of hours for a heavy engine, the length worked out from the price
— with a **charge bar on the tile** while it works, and the part is on
the ship: the room the crew live in is laid out again under them with the
new wall a solid in it, the new shelf somewhere to fetch from, and
nobody's errand is lost for it.

**A part's price leaves the pool when its site is begun**, not when it is
laid out, and the crew are never allowed to begin more than they can pay
for: a site whose price the money left over — less every site already
begun — will not cover simply waits, and the log says *not enough money*.
**Taking a part off gives the whole price back.** And a site is paid for
**anywhere**: docked, holding station or landed on a planet, because
euros are not a shelf and a crew with money in hand can build with it
wherever they are. The Build tab's **Laid out** list says what each site
costs and what is under way, with a way to call each one off; a site
called off costs nothing.

A site can be **outside the hull**. Plating laid out against the skin, an
outside wall on it, a thruster in the void beside the ship: any site with
no tile beside it that a body can stand on from the deck is reached from
outside, and the crew take the suit from the locker, go out through the
airlock, walk round the hull on the outside's grid to the tile beside the
site, build there, and come back in, through the same one-at-a-time
airlock. No tools are needed for any of it, only the money.

**Nothing was ever built on a ship that was moving**, and a ship never
moves now: it is docked, landed or holding station wherever the crew are,
so a site can be laid out and worked at any of them.

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
colour, the way the galaxy chart tags your star. **Where you have already
been is ticked**: a small grey tick at the shoulder of every station,
planet and belt the ship has actually stopped at — docked, landed or
holding beside — and on the galaxy chart a grey ring round every star you
have been to, so a map you have travelled about says where you have been.
Click a station or a settlement and the world map's list picks it, with
the trip quoted; *Propose* puts it to the crew (see [the
loop](#the-loop-world-map-travel-missions)). A planet you are alongside
is drawn under the hull in the ship view, as the ground.

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
navigation plans straight through an unlocked one. Four things are on a
right-click: **hold open**, **close** (let it look after itself again),
**lock** and **unlock** — each an errand that walks the Bim you steer to
the panel. A locked door is a
wall until it is unlocked: no route is planned through it, the readout
says `Door · locked`, and the leaves wait for anyone standing in the
opening before they shut. A hydroponic bay, where a station has one, is a
**run of six trays**, one a tile — a bay standing north–south is six
trays down. In the ship view a station is drawn where it is and as big as
it is, so it *approaches*: from far off a plate with its icon on it,
nearer its hull tile by tile, and within fifty tiles of its hull the
people who live there — two on most stations, one on a relay, nobody on a
derelict, whose room opens all the same so its fixtures are drawn — are
the room's Bims again, up and about between the room's own pictures of
its fixtures, with names over their heads. Leave and come back and they
are there again — **the living ones**: whoever died there stays dead, and
a mercenary hired away is not there to hire twice. **And the dead are
still lying there**: every body stays where it fell, in the coverall it
wore and with whatever it had on it, for as long as the station stands —
so a town the machines came for is a town with bodies in its streets when
the crew come back to it. The ship **docks beside it, airlock to
airlock**: a trip ends at the berth, the ship turned so its airlock
faces the station's, the two collars — each stands half a tile out of
its skin — meet as a tube between the hulls, and both doors are drawn
parted. The game opens docked at the first station somebody lives on, never
at a derelict. Escape opens a settings sheet with every key on it. An airlock has to be in the skin for that — one on the deck is a door
to nowhere, and the checks say so. And docked, **the two are one deck**: the
ship's and the station's on one navigation grid with the passage between
them, so a right-click on the station's deck sends the Bim you steer
through the airlocks. The station's people stay on the station: they live
in a room of their own, each on the round its role deals it ([A
run](#a-run)). The Management tab is the crew's own and reaches nobody
ashore. Who is who is on their backs: the crew wear the ship's blue
coverall and the station's people the station's orange one. Leaving takes
the deck apart again, once the crew are back aboard — see *Back to ship*
under [the loop](#the-loop-world-map-travel-missions).

Beyond the chart, the ship sees `VISION_RANGE` with the crew's own eyes,
which out here is almost nothing, and a great deal further with a **sensor
array**; what it sees is discovered, shared by the whole crew and never
forgotten.

Most systems have a station now — about three in five, and often more than
one — so a start is rarely far from somewhere to go.

### Speed, and who decides

Pause, 1×, 3×, 10×, 24× and 48×, on the keys — **Space** and **1** to
**5**, with no buttons for them on the screen since the HUD was cut down
(feature 107); the top frame says **Paused** while the world is.
**Every player has a request and the slowest one wins**; a pause by anybody
is a pause. That is not a compromise, it is the point: the player who needs
it slow is the player something is going wrong for, and nobody is ever
carried past something they wanted to look at.

### The HUD

The screen over the deck is kept to what a player needs to act on in the
next few seconds (feature 107); everything else is a key or a button away.

- **Top left, the crew's portraits**: one cell each, players and bots, eight
  to a row — the initial (and the number a bot is named by), a thin health
  bar with the armour's blue on the end of the green, and the level under
  it, `–` for no class. Your own is outlined in the green; a tick in the
  corner is a player who has pressed *Back to ship*, a red cross one who is
  down, and a greyed cell saying **out** is dead, or a player's Bim waiting
  to be bought back. Click a cell to pick that Bim as a click on it on the
  deck would; click your own twice for the character sheet.
- **Top centre, one frame**: the day, the pool, the bounty waiting on the
  place being cleared (`+€ n on clear`, while there is any), and **the one
  warning that matters most** — the machines' wave or the countdown to the
  next, then the alarm, then a blade at your Bim, then being recruited,
  then your standing order to the crew — with `+2` for however many more
  are up; rest on it for all of them. **Paused** beside it while the world is.
- **Bottom centre, the hero panel** — your own Bim: the level in a circle
  ringed by how far through it you are, the health bar, the experience
  (`Lv 4 · 50 / 250 XP`, `Max` at the tenth, *No class* without one), the
  class's keys and the medicine as boxes, a **+1** while a talent is waiting
  to be picked (it opens the character sheet), and a line saying what is
  taking the Bim down while anything is. Down, the panel greys over and
  says so, with the time the blood has left.
- **Bottom left, the tray**: **Stash** (Tab) — what the ship holds, then
  what each of the crew carries, with *Open pack* for your own whole pack
  and what is within reach — **Squad**, every bot with its class, level and
  health and the orders the crew take (Attack, Retreat, Follow me, and a
  commander's Fall back and Stand ground), and **Map**, the world map as
  **M** is; and **Trade** while the ship is at a desk. The panel of the one
  pressed opens upwards; pressing it again folds it away.
- **The character sheet** — **K** — on the left: the class and level with
  the experience, the class picker while it may still be changed, each
  part's health, what is worn with its tier and what is left of it, the
  weapon and its tier, and the talent tree, where a level's pick is spent.
- **Bottom right**, *Back to ship* — `Returning · 1 / 2` once pressed, the
  players aboard who have pressed it of the players the ship waits for —
  and what just happened directly over it, four lines at most, each gone
  eight seconds after it came; experience comes in a line a source a second.
- **Right**, the panel of a crewmate you have picked: its health, its
  perils and its sheet. Your own is the hero panel's and the character
  sheet's, so it has none.
- Rest the pointer on the deck and a small readout beside it names what
  is under it and the tile.

A player whose Bim is **out** watches the mission through a crewmate's —
the portraits pick whose — under a *You're out* banner with the buyback's
price and the pool, and has no hero panel and a tray of the Map alone.
What the ship view draws over the ship — the plain deck or the
electricity — is on the Esc sheet's menu. The Work, Management, Build,
Research and Ship tabs are gone from the HUD, and the crew keep to what
they were left on.

### Trading

> **In a run** (feature 102, [A run](#a-run)) a desk sells the crew
> **gear** — guns and armour, every tier — and nothing else: the food,
> the suits and the medicine on its shelf are greyed out. It still buys
> whatever it buys.

**Money only works while docked**, because a station is where there is somebody
to buy from. Holding station beside one is not docked — that wants an airlock —
and out between them the pool buys nothing at all. Supply is unlimited; what
bounds a purchase is the money and the hold. Since the money rework trading
is most of the economy: there are no materials and almost nothing is made,
so what a crew carry, wear and shoot with was bought at somebody's desk.

**Every station charges its own prices, and two numbers a thing.** There
are two ideas of what a unit is worth, and they are kept apart on
purpose. The **book value** (`economy::trade_price`) is what a thing *is
worth* — the same everywhere, and used only to value what the crew own:
what they set out with, what they are worth now, how many hands a
station has for hire. Nothing is ever bought or sold at it, and the
machines never look at it. What a station's
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
a hand-written table, starting values to be tuned: a mining outpost pays
well for food and medicine and is dear on gear; a refinery is the
cheapest place to buy a gun; an orbital sells food cheap; a relay is dear
on everything and pays well for food and bandages; a planet's settlement
sells food cheap; a derelict has no market at all — nothing to buy, and
nobody to sell to.
`local_bias` is the station's own lean, one small whole number per cent
a resource in `−15..=15`, rolled by the generator off the station's seed
for every resource whether it stocks the thing or not, and in the galaxy
checksum beside its shelf — so two outposts in one system are two
different outposts, and two players on one seed see the same numbers.
`SPREAD_BP` is the desk's cut, a thousand basis points: five per cent
either side of the mid, and never less than a euro. A desk **inside the
front** — within three hyperlane hops of the machines — charges over the
odds for what a fight is fought with on top of all that; see *The
crisis*.

**The station you start at leans only the way its kind does**: the
generator rolls it a local bias like any other, and the world sets it to
nothing (`World::start`, and the yard's Station panel with it), so an
opening pool buys the same at a kind of station whatever the seed
rolled, and what the crew set out with is worth its book value.

**Not every place sells gear.** Beside its shelf of goods, every place
with a market rolls **two trades** off its own seed, each independently
and each two in five: a **weapon trade** — handguns, shotguns, auto
rifles, sniper rifles and schwords — and an **armour trade** — helms,
kevlar and leg guards. A place may have both, one or neither, a derelict
has neither, and the trade is all of its list or none of it: nowhere
sells three of the five guns. Which trades a place has is said before you
go there — on the map — and on a line of its own at the top of the trade
window, because a
crew looking for a rifle are choosing a station rather than a system.

**Every tier is on sale where its trade is.** A weapon or a piece of
armour comes at tier one, two or three, and a market that deals in it
deals in all three: the price is the book times **one, four and sixteen**
(`economy::TIER_PRICE`), on the ask and the bid alike. The trade window's
gear rows carry a **1 2 3** chooser for it — pick a tier and the row's
*Costs* and *Pays* are that tier's, and a line already in the cart goes,
since a tier changed under a line would be that line at another price.
Selling needs no chooser: a sale gives up the **lowest** tiers in the
hold first and the desk pays for each thing at its own tier, so a crew
that has combined two pistols into one keeps the good one until they
choose to let it go.

**Tier one's book prices are hand-written**, and the armour's are a rule:
a handgun 1 500, a shotgun 3 000, an auto rifle 4 000, a sniper rifle
5 000, a schword 5 000; and **a hundred euros a point of health** for
armour — leg guards 1 000, a helm 1 500, kevlar 2 000. The labour rule
that used to price everything a bench made went with the production
chains it was written to keep honest: with one recipe left there is no
chain to print money along, and every book value is a number somebody
chose.

The shelf is a window — **Trade** on the tray, docked, opens it in the
middle of the screen and the cross, Escape or leaving shuts it — and
what is *on* it is two rules deep. The kind's is the ceiling: nothing at
a derelict — there is nobody aboard to sell it. Under that each station
keeps a shelf of its own, rolled off its seed: **vegetables, tofu and
medkits are on every one**, because a station where the crew can buy
nothing to eat and nothing to treat a wound with is a trap, and each of
the rest is there or not, so two refineries stock different things and
there is a reason to go to the other one. A row the
station does not sell is greyed with its buy buttons off, and stays,
because what is aboard can still be sold there, at the bid. Every station
somebody lives on buys anything; a derelict buys nothing, since there is
nobody at its desk. A station is not yet *for* anything beyond the way
its kind leans and the trades it rolled — a theme would replace the roll,
not the ceiling.

### Mercenaries

At a station somebody lives on there may be a **mercenary**
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
two tiles and the first month's fee in hand — the window greys the
button and says which is missing; a bunk is furniture, and there need not
be one free. Hired, they walk out of the station's room and onto your
deck, a crew member from then on: they work and fight like the rest, and
take orders like a crewmate (only the Bim you steer takes yours). **They are paid by the month**,
every thirty days out of the crew's money, and the log says so. A month
the money will not cover is a month owed: the log says they are unpaid,
and at the next berth they walk off — back into the station's room, for
hire again when you can afford them. The `test` command always has one
for hire at its dock.

**About a third of them are field medics** (feature 86), and the Hire
window says so at the top before it says what they carry. A field medic
is hired for its trade and not for its gun: **four thousand a month** on
top of the kit, and for it you get somebody whose whole business in a
fight is your crew. It has **none of a medic's own skills** — no beam,
no surge, no class at all — and what it does instead is this:

* it keeps to the **far end of its weapon's reach** and shoots from
  there, so it is still standing when somebody needs fetching;
* the moment a crew member goes down within about eighteen tiles — out
  cold, or in a dying state — it goes and **picks them up**, walks them
  clear of the fight at six tenths pace with its fire held, sets them
  down somewhere nothing can see them, and **treats them there**;
* it carries a **medic's medicine** — four medkits and ten bandages, where
  anybody else carries one and five — and each one it spends comes back
  into its pack on the same cooldown as anybody's (see *Medkits and
  bandages are charges* under *Getting hurt*).

Before anybody is down it is an ordinary crew member with a gun, and it
works and fights like the rest.

### The trading desk

Every station keeps a **trading desk** just inside its port — a wooden
counter with a ledger and a terminal on it, against the corridor's north
wall. The station is traded with across it: **Trade** on the tray still
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
under the light fog. Home is the station you set out from, a station the
machines hold is hostile, and every other is neutral.

### The two views

**Ship** is the live ship at tile scale, drawn turned to its heading, with the
starfield behind it. The hull is plated, with running lights to port and
starboard and a strobe at the bow. A run's ship is never under way — it is
docked, landed or holding station — so its engines never burn and its
thrusters never puff: the exhaust the flown trip drew went with it
(feature 104). **System map** is the star, what the crew have found, the
ring the scanner reaches to, and a little hull pointing where the ship is
pointing.

The camera is **head up in both by default**: the ship held square to the
window, the deck the way it was laid out, and the sky and the map turned
round it instead. **North up** (`N`) is the other choice:
the camera never turns and it is the ship that turns on screen.

The ship view **follows the crew member you steer**: your Bim is what sits
in the middle, on the deck or across a station, and a drag can shove the
view only so far before it would be off the edge. **Free camera** (the View
tab, or `V` — `F` is the attack order since feature 84) lets it go — the view stays where it is and a drag or the
keys take it anywhere, for looking at the far end of a station while the
crew are busy at this one — and **Follow** snaps it back.

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
  mass, inertia, acceleration, how fast it turns. It also has the closed-form
  plan the flown trip was read off, worked out **once** and read at a time so
  that nothing integrated; a run flies nothing, and its trips are quoted off
  the ship's accelerations instead (`physics::travel_days`, in *The loop*).
- **`crates/world`** is the star system, the ship in it, the run, the clocks,
  and the order things happen in. It renders nothing and knows nothing about
  a window.

What the steps after this one are promised is written down at the top of
`crates/world/src/lib.rs`: Bims live in ship-design tile coordinates and the
ship's rotation does not reach them, every ship change goes through
`on_ship_changed`, construction spends what is aboard and asks
`can_modify_part` first, and money is only for docks.

Which world you land in is a seed and a galaxy shape, and for now they come off
the settings the lobby hands over, with a fixed default behind them. The
lobby's World tab will pick them instead; nothing else about them changes.

## What the ship will make

None of this exists yet. It is the contract the next few steps are built to,
written down first so that each of them is measured against something, and
each heading below is replaced by a description of the real thing as it
lands. The order is the order of the headings: power, the resources, the
workstations, the walk outside and the armoury — all built, and described
under the designer and the game — then the fight, of which the machines'
half is built, the boarding half was built and deleted again, and the
rest is not.

### One mechanism, the tree, and the mass

Built, and then mostly **taken away again**. *Making things* under the
game is the mechanism and the table as they stand: one row, two
vegetables into a medkit at the drug lab. The tree of production the
mechanism was built for — ore into metal into components into emitters
into guns — went with the money rework: there are no materials, a part of
the ship costs euros, and a gun is bought rather than smelted. What the
mechanism kept is the mass rule, which still holds for the one recipe
there is: a medkit weighs exactly the two vegetables that went into it.

### Power

Built. It is *Power* under the designer and *The reactor, the batteries and
the brownout* under the game, below.

### The walk outside

Built, and then emptied: *Mining, on foot* under the game says what is
left of it. The suit and the airlock are still how a construction site
beyond the hull is reached; there is nothing out there to dig.

### The armoury

Built, and then **stripped back to a cabinet**. The **armoury** used to
be a bench and a locker in one, making the laser handgun, the vest and
the four other weapons out of metal, components and emitters. It makes
nothing now: since the money rework a weapon is **bought** at a desk that
has the weapon trade (see *Trading*), and the armoury is what it always
also was — the locker-class cabinet a crew fetch a gun out of and stow
one into, eighty cells of it. The **vest** is gone entirely; the three
real pieces of armour replaced it. Beside it stands the **drug lab**, the
one bench that still makes anything: two **vegetables** into a
**medkit** in a quarter of an hour. A medkit is what gets a Bim out of a
dying state and a **bandage** — bought, not made — is what closes the
wounds on one part of a body; see *Getting hurt* under the game. What a
Bim does with a handgun is the fight, below.

### Armour

Built. Three pieces — a **helm** (+15 health, 2 protection), **kevlar**
(+20, 2) and **leg guards** (+10, 1) — **bought** rather than made, at
any place with the armour trade, priced at **a hundred euros a point of
health**: leg guards 1 000, a helm 1 500, kevlar 2 000. The playtest ship
carries one of each. A
piece is two things at once, on purpose. **In a container it is a
resource** — `Helm`, `Kevlar`, `LegGuard`, locker class beside the medkits
— so buying, selling, mass and the shelves work on it with no
new mechanism; **anywhere else it is an instance**, with an id that only
climbs and a health it keeps wherever it goes. The world keeps every piece
there is (`World::pieces`: id, kind, health left, and where — the hold, a
pack cell, or worn by somebody) and holds one invariant against the hold:
the count of each armour resource is always the number of pieces in the
hold of that kind. A purchase or a workbench upgrade pushes a whole piece;
a sale takes the most damaged one first.
What is worn and what is in the pack are the
room's (`bims::combat::Gear`), since the room's health reads them, and the
world reads them back every step for the checksum.

Five commands move a piece about, the way a craft target is a command —
the hold is the world's, and every player's ship has to agree what is in
it: **Fetch** takes a piece (or a unit of anything) out of a container into
the crew member's pack, **Stow** puts one back, **Equip** puts on what
is in a pack cell and swaps what was worn into it, **Unequip** takes a
piece off into the pack, and **Discard** throws one away — and a sixth,
**Loot**, takes one thing off a body that is down (see [Combat
mode](#combat-mode-and-the-inventory)). A fetch or a
stow wants the crew member within two tiles (`REACH`) of a container that
takes the thing — a shelf, the armoury or the drug lab for locker goods,
the cold store for food —
and is refused *out of reach* otherwise; a full pack refuses a fetch, a
full class refuses a stow, and a **broken** piece — at nought — cannot be
stowed or sold at all, only discarded. Equipping wants no container. What
a worn piece does to a hit is [Getting hurt](#getting-hurt): the
protection comes off the damage first, what is left drains the piece, and
only what the piece could not take reaches the body.

**Every weapon and every piece has a tier**, one to three. A tier is
**bought** — any market that deals in gear deals in every tier of it, at
the book times one, four and sixteen — or **made at the workbench**, once
the crew know how: the
**upgrades** node of the research tree, tier two, behind a tier-two key
off the desk of a station the generator rolled hostile
([Research](#research)); until it is researched the
button says so and the crew carry nothing to the bench. Two of a kind at
the same tier go into
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
- **Boarding** was docking to somebody hostile — a station whose people
  were the enemy, three in ten of them as the generator rolled it, or a
  raider tied up to the ship — with the rooms joined into one, one nav
  grid, and Bims with handguns in the corridors. It was built, both ways,
  and it is gone: every enemy is a machine since the first step of the
  roguelike redesign, and the third deleted the human ones — the hostile
  stations, the raiders, their garrisons and their plunder (feature 104).
  What it was built on is all still there: the rooms joined, the
  corridors, the cover, and a hit that is a wound on a part of the body
  bleeding until somebody dresses it — see [Combat
  mode](#combat-mode-and-the-inventory). Armour is in too — the section
  above.

- **The machines** are the endgame enemy, and they are in as far as a
  probe goes: a race of three — the low, quick **Husk** that scuttles on
  four legs and snaps its claws at arm's length; the **Trooper**, upright
  with a gun built into its forearm, which walks into the open and fires
  on the move; and the broad, shoulder-plated **Warden**, whose
  **Unmaker** strips the armour off whatever it hits rather than opening
  the body under it. Close to where the machines began — tier three —
  a wave has a fourth: the **Guardian**, the largest, a heavy walker
  behind a **shield** that stops every bolt and every blow coming at it
  from the front, the ±60° its plate covers, flaring where it stops one.
  The shield cannot be broken, but a grenade's burst is not stopped by
  it, and the arc is marked faintly on the deck under it: **get round
  it**. It turns to face the nearest crew member it sees — a tank's
  taunt turns it — no faster than 75° a second, and walks in the open,
  the shield being its cover. Its weapon is the **Sweeper**, a beam: when
  it has a crew member in front of it, it **plants its feet and winds
  up** for a second and a fifth — its heading fixed, so that is the
  moment to get round it — then **sweeps** the beam through twenty
  degrees across where it aimed in half a second, and rests three and a
  half before the next. The beam reaches twenty tiles, stops at the
  first wall or shut door, and goes **through** bodies: everybody in
  its arc takes thirty, once a sweep. Sandbags between you and it are
  cover — the beam goes over — unless you are leaning out of them, and
  a tank's wall and a medic's surge work on it as on a bolt. It hurts
  no machine. One in eight of a tier-three wave is a Guardian, at least
  one from four machines up.
  `nix run .#guardian` is the fight against one. A machine is not a Bim: it has four parts rather
  than three (head, chassis, arms, legs), no blood, no dying state — head
  or chassis at nothing and it is a wreck that instant — and nothing to
  loot, since its arm is part of it. Shoot its arms off and it aims half
  as well; shoot its legs off and it fights where it stands.
  A **droid-held station** has no people at all, and its machines come in
  **waves**: how many waves is fixed the first time the crew dock there,
  how big each is worked out as it lands — off the world clock and the
  number of players, and nothing else — and the next never arrives
  while one of the last is still standing — two hours of the mission clock
  after the last one falls, a reinforcement ship tying up at the far airlock or
  a lander coming down on the plain beyond a town's gate.
  `nix run .#droids` and `.#droids_planet` are how one station's fight is
  looked at on its own.

- **The crisis** is which stations they hold, and it is a clock rather
  than a dice roll. Every galaxy is webbed with **hyperlanes** — each
  star joined to its three nearest neighbours, and whatever else it takes
  to leave one connected graph, drawn faintly under the stars on the
  galaxy chart. **From day nought** the machines hold **one star**,
  rolled at the start of the game at least eight lane hops from wherever
  the crew began, and from there the infestation spreads **one hop every
  five days**. That is the whole rule: a star is theirs from
  `5 × hops` days onwards, so two players in one game agree about the
  whole galaxy without a word passing between them, and the chart can say
  under any star you pick exactly which day it is due. At three lanes a
  star a galaxy runs a hundred-odd hops across, so the last star in it
  falls somewhere around **day 480 to 715** — the crisis is a season, not
  a raid. (It used to hold off until day ten; since feature 102 it is
  there when the run opens, and a system due by then is the machines'
  whole the moment you arrive in it.)

  **An infested system is the machines'**: every station in it, orbital
  or town, its people gone, nothing to trade, nobody to hire, no shelf to
  buy from — and a wave of machines standing in each. A station the crew
  **cleared** stays cleared: the crisis never re-arms it. Since only travel
  moves the clock, a system's day never comes while the crew are in it: it
  comes on the way, and the crew arrive to find the place the machines'.
  `nix run .#crisis` opens the simulation on the eve of it.

- **The jammer** is what makes an infested system a corner rather than a
  place to pass through. Every one of them has exactly one — the orbital
  station with the lowest number, or, where a system has no orbital
  station at all, one the machines built themselves out in the dark — and
  while it stands, **the lanes inward are shut**: a jump to a star
  *nearer* the machines' origin than this one is refused. Sideways and
  outward are open, and a trip *into* an infested system is never
  refused. So pushing towards where the machines began is a one-way door
  each time, and the way back opens only by boarding the jammer station
  and destroying every wave on it — once cleared it stays cleared, through
  a save, a load and every later spread. The chart says which station
  holds it and bars in red every step of a route a jammer would turn
  back, and the world map's list says which place is a system's jammer.

  **And the machines get harder the nearer their origin you are**: within
  two hops of it every wave comes at **tier three**, with tier-three arms
  and tier-three armour, where the rest of the galaxy meets them at tier
  one.
  `nix run .#jammer` opens the crew inside an infested system two hops
  from the origin, with the jammer standing and a wave aboard.

- **The front** is the ring of systems just outside the infection, and it
  is a place to trade in rather than a warning to read. A system three
  hops or fewer from the edge is on it, and the nearer it is the more it
  shows: **weapons, armour, medkits and bandages** cost and fetch fifteen
  per cent over the odds on the edge itself, ten two hops out and five
  three hops out, while the food is priced as it is anywhere: a crew
  buying its guns a hop from the machines pays over the odds for them,
  and one selling its kit there is paid over the odds. The trade window says
  so outright when you are at such a desk. And there is **one more hand
  for hire** at every station on the front — people are on the move where
  the machines are coming, and armed.

- **Defending a town** is what the front is *for*. A friendly settlement
  whose system is one hop outside the infection is **threatened**, and
  the map says so under its planet's name. Set down at one and the
  machines come for it: an hour of the mission clock later a wave lands
  outside a gate and walks in, and another after each is destroyed. It is
  the one fight with somebody else on your side — the town's **guard** and
  whatever **mercenaries** live there take arms and fight beside you,
  everybody else walks into the nearest house and stays there, and the
  machines come for the townsfolk as readily as for you. **Go back to the
  ship before the last wave is down and the town falls to them.** Let the
  last wave be destroyed and the town is **held**: it stays friendly for
  good — trading and hiring even after its own system falls to the
  crisis, the one friendly desk left inside it — and **some of its people
  join your crew**, a fifth of the survivors, never the guard, as
  classless hands who draw no wages. Let every one of its people die and
  it falls like any other place.
  `nix run .#defense` opens a town with the machines a minute away.

### Classes and levels

Each player picks a **class** for their crew member — **None**,
**Engineer**, **Soldier**, **Medic**, **Tank** or **Commander** — on the setup tab before Start (the lobby
deals it with the slot, like the hair), and can still change it on the
crew panel until the ship first leaves the place the run opened at. A crew member has one
class, and a dead one's level, experience and picks die with it.

**A class is worn**, so the deck says who is who at a glance: the
coverall is dyed the class's own shade — an engineer ochre, a soldier
olive, a medic white, a tank steel, a commander navy, and a crew
member with no class the blue it always was — with something on the
head and something over the chest to go with it. The engineer is in a
hard hat with a tool belt and pouches; the **soldier** wears
sunglasses, crossed webbing with three magazines on it and a belt; the
medic a capped red cross with another on the chest and a bag on the
hip; the tank a heavy helm with a pauldron on each shoulder, and the
biggest body of the six; the commander an officer's cap banded in
gold, gold shoulder boards and a sash. The coverall underneath still
says whose it is — an engineer of the crew is an ochre-ish blue where
one ashore is an ochre-ish orange — and a helm worn over the class's
own cap carries a band of its colour instead, so a helmeted crew are
still read a class at a time. The setup tab's portrait wears whatever
class is picked under it.

**A class owns abilities, never jobs or money.** Every crew member does
every job, takes every errand, places every site and uses every weapon,
and every one brings the same money to the pool, whatever its class. A
class only adds its own two keys and its talents, which nobody else can
use: **Q** is the class's first action and **E** its second, whichever
class you steer — an engineer's sentry and sandbags, a soldier's grenade
and brace, a medic's surge and heal beam, a tank's taunt and
wall, a commander's rally and attack order — and both are rebindable on
the Controls page as one pair,
not one a class. The **commander** has two keys more, his alone: **X**
calls the squad back and **Z** has it stand its ground, and both do
nothing for any other class. The **medic** has one of his own: **G**
picks a crewmate up and sets them down. With a classless crew member
steered the keys do
nothing; a press the world refuses says why in the log.

**Every ability has a box at the foot of the screen**, one a key: the
key in its corner, the picture in the middle, how many are left in the
other, and the name under it. A commander has four of them — rally,
attack, fall back, stand ground — and a medic three, the carry beside
its surge and beam; everybody else has the two. **Resting on a box rings
the Bims that cast would reach** on the deck: the crew a rally would
lift, the squad each of the three squad orders commands, the patients a
beam holds, whoever a carry could pick up. It is the panels' own rule —
resting on a row rings what it names — asked of an ability instead of a
fixture, and it is how you see what an order is about *before* you give
it. While a rally is actually running, the Bims it lifts wear its own
chevron rather than the aura's plain ring, so a rally called is told
from an aura standing there all along.

**Experience** comes from six things and nothing else: an enemy going
down within fifty tiles is 10 to every classed crew member in range, an
enemy dying 5, a construction site finished or a kit laid by the
engineer or anybody within fifty tiles of it is 2 to that engineer, a
**medic** finishing a bandage or a medkit treatment on a crewmate — any
crew member but itself, mercenaries included — is 5 to that medic, and
an enemy's shot or blow landing on a **tank** is a fifth of a point to
that tank, counted five hits for one, and a hire that goes through from
the slot steering a **commander** is 10 to that commander. Each
enemy counts once for each, a crewmate or a hire going down is nothing,
a kit packed up and laid again is nothing, an interrupted task is
nothing, a crew member who is not a medic doctoring earns nothing
at all, a hit on anybody who is not a tank is nothing, a refused hire is
nothing and a hire from anybody else's slot earns a commander nothing,
however near he stands. Ten levels — 100, 250, 450, 700, 1 000,
1 400, 1 900, 2 500 and 3 200 for the second to the tenth — the same
shape for every class: the first, third and seventh are fixed, and every
other level is a **pick of two talents**, never changed once made.

**A level is spent on the character sheet** (**K**, feature 107; it was
the tray's Skills tab). The class's ten levels are a tree
on the sheet, numbered down the left: a level
with nothing to choose is one slot across the width, a pick level is two
side by side, and over the tree is how many **skill points** are waiting
— one for every level reached that has not been chosen at. A slot
learnt is filled in, one given up is struck through, one open is ringed,
and one at a level you have not reached is dark. Click a slot and the
lines under the tree say what it does — and **what it is worth in
numbers**: what the figure is now and what it becomes, a sentry's 60
health to 90, a heal beam's 6 tiles to 9, a grenade's 2-second fuse to
1, so the choice between a level's two slots is a choice between two
numbers. A slot that is open has the
*Learn* button on it, which spends the point and cannot be undone.
While a point is waiting the hero panel carries a **+1**, which opens the
sheet; nothing opens by itself, nothing is learnt until it is spent, and
the choice waits as long as you like.

**What the two keys do is drawn on the hero panel**: a box each
for **Q** and **E**, with the key in one corner, the picture of what it
does in the middle, **how many are left** in the other — kits and
grenades in the pack, sentries the talents allow standing, beams free to
link, the squad's size. Resting on a box names it and says
what the key does. A box is lit while the key would be taken and dim
while it would not: the cooldown counts down over the picture, the
surge's charge is a bar along the foot, and a key not learnt yet says the
level it is learnt at — every class's **E** from the first level and its
**Q** from the third. A classless crew member has no keys and no boxes.

### The engineer

The first class (feature 74): sandbags, a sentry, and the workbench's
friend. It works on **charges**, not on kits made at a bench (feature
88): **three sandbag charges** and, from the third level, **one sentry
charge**, each spent charge coming back into the pack on its own
cooldown — **45 seconds** for a bag and **60** for the sentry. Nothing
is crafted, nothing is fetched and nobody walks for it: the charge
simply lands in the pack as the cooldown runs out, which is what the
count in the corner of the key's box is.

| level | left | right |
| --- | --- | --- |
| 1 | three sandbag charges, laid; packs deployables up | — |
| 2 | **Reinforced sand** — +50 sandbag health | **Site foreman** — build a quarter faster |
| 3 | **Sentry** — one sentry charge | — |
| 4 | **Sandbagger** — sandbags in half the time | **Bulk bags** — one charge lays two tiles |
| 5 | **Armoured sentry** — health ×1.5 | **Enhanced optics** — fire range +10 tiles |
| 6 | **Armourer** — mends armour at the workbench | **Higher quality armour** — his armour +5% health, +1 protection |
| 7 | **Sentry mark II** — its rifle at tier two | — |
| 8 | **Dug in** — sandbags anywhere between a sentry and the shooter are cover | **Quick build** — a sentry in half the time |
| 9 | **Extra bags** — one more sandbag charge | **Steady hands** — a hit no longer stops a deploy |
| 10 | **Second sentry** — two sentry charges | **Sentry mark III** — a tier-three sniper rifle at double the rate and a fifth more damage |

Every talent is a fighting talent: the tree had *quick hands* (crafting),
*deep magazine* and *field refit* (a sentry's ammunition) and *salvage*
(a kit back) on it, and none of the four is about a fight.

Only an engineer can lay them: `e` over a deck tile lays **sandbags**
there — the engineer walks beside it and works four minutes, and a hit
drops the errand with the charge still in the pack — and `q` sets up a
**sentry**, eight minutes, from the third level. Either wants clear deck
floor within reach that is not a door, with nothing on it. What is laid
is a **deployable**, never a part of the ship: it touches neither the
design nor its mass. Sandbags are cover exactly as the part is, in both
rooms of a docked fight — the enemy duck behind them too — take every
bolt a body dodges behind them, and **are gone for good at 200 health**;
a **grenade's burst destroys them** outright, whatever they had left.
There is no limit on bags laid: lay one every time a charge comes back
and the deck fills up with them.

A sentry is an auto rifle on a stand with 60 health, and it **never runs
out of shots** — nothing in this game carries ammunition, so there is
nothing to reload and nothing to walk over and refill. It fires at the
nearest enemy it can see in range through the very same trigger and hit
roll a Bim shoots with, the enemy's nearest-target rule includes it,
their hits drain it, and at nothing it is shot to pieces. **The charges
are the limit**: with one charge only one sentry stands, and setting a
second up destroys the first rather than being refused — so a sentry is
moved about the deck by laying another one where you want it. The row
beside one on the **Nearby** strip packs it up into the engineer's pack.
On the ship's deck a deployable stays
wherever the ship goes; on a station's deck it is lost when the ship
leaves. The **armourer** has a **Repair** button on
the workbench window: a damaged piece in the first slot and a hundred
euros out of the pool, ten minutes at the bench worked by that engineer alone, and
the piece back out with ten points on it, up to its full health.

### The soldier

The second class (feature 75): a line held, and grenades. A soldier sets
out with a basic **auto rifle** in hand, the laser pistol in the pack
beside it, and **two grenades**. Its experience is the shared rules'
alone — it has no source of its own.

**Brace** is `E`, from the first level: a toggle. Braced, the soldier
holds where it stands — whatever it was on put down, its walk dropped,
no errand taken up, under arms — never runs from a fight however badly
hurt, shoots at **1.15** the odds, and takes cover from what is round it
as usual. Four heavy brackets round the body say so on the deck, and the
crew panel says *Braced*. It ends when `E` is pressed again, when the
soldier is ordered anywhere, or when it goes down.

**Grenades** are `Q`, from the third level, at the deck tile under the
pointer: within **8 tiles** with nothing opaque between (walls and shut
doors stop a throw; sandbags do not), no sooner than **5 seconds** after
the last, one out of the pack at once. Holding `Q` draws the burst's
radius round the tile in the caution colour, or the warning colour
where the throw would be refused. The grenade flies to the tile and lies
there with its fuse blinking, and **2 seconds** after the throw it
bursts on everything within **2.5 tiles** of the tile that has a line to
it — walls and shut doors stop the burst, sandbags do not: every crew
member, hire and enemy alike, the thrower included, takes **40** at the
centre falling in a straight line to half at the edge, on one part
rolled the way a bolt's is, through that part's armour, as a strike
rather than a cut — halved for a body in cover from the burst's side —
and the blood is thrown over the deck as a cut throws it; a sentry in
it takes the same off its health; laid sandbags in it are destroyed;
the parts of the ship and the station are untouched. Enemies neither
throw nor dodge grenades. A grenade is a **charge**, like an engineer's
kit (feature 90): two of them, each coming back into the pack thirty
seconds after it is thrown, and nobody makes one, sells one or buys one.
Only a soldier throws. The crew panel says how many are in the pack and
how long until the next throw.

| level | left | right |
| --- | --- | --- |
| 1 | **Brace** | — |
| 2 | **Marksman** — accuracy ×1.15 | **Point blank** — damage within the weapon's sweet range ×1.2 |
| 3 | **Grenades** — may throw them | — |
| 4 | **Runner** — pace ×1.2 while an enemy is in sight | **Steady aim** — the walking penalty halved: three quarters of the odds on the move, not half |
| 5 | **Iron nerve** — never flees | **Cover master** — the odds of dodging in cover ×1.5 |
| 6 | **Long throw** — grenade range ×1.5 | **Short fuse** — fuse ×0.5 |
| 7 | **Drill** — fire rate ×1.2 on every weapon | — |
| 8 | **Frag** — burst radius ×1.5 | **Quick draw** — cooldown ×0.5 |
| 9 | **Bruiser** — melee damage ×1.5, fists and schword | **Dug in** — dodge +10% while braced |
| 10 | **Deadeye** — every weapon's far accuracy is its near | **Rampage** — each enemy downed raises the fire rate ×1.1, up to three, until the fight ends |

Every talent applies to the soldier who holds it alone, with whatever
weapon it carries, and all of them go through the one shooter every
Bim and every sentry fires with: the odds are the weapon's through the
soldier's skill, the walking odds its own, the point-blank factor rides
on the bolt to where it lands, the cover odds and the dodge are read
where a bolt reaches the body, and a blow's damage is multiplied as it
is swung. A *rampage* ends — its stacks gone — when the rooms unjoin or
no enemy is standing in the room.

### The medic

The third class (feature 76): a crewmate held up, and a shield over the
pair of them. A medic sets out with the **laser pistol** in hand as
everybody does, and carries **four medkits and ten bandages** where
anybody else carries one and five, each coming back on the same cooldown. Its
own source of experience is the only one any class has: **5** every time
it finishes bandaging a crewmate or treating a crewmate's trauma with a
medkit — a crewmate being any crew member but itself, mercenaries
included, counted when the task finishes and the bandage or the kit is
used, once a task. An interrupted task, a bandage on itself and anybody
who is not a medic doing the same give nothing.

**The heal beam** is `E`, from the first level, on the crew member under
the pointer: a player's Bim or a mercenary, never an enemy and never
itself, within **6 tiles** and in the medic's line of sight. Pressed on
the one it already holds, or on nothing, it unlinks. While it is linked
the patient's open wounds and untreated traumas **do not bleed** and its
blood comes back at **30 an hour** to full — so a patient out cold wakes
when the blood passes the line under the ordinary rule — and the medic
may walk but **fires nothing**. The beam *holds* a patient; it does not
cure one: wounds stay open until bandaged, a trauma stays until a medkit
treats it, and a part at nothing stays at nothing. It breaks when the
patient leaves the range or the medic's sight, dies or leaves the room;
when the medic goes down, is ordered to an errand — a plain walk keeps
it — or unlinks. A line in the beam's green is drawn between the two on
the deck, and the crew panel says who is held.

**The surge** is `Q`, from the third level. The charge fills while the
beam is on a patient that wants holding — under full blood, or with a
wound open — and is full after **40 minutes** of such beaming; it keeps
across fights and is lost only on the medic's death. Triggered with the
beam linked and the charge full, for **8 minutes** the medic and every
linked patient **take nothing from any hit**: no wound, no armour
drained, no trauma, the whole of it absorbed. The charge empties;
unlinking does not end a surge already running on the patient. A ring
round the body says who is surging.

**The carry** is `G`, from the first level and with no talent behind it
(feature 86): the crewmate under the pointer picked up into the medic's
arms — out cold, in a dying state, or bleeding — and carried out of the
fire. With the pointer on nobody it takes up the nearest it could, so
the key is worth pressing in the middle of a fight without aiming it.
Carrying, the medic walks at **six tenths** of its pace and **fires
nothing**: both its hands are the carry. `G` again sets the body down
where it stands, and treating it there is what comes next — a crewmate
is doctored only in the calm, so carrying somebody somewhere quiet is
also what makes treating them possible. A body in somebody's arms walks
nowhere of its own, and the carry is let go the moment either of the two
goes down. A crew member on its feet and whole is nobody's to carry: the
key is for getting somebody *out*, not for moving the crew about.

| level | left | right |
| --- | --- | --- |
| 1 | **Heal beam**; **carry** | — |
| 2 | **Field dressing** — bandages in half the time | **Surgeon** — treats in half the time |
| 3 | **Surge** — may trigger it | — |
| 4 | **Long beam** — beam range ×1.5 | **Strong beam** — beam blood rate ×1.5 |
| 5 | **Clean hands** — a trauma it treats leaves nothing lasting | **Steady hands** — a part it treats comes back to half again as far |
| 6 | **Quick charge** — the surge charges ×1.5 faster | **Long surge** — a surge lasts ×1.5 |
| 7 | **Mender** — a beamed patient's parts mend ten times as fast | — |
| 8 | **Self-care** — its own wounds do not bleed while it beams | **Double link** — the beam holds two at once, each at the full rate |
| 9 | **Gunner medic** — fires while beaming, at half the rate | **Closing surge** — a surge ending closes every open wound on the patient |
| 10 | **Mass surge** — a surge covers every crew member within 3 tiles of the patient | **Field surgeon** — once a fight, treats a trauma with no medkit in half the time |

Every talent applies to the medic who holds it alone: a crewmate
bandaging the medic takes the ordinary ten minutes whatever the medic
has learnt. *Mender* mends only the parts that are above nothing — a
part at nothing waits for a medkit like anybody else's. *Double link*
fills the charge off either patient and a surge covers both; a third
patient takes the first's place. *Closing surge* closes wounds without
giving any experience: only a finished bandage or medkit task does.
*Field surgeon* comes back when the fight ends — the rooms unjoined, or
no enemy standing in the room — like a soldier's *rampage*.

### The tank

The fourth class (feature 77): a wall the crew stand behind, and the
enemy's fire drawn onto himself. A tank sets out with the laser pistol
everybody does and a basic **helm, kevlar and leg guards** on — his kit
comes with him and costs the hold nothing. His own source of experience
is being shot at: every enemy shot or blow that **lands** on him is a
fifth of a point, counted five for one, and the count starts again at
every point. A hit counts after the roll and any dodge, whether his
armour, a surge or his body took it; a miss, a dodge, and a hit from his
own side — his soldier's grenade — count for nothing, and nobody but a
tank gains anything from being hit.

**His armour drains at half rate.** A piece's protection comes off a hit
as it does for anybody, and what gets past it drains the piece at half
the rate, so the same kevlar absorbs twice as much on him before it
breaks — a kevlar with 20 left takes 40 on a tank and 20 on anyone else,
and what the piece cannot take reaches the body exactly as before. The
piece's stored health is never doubled: it moves between crew members
unchanged.

**Bulwark** is `E`, from the first level: a toggle. With the wall up he
walks at **half pace**, and a crew member within **1.5 tiles** of him
that he stands between and the shooter — nearer the shooter than the
target is, and within 1.5 tiles of the line the shot travels — is **in
cover** against it and dodges it half the time, exactly as behind
sandbags. It is his own side's shelter and nobody else's: an enemy never
takes cover behind him. A ring of shield round him says the wall is up,
and the crew panel says so. It ends when `E` is pressed again or when he
goes down.

**Taunt** is `Q`, from the third level. For **6 minutes** every enemy
within **10 tiles** that can see him, with him inside its weapon's
reach, fires at **him** before any nearer target; melee chargers are
unmoved by it until *magnet*. The next taunt waits **20 seconds** of the
clock from the last. A dashed ring in the warning colour shows how far
it reaches while it runs, and the crew panel counts the minutes left and
then the cooldown.

| level | left | right |
| --- | --- | --- |
| 1 | **Bulwark**; armour drains at half rate on him | — |
| 2 | **Plated** — armour protection ×1.5 on him, given outright | — |
| 3 | **Taunt** — may use it | — |
| 4 | **Breacher** — forces locked doors in half the time | **Unmovable** — never flees, and loses no pace to low blood while his kevlar holds |
| 5 | **Wide wall** — bulwark reach ×2 | **Fast wall** — bulwark pace ×1.5 |
| 6 | **Loud taunt** — taunt radius ×1.5 | **Long taunt** — a taunt lasts ×1.5 |
| 7 | **Iron frame** — a hit rolled on his head lands on his body | — |
| 8 | **Hold fast** — his wounds and traumas do not bleed while he taunts | **Guarded** — dodge +10% while the wall is up |
| 9 | **Interpose** — a bolt that would hit somebody the wall shelters hits him | **Magnet** — a taunt turns every charging blade within its reach toward him |
| 10 | **Fortress** — armour drain ×0.5 again, a quarter in all | **Rallying wall** — while he taunts, every crew member within 3 tiles drains armour at half rate too |

Every talent applies to the tank who holds it alone, *rallying wall*
being the one that reaches past him. *Interpose* resolves the redirected
bolt against him as a fresh hit, armour and all, and it counts towards
his experience; one he slips is gone rather than rerolled onto the crew
member he shielded. *Unmovable* is the blood's halving alone: the legs
he has lost and what a trauma costs him still tell.

### The commander

The fifth class (feature 78): everybody near him fights better, and the
crew nobody is steering take his orders. A commander sets out with the
laser pistol everybody does and nothing else. His own source of
experience is **hiring**: a hire that goes through from the slot
steering him is **10**, whether it is a mercenary at the dock or any
other hire later. A refused hire is nothing, and a hire somebody else
sends is nothing to him however near he stands.

**He does two different things, and they reach different Bims.**

**His aura and his rally lift every friendly Bim near him**, a player's
own steered Bim as readily as a bot or a hired hand — never an enemy,
and never himself.

**His squad orders command only the squad**: every crew member **no
player is steering**, the crew's own bots and the hired hands alike.
They never move, hold or aim a Bim a player steers, and a Bim a player
starts steering leaves the order at once. Every player keeps every order
they have today: during the alarm anybody can still click a crewmate or
a mercenary and send it somewhere, and a click like that takes that one
out of the squad order until the next.

**The aura** is on from the first level and needs no key. While he is
conscious, every friendly Bim within **8 tiles** of him works a tenth
faster, shoots a tenth straighter, and holds its ground for a while
before it runs from a fight — where a dying body with no commander near
it runs at once. Two commanders' auras never stack: a Bim takes the
strongest one reaching it and no product of the two. A faint ring round
him shows how far it reaches and a small ring marks every Bim in it.

**Hiring is cheaper.** When the slot sending a hire is steering a
commander fit to act, the mercenary's fee is **a quarter off**, rounded
down to whole euros, and that is the fee written into the contract for
the whole engagement — it stays that hand's price whatever happens to
the commander afterwards. Dismissal still refunds nothing. The hire
window shows the discounted fee while a commander is steered.

**The three squad orders** work with the alarm and without it, so a crew
can fall back to the airlock before the machines are in sight. Each
reaches every squad member within **20 tiles** of him — the whole room
from the seventh level — and a member under one is in combat mode like a
recruited Bim: weapon drawn, errands stopped.

- **Attack** (`E`) on the enemy under the pointer: every squad member
  fires at that enemy ahead of any nearer target, and advances on it the
  way a crew member who has seen an enemy for itself does — to cover
  within range, peeking round it. The mark ends when that enemy is down
  or dead.
- **Fall back** (`X`) to the deck tile under the pointer, or to the
  commander himself when the pointer is on nothing: every squad member
  walks to a slot round that point — the same ring the crew gather in —
  holding its fire while it walks, then holds there and shoots what it
  can see.
- **Stand ground** (`Z`): every squad member holds exactly where it
  stands, shooting what it can see, never walking to cover and never
  running.

An order lasts until he gives another, gives the same one again (which
lets the squad go), goes down or dies; it is called off by the rooms
unjoining and by anything that moves the crew's indices — a hire, a
dismissal. A bracket over each squad member says it is under one, with a
thread to the enemy it was sent at or the tile it was called back to.

**Rally** is `Q`, from the third level. For **6 minutes** every friendly
Bim in his aura — a player's own included — shoots at ×1.3 and does not
run at all. It stacks with the aura; two rallies do not stack with each
other. The next rally waits **30 seconds** of the clock from the last,
and the crew panel counts the minutes left and then the cooldown.

| level | left | right |
| --- | --- | --- |
| 1 | the **aura**; hires at a quarter off; **Attack**, **Fall back**, **Stand ground** | — |
| 2 | **Wide presence** — aura radius ×1.5 | **Strong presence** — each aura bonus ×1.5 (a tenth becomes three twentieths) |
| 3 | **Rally** — may call it | — |
| 4 | **Haggler** — hires at two fifths off | **Outfitter** — a mercenary he hires arrives with one basic piece it lacked, free |
| 5 | **Focus fire** — the squad's odds against the enemy it attacks ×1.15 | **Pincer** — an attack may mark two enemies, the squad split between them |
| 6 | **Long rally** — a rally lasts ×1.5 | **Quick rally** — the rally cooldown ×0.5 |
| 7 | **Long reach** — a squad order reaches every squad member in the room | — |
| 8 | **Steady ranks** — Bims in his aura bleed ×0.75 | **Double time** — Bims in his aura walk at pace ×1.1 |
| 9 | **Relentless** — an attack's mark lasts until the enemy is dead, not merely down, and then the attack moves on to the enemy standing nearest the commander | **Grit** — during a rally, Bims in it lose no pace to wounds or traumas |
| 10 | **Anchor** — the aura's bonuses double while he stands still | **Warcry** — a rally covers every friendly Bim in the room |

Every talent applies to the commander who holds it alone. The **aura and
rally talents reach every friendly Bim the aura or the rally reaches**,
a player's own steered Bim included; the **squad talents reach only the
squad**. *Outfitter*'s piece is the lowest basic one the mercenary is
missing — helm, then kevlar, then leg guards — made for it at the hire,
and the fee is not raised for it.

## The simulation

`nix run .#simulation` is the game without the front of it: it opens the
world at once, for one player, on the **playtest ship** —
`shipdesign::playtest_ship()`, which is the default ship every run sails,
a twenty-tile hull laid out with one of everything the old game's crew of
one needed to live and to fly. It is laid out the way a small ship would
be: a bow cut back to a point in diagonal hull, with the bridge in it —
the helm on the centreline, life support and a battery in the corners the
cut leaves; the main deck amidships, the galley along the bridge bulkhead
to port with the table under it, the bay in the middle, the bunk against
the starboard skin and the airlock behind it; and engineering aft — the
tank and the reactor down the port side, a shelf of stores, the heads,
and the main engine set into the stern so its bell is the stern. Three
compartments, two bulkheads with a two-tile doorway each, a run of conduit
from the reactor to the helm, and thrusters in the skins fore and aft.
The helm, the galley, the bunk, the bay and the heads are pictures now,
as they are on every ship. Every doorway and every gangway is two tiles
wide on purpose: the room's navigation cannot walk a one-tile gap. It has
`SIMULATION_MONEY` (a placeholder €50 000) in hand. It is for playtesting
the world quickly, and it is the one place the old "lowest star with a
station" spawn survives: the world is the fixed default seed's, a two-arm
spiral, docked at that system's first station. `nix run .#test` is much
the same somewhere else each time — the combat ship, which is the
playtest ship with bunks and chairs for five, a random seed, and a dock
somebody lives on picked at random across that galaxy.

The ship's part count and `design_hash` are pinned — `PLAYTEST_PARTS` and
`PLAYTEST_HASH` — and checked like the reference design's, so the simulation
opens on the same ship on every machine.

## The crew

Every player steers one Bim, and the rest of the crew are bots. Their
names are written over their heads on the deck and their portraits are
at the top left, and a crewmate you have selected has its health and its
crew sheet down the right — a click, a portrait or a drag to select,
right-click the deck to move, `r` to recruit, and the tray at the bottom
left ([The HUD](#the-hud)). The panels are one
module, `crates/app/src/crew.rs`. In the simulation the crew is one Bim,
James; in `droids` it is sixteen.

They are not two kinds of thing. A player's Bim and a bot are the same
body on the same clock, and nothing in the simulation tells them apart but
an index and whether anybody is steering it. The one asymmetry is the
player: **every order you give goes to the Bim you steer**. Selection, the
right-click move order, recruiting, and every row on every menu act on it
and only it — with the exceptions a fight makes: while the alarm is up a
crewmate you click takes your move orders too (see [Combat mode, and the
inventory](#combat-mode-and-the-inventory)), **F** and **T** are orders to
every bot that follows you, and a commander's squad orders move the bots
nobody steers (*The commander*).

A Bim's name, coverall and hair are the app's business and the drawing's; the
simulation knows crew member 0 and crew member 1. No strings come out of the
room, so the names are written over the deck by the app after the shape
buffer has been replayed, not carried across in it.

### Picking one, and the crew sheet

**Clicking a Bim selects it, and selecting is what puts its panels on the
right-hand side**: its health, and a character sheet under it. One at a
time, and nothing at all when nothing is picked — the right-hand side
answers *who am I looking at*, not *what is everybody up to*. Your own
Bim, which is picked from the start, has no panel there: it is the hero
panel's and the character sheet's (feature 107).

**Selecting is looking at, not taking charge of.** Any of them can be
picked; in peace only your own takes your orders. Click a crewmate,
right-click the floor, and nothing happens — which is the same answer as
before, arrived at more visibly.

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
- **Memory** — the Bim's own account of the crew it has lost.

### What a Bim remembers

**A crewmate's death, and nothing else.** Newest at the top, and a crew
that has lost nobody leaves the page **blank** — it says *"Nothing has
gone wrong."* and that is the good outcome rather than a panel that
failed to load.

It used to keep a great deal more: the day's work at first, then only
what went wrong — an accident, being sick, dropping off standing up, each
stage further into hunger or sleeplessness, the low moments of a Bim
nobody had spoken to — and what it saw happen to the others near it. All
of that was the needs', and went with them (feature 104). What is left is
the one entry that was always written into every surviving diary wherever
they happened to be standing, because it is the one thing aboard nobody
could fail to notice: *"Kate died today."* A Bim does not record its own
end; there would be nobody to read it back.

No strings come out of the room, here as everywhere. An entry is a day, a
time, a code and one number — which of the crew died — and `memory_line`
in `crates/app/src/names.rs` is where the sentence lives. The book is
bounded — a few hundred entries, oldest falling off the front — so a game
left running does not grow without end.

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

## Controls

| | |
| --- | --- |
| Click the armoury or a shelf, or pick the cold store on the **Nearby** strip | Its grid — the lockers, the shelves, the food — and the inventory of the Bim shown beside it; the Bim walks over |
| `Tab` | The inventory of the Bim you steer, and with it the nearest thing within reach — a container or a body down — with the rest of them a click away on the **Nearby** strip over it |
| Right-click a cell | **Take** · **Store** · **Equip** · **Discard**, or **Unequip** on a worn slot — greyed with the reason when it cannot go |
| Ctrl-click a cell | The quick move: container to pack, pack to the open container |
| Click or right-click a door | Menu: hold open, close, lock, unlock — the Bim walks to the panel |
| Right-click a Bim | Menu: a bandage and a treatment for each part, greyed with the reason; **Loot** on a body that is down |
| `1` | Select the Bim you steer (control group 1) |
| `r` | Recruit it, or let it go — see below |
| `q` / `e` | The steered crew member's **class actions**: an engineer **sets up a sentry** / **lays sandbags** on the deck tile under the pointer, out of a charge in its pack; a soldier **throws a grenade** at it (hold `q` to see the burst's radius) / **braces** where it stands, or stands easy; a medic **triggers its surge** / **beams the crew member under the pointer**, and unlinks when pressed on the one it holds or on nobody; a tank **taunts** / **puts its wall up**, or takes it down — see [Classes and levels](#classes-and-levels). The log says why not; nothing with a classless crew member |
| `f` | **Attack**: the pointer turns into a red crosshair, and the next click on the deck plants an **attack banner** there. The crew nobody steers fight their way to it — taking the cover on the way, pushing on when nothing is in range — and hold it. `f` again, `Esc` or a right-click puts the crosshair away; the banner clicked where it already stands calls it off |
| `t` | **Retreat**: the crew nobody steers fall back to the ship and hold there. `t` again and they go back to keeping to your side. Nobody leaves a fight *aboard* the ship — cornered in your own hull they stand and shoot whatever they were told |
| `m` | The world map; during a mission it is only looked at |
| Drag a box over one | Select it. Selecting shows its crew sheet |
| Click one | Select it — a click is just a box of no size. Only the Bim you steer takes your orders |
| Click empty floor / `Esc` | Deselect — the right-hand panels go with it. `Esc` first shuts whatever is up, innermost first: a cell's rows, a menu, a grid window |
| Right-click the floor | Send the selection there — opens a door on the way if it must |
| **Shift** with a right-click, a right-drag or a menu row | The order **waits its turn** behind what the Bim is on and whatever was queued before it, the way RimWorld queues them (feature 69): the queued walks are drawn on the deck as a dashed thread with a pip at each spot, and the rest show on the agenda. A plain order afterwards calls the queue off. What cannot be begun when its turn comes — a bandage with no dressing left to wind — is dropped without a word; a walk with nowhere to go is refused at the click |
| Tray, bottom left | **Work** — which jobs come first; **Management** — what the crew keep doing of their own accord; **Inventory**, **Research** and **Skills**; on the ship, **Build** — lay parts out for the crew to build, where the shipyard is on |
| Point at anything | Top left says what it is |
| Point at a row that names a place | The place is ringed on the deck |
| Speed buttons, top left | Pause, and 1× up to 48× |
| Rest on an underlined word or a ? | It explains itself, after a third of a second |

Nothing on the page explains itself in prose any more. The explanations are in
tooltips, and a tooltip is always *asked for* rather than stumbled into. Two
rules, and nothing else pops anything up:

- **An underlined word.** Where the thing already has a name — Health,
  "Speed" — the name carries it, dotted-underlined and with the help cursor.
  Hovering the row, the bar or the panel does nothing.
- **A "?".** Where there is no word of its own to underline, a small question
  mark sits beside the controls — beside a block of rows, say, that no one
  word belongs to.

Either opens after the pointer has rested on it for 300ms. The delay is the
point: crossing a panel on the way somewhere else sets nothing off. The ring a
row draws round the place it names is not one of these and needs no
affordance — nothing pops up, nothing is said, and it is gone the instant the
pointer moves on.

Either button opens a fixture's menu, and doing so leaves the selection alone;
a *sweep* across one is still a marquee. Right-clicking bare floor is still a
move order — the hit test decides which of the two a right-click meant. Nothing
is greyed out for being busy any more: a new errand takes over and the old one
goes on the agenda (below). Items are only ever greyed out for a reason of their
own — a locked door, a pack with no room left in it.

## Nothing is remote

Every menu item is an errand, including the ones that look like switches.
Asking to hold a door open, close it, lock it or unlock it sends the Bim
walking to that door's panel; the state changes at the moment its hand
arrives and not before. There is no way to reach into the room from outside
it.

That falls out of one shared pair of steps — walk to it, put a hand on it —
carrying which switch on the task rather than in the step, so a switch errand
queues, resumes and shows up on the agenda exactly like any other.

Where a door has two sides, the Bim goes to whichever panel it is nearest,
so it can let itself out as readily as in.

## Interrupting, and getting back to it

Send a selected Bim somewhere, or start it on something else, and the new thing
wins immediately. What it was doing is not thrown away: it goes on a queue with
its progress intact, and the Bim picks it up again as soon as it is free — after
walking to wherever it was sent, or after the errand that displaced it.

The queue is a stack. Each interruption goes on the *front*, so a chain always
resumes directly after whatever displaced it: interrupt a dressing with a walk
and the walk with a door to unlock, and the order back out is the door, the
walk, the dressing.

**Hold Shift and the new thing waits instead** (feature 69). A right-click on
the deck, a right-drag for a line, a row of a fixture's menu, a bandage on the
crew sheet, a gun picked up off the deck — given with Shift held, any of them
goes on the *back* of the same queue, behind what the Bim is on and whatever
was queued before it, the way RimWorld queues orders. The walks are drawn on
the deck as a dashed thread from where the Bim is bound now through every spot
in turn, a pip at each, until they are walked; the rest sit on the agenda under
their own names. When its turn comes an order is *begun* the way the row would
have begun it, against the room as it is then: a bandage queued behind a walk
finds the pack empty and is dropped without a word, the way the greyed row
would have refused it. A plain order — anything given without the key — calls
the queue off, though what the Bim had put down of its own is kept to be picked
up. Shift on a greyed row does nothing: what cannot be ordered now cannot be
queued either.

Resuming is the part with a trap in it. Standing steps assume the Bim is
already in the right place — a pair of hands on a wound works on whatever is in
front of them — so dropping the Bim back on the step it was interrupted at
would have it working on thin air in the middle of the floor. Instead, resuming
rewinds to the most recent *walking* step of that chain and lets the Bim walk
back first, then drops into the interrupted step with the time it had already
put in. The steps skipped on the way in are exactly the ones whose work the
room is already holding. What the room cannot hold is what was in the Bim's
hands, so that is snapshotted too, and put back after the walk.

### The agenda

The panel over the top-left of the deck is the checklist for all of this. One
row per chain — the one running first, then the queue in the order they come
back — each with a progress bar at the left-hand end, a tick box that is filled
for the chain actually running and empty for one that is waiting, and the name.
A queued row keeps the progress it had when it was put down, so you can see a
dressing sitting at 32% while the walk it was interrupted by runs at the top.
The panel hides itself when there is nothing on.

## The Management tab

> **Gone from the HUD** (feature 107), with the Work, Build, Research,
> View and Ship tabs beside it: the crew keep to what they were left on —
> autonomy on, gear not combined by itself, nothing kept made, every job
> at 3, nothing queued for research — and what follows is what the tab
> did while it was there.

**Management** in the tray is what the crew keep doing of their own
accord, and three things are on it: the **autonomy** box, whether the
crew take work up by themselves when they have nothing on; **Combine
matching gear**, whether they carry two of a kind at one tier to the
workbench and the better one back without being told (see *Armour*,
under *What the ship will make*); and the **crafts**, a row for each
thing a bench aboard can make — how many there are, where they are kept,
and how many to *keep made* — which is how the drug lab is asked for
medkits (*Making things*). The food, the stew and the dressings each crew
member kept in its pack were rows here too, until the needs went and the
medicine became everybody's charges.

## Work priorities

The **Work** tab in the tray is the answer to "what should they do first".
One row per job, a number from **1 to 5** in a box beside each, and **1 is
done first**. Click a box and it steps one less important, from 5 back round
to 1. Everything starts at **3**, all equal, so out of the box this changes
nothing at all, which is deliberate: a default that is already an opinion is
a default you have to undo before you can use anything.

| | |
| --- | --- |
| **Hauling** | carrying gear between the lockers and the workbench |
| **Making things** | working a bench the crafts table has an order for |
| **Building** | putting a laid-out part together, once the crew can pay for it |
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

Resting on a row rings the places the job is done at — **every one of that
kind**, so a ship with two benches sees both rung. Building and medical ring
nothing: a site is wherever it was laid out and a patient wherever it fell.
Nothing pops up and nothing is said: a highlight is not a tooltip (see *What
the pointer is over*).

**Medical is the one row that can interrupt.** It is on offer while somebody
aboard is dying and there is a medkit to hand — the nearest crewmate in a
dying state, its worst trauma first; never the Bim's own, since nobody
treats their own — or bleeding and there is a bandage to hand — the Bim's
own wounds first, else the crewmate with the most open, and that one's
worst part — and at any number from 2 to 5 it waits its turn like the rest.
Set it to **1** and it is urgent: a Bim drops whatever it is on the moment
there is a wound to dress, the errand going onto the queue the way any
interruption would push it, and picks it back up when the hands come off.
Set it to **never** and nobody doctors of their own accord; your own
bandage orders from the inventory or the menu on a body still work. A
patient nobody can walk to, or one somebody is already walking over to
dress, is not offered. A patient somebody is walking over to holds still
for them.

### What it actually changes

Only what a Bim takes on **of its own accord**. A Bim with nothing on looks
at whatever work is going — a wound to dress, a site to build, a gun to
carry to the bench — and does the one nearest the top of the list.

**Hauling is gear, and nothing else.** A gun or a piece of armour carried
from the lockers to the workbench, and the upgraded one carried back (see
*Armour* under *What the ship will make*), is offered and taken at
hauling's own number. It was a harvest carried to the cold store as well
while there was a bay to harvest, and materials to a construction site
until the money rework: there are none to carry.

## What the pointer is over

Top left, above the agenda, one line says **what is under the pointer**:
deck plating, a bulkhead, a door, a bench, the lockers, a bunk, the cold
store, the table — and *outside the hull* if the pointer is off the ship
altogether. Where there is something worth saying about the state of it,
it says that too: a door **locked**, **held open**, **open** or **shut**.
It does not name what is lying on a tile: the blood on the deck is drawn
and not named, so a stained tile reads as the deck it lies on.

The readout is a different question from a click, and answers accordingly. A
click asks *what would this act on*, which is why only the few things with a
menu behind them are hit-testable and why each of those is given a few pixels of
slack. The readout asks *what is this*, so everything aboard has a name and
nothing is expanded: a pixel beside a bench is deck, not bench.

**A highlight is not a tooltip.** Resting on a row of a panel that names a
place — a Work row, a fixture's — rings that place on the deck, in the
ship's own cyan rather than the green that means "selected" or the warm
colours that mean trouble. It does not go through the tooltip machinery:
nothing pops up, nothing is said, and a pointer crossing the panel on its
way somewhere else lights a fixture for a moment and leaves nothing behind.
The ring is worked out afresh every frame from what is hovered, so a panel
folding away under the pointer cannot leave one lit.

## Recruiting

Press **r** and the Bim is under direct orders. It stops doing anything of its
own accord: no work taken up, nothing picked back up off the queue, and none
of the pottering about it does between jobs. It stands where it is put until
told otherwise. Press **r** again to let it go.

Everything the player asks still works: move orders, menus, all of it.
Whatever it was in the middle of when recruited is left to finish rather than
cancelled, since throwing away a half-dressed wound for tidiness would be
worse, and a right click interrupts it anyway. The queue is kept rather than
emptied, so letting the Bim go picks up where it left off.

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
when one of them **sees an enemy for itself** does it go and fight for
itself — walking to cover within range, peeking round it — so a squad
does not charge in headfirst after something only the log knows about.
**And while the alarm is up you can order them**: click a crewmate to
select it and right-click the deck, the way you send your own; it goes
and holds that spot, shooting from it, until the alarm is over. In peace
a crewmate takes no orders, as before. **A commander's squad orders sit
beside that and never replace it**: they command the crew nobody is
steering — attack, fall back, stand ground — and never move, hold or aim
a Bim a player steers; a player's own click order to one of the squad
takes that one out of the order until the next. The alarm lasts until
nobody is near, nobody has seen an enemy and nobody has been hit for half
a minute, when they go back to their work, however many machines are
still standing somewhere on the station. (A crew member that bled past
the line in the fight stays **out cold** on the deck until
somebody bandages it; see *Getting hurt*.) A hired mercenary does the
same. The Bim you steer is yours alone: recruit it yourself or leave it
to its work; the alarm never touches it.

A recruited Bim is in **combat mode**: it draws its weapon — held in both
hands, out in front — and, whenever an enemy is in range and in its own
line of sight — the peek round a wall included — it fires. Everybody
carries a **hand laser pistol** from the start; the other four guns are
bought, and a Bim swaps to one out of its pack. It does nothing else about
the enemy: it stands where it was put, as a recruited Bim does, and shoots
from there; walking it somewhere is still yours to order, and it holds its
fire while it walks. Let it go and the weapon is holstered. Your own
Bim is heard drawing and holstering — the same recording, the holstering
slowed a little — and nobody else is.

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
blood over the tiles round the body. **Every enemy is a machine**, ringed
in red, with no blood to bleed and nothing to loot, and what each of the
three does is *The fight*, under *What the ship will make*: a Trooper's
forearm is a gun, whose shots fly in the crew's room in red and land on
the crew's own bodies; a Husk's claws are a blow at arm's length; and a
Warden's Unmaker takes armour off rather than blood. A hit on a crew
member is on the head, the body or the legs, and what it does is
[Getting hurt](#getting-hurt).

Two things a corridor fight turns on. **Peeking exposes the peek.** A Bim
that aims from the peek beside a wall leans out to it, and that is where
the enemy shoots at — but it is in cover, and a bolt reaching a body that
is peeking is **dodged half the time** and flies on. A line of
**sandbags** is **low cover**: a body can walk over it and see over it,
but standing close behind it — the tile next to it, on the far side from
the shooter — half the bolts coming over it miss, the same as a peek; a
bot picks such a spot to shoot from, and every station's corridors have a
barricade of them; a ship can build them too, under Structure. **Walking
halves your aim.** Every gun shoots on the move at half its odds, so a
bot with a shot from where it stands stays put and takes it, and walks
only for cover or when it has no shot at all; a Bim you send across a
room fires as it goes, at half. And **a schword is a melee**: a Bim with
one is **locked** with anything within arm's reach and swings, and
walking out of reach ends it. The log says *locked in melee* the step it
happens. A crew member dying, treated or dead is said in the log, and so
is every hit.

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
deck; and under the weapon the **pack** on the Bim's back, a grid ten
cells across by five down with every thing in it over its footprint.
Beside each armour slot is the count of open wounds on that part of the
body and a **Bandage** button — see the next section — and under the
lot, how many dressings the Bim you steer has in its own pack. A box of
dressings is two cells by two and holds five, with the count in its
corner; right-click it for **Bandage all wounds**.

**Inventories are grids**, and every container aboard is looked at the
same way. Click the **armoury** (or the drug lab — either opens the
lockers) or a **shelf**, or pick the **cold store** on the Nearby strip,
and a window opens showing what the hold keeps in that class, laid out
as its grid: every thing over its footprint, a piece of armour with a
sliver of its health under it, and a stack with its count in the corner.
The window is the class, not the cupboard: a second shelf is the same
grid, and a helm put away across a shelf turns up in the lockers' window,
because the lockers are where the hold counts it. Clicking a container
also opens the inventory of the Bim shown beside it and walks that Bim
over, since nothing can be moved until it stands within **two tiles**.
**Right-click** a cell for the rows —
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

**A crewmate that is down can be looted** — dead or out cold; nobody on
their feet can be, and a machine carries nothing to take. Right-click the
body and the row is **Loot** (beside the bandage rows for a crewmate who
is only out cold). It walks your Bim over and opens the **Loot** window:
the body's pack, and under it a row of four — head, body, legs, and the
weapon in its hand — drawn like any container's grid, a worn piece with
the sliver of health it has left. **Take** on a right-click, or
ctrl-click, moves one thing into your Bim's pack, once it stands within
two tiles and has room; a piece comes off the body as it is, broken or
not, a weapon goes into the pack to be equipped from there, and the
stripped body draws bare. Nothing can be put onto a body. Down and reach
are checked when the command lands, not when the window opened: a
crewmate who comes round meanwhile is no longer a body — the window shuts
on them — and the command is refused *not down*.

## Getting hurt

A Bim's health is one bar of a hundred, and it always was; what is new is
that the bar is **three parts added up** — the **head** (5), the **body**
(75) and the **legs** (20) — and the panel shows the three under it, a thin
bar each, with a fourth for **blood**. Mending regrows the bar as a
whole, over all three in proportion; a shot is what tells them apart.

A shot lands on one part — one in twenty the head, three in four the body,
one in five the legs — and takes the weapon's damage *at the distance it
flew* off that part alone; a blow in a melee, a fist's twenty or a
schword's thirty-five, lands the same way, on a part rolled where it lands.
A part at nothing is not death any more: it is a **dying state**, rolled
for the part the moment it goes. The panel names it in red beside the
part's own bar at zero — *Head · 0 · Skull fracture* — the block above
says what it is costing in blood and how long that leaves it, and a
**red cross** stands over the body on the deck. The
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
does it too, whatever you told it; with the enemy gone it stops where it
is and waits for the medkit. **A crewmate that is merely hurt runs too** —
bleeding from a wound, or down to half its blood — rather than walking
up to the guns for a firing spot the way a whole one does: it gets out
of the enemy's sight and, out of it, **binds every wound it has** out of
its own pack — the same *Bandage all wounds* the box in the inventory
offers — and comes back into the fight dressed. It stops for anybody
you send over with a bandage or a kit, once they are nearly at it. Your
own Bim goes where you send it, hurt or not: only dying makes it run. And
a body **out cold
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
has to pick it back up when it comes round. A crewmate does that on its
own — the walk over and a moment bending
for it, *Picking a weapon up* on the agenda; your own waits to be told:
right-click the gun and the one row is **Pick up**, into the hand if it is
empty, else into the pack. A body that dies with its gun on the deck takes
it back, so looting the body finds it. Blood comes back on its own, over two days, once
nothing is open. It shows: a dark blotch on the head, the middle of the
coverall or the boots while that part has a wound open, **blood on the deck**
under a Bim that is bleeding — a drop every second or so a wound (below) —
the blood bar in red once it is low enough to slow it, and every hit and
every crew member down written in the log.

**Blood on the deck stays where it falls.** A drop stains the tile under
the body, so a Bim standing still with a wound open pools it in a few
drops and one walking leaves a trail, and a cut or
a grenade's burst throws it over the tiles round the body besides. Boots
carry it: a step off a bloodied tile onto the next has one chance in four
of taking a quarter of what was there along with it — moved, not copied,
so a trail thins as it lengthens and dies out a few steps on. It is drawn
in its own dark red, and nothing takes it up: the broom that did went with
the needs (feature 104), and so did every other kind of mess the deck
used to keep. It stayed because its rolls are on the room's one stream of
dice, which every fight draws from; see *The old game deleted* in
`CLAUDE.md`.

**The panel says what the Bim is dying of, and how long it has.** Under
the health bar is a framed block naming the thing that is taking it
down now: **Blood loss**, with what it is losing an hour, **how long
that leaves it** — the blood it has left divided by the rate, so it
moves the instant a bandage or a medkit does — and the list it is made
of, worst first, each with its own share: *Fractured femur · 40 an
hour*, *Legs · 2 open wounds · 20 an hour*. Under that is what stops
it. The block is graded the way the deck is: red, headed *Dying of*, for
a body in a dying state — the one a medkit is the answer to — and amber,
headed *Losing*, for one that is merely bleeding through wounds a bandage
closes, since a scratch that would empty it in ten hours is worth a
number rather than a fright. A part a trauma holds at nothing names that
trauma in its own row, beside the bar at zero. And a Bim that has died
says what of, read off the body, since that is how the game decides it
too. (Starving was the other thing the block could name, while there
was hunger.)

**A Bim in a dying state wears a red cross**, a white disc with a
medical cross on it, over its head on the deck. It is the one thing out
there that is not another coloured ring, and it means exactly one
thing: a part of that body is at nothing with its trauma untreated, and
only a crewmate with a medkit ends it. It goes when the medkit lands —
and it is never over a body that has died, since nothing can be done
for one.

**Armour is worn over all of that**, one piece a part — a helm, kevlar,
leg guards (*Armour* under *What the ship will make* says what each is
made of) — and it is drawn on the Bim: a steel-blue cap over the hair, a
dark plate over the coverall with the yoke still showing, darker boots
with a band across the shin. A piece has a **health** of its own that is
added to the part's — the body at 75 in a 20-health kevlar reads 95, and
the Bim's bar 100 → 120 — and it is drawn as a **blue bar on the end of
the green one**, on the crew sheet's big bar and on each part's own
alike — the big bar's number reading `100` with a blue `+20` beside it,
a part's `75 + 20`. A hit on that part goes to
the piece **first**: its **protection** comes off the damage before
anything else, so a shot that does no more than the protection does
nothing at all — no wound — and what is left drains the piece; only what
the piece cannot take reaches the body, and only that opens a wound. A
15-damage shot into a 20-health kevlar leaves the Bim untouched and the
kevlar at 5. At nothing the piece is **broken**: still worn, drawn with a
crack across it, doing nothing — its protection is lifted with its
health — and worth nothing put away, so it cannot be stored or sold, only
discarded and another bought. The log says when a piece breaks. What is
worn and what is in the pack go with the Bim and keep their damage
wherever they go: take a dented helm off and put it away, and it is a
dented helm in the lockers, with its health under it in the grid. Taking
"a helm" out of the lockers takes the least damaged one there; selling
one sells the most damaged.

A **bandage** is a thing a Bim carries, not a number on a shelf: a **box
of dressings** takes two cells by two of a pack and holds **five**, and
the one that gets used is the one in the pack of whoever winds it.

### Medkits and bandages are charges

Nobody fills a pack out of the hold any more, and nobody walks to a
cabinet for a kit. **Every crew member carries one medkit and five
bandages**, whatever its class — a **medic**, of the class or a hired
field medic, carries **four and ten** — and each one used **comes back
into the pack on its own**: a medkit a minute of the clock after it
was spent, a bandage thirty seconds after, one at a time, in a fight as
out of one. Two boxes at the foot of the screen, past a rule beside the
class's own, say how many your Bim has: the count sits on a disc in the
corner, and while the next is on its way a **ring** round the disc
fills clockwise; with none left the whole box goes dark and the dark
**sweeps back clockwise from twelve o'clock** as the wait runs out, with
the seconds in the middle — the way Dota 2 shows a skill coming back,
and the way every cooldown on that bar is drawn now. A medkit or a box
of dressings cannot be put in the hold — it would only come back — and
a crew member with an empty pack cannot bind or treat anything, however
many are in the lockers.

A bandage closes every wound on one part. There are three ways to order
one, and all of them go through the crew member you steer:

- **From the inventory** — the tab, or the pop-up that opens on
  recruiting — for whoever is shown: each armour slot's row says how many
  wounds are open on that part, and its **Bandage** button lights when
  there is one and a bandage to hand.
- **Right-click a Bim** on the deck, yourself or a crewmate, and the menu
  is the three parts — *Bandage the head · 2 wounds* — each greyed with a
  reason when there is nothing open there, no bandage in the pack, or your Bim
  is in no state to walk over, with **Bandage all wounds** under them. A crewmate out cold gets a **Loot** row
  under them as well; a dead one has only that — see [Combat
  mode](#combat-mode-and-the-inventory).
- **Right-click a box of dressings** in the inventory: **Bandage all
  wounds** dresses the worst part now and queues the rest behind it, one
  dressing a part. That is the same thing a Bim that has run out of a
  fight does for itself the moment nothing can see it — see the flight
  above.

Either way your Bim walks to the patient (a tile off, or nowhere if it is
dressing itself) and spends **ten minutes** with its hands on the part;
*Dressing a wound* is on its agenda, and the dressing comes out of its pack
when the ten minutes are up — provided the patient is still alive and
within reach, since a patient that walked off is ten minutes lost. A
bandage on a part with nothing open is refused outright as a waste. The crew
dress each other **on their own** as well — the **Medical** row on the Work tab
is that, and at priority 1 it interrupts whatever they are doing; see [Work
priorities](#work-priorities).

A **medkit** is the other thing, and the only way out of a dying state.
It is ordered the same two ways — a **Treat** button under the part's
Bandage one on the inventory, *Treat the head · skull fracture* on the
menu on a body — and the hands are your Bim's for a crewmate; for your
own, since **nobody treats their own**, the nearest crewmate that is free
walks over instead, and the row says who. Twenty minutes with hands on
the part, *Treating a trauma* on the agenda, the kit opened where the
helper stands and gone from its pack when the hands come off, and the
part starts again from half — a treatment given up before then keeps its
kit. The patient holds still while the helper walks over. The crew treat
each other on their own too — a dying
crewmate comes before any wound on the Medical row.

## Time

`crates/game/src/clock.rs` keeps the room's clock: minutes since midnight,
and which day it is, on the `time` crate's lengths. One real second is one
game minute at 1×, so at 24× a minute goes by in two and a half frames. In
a run the room's clock stands still with the world's — only travel moves
the day (see [the loop](#the-loop-world-map-travel-missions)) — and
everything inside a mission is timed on the world's **mission clock**
instead, which counts the steps since the crew arrived.

Nothing about time comes out of the room as a string: the app reads the
minute count and formats it. The light level comes from the same clock,
eased in at dawn and out at dusk, and is laid over the finished frame as
one tinted rectangle — so nothing in `room.rs` has to know what time it is.

## The look of it

Everything is drawn from the two primitives in `crates/game/src/draw.rs`, so "futuristic"
here is a matter of palette and of what gets a light on it rather than of any
new drawing machinery. The deck is dark blue-grey, the fittings are composite
panel in three shades, and one cyan running light is picked up by every powered
surface: the counter fascia, the coils etched into the hob, the rim of the
table, the jambs of a door. Warm colours are held back for trouble, which is
why a locked door reads instantly.

The first two of any crew are told apart by their hair and the yoke on their
coverall, not by where they happen to be standing: James has cropped hair and
a pale yoke, Kate a mauve one with hers worn long, which from directly above is
a second ellipse behind the head; everybody after them is dealt a build and a
look of their own (*The multiplayer seam*). The coverall is the ship's blue on
the crew and the station's orange on the station's people, so a joined deck
says at a glance who is going ashore when the crew leave. Their names are
written over them.

The page around it is arranged the same way: nothing is framed. The deck runs to
the edge of the window, the canvas carries no border of its own, and every panel
sits flush in a corner of it — the portraits and the top frame along the
top, a picked crewmate's health and crew sheet on the right, the hero panel
at the foot, the tray in the bottom-left corner ([The HUD](#the-hud)). A
rounded box drawn round the whole interface reads as a window frame inside
a window, which is one frame too many.

The tray is the exception to everything being small. It is set a size larger
than the rest — wider, and bigger type — because what is in it, the work
list and the two trees, is read and edited rather than glanced at.

## How it fits together

The simulation is entirely Rust, and so is the window. Each frame the app
steps whichever screen is up and asks it for a flat buffer of shapes; the
buffer is tessellated into triangles (`crates/app/src/shapes.rs`) and drawn
as Bevy meshes under egui's panels (feature 97), and the words — names over
heads, labels on the map, the readouts — go on after. The rules crates hand
over numbers and shapes and nothing else: no string leaves them, and no
sound either — the room says a door slid or a shot was fired as a *cue*,
and the app owns the recording — so the native server that will one day
run the same crates has nothing to say and nothing to disagree about.
`cargo build` is the whole pipeline.

### Thirteen crates

The repository is a cargo workspace and everything is under `crates/`. The
split is not tidiness — each line of it is something that has to give the same
answer in two places at once:

| Crate | What it is | Who else needs it |
| --- | --- | --- |
| `app` | The window: Bevy and egui, the screens, the pointer, the words. The game's one binary | — |
| `game` | The room: the simulation, and the shapes it draws itself as | `world`, which runs it aboard the ship and on every station's deck |
| `ship` | The design phase and the game it starts: `Session`, the editor, the two cameras, the painters | `app` |
| `lobby` | The World tab's galaxy: a camera over it, a pick, and the system diagram | `app` |
| `world` | One star system, the ship in it, the run, and the clocks they all run on | `ship`; the native server, which has to run the identical loop |
| `flight` | What a design does when you push it, and the closed-form plan the flown trip used | `world`, which quotes a run's trips off the ship's accelerations; `ship` and `app`, which read them |
| `shipdesign` | What a ship is made of and the rules for putting one together | `ship`, `flight` and `world`; the native server, which has to admit the same ships |
| `worldgen` | The galaxy, what is in each system, and station blueprints | The native server that will one day be authoritative, which has to generate the identical world from the same seed |
| `physics` | Ship mass, engine thrust, travel time. Pure arithmetic | `worldgen`, to check its layouts; `shipdesign` and `flight`, to weigh a real ship; `world`, to say how long a trip is |
| `economy` | Money: whole euros, the shared pool, what a station charges and which hold it goes in | `shipdesign`, `world` and `ship`; anything that ever charges for anything later |
| `time` | How long a minute, an hour and a day are | All of them — a day that is two lengths is two games |
| `wire` | What a client and the relay say to each other: rooms, codes, and bytes passed along | `app` and `server`, and nothing else |
| `server` | The relay, `bims-server`: it introduces the players and passes their bytes between them | Nobody — it links `wire` and nothing else of the workspace |

The `health` crate that was a fourteenth — a body's radiation dose, its
sickness and its cancer — went with the radiation (feature 104); what a
body is made of, its parts, its blood and its wounds, was always the
room's (`crates/game/src/health.rs`).

`cargo test --workspace` runs every crate's tests, and `nix flake check`
builds the app and runs them. `game` has tests of its own on a bare room
(`Game::bare`), with three native probes in `scratchpad/` beside them;
`./check smoke`
opens four windows for sixty frames each and keeps a picture of three of
them.

### Inside the room

Relative to `crates/game/src/`:

| File | What lives there |
| --- | --- |
| `lib.rs` | The module list, and why `time` is reached as `crate::time` |
| `game.rs` | Ties the room, the Bims and their running tasks together; input |
| `room.rs` | The room: its layout, its fixtures as solids, and how it is drawn |
| `health.rs` | The body's three parts, the blood, wounds, traumas and bandages |
| `blood.rs` | The blood on the deck: where it lies, and the boots that carry it |
| `fixtures.rs` | The fixtures the room only draws — the galley, the table and chairs, the bunks, the broom locker, the bays and fields, the heads — as a fresh room draws them, for the deck and the designer alike |
| `sight.rs` | What each Bim can see, traced on the tile grid, and the peek round a wall |
| `combat.rs` | Weapons, bolts, hits and shots; the enemy's choice of where to stand |
| `droid.rs` | The machines: their bodies, and how they fight |
| `routine.rs` | A station's people and the rounds their roles walk |
| `task.rs` | The scripted chains — a dressing, a treatment, a part put together, a gun carried to the bench |
| `clock.rs` | The time of day, and how much light there is — restated from the `time` crate as `f32`, not defined again |
| `character.rs` | The Bim — how it decides where to go, and how it is drawn |
| `draw.rs` | The shape buffer and the local frame used for sprites |
| `math.rs`, `rng.rs` | Vectors, rectangles, angles, and a PCG32 generator |

And in `crates/app/src/`:

| File | What lives there |
| --- | --- |
| `main.rs` | Every command, and the screen state machine |
| `screens/builder.rs` | The menu, the setup screen and the lobby, with the World tab |
| `screens/designer.rs` | The design phase, and the `Net` seam every edit goes through |
| `screens/game.rs` | The game: the deck, the map, the station |
| `screens/hud.rs` | The HUD (feature 107): the portraits, the top frame and its warning, the hero panel, the log |
| `screens/worldmap.rs` | The world map: where a trip can go, the vote on one, *Back to ship* |
| `crew.rs` | The crew's panels: the side panel, the character sheet, the tray, the menus on a door or a body |
| `names.rs` | Every word on the screen, indexed by the codes the crates hand over |
| `shapes.rs` | The shape buffer, turned into triangles |
| `sound.rs` | Every sound, the way `names.rs` is every word: the room's cues and the world's events played as clips cut from `Sounds/` |
| `settings.rs` | The Esc sheet: the UI scale, the audio volumes, the keys, and the run put back to where it opened |
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

A ship's doors and a station's are powered: a body walking up to one opens it,
and it shuts a moment after the doorway is clear. So an unlocked door is never
in the way of an order. The pathfinder plans straight through it, open or shut,
since it will be open by the time the Bim gets there, and the Bim does not stop
at a panel on the way. There is one grid for the deck, and nothing is worked
out twice.

A **locked** door is the one state of a door the pathfinder knows about: it is
a wall, and the grid is built again the moment a lock changes. An order to
somewhere only a locked door leads to has no route, so it is refused — the Bim
does not walk over and fail, and it does not drop what it was doing either. A
lock ordered while somebody is in the doorway waits for them, since the leaves
never close on a body, and a walk already under way through a door locked since
it was planned is caught by the watchdog that catches a frozen walk: it is
planned again from where the Bim stands, or put down if there is no way round.

What a door does is changed by hand, from the menu on it: hold it open, let it
close again, lock it, unlock it. Each is an errand of its own — the Bim walks
to the door's panel, a pace out of the opening on its own side, and works it —
and it goes on the agenda like any other.

Two things can come back from an order instead of a walk:

- **Nowhere to go.** No route there as the doors stand: a locked door in the
  way, or a spot no body can reach.
- **Ignored**, if the Bim is not one this player may order, is dead, or is
  outside the hull.

The refusal is said out loud, because an order that quietly does nothing reads
as a broken click: the log says it could not be done, there being no way there
at all, and a cross is drawn where the order landed.

### A chain never starts somewhere it cannot walk to

An empty route reports itself *arrived* the instant it is handed over — the
alternative, waiting on a walk that will never finish, hangs the chain for good.
That is fine for one step and quietly disastrous for a whole chain: every
remaining step finds itself already arrived and runs in the same frame, until
one of them sets a position outright and flings the Bim across the deck. A Bim
shut in behind a door and started on an errand on the far side of it once
turned up there without having walked a step of the way.

Two things stop it now. A walk with nowhere to go gives the chain up rather than
taking it for an arrival, putting the world back in a state the Bim can be left
in. And no chain is begun at all unless the Bim can reach the place it starts
at, so a Bim behind a shut door does not set off for the bench on the other
side of it. Queued chains are held back the same way rather than dropped —
they wait on the queue for the door.

Waiting is only right when the Bim can do nothing about it. A shut door is not
that: the panel is on the inside too, so a Bim that finds its work out of reach
behind one goes and opens it, and the work comes round again with the way
clear. Only a *locked* door is a real wait.

Reachability is worked out afresh every frame rather than cached, so opening or
unlocking a door needs no nudge: the moment the way is clear the Bim takes up
whatever it could not reach a moment ago, queued work included. Queued work it
cannot reach does not count as having something on, either, or a Bim would
stand over an unreachable errand with everything else it could be doing held
up behind it.

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
