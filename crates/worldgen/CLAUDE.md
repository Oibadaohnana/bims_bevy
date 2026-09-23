# The generator

Notes on `crates/worldgen` — the galaxy, what is in each system, and the
station blueprints. The lobby that draws it is `crates/lobby/CLAUDE.md`; the
world that opens one system of it is `crates/world/CLAUDE.md`. The crate's
own `lib.rs` and `data.rs` doc comments carry the three rules and the bump
list, and are the place to start.

## A galaxy is a seed, a type and a version, and the checksum is what proves it

`galaxy_checksum` (`checksum.rs`) eats every star and then every system —
each body, then each station's id, kind, parent, position, name, **shelf**
(`stock.0`), **price bias** (`bias`, one entry a resource, sign-extended)
and **side** (`hostile`). The last three are in it because they
are what a player meets the moment they dock: two builds whose stations
stood in the same places and disagreed about which of them shoot, or
what the ore costs, would be two galaxies. `fixture::REFERENCE_CHECKSUMS` pins it per galaxy type for
`REFERENCE_SEED`; `the_reference_galaxy_comes_out_at_the_numbers_it_is_pinned_to`
is the native end, `Lobby::checksum` the other, and
`crates/lobby/src/tests.rs` parses the constants out of `fixture.rs` rather
than carrying a copy — so re-pinning here is enough for both.
`the_checksum_notices_a_station_changing_sides` flips one station's
`hostile` and asks for a different number;
`the_checksum_notices_a_station_leaning_on_a_price` moves one entry of a
bias and asks the same.

Anything on `data.rs`'s bump list moves `GENERATOR_VERSION` (**7** now: 4 was
the body count going from one-to-seven to two-to-ten so a system could
hold more stations, and every position moved with it; 6 took the stations
off the belts — see below — which re-sites a station in every system with
a belt); 7 was the money rework — eight resources deleted, so every
shelf's bits moved, and a new stream for the gear trades. The
station shares, the shelves and the hazards are *not* on that list, on
purpose: they change what is in a system without changing whether its layout
passes `layout`, so they can be tuned while the galaxy keeps its shape. The
rolls are arranged so that they can: the shelf, the hazards, the map seed
and the side each come off their **own branch** of the station's stream
(`furnish`), so reworking one does not move the others, and a new resource
is drawn *after* the existing ones in `Stock::roll` (it walks
`ResourceId::ALL` in order), so a shelf already rolled keeps what it had.
The checksum still moves, because the shelf is in it.

## A station's desk leans on every price, and the roll is an integer

`StationBlueprint::bias` is an `economy::market::Bias` — one small whole
number per cent a resource, `-MAX_BIAS..=MAX_BIAS` (15), in
`ResourceId::ALL` order — rolled by `data::price_bias` in `furnish` off
`base.branch(0x_4249_4153_0000_0000 ^ id)` ("BIAS"), its own branch like
the shelf's, and **never for a `Derelict`**, which keeps no desk
(`Bias::NONE`). It is drawn for **every** resource whether the station
stocks it or not, so a resource added later is drawn after the rest and
moves none of them; and by `Rng::below` alone — no `unit()`, no
`range()`, nothing off the desolation, which is a `powf` — so it is the
same integer on every target, which is why it can go into the checksum
without a grid. `economy::market::quote` is what reads it: the book
price leaned on by the kind and by this, split into an ask and a bid
(`crates/economy`). The world's settlements roll theirs the same way
(`world::surface`, off the settlement stream) and the world sets the
**spawn's** to nothing (`World::start`). Adding the bias to the checksum
re-pinned the four `fixture::REFERENCE_CHECKSUMS`; it is not a
`GENERATOR_VERSION` bump, since no layout moved.
`a_station_leans_on_every_price_inside_the_range` pins the range, the
derelict, that every entry of the range turns up somewhere, and that
the same seed twice is the same lean. This crate depends on `economy`
for the type alone.

## A system has up to nine stations, and two of a kind is allowed

