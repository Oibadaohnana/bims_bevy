# The generator

Notes on `crates/worldgen` — the galaxy, what is in each system, and the
station blueprints. The lobby that draws it is `crates/lobby/CLAUDE.md`; the
world that opens one system of it is `crates/world/CLAUDE.md`. The crate's
own `lib.rs` and `data.rs` doc comments carry the three rules and the bump
list, and are the place to start.

## A galaxy is a seed, a type and a version, and the checksum is what proves it

`galaxy_checksum` (`checksum.rs`) eats every star and then every system —
each body, then each station's id, kind, parent, position, name, **shelf**
(`stock.0`) and **side** (`hostile`). The last two are in it because they
are what a player meets the moment they dock: two builds whose stations
stood in the same places and disagreed about which of them shoot would be
two galaxies. `fixture::REFERENCE_CHECKSUMS` pins it per galaxy type for
`REFERENCE_SEED`; `the_reference_galaxy_comes_out_at_the_numbers_it_is_pinned_to`
is the native end, `Lobby::checksum` the other, and
`crates/lobby/src/tests.rs` parses the constants out of `fixture.rs` rather
than carrying a copy — so re-pinning here is enough for both.
`the_checksum_notices_a_station_changing_sides` flips one station's
`hostile` and asks for a different number.

Anything on `data.rs`'s bump list moves `GENERATOR_VERSION` (3 now). The
station shares, the shelves and the hazards are *not* on that list, on
purpose: they change what is in a system without changing whether its layout
passes `layout`, so they can be tuned while the galaxy keeps its shape. The
rolls are arranged so that they can: the shelf, the hazards, the map seed
and the side each come off their **own branch** of the station's stream
(`furnish`), so reworking one does not move the others, and a new resource
is drawn *after* the existing ones in `Stock::roll` (it walks
`ResourceId::ALL` in order), so a shelf already rolled keeps what it had.
The checksum still moves, because the shelf is in it.

## A system has up to six stations, and two of a kind is allowed

`data::MORE_STATIONS` is five rolls — `[0.8, 0.65, 0.5, 0.4, 0.3]` — for a
second, third and on up to a sixth station once a system has one; each is
drawn whether or not the last one took, so the stream stays in step
between a system that stopped at one and one that went on to two. It was
one roll, and a galaxy where most systems had one dock had nowhere to go
once that dock turned out to be an enemy's. `pick_kind` no longer refuses
a kind already wanted — two orbitals round two rocky planets is the whole
point — but it still asks only whether the system has a body of the right
sort at all; **whether that body is free** is `site`'s question, since what
is already wanted has not been placed yet, and a kind that finds every body
of its sort taken is simply not there. "One station per parent body" and
the relay's desolation rule are unchanged, and so is the pruning of a
station with nowhere in its own system to fly to.
`a_system_with_a_station_has_two_on_average` pins the mean across the
reference galaxies at two or more and never more than
`1 + MORE_STATIONS.len()`; `about_three_fifths_of_systems_have_a_station`
still pins the first roll at `0.5..0.7` — five rolls of extras change
nothing about it — and asks that more than half of those systems have a
second station.

## A station's side is rolled here, and a derelict has none

`StationBlueprint::hostile` is `data::HOSTILE_SHARE` (0.3) of the stations
somebody lives on, rolled in `furnish` off `base.branch(0x_484f_5354_0000_0000
^ id)` — its own branch, so a station does not change sides when its
neighbour gains a hazard — and **never for a `Derelict`**, which draws
nothing from that branch at all: there is nobody aboard to be hostile. The
kind does not enter into it; any lived-on kind can be an enemy's
(`a_hostile_station_stays_hostile_and_any_kind_can_be`), and
`some_stations_are_hostile_and_derelicts_never_are` pins the share in
`0.2..0.4` for every galaxy type. The generator says only what was rolled:
the world's `World::stance` is the rule that reads it (home is friendly
whatever it rolled, `combat` makes the dock hostile whatever it rolled),
and the lobby's `can_start` is what keeps a crew from starting at one. A
station's `Station::hostile` in the world is this bit carried across,
kept for anyone who wants the roll rather than the rule.

## Who sells what is one `match`, and its test enumerates every pair

`StationKind::sells` in `data.rs` is the only place the shelf's ceiling is
written down, and `a_station_stocks_the_staples_and_rolls_the_rest` in
`tests.rs` walks every kind against every resource, so a resource added to
`physics` without an arm here is a test that names it. Fibre is sold at orbitals — where
there is ground to grow it — and a bandage at orbitals and refineries;
neither is a staple, so a station rolls them like components. Galvum is
the outposts' alone, an emitter, a handgun and a vest are nobody's, rock is
nobody's, and a derelict sells nothing.
