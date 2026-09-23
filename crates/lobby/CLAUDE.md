# The lobby

Notes on `crates/lobby` — the World tab's galaxy. The screen it is drawn on
is `crates/app/src/screens/builder.rs`.

> **Since the port to Bevy (September 2026):** the browser host is gone.
> Every name table this file points at in `web/*.js` is now
> `crates/app/src/names.rs`; the pages are the screens in
> `crates/app/src/screens/`; the `bims_*` and `ship_*` exports are methods on
> `bims::game::Game` and `ship::Session`. The node harnesses
> (`scratchpad/*.mjs`, `smoke.mjs`, `stub.mjs`) are gone with the host — a
> check this file says lives in one of them lives nowhere now unless it says
> so in `crates/app`'s tests or `./check smoke`, and is worth restoring as a
> unit test if the thing it guarded moves again.

## "Has a station" is answered by generating the system, never by the designations

`Galaxy::designation_for` knows about the five stars *promised* a station of
each kind and nothing else. A quarter of the rest roll one on their own, and
a promised one can still lose it — a station with nowhere in its own system
to fly to is pruned. So `crates/lobby` builds **every** system once per
galaxy (`Galaxy::every_system`) and keeps a `has_station` bit per star; the
inspected system is then generated *afresh* from `Galaxy::system`, so the
harness's comparison of "reported with a station" against "what inspecting
it lists" is two paths and not one path against itself.

`galaxy_checksum` covers the systems as well as the stars for the same
reason: two players whose stars all matched and whose stations did not
would have spawn pickers on different stations. `worldgen::fixture` pins it
per galaxy type; `crates/worldgen/src/tests.rs` is the native end,
`Lobby::checksum` the other, and `crates/lobby/src/tests.rs`
parses the constants **out of `fixture.rs`** rather than carrying a copy.

## A crew cannot start at an enemy's

About three in ten of the stations somebody lives on are hostile
(`StationBlueprint::hostile`, rolled in `worldgen`; a derelict never is).
The star still counts as having a station — it is somewhere to fly to — so
`Lobby` keeps a second bit beside `has_station`: `can_start`, a star with at
least one station that is not hostile. The rule that a start is never at a
hostile station lives here and not on the page: `Lobby::random_start(roll)`
is the random pick, `Lobby::can_start_at(star, station)` is what the page
asks before it offers "Start here", and `Lobby::station_hostile(station)`
answers for the inspected system so the row can say why. The diagram rings a
hostile station's icon in `draw::ENEMY` — the same red the room draws a
hostile body in, written out here because the lobby imports neither
`game` nor `ship` (and `ship::world_paint` writes it out again for the
system map, for the same reason: change one, change all three).
`can_start_is_has_station_less_the_hostile_ones`,
`a_start_is_never_at_a_hostile_station` and
`the_diagram_rings_the_hostile_stations` pin the three, and the page's
"Start here" is `add_enabled(can_start_at(star, i))` with the row named
in `theme::BAD` and "· hostile" beside it, so the refusal says why. The
side is the generator's roll and nothing else — `crates/worldgen/CLAUDE.md`
has where it comes from; the world's `stance` is what reads it once the
game is open.

## The marks are the page's, and one of them is a wake

`preview::Marks` is everything the page asks to be drawn over the star
field and nothing the lobby decides: the star under the pointer, the
pending start, and — in the game, off the chart on the map — `here`, the
star the ship is at, `target`, the one picked for a jump, and since
feature 85 `visited`, every star the crew have been to, ringed in
`VISITED` grey inside the *here* ring so a star that is both reads as
both. The list is `World::stars_visited` — the systems the world has a
memory of plus the one it is at (`crates/world/CLAUDE.md`, "The dead lie
where they fell") — set on `Lobby::visited` every frame by
`screens/game.rs`, and empty in the lobby proper, where nobody has been
anywhere yet. The system map ticks a visited *node* in the same grey
(`ship::world_paint::paint_tick`), which is the third colour written out
in two crates rather than shared.