`data::MORE_STATIONS` is eight rolls — `[0.9, 0.85, 0.8, 0.7, 0.6, 0.5,
0.4, 0.3]` — for a second, third and on up to a ninth station once a
system has one; each is drawn whether or not the last one took, so the
stream stays in step between a system that stopped at one and one that
went on to two. It was one roll, then five, and a galaxy where most
systems had one dock had nowhere to go once that dock turned out to be an
enemy's; five became eight, with `MIN_BODIES`/`MAX_BODIES` in `system.rs`
going from `1..=7` to `2..=10` (stations sit one to a body, so the rolls
alone could not do it — a system with a station had three on average and
now has four and a half). `pick_kind` no longer refuses
a kind already wanted — two orbitals round two rocky planets is the whole
point — but it still asks only whether the system has a body of the right
sort at all; **whether that body is free** is `site`'s question, since what
is already wanted has not been placed yet, and a kind that finds every body
of its sort taken is simply not there. "One station per parent body" and
the relay's desolation rule are unchanged, and so is the pruning of a
station with nowhere in its own system to fly to.
`a_system_with_a_station_has_four_on_average` pins the mean across the
reference galaxies at four or more and never more than
`1 + MORE_STATIONS.len()`; `about_three_fifths_of_systems_have_a_station`
still pins the first roll at `0.5..0.7` — eight rolls of extras change
nothing about it — and asks that more than half of those systems have a
second station.

## Nothing stands at a belt

`data::parent_suits` is the matching rule, and its first arm refuses
**every** kind a belt for a parent. A belt used to be the crew's mining
site — `world::mining` laid the asteroids out about the ship holding at
it, and the money rework (feature 95) took the mining away — and a
station in orbit of one got in the way: a trip to a body ends
`flight`'s `ARRIVAL_RADIUS_BODY` (15 000) short of it, a station orbits
its parent at `STATION_ORBIT` of the minimum separation (about 10 000),
so a ship that came in on the station's side had the station for its
nearest node, the world's view settled on that and no site was laid out.
The mining outposts, which were bolted to belts, are dug into rocky
planets and ice worlds now, beside the orbitals. The rule outlived the
mining it was written for, and it stays: a station orbiting a belt is
still a node a trip would settle on instead of the belt. `nothing_stands_at_a_belt` (in `system.rs`'s
tests) walks every station of the four reference galaxies: none has a
belt for a parent, none stands within twice the arrival radius of one,
and there are still outposts. The world's `spawn` still wants a belt in
the spawn system, not because anything is done there any more but
because moving the simulation's dock would re-pin every hash in the
workspace (`crates/world/CLAUDE.md`).

## A station's side is rolled here, and the enemy's are together in one corner

`StationBlueprint::hostile` is `data::HOSTILE_SHARE` (0.3) of the stations
somebody lives on, rolled in `furnish` off `base.branch(0x_484f_5354_0000_0000
^ id)` — its own branch, so the count does not change when a neighbour
gains a hazard — and **never for a `Derelict`**, which draws nothing from
that branch at all: there is nobody aboard to be hostile. But the roll is
only **how many**: `take_sides`, the last thing `place_stations` does, draws
one direction for the system off `SIDES_BRANCH` of the same stream and
hands the hostile rolls to the stations furthest along it, so a system's
enemies sit together at one end of it and the friendly stations at the
other, and a crew that has learnt which corner is theirs can keep out of
it. The ordering is by the distance along that direction rounded onto
`SIDES_GRID` (a thousandth, the checksum's grid), then by id, so it is
plain arithmetic and not the last bit of a libm's `cos`.
`the_enemys_stations_are_in_one_corner_and_the_friendly_ones_in_the_other`
draws the direction again and checks the nearest of theirs stands at or
beyond the furthest of ours in every reference system with both. The
kind does not enter into it; any lived-on kind can be an enemy's
(`a_hostile_station_stays_hostile_and_any_kind_can_be`), and
`some_stations_are_hostile_and_derelicts_never_are` pins the share in
`0.2..0.4` for every galaxy type. The generator says only what was rolled:
the world's `World::stance` is the rule that reads it (home is friendly
whatever it rolled, `combat` makes the dock hostile whatever it rolled),
and the lobby's `can_start` is what keeps a crew from starting at one. A
station's `Station::hostile` in the world is this bit carried across,
kept for anyone who wants the roll rather than the rule.

## Who sells what is one `match`, and the gear is two flags beside it

`StationKind::sells` in `data.rs` is the only place the shelf's ceiling is
written down, and `a_station_stocks_the_staples_and_rolls_the_rest` in
`tests.rs` walks every kind against every resource, so a resource added to
`physics` without an arm here is a test that names it. Since the money
rework the table is short: `STAPLES` — **vegetables, tofu and medkits** —
are on every lived-in shelf whatever the roll, bandages and suits are
rolled at `STOCKED_CHANCE`, the class charges are nobody's, and a
derelict sells nothing.

