# The room

Notes on `crates/game` — the Bims' simulation: the room aboard a ship and
on every station's deck, and the bare room the tests and the probes stand
bodies in (`Game::bare`). The root `CLAUDE.md` is how to run and verify
anything; the probe notes are `scratchpad/CLAUDE.md`, and the app that draws
the room and its panels is `crates/app` (`screens/game.rs`, `crew.rs`).

> **Since the port to Bevy (September 2026):** the browser host is gone.
> Every name table this file points at in `web/*.js` is now
> `crates/app/src/names.rs`; the pages are the screens in
> `crates/app/src/screens/`; the `bims_*` and `ship_*` exports are methods on
> `bims::game::Game` and `ship::Session`. The node harnesses
> (`scratchpad/*.mjs`, `smoke.mjs`, `stub.mjs`) are gone with the host — a
> check this file says lives in one of them lives nowhere now unless it says
> so in `crates/app`'s tests or `./check smoke`, and is worth restoring as a
> unit test if the thing it guarded moves again.

> **Since feature 104:** the needs are gone — hunger, sleep, the heads, the
> galley, the mess and everything else that made the room a life sim — and
> so is the classic room, the behaviour test room the `room` command opened.
> What is left is a deck the crew walk, work, doctor and fight on, with the
> fixtures standing on it as pictures. "The needs deleted (feature 104)" at
> the end says what went, what stayed, and why the parts that look dead are
> kept.

## An empty route means "arrived"

`Character::follow_path` with an empty route deliberately does not enter the
marching state, so `arrived()` is true immediately — the alternative is a task
that waits for ever on a walk that cannot happen. Any code that treats arrival
as proof the Bim is somewhere has to handle that case, or a whole chain will run
through in one frame and the first step that sets a position teleports the Bim
across the room. `Task::enter` now marks the chain blocked instead, and
`Game::can_begin` refuses to start one whose first station is unreachable.

## A route is planned once, never replanned

`Character::follow_path` is handed a finished list of waypoints and walks it to
the end. Nothing re-runs the pathfinder because the room changed shape, so
anything that puts a solid across a walk already in progress — a door locked
in its way, a part built on it — leaves the Bim pressed against it: still
*marching*, never arriving, with the chain waiting on an `arrived()` that will
not come. It looks for all the world like a stuck animation.

A ship's powered doors do not do this on their own account: an unlocked door
is never a solid, so leaves shutting behind one body never close a route
another is walking ("A ship's doors are powered" below). What catches the rest
is `Game::unstick`, the watchdog in "A frozen walk is caught": it plans the
walk again from where the body is, or puts it down. The walks that are planned
again on purpose are a fight's stands (`plan_stand`, every `PLAN_EVERY`), a
helper's walk to a patient that moved (`Task::patient_at`, `FOLLOW_SLACK`) and
the legs of a walk on the plain (`continue_far_walk`).

## Adding furniture moves everything

A rect added to `Room::solids` changes the nav grid, which changes every
route's length, which changes the frame on which the next roll off the room's
own stream is drawn — and every roll after it. That stream (`Game::rng`) is
drawn by each Bim's making (`Bim::new`: the heading, the idle glance, the
first wander, `born_year` and `born_day`), by the wander's steering
(`Character::update`), and by the blood on the deck (`blood.rs`: a drop's
scatter, a boot's crossing, a cut's splash). The hydroponic bay's one rect
once failed two long-run probes on that alone, neither of them in the code
under test.

What catches it now is the survivor tests — `SURVIVORS` in
`crates/world/src/tests_survivors.rs`, `PINNED` and `PICTURES` in
`crates/ship/src/tests_survivors.rs`: a solid added, dropped, grown or moved
is a run that plays differently, and they move. That is why every fixture
frame stayed a solid, in the same order, when feature 104 took away
everything the fixtures did ("The needs deleted (feature 104)" at the end).

## A route the body cannot hold to

`nav.rs` marks a cell blocked by its *centre*, so a straight line between two
free cells can pass within half a cell of an inflated obstacle and still test
clear. The Bim walks that line, the collision push-out shoves it back, and
where the push is exactly opposite the walk — a waypoint through the corner of
the table — the two cancel exactly: it marches on the spot for ever, `arrived()`
never comes true, and the chain waiting on it never finishes. It stood there
for ten game hours with an errand on its agenda, and nothing in the game says
anything is wrong: it is simply not moving.

`line_clear` now samples half a cell either side of the line as well as along
it. If a Bim is ever found frozen mid-errand, check its position against the
inflated furniture before looking anywhere else.

**It is rarer since, and which seeds hit it moves with the RNG — and what
is left is caught by `Game::unstick`, the watchdog described below ("A
frozen walk is caught").** Which seeds showed it once moved with every change
to the needs' rolls, and a long-run probe that froze a Bim for two days read
as starvation or a filthy deck — what happens to a Bim standing still that
long — when the cause was a Bim `is_walking()` at a fixed position with a
destination it never reaches. So **any change that draws from the room's
stream at all re-rolls which seeds show it**, and a seeded test that starts
failing at one seed after an unrelated change is very likely this and not the
change. The trace to run is the one that prints `activity`, `is_walking`,
`bim_pos` and `destination_for_probe` for a Bim whose errand has lasted a
while — a frozen position with `walking true` is the signature, and nothing
else aboard looks like it.

## The `SPOT_` codes are not the `HIT_` codes

There are two "what is at this point" questions and they want opposite
answers. `Room::hit` answers *what would a click act on*: few codes, only the
things with a menu behind them, and every rect expanded a few pixels so a near
miss on a handle still opens the menu. `Room::spot` answers *what is this*, for
the readout at the top left: everything aboard has a name, including the
worktop and the bulkheads, and nothing is expanded — a pixel beside the pan has
to read as deck.

Keep them separate. Folding one into the other loses the slack a click needs or
gives the readout a lie. `SPOT_NAMES` in `crates/app/src/names.rs` is indexed by the code,
so a new fixture needs a name added there in the same commit or the readout
says "Something". Feature 104 closed the `SPOT_` codes up to 0..=20
(`SPOT_NOTHING` to `SPOT_RESEARCH`) when the heads' door and the helm went, and
`SPOT_NAMES` is twenty-one long; the `HIT_` codes were left where they stood,
gaps and all (`HIT_SHIP_DOOR` 9, then 11 to 18), since the app matches them
rather than indexing a table by them. A fixture that only stands there — the
galley, a bunk, the heads, a bay, the broom locker, a shower — is `HIT_NONE`
under a click, like bare deck.

`Room::spot_rects(spot)` is the third question — *where is it*, for the
ring a panel row puts on the deck (`Game::set_highlight`, drawn in
`render`) — and it answers with **every fixture of the kind**: a row names
a kind, so the Making things row rings every bench. The chairs, the bunks,
the toilet and the basin are the exception, the first standing for the
rest, and the deck and the bulkheads answer nothing rather than the whole
room. `spot_rect` is the first of them. A new fixture list on the room
wants an arm there too, or its row rings nothing.

## There are many Bims, and `who` goes everywhere

`Game` holds `bims: Vec<Bim>` and almost every method that touches one takes a
`who: usize` first. `crates/game/src/bim.rs` owns the per-body state — the
figure, the errand and its queue, the health, the gear, the diary, the round —
and the room, the room's clock, the blood on the deck and the work list stay
on `Game` and `Room`, because they are the deck's.

Two rules that keep it honest:

- **A player steers its own Bim and nobody else's.** `players` says how many
  of the crew are players' own — slot *i* steers Bim *i* — and the rest are
  bots. Selection, orders and the menus go through the slot; a bot takes a
  player's orders only while the crew are under arms (`orderable`, see
  "Selecting is not commanding"), and another player's own never. Anything
  that *reads* takes a `who`, so the host can show whoever is selected.
- **A bunk is dealt to a Bim**, and a crew carried from one room to the next
  keeps to it — see the next section.

## A bunk is still dealt, and does nothing but stand a body somewhere