**The gear is not on that shelf.** A weapon or a piece of armour is
behind one of two flags a place rolls off its **own** seed, on a branch
of its own (`Purpose::GearTrade`, 14): a **weapon trade** — handgun,
shotgun, auto rifle, sniper rifle, schword — and an **armour trade** —
helm, kevlar, leg guards — each at `WEAPON_TRADE_CHANCE` /
`ARMOUR_TRADE_CHANCE` (0.4 apiece) and each independent of the other, so
a place may have both, one or neither. `Stock::roll(kind, roll, gear)`
takes the gear stream beside the contents one and sets the bits;
`Stock::weapon_trade()` and `Stock::armour_trade()` read them back, and
`Stock::sells(resource)` answers for a gun or a piece off its trade's
flag rather than off the kind. A **derelict rolls nothing** — there is
nobody at its desk. `the_gear_trades_are_a_roll_of_their_own` pins that
the flags are a function of the seed alone, that each trade is all of its
list or none of it, and that both turn up over a galaxy.

That is what moved `GENERATOR_VERSION` to 7: the gear stream is a new
branch, the eight deleted resources moved every shelf's bits, and the
lean is drawn once a resource.

## The hyperlanes are a web, and they are in the checksum (feature 92)

`Galaxy::lanes` is an adjacency list over the thousand stars, built with
them in `Galaxy::with_version` by `galaxy::weave` and hashed in
`galaxy_checksum` right after the stars and before the systems — which
re-pinned all four `fixture::REFERENCE_CHECKSUMS` and is **not** a
`GENERATOR_VERSION` bump: no star moved and no system changed shape.
They are there for `world`'s crisis (`crates/world/CLAUDE.md`), which
spreads one hop every five days; **nothing flies down one**, and a
hyperdrive still reaches any star on the chart.

Two passes, and the second is the one that earns its keep. Each star
lanes to its `LANE_NEIGHBOURS` (3) nearest by **squared** distance, ties
on the lower star id — and because a lane is undirected a rim star ends
up with more than three (the measured spread is 3 to 7 or 8). That alone
leaves **islands**: four stars huddled inside the minimum separation pick
only each other and nothing else picks them. So the completion adds, while
more than one component is left, the shortest lane joining two of them —
Kruskal's over the pairs the first pass did not union, each round
flattening the components to a root and walking the pairs once. What comes
out is one connected graph, which is what makes `Galaxy::hops_from`
(breadth-first, `u16::MAX` for unreachable) finite at every star.

**No `sqrt`, no `powf`, no transcendental anywhere in it.** Every
comparison is on the squared distance — a subtraction and two
multiplications of f64, the same number on every target — because the
graph is in the checksum and the crisis's whole timetable hangs off it:
two builds that disagreed about one lane would put the machines at one
crew's door a week before the other's. `nearer` and `shorter` are the two
total orders (length, then the lower id, then the higher), and the lengths
are all finite so neither meets a NaN.

`the_lanes_are_one_connected_web_and_the_seed_decides_it` pins the lot —
symmetry, no self-lane, sorted and distinct, at least three each, one
component from star nought, a neighbour never more than one hop further
off, and the seed deciding it — and `the_checksum_notices_a_lane` that
dropping one moves the number. How far a galaxy runs in hops, and what
that means for the crisis, is measured in the root `CLAUDE.md`.

## A route along the lanes, and how its ties are broken

`Galaxy::route(from, to)` (feature 93) is the shortest chain of lanes
between two stars, both ends in it — `[from]` for the star itself, `None`
only for a star this galaxy has not got, the graph being one piece. It is
the same breadth-first walk `hops_from` is, kept separate because a route
wants the chain and the hop table wants the distances.

**Ties go to the lower star id**, and the rule is written down once: each
layer of the walk is taken in **ascending id order** and a star's lanes
are already sorted, so a star is first reached from the lowest-numbered
of the stars one hop nearer the start. That makes the route a function of
the graph alone — the same on every build, on every machine and in both
directions' own sense — which matters because `world` draws it on the
chart and charges a jump for its first step, and two clients that drew
different routes would disagree about where a jump goes.
`a_route_is_the_shortest_chain_of_lanes_and_the_same_every_time` and
`a_tie_goes_to_the_lower_star` pin both halves.

`Purpose::Jammer = 13` is the world's, like `DroidOrigin`: the station
the machines put in a system with no orbital station of its own
(`world::jammer`). The generator draws nothing from it and the galaxy
checksum never sees it.