Nobody sleeps since feature 104, but a bunk is still dealt to a Bim, because
the deal says where a body stands. `Room::bunk_of[who]` is which bunk is
whose — `Room::bed_of(who)`, `bed_owner(bed)` — and `with_room` deals bunk
`i` to Bim `i` as far as the bunks go (`Room::bunks()`, the stand-in a layout
with none gets not counted — `stand_in_bed`). `take_crew` writes each Bim's
entry onto `Bim::bed` for the journey and `adopt` reads it back — kept when
it names one of this room's own bunks nobody here has (`bed_assignable`: a
real bunk, and not a docked station's, by the `foreign` box `set_foreign`
named), else the first spare, else none — so the ship's crew keep their
bunks across a dock and a hire takes what is spare (the world's `hire`
clears the one it arrives with first, which was the station's); `die` frees
it; `Room::relayout` follows the bed by its *frame*, since a bunk taken out
of the design ahead of somebody's shifts the rest down.

What reads the deal: `adopt`, which stands a body whose feet would land off
the deck at its bunk (`bed_station`), and `crate::aboard::starts`, which
starts Bim *i* at bunk *i*'s use spot. The bunks' frames are read without
it — the solids, `nearest_shelter` (a townsperson sheltering walks to the
nearest bunk, feature 94) and the rooms of a round (`routine::Anchors::of`).
There is no bunk menu, no name on the deck and no bot taking a bunk going
spare: those went with the sleep.
`a_bunk_goes_with_its_bim_from_one_room_to_the_next_and_a_spare_one_is_given`
pins the deal, on the combat ship. Adding a field to `Bim` or `Room` is a
`SAVE_VERSION` bump.

## Borrowing one Bim and the room at once

Anything that holds `&mut self.bims[who]` across a call to another `&mut self`
method does not compile, and a chain's constructor wants the Bim's
`character`, the `room` and the `maps` at once. Split the fields locally
first:

```rust
let (bims, room, maps) = (&mut self.bims, &mut self.room, &self.maps);
let bim = &mut bims[who];
```

Disjoint field borrows are fine *within one function body*. That is why the
per-Bim half of the frame is written as `&mut self` methods taking `who` and
re-indexing each time, rather than as one long-lived borrow.

## A chain holds a bench, the airlock or a site to itself

`Exclusive` in `game.rs` is the whole of what an errand needs to itself:
`Bench(i)` for a craft at a bench and for a carry putting a thing on one,
`Airlock` for a walk outside (one body out at a time, since the suit is
counted by the world and not taken out of the hold), and `Site(id)` for a
site aboard. Two places ask it, and forgetting the second is the trap:
`can_begin` covers an errand that *starts*, and `queue_ready` one that
**resumes** off the queue — without the second a half-done craft picks up
at a bench somebody else is standing at. Nothing free is `blocked`, the way
a locked door is. The fixtures the needs worked were lists a chain picked
the closest free one of (`task::Picks`, `Taken`); that went with them in
feature 104.

## The bed is a bed, and its footprint is still the bunk's

`fixtures::Berth` draws a single bed — headboard, footboard, mattress,
pillow, and the duvet over it (`draw_bedding`, drawn after the bodies) —
where it once drew a bunk bed with a ladder, and nobody lies in it any more.
The **footprint never changed** with the picture, and must not: it is what
the nav grid is built on, and a bed one pixel smaller is every route
re-lengthened and every roll after it re-rolled — see "Adding furniture
moves everything". `Berth::station()` is the old stand spot expressed off
the frame, and is where `adopt` stands a body coming aboard off the deck.

## A per-Bim clock must not live on a shared object

The deck is one object and a body is one of many, and a clock on a **body**
kept on a shared object is a clock every body resets for the others. It
happened: the deck's mess once held how long *this* Bim had stood in it, and
with two aboard whichever was comfortable zeroed the other's clock on the way
past, so the stage built on it never came — while the levels on the panel
looked right the whole time. The needs it belonged to went in feature 104;
the rule stays. **When adding a second of something, every mutable clock has
to be asked whose it is** — `Bim::plan_wait`, `fear`, a body's `seen_for` and
a sentry's `Trigger` are each a body's for that reason. A level that looks
right is not evidence that the machinery hanging off it does.

## Autonomy off is the work list off

`set_autonomous(false)` stops a Bim nobody steers *deciding* anything:
`consider_errand` bows out, so no job on the list is taken of its own accord,
and the medical row no longer puts an errand down at `HIGHEST`. An order, a
chain already running, the queue, the fight and a dressing the player orders
are untouched. The tests switch it off to keep a crew from walking onto work
while one body is measured, and a test of the medical row switches it back on.

## Selecting is not commanding

Any of the crew can be selected; only a player's own Bim takes that player's
orders in peace, and a bot only while the crew are under arms (`orderable`:
the alarm, or a player leading them — see the fight). The two are
deliberately separate questions, and the split is what lets the right-hand
side show whichever Bim you clicked while `order_move` still refuses a bot at
its errands. **A click
selects one; a marquee selects everybody it touches** (`select_many`,
`selected_all`, `selected_count`), and the panels show the first of them —
`selected()` — which is the player's own when it is among them. An order
with several selected goes to each that takes orders (`orderable`): a
right-click is a huddle round the point, a spot apiece off `CLUSTER_SLOTS`
in crew order; a **right-drag is a line** (`order_line`, RimWorld's
formation drag) — the squad spread evenly along it, ends included, each to
the point nearest its own place along the line so nobody crosses anybody,
one alone going to where the drag began. The screens hold the button:
`order_drag_begin/update/end(x, y, dragged)`, the glass deciding whether
it moved more than `CLICK_SLOP`, and `render` draws the line with a pip
where each will stand while it is dragged.
`a_marquee_selects_everybody_it_touches_and_a_right_drag_forms_them_up_along_a_line`
pins it.

## An order given with Shift waits its turn (feature 69)

`Game::order_later(slot, order)` is `Game::order` with Shift held: the
errand goes on the **back** of the Bim's `queue` rather than displacing
what it is on, so a Shift-click on a row, a Shift-right-click on the
deck and a Shift-drag for a line are RimWorld's queued orders. The
entry is a `Saved::ordered(who, kind, minutes, target)` — `Saved::fresh`
with its `ordered` flag up — and the flag is what tells the two apart at
`pump_queue`: a chain that was *put down* is `Task::resume`d where it
was, an order that was never begun is **begun** through the same door
the live order goes through (`begin_ordered`: a walk, `send_to_switch`,
`fetch`, `bandage`, `treat`), so every check the row makes is made
then, against the room as it is then, and what cannot be begun is
*dropped* rather than walked through to a patient that has gone.
`queue_ready` asks what beginning asks — `can_begin` rather than
`resume_station`, and for a walk `walk_ready`, a way there as the doors
stand — so a bench in use or a locked door is waited for rather than the
entry dropped.

**A walk is `Kind::Walk { post }` with the spot on `Saved::target`**,
and it is never run as a chain: `pump_queue` hands it to `walk_order` —
`order_move_for`'s body, the selection check taken off — so the plain's
windows are walked leg by leg and the refusals are the right-click's;
`post` is a crewmate's under the alarm (`queue_squad` sets it for
everybody but a player's own), so the Bim holds the spot the way
`order_one` posts it. `Step::GoToSpot` exists only so the entry has a
first step and `Kind::steps` a length. `queue_walk` refuses at the click,
with the cross, when there is no way there as the doors stand
(`can_reach` of the snapped stand), and `walk_ready` holds a queued walk
for a door locked since like an errand. `huddle` and `formation` are the spots a click and a drag deal
out, shared by the live order and the queued one; `queue_squad` is
`order_squad` for the queue.

**A plain order calls the queue off**: `drop_ordered(who)` throws away
the `ordered` entries and keeps the chains put down. `Game::order` calls
it for every errand variant (`CrewOrder::errand_for`) and
`order_move_for` for a walk — never `walk_order`, which is also how a
queued walk is begun, and never the bots' own `take_over`, so a Bim
taking up work of its own accord does not lose what the player queued. A
Shift on anything that is not an errand — a selection, a Management box —
is the plain order.

**Nobody takes a Bim off a walk the player gave it.** A chain that is
interrupted goes onto the queue and is picked up again (`interrupt` →
`pump_queue`); a *walk* has no chain behind it, so an interruption does
not put anything anywhere — the route is simply gone, and the Bim ends
up wherever the errand started it with every leg still to come walked
from the wrong place. So everything a Bim starts on *itself* asks
`character.arrived()` first (`consider_errand`, `pump_queue`,
`return_to_post`). The urgent-wound doctoring in `tick_bim` is the
deliberate exception and stays one: a Bim binding its own wound as it
runs from a fight has a route in hand and no chain behind it too, and that
is the one interruption the medical row is for. (The one errand that
reached *across* a Bim's own agenda was a crewmate walking another over
for a word; it went with the chat in feature 104.)

**The queued walks are drawn**: `render` threads a dashed line from where
the Bim is bound now through every `Kind::Walk` on its queue, a pip and a
ring at each in the ping's `ACCENT`, for the viewer's own Bim and whoever
it has selected; `queued_walks(who)` is the same list for the probes and
`ordered_count(who)` how many orders wait. The agenda shows the rest
under `JOB_WALK` = 27 ("Walking over"). Adding the flag to `Saved` is
`SAVE_VERSION` 13; `world::Command::CrewLater` and the app's
`Order::CrewLater` carry it over the seam (`wire::PROTOCOL` 6).
`a_shift_order_waits_its_turn_and_a_plain_one_calls_the_queue_off` in
`order::tests` and `a_shift_order_is_a_command_that_waits_its_turn` in
the world's `tests_orders` pin it.

## The room says what it sounds like, and plays nothing

`cue.rs` is the diary's arrangement for sound: a `Cue` is a code and a
place — a door's leaves starting to slide, a locked door heaved at and
giving, a shot leaving a gun, a bolt landing on a body or a bulkhead or
glancing off, a blow landing, a weapon drawn or put away, a grenade thrown
and bursting — pushed onto `Room::cues` or `Combat::cues` the step it happens and
drained by the app through `Game::take_cues`. What each is played as is
`crates/app/src/sound.rs`'s; a server drops them.

- **A door is heard when its leaves change direction**, not when a target
  changes. `cue::door_motion` keeps a `moving` memory on the door (the
  heads' door had one too, until it went in feature 104) and compares
  `open` before and after the step, so a door reversing part way is heard shutting, a
  held door told to hold again is silent, and the powered doors — which
  open for whoever walks up — say nothing about *why*.
- **A cue is a list, never a state.** At the world's top speed one step
  holds a door's whole open-and-shut; a flag read afterwards would show
  nothing. The room says every one and the app thins them — so do not
  rate-limit here.
- **Shots are heard where they fly.** A hostile room's people record
  `Shot`s rather than firing (`Combat::shoot`), and the world flies those
  in the crew's room through `Game::enemy_fire`, which is where the cue
  comes from; a hostile room's `Combat` says nothing about them. A blow on
  a crew member is `Game::enemy_strike`'s cue, on an enemy `Combat::brawl`'s.
- **A hand changing is a cue too.** `tick_combat` works out each step
  whether a Bim is armed and `Cue::Holster { drawn, player }` is said
  the step that answer changes — for everybody, `player` marking a
  player's own Bim in the crew's room, which is the one the app plays.
- **A cue that went takes its clip with it.** The knife on the board was
  the galley's, and went with it in feature 104 — `Action::Chop` is still
  the pose a deploy and a build are worked in, and never said anything —
  and `sound.rs` lost the clip in the same change. `BIMS_SOUND_LOG=1` (the root
  `CLAUDE.md`) is how what a fight sounds like is read from a terminal,
  now that the probe that counted the cues (`scratchpad/cues.rs`) went
  with the stew and the heads' door it counted.

## The diary keeps no words, and one entry

`memory.rs` stores `(day, minutes, code, one number)` and nothing else;
`names::memory_line` in `crates/app/src/names.rs` is where the sentence lives.

**It only records what went wrong, and since feature 104 that is one thing:
a crewmate's death** (`What::CrewDied`, code 30, written into everybody
else's diary by `Game::die` and not into the dead one's). Every other entry —
the meals, the nights, the accidents, the poisoning — went with the needs it
recorded, and the day's work was never written down: a page that filled
with "Went to the heads." three times a day was the reason. So a run in which
nobody died leaves the diary *empty*, and that is the intended reading. If you
find yourself adding a `What` for something that goes right, that is the rule
saying no.

An entry with no line in `memory_line` is **dropped from the page** rather
than rendered as a placeholder. **A new `What` is two edits**: the variant in
`memory.rs`, with a code no other has had, and its arm in `memory_line` —
miss the second and the entry never appears, which nothing but reading the
page would notice.

An entry is dated off the room's clock, which stands where the room was built
("The needs deleted" at the end), so every death in one mission carries the
day and the minute its room was built at. **`born_year` and `born_day`** on
`Bim` are the one other thing of a Bim's past that is kept: only the About tab
reads them, and they stay because they are two draws on the room's stream at
a Bim's making.

## The crew pass through each other on purpose — in peace

Bodies do not collide. Two Bims that meet overlap, and both drop to
`CROWDED_PACE` while they are within `CREW_CLEARANCE`. That is the whole rule —
`Game::crowding` and one multiplier on the pace.

**Under arms they do collide.** `Game::separate_under_arms`, after every
body has moved: every pair of *recruited* bodies on their feet on the deck
— an enemy's people at war, the crew under the alarm — closer than
`CREW_CLEARANCE` is shoved apart, each half the overlap along the line
between them (a nudge off the combat stream when they are on one spot).
The deadlock below is what a fight already guards against: `plan_stand`
replans every `PLAN_EVERY` and `unstick` watches the rest. Both bodies
recruited, so a recruited body is never moved by one walking past about
its errands, and a body in somebody's arms or carrying one is left alone
(feature 86). The other half of the same picture (a squad stacked on one tile) is
`Tactics::stand_with_cover`'s `taken`: where the rest of its side stand
or are walking to, no cell within a tile of one being a stand.
`bodies_under_arms_are_pushed_apart_and_in_peace_they_are_not` pins the
push; the sandbags test in `combat::tests` the second stand.

It is deliberate and it is the *only* resolution that cannot deadlock. **A
route is planned once and never replanned**, so a Bim whose line goes through
another has no second plan; anything that stops it reaching that line risks two
of them standing nose to nose for ever with errands on both agendas.

There was a whole apparatus here before — push apart, pick one at random to
stand aside for three seconds, shove that one sideways out of the other's lane,
with a blunt backstop for when the sideways step hit a bulkhead. It worked, but
only after two rounds of fixing what the fix broke, and every part of it existed
to keep bodies from overlapping. Once overlapping is allowed it all has nothing
to do. If a reason to reinstate collision ever appears, reinstate the deadlock
with it and read that history first. `scratchpad/crowding.rs` is the probe:
twelve head-on meetings that all get past, and the slowdown measured.

## The blood lives on `Room`

The blood on the deck (`blood::Blood`, `Room::blood`) is the room's because
the deck *is* the room, and a chain is handed the room and nothing else.
`Game::render` still draws it separately — `self.room.blood.draw` — because
the layering matters and only `Game` knows the order: under the fight's
scorches and the bodies. What it is and why its three rolls are pinned is
"The needs deleted (feature 104)" at the end.

## Anything held has to survive the chain being put down or given up

`Task::let_go` is where a chain leaves the world in a state the Bim can walk
away from, and it is reached two ways: `abandon`, for good, and `suspend`, for
a chain that goes on the queue to be picked up again. **Given up for good**, a
medkit in the hands goes back on the room's shelf (`Room::medkits`), and a
thing carried between two benches past `TakeGear` is said to have come back
(`Room::ferry_returned`, for the world to put away). **Suspended**, the kit
stays in the hands — it is on `Saved::main` and walks on with the chain — and
nothing is banked. Getting the two confused banks a thing twice: the chain
comes back with it *and* the shelf has it. Either way a body outside is
brought back in through the gangway, since a walk outside resumes from there.
Add a held item and this is the place to touch.

## Adding a job to the work list is three edits

`work.rs` for the variant, **appended** — the code is the row's identity;
`WORK_NAMES` in `crates/app/src/names.rs` for the word and `WORK_SPOTS` in
`crates/app/src/crew.rs` for the fixture the row rings (`SPOT_NOTHING` for
a job with no fixed place, like building and doctoring), both pinned
against `Job::ALL` by length tests. Same shape as `memory.rs`'s `What` and
for the same reason — no strings cross the boundary, so the ship knows
`Job::Haul` and only the host knows "Hauling". Miss the name and the row
comes up blank, and a blank row is a row the player cannot use that nothing
else would notice. Feature 104 took the list down to four — `Haul` (a
thing carried between two benches), `Craft`, `Build` and `Medical`, codes 0
to 3, closed up — when the needs' jobs and the helm went.

The host builds its rows off `work_count()` rather than off the length of
its own name table, so the two disagreeing shows up as a blank row rather
than as a job silently missing from the panel. Keep it that way.

Then `work_on_offer` says when the row is on offer and `do_some_work` what
taking it starts. `Job::Medical` (3, the last) is the one row that is also
an **interruption**: `medical_on_offer(who)` — a bandage to hand, the Bim
itself while it bleeds, else the crewmate with the most open wounds, and
that one's worst part — is asked in `tick_bim` as well, before the errand
is stepped, and at `HIGHEST` it `bandage`s on the spot, which puts the
errand on the queue the way any interruption does; at any other number it
waits its turn like the rest, and it is offered **first** among equals,
code notwithstanding, so an untouched list dresses a wound before it takes
anything else up. Two things it asks that the menu's `bandage` does not, because
the menu is a player who can see and this runs every step: a patient the
helper has no route to is not offered (`task::patient_stand`, the same
question the walk asks — a chain that starts, finds no route and is given
up would be offered again next step, for ever), and a part somebody else
is already walking over to dress is left to them.
`medical_at_the_top_puts_an_errand_down_to_dress_its_own_wound` and
`a_crewmate_bleeding_is_dressed_by_whoever_is_free` in `game::tests` pin
it. A wound now puts a queued errand down the step it opens, so a test
that orders a pick-up after wounding somebody finds the fetch on the queue
behind the dressing.

**`Job::Craft` is the one row a bot never takes** (feature 89).
`craft_on_offer` asks `Game::is_bot(who)` — `hostile_bodies || !is_player`,
the same question `medical_on_offer` asks — and an **open** order (one
with no `only`) is offered to a player's own Bim alone, whatever the
Craft row is set to. Standing at a bench is the player's work: the
workbench, the armoury and the drug lab all go the same way, since the
room knows a bench only by `Bench::kind` and the rule is
about who is standing there rather than about which bench it is. An
order *named* for one Bim (`Order::only`, an engineer's armour repair at
the workbench) is still that Bim's, bot or not — the world keeps such an
order posted until it is finished, so a bot refusing its own would leave
the bench held for ever. Everything else a bot did it still does: it
ferries, builds, doctors and shoots. `a_bot_never_stands_at_a_bench_and_a_player_s_bim_does`
in the world's tests pins both halves.

An **activity code** (`JOB_*` in `game.rs`, what `activity()` and the
agenda say a Bim is doing) is the same shape one table over: appended after
the last, and named in `job_name` in `names.rs`, a `match` with a `Busy`
fallback — so a code left out is a Bim reading "Busy", which nothing but a
test notices. The codes kept their numbers when the needs' went: `JOB_DOOR`
7 and `JOB_DOOR_LOCK` 8, then 18 to 28, with 25 (finishing a body off) free
and 19 and 20 (the walk out to mine, the haul to a site) still named but
never said, so the next is 29. The `the_bandage_job_has_a_name` block in
`names.rs`'s tests pins the newest of them against `job_name`; a new one
wants a line there.

## Health mends every frame, so a sudden nothing is not nothing

`Health::update` runs before `Game` looks at whether the Bim is dead, and a
living body's parts mend. Anything that takes the bar to zero *later* in the
frame — a body given up (`Health::give_up`, which is how `kill_for_probe`
and a grave laid out kill one) — was therefore back above zero by the time
the check came round, and the death simply never happened: the run ended
with a Bim reading nought health and still walking about. `update_held`
now returns immediately when `is_dead()`. Anything new that damages health
in one go depends on that, so do not "tidy" it away.

**The bar is three parts and the blood is a fourth number**
(`crates/game/src/health.rs`). `Part::Head`, `Body`, `Legs` — codes 0–2,
the armour slots' order — with `Part::max` 5, 75 and 20, and `points()` is
the three added up, so the panel's bar is what it was: mending regrows all
three **in proportion** to their size. `is_dead` is the blood at nothing,
or the head and the body both at nothing with no trauma on either — which
only `give_up` does, zeroing all three and clearing the traumas. A shot
(`Health::shot(part, damage, cut, roll)`) is rolled onto a part by
`Part::hit_by(unit)` off `HIT_ODDS` (one in twenty, three in four, one in
five), takes the damage off that part and opens a **wound** there; a part
reaching nothing is a dying state and not a death, and a leg is lost only
to a crushed-leg trauma ("A part at nothing is a dying state" below).
Every open wound bleeds `BLEED_PER_WOUND` (10) of `MAX_BLOOD` (100) an
hour until `bandage(part)` closes every wound on that part; under
`SLOWED_AT` (three quarters) the Bim walks at half pace, under `OUT_AT`
(**half**, feature 89) it is `unconscious()`, at nothing it is dead, and
the blood comes back over two days once nothing is open. What kills is
the blood, read at the top of the body's next tick and not at the hit,
which is why the world reads a death off `is_dead` per body and not off
the hit (see `crates/world/CLAUDE.md`). `the_parts_add_to_a_hundred_and_the_odds_to_one`
and the tests beside it in `health.rs` pin the arithmetic; the fight that
inflicts it is "The fight" below.

## Aboard, the nav grid is phased to the tiles, and that is what makes a one-tile corridor walkable

`nav.rs` inflates every obstacle by a `BODY_MARGIN` of 23 over a 52-unit
tile, so a one-tile gap leaves a six-unit strip down its middle — narrower
than a cell. Whether any cell centre lands in it used to be luck: the
classic grid (`Nav::new`, 10-unit cells, what a bare room still gets)
starts from the walkable area's edge, and the phase against the tiles
drifted two units a tile, so one corridor walked and the next did not.
Aboard the room now builds `Nav::tiled` — `CELLS_PER_TILE = 5`, origin half
a cell in from the interior's tile-aligned corner — so every tile's middle
is a cell's middle and the strip always holds one. `Room::nav_tile()` says
which grid a room gets (`None` for a bare room, which is laid on no tiles;
`Some(TILE)` for every room laid out from a design), and `Maps::new` takes
it. The `a_one_tile_corridor_can_be_walked` block in the world tests pins a
straight gap and an L, and fails on the old grid. That moved
`REFERENCE_CHECKSUM`.

What is still not walked is a gap that is only **diagonal** — two solids
corner to corner one tile apart — because nothing of that radius fits
through it; `validate` does not know that, and the station layout's rule
about it stands. The contract at the top of `crates/shipdesign/src/lib.rs`
says the same; keep the two in step.

## A ship's doors are powered, and a lock is the only thing the nav sees

`crates/game/src/door.rs` is a designed `Door` part aboard: a sliding door
that **opens by itself** for any body within `REACH` and shuts `SHUT_AFTER`
after the doorway is clear. The part is **1×2, like the airlock** — two
tiles along the bulkhead, one deep, so one door is the whole of a two-tile
doorway — and which way its leaves slide is its **rotation**, through
`parts::door_slides_along_x` (the long side of the turned footprint: `R0`
in a bulkhead running north–south, `R90` in one running east–west). It
used to be one tile with the direction guessed off the neighbours, which
is why a door standing in nothing was drawn one way and walked another;
now `aboard.rs` and `fittings::door` read the same function, and
`fittings::door` draws in the part's own frame through `hull::Local` so it
turns with the part. A doorway in a layout is therefore **one `put`**:
`playtest_ship` and `station::build_layout` both place the door at the
run's first tile, `R90` along a row and `R0` down a column, and
`PLAYTEST_PARTS` moved with it (it is 647 now, with the armoury and its
reactor aboard; `crates/shipdesign/CLAUDE.md` keeps the current
number). A ship has a door in every bulkhead and a grid per combination of
them is not a thing, so **an unlocked door is never a solid** — the
pathfinder plans through it open or shut, and the leaves are open by the
time the body arrives. (The classic room's heads' door was the one door
that was a solid when shut, with a grid per state in `Maps`; it went with
the heads' compartment in feature 104, and `Maps` is one deck grid,
`Maps::deck()`, beside the outside's and the plain's.) `Room::doors` holds them
(`Layout::doors` from `aboard.rs`, which reads which way the leaves slide
off the rotation), `Game::update` ticks them with everybody's position,
and the room draws them (`Door` is in `drawn_by_room`).

**Locked is the one state routing has to know about**, and it costs a
rebuild of `maps`: `Game::refresh_maps` when `locked_doors().len()` changes,
`refresh_blockers` when `shut_doors().len()` does. `Nav::new` now
rasterises each solid over the cells its inflated box reaches instead of
testing every cell against every solid — the same predicate, so the grids
are identical, and a joined room's rebuild is milliseconds. A lock is
ordered but the leaves **wait for anyone in the opening** (`IN_THE_WAY`),
and nav treats the door as solid from the order, so a route planned after
the click already goes round.

The player's four words — `door::Order` Open (hold), Close (let go), Lock,
Unlock — are each an errand through `Switch::Door(index, order)` that walks
the Bim to the panel (`Door::station`, a `STAND_OFF` out of the opening on
its side), `JOB_DOOR` or `JOB_DOOR_LOCK` on its agenda. The host asks
`Game::hit_door()` after `Game::hit_at` said `HIT_SHIP_DOOR = 9`; the
readout's `SPOT_SHIP_DOOR = 16` carries the door's state through
`Game::door_at(x, y)` and `ship_door_is_locked`, `ship_door_is_held` and
`ship_door_is_open`. The
`a_locked_door_is_a_wall_and_an_unlocked_one_is_not` block in
`crates/world/src/tests.rs` pins the routing half. One thing that bit:

- **A station's room draws no doors while docked**
  (`Game::set_doors_drawn(false)` in `World::join_rooms`). The joined deck
  has the same doors and draws them, and two pictures of one door would be
  a door in two states; the residents, who walk about in the station's
  room, are put on the joined deck as *visitors* (`Game::set_visitors`,
  from `Aboard::visit` every step) so its doors open for them too. Visitors
  are bodies for the doors and nothing else — not crew, not solid, not
  selectable.

## The bay is six tiles, and which way its trays run is data

`PartKind::HydroBay` is `(6, 1)` with six use spots along its north side,
and the room draws it as `fixtures::Bay`: six trays (`TRAYS`) under grow
lights, one a tile. `Bay::at(frame, side)` lays the trays along the long
axis, reading `side` — `Layout::bay_side`, which `aboard.rs` reads off the
part's first use spot — so a bay turned to `R90` or `R270` is six trays
down rather than six strips across. Nobody tends a bay since feature 104
and nothing grows in one: the trays are drawn planted with nothing, which
is how a fresh room always drew them.

**The side is which edge of the frame the spot lies beyond, never which
way it is off the centre.** The first spot is at the *end* of the run, and
measured from the middle of six tiles it reads as off the end — which laid
the trays *across* the bay, six strips one tile long.
`the_bay_aboard_is_a_tray_a_tile_along_its_run` in
`crates/world/src/tests.rs` pins a tray a tile for a bay lying and one
standing, through `Bay::trays`.

Laying a run of six is where the corner-to-corner rule bites hardest, and
`a_station_s_rooms_can_all_be_walked_from_its_door` found both cases in one
afternoon: a run ending diagonally against the locker pinched the locker's
spot, and a run two tiles from the chamfer's corner piece — one tile
between them — left the whole strip under it as deck nobody could reach.
The station's runs start three tiles in from the west wall and stop three
rows off the south one, and the locker went to the west wall.

`nav_map_of_a_station` beside that test is `#[ignore]`d on purpose: it
prints the nav grid of one station as a digit per tile, and it is the
first thing to run when the walkability test names a tile that looks fine.

## Aboard, a fixture is drawn inside its tile

The room's pictures were once drawn at the classic room's offsets, and on a
ship's one-tile part they ran out of the tile: the pot overhung its
neighbours, the chair was wider than its tile and the dishwasher was only its
door. The rule since is that everything the room draws for a part is laid out
**off the part's rect**, not at fixed offsets, and it is a rule of
`crates/game/src/fixtures.rs` now: `hob_scale_of` fits the burner inside a
one-tile hob and the pot is `POT_SIZE` of that; `burner_of` centres it, but on
a double-width run keeps it at `centre - 30`, where the left of an old pair
stood; `CHAIR_SIZE` is 48×42 with the backrest a unit inside the tile; and a
dishwasher on a tile of its own draws the whole tile as its `body`. A new
picture for a fixture goes in as a function of its rect. No assertion can see
a picture spill: a screenshot of the deck or the yard (`BIMS_SCREENSHOT`, the
root `CLAUDE.md`) is how to look, and `PICTURES` in
`crates/ship/src/tests_survivors.rs` says, bit for bit, whether a picture
moved at all.

## A berth has an axis

`fixtures::Berth::lying` is a bunk turned a quarter — a frame wider than
tall — and everything about the bed goes through `Berth::at(across, along)`,
`size(across, along)`, `breadth()` and `length()`: the picture, the pillow,
the bedding, and where a body is stood beside it (`station`). A bed lying
down is the same bed on its side rather than a blanket drawn off the end of
it.

## A frozen walk is caught

`Game::unstick` is the watchdog: a Bim marching with a destination that
has moved less than `STUCK_STEP` a second for `STUCK_AFTER` is stopped
dead, and nothing else aboard looks like that. It plans a fresh route to
the same destination from where the body is, on the body's own grid
(`nav_for`, so a body on the plain is not clamped to the deck's edge); if
there is none — a door locked since the route was planned — it clears the
walk (`follow_path(Vec::new())`, so `arrived()` comes true and the Bim can
start something else) and `interrupt`s the chain onto the queue, where
`queue_ready` holds it until the way is open again.

The case it was written for: **the turn is an arc.** A body turns at
`TURN_RATE` while moving at `MARCH_SPEED`, a radius of some seventeen units,
so a walk that starts facing the wrong way leaves the line it was given by
up to a body's width. Off the line against the inflated face of the table
with the next waypoint straight through it, the push-out undid every step
exactly: a Bim stood at (315, 320) for days. The replan takes it round. The
other case it caught, a door shut by hand across a walk into the heads, went
with the heads; a door locked across a walk is the same case today. A body
that stops moving with an errand on its agenda is a frozen walk until proved
otherwise.

## Outside, the body is on a second grid

A walk outside is a walk, not a clock, and since the mining went (feature 95)
the one errand that takes it is a construction site beyond the hull (the next
section): `GoToSuitLocker` → `TakeSuit` → `GoToGangway` → `StepOut`, whose
`leave` stands the body a tile beyond the collar (`Character::go_outside` —
standing, the suit on, no longer seated), then the site's own steps, and
`WalkToPort` → `StepIn` → `BackToSuitLocker` → `PutSuitBack` home.
`Kind::fork` is where a chain turns off onto the steps of its own, shared by
`Kind::steps` and `Task::next_step` so the agenda and the chain cannot
disagree, and `outside_half(kind, step)` is which steps are out there, for
`rewind` and `resume`: a walk put down out there — an order, a wound —
restarts its outside half from the gangway and goes out again, rather than
dropping back into a step at a place it is no longer beside.

The grid it walks is `Maps::outside`, `Nav::outside`: one cell a **tile**
(not five), `OUTSIDE_RADIUS` (100) tiles every way about the body, over
`Room::hull` (every structure tile, from `aboard.rs`). `Game::refresh_outside`
at the top of every `simulate` builds it while there is a site
(`Room::builds`) or somebody out there, centred on whoever is out or on
`Room::outside` when nobody is; builds it again when the body is
`OUTSIDE_RECENTRE` tiles off its middle, or `Room::rocks_version` moves
(which nothing moves since the rocks went); and drops it when there is no
site and nobody out. Everything that plans or moves a body asks which grid
by `Character::is_outside()`: `Task::enter` and `unstick` through
`Maps::for_body`, and `Game::move_body` — the **one** call that moves a Bim —
hands an outside body the grid's span and `outside_blockers` (the hull)
instead of the deck and the furniture, so nothing shoves it back through the
hull. `crowding` skips a body outside; `walk_order` refuses one (the walk
brings it in).

## A site is worked from the deck or from outside, and the room is relaid under the crew

`Kind::Build { site, outside }` is the construction chain — `GoToSite →
Construct` — with, for a site beyond the hull, the walk outside's steps either
side (`GoToSuitLocker` … `StepOut` before, `WalkToPort` … `PutSuitBack`
after). Nothing is carried to a site since the money rework (feature 95): a
part is paid for out of the crew's pool, so the errand is the walk and the
work. The world says what there is to build (`Game::set_build_orders`,
`Room::builds`, one `Build` a site with its tiles in room units and its
minutes) and who may suit up (`Room::suit_ok`); the room says on
`Room::built` — `(site, who)` — that a site is done, and **changes no ship**:
the world takes the price and puts the part down. `JOB_BUILD = 21`;
`Job::Build` is the third row.

- **Inside or outside is decided by `task::site_stand`**, one place: the
  nearest tile beside the footprint — four ways, never a corner — that the
  grid has a route to, else a tile *of* the footprint (plating goes under
  the Bim's own feet). Asked of the deck's grid from the Bim, and failing
  that of the outside grid from `Room::outside`, in `Game::site_reach`;
  asked again as the walk is entered, from wherever the body is on
  whichever grid it is on. A site the world has dropped has nowhere to
  stand and the walk is `blocked`, the way a locked door blocks one. It is
  asked before the errand is offered, since `first_station` has no answer
  for a chain that picks its spot on entry.
- **`refresh_outside` keeps the outside grid while there are sites**, since
  the decision above needs it.
- **`Exclusive::Site(id)`** is one pair of hands per site aboard; a site
  outside takes `Exclusive::Airlock` instead, which is one body anyway.
- **`Room::relayout` and `Game::relayout`** lay the room out again under
  the crew for a design that changed: geometry from the new layout, state
  kept — `Blood::resized` carries the blood across by position, the doors
  match by opening, the bunks by frame (index is identity, and new ones go
  on the end) — and the fixtures that are only pictures laid out afresh,
  having nothing to remember. The grids and blockers are rebuilt; a Craft
  chain whose bench index now names a different bench is abandoned.
  `from_layout` and `relayout` share `bed_side`, `layout_chairs` and
  `galley_faces` so the two cannot lay the furniture out differently.

## Sight is traced on the tile grid, shared, and the fight reads it

`crates/game/src/sight.rs`. Every Bim sees all the way round; what stops
its eyes is what stands in the way, and `Sight` works that out by tracing
a straight line from each crew member to the middle of every tile of the
room's grid (`TILE`, 52) — the grid-line-by-grid-line traversal, so a
diagonal gap between two opaque tiles is stopped — and marking the tile
seen when nothing opaque is crossed before it. The tile the line stops at
is seen too, so a wall is seen from the room it walls. Opaque is
`Layout::opaque` (`aboard.rs`: every no-deck tile in the deck's box and
every Object-layer part whose `PartDef::blocks_sight` says so — the rule
is in `shipdesign::parts`, and it is the movement rule minus the low
furniture) plus everything outside `interior`, plus what is shut **right
now**, added at every trace: `Room::shut_leaves(bodies)` is every powered
door whose leaves are not open — locked or not; `shut_doors()` is the
pathfinder's question, locked *and* shut, and asking that let sight
through every unlocked door — and every airlock (`Layout::airlocks`)
with nobody within a door's `REACH` of it, since the room has no leaves
for an airlock and the ship painter's `airlock_ajar` is a picture. The
bodies are the crew and the visitors, as for the doors themselves. A bare
room has nothing opaque inside its walls; the classic room's heads, with
their walls and their shut door, were the last thing in the room that was
opaque without being a part, and went in feature 104.

- **It is recomputed only when it could have changed** — an eye crossed a
  tile, or the set of shut doors differs — and once a frame, from
  `Game::render`, not once a step. A whole trace of a joined room is about
  a hundred microseconds. A probe that wants `Game::seen_at` without a
  picture calls `Game::observe` itself. **The shut doors alone go in
  every step**, at the top of `tick_combat` through `Sight::set_shut`
  (`Game::shut_now` is the one list, the trace's too): a station's room
  under a joined deck is `Fog::None`, so `observe` never traces it, and
  its people aim through `Sight::cells` — without the doors there they
  shot through every shut one and `Tactics::stand` scored every stand as
  if no door stood between. `set_shut` marks the mask stale so the next
  `observe` does not skip a trace over doors it was handed already.
  `a_shut_door_stops_a_line_of_sight_without_a_trace` in the world tests
  pins both.
- **The mask is shared** by construction: one trace per eye, OR'd. Every
  Bim in `Game::bims` is an eye, which aboard is the crew and nobody else.
- **The fog is drawn over `Layout::hull`** (every structure tile), over
  the deck's box on a plain, and over nothing in a bare room, which is
  handed no hull; unseen tiles are merged into
  row runs and stacked into rectangles so it is a few dozen shapes. It
  goes over the fixtures and under the night wash, the highlight ring and
  the marquee.
- **`sight::Fog` says whose eyes a room is drawn through.** `Crew` is the
  default: the crew's trace, every body drawn. `All` is a room looked at
  from outside — a station's while the ship is merely alongside — with
  everything fogged and nobody drawn. `None` is a station's room under a
  joined deck: no fog of its own (the joined room's covers both hulls) and
  a body drawn only where the world said it is in view
  (`Game::set_seen`, from `Aboard::seen` every step in `World::visit`).
  `Game::body_seen(who)` is the one question, and
  `Session::resident_on_screen` asks it so a name never floats over a
  body that is not drawn. `sight_is_traced_and_stops_at_walls_and_shut_doors`
  and `docked_the_crew_see_what_is_in_view_and_the_station_s_people_only_there`
  in `crates/world/src/tests.rs` pin all of it.
- **A Bim against a wall peeks round it.** `Sight::eyes_from(p)` is the
  body's own eyes plus, for every opaque 4-neighbour of its tile, the free
  tile either side of it *along* that wall, each restricted to tiles past
  the wall's line (`Eye::admits`: `(tile − body)·wall ≥ 1`). `observe`
  traces every eye of every body; `sees_from(body, target)` answers for
  one body and one target and says *which* eye saw it, and a shot is
  fired from that eye — so a peeking Bim shoots from the peek and not
  through the wall. The peek is also where it is *shot at*, and the body
  is drawn leaning out to it — see "A peek exposes the peek" in the
  fight section. A tile back from the wall there is no peek and the
  trace stops at the corner's edge. `a_bim_against_a_wall_peeks_round_it`
  in the world tests is the layout the rule was asked for with.
- **Whose a tile is decides its fog.** `sight::Stance` — Friendly,
  Neutral, Hostile — on the room's own tiles (`Game::set_stance`) and on a
  foreign box (`Game::set_foreign`, the station's box on a joined deck;
  both kept on `Game` and put back on the fresh grid at `relayout`). A
  friendly tile unseen is the semi `FOG`; a stranger's is `FOG_BLACK`
  where no line of sight has ever reached, `FOG_GREY` where one has and
  none does now — `explored`, OR'd from `seen` after each trace and
  **sticky**: what has been looked at stays grey, and only the bodies in
  it are forgotten. There is no ring: a wall is seen from the room it
  walls, so a room's outline comes in as its inside is looked at, and
  nothing behind a bulkhead shows until somebody has seen past it (the
  three-tile ring that used to reveal the next room's console through
  the wall is gone, September 2026). Under `Fog::Crew` the picture is
  the light map's (below); `draw` is the tile passes for `Fog::All` —
  three passes of the run-merging, one a veil; an opaque rectangle is
  grown by `OVERLAP` so the feathered seams between black runs do not
  show as lines. `Game::veil_at` reads the tile rule for the probes, and
  `Game::observe_from_for_probe(eyes)` traces from wherever a probe says
  — from nowhere, to see what is remembered. Under `Fog::All` a
  stranger's room is black entirely, which is what a hostile station
  alongside looks like; beyond the residents' range the ship painter
  keeps drawing a stranger as its far plate (`HULL_UNKNOWN`) so nothing
  is revealed at fifty tiles.
  `a_stranger_s_deck_is_black_where_nobody_has_looked_and_grey_where_they_have`
  in the world tests pins it.
- **A body on somebody else's deck stays drawn for `SEEN_FOR`** (2 s)
  after the world last said it was in view: `set_seen` re-arms
  `seen_for` per body, `body_seen` reads it, `simulate` counts it down.
- **The fight is the first thing to read sight.** See the next section.

## The fight: targets in, hits out, and the room never decides who is an enemy

**In a run every enemy is a machine** (feature 83 brought them, 102 made
them the only enemy and 104 deleted the human ones). The room still has the
code for a room whose *Bims* are hostile — `Game::set_hostile_bodies`, the
war and its `muster`, an enemy's `plan_stand`, the recorded `Shot`s the
world flies across the seam, the hunt, the breach and the seal — and this
section describes it as it is. `game::tests` and `combat::tests` stand it
up on a bare room or on the playtest ship made hostile (`hostile_ship`),
and the machines reuse most of it (`tick_droids`, "A droid is not a Bim"
below). No room of a run has hostile Bims: a station the machines hold has
none of its people left (`World::people_of` is nought), so the world's
`EnemyDown` is never said, and a town's people defending it are friendly
and fight the machines inside their own room (feature 94, "The machines
have a list of their own" below).

`crates/game/src/combat.rs`. Every `Bim` has a `gear: Gear` — three armour
slots (`Option<Piece>` each, see "Armour is clothes" below), a weapon
slot, and a nine-cell `pack` — `Gear::issued()` a
`WeaponKind::LaserPistol` and nothing else — plus `reload`, `hit_flash`,
`burst_left`/`burst_timer`, `locked`/`melee_timer` and `peek`, all of
which `tick_combat` owns. `WeaponKind` is five kinds with written-out
codes — `LaserPistol = 1`, `Shotgun = 2`, `AutoRifle = 3`, `SniperRifle
= 4`, `Schword = 5` — and `resource()`/`from_resource` is the one table
from a kind to its `ResourceId` code (8, 17, 18, 19, 20 since the fuel resource went), a number since
this crate does not know `physics`; the world's `armour::weapon_resource`
is its typed mirror. `WeaponKind::stats()` is the one table of numbers,
range and speed in **tiles**, and a weapon's odds and its damage are
**two points and a line**: `accuracy` out to `sweet` tiles and
`accuracy_far` at `range`, `damage` and `damage_far` the same, straight
between (`WeaponStats::along`), read through `hit_chance(tiles)` and
`damage_at(tiles)`. `ACCURACY_RANGE` and the `accuracy ^ (tiles / 10)`
curve are gone. **The second tuning of September 2026** put every gun's
damage up a fifth and its odds down a tenth, and the pistol's and the
auto rifle's range up ten tiles; the pistol is `range 22, sweet 0,
0.855 → 0.585, 7.2 damage flat, speed 18, fire_rate 1.5` — 0.73 at ten
tiles, where the app used to print 0.70. `burst`
is shots to a trigger pull and `burst_gap` the seconds between them;
`fire_rate` is then **trigger pulls** a second, and `dps()` is `burst ×
fire_rate × damage`. `melee` is a blade: `sweet = range = MELEE_RANGE`,
accuracy 1, `fire_rate = 1 / MELEE_PERIOD`, `speed 0`. The numbers the
user asked for, after that tuning: shotgun range 10, sweet 4,
0.81/0.54, 60/36 (so 48 at seven), speed 20, one pull every four
seconds; auto rifle range 26, sweet 8, 0.765/0.45, 6/4.8, speed 22,
`burst 8`, `burst_gap 0.25`, one pull every four seconds (eight in two,
two to recharge); sniper rifle range 35, sweet 20, 0.9/0.63, 54/30,
speed 60, one every four seconds; schword 42 a swing, its odds left at 1
since a swing lands by reach and not by a roll. A body is 75, so no
single shot but a head shot kills. The constants beside them:
`MELEE_RANGE = 1.2` tiles, `FIST_DAMAGE = 20`, `MELEE_PERIOD = 2.0` s,
`SWING_TIME = 0.5` s, `DODGE_IN_COVER = 0.5`, and `WALKING_ACCURACY =
0.5` — **a shot on the move is at half the odds**: `Combat::fire` takes
`moving`, a hostile room's `Shot` carries it, and the world hands it to
`enemy_fire`, so an enemy's bolt fired in the crew's room is rolled
by the same rule. Everybody shoots on the move now — the crew held
their fire while walking before — and what keeps the bots from doing
it is `plan_stand` (below). **Every one of those numbers lives in
`crates/game/src/balance.rs`** — the five `WeaponStats` and the three
`ArmourStats` as `const`s, the melee constants beside them — and
`WeaponKind::stats`/`ArmourKind::stats` are lookups into it; `combat.rs`
and `character.rs` re-export the constants so every old path still
reads. It is the one file to open for tuning; the tests that pin the
curves (`the_curves_pin_the_numbers_the_guns_were_asked_for`, the
app's `the_weapon_lines_say_the_curves_the_user_asked_for`) say what
moved. The app names kinds through `WEAPON_NAMES`/`ARMOUR_NAMES` and
prints the curves itself.

- **Combat mode is being recruited with a weapon**, and nothing else:
  `Game::tick_combat` (after the crew have moved, in `simulate`) sets
  `Character::set_armed(Option<WeaponKind>)` — recruited or braced, alive,
  armed, not outside or out cold, not dressing a wound, carrying or
  carried — which is drawing only:
  `draw_weapon` holds **every weapon in both hands, angled across the
  front** of the body (feature 82): `weapon_grip` puts the grip a little
  ahead of the two arms' average (`GRIP_AHEAD`) and over towards the
  firing shoulder (`GRIP_ACROSS`), the rear hand on it and the forward
  hand out on the fore-end, and the gun is laid at `CARRY` (0.30 rad)
  off the facing with its muzzle lit in the friendly-bolt blue — that
  being the **carry**, what a Bim with nothing to shoot at holds. *A
  little ahead* is load-bearing, and so is the short furniture behind
  each grip: the head is drawn before the weapon, so a stock that
  reaches back over it hides the hair, the nose and whatever the class
  wears on its face — feature 81's sunglasses went missing under an auto
  rifle exactly that way. **And a gun is drawn at `GUN_SCALE` (0.72) of
  the body's own size**, in the hands and on the deck alike
  (`draw_dropped` uses it too), because at full size down the centreline
  a rifle is a lance longer than the figure is wide. The
  schword is the same hold: a hilt (`HILT`) in both hands with the blade
  forward and angled in at the same `CARRY`, `draw_blade` — a white core
  (`BLADE_CORE`) between two cyan strokes (`BLADE_EDGE`, one wide and
  faint, one thin and bright), which is as near as a flat colour gets to
  a glow.

  **Aiming, the gun swings onto the target, and the shot leaves the
  muzzle** (feature 84). `Character::set_aim(Option<Vec2>)` is what the
  weapon points at, in room units, set by `tick_combat` the step it aims
  — beside `set_lean`, and cleared wherever that is — and
  `Character::gun_rot` is the one angle every drawing of the weapon is
  laid at: the carry with nothing aimed at, else the line **from the
  grip to what is aimed at**, held within `AIM_ACROSS` (1.15 rad) of the
  facing so a Bim firing as it walks somewhere else keeps the barrel on
  its target without the arms wrapping round its back. `on_gun`,
  `weapon_hands` and the blade's own line all read it, so the hands and
  the elbows follow the gun. Then `Character::muzzle()` is the emitter
  in room units — `muzzle_ahead(weapon)` along that barrel, the same
  number `draw_gun` lights the eye at, `None` for a blade or a body down
  — and **`tick_combat` fires from it** rather than from the body: the
  aim goes on the picture first, and the shot is read off it, `eye` (the
  body, or its peek)
  being only the fallback for a body with nothing drawn in its hands.
  Because the barrel runs from the grip *through* the target, a bolt
  leaving the muzzle flies exactly down it — the picture and the shot
  are one line, which is the whole of why the angle moved. What is
  rolled is unchanged: `Combat::fire_as` still takes the origin it is
  given for the odds, the miss and `damage_at`, and the muzzle is under
  a tile ahead of the body.

  **The arms reaching for it are limbs, not blobs, and they go down
  before the torso.** With the hands free an arm is one circle a side at
  `SHOULDER_ACROSS`, which is all an arm needs to be with nothing at the
  end of it; with a weapon up, `Character::draw_arm` runs a sleeve
  (`SLEEVE_WIDE`, rimmed `SLEEVE_RIM`) from the shoulder on its side out
  to that gun's own hand — `weapon_hands`, the one place that knows
  where the two hands on a given weapon are, the rear one on the right
  shoulder's arm — with the elbow swung out away from the body by
  `ARM_BEND` as far as the reach falls short of `ARM_REACH`, since a
  straight capsule to a hand a body-length away reads as a pole. They
  are drawn **under the coverall**, right after the boots: an arm seen
  from directly above is mostly shoulder, and laid over the torso the
  two of them hid the body they belong to. What shows is the forearm
  past the chest, and the head lies over an elbow rather than under one;
  only the gun goes on top, at the end. An armed Bim asks
  `Combat::aim` for the nearest target in range that `Sight::sees_from`
  says it sees, faces it, and fires when its trigger is ready — on the
  move too, at the walking odds. **Recruiting is per Bim**: `pump_queue`
  and `consider_errand` ask `bims[who].character.is_recruited()`, not
  `Game::is_recruited()` (the player's) — they used to ask the player's,
  and recruiting one Bim froze every crewmate's errands with it.
- **What a gun looks like is `draw_gun`, and only there.** One free
  function takes a `Brush`, a grip and a `WeaponKind` and lays the gun
  along the frame's `+x`; the gun in a Bim's hands and the one lying on
  the deck where a body dropped it both go through it, so there is one
  place to change a gun and no second copy to forget. Its `lit` argument
  is the emitter's glow at the muzzle — a gun on the deck is cold.
  Seen from above every piece is a block along the length, and what
  would hang *under* the gun is canted out to the side instead: the auto
  rifle's magazine, the sniper's two bipod legs. What tells the four
  apart before any detail does is `gun_reach`, how far ahead of the grip
  each one goes — the pistol 13.5 against the auto rifle's 33 and the
  sniper's 44 — and `fore_hand` is where the forward hand sits on each.
  Both are in the **gun's own frame**, which every drawing of it enters
  at `GUN_SCALE` of the body's size, so the whole rack is made bigger or
  smaller at that one knob and no block in here is renumbered for it.
  Then the detail: the pistol a stubby slide and a butt and *nothing
  else*, since everything a pistol has it has less of; the shotgun a
  wide bore over a wooden pump (`STOCK`) with the butt braced behind;
  the auto rifle a rifle — a vented handguard, a canted magazine, a
  sight rail (`SCOPE`) down the top and a brake (`GUN_EDGE`) on the end;
  the sniper the long one, a thin barrel out of a bipod with a cheeked
  stock and a scope down the middle whose objective lens (`LENS`) is
  what says *sniper* before the length does. `crates/app/src/icons.rs`
  draws the same four from the side for a cell of the lockers, and the
  note there points back here: change a gun in one and change it in both.
- **The targets are the world's, and each comes with its weapon.**
  `Combat::targets` is `Vec<Option<combat::Target { at, weapon, peeking, stale, dodge
  }>>`, index for index with the other room's people (`None` for one
  down), set every step by `World::visit` through `Game::set_hostiles(
  Vec<Option<(Vec2, Weapon)>>)` while the station is hostile — on
  **both** rooms, each told where the other's are — and cleared
  otherwise. The weapon is what says which targets lock a gunner (below).
  `peeking` is a **second call**, `Game::set_hostiles_peeking(&[bool])`
  → `Combat::set_peeking`, index for index, because the tuple the world
  hands over has no room for it; nobody is peeking until it is called,
  so the world calls it right after every `set_hostiles`. `at` is the
  target's **exposed** position — the peek it leans out to while it
  peeks, else where it stands (`Game::exposed_at`) — and
  `Combat::target_positions()` is the plain `Vec<Option<Vec2>>` the
  tactics read.
- **A trigger pull is a burst.** `reload` out and a target seen: `reload
  = 1 / fire_rate`, `burst_left = burst − 1`, `burst_timer = burst_gap`,
  and the first shot goes now; every `burst_gap` after, while
  `burst_left` is up, another, each rolled on its own with the aim
  **re-read** (`aim` is asked every step, so the shot follows the target).
  The rifle's eight therefore take 1.75 s from the pull and the recharge
  is the rest of the four; the app rounds it to the "8 in 2 s, then 2 s"
  the user said. A walk, a lost target, a lock or the weapon going away
  zeroes `burst_left`, so a burst is never resumed on arrival.
  `a_burst_is_eight_shots_in_two_seconds_and_then_a_gap` in `game::tests`
  pins it.
- **A bolt flies in one room only.** The crew and a station's people are
  two rooms, and a bolt that flew in both would be two bolts. So a
  friendly bolt (`hostile: false`) looks for the targets, a hostile one
  for this room's **own bodies** — `Combat::step(dt, sight, bodies)`,
  `bodies[i]` `Some((position, peeking))` for Bim i while alive and on
  the deck, conscious or not, at its **peek position while it peeks** —
  and a room whose bodies are hostile (`Game::set_hostile_bodies`)
  fires **no bolts**: its people's shooting is a `combat::Shot { from,
  at, weapon, melee, damage, cut }` on `Combat::shots`,
  `Game::take_shots()`, which the world maps into the crew's room and
  fires there as a red bolt through `Game::enemy_fire(from, at, weapon)`
  → `Combat::fire(.., true)`. A `Bolt` carries its `kind` (its picture
  and its curve) and `fired_from`, and **the damage is worked out where
  it lands**, `damage_at(tiles flown)`, not at firing — so an enemy's
  shot, fired in the crew's room, falls off by the same rule as the
  crew's. `fire` rolls only *whether* it hits, `hit_chance(tiles to the
  target)`, and aims a miss `MISS_BY` wide. A friendly bolt landing
  pushes a `combat::Hit { who, part, damage, cut }` onto `hits`
  (`Game::take_hits`, for the world to carry to the residents' room's
  `Game::strike`); a hostile one onto `Combat::wounds_taken`, which
  `tick_combat` drains into `Game::strike` at once and keeps a copy of
  for `Game::take_wounds_taken()` — the world logs `CrewHit` off that,
  it does not apply it. The **part** is rolled the instant the bolt
  reaches the body, `Part::hit_by(rng.unit())` off the combat stream.
  Each gun's bolt has a look of its own in `Combat::draw`, and since
  feature 98 **every one is a laser in its side's colour** — see "The
  fight's passing lights" at the end of this file for the shapes, the
  muzzles, the flashes and the scorches.
- **A peek exposes the peek.** When `aim` answers with an eye that is
  not the body — `Sight::eyes_from`'s peek beside a wall — `Bim::peek`
  is that eye and `Character::set_lean(Some(eye))` draws the body at
  `pos + LEAN (0.55) · (eye − pos)` (`Character::drawn_at`, which the
  body, its rings, its held things and the hit flash all go through)
  turned to face down the corridor from the eye. `Game::peek(who)` and
  `Game::exposed_at(who)` are the readouts, and the exposed position is
  what the world hands the other room as the target's, so it is shot at
  where it leans out and not at the wall. Being in cover, a bolt
  reaching a peeking body is **dodged** with `DODGE_IN_COVER`: the bolt
  remembers the one body it slipped past (`Bolt::dodged`) so a bolt
  crossing a body over several steps is rolled once, and flies on. The
  same for a target the world marked peeking. Standing square, walking,
  locked or holstered, `peek` is `None` and the lean is off.
  `a_body_peeking_from_cover_dodges_half_the_bolts` in `combat::tests`
  is seeded both ways.
- **A melee lock is a distance, and it comes before aiming.**
  `Combat::melee_with(sight, from, own)` is the enemy a body is in a
  melee with: the nearest target within `MELEE_RANGE` that it can see —
  **any** target for a body with a blade, and only a target *carrying*
  a blade for a body with a gun, since a gunner is locked by a blade at
  its throat and not by a pistol beside it. `tick_combat` asks it first,
  every step, and `Bim::locked` is the answer (`Game::is_locked(who)`):
  locked, the Bim neither aims nor fires, its burst is over, it faces
  the enemy, and every `MELEE_PERIOD` on `melee_timer` it lands a blow —
  `Combat::brawl(from, target, weapon, damage, cut, as_shot)`, a `Hit`
  straight onto `hits` in the crew's room (part rolled then) or a melee
  `Shot` in a hostile one (`as_shot`, because `Combat` does not know
  `hostile_bodies`) — `stats.damage` with `cut: true` and an
  `Action::Swing` for a blade, `FIST_DAMAGE` with `cut: false` and an
  `Action::Punch` without, either antic `SWING_TIME` (0.5 s) long. **A
  swing is not a hit until it has been swung**: the step the timer runs
  out puts a `combat::Blow { target, left: SWING_TIME, damage, cut }` on
  `Bim::blow`, and `brawl` is called only when `left` has run out — and
  only if `Combat::within_reach(from, target)` still says so, so a body
  that stepped back during the swing is missed and the swing lands on
  nobody. The blow counts down whether or not the lock still holds, and
  holstering (`weapon` going `None`) drops it. So the first blow lands
  half a second after the lock forms, since `melee_timer` starts at
  nothing and the swing starts then;
  `a_blow_lands_when_the_swing_ends_and_misses_a_target_that_stepped_back`
  pins both halves. A blade with nobody in reach does nothing at all — no aim,
  no fire — and walking out of reach ends the lock, since nothing but
  the distance keeps it. The world delivers a melee `Shot` to the crew's
  room as `Game::enemy_strike(from, who, damage, cut) -> bool`, which
  re-checks the reach in the receiving room — `MELEE_RANGE` plus half a
  tile of slack for the step between the two rooms — rolls the part off
  its own stream (`Combat::struck`), applies it and keeps it on
  `wounds_taken` so `CrewHit` is logged. `Action::Swing` draws the blade
  swept through `SWING_ARC` (100°) in front of the body with a fading
  trail behind it; `Action::Punch` is the right fist out and back with
  the left up as a guard.
  `a_blade_within_reach_locks_the_gunner_who_stops_firing_and_lands_fists`
  and `a_blade_enemy_charges_and_its_blow_is_a_cut_that_splashes_the_deck`
  in `game::tests` pin both ends.
- **`Game::strike(who, part, damage, cut) -> WoundOutcome`** is the
  general wound — `Game::wound(who, part, damage)` is kept as "a shot",
  `strike(.., false)`, for the tests and `scratchpad/layout.rs`. The
  armour on that part first (below), then `Health::shot(part, damage,
  cut, roll)` with what got through: the damage off that part, and a wound
  opened on it in **units** — one for a shot, `CUT_WOUND` (3) for a cut,
  since `BLEED_PER_WOUND` is per unit and a blade is meant to bleed three
  times what a bolt does; a bandage still closes the lot on a part —
  the `trauma` the part rolled if it reached nothing (`roll` off the
  combat stream, `Combat::roll`; see "A part at nothing is a dying
  state" below) and `leg_lost` when that trauma is a crushed leg, the
  flash, and `Character::set_wounds` — a
  `BLOOD` blotch on the head, the coverall's middle or the boots while
  that part bleeds, standing or lying. Then the blood: `drip_timer` is
  zeroed so the first drop lands under the body at once, and a **cut
  splashes**, `Blood::splash(at, rng, can_get_to)`: the tile under the
  body always, plus `SPLASH_TILES` (3 to 5, counting it) of the nine round
  it, each picked out of what is left so no tile is bloodied twice, through
  a `can_get_to` filter (blood flung under a counter's lip or into the
  corner past a bunk is blood on deck no body can reach; `strike` passes
  `nav.can_reach`). `part_health`, `blood`, `wounds(who, part)`,
  `bleeding`, `legs_lost`, `trauma(who, part)`, `is_dying`, `lasting`,
  `is_unconscious` are the readouts. `tick_bim`
  multiplies the pace by `Health::pace()` (the legs lost, the blood
  under half, and every trauma on it) and the effort by
  `Health::works_at()`; after the death check, `Health::unconscious()`
  differing from `Character::is_unconscious()` is `knock_out(out)` —
  going out `interrupt`s the errand onto the queue first **and drops
  the gun** (`drop_weapon`, below), and the frame stops there for that
  Bim until the blood comes back (`OUT_AT` is **half its
  blood** since feature 89, with `SLOWED_AT`'s half pace moved up to
  three quarters so the band above it is still walked). **A body down
  does not lie down** (September 2026, the user's word: "they don't have to
  lie down … just very distinguishable from alive and walking bims").
  Both states are `draw_down`: the **standing figure at the standing
  size, gone slack** — boots splayed, the arms fallen out wide of the
  shoulders, no gun, the head lolled onto one shoulder and turned on its
  cheek (`draw_hair_lying`), the class's kit left off — and every colour
  on it put through a tone. **Dead** (`draw_fallen`) is `ashen`: every
  colour most of the way to `ASH`, a dark slate grey — darker than a
  machine's pale plating, so a corpse is never read as a Husk — over a
  dark `POOL` of blood, and still. **Out cold** (`draw_lying`) is
  `faded`: half the way to grey and dimmed, so it is still plainly its
  side's, with a slow breath and `DAZE_STARS` (three) yellow stars
  circling the head (`DAZE_TURN`) — the one mark no other state has.
  Colour, the pool and the stars are what tell the three apart from
  across a room; they were a half-size sprawl lying along the heading
  (`draw_flat`, `FLAT_SCALE`) until then, which at a fight's zoom read
  as one more small standing Bim. The first cut drew the dead as the
  standing figure in cold greys too, which the user remembered as the
  one that worked. A click on a body down is the standing circle again
  (`Character::picked_at`). `scratchpad/layout.rs dead` puts both
  figures side by side. `Bim::tick_drips` drips **blood on the
  deck** every `DRIP_EVERY / wounds` seconds while it bleeds and lives,
  scattered `DRIP_SCATTER` (±10) off the body by two rolls off the room's
  stream — the first of the blood's three draws, which every fight makes
  ("The needs deleted" at the end) — and a drop takes `BLOOD_COST` (sixty)
  off the tile it lands on (`Blood::drop_at`), so a Bim standing still
  bleeding takes the tile under it to `FOULED` in a couple of drops. There
  is no picture of a drop of its own; `Blood::draw` paints a stained tile
  in a dark red wash with blobs on it, and nothing takes it up again.
  `Game::bloody_tiles()` counts the stained tiles for the tests.
  `a_cut_bleeds_three_units_and_a_bandage_closes_the_lot` in
  `health::tests` pins the units.
- **The crew's alarm is the other side of the war switch.** A room whose
  bodies are not hostile is **alarmed** (`Game::alarm`, `is_alarmed`)
  while any target the world named is within `ALARM_RANGE` (30 tiles)
  of any living crew member, or was in any waking crew member's sight
  within `ALARM_HOLD` (30 s, `enemy_unseen_for`), or a crew member was
  hit within as long — `attacked_for` is re-armed by a hostile bolt
  landing in `tick_combat` and by `enemy_strike`. **The alarm is a
  fight, not a berth**: the range was a hundred tiles — any enemy on
  the station — and a crew docked at an enemy's stood under arms for as
  long as they were tied up there, whoever was left alive at the far
  end of it, and under arms nobody takes an errand of its own
  (`consider_errand` and `pump_queue` both bow out for `is_recruited()`).
  Thirty is half again `CALM_RANGE`, so between the two a crewmate is
  under arms *and* doctors, which three tests lean on with an enemy
  twenty-five tiles off beyond a bare room's walls. The other thing that
  keeps a crew down after a fight is not the alarm at all: a body that
  bled past the line is **out cold** on the deck until somebody bandages
  it, and that is the medical system. Going up, `muster_crew`
  puts every living crew member **but `PLAYER`** under arms the way
  `muster` does an enemy's people — errand interrupted, post dropped,
  `plan_wait` zeroed — and first takes a weapon out of the pack into an
  empty hand (`equip` on the first `Item::Weapon` cell), since a recruited
  body with nothing in its hand stands still. **They keep to the player's
  side until they see an enemy for themselves**: every step a crewmate
  under the alarm with no post either runs `plan_stand` like an enemy —
  when `Combat::sees_any(sight, from)` says a target is in its sight at
  any range, or the player's Bim is dead or outside — or `gather`s: a
  slot in the ring round the player's Bim (`GATHER_SLOTS`, by rank among
  the crew, the first slot round that `Nav::can_reach` from where it is,
  since one inside the wall the player stands against is nobody's), walked
  to on its own `plan_wait` clock when it is more than `GATHER_SLACK` off
  it, not a post, so the ring moves with the player. It shoots what it
  sees from there like any recruited body. **And the crew take orders,
  but only then**: `order_move` acts on whichever Bim is selected — the
  player's own always, a crewmate only while `alarm` is up
  (`order_move_for` is the old body with `who` for `PLAYER`) — and a
  crewmate ordered somewhere gets a **post** at the spot, which is what
  keeps the gathering and the tactics off it; `muster_crew` clears every
  crewmate's post on both edges of the alarm, so the order lasts as long
  as the alarm. Going down they are let go and the queue picks their
  errands up. The player's own Bim is never touched: recruiting it is the
  player's (`toggle_recruited`), and a hired mercenary is a bot like any
  other. Consequences for tests: in a fight every crew member takes arms,
  so a test that counts one Bim's hits or
  bolts issues the others `Gear::default()` (no weapon) — the blade lock,
  the bandage-holsters and the shoot-on-the-move tests in `game::tests`,
  the sniper's long shot in the world's. `the_crew_take_arms_when_an_enemy_comes_within_range_and_stand_down_after`
  pins the range, the pack, the stand-down and the hold;
  `under_the_alarm_the_crew_gather_round_the_player_and_take_orders` the
  ring, the following, the order and its post. The header says
  `ALARM_STATUS` while it is up. **Both musters are
  on an edge, and a room is thrown away at every dock and undock**, so
  `take_crew` stands the war and the **muster** down first: a body
  carried into a fresh room — which starts at peace — still recruited
  had nothing to let it go, and the crew went home from a fight still
  in combat mode. The new room musters them again the step an enemy is
  in range, or the step it reads a player's standing order.
  **It is `mustered` and not `alarm`**, and it was the alarm alone from
  feature 84 — which split the two — until this was found: a crew mustered by a player
  leading them rather than by an enemy — a weapon drawn, an attack
  banner down — went through with `alarm` false and the edge already
  spent, and so did a squad's, whose `squad_armed` is rebuilt empty in
  the new room and can therefore never let anybody go either.
  **Recruited and not mustered is a body that does nothing at all**:
  `consider_errand` and `pump_queue` both bow out for it and
  `bot_stand` is not reached, so it stands where it was put for ever.
  `a_room_taken_apart_stands_down_a_crew_a_player_was_leading` here pins
  the leading half; the alarm's half was pinned by a world test that cast
  off from a hostile station, which went with the flown trip and the human
  enemies in feature 104.
- **Sandbags are low cover.** `PartKind::Sandbags` is half a body's
  height since September 2026: `blocks_movement: false`, so the nav grid
  walks over it and (`blocks_sight` following) a line of sight goes over
  it too, and `shipdesign::is_cover` marks it the one part that is cover
  of this kind. `Layout::cover` collects them and the room hands them to
  `Sight::set_cover` (part of the fixed picture, so `set_shut` keeps
  them). `Sight::covered(body, from)` is the rule: a cover tile on the
  straight line from the body towards `from` within `COVER_REACH` (1.5)
  tiles of the body — the tile beside it, diagonals in — and the body not
  standing on the bags itself. Two places read it: `Combat::step` dodges
  a bolt reaching a covered body with `DODGE_IN_COVER`, the same roll as
  a peek, for its own bodies and for the targets alike, each room off its
  own mask (the joined deck has the station's bags; the residents' room
  its own); and `Tactics::view_from` scores a spot the body sees the
  target from *through* its bags as cover, `COVER_WORTH`, so a bot picks
  the tile behind a barricade over the open. The stations lay one in
  each arm (`crates/world/CLAUDE.md`).
  `sandbags_are_a_stand_the_tactics_take_and_half_the_bolts_over_them_are_dodged`
  in `combat::tests` and `sight::tests` pin it. A body *on* the bags is
  in the open, and one two tiles back is past the reach — a bolt comes
  over.
- **The enemy's tactics.** A room with hostile bodies **and** a `Some`
  target is **at war** (`Game::at_war`, `muster`): every living body is
  `set_recruited(true)` with its errand `interrupt`ed (queue kept) and its
  post dropped; targets cleared is `set_recruited(false)` and the queue
  picks up. At war each armed body runs `plan_stand` every `PLAN_EVERY`
  (1.5 s, its own `plan_wait`): `combat::Tactics::stand(sight, nav, from,
  targets, stats, doorways)` — `targets` the combat's whole `Target`s,
  weapon kinds included, `doorways` the room's doors and airlocks, none
  of them a stand — scores a tile-spaced lattice of free cells within the
  weapon's reach of every target (`Nav::free_cells_within(centre, radius,
  spacing)`, kept to `can_reach`) plus the spot it stands on, **in tiles
  of walking**, so every weight reads against the walk: **cover** (the
  body's own eye blind to the target, a peek beside a wall seeing it) is
  `COVER_WORTH` (15) over the **open** (the body's eye seeing it), so the
  cover near it wins and the cover across the station does not; the
  **weapon's fit** (`Tactics::fit`: its own `dps_at` the distance less the
  target's weapon's, as a share of the better of the two at nought, −1 to
  1) is worth `FIT_WORTH` (30) either way — the sniper rifle at twenty
  tiles, the shotgun inside four, the auto rifle just past a pistol's
  twelve, every gun out of a blade's reach, and two pistols a match; then
  `DISTANCE_WORTH` (1) a tile of range for what is left; less
  `WALK_COST_PER_TILE` (0.5) a tile of walking, plus `STAY_BONUS` (2) for
  the spot it is on so equals do not have it dithering — and marches
  there when it is more than a tile from where it is already going
  (`follow_path`, no marker: a ping through the fog). A **blade charges**
  instead: `stand` hands a melee weapon to `Tactics::charge(nav, from,
  targets)`, the free cell it can reach nearest the nearest target, every
  cell within `CHARGE_LOOK` (3) tiles of it a candidate rather than the
  lattice, since the lattice's nearest cell can be half a tile short of
  reach and a blade wants to stand *beside* its target; cover means
  nothing to it. It **shoots when it can**: the moment `aim` sees a
  target, mid-plan or not, and on the move too — from its own eyes only,
  the walk keeping the facing, at half the odds; a blade swings the
  moment `melee_with` says so. **A bot with a shot stands still for it**:
  `plan_stand` reads `Tactics::stand_with_cover` (`Stand { at, cover }`,
  `stand` being the same less the flag) and, when `aim` has a target
  from where the body is and the stand it picked is not cover, halts
  the walk and stays — walking would halve its odds — so a bot moves
  only for cover or when it has no shot at all. The crew's bots under
  the alarm run the same `plan_stand`; the player's own Bim walks where
  it is sent and fires as it goes.
  `a_bot_with_a_shot_stands_still_and_the_player_s_bim_fires_on_the_move_at_half_the_odds`
  in `game::tests` pins both halves and the odds. `Eye::is_peek`,
  `Eye::admits`, `Sight::tile_of` and `Sight::clear_line` are public for
  it. **Enemies know only what they have seen.** The world hands a
  hostile room every crew position, and `Game::set_hostiles` there
  keeps `last_seen` a target: one any of the room's living, waking
  people on the deck `sees_from` (any range) is known where it is; one
  nobody sees is believed where it was last seen and handed to the fight
  **stale** (`Combat::set_stale`) — the tactics walk there, through the
  passage and onto the ship if that is where it went, while `aim`,
  `sees_any` and `melee_with` skip a stale target so nobody shoots at a
  belief; and one unseen for `FORGET_AFTER` (60 s) is `None`, so with
  the last crew member out of sight for a minute the room leaves war and
  its people go back to their day. The crew's own room takes the list
  as it comes. `a_hostile_room_chases_what_it_last_saw_and_forgets_it_after_a_minute`
  pins it. (The chase across the seam onto the ship was the world's, and
  its test went with the human enemies; the machines' own beliefs go
  through the same `believe`, "The machines have a list of their own".)
  **And the airlock is watched** (since September 2026): `Game::watched`
  is a spot the room's people have eyes on whatever they are doing —
  `set_watched(Some(p))`, which the world sets to the tile just inside
  the station's door while the ship is docked (`Residents::join`) and
  the room otherwise has none — and `set_hostiles` counts a target
  within `AIRLOCK_WATCH` (2.5) tiles of it as *seen*, not stale, with
  nobody looking. A boarding is therefore what starts the fight at a
  held station, at the door, rather than the moment one of its machines
  happens to look down the right corridor; a crew that stays aboard its
  own ship (the ship's airlock is three and a half tiles from the spot)
  is still nobody to them. A shot still wants real sight, so nothing
  fires at the door from across the station.
  `a_hostile_room_s_airlock_is_watched_and_a_boarder_at_it_is_seen_by_nobody_in_particular`
  (game) pins it.
  `an_enemy_stands_where_its_weapon_beats_the_target_s` (combat) and
  `a_bot_with_a_shot_stands_still_and_the_player_s_bim_fires_on_the_move_at_half_the_odds`
  (game) pin the two.
- **The bots follow a player, and a player has two orders for them**
  (feature 84). What decides what a crewmate nobody steers does with
  itself is one branch, `Game::bot_stand`, reached when the crew are
  **under arms** and nothing nearer to hand has claimed the body — a
  chain, a patient being seen to, a commander's squad order, or a post
  its own player right-clicked for it, each of which outranks it in that
  order. Three things in it:

  * **Under arms is no longer the alarm alone.** `Game::mustered` is
    `alarm || led()`, and `led()` is any player's own Bim alive, on the
    deck and **recruited**, or any player's standing order something
    other than following. So drawing your own weapon musters the crew
    behind you — a weapon out of the pack, the armour on for a blade,
    the errand put down, exactly as an enemy coming within
    `ALARM_RANGE` does — and holstering it stands them down to their
    errands again. `muster_crew` is edged on `mustered`, not on `alarm`;
    `is_alarmed` is still the alarm proper, which is what the header
    says and what `ALARM_HOLD` holds. `orderable` reads `mustered` too,
    so a crewmate takes a right-click whenever it is under arms rather
    than only in a fight.
  * **The order is the world's, one a player slot** —
    `game::Standing::{Follow, Attack { at }, Retreat}`, handed over every
    step by `Game::set_standing` and never saved, like `Squad`. **Whose
    order a bot is under is whose Bim it is nearest** (`standing_for`):
    with one player that is the one order there is, and with several each
    player leads the bots about them. *Follow* is the ring round the
    nearest player that is up and in (`gather`), broken off for its own
    `plan_stand` the moment it sees an enemy for itself — which is what
    the alarm alone used to do. *Attack* is `assault`: anything up within
    the weapon's reach and it fights its own stand, cover and all;
    nothing in reach and the banner more than `BANNER_HOLD` tiles off and
    it **pushes** (`Tactics::advance` — the reachable cell within
    `ADVANCE_LOOK` that gets it nearest, a tile of ground made good worth
    `GROUND_WORTH` against `COVER_WORTH` for cover from the nearest
    target, never a doorway, never a cell of its own side's, and the
    whole walk planned instead where nothing near is nearer, which is a
    banner round a corner or out on a plain); nothing in reach and the
    banner reached and it holds the ring round the banner. *Retreat* is
    that ring round `ship_anchor()` — the deck just inside the port —
    and, unlike a commander's *fall back*, it does **not** hold its fire
    on the way: a crew walking home under fire that would not shoot back
    is a crew that does not get home.
  * **Where the ship is, is the world's word.** `ship_anchor()` reads
    `Game::home` — `Game::set_home`, said every step from
    `Aboard::gangway` (`crates/world/CLAUDE.md`) — and only falls back
    to the room's own `Room::gangway`, which is right for a ship flying
    alone and **wrong on a joined deck**: `gangway` there is the joined
    design's first *free* airlock, and the ship's own is mated to the
    station, so the room's answer is the station's far door at the other
    end of the building. That is what a retreat used to walk the crew
    out to, which is what the key looked like doing nothing at all.
    `World::fall_back_point()` is the same spot for the app's defend
    sign, so the mark and the walk cannot disagree.
  * **A fall back is walked its own way** (`character::FallBack`, the
    one thing on the body that says so). `fall_back_aboard` marks the
    body at a **sprint** — `SPRINT_PACE`, head down, a little faster
    than a walk — and the aim turns that into **backwards**,
    `BACKSTEP_PACE` with the facing the enemy's, the step it finds
    something to shoot at, so a crew member covering the retreat gives
    ground with its gun up and one with nothing in front of it runs for
    the airlock. In `Character::update` the facing and the feet part
    company for it: the heading goes to the fall back's angle and the
    body moves along `intent`, the route's own direction, with the
    stride run backwards so the legs read as stepping back. A
    commander's *fall back* is a sprint too, since it holds its fire.
    The flag is cleared for every body at the top of every
    `tick_combat` and set again below, so nothing carries over, and it
    is `serde(skip)` for the same reason `aim` is.
  * **The ship is the last stand.** `Game::is_aboard(p)` reads what
    `set_foreign` said — the station's box is somebody else's and the
    rest the ship's, or on a planet the ship's own box against the town
    — and `cornered(who)` is aboard with a target up aboard as well.
    A cornered body runs `plan_stand` whatever its order was, and
    `would_flee` is false for it, so a dying crew member with the enemy
    in its own hull stands and shoots instead of walking deeper in.
    **A room with no foreign half is not a last stand**: a bare room and
    a ship on its own have nowhere else in them, so
    `cornered` is false there and the dying run is what it always was.
    The other half is `flee` itself: a dying body **of the crew's side**
    outside the ship makes for the gangway rather than merely away from
    the enemy, where there is a way there. An enemy's people have no
    ship and are `Tactics::flee` throughout.

  `Standing` is in `world_checksum` (`crates/world/CLAUDE.md`), so all of
  this is a command and not a click.
- **What a station's people carry is rolled, not kept.**
  `Gear::issued_for(seed)` draws one roll off `Rng::new(seed)` against
  `ISSUE_ODDS` — pistol 0.50, shotgun 0.20, auto rifle 0.15, sniper 0.05,
  schword 0.10 — and issues that and nothing else, **unless it is the
  schword: a melee bot always wears the basic armour**
  (`Gear::basic_armour`, a fresh helm, kevlar and leg guards, ids 1–3,
  the room's own until a loot renumbers one), since a body of 75 with a
  blade is dead before it closes. `Gear::hired_for` does the same for a
  mercenary rolled the schword (its armour rolls are drawn either way,
  so the stream is the same), and a crewmate bot drawing a blade at the
  alarm puts on every whole piece in its pack over a part with nothing or
  a broken piece (`muster_crew` → `wear_what_it_has`) —
  `a_crewmate_with_a_blade_puts_the_armour_in_its_pack_on_at_the_alarm`
  pins all three. A function of the seed alone, so the world derives it (`Residents::open`, `map_seed ^ who`)
  and `world_checksum` hashes nothing new. `Game::issue(who, gear)` puts
  it in the hand and refreshes the picture; `Game::weapon(who)` and
  `Game::gear(who)` read it back, and the app's `BIMS_WEAPON`,
  `BIMS_ENEMY_WEAPON` and `BIMS_ARMOURED` dev hooks go through those two
  rather than a probe hook here.
- **The combat RNG is its own stream** (`Rng::new(seed ^ 0xC0BA7)`): a
  fight rolls nothing the rest of the room rolls, so the room's stream
  is the same whether or not somebody shot. The one exception is the
  blood on the deck — the drips' scatter, the boots' crossing and a
  cut's splash come off the room's stream — and since every fight
  bleeds, those three draws are part of what the survivor tests pin
  ("The needs deleted" at the end).
- **`Character::hostile`** rings a body in red; the world sets it on every
  Bim of a hostile station's room (`Game::set_hostile_bodies`) — a room
  that, in a run, has no Bims left to ring.
- **`HIT_BIM = 11`**: `Game::hit_at` answers it, after noting the
  fixtures and before asking the room, for a click within `pick_radius`
  of a living Bim, and `Game::hit_bim()` says which — the bandage menu.

Two things about the seam that bite, found while the world's and the
app's halves were built and not fixed here:

- **The crew's room steps before the residents'** (`World::step`), and
  each room reads the *other's* positions as they were handed over last
  step. So the step a charging blade or claw arrives, the crew member's
  `melee_with` still sees it a tile off and does not lock, while the
  charger's own `melee_with` — in the residents' room, stepped after —
  sees the crew member in reach, lands its first blow that same step
  through `visit`, and only next step does the crew member's lock form:
  the first blow of every melee is unanswered. Since a blow lands
  `SWING_TIME` after the swing starts, the world's claw test
  (`a_claw_charges_and_locks_the_crew_member_who_fights_with_its_fists`,
  a tier-two Warden so that no single fist wrecks it) runs on step by
  step until the fist lands on the machine rather than reading it the
  step of the lock.
- **A doorway is never a stand.** `Tactics::stand` scores a spot by
  `Sight::eyes_from` *from that spot* against the doors as they are
  now, and a shut door's leaf is opaque — so from afar a doorway tile
  read as cover (the other leaf a wall to peek beside) and so did the
  corridor tile against a shut door; but a door opens for a body
  within `door::REACH` of it (64 units, more than a tile), so the
  moment the enemy arrived the "wall" was gone, `aim` was `None`, and
  it re-planned to the mirror doorway across the corridor every 1.5 s
  for ever — the default-seed station's sniper never fired. `stand`
  therefore takes the room's doorways (every powered door and airlock,
  from `plan_stand`) and drops every lattice cell within `door::REACH`
  of one; the spot the body stands on is still scored as it is, since
  that is the real view.
  `a_doorway_is_never_a_stand_since_it_opens_for_whoever_comes` pins
  both halves on a hand-laid room, and the world's sniper test
  (`a_sniper_rifle_reaches_from_twenty_tiles…`) has a Warden with the
  rifle walk to its range and land from past a pistol's reach.

`combat::tests` pin the tactics on a hand-laid walled room, which bodies
each kind of bolt finds, the curves
(`the_curves_pin_the_numbers_the_guns_were_asked_for`,
`damage_falls_off_with_distance_and_a_body_peeking_or_behind_sandbags_dodges_half`), the dodge, the
issue (`an_issued_gear_rolls_every_kind_across_seeds_and_the_same_for_a_seed`)
and the charge and lock rule
(`a_blade_charges_the_nearest_target_and_a_gun_is_locked_only_by_a_blade`);
`game::tests` pin the war, an enemy's shot wounding and the tile under
the bleeding body stained, the burst, the lock, the charge and the cut's
splash, the knock-out and `HIT_BIM`, on a bare room.

## Armour is clothes, and a piece is a thing

`combat::ArmourKind` — `BasicHelm = 1`, `BasicKevlar = 2`, `BasicLegs =
3`, each cut for one `health::Part` (`slot()`), each a `ResourceId` code
in the hold (`resource()`: 14, 15, 16 since the fuel resource went — a number, since this crate does
not know `physics`), each with `stats()` of `health` and `protection`:
(15, 2), (20, 2), (10, 1). A **piece** (`combat::Piece { id, kind, tier, health
}`) is one instance of one: the `id` is the world's and only climbs, so
the piece in the hold and the piece on a body are the same piece and
its damage goes with it. `broken()` is health at nothing;
`effective_protection()` and `bonus()` are nothing then. The rule of a
piece in a container being a resource and a piece anywhere else being an
instance is the world's (`World::pieces`, `crates/world/CLAUDE.md`); the
room only ever holds instances — on a body, or in its pack.

- **The hit order is `Game::strike`'s and nowhere else** (`Game::wound`
  is the same call for a shot). The worn piece
  on the part, if any and not broken: its protection comes off the
  damage — nothing left is nothing, no wound, and `absorbed` is the whole
  shot; else the rest drains the piece's health, and only what the piece
  could not take (`through`) reaches `Health::shot`, which is what opens
  a wound. A piece at nothing is broken — still worn, still drawn,
  doing nothing — `piece_broke` is set, and `(who, kind)` goes on
  `Game::take_pieces_broken()` for the world, since a hostile bolt is
  applied by the room itself and the world never sees the outcome.
  `armour_health(who)` is the unbroken worn pieces summed — the blue bar
  on the end of the green one — and `part_bonus(who, part)` one part's.
- **The pack is `Gear::pack`, `[Option<Item>; PACK_CELLS]`** (ten across by
  five down — seven by seven at feature 49, September 2026, widened the
  same month — row by row), an `Item` being
  `Armour(Piece)`, `Weapon(Weapon)`, `Stack(resource code)` or `Key(tier)`
  — **each thing kept in the cell its top-left corner is in and reaching
  over its `Item::footprint()`**, turned a quarter round where
  `Gear::turned[cell]` says, the way the world lays the lockers out. The
  footprint table is the world's (`economy::footprint`) said again by
  resource code, since this crate knows no `physics`, and
  `the_pack_lays_things_by_the_same_footprints_as_the_lockers` in the
  world pins the two together; a key is two tall, a rifle seven along, a
  vest four by four. `Gear::head_of(cell)` is the corner of whatever
  reaches over a cell, `occupied` whether anything does,
  `fits_turned(cell, item, turned, ignoring)` the one rule (on the grid,
  no wrap round the edge, nothing under it bar the thing being moved),
  `first_fit` row by row unturned everywhere before turned anywhere,
  `put` lays a thing the way round it fits, `take_out` takes it by any
  of its cells, and `rearrange(cell, to, turned)` is a drag —
  `Game::rearrange`, the world's `Command::Repack`.
  `a_thing_lies_over_its_footprint_and_turns_to_fit` pins it. **`Gear`'s
  `Default` is by hand**: an array of forty-nine has none of its own.
  `Game::give(who, cell, item)`
  (the first place it fits for `None`) and `Game::take(who, cell)` are the
  world's two halves of a fetch and a stow; **`take` refuses a broken
  piece** and leaves it in the cell, because the hold counts pieces as
  resources and a broken one is worth nothing there — `Game::discard`
  is the only way out for it. `Game::equip(who, cell)` puts a piece on
  the part it is cut for and drops what was worn back into the cells it
  left — or the first place it fits, and **refuses the whole swap when
  nowhere does**, so nothing is ever lost — (a weapon — `Item::Weapon(weapon)`,
  any of the five at any tier — swaps with the hand; a stack is refused),
  returning what came off; `Game::unequip(who, part)` takes it off into
  the first place it fits. Both refuse a dead Bim; neither cares where the Bim stands —
  reach is the world's check (`Game::within_reach(who, container,
  tiles)` and `container_frame` are there for it).
- **The picture is `Character::set_worn([Option<Worn>; 3])`**, `Worn {
  kind, broken }`, drawing only, refreshed by `Game::refresh_worn` from
  the gear whenever it changes — equip, unequip, a piece breaking. A
  helm is a steel-blue cap over the hair, kevlar a dark plate set
  forward over the torso with the yoke still showing behind it, leg
  guards the boots darker with a band across the shin; a broken piece
  has a light diagonal stroke (`CRACK`) across it. A body down
  wears the same. The field on `Character` is `armour`, not `worn` —
  `worn` was already the coverall a suit goes back over — and the app's
  icons (`crates/app/src/icons.rs`) use the same three colours, so the
  cap on the head and the cell in the grid agree.
- **`Gear` is still `Copy` and no longer `Eq`**, because a `Piece` holds
  a health that is an `f32`. Nothing compared two gears; anything new that
  wants to should compare the ids of the pieces, which is what identity
  is here.
- **`HIT_BENCH = 12` and `HIT_SHELF = 13`** are the containers: any
  workstation (`Room::bench_at`, by frame) and any shelf
  (`Room::shelf_at`), with `Game::hit_bench()`/`hit_shelf()` the index
  and `Game::bench_part(i)` the bench's part code so the app knows
  whether it is the armoury. The cold store has no click of its own since
  feature 104 (`HIT_FRIDGE` went with its menu); it is still a container,
  reached from the Nearby strip, and stood at from `Room::fridge_station`.
  `Game::container_spot(Container::Bench(i) | Shelf(i) | Fridge(i) | Desk(i))` is
  where the Bim stands to use it, for `send_to`. **A left click notes
  the fixture indices now too**: `hit_at` and `drag_end` share
  `note_fixtures`, so the grid that opens off a left click is the
  fixture under *that* click and not the last right-clicked one.

`a_vest_takes_a_hit_first_and_its_protection_lifts_when_it_breaks`,
`equipping_swaps_with_what_is_worn_and_a_weapon_with_the_weapon` and
`give_and_take_round_trip_and_a_broken_piece_is_only_ever_discarded` in
`game::tests` pin it; the world's tests pin the hold's side.
`a_recruited_bim_shoots_the_machines_it_can_see_and_they_are_hurt` in the
world tests stands crew member 0 inside a held station's door with one
Trooper down the corridor (`World::stage_droid_fight_for_probe`, which took
the place of the human `stage_fight_for_probe` in feature 104) and runs
until `DroidDown`; `bims droids` is the picture, and `BIMS_WEAPON=schword`
(or `pistol|shotgun|rifle|sniper`) in the crew member's hand,
`BIMS_ENEMY_WEAPON=…` in every resident's — a town's guard in `defense` —
and `BIMS_ARMOURED=1` for a fresh helm, kevlar and leg guards on the crew
member are how a swing, a burst or a lock is looked at without waiting for
one — without the armour a crew member locked by a blade is dead before it
is ever seen locked.

## A weapon is a value with a tier, a piece has one too, and the tier scales the kind

`combat::Tier` — `One = 1`, `Two = 2`, `Three = 3`, never nought, with
`next()` — is how good a piece of equipment is, since September 2026
(38.6). **A weapon is `combat::Weapon { kind, tier }` now**, not a bare
`WeaponKind`: what a hand holds (`Gear::weapon`), a pack carries
(`Item::Weapon`), a `Shot` or a `Bolt` was fired from, a `Target` or a
`Seen` carries and a `Dropped` lies as. `WeaponKind::basic()` is the kind
at tier one — what everybody is issued, what a bench makes, what every
resident and mercenary carries — and `kind.at(tier)` any other. A weapon
stays a value because it has no wear and no id: two pistols of a tier are
the same pistol, which is why the world keeps the hold's weapons as a
list of `Weapon`s and the armour as instances. A `Piece` has `tier`
beside `id`, `kind` and `health`; `Piece::new(id, kind, tier)` is whole
at the tier's health.

**The tier scales the kind's numbers, and everything reads the
instance.** `Weapon::stats()` and `Piece::stats()` are the kind's
`balance` row multiplied by the tier's factors, and every curve, the
tactics, the tooltips and the health bars read those — a `kind.stats()`
in new code is a tier-one number where a tier-two one was meant. The
factors are `balance.rs` constants:

| | damage, damage_far | accuracy, accuracy_far (capped at 1) | range, sweet | armour health, protection | dodge |
| --- | --- | --- | --- | --- | --- |
| One | ×1 | ×1 | ×1 | ×1 | 0 |
| Two | ×1.25 `TIER_TWO_DAMAGE` | ×1.25 `TIER_TWO_ACCURACY` | ×1 | ×1.5 `ARMOUR_TIER_STEP` | 0 |
| Three | ×1.25×1.25 (`TIER_THREE_DAMAGE` on top) | ×1.25×1.05 (`TIER_THREE_ACCURACY` on top) | ×1.2 `TIER_THREE_RANGE` | ×1.5×1.5 | 0.10 `TIER_THREE_DODGE` |

The pistol's 0.855 at tier two is capped at one, so its `accuracy_far`
is where the quarter shows; a blade's `range` scales too, so a tier-three
schword is swung a little further. **Dodge** is the one thing a tier adds
that a kind has not got: a whole tier-three piece gives its wearer
`TIER_THREE_DODGE`, and `Gear::dodge()` combines the worn pieces'
(`1 − Π(1 − d)`, so three are a little over a quarter). It is rolled in
`Combat::step` after the cover roll, for bolts only — a blade's blow is
not dodged — and **only for a body whose dodge is above nought**, so a
fight with nobody in gold draws exactly what it always did and no seeded
probe moved. The bodies handed to `step` carry it as a third element,
`(position, peeking, dodge)`, and a target's comes across from the other
room the way its peeking does: `Game::set_hostiles_dodge(&[f32])` →
`Combat::set_dodge`, called right after `set_hostiles`, off the other
room's `Game::dodge(who)`. The deck draws no tier — `Character::set_armed`
still takes the kind — and the app tints the cell and the slot instead.
`a_tier_scales_the_numbers_and_tier_three_armour_dodges` in
`combat::tests` pins the table, the cap, the combining and both rolls;
how two of a tier become one of the next is the world's
(`crates/world/CLAUDE.md`, "Two of a kind go onto the workbench").

## A carry between benches is the whole of hauling

Feature 56 (September 2026): the world wants a gun or a piece of armour
walked from the lockers to the workbench's slots and the upgraded one
walked back, and the room's half is `Kind::Ferry { from, to }` — both
indices into `Room::benches` — modelled on the haul of materials to a
site that went with the money rework (feature 95): `GoToStore`
(bench `from`'s use spot) → `TakeGear` (1.2 s, reaching in) →
`CarryGear` (to bench `to`, `Held::Crate` in the arms — put there on
the walk's *entry* too, so a chain picked back up off the queue carries
it) → `PutGear` (1.2 s). **The room never knows what the thing is.**
The world posts `Room::ferries: Vec<game::Ferry>` every step
(`Game::set_ferries`, at most one), and the chain reports on three
lists the world drains: `ferry_picked` as `TakeGear` ends,
`ferry_dropped` as `PutGear` ends — which also takes the order off the
room's own list, since that copy is a step behind the world and the
same carry would be offered again — and `ferry_returned` from
`let_go(for_good)` past `TakeGear`, the hands emptied. There is no `Job`
of its own: the carry is `Job::Haul`, and since the materials went it is
the only thing that row does — `work_on_offer` offers `Haul` when
`ferry_on_offer` has something and `do_some_work` takes it as `ferry` —
with its own readout code `JOB_FERRY = 26`. `ferry_on_offer` wants both benches in the room, a
route from the Bim to the first, and `can_begin`, which holds
`Exclusive::Bench(to)`: two Bims carrying to the workbench at once would
be two hands in one slot, and one at work on it (the `UPGRADE_ORDER`
craft holds the same) is at it already. The world keeps an order posted
until its drop lands, so a chain on its way never finds its target
gone; what the drop *is* — into a slot, into the hold — is decided at
the world's end. `world::tests`'
`two_pistols_or_two_helms_are_combined_at_the_workbench_over_a_day` is
the chain run for real, twice over and back.

## A room can be built over a grave

`Game::lay_out_dead(who, at)` (feature 85) stands one of the room's
bodies at a point — snapped to somewhere a body fits, as `adopt` snaps
one — and kills it **outright**: `Health::give_up`, `Character::die`,
the errand and the queue dropped, the bunk given back, and **nothing in
anybody's diary** — `Game::die`'s `What::CrewDied` is for a death the
room watched, and this one happened before the room existed. It is the
world's door for laying a station's dead back on its deck when its room
opens again (`world::memory::Grave`, `crates/world/CLAUDE.md`, "The dead
lie where they fell"): the room is built with that many extra bodies,
each given the coverall, the look and the gear the grave kept, and then
laid out. The room itself remembers nothing of it — a body laid out is
an ordinary dead body of the room, lootable and in the way like any
other.

## A body is looted, and down is dead or out cold

`Game::is_down(who)` is `!is_alive || is_unconscious`: what a Bim has to
be to be looted, and what the world hands the other room as
`set_visitors_down`. `Game::loot_cells(who)` is the body laid out as
`[Option<Item>; LOOT_CELLS]` (54) in `combat::LootCell` order — `Pack(0..49)`,
then `Head = 50`, `Body = 51`, `Legs = 52`, `Weapon = 53`, `code()` /
`from_code()` — a worn piece as `Item::Armour` with its health, the
weapon as `Item::Weapon`, which is what the Loot window draws.
`Game::take_from_body(who, cell)` strips one cell — a pack cell emptied,
a piece off its part *broken or not* (the looter's pack can hold what the
hold will not), the weapon out of the hand — and refuses (`None`) a Bim
that is not down or an empty cell. It is the room's half only: the world
puts what comes back into the looter's pack with `give`, and reach is the
world's check. Things that are not obvious:

- **`hit_at` no longer skips the dead.** A dead crew member under the
  click is `HIT_BODY = 14` (`Game::hit_body()` the index); an
  unconscious one stays `HIT_BIM`, because the menu offers the bandages
  *and* the Loot row and which the player means is theirs. A visitor —
  one of the station's people, in its own room, drawn on this deck — is
  `HIT_VISITOR = 15` (`Game::hit_visitor()`) **only when the world has
  marked it down**: `set_visitors_down(&[bool])` is index for index with
  `set_visitors`, told right after it every step, and `set_visitors`
  clears it, so nobody is down until the world says so again. One on its
  feet is not hit at all — it is not the crew's to click. Bodies before
  fixtures, crew before visitors.
- **The hand is emptied now, not next step.** `take_from_body` calls
  `set_armed(None)` and `refresh_worn` itself, since `tick_combat` would
  only holster a body that is down on its next tick and the picture
  would lag the pack by a frame. A stripped body draws bare; a crewmate
  that comes round is without the piece.

`a_dead_bim_under_the_click_is_a_body_and_an_unconscious_one_is_still_the_bim`,
`looting_strips_a_worn_helm_with_its_damage_and_an_awake_bim_is_refused`
and `a_visitor_marked_down_is_a_body_and_one_on_its_feet_is_not` in
`game::tests` pin it; the command, reach and the piece's new id are the
world's (`crates/world/CLAUDE.md`).

**A left click on a body opens its inventory straight off** — the app's
`CrewPanels::body_under_click`: `HIT_BODY`, or `HIT_BIM` with the Bim
down, and the click goes to `open_loot` (which also walks the Bim shown
over) instead of a menu; the right-click keeps the rows, since a Bim out
cold has the bandages on it too. **Only a crewmate is looted** since
feature 104: a station's person down is still `HIT_VISITOR` under a click,
and the app offers nothing for it — the world refuses a resident's loot
(`NotACrewmate`, `crates/world/CLAUDE.md`) — and a machine has nothing on
it to take (`loot_cells` is empty for one).

**A downed enemy was finished off** until feature 104 (RimWorld's
execution: `Kind::Execute`, the walk to the body, `Room::executed` and
`Game::execute_body`), and went with the human enemies: a machine is
wrecked outright, and nothing lies down and alive for it to finish.
`JOB_EXECUTE` (25) is left free, and `combat::Trigger::pull_single`, the
one-shot pull it fired with, has no caller.

**A visitor on its feet is hit only when the world says it may be
spoken to.** `Game::set_visitors_hailable(&[bool])`, told right after
`set_visitors` every step like `set_visitors_down` (and cleared by it),
marks the world's mercenaries for hire; `hit_at` answers `HIT_VISITOR`
for one of those on its feet as well as for a body down, and
`Game::visitor_down(i)` says which it is — the app's Hire row is for one
on its feet. The room knows nothing of what is said; the fee, the
hire and the walk over are the world's (`crates/world/CLAUDE.md`). Two
things beside it for the same feature: `Uniform::Mercenary` (olive
`SHIRT_MERCENARY`/`SLEEVE_MERCENARY`) is drawing only, read back through
`Game::uniform(who)`, and a hired hand keeps it aboard; and
`Gear::hired_for(seed, piece_ids)` rolls a mercenary's kit —
`MERCENARY_ODDS` for the gun, `MERCENARY_ARMOUR_ODDS` a piece — with
fresh pieces numbered from `piece_ids`, the way `issued_for` rolls a
resident's, both through one `roll_weapon`. `Game::bed_count()` is the
bunks: what a hire asks before `adopt`, which gives the hire the first
bunk nobody has (`Bim::bed` cleared by the world's `hire` first, since
the one it arrives with is the station's).

**A station's trading desk is a fixture the room only points at.**
`Layout::desks`/`Room::desks` (`PartKind::TradingDesk`, footprint and
stand spot like a shelf), `Room::desk_at`, `HIT_DESK = 16`,
`Game::hit_desk()`, `Game::desk_spot(i)` and `Game::desks()`. No chain
walks to it and no spot code names it — the ship's readout names the
part — it is where the world wants a crew member standing to trade
(`World::at_the_desk`). Kept on a joined deck, since the station's desk
is the one that matters.

## "Can it get there" is two lookups, and a step must never search the grid for a no

`Nav::can_reach(from, to)` compares the region labels of the two cells —
`Nav::label` floods every patch of free cells once when the grid is
built, stepping by `steps_from`, the one neighbour rule the search and
the labelling share — and `Nav::path` refuses a pair in different patches
before searching. Every yes/no reachability question in the room asks
`can_reach`: `Game::can_reach`, `queue_walk` and `walk_ready`,
`site_stand`, the patient's and the kit's and the dropped gun's stands,
`ferry_on_offer`, a round's stops and the splash's filter. `path` is for
a route that will be walked.

Why it is a rule and not a tidy-up: `work_on_offer` runs for every idle
Bim every step, and it asks about every site, every patient and the
benches. When those asked `!nav.path(..).is_empty()`, an A* that finds
nothing walks every cell of its patch first — 40 000 cells on a station's
grid — and a station's room cost **900 µs a step**, which at 24x is the
whole frame (it was the needs' shelves and bay that asked, then). A docked step is
about 7 µs now; measure a new per-step question with the scratch bench
before adding it, and if it is a route, ask whether it is asked only when
the answer is used.

## A second table or a lone basin is drawn as itself, and a station's fixtures stay the station's

Every fixture the room has a picture for is drawn by the room, and
`aboard::drawn_by_room` lists every part of those kinds — the doors, the
galley's four, the table and chairs, the bunks, the broom locker, the bays,
the fields, the toilet and the basin — so the ship painter draws none of them
as a coloured block. The kinds the room keeps a list of (`worktops`, `hobs`,
`fridges`, `dishwashers`, `lockers`, `bays`, `heads`, the first of each from
the layout and the rest from `Layout::more`) are drawn one by one; `showers`
is a list too, for the readout and the rounds, and the ship painter draws a
shower. What the room has no list for — a second table, every chair being
seated already, and a basin no toilet pairs with — `layout_of` lists as
`Layout::extras` (a `fixtures::Still` kind and the rect), and `Room::stills`
(`Stills::from_extras`) draws them after the fixtures, in `draw` and in the
designer's `draw_fixtures` alike. A new picture goes in as a function of its
rect ("Aboard, a fixture is drawn inside its tile").

**On a joined deck the station's fixtures are not the crew's**:
`Aboard::leave_the_station_s` drops every `more` entry and every extra
standing inside `Aboard::station_box` — at the join and at every
`Aboard::relayout` after it, since a part built relays the whole deck —
because the station's own room draws those, and the two pictures of one
fixture would lie one over the other.

## A bandage is a thing in a pack, and the dressing is a walk to a crewmate

(The bay's fibre, which a bandage was once made of, went with the bay's
growing in feature 104; a dressing comes from nowhere but a pack now.)

**A bandage is a thing in a pack, and there is no count anywhere else**
(feature 87). `Room::bandages`/`bandages_used` are **gone**: a dressing
is `bims::game::BANDAGE` — `Item::Stack(combat::BANDAGE_CODE)`, code 5
said here because this crate does not know `physics` — kept in a Bim's
own `Gear`, **five to a box** (`Item::stack_limit`,
`combat::BANDAGES_A_BOX`) over a two-by-two footprint. `Game::bandages_of(who)`
is how many that Bim carries, `Game::give_stack`/`take_stack` put them
in and out by the unit, and `spend_bandage` is the one place one is
spent: the **emptiest** box first, so a pack tidies itself by being
used. A bare room deals every Bim `BANDAGES_AT_DAWN` (3) at `Game::bare`,
drawing nothing off any stream, so a test can try the chain; aboard the
dressings are everybody's charges, a pack topped up one at a time by
`World::restock_charges` (`crates/world/CLAUDE.md`, "The medicine is
everybody's charges"), and a station's people are dealt
`data::RESIDENT_BANDAGES` when their room opens. Nothing crosses the seam
as a number any more — the world hands
the room no bandage count and reads none back.

**The pack stacks now, and the count is the cell's.** `Gear::count` is
a `u32` a cell beside `pack` and `turned` — **nought reads as one**, so
every `gear.pack[c] = Some(item)` written by hand is still one of it —
and `Gear::units(cell)` is the one accessor. `put_many`, `take_one`,
`room_in`, `stack_with_room`, `stack_to_spend` and `units_of` are the
rest; `put` is `put_many(.., 1)`, `take_out` takes the **whole** stack
and `rearrange` pours one box into another where the two are the same
thing with room, whatever is left staying where it was. A stack is the
one thing in a pack that stacks: the hold's goods stack on a shelf and
never on a back.

`Kind::Bandage { patient, part }` is the chain — `part` a `health::Part`
code, carried as a number because the room never reads it — two steps,
`GoToPatient` and `Dress`, the second `BANDAGE_MINUTES` (10) riding in
`rest_minutes` like a craft's. `Game::bandage(who, patient, part)` starts
it, refusing a dead or out-cold or outside helper, a dead patient, no
bandage, or a part with nothing open on it. Three things about it that
are not like the other chains:

- **The room is told where the crew stand.** A chain is handed the room
  and nothing else, and the patient is a `Bim`. `Room::crew` is every
  body alive and on the deck, by index, written by
  `Game::tell_the_room_where_the_crew_are` at the top of every step *and*
  as the order is given — the order can come before the first step, and
  the walk picks its spot from it in `Task::enter` (`patient_stand`: the
  Bim's own spot for itself, else the nearest free cell a tile from the
  patient towards the helper, and `blocked` when the patient has no
  position or no route). Picked as the walk is entered, so a chain picked
  back up off the queue walks to where the patient has got to. **And
  the walk follows a patient that moves** (since September 2026):
  `Task::patient_at` is where the patient stood when the walk was
  planned, and `Task::update` re-`enter`s `GoToPatient` — a fresh
  stand and route from where the patient is now — the moment it is
  more than `FOLLOW_SLACK` (1.5) tiles from there. A route is planned
  once everywhere else; this is the one walk whose destination is a
  body that runs from a fight, and it used to arrive at the empty spot.
- **The room says, the game dresses.** `Dress`'s `leave` pushes
  `(helper, patient, part)` onto `Room::dressed` — the helper too,
  because by the time the game looks its chain is finished and gone —
  and `Game::apply_dressings`, after everybody has moved, closes the
  part's wounds (`Health::bandage`), takes a dressing out of **the
  helper's own pack** and refreshes the blotch, only if the patient is
  alive, the two are within two tiles (or one and the same) and the
  helper is still carrying one. A patient that walked off, or a pack
  emptied since the order, is ten minutes lost and nothing else.
- **`JOB_BANDAGE = 22`** is its code for `activity` and the agenda; it
  holds no `Exclusive` and no `Fixture`, so two crew can dress two parts
  of one patient at once.
- **Both hands are on the bandage.** `Dress` poses `Action::Bandage` —
  the hands going round one another with the roll in the right and the
  turns laid as a ring between them — and `Task::is_dressing` is what
  `tick_combat` reads to holster a recruited Bim's weapon for the ten
  minutes and draw it again after; the walk to the patient is not the
  dressing and holds its fire the way any walk does.
  `winding_a_bandage_holsters_the_weapon_and_it_is_drawn_again_after`
  pins it.
- **Every wound at once is `Game::bandage_all(who, patient)`** (feature
  87): the worst part now and the rest queued behind it through
  `Game::order_later`, one dressing a part and no more than the pack
  has dressings for. What `CrewOrder::BandageAll` sends — the row on a
  box of dressings in the inventory — and, the other half of the same
  call, what a Bim running from a fight reaches for by itself: the
  fleeing branch of `tick_combat` calls it the step nothing can see the
  body (`Combat::sees_any`), for the crew's side only, since an enemy's
  run seals itself in and binds out of its own pockets
  (`seal_and_bind`). Nobody already being walked over to is touched.

`a_bandage_is_walked_over_and_closes_the_wounds_on_one_part` and
`a_dressing_comes_out_of_the_pack_and_a_bim_out_of_sight_binds_every_wound`
in `game::tests` pin both — the second at frame rate, because **`simulate(1.0)` cannot
walk a Bim**: a one-second step overshoots every waypoint and the body
marches for ever, which is what `unstick` then sees every second. The
tests that step a whole second at a time are the ones where nobody walks.

## A part at nothing is a dying state, not a death; a Bim dying runs, and one out cold is no target

`crates/game/src/health.rs`, since September 2026. A part reaching
nothing used to be death (the head or the body) or a leg gone (the
legs). Now it rolls a **`Trauma`** for that part — `Trauma::roll(part,
unit)`, three each for the head and the body evenly, four for the legs:
femur and knee 45% each, the crushed right and left leg `CRUSHED_ODDS`
(5%) each — and the Bim is **dying** (`Health::dying`, `Game::is_dying`):
the part stays at nothing and **does not mend**, the trauma bleeds it
(`Trauma::bleed`: `HEAVY_BLEED` 40 an hour = ten a quarter hour, or
`SLOW_BLEED` 20) or slows it (`Trauma::pace`, `Trauma::works_at`), and
a hit on a part already at nothing opens a wound and nothing more. A
crushed leg is **lost the moment it is rolled** (`legs_lost`, for ever,
`LEG_LOST_PACE` 0.8 each, multiplied) and bleeds until the stump is
treated. **What kills a Bim is its blood**: `is_dead` is blood at
nothing, or the head and the body both at nothing with no trauma on
either — which only `give_up` does, since it clears the traumas. The
codes 0–9 are the app's (`TRAUMA_NAMES`, `TRAUMA_LINES`,
`TRAUMA_AFTER`, `TRAUMA_LASTING` in `names.rs`, pinned against
`Trauma::ALL`). `Health::shot` takes the `roll` and hands the trauma
back; `WoundOutcome::trauma` carries it and `leg_lost` is derived.

- **Only another Bim, with a medkit, gets it out.** `Health::treat(part)`
  takes the trauma off, puts the part back to `TREATED_TO` (half its
  base — nought for legs both gone) and pushes a `Lasting { trauma, left
  }` if `Trauma::after()` says the trauma leaves something: concussion
  and broken ribs a quarter slower walking *and* working for two days,
  cranial trauma half for a day, chest trauma half pace walking for a
  day, the knee a quarter slower walking for two; the fractures and the
  bleeds nothing; the crushed legs only the leg. `lasting` counts down
  in `update` and `pace()`/`works_at()` multiply every untreated and
  every lasting trauma in. `Kind::Treat { patient, part }` is the chain:
  the bandage's two steps, `GoToPatient` and `Dress`, `TREAT_MINUTES` (20)
  riding in `rest_minutes`, `Kind::patient()` the one accessor both
  kinds answer, and `Dress`'s `leave` pushes onto `Room::treated`
  instead of `Room::dressed`; `Game::treat(who, patient, part)` starts
  it and **refuses `who == patient`** — nobody treats their own — and
  `apply_treatments` does it after everybody has moved, under the
  dressing's conditions, off `Room::medkits` (`set_medkits`,
  `take_medkits_used`, `MEDKITS_AT_DAWN` = 2 in a bare room; nought
  aboard, below). `JOB_TREAT = 23`. The world is told through
  `take_traumas()` and `take_treated()` (`CrewDying`, `CrewTreated`).
- **The medical row: itself first, then a kit for the dying, then the
  rest.** `medical_on_offer` answers a `Care`: `Care::Bandage(who, part)`
  for the helper's own worst wound while there is a bandage; else
  `Care::Treat(patient, part)` for the nearest crewmate dying that the
  helper can get to, its worst-bleeding trauma, while there is a kit for
  it — one in the helper's **own pack**, else one on a shelf to walk to
  — never the helper's own body; else the crewmate bleeding most. `give_care`
  starts either. A part somebody else is already walking to is left to
  them — a dressing and a treatment apart (`being_dressed`,
  `being_treated`), since a part being bandaged can still want its
  trauma treated. **The row starts at `HIGHEST`** (`Priorities::new`),
  so a wound is dressed the moment there is a bandage. **A bot under arms with nothing in sight
  doctors** — recruited, no target in `sees_any`, no lock and no blow —
  and takes its weapon up again the moment `aim` sees something; the
  player's own recruited Bim never does, being the player's. The whole
  doctoring errand is holstered (`dressing` in `tick_combat` covers the
  walk now), which is also what keeps the tactics off it.
  **A crewmate is doctored where it lies out of harm** (since September
  2026): the room's calm, *or* the patient itself out of the fight —
  `Game::out_of_harm` of where it lies, no target up within
  `RESCUE_CLEAR` (12) tiles of it and nothing that could see it there,
  the same test a field medic sets a body down by. It was the room's calm
  alone, and a fight in waves is never calm from the first machine to the
  last: a body carried clear lay there untreated until the whole station
  was cleared, and "the bots only tend the wounded once the last enemy is
  dead" was the user's report. **In a fight one helper goes to a
  patient** — a patient somebody else is already walking to is theirs,
  every part of it (not counting its own self-dressing) — since a part
  apiece, the calm's rule, took nine bots off the line at the first lull
  for three patients; in the calm, a part apiece as before. And **a bot
  puts its doctoring down the moment it has a shot**
  (`Game::care_gives_way`, at the top of each body's turn in
  `tick_combat`): a recruited body nobody steers, with a `Bandage` or a
  `Treat` in hand — its own wound or a crewmate's — and a gun's shot in
  reach (`Combat::aim`) or, for a blade, an enemy in sight has the chain
  `abandon`ed for good (a kit in its hands goes back on the shelf) and is
  armed that same step. It began with nothing in its sight and used to
  wind on for the ten minutes with a target in front of it. Never a
  player's own Bim, a body on the run, or a medic whose beam holds its
  fire anyway. `Game::calm` itself is three things at once — `Combat::lull()` (seconds
  since anything was fired or landed in this room, either side's: `fire`,
  `shoot`, `brawl` and `struck` set it to nought, `Combat::age` counts
  it up every step of `tick_combat`, `f32::MAX` in a fresh room) at
  least `CALM_AFTER` (20 s); `enemy_unseen_for` (seconds since any of the
  room's living, waking people on the deck had a target in sight — in a
  hostile room, since any target was a sighting rather than stale, the
  airlock's eyes counting) at least `CALM_AFTER`; and no enemy within
  `CALM_RANGE` (20) tiles of any of them (`enemy_within`, against the
  targets as handed — the crew's room knows every enemy's position, a
  hostile room its beliefs). `medical_on_offer` asks the two **after**
  the helper's own bandage and before either kind of care for anybody
  else (`can_get_to`), so a wound of its own is still dressed on the
  spot whenever it has nothing in its own sight. The world's restock of
  the packs asks nothing of the calm since the medicine became
  everybody's charges (`World::restock_charges`, in a fight as out of
  one). It holds for both rooms' people, so a hostile room's would stop
  patching each other under fire too. The player's explicit
  `bandage`/`treat` orders are not gated: the menu is the player's call.
  `a_crewmate_is_doctored_where_it_lies_out_of_harm_whatever_the_room_s_clocks_say`
  pins it — an enemy hidden in a closet (`room_with_a_closet`) holds the
  helper back (the room is under twenty tiles across, the enemy inside `RESCUE_CLEAR` of
  the patient), a hostile bolt fired a moment ago and a sighting that has
  ended do not — and the own wound bound regardless;
  `in_a_fight_one_helper_goes_to_a_patient_and_in_the_calm_one_a_part`
  the helper count; `a_bot_puts_its_dressing_down_the_moment_it_has_a_shot`
  the dressing given up for a shot.
  **Its own kit before a new one.** The world says every step how many
  medkits each Bim carries in its **own pack** (`Game::set_pack_kits`,
  off the packs; `Room::pack_kits`, `Room::carries_kit`), and a helper
  that has one opens it where it stands: `GoToKit`'s target is the Bim's
  own position — no walk to a cabinet at all — and `TakeKit` takes it
  out of the pack rather than off the shelf, saying whose on
  `Room::pack_kits_used` (`Game::take_pack_kits_used`). **Since the
  medicine became everybody's charges** (`crates/world/CLAUDE.md`, "The
  medicine is everybody's charges") that is every kit aboard: the world
  sets the shelf to nought and hands no kit stands, and it takes the
  kit out of the helper's pack **when the treatment is done** (off
  `Healed { with: Medkit }`) rather than at `TakeKit` — so until then
  the pack still holds the kit in the hands, and `set_pack_kits` leaves
  one out for a Bim whose hands hold `Held::Medkit`, or a second
  treatment would be offered on the same kit. A treatment given up puts
  the kit "back on the shelf" in `let_go`, which the world sets to
  nought again next step, and the pack never lost it. A kit in the pack
  counts as a kit everywhere the shelf's does: `medical_on_offer` offers
  the treatment for it and `Game::treat` is bare-handed only with
  neither.
  **Otherwise the kit is fetched** — in a bare room and a station's,
  which still keep a shelf. `Kind::Treat` is `GoToKit → TakeKit →
  GoToPatient → Dress`: `task::kit_stand` picks the nearest of
  `Room::kit_stands` — the use spots of every container that takes a
  medkit, `Game::set_kit_stands` from the world every step
  (`hand_the_room_the_hold_s_medicine`, empty while the hold has none) —
  and a room with none (a bare room, a station's) takes the kit on
  the spot: the target is the Bim's own position and the walk is of no
  length. `TakeKit` (0.8 s, `Action::Reach`) puts `Held::Medkit` in the
  hands — a white box with a red cross — and takes one off
  `Room::medkits`; `Game::set_medkits(n)` sets the shelf to the hold
  *less the kits in hands*, so a second helper does not set out for the
  last one; `Dress`'s leave for a Treat **spends the kit** (hand emptied,
  `medkits_used += 1`, the pair on `Room::treated`) — there rather than
  in `apply_treatments`, because the finished chain is let go of first
  and `let_go` puts a kit still in the hands back on the shelf (for good
  only; a suspended treatment keeps it on `Saved.main`).
  `a_treatment_walks_to_the_kit_first_and_carries_it_to_the_patient` and
  `under_the_alarm_a_crewmate_with_nothing_in_sight_doctors…` pin it.
  **A patient holds still**: `tick_bim`
  stops the frame for a Bim that `is_being_seen_to` (somebody else's
  `Bandage` or `Treat` names it), its errand interrupted onto the queue
  and `Character::halt`ed — a dying Bim walking its errands had the
  helper arrive at an empty spot, twenty minutes lost and the walk
  begun again, for as long as it kept walking. Not while it flees.
- **A Bim dying runs from the fight — a crew member shooting back as it
  goes, an enemy with its weapon holstered.**
  `Game::is_fleeing(who)`: dying, on its feet on the deck, and any target
  the world named is `Some`. In `tick_combat` that comes before arming:
  the lock, blow and peek are dropped, and `flee(who, dt)` runs on the
  `plan_wait` clock — `Tactics::flee(nav, from, targets)`: the enemy is
  one place, the **average of every target up**, and the best cell is the
  reachable one within `FLEE_LOOK` (10) tiles furthest from it less
  `WALK_COST_PER_TILE` a tile, so it runs the opposite way and round a
  wall if it has to; a route there, the errand interrupted first. It
  goes for **the player's own Bim too**, recruited or not, and for a
  hostile room's people (one shot to a dying state runs from the crew).
  `pump_queue` and `consider_errand` skip a fleeing Bim like a recruited
  one; the enemy gone, it stops where it is and the
  queue picks up. **The crew's run is a fighting withdrawal** (since
  September 2026; `shoot_on_the_run`): a crew member with a gun keeps it
  drawn, aims from its own eyes at whatever `Combat::aim` gives it —
  never a peek — and fires on the move at the walking odds, the walk a
  **backing** one ([`FallBack::Backwards`], the fall back's: the face on
  the target, the feet on the route, at `BACKSTEP_PACE`); standing, it
  squares up. It used to holster for the whole run, and a crew member
  with a broken leg walking off from an enemy in plain view without a
  shot was the user's "the bots sometimes won't fire and walk somewhere
  instead". A blade has nothing to do on the run and stays sheathed, the
  hands on a bandage hold the fire, and **an enemy's people still run
  silent** — their run seals itself in instead.
  `a_dying_crew_member_backs_away_from_the_enemy_shooting_as_it_goes`
  pins the crew's run, and the end of
  `a_crew_member_merely_hurt_fights_on_and_runs_only_dying` an enemy's.
  **Dying, and nothing short of it** (since September 2026): a crew
  member merely hurt — `Health::is_hurt`: an open wound, blood under
  `SLOWED_AT` — stands its ground and shoots on, whoever it is. For a
  fortnight a crew member that was not the player's own ran at any
  wound, and a fight was a crew that scattered at the first hit; the
  run is the dying-state penalty's and no earlier. `is_hurt` is still
  what the medical row looks at. Two things keep the run from being a
  bleed-out, and the first also dresses a wound that is not a run:
  - **It binds its own wound where it stands once out of sight.** The
    medical row's own-wound case (`medical_on_offer`, off `tick_bim`'s
    `HIGHEST` interruption) is not gated on fleeing, and `tick_combat`
    leaves a fleeing Bim's doctoring chain (`dressing`) alone while
    `sees_any` is false — the run waits for the hands to come off —
    and interrupts it and runs on the moment an enemy comes into view.
    Dressed, a dying Bim runs on until a kit is put to it; a wounded
    one that was not running is back on `plan_stand`.
  - **It holds still for a crewmate nearly at it.** `is_fleeing` is
    false while `helper_near`: somebody's `Bandage`/`Treat` names it
    and that somebody is within `HELPER_NEAR` (3) tiles or already in
    `Dress` — which is what lets `tick_bim`'s hold-still take over, so
    a helper can catch a runner at all. Without it a bandage ordered on
    a running Bim was ten minutes at an empty spot and nothing else,
    which is what "the bandage did nothing" looked like.
  `a_crew_member_merely_hurt_fights_on_and_runs_only_dying`,
  `a_hurt_crew_member_out_of_the_enemy_s_sight_binds_its_own_wound_and_comes_back`
  and `a_helper_follows_a_patient_that_moved_and_a_runner_holds_still_for_it`
  pin the three.
- **Out cold is nobody's target.** The world hands `None` for an
  unconscious body on both sides (`crew_ashore`, and `alive` in
  `visit`), so `aim`/`melee_with`/`sees_any` never pick one, and
  `tick_combat`'s `bodies` for a hostile bolt leave it out too, so a bolt
  already flying passes over it. A Bim bleeding towards nothing therefore
  goes out cold at **half** its blood (feature 89: the line was 40%, and
  the crew are meant to be out of a fight before they are dead), is left
  alone, and dies of the blood alone — which is how most of a fight's
  dead die now, and why a test that wants a body dead on the spot uses
  `Game::kill_for_probe` (health `give_up`) rather than a head shot.
  Nobody is asleep any more (feature 104): what the world asks before a
  crew member may act (`World::fit_to_act`) is alive, not out cold and
  not outside.
- **A body going out cold drops its gun.** `drop_weapon` in the
  knock-out: the weapon out of the hand onto `Room::weapons_down` as a
  `room::Dropped { id, at, weapon, owner }`, numbered from
  `next_weapon_down`, drawn on the deck under the bodies by
  `character::draw_dropped` (the gun the hands draw, without the hands,
  askew on a shadow). `Kind::Fetch { item }` is the chain back to it —
  `GoToDropped` (`task::dropped_stand`, `None` if it is gone) and
  `PickUp` (0.8 s, `Action::Reach`), `Room::picked_up`, and
  `apply_pickups`: for a bot into the hand if empty, else a free pack
  cell; for the player's own Bim **into the pack** (the hand only when
  the pack is full), since the right-click is "into the inventory" and
  what goes in the hand is the player's to choose; else left lying;
  `JOB_FETCH = 24`. **Bots fetch their own**:
  `fetch_own_weapon` in `tick_bim`, for every Bim but `PLAYER` in the
  crew's room and every one of a hostile room's, hand empty and its own
  gun on the deck with a way to it, not while fleeing. The player's own
  waits to be told: `HIT_DROPPED = 17` under a click (`hit_dropped()` the
  id, after the bodies and the visitors, before the room), and **the
  right-click itself is the order** — no menu: both screens call
  `Game::fetch(who, id)` for the Bim shown, and say `PICK_UP_REFUSED`
  when it cannot go. `Game::dropped_at(x, y)` is the hover's question,
  the same reach without noting anything, and `set_hover_dropped(id)`
  rings that gun on the deck in the highlight's cyan, worked out afresh
  every frame by the screen like the highlight. And **a body that dies with its
  gun on the deck takes it back** (`die`): the gun lies beside the
  corpse and the corpse is what gets looted — a resident's, in the other
  room, would otherwise be on a floor the crew can never reach.
  `a_bim_knocked_out_drops_its_gun_and_a_bot_comes_back_for_it` and
  `a_dying_crewmate_is_treated_with_a_medkit_by_whoever_is_free` in
  `game::tests`, and the four trauma tests in `health::tests`, pin it.

## A key is two cells tall, and a research desk is a container the world reads

`combat::Item::Key(tier)` was the one pack item that took more than a
cell, before every thing had a footprint (above): it is two tall and one
wide — `Item::rows()` is two for it, which is what a desk's slot grid
still draws — kept in the **upper** cell with the cell under it
(`PACK_COLS` further on) reached over, taken by either, and refused by
`equip` like a stack.

`Layout::research`/`Room::research` are the research desks the way `desks`
are the trading desks — footprint and stand spot, from `aboard.rs` off
`PartKind::ResearchDesk`, kept on the joined deck (the ship's first, the
station's after) — with `HIT_RESEARCH = 18` under a click,
`Game::hit_research`, `research_spot(i)`, `research_desks()`, and
`Container::Desk(i)` for the reach check, since the world puts a key into
the ship's desk through the same `Stow` a bandage goes into a locker by.
`SPOT_RESEARCH = 20` is the ringing code, the last since feature 104
closed the codes up, so `SPOT_NAMES` in the app is twenty-one long. The
room knows nothing of what a key opens.

## An airlock is a door, a locked door is smashed, and a fleeing enemy seals itself in

September 2026. **Every airlock is a `Door`** (`Door::airlock`,
`Door::new_airlock`/`of_kind`): `aboard::layout_of` appends the airlocks to
`Layout::doors` as `(rect, along_x, true)`, so they lock and unlock from
the panel like the bulkhead doors (`HIT_SHIP_DOOR` lands on one, the same
four words), a locked one is a solid to the nav and a wall to the eye, and
`Room::airlocks` still keeps the footprints for the walk outside. The
hull draws the airlock; `Door::draw` draws only its lamp while it is
locked and the bar of a smashing. **To the eye an airlock is still the
passage it was**: `shut_leaves` reads a bulkhead door off its leaves
(`!is_open()`) but an airlock off nearness — nobody within `REACH`, or
locked with the leaves shut — because the fifth of a second the leaves
take to part lost a hunter its quarry: a station's people chasing a crew
member onto the ship (a chase that went with the human enemies in
feature 104) turned on seeing it *in* the ship's airlock the instant it
reached the station's, and read off the leaves the belief stuck in the
doorway.

**Smashing.** `Door::locked_by` is a `Locker` — `Crew` from the panel,
`Body(who)` for one of the room's own — and `Door::smash(by, dt)` is a
body heaving at a locked door: one at a time (`Smash { by, done,
since_heave }`), `SMASH_DOOR` 15 s for a bulkhead door and
`SMASH_AIRLOCK` 30 s for an airlock, a `Cue::DoorSmash` every two seconds
and `Cue::DoorForced` when the lock gives (`unlock`), drawn as a bar over
the door across the opening in `WARN` (`smash_progress`). The app plays
`Force_opening_Door_and_Arilock.mp3` cut to one heave (`door_force.ogg`,
`Kind::Smash`) for both. **Only a hostile room's people smash**, and the
machines (`breach_droid`, "A droid is not a Bim" below):
`Game::breach(who, dt)`, after `plan_stand` for a body at war, on its own
`breach_wait` clock — nothing while a route to any target is open;
otherwise the nearest locked door whose panel (`station`, `DOOR_STAND_OFF`)
it can reach, its own locks first, and it walks there; at the panel it
unlocks its own lock or sets `Bim::smashing` and heaves every frame until
the door gives or it moves off (`drop_smash`). The crew's bots never do
— the crew are the player's to send.

**The hunter and the post** (September 2026, for the raiders, which went
with the human enemies in feature 104; the code stays, and a machine's
hunt is the same rule). Two things the raid found wanting, both in
`plan_stand` and `breach`:

- **A hostile gunner with nobody in sight hunts.** With every target
  stale — believed, seen by nobody on its side — `plan_stand` sends it
  to where it last saw the nearest of them as a blade charges
  (`Tactics::charge` on the beliefs) and sets `Bim::hunting`; the moment
  one is in sight it picks a stand again. Before this a stand scored
  against a belief was a spot with a view of the doorway the quarry went
  through at the far end of the weapon's range (`DISTANCE_WORTH` sends a
  pistol to twenty-two tiles), and a gunner that stood there never
  followed anybody through a passage — nine boarders in ten are gunners
  (`ISSUE_ODDS`). **And a hunter never gives ground again**: for the
  rest of the war `stand_with_cover`'s `closing` drops every candidate
  farther from the nearest target it can see than the body is now, a
  tile's slack allowed — it holds or closes. Without that half the
  hunter stepped back to the far cover the moment it had its quarry in
  sight again, lost it there, hunted, stepped back, in the doorway for
  ever. `hunting` is cleared by `muster` when the war ends, so a body
  that had its target in sight from the start is never hunting and a
  rifle walks off to its range as it always did
  (`a_sniper_rifle_reaches_from_twenty_tiles…` in the world's tests
  still holds). The crew's bots take the list as it comes and never
  hunt.
- **A post behind a locked door is forced.** `breach` reads *goals*:
  at war the targets, believed or seen; off war the body's post, if it
  has one — and `tick_combat` calls it for a hostile body off war with
  a post, before the weapon is asked for (off war nobody is under arms),
  so a raider's boarders posted at the ship's gangway with the airlock
  locked against them smashed it and walked on. `Game::post_at(who, to)`
  is `send_to` that keeps the post when there is no route yet —
  `return_to_post` plans the walk again as the way opens (a `path` with
  no route is a region check, not a search) — and a townsperson told to
  shelter is posted at the nearest bunk with it (`set_sheltering`,
  feature 94), its one caller now. `has_post` has none: the world
  re-posted the boarders by it after a fight.

The world test that pinned the whole of it across the two rooms — a crew
retreating aboard and the enemy forcing the ship's locked airlock — went
with the human enemies; the room's own tests below still pin the smash.

**Sealing in.** In `flee` a hostile body's run notes the first unlocked
door its route passes and which side it set out from (`Bim::seal`);
`seal_and_bind`, in the fleeing branch of `tick_combat` for hostile rooms
only, locks that door `Locker::Body(who)` once the body is through and
clear of the opening (`Bim::sealed_in`), and while its lock stands binds
its wounds every `BIND_EVERY` (10 s): the part bleeding most bandaged,
else a trauma treated — a field dressing out of its own pockets — so with
nothing bleeding it is dying no more, `is_fleeing` is false, `breach`
finds its target behind its own lock, unlocks it, and it fights again.
The crew's fleeing Bims do neither, and a machine never runs.
`an_airlock_is_a_door_that_locks_and_an_enemy_smashes_through_it_in_fifteen_seconds` and
`a_fleeing_enemy_seals_itself_in_binds_its_wounds_and_comes_back` in
`game::tests` pin it, on the playtest ship as a hostile room
(`hostile_ship`); a hostile room's people hunt only what they have seen,
so the smash test lets the enemy see its target through the closing door.

**Two rooms, one lock.** The station's doors are in the joined deck and
in the station's own room, so `Game::door_states` (`DoorState`: centre,
locked, by_crew, smash, changed), `door_index_at`, `mirror_door_lock`,
`mirror_door_smash` and `door_change_seen` are the seam, and
`World::sync_doors` in `visit` matches each of the residents' doors to
the deck's by its middle through the station frame and copies whichever
side changed (`Door::changed`, set by `lock`/`unlock`, cleared when
carried), the smashing one way — see `crates/world/CLAUDE.md`.

## The dark: lights, ten tiles, and a smooth picture over the tile mask

September 2026, `sight.rs`. **The rule stays on the tile grid** — the fight
and the world read it, and a server has to agree — and a **picture** is
laid over it. Two things:

- **Light.** `sight::Light { at, reach }` is a lamp; `Layout::lights` is
  every `shipdesign::light_tiles` part (`WallLight` 7 tiles, `StandingLight`
  9) — a standing light at the middle of its tile, a wall light
  `aboard::WALL_LAMP_IN` (0.38 of a tile) towards the wall it hangs from
  (`shipdesign::wall_light_back` of its rotation), so its shadows fan
  out from the wall and not from the middle of the gangway — and
  `Sight::set_lights` marks a tile **lit** when a straight line from some
  light reaches its middle within the reach over the **fixed** cells —
  the walls and the tall parts, never a door: a door's leaves are not
  what a light waits for. A lamp **draws** (`shipdesign::WALL_LIGHT_POWER`
  25, `STANDING_LIGHT_POWER` 40, since September 2026) and the world puts
  one out for want of it — `Lamp::powered`, `Sight::set_lamp_powered`,
  below under "A lamp has health" — but the room itself decides nothing
  about power: every lamp starts `powered`, and a room the world never
  tells keeps them so. A room *never handed* lights is lit throughout
  (`Room::bare`); a designed deck handed none is dark everywhere. **In
  the dark a Bim sees
  `DARK_RANGE` (10) tiles**: `in_the_light(eye, tile)` — lit, or within
  ten tiles of the eye — gates `observe` and `sees_from` on top of the
  clear line, so an enemy in an unlit corridor twenty tiles off is nobody
  until it is lit or close. The fixtures and `station::build_layout` carry
  lights (the reference's four wall lights and a lamp, the playtest's six
  and a lamp, a station's inner corners and every sixth tile of wall —
  last, so a lamp never takes a fixture's tile, and each turned to its
  wall, `crates/shipdesign/CLAUDE.md`), and the reaches were raised from
  5.5/7 to 7/9 when the station's dark patches had the sniper walking in
  to 19 tiles. `the_dark_is_seen_ten_tiles_and_a_lit_tile_further` in
  `game::tests` pins the rule; `a_wall_light_wants_a_wall_at_its_back` in
  shipdesign the parts.
- **The picture is `sight::LightMap`**: two bytes a pixel at
  `MAP_PX_PER_TILE` (8) over the whole grid — the **darkness** to draw,
  and the **glow**, how much lamplight falls there — by **marching**
  `RAYS` (2048) rays out of every eye and every light until an opaque
  cell stops them, so what is seen and what is lit have the walls'
  straight edges and not the tile grid's steps, and a lamp throws a cone
  through a doorway. A ray is the grid traversal in pixels (`march`):
  it steps to whichever pixel edge comes next, so every pixel the line
  crosses is visited once and none skipped — the same walk as
  `clear_line` at eight times the resolution, integers bar one add a
  step. It used to step half a pixel at a time in floats with four
  divisions a step, and a joined deck cost 18 ms every time anybody
  crossed a tile. A pixel is seen from an eye when a ray reaches it and
  it is lit or within the dark range of the *body*, as the trace
  measures it. **Every fogged tile is in it, the crew's own and a
  stranger's alike** — seen and lit is nought darkness and `GLOW` (0.24)
  of lamplight; seen and unlit is `MAP_DARK` (0.50); a friendly tile
  unseen is `MAP_FOG` (0.62); a stranger's tile once seen and unseen now
  is `MAP_GREY` (0.80), `explored_px` remembering per pixel; a
  stranger's never seen is black, no glow — so the station's fog has the
  same straight edges as the ship's and no tile pass is drawn under
  `Fog::Crew`. The lamplight shows through the fog and the grey at
  `GLOW_UNDER_FOG` (half), since a lamp does not move and the crew
  know where they hang. The **light field** is cached per layout
  (`build_light_fields`): every lamp is marched **twice**, its direct
  fall, which every opaque cell stops, and a fill of `SHADOW_FILL`
  (0.55) of it which the furniture lets past and the walls do not —
  `Layout::tall` / `Sight::set_tall` marks the opaque cells that are not
  `shipdesign::is_wall` as `Cell::soft` — so behind a shelf is a shade
  and behind a bulkhead the dark; full brightness to `LIGHT_CORE` (0.35)
  of the reach, then falling off at `LIGHT_FALL` (1.6). **Each lamp's
  fall is kept on its own** (`LampField`, a byte a pixel over the box
  its reach can fall in) **and the field is their sum**, saturating
  (`sum_fields`). It was the brightest alone, and that drew a dark
  four-pointed star on the deck between every four lamps — the point
  equidistant from all four got one lamp's half-light where it should
  have had four — which is the "far shadows" the user saw. A room lit
  for want of any lamp (`lit_everywhere`) has no glow.
- **A lamp has health, and is shot out.** `sight::Lamp` — one a light,
  `Sight::lamps()`, `Game::lamps()` — carries `health` of `LAMP_HEALTH`
  (16: two pistol bolts leave it failing, a third puts it out, a shotgun
  or a sniper does it in one) and `level`, how bright it is shown.
  `Combat::step` stops a bolt within `LAMP_RADIUS` (10 units) of a lamp
  that is not out, like at a wall — a bolt aimed down a gangway misses
  the lamps on its walls, a miss (`MISS_BY`) may not — and puts the
  damage at the distance flown on `Combat::lamp_hits`; `tick_combat`
  takes it off the lamp (`Sight::damage_lamp`), which sets it
  flickering for `LAMP_HIT_FLICKER` (0.6 s) and, at nought,
  `lamp_switched`: `relight` (the tile mask without it, `stale`,
  `views_stale` — what it lit is what was seen by it), and its box of
  the field summed again. Below `LAMP_FAILING` (a fifth) it is
  **failing** and starts a `LAMP_FAIL_FLICKER` on `FAIL_FLICKER_ODDS`
  (0.3) a second of its own. **The flicker is the picture's alone**:
  there are two fields, `light_field` — every lamp not out at full,
  what `view_of` reads for the seen rule — and `shown_field`, with each
  lamp at its `level`, what `compose` draws; and it is `noise(lamp,
  slot)` of the lamp's index and the clock (`Room::update` →
  `Sight::tick_lamps`, `FLICKER_RATE` 24 a second), never a roll, so a
  fight that shoots a lamp draws nothing off any stream. A level change
  is the lamp's box summed again and `field_dirty`, which `light_map`
  joins into the frame's dirty box. The world remembers the damage
  across a relayout and mirrors it to the other room
  (`Game::take_lamp_changes`, `set_lamp_health`, which also puts a
  lamp back; `crates/world/CLAUDE.md`). The fitting's glass is the
  ship painter's (`fittings::lamp_face`: dark and cracked out, veiled
  while dim). `a_lamp_shot_out_goes_dark_and_flickers_on_the_way` in
  `game::tests` pins the lot; `Game::damage_lamp_for_probe` lands a hit
  with nothing fired. **A lamp without power is dark the same way and
  whole**: `Lamp::powered` is the second reason a lamp gives no light,
  `is_dark()` is `is_out() || !powered` and is what `relight`,
  `sum_fields` and `tick_lamps` read, while `is_out()` stays the
  fight's question (`Combat::step` shoots an unpowered lamp like a lit
  one) and `set_lamp_health`'s. `Sight::set_lamp_powered` /
  `Game::set_lamp_powered` flips it through `lamp_switched` — the mask,
  the fields and the eyes redone, level nought, no flicker — and the
  world sets it every step for the ship's lamps off the wiring and the
  brownout (`World::sync_lamp_power`, `crates/world/CLAUDE.md` "A
  brownout is dark, and none of it is lethal"). The bay had the same
  switch until its growing went in feature 104.
- **It is worked out per body, and only for a body that moved.**
  `Sight::light_map(bodies)` is asked every frame from `Game::render`
  under `Fog::Crew`; it keeps a `View` per body — the eyes it was marched
  from, a flag a pixel, and the box those pixels lie in — and marches a
  body again only when one of its eyes is half a pixel or more from
  where it was, or its peeks changed; every body when the cells did
  (`views_stale`: `set_shut` with a change, `set_tall`, `set_lights`).
  Then `compose` puts the views, `explored_px` and the light field
  together **over the box the changed views cover** and nowhere else,
  bumps `version`, and says so in `LightMap::changed` (`(x, y, w, h)`,
  whole tiles; `None` for the whole map — a stance change through
  `map_stale`, or a fresh grid). Nothing moved is no version and no
  upload. On the docked deck of the world tests that is under 1.5 ms in
  a frame somebody walks and about 0.5 ms a marched eye; the tile trace
  beside it is 0.3 ms. `Game::light_map()` hands it to the host; the
  shape buffer cannot carry it, so `crates/app/src/fogmap.rs` composes
  the two bytes into one premultiplied pixel (black under `LAMPLIGHT`),
  writes the changed box into a Bevy image when it holds the version
  before (else the lot) and draws one textured quad, filtered, on the
  world's canvas — **between** the two halves of the room's picture that
  `Game::shapes_fog_split` cuts at `fog_from` (feature 97): over the
  deck, the bodies and the bedding, under the shots, the night, the rings
  and the marquee, and under every word — its corners the map's through
  the screen's transform, `world_paint::light_map_on_screen` (the crew's
  names' arithmetic). The lamps' own pictures (`fittings::wall_light`, flush to its
  wall; `standing_light`) draw no halo: the map is the light. `Fog::All`
  is unchanged: tiles, black.
- **The march is the most expensive thing in a fight's frame, and it is
  not to be made cheaper by marching less** (feature 96). Measured in a
  release build, `bims combat` (the command as it was then, `droids`'
  ship and crew against the arena's people; it went in feature 102) —
  fourteen crew on a joined deck —
  `Sight::light_map` was **2.3 ms of a 9.2 ms frame**, against 0.05 ms in
  the simulation, whose crew is one: a body whose eyes moved half a map
  pixel is marched again, and a fight is fourteen bodies moving. Nearly
  all of it is the ray walk itself, some six hundred thousand pixel steps
  for one pair of eyes on a station's deck — so what was made cheaper is
  the **step**: `Sight::march` takes its callback by type rather than as a
  `&mut dyn FnMut` (an indirect call a pixel was a third of it), the
  `RAYS` directions are a table worked out once rather than two trig calls
  a ray, and `seen` is cleared with `fill`. **`RAYS` is not the knob**:
  four thousand rays are one pixel apart eighty tiles out, and a deck's
  own march is unbounded by range — a body sees as far as the walls let
  it — so a hundred-tile arena is already at the edge of what 4096 covers
  and fewer would streak. `the_light_map_is_the_same_picture_it_was` in
  `sight::tests` hashes both planes of a marched room and pins them, and
  the numbers in it were read off the march as it stood *before* any of
  this: a speed-up that moves a pixel fails it. The frame it bought is in
  the root `CLAUDE.md`, "Where a frame goes".

## A field is a bay laid on open ground

Feature 54. A town on a planet has **fields**: `PartKind::Field` is six
tiles long with use spots along its north like the hydro bay, and the room
makes one a `fixtures::Bay` — `Bay::field(frame, side)`, `Bay::at` with
`outdoor` set — drawn as turned earth (`EARTH`) the frame's size with a
darker edge (`EARTH_EDGE`) and a furrow down each tray (`FURROW`): no
panel, no grow lights, no glow. `is_field()` says which it is. A field
grew at half a bay's pace and wanted no plug until feature 104 took the
bays' growing; what is left is a picture and a solid.

The layout carries them as `More::fields` (a frame and the side it is
worked from, `aboard::layout_of` off the part's first use spot like a
bay's); `Room::from_layout` and `relayout` chain them after the bays into
`Room::bays` (`bays_of`), laid out afresh each time. None is ever the
layout's own bay (`Field` is not in `MAPPED`), so every one is a solid
through `Room::others` as well as through the bays in `Room::solids`.
`drawn_by_room` lists them so the ship painter leaves them to the room.

**Daylight** is `Sight::set_daylight(over: Option<Rect>)` /
`Game::set_daylight`: every tile whose middle lies in `over` is lit
whatever the lamps say — `relight` marks it after the lamps' pass, and
`sum_fields` puts 255 in both fields under it, so the mask and the
picture agree and a lamp going out under the sky changes nothing there.
It is the **world's** word for a settlement's ground (the residents'
room over its bounds, a joined deck over the station box —
`crates/world/CLAUDE.md`), not the layout's, so `Room::relayout` reads
it off the old `Sight` and sets it on the new one after `set_lights`,
the way the world puts a lamp's health back. `lit_everywhere` is
untouched: a room never handed lights is lit throughout already, and
the sky changes nothing there.

## On a planet the box has a margin, and beyond it a body is afield

`crates/game/src/terrain.rs` (feature 55): the plain a landed town
stands on, and how the room walks and sees it. The rule and the room's
side of it are one module — `Terrain` the ground at a station tile,
integer noise only; `Plane` the same read in the room's tiles through
the join's frame, with a chunk cache and the fog — and the room's own
dense structures never grow past `Room::interior`, which on a plain is
the deck's floor box and `DECK_MARGIN` tiles of ground
(`aboard::layout_of_on`). What a probe or a test has to know:

- **`arrived()` is false while `Character::far` is set**, path or no
  path. A walk beyond the box is legs on a window (`Game::plan_route`,
  `continue_far_walk`, `crates/world/CLAUDE.md`), and the chain waits
  on the whole of it. `path_done()` is the leg alone. `follow_path`
  clears `far`: any fresh route is a fresh walk, which is what lets a
  chain take a body home from the plain.
- **Two things replan a walk, and both must use the body's grid.**
  `unstick` and `return_to_post` used the deck's grid, which clamps a
  point on the plain to the box's edge; a body walked to the edge and
  stood there, or walked back out to a post that was the clamp. They go
  through `Game::nav_for` and `plan_route` now.
- **A body bound beyond the box is held to its window, not the box.**
  `move_body` clamps a body into `interior.expand(-BODY_MARGIN)`, and
  a body on the deck with a route out of the box could never leave it:
  `afield || far.is_some()` picks the window's interior and solids.
- **A leg that ends where the body stands ends the walk.** The nearest
  free cell to a target in a cliff is the tile the body is on; without
  this `continue_far_walk` planned the same empty leg every step, for
  ever, never arriving.
- A bare room and a ship on its own have `plane: None` and none of
  this runs: `refresh_afield` returns at once and `plan_route` is the
  deck's grid, `nearest_free` and `path` as a walk always was, planned
  before the interrupt so the routes are the ones they were.
- `Maps::afield` and `Game::afield_blockers` are `serde(skip)`: a load
  rebuilds them on the first step. `Plane::chunks` and `seen` likewise;
  `explored` is saved.
- **The plain's picture is the light map's march, a chunk at a time**
  (feature 67, September 2026; the note before `terrain::PICTURE_PX`).
  `Plane::observe` stays the rule — tiles, for `seen_at` and the world
  — and `Plane::picture(eyes, tile, blocked, cells, window)` is what is
  drawn: from every eye, `sight::march_rays` (the deck's ray walk,
  pulled out of `Sight::march` so the two pictures have the same
  edges) over a window of `VIEW` tiles each way at `MAP_PX_PER_TILE`,
  stopped by the ground's opaque tiles off the deck and by `blocked` on
  it — `Sight::opaque_room_tile`, doors and all — into a `PlainView`
  a body (kept while its eye is within half a pixel and the deck's
  `Sight::cells_version` is the one it was marched over; a relayout
  `forget_views`), and composed into a `sight::LightMap` per chunk of
  the *room* (`CHUNK` tiles a side in the room's frame; the ground's
  own chunks are the station's) for the chunks the host's `window`
  touches. Nought where a view reaches, `MAP_GREY` where a ray has ever
  reached (`Picture::explored`, a bit a pixel, kept for good), black
  elsewhere; no glow — the plain is daylight and has no lamps. The
  deck's pixels are composed like the rest and never drawn: the host
  cuts the box out of the pieces it draws, and composing them keeps the
  blur continuous at the box's edge. Each picture carries a
  `PICTURE_APRON` of its neighbours' pixels for the same reason at
  chunk seams. `Game::picture_plain(window)` is the room's side —
  `Fog::Crew` only — and `Game::plain_pictures` hands them over; the
  window comes from the ship's camera (`ship::Game::picture_the_plain`,
  `world_paint::plain_window`), which is why it is asked after
  `render` and not in it. Nothing of it is saved: a plane read back
  seeds a chunk's memory from the tile rule's as it stood when the
  picture began (`seed`, whole tiles) and a plane made here from
  nothing, since the tile rule reaches half a tile past the rays with
  a stepped rim. A marched outdoor eye costs about what the deck's does
  (~10 ms in release over open ground, most of it the two million ray
  steps), a composed chunk a fraction of a millisecond.

## A look is a yoke, a hair, a shade and a build, and it is drawing only

Feature 62 (September 2026), `character.rs`. `Look` is a struct now —
`yoke: Yoke` (Pale/Mauve, alternating down the crew as it always did),
`hair: Hair` (eight styles, `Hair::ALL`), `shade: Shade` (six colours,
`Shade::ALL`), `build: u8` (`0..BUILDS`, five; `Look::scale()` is `1 ±
BUILD_STEP` a step from the middling one) — and `Look::of(who)` deals
one: `Look::CLASSIC` for the first two, the classic room's pair as they
always were, and for every index after a hair, a shade and a build off a
multiplicative hash of the index. **Not off the RNG, on purpose**: a
draw in `Bim::new` would move every roll after it on the room's stream
("Adding furniture moves everything"). `Game::look(who)` and
`Game::set_look(who, look)` read and change it; the world's
`Session::dress_crew` puts a player's chosen hair on their slot with
`Look::with_hair`.

- **Everything the body draws goes through `Character::body_scale()`**,
  `BODY_SCALE` by the build — the brush, the shadow, the rings, the
  held things and the weapon — so a sturdy Bim's hands and gun are as
  much bigger as its body. `picked_at`, `PICK_RADIUS`, `BODY_MARGIN`
  and the reach do not: a build is a picture, not a rule, and the
  checksum never sees it. `draw_dropped` and `draw_blade` keep the
  plain constant, having no body.
- **The hair is two functions**, `draw_hair_standing` (the head's own
  frame, face to +x, what a style adds drawn before the crown so the
  crown lies on top and the nose after so a puff still leaves the face)
  and `draw_hair_lying` (the head on its cheek in `draw_down`, called
  twice: `under` for what spreads on the deck before the head, then
  what sits on it). A new style is an arm in each, a name in the app's
  `HAIR_NAMES` (pinned to `Hair::ALL` by `names.rs`'s test) and an
  entry in `Hair::ALL`; `code()` is the place in that list and is what
  crosses the wire.
- `character::portrait(look, heading, list)` is a figure at the origin
  for the setup's chooser: the same `draw`, off `Rng::new(0)` with the
  idle glance zeroed, so it stands still.
- `Look` changed shape in the save (`SAVE_VERSION` 10).

## One trigger for a Bim and a sentry, laid cover, and the deploy errand (feature 74)

**The shooting is one rule in three places.** `combat::Trigger` is the
reload and the burst — `tick` every step, `pull(dt, stats)` when aimed,
`hold` when not (`pull_single`, the one-shot pull an execution fired
with, has had no caller since feature 104) — and `Bim::trigger`
replaced the three fields that were it. `Combat::fire` rolls the hit and
`Combat::step` the dodge, the cover and the damage, and nothing else
does: a **sentry** (`combat::Sentry` — id, position, weapon, a `Skill`,
`dug_in`, its own `Trigger`) is fired by `tick_combat` after the crew
through the same `aim`/`fire_as`, in the crew's room only, and stands
**after the crew on the bodies list** a hostile bolt looks for, with no
armour and "peeking" only when dug in with sandbags anywhere on the
line from the bolt's origin (`Sight::cover_anywhere_between`). A hit
past the crew's count is a sentry's: `tick_combat` routes it to
`sentry_hits` rather than `strike`, and `enemy_strike_sentry` is the
melee case. The world sets the list every step (`set_sentries`, which
keeps a known id's trigger) and reads back `take_sentry_hits`.

**A sentry never runs out of shots** (feature 88): nothing in the game
carries ammunition, so `Sentry::shots`, `Game::take_sentry_shots` and the
hold-when-dry branch are gone, and the field they were counted for is
gone from the world's `Deployable` too. What the owner's talents do to
the turret is the `Skill` instead — three fields feature 88 added to it
and the `Bolt`, all of them one for everybody else:

- **`Skill::damage`** multiplies a bolt's damage at *any* distance, where
  `point_blank` bites only within the weapon's sweet range. It goes onto
  the `Bolt` as `Bolt::damage` and is multiplied in where the bolt lands.
- **`Skill::range`** is *tiles added* to the weapon's range, applied in
  `Skill::stats_at`. It goes onto the `Bolt` as `Bolt::range`, and
  `Bolt::stats()` — the weapon's stats with those tiles on — is what the
  damage curve is read off where it lands, so the curve a bolt lands on
  is the one it was aimed along. A longer reach is therefore also a
  gentler falloff, which is the point of an optic.
- **`Skill::armour_protection_add`** is added to a worn piece's
  protection in `Game::strike`, *after* `armour_protection` has
  multiplied it: an engineer's *higher quality armour*.

**A bolt dodged behind sandbags is in the bags.** `Combat::step` asks
`Sight::cover_between` (was `covered`, which is now it `.is_some()`)
and, on a dodge that was not a peek, records the tile and the damage
on `cover_hits` — `Game::take_cover_hits`, for the world to take off a
laid deployable's health. A sandbag *part* is asked nothing.

**Cover laid at run time.** `Sight::set_laid_cover(&[Rect])` marks the
list over the layout's (`set_cover` keeps both, `layout_cover` and
`laid_cover`, and re-marks), a no-op when the list is what it was; a
fresh `Sight` has none, which is why the world says it again after
every relayout, join and unjoin. `Game::set_laid_cover`/`laid_cover`.

**`Kind::Deploy { x, y, sentry }`** is a room tile: `GoToDeploySpot`
(`deploy_stand`: the nearest of the four tiles beside it, else the tile,
free and reachable) then `Deploy`, `rest_minutes` of `Action::Chop` with
`effort` on it, and `Room::deployed` gets `(who, tile middle, sentry)` on
the way out — the world owns what was laid and takes the kit from the
pack then, so a deploy given up leaves the kit. `Game::deploy(who, tile,
sentry, minutes)` is the order (a live one: `interrupt_for_order` and
`drop_ordered`), `deploy_tile_ok` the tile check, `is_deploying` the
question, and `JOB_DEPLOY` (28) the job code. **A hit drops it**: `strike`
calls `drop_task` — suspended and not kept — unless `set_steady_hands`
named the Bim. `set_work_factors` is `(craft, build)` a Bim, multiplied
into `effort` in `tick_bim` for a `Kind::Craft` or `Kind::Build` on hand
and nothing else — the **craft half is one for everybody** since feature
88 took *quick hands* off the engineer's tree, and the pair is kept
because the mechanism is the room's rather than that class's. `Room::built` is `(site, who)` now, and `Order` grew
`only: Option<usize>` — `craft_on_offer` skips an order that is somebody
else's — for the armourer's repair. `Item::footprint` knows the two kits
(23 → 2×2, 24 → 2×3).

**`Game::working_at(who)`** (feature 91) is the other side of the same
errand, for the app's charge bar on the tile: the middle of the tile a
deploy is laying its kit on, or of the construction site whoever it is
is building, in **room** units, with `Task::progress` beside it. It
answers for `Kind::Deploy` and `Kind::Build` and for nothing else, and
the walk counts towards the number — a bar standing on the tile before
the builder arrives is what says *that* tile is the one being worked. A
site the world has stopped asking for has no entry in `Room::builds` and
so no bar, which is the right answer: nothing is being built there any
more.

## One shooter, and the soldier's skill on it (feature 75)

**A `combat::Skill` is what a Bim's talents do to the one shooter**, and
there is still one hit calculation. The world hands the room one a Bim
every step (`Game::set_skills`; `Skill::NONE` for anybody it does not
name — a sentry, an enemy, the crew without a class) and it is applied
in exactly one place each: `Skill::stats(weapon)` is the weapon's
numbers through it — the odds multiplied and clamped to one, the far
odds the near with *deadeye*, the trigger rate multiplied — and
`tick_combat` reads those in place of `weapon.stats()` for the Bim's
aim, its trigger and its shots; `Combat::fire_as` (which `fire` is, with
`Skill::NONE` and no shooter) rolls the hit off them, uses
`skill.walking` in place of `WALKING_ACCURACY` on the move, and puts
`skill.point_blank` on the `Bolt`, where `Combat::step` multiplies the
damage while the bolt has flown no further than the weapon's `sweet`;
`Combat::set_own_cover_dodge` is each own body's odds in cover, read by
`step` for a hostile bolt in place of `DODGE_IN_COVER`; `skill.dodge` is
added to the armour's odds on the bodies list; `skill.melee` multiplies
a blow's damage, fist or blade, as it is swung; `skill.pace` is
`Game::runner`, multiplied into the pace while `sees_any` says an enemy
is in sight; and `skill.nerve` is one of the two things that keep
`is_fleeing` false. `Bolt` and `Hit` carry `by: Option<usize>` — the
crew member whose bolt or blow it was, `None` for a sentry's or an
enemy's (`brawl` takes it, `struck` says none) — for the world's
*rampage*.

**The brace is `Bim::braced`**, set by `Game::set_braced` (the walk
halted, the errand interrupted onto the queue, the post dropped) and
cleared by `interrupt_for_order` — so any order that moves the Bim ends
it — and by `tick_combat` the step the Bim is down, out cold or outside.
Braced, a Bim is armed like a recruit (`armed` says so), takes no errand
(`pump_queue`, `consider_errand`) and never flees. `Character::set_braced`
draws four heavy brackets round it (`BRACED`), a stance held, inside the
recruit ring. `Bim::rampage` is the world's count, kept here so a save
carries it; the room only reads it back (`rampage`, `set_rampage`).

**And since feature 91 a braced Bim is drawn braced.** The brackets say
*that a thing is on*; the figure says *what it is doing*, which is what
the player asked for. Two changes in `Character::draw`, both off the same
flag and both drawing only: the **feet are planted** — set
`BRACE_FEET_APART` wide, the toes turned out `BRACE_TOE_OUT` and the
heels `BRACE_SET_BACK` under the body, with no stride left in them
whatever the legs were doing the step before — and the whole figure is
drawn at `BRACE_CROUCH` of its size **against an unchanged shadow**,
which seen from directly above is the only way a picture can say
*lower*. The width is the number that matters and it is wide for a
reason: **the boots are drawn before the torso**, which is 33 units
across, so a foot inside 16 of the middle is a foot nobody ever sees —
standing at ease they are hidden entirely and a stride is what brings one
out. The first cut set them 11 apart and the two pictures were identical.

**The medic's beam and surge are two more things the world sets on a
body** (feature 76). `Game::set_held(Vec<Option<health::Beamed>>)` is
what each body's health tick runs under — `Health::update_held`: nothing
bleeds, the blood comes back at the beam's rate (or the body's own once
nothing is open, whichever is more) and the parts above nothing mend at
`mend × HEALTH_RECOVER` — and a body a beam holds drips no blood on the
deck either. `Game::set_doctoring(Vec<health::Doctoring>)` is what its
bandaging and treating run at: `bandage` and `treat` are effort factors
on the working step (in the `effort` product beside the engineer's, by
the errand on hand), `bare` lets `Game::treat` start with no medkit on
the shelf at all — `Kind::Treat { bare: true }` skips `GoToKit`, spends
nothing and says so on `Room::treated`'s fourth field — `clean_hands`
and `treated_to` are what `Health::treat_as` leaves and where the part
starts again from. Every dressing and treatment that lands is a
`game::Healed { helper, patient, with: Healing::{Bandage, Medkit, Bare} }`
on `Game::take_healings`, for the world's experience. `Bim::beaming` is
the medic's link as the world last set it (`Game::set_beaming`), cleared
by `Game::order` for any errand — a walk keeps it — and by `tick_combat`
when the medic goes down; the world reads it back and breaks the link.
`Bim::surge` is a `bim::Surge { left, closing }` (`Game::set_surge`):
while it runs `Game::strike` absorbs the whole of a hit and returns —
no wound, no armour drained, no trauma — `tick_combat` counts it down,
and a `closing` one closes every open wound as it ends.
`Character::set_surging` draws the halo (`character::SURGE`).
`Game::crew_at(x, y)` is the living crew member under a room point, for
the beam's key, and `Game::sees(who, p)` the trace's question the world
asks of the beam's line.

**A grenade is `combat::Grenade`**: thrown by `Game::throw_grenade` →
`Combat::throw` (a `Cue::Throw`), in the air for `GRENADE_FLIGHT` of its
fuse and then on the tile with the fuse blinking quicker as it runs
down (`Grenade::pos`, `Combat::draw`), and `Combat::tick_grenades` hands
every one whose fuse ran out to `Game::burst` and lights a `Blast` (a
`Cue::Burst`). The burst is one rule over four lists, in a fixed order
so two runs on one seed roll the same: every own body up and on the
deck, then every target standing, each within `radius` of the burst
with `line_clear` to it (walls and shut doors, never sandbags), takes
`damage` falling in a straight line to half at the edge, halved again
peeking or with `cover_between` bags on the burst's side, on a part off
the combat stream — an own body through `Game::blast` (a `strike`, no
cut, then `Blood::splash` the way a cut splashes when anything got past
the armour), a target as a
`Hit { blast: true }` for the world to carry to the other room's
`blast`; then every sentry in it onto `sentry_hits`, and every laid
sandbag tile in it onto `bags_blown` (`take_bags_blown`) for the world to
take the deployable off. `Game::line_clear`, `is_deck_tile` and
`grenades` are the readouts the world's checks use. `quiet()` counts a
grenade out as not quiet.

## The tank's wall, its armour and its hits (feature 77)

The fourth class adds no new seam: it is three more fields on
`bims::combat::Skill`, a flag and a count on `Bim`, and one new list on
`Combat`. The world works every one of them out
(`crates/world/CLAUDE.md`, "The tank: the wall, the taunt and the
hits"); the room applies each in exactly one place.

- **`Skill::armour_drain`** is what a worn piece's health loses of the
  damage that gets past its protection, and `Skill::armour_protection`
  what that protection is multiplied by (*plated*). Both are read in
  `Game::strike` and nowhere else: the piece can take
  `health / drain`, and only what is past *that* reaches the body — so a
  kevlar at 20 absorbs 40 on a tank and 20 on anybody else, and **the
  piece's stored health is never doubled**, which is what lets it move
  between bodies unchanged. `Skill::iron_frame` is read at the top of
  the same function: a hit rolled on the head lands on the body, so the
  kevlar takes what the helm would have.
- **`Skill::walk`** is the pace multiplied at all times, where
  `Skill::pace` (*runner*) is multiplied only while an enemy is in
  sight; both go into `tick_bim`'s pace product.
  **`Skill::steady_pace`** picks `Health::pace_steady()` over
  `Health::pace()` there — the blood's halving left out, the legs and
  the traumas still counted — and only while the body's kevlar is worn
  and unbroken. **`Skill::smash_rate`** is what `breach` heaves at a
  locked door with (`Door::smash_at`, `smash` being it at one): the
  progress goes faster, the heaves are heard at their own cadence.
- **`Bim::bulwark`** is the wall up, set by `Game::set_bulwark` and
  dropped by `tick_combat` the step the tank is down, out cold or
  outside, like `braced`. **`Bim::hits_taken`** is how many enemy hits
  have landed on the body since the last point of experience they made;
  `Game::count_hit_taken` bumps it where a hit is *applied* — the
  hostile bolts drained in `tick_combat` and `enemy_strike`'s blow — for
  a hit with no `by`, so what one of this room's own did (a soldier's
  grenade carries the thrower) is never one. Both are saved and the
  world hashes them.
- **`Combat::bulwarks`** is the walls standing among this room's own
  bodies, said every step with the skills
  (`Game::set_bulwarks`): a `Bulwark { who, reach, interpose }` each.
  `Combat::step` reads them **only for a hostile bolt**, so nobody
  shelters behind a tank but his own side: a body the wall `shields` —
  within its reach of the tank, the tank nearer the shooter than the
  body and within its reach of the line the bolt travels — counts as in
  cover and dodges the bolt like a peek's or the sandbags'. With
  *interpose* the bolt the wall did not turn aside lands on **the tank**
  instead, rolled afresh against his own armour's odds, and either way
  it is spent: a bolt he slips is never rerolled onto the body he stood
  for.
- **`Target::taunting`** is how far a taunt on that target reaches, in
  room units (nought for none) and **`Target::magnet`** whether it pulls
  blades; `Combat::set_taunting` sets them index for index like
  `set_peeking`, and the world says them right after the targets every
  step. `Combat::aim` prefers a taunting target inside its own radius
  over any nearer one — in reach and in sight as ever, so a taunt
  chooses between shots rather than making one possible — and
  `Tactics::charge` puts a *magnet*'s taunt at the head of its order the
  same way.

Two things to keep straight. A taunted target still has to be **seen**:
the bolt flies in the room the bodies are in, so a crewmate standing on
the line between the enemy and the tank takes it, taunt or no taunt —
which is the wall working, not the taunt failing. (The tank's *pack
mule* once rode on `Room::picked`, a load carried to a site; that went
with the haul of materials in feature 95.)

## The commander's aura, and the squad's orders (feature 78)

The fifth class adds no new seam either: three more fields on
`bims::combat::Skill`, one counter on `Bim`, and one list the world sets
every step. The world works all of it out
(`crates/world/CLAUDE.md`, "The commander: the aura, the squad and the
rally"); the room applies each in exactly one place.

- **`Skill::effort`** is a factor on the **working steps of every
  errand**, where the engineer's `work_factors` are a craft's and a
  build's alone: it goes into `tick_bim`'s `effort` product last. It is
  the one factor there that is nobody's own class's — a commander's aura
  lifts whoever is standing in it.
- **`Skill::nerve_hold`** is seconds a dying body holds its ground
  before it runs. `Bim::fear` counts up in `tick_combat` while
  `Game::would_flee` holds — everything `is_fleeing` used to be — and
  zeroes the moment it does not; `is_fleeing` is then `would_flee &&
  fear >= nerve_hold`. The hold is nought for everybody, so a body with
  no commander near it runs at once exactly as it always did, and
  `Skill::nerve` still beats the lot. `Game::fear(who)` is the readout,
  and the world hashes it.
- **`Skill::marked_accuracy`** is what the odds against the enemy a
  squad order marked are multiplied by (*focus fire*), applied by
  `Skill::stats_at(weapon, marked)` — `stats` is that with `marked`
  false — and read in `tick_combat` **after** `aim` has said which
  target it is. **`Skill::unhurt`** (*grit*, during a rally) drops the
  whole of `Health::pace()`'s hurt factor, where the tank's
  `steady_pace` drops only the blood's halving.
- **`bims::health::Beamed::bleed`** is what the bleeding is multiplied
  by: nought for a beam, which stops it dead, and a share for a
  commander's *steady ranks*, which only slows it. `Beamed::HELD` is the
  stop-it-dead entry every other caller wants.
- **`Game::squad`** is one `Squad` a Bim (`set_squad`, `squad_for_probe`,
  `serde(skip)` — the order itself is the world's), and
  `Squad::None` for everybody the world does not name, **always**
  including a Bim a player steers. `tick_combat` reads it three times:
  `muster_squad` puts a squad member under arms whether or not the alarm
  is up (and lets it go again when the order ends and the alarm is
  down); `squad_stand` runs in place of the gather ring and the body's
  own tactics — an attack is `plan_stand` with the marked target alone on
  the list, a fall back is `gather_round` anchored on the tile rather
  than on a player's Bim, stand ground halts the walk and plans nothing —
  and the aim holds its fire while a fall back is still walking and
  prefers the mark through `Combat::aim_marked`.
- **`Combat::aim_marked`** is `aim` with one target preferred over the
  taunt and over any nearer one, as long as it is seen and in reach; a
  mark that cannot be shot falls through to the ordinary rule.
  `Game::gather` is now `gather_round` with the nearest player's Bim as
  the anchor, and `Game::take_up_arms` is the weapon-out-of-the-pack half
  of `muster_crew`, shared with the squad's muster.

Two things to keep straight. A squad order is the *world's*, so a member
that is out of range when it is given is never in it and one that walks
out of range afterwards stays in it — the members are fixed at the
order. And `Game::is_standing_still(who)` is what *anchor* reads off the
commander: it is `!is_walking()`, live, so a test that wants him still
has to `halt_for_probe` him — `put_for_probe` moves a body without
dropping its walk.

## A class wears its own kit, and the room is only told (feature 81)

`character::Outfit` — `Plain`, `Engineer`, `Soldier`, `Medic`, `Tank`,
`Commander`, in the order `world::Class::ALL` lists the classes — is what
a crew member's class shows on the body. It is **drawing only and the
world's to say**: `World::hand_the_room_the_outfits` walks the crew every
step and calls `Game::set_outfit(who, class.outfit())`, and nothing in
the room decides it. So the crew past the players (a hire, a mercenary)
and every one of a station's residents are `Plain`, which is what every
Bim looked like before.

- **It is left out of a save**, `serde(skip)` on `Character::outfit`, for
  the same reason the draw buffer and a surface's settlement are: it is a
  function of what *is* saved (`World::classes`), and the step that
  follows a load puts it back before the first frame. No `SAVE_VERSION`,
  no `wire::PROTOCOL`, no checksum moved for this feature.
- **A class says itself three times over**, because there is not much of a
  body to look at from directly above: the coverall **dyed** `DYE` (0.45)
  of the way towards the class's colour — a *mix*, so the [`Uniform`]
  underneath still says crew or station, and an engineer of the crew is an
  ochre-ish blue where one ashore is an ochre-ish orange; something on the
  **head** (`draw_class_head`, between the hair and the nose so the face
  still shows under a peak); and something over the **chest and
  shoulders** (`draw_class_rig`, after the arms so a pauldron caps the
  shoulder it is strapped to and a strap crosses the vest under it).
  A pressure suit is a pressure suit: none of the three is drawn, and
  nothing is dyed, while `Uniform::Suit` is worn.
- **A helm is worn over the head kit, not beside it.** With armour on the
  head the class's own cap is left off and a band of the class's colour
  goes across the helm instead, so a helmeted crew is still read a class
  at a time. The soldier's **sunglasses** are the one piece nothing is
  worn over: they go on last, over a helm as well.
- **`Outfit::scale()` multiplies `Character::body_scale()`** — the tank
  1.08, the soldier 1.02, the medic 0.96 — so the model differs and not
  only the paint. Like `Look::scale`, it is a picture and never a rule:
  `PICK_RADIUS`, `BODY_MARGIN`, the reach and the checksum are the same
  for all.
- `character::portrait(look, outfit, heading, list)` takes the kit now, so
  the setup tab's chooser shows the class picked under it.

A class added later wants an arm in `Class::outfit` (`crates/world/src/class.rs`),
one in `Outfit`, and an arm in each of the two drawing functions — miss the
last two and it looks like everybody else.

## A droid is not a Bim, and the deck serves both (feature 83)

`crates/game/src/droid.rs`. The endgame enemy is a machine race, and the
first rule about it is what it is *not*: a [`Droid`] is not a `Bim` and
has no `Character`. No bunk, no errands, no memory, no blood, no wounds,
no traumas, no gear and no pack. What it has is a position, a heading,
four parts that break, an
arm that is part of it, and a route.

**`Game::droids` is beside `bims`, and there is one body index space.**
A droid-held station's room (`world::World::infested`,
`crates/world/CLAUDE.md`) has `bims` **empty** and `droids` full; the
deck's `Nav`, `Sight` and `Combat` serve both without knowing which is
which. `Game::crew_count()` is still the Bims, `Game::droid_count()` the
machines, and **`Game::body_count()` is the two together, the Bims
first** — which is the index every `who` the world hands in and reads
back is in. The handful of methods that had to learn it are the ones the
world asks of the other room: `is_alive`, `is_down`, `is_unconscious`,
`weapon`, `peek`, `exposed_at`, `dodge`, `body_pos`, `loot_cells`,
`take_from_body` (and `execute_body`, until the execution went in
feature 104), and `strike_droid` in place of
`strike`. Inside the room a `who` is still a Bim's, which is why nothing
else changed.

- **The body is four parts and there is no dying.** `DroidPart` — Head,
  Chassis, Arms, Legs, codes 0–3 — with `balance::DROID_HIT_ODDS`
  (0.05 / 0.60 / 0.15 / 0.20, adding to one) and its own healths a kind
  (`DroidKind::body`, `balance::{HUSK,TROOPER,WARDEN}_BODY`), every tier
  above one multiplying all four by `ARMOUR_TIER_STEP` the way a piece of
  armour's health climbs. **Head or Chassis at nothing is destroyed at
  once**: no dying state, nothing comes round. Arms at nothing and a
  gun's odds are halved (`DROID_ARMS_ACCURACY`) and a claw's damage is
  (`DROID_ARMS_DAMAGE`) — the Unmaker counts as a gun, so its odds fall
  and its strip does not. Legs at nothing and it cannot move and fights
  where it stands. **A hit on a limb already at nothing lands on the
  Chassis**, so shooting a Warden's legs off is never a way to make it
  unkillable. A droid wears nothing, so there is no armour step on it and
  a strip does nothing to one.
- **The part is read off the hit's own roll.** A `combat::Hit` carries
  `roll` — the unit draw `Part::hit_by` was read from — so a hit on a
  droid target is read as `DroidPart::hit_by(hit.roll)` instead. One roll
  either way, so *which kind of body a target turns out to be does not
  move the combat stream*: a fight against machines draws exactly what a
  fight against people draws.
- **Two weapons are built in.** `WeaponKind::Claw = 6` and `Unmaker = 7`,
  in `WeaponKind::BUILT_IN` and `EVERY` but **not in `ALL`**, which is now
  *the five a body can carry*. `WeaponKind::resource()` is `Option<u32>`
  and `None` for both, and that one thing is what keeps them out of the
  hold, the bench, a pack and a loot — `from_resource` searches `ALL`, so
  no code ever reads back as one. `WeaponKind::carried()` says it
  outright and `no_built_in_arm_is_ever_a_thing` pins it.
- **The Unmaker strips rather than wounds.** `WeaponStats::strips` /
  `strips_far` is a two-point curve like the damage, nought for every
  weapon but this one, and a tier scales it by exactly what it scales the
  damage. A bolt landing puts `strips_at(flown)` on the `Hit`, and
  `Game::strike_stripping` is where it is applied: with the struck part
  wearing an **unbroken** piece the piece loses that much *with its own
  protection ignored* and the part takes nothing — what the piece cannot
  take is **lost**, not passed on — and a bare part, or one whose piece is
  already broken, takes the plain damage the ordinary way. `Game::strike`
  is that with a strip of nought, so every other weapon goes through the
  code it always did. A medic's surge still takes the whole of it, and a
  tank's *iron frame* still moves a head shot onto the body first, so the
  strip lands on the kevlar.
- **The step is `Game::tick_droids`, after the crew.** It is the hostile
  half of `tick_combat` written for a body with no gear and no errands:
  the route walked (`Droid::walk`), a stand planned every `PLAN_EVERY`
  (`plan_droid_stand`), the doors forced (`breach_droid`), the melee
  lock, the aim and the trigger. A droid is only ever in a **hostile**
  room, so every shot is a `combat::Shot` for the world to fly in the
  crew's room, exactly as a hostile Bim's is. A machine that is a wreck
  does nothing at all.
- **Each kind stands differently.** `Tactics::stand_scored` is
  `stand_with_cover` with the **worth of cover said** rather than taken
  as read, and `stand_with_cover` is it at `COVER_WORTH`. A **Warden**
  takes the cover and peeks like a hostile Bim; a **Trooper** is scored
  at a cover worth of *nought* — it advances in the open and fires on the
  move, and never uses a peek, since a body shot at where it stands must
  not be shooting from beside a wall; a **Husk** has a claw, so
  `stand_scored` hands it straight to `Tactics::charge`. `taken` is
  every other machine's destination, so a wave does not pile onto one
  tile. The hunter's rule is a hostile Bim's: with nobody in sight a
  gunner walks to where something was last seen and from then on holds or
  closes and never gives ground.
- **A machine is a body for the doors and an eye for the sight.** It is
  in `update_doors`' list and in `shut_now`'s, so an unlocked door opens
  for one walking up to it; and it is in `set_hostiles`' eyes, so a room
  with nothing but machines sees the crew for itself. `breach_droid` is
  `breach` for a body that is not a Bim: the nearest locked door it can
  reach the panel of, heaved at until the lock gives. A machine never
  locks a door, so there is no lock of its own to undo.
- **Nothing is looted.** `loot_cells` is empty for a machine and
  `take_from_body` refuses one, wreck or not, and there is no
  down-and-alive state to finish off. What keeps the
  *window* from opening is the world's: the `down` list `set_visitors_down`
  is told is false for a machine, so a click on a wreck is a click on the
  deck.
- **The drawing is `Droid::draw`, and nothing of the Bim's.** Built from
  the `DrawList` primitives: dark gunmetal and steel — none of the
  uniform colours — with the sensors in `combat::HOSTILE_BOLT`'s red. A
  **Husk** is low and wide and crab-like: a flat hull, four short legs
  that scuttle out of phase, two forward claws that snap shut through a
  strike and hang and drag with the arms gone, and flat on its hull with
  the legs gone. A **Trooper** stands: a boxy chassis wider than deep, a
  small square sensor head with one red slit, the gun *built into the
  right forearm* — the barrel is the arm — a stub left arm, and two legs
  that step. A **Warden** is the heaviest: a broad chassis with shoulder
  plates, the head recessed between them behind a sensor band, and the
  Unmaker as a long twin-rail lance along one side with a ring at the
  muzzle that brightens as the trigger is held. Half-widths 15, 17 and 26
  room units, never over thirty, and all three navigate with
  `BODY_MARGIN` and are hit at `HIT_RADIUS` like a Bim. A **spark on the
  struck part at every hit**, and a fresh wreck spits for `WRECK_SPARKS`
  seconds and then lies still. The Unmaker's bolt has its own look in
  `Combat::draw`: a long crackling line, the discharge jagging off a
  straight core in `LANCE_KINKS` steps.
- **A wreck is its own drawing, not the standing machine shrunk.** A
  destroyed one goes to `draw_husk_wreck`, `draw_trooper_wreck` or
  `draw_warden_wreck` instead — the three live ones are never asked
  about `destroyed` at all now — and each is a *broken* machine: a
  Husk's shell split down its length with the two halves tipped apart,
  its legs snapped off and thrown clear and one claw lying open on the
  deck; a Trooper on its back with the chest torn open, the head
  knocked off its socket and lying ahead of it and the gun arm snapped
  at the elbow beside it; a Warden broken across with the skirt come
  away aft, one shoulder plate torn clean off and the Unmaker in two
  pieces with the muzzle ring out. Under all three is the same mess
  (`Droid::draw_wreck_ground`, drawn before the hulk so the hulk lies
  on top of it): the `SCORCH` burnt into the deck, the `COOLANT` run out
  of it — a machine's answer to the blood round a body, and never the
  Bim's red — and `SHARDS` of plate flung clear. Where every piece of
  that lies is `Droid::scatter`, a hash of **where the machine fell**
  (its position, its wave and its kind) rather than a roll: a drawing
  may not touch the sparks' own stream, and a wreck has to lie the same
  way on every frame and the same way again after a save and a load. A
  fresh one still burns — `Droid::draw_rift` puts an `EMBER` in the
  split, out by `EMBER_LIFE` (9 s) — and spits sparks for
  `WRECK_SPARKS`; an old one is cold.
  `a_wreck_is_its_own_picture_and_lies_the_same_way_every_frame` pins
  all three against the standing machine, and `BIMS_DROIDS=1` is the
  rack with the wrecks in its last column.

The wave plan — how many machines, how many waves, when the next comes
and which stations are held — is the world's: `crates/world/CLAUDE.md`,
"The machines hold a station, and they come in waves".

## A body in somebody's arms (feature 86)

A medic can pick a crewmate up and carry it out of the fire. Two fields
on `Bim` and one pass at the end of the step:

- **`Bim::carrying`** is whom this body has in its arms (saved, and in
  `world_checksum`); **`Bim::field_medic`** is whether it is a hired one,
  said by the world every step like `Squad` and neither saved nor hashed.
- **`Game::carry_the_carried`** runs in `simulate` **after**
  `separate_under_arms` — before it, the shove would push the body out
  of the arms it was just put in — standing each carried body a third of
  a tile ahead of its carrier's facing and letting go of any carry that
  can no longer hold: a carrier dead, out cold or outside, or a body that
  died in the arms. `separate_under_arms` skips both of a carrying pair.
- **`Game::{can_take_up, take_up, set_down}`** are the rule, and
  `carrying`/`carried_by`/`is_carried` the readouts — `carried_by` is
  derived by a scan rather than kept, since a crew is a handful of bodies
  and one truth about a carry is one thing to put back when an index
  moves. `needs_rescue(who)` is what a body has to be to be worth
  fetching: alive, on the deck, and out cold, dying, or bleeding through
  a wound nobody has dressed.
- A carrier **holds its fire** (the `armed` test in `tick_combat`) and
  walks at `CARRY_PACE`, multiplied into `tick_bim`'s pace product; a
  carried body shoots nothing either.

**A field medic's own branch is `Game::rescue`**, the first thing
`bot_stand` tries, ahead of the last stand and of the player's standing
order. Carrying and clear of the fight (`Game::out_of_harm`: no target
up within `RESCUE_CLEAR` tiles **and** nothing a body there could see) it
sets the body down, and the medical row takes it from there — doctoring
a crewmate wants it out of harm (or the room calm), which is exactly
where it has been carried, so the treatment starts there and then
rather than when the fight is over. Carrying and still in it, it runs `Tactics::flee` with the
body in its arms. Carrying nobody, `Game::worth_fetching` is the nearest
crewmate within `RESCUE_LOOK` that is **out cold or dying** — a body
merely bleeding is on its feet and can walk itself out — in nobody's
arms, still in the fire, and reachable. Nothing to fetch and it falls
through to the rest of `bot_stand` and fights, which is where the other
half of the trade shows: `plan_stand` hands `Tactics::stand_with_cover`
`combat::KEEP_BACK_WORTH` instead of `DISTANCE_WORTH` for a field medic,
so it stands at the far end of its reach. That `distance_worth` is a new
last argument on `stand_with_cover` and `stand_scored`, and is the only
reason either signature moved.

The command, the contract, the medkits and the restock are the world's —
`crates/world/CLAUDE.md`, "A medic carries a body out".

## The machines have a list of their own, and some bodies shelter (feature 94)

Every fight before this one was one room against another: one
`Combat::targets` a room, shared by every body in it, and a bolt flying in
the crew's room alone. A **town the crew are defending** is not that — the
machines and the town's own people stand in the residents' room together —
so two things were added, both of them empty and inert everywhere else.

- **`Combat::machines`**, the machines' own target list, with
  `Combat::machines_cross` saying where it turns from the world's (the
  crew, across the seam) into **this room's own bodies** (by index
  `i - cross`). `Combat::machine_targets()` falls back to `targets` when
  the world has given them none, and `machine_cross()` to its whole
  length — so in every other room `tick_droids` reads exactly what it
  read before. `Game::set_machine_hostiles(at, cross)` sets it with a
  belief of the machines' own (`Game::machine_seen`, `last_seen` for
  their list; `believe` is the one function both go through now), and
  `clear_machine_hostiles` puts it back.
- **The routing is the index.** In `tick_droids`, a shot or a blow at a
  target below `cross` is the `Shot` it always was, for the world to fly
  in the crew's room; at or above it, the bolt **flies here** hostile
  (`Combat::fire(.., true, ..)`, which `Combat::step` lands on this
  room's own bodies) and a blow goes through `Game::enemy_strike`, which
  re-checks the reach. `Combat::aim_among`, `melee_among`,
  `within_reach_among` and `brawl_at` are the existing rules over a
  target slice the caller names; the old methods are those over
  `self.targets`.
- **Their war is their own list's.** `tick_combat` passes
  `war || machine_war` to `tick_droids`, `machine_war` being any machine
  target at all — because a town's room is *friendly*, so `war`
  (`hostile_bodies && …`) is false in it and the machines would stand
  about doing nothing.
- **`Game::sheltering`** is which of this room's own bodies take no part:
  `Game::set_sheltering(&[bool])`, said every step by the world, posts a
  body newly told to shelter at the **nearest bunk** — the nearest house
  — and `muster_crew` leaves it alone, so the alarm never arms it.
  `Game::under_attack()` is "the world has said who shelters", and it
  does two more things while it holds: `muster_crew` musters body nought
  as well (a station's room has no player in it, and `players` is never
  under one, so the **guard** would otherwise be skipped as a player's
  own and stand there unarmed), and `bot_stand` goes straight to
  `plan_stand` — a town's defender has no player to gather round and
  nowhere to fall back to.

Neither list is anybody's but the world's: the room decides nothing about
who is on which side. The world's half is `crates/world/CLAUDE.md`,
"Defending a town".

## The fight's passing lights (feature 98)

`crates/game/src/fx.rs`, held as `Combat::fx` (`serde(skip)`). **Every
gun is a laser**, drawn in one language: a core past white that blooms
(`fx::laser`, `fx::hot`, feature 97's emissive), in the side's colour —
`FRIENDLY_BOLT` blue for the crew, `HOSTILE_BOLT` red for the enemy,
whatever the gun — and the guns told apart by **shape, length, thickness
and rhythm**: the pistol one short clean bolt, the shotgun five short
pulses whose spread opens as it flies (`PELLET_FAN*`, `PELLET_STAGGER`),
the auto rifle a thin dash with a shorter one behind (`PULSE_*`), the
sniper a beam from the muzzle to the bolt, faint where it left and bright
over the last `STREAK_LENGTH`. A tier above the first is `TIER_WIDTH`
thicker and `TIER_HEAT` hotter (`fx::tier_look`), never another colour.
The Unmaker's lance is as feature 83 drew it.

- **The lights are the host's, on the host's clock.** `Game::fade(real)`
  (`Session::age_effects`, called once a frame by both screens with the
  window's frame time, nought while paused) ages them in **real
  seconds** and switches recording on. A room nobody fades records
  nothing — every test, probe and server — and draws what it drew before
  this feature: the sim-time spark where a bolt lands and the whole-body
  hit flash (`HIT_FLASH`), which `Combat::draw` and `Game::render` fall
  back to when `Fx::is_on` is false. That is also why the save round
  trip's picture comparison still holds: neither session is faded.
- **Nothing the simulation reads.** Spawning is a push onto a capped
  list; no rule, no roll, no checksum reads one back, and which way a
  spark flies is `fx::scatter` (a hash), never the combat stream.
  `the_fight_s_passing_lights_are_the_host_s_picture_and_leave_the_world_alone`
  in `crates/ship/src/tests.rs` steps a lit and an unlit session side by
  side and asserts the same checksum.
- **A fresh effect is aged by at most `FIRST_FRAME`** (a sixtieth) in the
  frame it was spawned in: a long frame — the first of a run, a hitch —
  would otherwise age a four-frame flash out before it was ever drawn.
- **Where each is spawned.** The muzzle: `Combat::fire_as` for our own
  side, at the `from` it is given (the muzzle, feature 84); an enemy's is
  lit where its **body** is by `Game::lit_muzzle` — a hostile room's Bim
  beside its `combat.shoot`, a machine at `Droid::muzzle` (the end of the
  arm it is drawn with, not the eye its shot is traced from) — because
  the bolt itself flies in another room from a seam point. The flash, the
  scorch (not on a body) and the sniper's lingering beam:
  `Combat::step` where a bolt stops (`Fx::landed`); a bolt that flies out
  its range leaves only the sniper's beam (`Fx::spent`). The cut: a
  blade's blow, `Combat::brawl_at` for ours and `Game::enemy_strike` for
  theirs. The flash on the **part** struck: `Game::strike_stripping`
  (`Character::part_mark` says where head, body and legs are drawn) and
  `Game::strike_droid` (`Droid::part_mark`), which also bursts a machine
  the instant its head or chassis goes.
- **Where each is drawn.** Scorches after the blood, under the bodies
  (`Fx::draw_ground`); a part's flash right after the body it is on, so
  it follows the body; a machine's thrown plates after the machines
  (`Fx::draw_debris`, cold, non-emissive); everything else over the fog
  with the bolts (`Fx::draw_air` from `Combat::draw`).
- **The blade and the emitter take the side too.** A schword in a hand
  has its core lit past white in its edge's colour (`BLADE_TINT`,
  `BLADE_HEAT`; the crew's cyan, an enemy's red) and one on the deck is
  cold; `draw_gun`'s emitter glow is `Option<Color>`, red on an enemy's
  gun. A machine's sparks are `SPARK_HEAT` past white.
- **Shadows are soft.** `DrawList::soft_ellipse` (`SOFT_LAYERS`, three
  ellipses) under every Bim standing or lying and every machine, the same
  darkness in the middle as the old single ellipse. The shade along the
  inside of the walls is the ship painter's (`world_paint::wall_shade`,
  `crates/ship`).

`BIMS_FREEZE=n+f` (`crates/app/src/dev.rs`) pauses the game `f` frames
after the n-th shot or blow the crew's room hears — `down:n+f` after the
n-th machine destroyed — which is how these are caught in a screenshot.

**Every constant feature 98 added, in one place** (real seconds, room
units, and alphas as drawn):

| where | constant | value |
| --- | --- | --- |
| `fx.rs` | `CORE_WHITE`, `CORE_TINT`, `CORE_HEAT` | (0.92, 0.97, 1.0), 0.4, 2.4 — the laser core |
| `fx.rs` | `TIER_WIDTH`, `TIER_HEAT` | 0.15, 0.35 a tier above the first |
| `fx.rs` | `MUZZLE_PISTOL`/`_SHOTGUN`/`_AUTO`/`_SNIPER` | (life, size): (0.08, 5), (0.11, 8), (0.06, 4), (0.14, 9) |
| `fx.rs` | `MUZZLE_RAY`, `MUZZLE_SPIKE` | 12, 16 |
| `fx.rs` | `IMPACT_LIFE`, `IMPACT_RAYS` | 0.15, 3 |
| `fx.rs` | `SCORCH_LIFE`, `SCORCH_HOT`, `SCORCH_SIZE`, `SCORCH_MERGE` | 4.0, 0.35, (13, 7), 6 |
| `fx.rs` | `SCORCH`, `SCORCH_WARM` | rgba(0.035, 0.03, 0.025, 0.55), rgba(1.0, 0.52, 0.22, 0.5) |
| `fx.rs` | `BEAM_LINGER` | 0.25 |
| `fx.rs` | `STRUCK_LIFE`, `STRUCK` | 0.22, (1.0, 0.95, 0.85) |
| `fx.rs` | `CUT_LIFE`, `CUT_ARC`, `CUT_REACH` | 0.16, 100°, 46 |
| `fx.rs` | `BURST_FLASH`, `BURST_SPARK_LIFE`, `BURST_DEBRIS_FLIGHT`, `BURST_LIFE` | 0.22, 0.65, 0.35, 1.6 |
| `fx.rs` | `BURST_SPARKS`, `BURST_DEBRIS` | 14, 7 |
| `fx.rs` | `BURST_HOT`, `BURST_SPARK`, `BURST_SPARK_HEAT` | (1.0, 0.72, 0.38), (1.0, 0.82, 0.48), 2.0 |
| `fx.rs` | `DEBRIS_PLATE`, `DEBRIS_DARK` | (0.33, 0.36, 0.40), (0.15, 0.16, 0.18) |
| `fx.rs` | `FIRST_FRAME` | 1/60 |
| `fx.rs` | `FLARE_CAP`, `SCORCH_CAP`, `STRUCK_CAP`, `BURST_CAP` | 160, 40, 32, 8 |
| `combat.rs` | `PELLET_LENGTH` (was 14), `PELLET_CORE`, `PELLET_FAN`, `PELLET_FAN_GROW`, `PELLET_FAN_MAX`, `PELLET_STAGGER` | 8, 1.5, 2, 0.06, 15, [5, 1, 8, 0, 4] |
| `combat.rs` | `PULSE_LEAD`, `PULSE_GAP`, `PULSE_TAIL`, `PULSE_CORE` | 13, 5, 6, 1.2 |
| `combat.rs` | `BEAM_CORE` | 1.5 |
| `character.rs` | `BLADE_TINT`, `BLADE_HEAT` | 0.35, 1.9 |
| `droid.rs` | `SPARK_HEAT` | 1.6 |
| `draw.rs` | `SOFT_LAYERS` | [(1.25, 0.30), (1.0, 0.35), (0.75, 0.45)] |
| `ship/world_paint.rs` | `WALL_SHADE`, `WALL_SHADE_COLOUR` | [(0.10, 0.16), (0.24, 0.09), (0.42, 0.05)], (0.0, 0.01, 0.03) |

Gone with it: `combat::{BOLT_CORE_TINT, BOLT_CORE_HEAT, PELLET, TRACER,
STREAK, TRACER_LENGTH}` (the core is `fx::CORE_TINT`/`CORE_HEAT` now, the
orange, yellow and white-blue tints are the side's colour).


## A station's people walk a round (feature 102)

Feature 102 put the room's whole life sim behind one switch
(`Game::needs_enabled`, told off by the world for every room of a run) and
gave a station's people something to do with it off; feature 104 deleted
the switch with everything it hid (the next section), and the round is
what is left.

**`crate::routine` is a station's people's peace.** A `Routine` rides on
the `Bim` (so `take_crew`/`adopt` carry it — `adopt` shifts its stops with
the body): a `Role`, the stops (a point and the minutes to stand there),
which one it is making for and how long it has left at it. It is planned
**once** by `Game::set_role(who, role, gates, seed)` off the room's own
fixture lists (`routine::Anchors::of`: the airlocks and any gates the world
names, the trading desks, the benches and research desks and shelves as
work, the bunks and showers and the rest as rooms, and a lattice of
reachable open deck), with every choice off `seed` through its own `Rng` —
never the room's stream, which every fight and the survivor tests are
pinned to. A role whose fixtures are missing falls back on a wander over
the open deck (`Routine::fallback`). `Game::keep_to_routine`, in
`tick_bim` before the body is moved, walks it — **only while the body is
its own**: up, awake, not recruited, not posted (which is how sheltering
reads), not braced, running, carrying or on an errand; otherwise the wait
is dropped and the stop is walked to again when it is free. Standing at a
stop is `Character::set_lingering`, the same hold-still a post gets, so the
body does not wander off its stop. A stop with no route is passed over for
the next, one a step. The world deals the roles (`Residents::deal_roles`,
via `routine::deal`) once, when the room opens — a town's first is its
guard, the first of a station that trades stands at the desk, a mercenary
for hire strolls — and deals nothing to a machine or the dead.

## The needs deleted (feature 104)

The third step of the roguelike redesign deleted everything feature 102
had switched off (the root `CLAUDE.md`, "The old game deleted"). In this
crate that was the needs and everything only they reached: `needs.rs`,
`social.rs`, `schedule.rs`, `galley.rs`, `dish.rs`, `bath.rs`, `hydro.rs`,
`manager.rs` and most of `filth.rs`; hunger, sleep and the nights in a
bunk or on the deck, the heads and the bath compartment with its door, the
shower, company and the chat, the mess, the surroundings and a comfort's
lift, food, the pot and the stew, the plates and the dishwasher, the cold
store's counts, the bay's growing and its fibre, poisoning and
malnutrition, the timetable and the manager's targets; every errand in
`task.rs` that served them, the menus on the fixtures and the jobs on the
list; the diary's every entry but a crewmate's death; the helm as a job
(`Job::Helm`, `Game::set_helm`) with the flown trip; the execution
(`Kind::Execute`) with the human enemies; the knife's cue on the board;
and `Game::needs_enabled` and `Game::clock_runs` with their setters. **The
classic room went as a game mode** with the `room` command: it was the
behaviour test room, a galley, the heads behind their door, two bunks and a
day to live. What is here now is the room exactly as it ran in a run with
the needs off, and the survivor tests say so — `SURVIVORS` in
`crates/world/src/tests_survivors.rs`, `PINNED` and `PICTURES` in
`crates/ship/src/tests_survivors.rs`, their constants read off
`needs-sim-final` before anything went and never edited. The last commit
with the old room is that tag; `git show needs-sim-final:crates/game/src/needs.rs`
reads a deleted file again.

What stayed, and why each one looks as if it should have gone:

- **`blood.rs` is what `filth.rs` was, cut down to blood.** `blood::Blood`
  is a score a tile — `BASELINE` clean, `FOULED` the worst — kept on
  `Room::blood`: a drop takes `BLOOD_COST` off the tile it lands on
  (`Blood::drop_at`), boots walk it on (`Blood::track`), a cut or a burst
  throws it about (`Blood::splash`), `resized` carries it across a
  relayout, and it draws as the red wash and blobs it always was. Every
  other mess, the broom and sweeping, a Bim fleeing filth, the
  surroundings and poisoning went. **It stays because its rolls are on the
  room's one stream, which every fight draws from**: a drop's two
  `signed()` for its scatter in `Bim::tick_drips`; the boots'
  `chance(SPREAD_CHANCE)` (0.25), drawn only when a step crosses a tile
  edge off a tile with blood of at least `WORTH_CARRYING` on it
  (`Blood::track`, called in `tick_bim` either side of `move_body`); and a
  splash's `below(3)` for how many tiles and a `below` a pick for which
  (`Blood::splash`, off `Game::blast` and `Game::strike_stripping`). The
  draws, their order and the scores that decide them are exactly what
  `Filth` had, so a fight after the first drop rolls as it did — take one
  away, reorder them, or change what a tile scores and every fight
  re-rolls, which `SURVIVORS` would say. `Game::bloody_tiles()` is the
  count the tests read.
- **`fixtures.rs` is the fixtures as pictures.** The galley (`Worktop`,
  `Hob`, `Fridge`, `Dishwasher`, `galley_faces`), the table and chairs
  (`draw_table_top`, `draw_chairs`, `CHAIR_SIZE`), the bunks (`Berth` and
  its bedding), the broom locker (`Locker`), the bays and the fields
  (`Bay`, `TRAYS`), the heads (`Heads`: the pan and the basin) and the
  extras (`Still`, `Stills`) — each cut down to the state its picture needs
  and drawn **as a fresh room drew it**: the drawer shut, the hob cold
  with an empty pot on it, the cold store shut, the dishwasher empty, the
  broom in its locker, the bunks made, the trays planted with nothing, the
  cistern full and the tap off. That is what the designer showed through
  `ship::paint::fixtures` → `Room::draw_fixtures`, and what a run showed
  anyway, since nothing changed a fixture with the needs off. The shapes
  are emitted exactly as before, the fully transparent ones included,
  because `PICTURES` hashes the floats: the designer's picture of the
  playtest and combat ships, and a run's deck on `simulation` and on
  `droids`. A picture drawn the same another way moves it, and is re-pinned
  only with a note in the test saying why it is the same picture.
- **Every fixture frame is still a solid, in the same order.**
  `Room::solids()` — the first worktop, hob and cold store, the table, the
  bunks, the bays and the fields, then `Room::others` — feeds the nav
  grids, the push-out and the wander, so a solid dropped is a route that
  moved ("Adding furniture moves everything"). Stand-ins included: a
  layout with no bunk still gets one on the worktop's frame. **The one
  exception is the classic bath compartment's three walls**, zero-sized
  and off the map aboard: they were dropped with the classic room, and the
  survivor tests stayed green, so no route moved. The heads aboard never
  had a compartment — the pan and the basin stand on the deck — and with
  the heads' door gone `Maps` is one deck grid (`Maps::deck()`, beside
  `for_body` and `for_who` for the outside and the plain), where the
  classic room kept one per state of that door.
- **`Game::bare(seed, w, h)` is the arena.** The classic room's size and
  walls (`ROOM_W` × `ROOM_H`, 860 × 580) with nothing on the deck — no
  fixtures, no compartment, lit throughout, the untiled grid (`Room::bare`,
  `nav_tile` `None`) — and `bim::CREW` (two) Bims in it, each start drawn
  off the room's stream *between* the Bims as the classic room drew them,
  and a box of `BANDAGES_AT_DAWN` dressings apiece that draws nothing. It
  is what the room's own tests (`game`, `order`, `sight` and `bim`), the
  world's `tests_droid.rs`, a test in the app's `crew.rs` and the probes
  stand bodies in; a test that wants somewhere out of sight builds it on
  top (`room_with_a_closet`), and `Game::drop_for_probe(at, weapon, owner)`
  lays a gun on the deck with nobody knocked out.
- **The room's clock stands where the room was built.** `Game::simulate`
  never advances it: `Game::wind_clock` is the one thing that moves it,
  and the world winds every room it builds — the ship's alone, a joined
  deck, a station's — to the world's minutes as it builds it
  (`crates/world/CLAUDE.md`, "The loop"), and the crew's calendar with a
  probe's `set_day_for_probe`. That was `set_clock_runs(false)`, told every
  step by the world's `hand_the_rooms_the_run`; with the switch gone it is
  simply what a room does, and nothing tells it. The night's wash over the
  deck and a diary entry's time are read off it, and so is `World::day`.
- **`born_year` and `born_day` stay on `Bim`**, though only the About tab
  reads them: they are two draws on the room's stream at a Bim's making,
  and dropping them would re-roll everything after.
- **The codes.** The `SPOT_` codes closed up to 0..=20 (`SPOT_NAMES` is
  twenty-one long) and `Job` to 0..=3 (`WORK_NAMES` is four), both tables
  pinned by their length tests; the `HIT_` codes (1 to 8 and 10 free), the
  `JOB_` activity codes (25 free, 19 and 20 never said) and
  `What::CrewDied` (30) kept their numbers, since the app matches them
  rather than indexing by them. `SAVE_VERSION` 35 covers the room's
  changed shapes along with the world's: the feature bumped it once.
- **The probes.** `scratchpad/` keeps `crowding.rs` and `layout.rs`, both
  ported onto `Game::bare`, and `droidwreck.rs`, which draws the machines
  with no room at all; the needs probes went, and `modules.rs` lists the
  room's modules as they are now (`scratchpad/CLAUDE.md`).

## The Guardian: a shield in front, a turn in sub-steps (feature 100)

`DroidKind::Guardian` (code 4), a fourth machine, only ever at tier three
(`droid::guardians_of`: `n / 8` of a wave of `n`, at least one from four,
out of the Troopers' share; `wave_kinds(n, tier)` takes the tier now).
Body `balance::GUARDIAN_BODY` (16 / 110 / 25 / 30), pace
`GUARDIAN_PACE` (0.7), half-width 28, `BODY_MARGIN` like the rest. Its
arm is `WeaponKind::Sweeper` (8), built in like the Unmaker — in
`BUILT_IN` and `EVERY`, never `ALL`, no resource. Arms at nothing halve
the Sweeper's **damage** (`Droid::stats`: a beam rolls no odds to lose);
legs at nothing stop the walk and nothing else.

- **The facing is a unit vector**, `Droid::facing`, kept for every kind
  and read for the Guardian alone. `Droid::heading` (the angle) is read
  off it for the picture only. **The turn is `Droid::turn_toward`**: whole
  sub-steps of `TURN_STEP_COS`/`TURN_STEP_SIN` (1.25°) at
  `TURN_STEPS_A_SECOND` (60), the sense a cross product
  (`Vec2::perp_dot`), the rotation `Vec2::rotate_by` — no angle is worked
  out, so two platforms turn it alike to the bit — the fraction of a
  sub-step a step's time does not buy carried on `turn_left`, and within
  a sub-step of the want it faces the want exactly. `Droid::walk` does not
  ease a Guardian's heading the way it eases every other kind's.
- **The shield is `Droid::shield()`**: `Some(facing)` for a standing
  Guardian, `None` for a wreck and every other kind; it cannot be broken.
  The world hands it across with the targets
  (`Game::set_hostiles_shields` → `Combat::set_shields` →
  `Target::shield`), turned through the station's frame, and it is
  decided **where a bolt resolves**: `Combat::step` stops a friendly bolt
  that would reach a shielded target (its line within `HIT_RADIUS` — a
  miss flies on) coming in from inside the front arc
  (`combat::shield_stops`: the heading dotted with the way back along the
  bolt, `>= GUARDIAN_SHIELD_COS`, the edge stopped) where it crosses
  `GUARDIAN_SHIELD_RADIUS` (34), rolling nothing — no `Hit`, no scorch, a
  `Cue::Ricochet` and `Fx::shield`, the plate flaring there. A blow from
  the front is stopped the same way in `Combat::brawl_at`, before the part
  is rolled. A grenade's burst (`Game::burst`) never asks.
- **`Game::tick_guardian`** is its branch of `tick_droids`: no melee lock
  and no peek; it turns towards the nearest crew body it sees from its own
  eyes (`Combat::aim_among`, so a taunt's pull comes first) or, with none,
  the way it walks; its stand is `stand_scored` with cover worth nought
  and **distance worth nought**, so the Sweeper's worth falling off past
  its sweet range (`balance::SWEEPER`'s `accuracy_far`, which is the
  tactics' alone — a beam rolls no odds) walks it in to that range; and
  when the Sweeper is ready and the target is within `WINDUP_COS` (15°) of
  its facing it **plants its feet and winds up** (`Beam::WindUp`: the aim
  and the target fixed, the heading held — `Beam::holds_heading`,
  `turn_toward` a no-op — for `SWEEPER_WINDUP`), lets the beam go, and
  cools (`Beam::Cooling`, `SWEEPER_COOLDOWN`).
- `tests_guardian.rs` is the rule: the shield's dot product and its edge,
  bolts from inside and outside the arc, a blow, a grenade, the turn rate,
  the heading held through a wind-up, a taunt, and the legs and arms.

### The Sweeper: a beam wound up, swept, and laid where its targets stand

`WeaponKind::Sweeper` is not a bolt. At the end of a wind-up
(`Game::let_the_beam_go`) the Guardian stays planted, its heading still
held, in `Beam::Sweep` for `SWEEPER_SWEEP`, then cools; the side the sweep
starts from alternates (`Droid::sweeps`). The beam leaves the **lens**
(`Droid::muzzle`) and turns from 10° one side of the fixed aim to 10° the
other (`combat::SWEEP_HALF_COS/SIN`), out to the weapon's reach (twenty
tiles, a tier-three reach longer), each body it crosses taking the
Sweeper's damage — thirty at tier one, scaled by the tier, halved with the
arms gone.

- **It travels as a `Shot`, recorded where the Guardian stands**:
  `Combat::shoot_sweep` records one with `Shot::sweep` = the aim point the
  sweep ends on and `Shot::at` the one it starts on — two points rather
  than an angle and a side, so the station's frame carries it across turned
  or mirrored by the points alone. The world lays it in the crew's room
  (`Game::enemy_sweep`); in a town under defence, where the machines'
  list has the room's own people on it, the Guardian lays one in its own
  room as well (`drawn: false` — one room draws a beam).
- **It is resolved where its targets stand**, by `Combat::sweep` and
  `Combat::step_sweeps` (after the bolts in `Combat::step`): `SWEEP_STEPS`
  (10) fixed sub-steps plus the first, one every `SWEEPER_SWEEP / 10`
  seconds of the sweep's own clock whatever the step, the direction turned
  by the written-out 2° (`SWEEP_STEP_COS/SIN`) with the sense a cross
  product. At each: the beam out to its reach, stopped at
  `Sight::first_opaque_along` (walls and shut doors), and every one of the
  room's own bodies within `HIT_RADIUS` of that segment (plain arithmetic,
  `off_segment`) and not rolled yet this sweep is rolled **once**: behind
  bags and not peeking it is not hit; peeking or behind a tank's Bulwark it
  dodges as a bolt would, *interpose* putting it on the tank; a
  tier-three body's own dodge after; else a hostile `Hit` on
  `wounds_taken`, which `strike` applies, so a surge takes it whole. It
  passes through bodies and never looks at a target, so it never touches a
  machine.
- **The picture** (commit 3's look aside): a drawn sweep is laid in
  `Combat::draw` over the fog with the bolts, lens to where the last
  sub-step stopped; a wall that stops it takes a scorch a sub-step
  (`Fx::burn`), a streak along the wall; a body it hits flashes
  (`Fx::landed`).
- `tests_guardian.rs`: each body once and through them, a wall and a shut
  door, bags, a peek and a Bulwark, a surge, a town's own sweep missing
  the machine in its way, the arms, and the heading held to the sweep's
  end.

### The Guardian's look

`Droid::draw_guardian` and `draw_guardian_wreck`, with two passes of its
own in `Droid::draw`: `draw_guardian_under` before the body (the shield's
arc faint on the deck, and while it winds up the **wedge** the sweep will
cross — `WEDGE_LINES` faint lines out from the lens to `WEDGE_REACH`) and
`draw_guardian_over` after it (the **plate**: nine overlapping segments
at `GUARDIAN_SHIELD_RADIUS`, a translucent red band and a thin rim;
the wind-up's **targeting line**, lens to the fixed point, flickering;
on a wreck the plate **collapsing** — shrinking and flickering out over
`SHIELD_COLLAPSE` — and **smoke** rising for `SMOKE_LIFE`). A broad,
hunched walker: a humped carapace, two stomping legs with wide feet set
out beside it, the shield's emitters at the front corners (dark with the
arms gone) and one large **lens** in the middle of the chassis
(`LENS_AHEAD`), the beam's origin (`Droid::muzzle`).

**What glows is what is past white, and nothing else is drawn to glow**
(feature 97's bloom: threshold one, additive): the lens
(`Droid::lens_glow` — a slow pulse under white at rest, climbing past it
over a wind-up, white-hot through a sweep, dimmed as the head is shot
away), the plate's rim (`SHIELD_RIM`), the targeting line, the beam's core
and the burning point where a wall stops it (`combat::SWEEP_HEAT`), and
the shield's flare where a bolt stops (`fx::draw_shield_flare`: the plate
lit over `SHIELD_FLARE_SPAN`, its rim past white). No halo and no glow
layer is drawn by hand. Every flicker is a hash of the moment
(`Droid::scatter`, `fx::scatter`), never a roll.

`Droid::wind_up` is the one reading the picture takes of the rhythm —
how far through, the aim and the point — and a machine held `lit` for a
picture is wound up whole, aimed six tiles ahead, which is the rack's
second column (`BIMS_DROIDS=1`, four rows now). `BIMS_FREEZE=sweep:n+f`
and `shield:n+f` (`Cue::Shielded`, a bolt or a blow stopped on the plate)
are how the beam and the flare are caught in a live fight; the root
`CLAUDE.md` has the pictures' recipe.
