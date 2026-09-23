# The room

Notes on `crates/game` — the Bims' simulation, the behaviour test room and the
room aboard a ship. The root `CLAUDE.md` is how to run and verify anything;
the probe notes are `scratchpad/CLAUDE.md`, and the app that draws the room
and its panels is `crates/app` (`screens/room.rs`, `crew.rs`).

> **Since the port to Bevy (September 2026):** the browser host is gone.
> Every name table this file points at in `web/*.js` is now
> `crates/app/src/names.rs`; the pages are the screens in
> `crates/app/src/screens/`; the `bims_*` and `ship_*` exports are methods on
> `bims::game::Game` and `ship::Session`. The node harnesses
> (`scratchpad/*.mjs`, `smoke.mjs`, `stub.mjs`) are gone with the host — a
> check this file says lives in one of them lives nowhere now unless it says
> so in `crates/app`'s tests or `./check smoke`, and is worth restoring as a
> unit test if the thing it guarded moves again.

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
the end. Nothing re-runs the pathfinder when the room changes shape, so anything
that puts the bathroom door across a walk already in progress leaves the Bim
pressed against the panels for ever — still *marching*, never arriving, with the
chain waiting on an `arrived()` that will not come. It converges on the door and
looks for all the world like a stuck animation.

`Game::tick_door_closer` therefore holds the door while
`Character::destination()` is on the far side of the bulkhead from the Bim, as
well as while the Bim is still in the opening. Anything else that shuts a door
by itself needs the same two guards.

## The cold store is two counts

`Room::veg` and `Room::tofu`, not one number. A stew is two vegetables, a bowl
is a block of tofu *and* a vegetable for the salad, so `can_cook(dish)` is the
question to ask and `has_ingredients()` only means "something can be made".
Anything that asks "is there enough" without naming a dish will let a chain
start that cannot finish.

## Adding furniture moves everything

The hydroponic bay added one rect to `Room::solids`, which changed the nav grid,
which changed every route length, which reshuffled the RNG stream — every roll
in `filth.rs` is drawn per frame, so a different number of frames before one
means a different outcome. Two long-run probes failed on that alone and neither
failure was in the code under test. When a day-in-the-life probe starts failing
after a layout change, check the trace before the logic: `mealtrace`/`messtrace`
in the scratchpad print every state change with a clock reading, which is how
the one that mattered — a Bim wetting itself in its sleep — was found.

## A route the body cannot hold to

`nav.rs` marks a cell blocked by its *centre*, so a straight line between two
free cells can pass within half a cell of an inflated obstacle and still test
clear. The Bim walks that line, the collision push-out shoves it back, and
where the push is exactly opposite the walk — a waypoint through the corner of
the table — the two cancel exactly: it marches on the spot for ever, `arrived()`
never comes true, and the chain waiting on it never finishes. It stood there
for ten game hours with a trip to the heads on its agenda, and nothing in the
game says anything is wrong: `is_stalled()` is false, it is not napping, it is
simply not moving.

`line_clear` now samples half a cell either side of the line as well as along
it. If a Bim is ever found frozen mid-errand, check its position against the
inflated furniture before looking anywhere else.

**It is rarer since, and which seeds hit it moves with the RNG — and what
is left is caught by `Game::unstick`, the watchdog described at the end of
this file.**
Sampling forty seeds for a week each: before dirt spreading, seeds 17, 18 and 29
froze a Bim mid-errand for 3152, 2651 and 1136 game minutes; after it, seed 8
for 189 and seed 25 for 49, and seed 101 — the seed `sweep.rs`, `crew.rs` and
`crowding.rs` all happen to pin — for about fifty hours, which is what those
three now fail on. The failure reads as starvation or as a filthy deck, because
that is what happens to a Bim standing still for two days; the cause is a Bim
`is_walking()` at a fixed position with a destination it never reaches.

So **any change that draws from the RNG at all re-rolls which seeds show it**,
and a probe that starts failing at one seed after an unrelated change is very
likely this and not the change. The trace to run is the one that prints
`activity`, `is_walking`, `bim_pos` and `destination_for_probe` for a Bim whose
errand has lasted an hour — a frozen position with `walking true` is the
signature, and nothing else aboard looks like it.

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
says "Something".

`Room::spot_rects(spot)` is the third question — *where is it*, for the
ring a panel row puts on the deck (`Game::set_highlight`, drawn in
`render`) — and it answers with **every fixture of the kind**: a row names
a kind, so the cook row rings both hobs of a ship with two galleys and the
cold store's row every cold store. The chairs and the beds are the
exception, the first standing for its adjacent pair, and the deck and the
bulkheads answer nothing rather than the whole room. A new fixture list on
the room wants an arm there too, or its row rings the first of them only —
which is what every kind used to do (`spot_rect`, kept as the first of
them for `scratchpad/probe.rs`).

## A need trigger is two things, and off is not zero

`needs::Trigger` is a level *and* a switch. Switching one off stops the errand
and nothing else: the need carries on draining, and everything going without
does to the Bim still bites. A probe that asserts a switched-off need "empties"
is wrong — a Bim past the rest trigger nods off where it stands, and that
claws back enough that the level settles just under the trigger rather than at
nothing. Assert it fell past the trigger and that `drowsiness()` is non-zero.

The drain rates in `needs.rs` are still derived from `URGENT`: each is the drop
from full to a tenth over the time that drop is meant to take. Moving a trigger
is the player's business and deliberately allowed, but it means that need's
slot no longer tiles the day the way the arithmetic says. Do not "fix" the
rates to follow the trigger — the derivation is the documentation.

## Two rules that have to move together

`Game::eats_first` and `Game::goes_first` hold a sleep back. Whatever *starts*
that sleep has to consult both, and whatever the sleep is waiting for has to be
started by something. When the rest trigger was added, `consider_errand` gained
both halves: `Need::Rest` refuses while either is true, and the block at the
bottom that starts the trip now fires for `wants_bed()` as well as for a queued
bed. Add a third reason to go to bed and it needs both halves too, or the two
rules deadlock and the Bim neither goes nor turns in.

## `suspend` must not bank what `Saved` is still carrying

`Task::let_go` takes the crop the Bim is carrying and puts it in the store,
because a chain given up for good must not evaporate a plant. `Task::suspend`
calls the same function — and a suspended chain is *kept*: the `Saved` it just
built still holds `lifted`, and the Bim will walk back to the fridge with it.
So `suspend` passes `None` and only the `blocked` path passes the crop, via
`take()` so nothing can bank it twice. Get this wrong and the bay quietly
doubles its output every time an errand is interrupted.

## The bay's progress bar stops short on a planting, deliberately

`Kind::Tend` is one chain with two endings: a planting finishes at the tray, a
harvest carries on to the cold store. `Kind::steps()` counts the whole thing,
so a planting's agenda row vanishes at about half a bar.

Do not "fix" this by weighing the chain against what the Bim is holding. It is
holding nothing until `WorkTray` has *finished*, so the bar would read 100% all
the way through the tray and then jump back to half when the carry began. A bar
that stops short beats one that goes backwards.

## There are two Bims, and `who` goes everywhere

`Game` holds `bims: Vec<Bim>` and almost every method that used to touch
`self.character` / `self.task` / `self.needs` now takes a `who: usize` first.
`crates/game/src/bim.rs` owns the per-crew state; the room, the clock, the deck's filth,
the timetable and the manager stay on `Game` because they are the ship's.

Three rules that keep it honest:

- **`bim::PLAYER` is the only one the player touches.** Selection, orders,
  recruiting and every fixture menu go through it, and the wasm exports for
  those take no index at all — the boundary itself says only James is steered.
  Anything that *reads* takes a `who` so the host can show both.
- **The timetable and the action thresholds are the ship's, not a Bim's.**
  `Schedule::due` re-arms itself the moment it is asked, so it is asked **once
  per frame** in `Game::update` and the answer is handed to each of the crew.
  Ask it inside the per-Bim loop and only the first one ever gets a night.
  `set_need_trigger` likewise writes to every Bim.
- **A seat belongs to a Bim, and a berth is given to one.** `Task` and
  `Saved` carry `who` for exactly this: `destination` needs it to pick a
  chair, and — through `Room::sleeps_in`, whose bed is whose — a bed, and
  a chain put down and resumed has to come back to the same ones. See
  "A bunk is one Bim's, and the deck is everybody's" below.

## A bunk is one Bim's, and the deck is everybody's

A Bim's index used to be its berth, clamped: a crew past the bunks all
slept in the last one, stacked. Now **`Room::sleeps_in[who]` says which
bunk is whose** — `Room::bed_of(who)`, `bed_owner(bed)` — and every
`Room::bed_*(bed)` accessor takes a *bed*, not a `who`; `task.rs` looks
the bed up first. The table lives on the room because a chain is handed
the room and nothing else, and it is the game's to keep: `with_room`
deals bunk `i` to Bim `i` as far as the bunks go (`Room::bunks()`, the
stand-in a layout with none gets not counted — `stand_in_bed`);
`take_crew` writes each Bim's entry onto `Bim::bed` for the journey and
`adopt` reads it back — kept when it names one of this room's own bunks
nobody here has, else the first spare, else none — so the ship's crew
keep their bunks across a dock and a hire takes what is spare; `die`
frees it; `Room::relayout` follows the bed by its *frame*, since a bunk
taken out of the design ahead of somebody's shifts the rest down. **The
player gives them**: `Game::assign_bed(who, Some(bed) | None)`,
`bed_assignable` (a real bunk, and not a docked station's — by the
`foreign` box `set_foreign` named, `bed_is_foreign`), and a bed that
changes hands stands up whoever is asleep in it first (`Task::abandon`
before the table moves, so `out_of_bed` makes the right blanket). The
app's bunk menu (`HIT_BED`, `Game::hit_bed()`) is a note saying whose,
**Assign to <the Bim shown>** or **Give up this bunk**, and Nap/Sleep on
the player's own bunk only.

**A bot takes a bunk going spare, and a bunk wears its owner's name**
(feature 61). `Game::settle_bunks`, at the top of every `simulate` before
the crew are ticked: while `free_bed()` finds one of the ship's bunks
nobody has, the first Bim alive past `players` with none gets it through
`assign_bed` — a hire past `adopt`'s spare, the `combat` command's
fourteen when one comes free, whoever was put on the deck when the
player moved a bunk. A player's own (`is_player`) is never given one this
way: giving a bunk up and taking one are the player's to do, so the
"Give up this bunk" row leaves a player on the deck and a bot on it a
step. `Game::bunk_tags` is the label's side: a `BunkTag` per assignable
bunk — the bed, its frame's centre in room units, the owner — which
`ship::world_paint::bunk_labels` turns through the ship's camera and the
app writes on the deck (`theme::bunk_tag`, `names::BED_TAG_UNASSIGNED`)
under the crew's names; a station's bunks on a joined deck wear none.
`a_bot_with_no_bunk_takes_one_that_is_going_spare_and_a_player_does_not`
pins the settling and the tags.

**A Bim with no bunk sleeps on the deck.** The same `Kind::Rest` chain:
`enter(GoToBed)` puts the nearest free cell under its feet on `target`
(and clears it for a Bim with a bunk, so `Task::sleeps_on_ground` reads
off it), `Doze` lies it there facing as it stood, `out_of_bed` stands it
where it lay. Three hours in six — `bim::GROUND_SLEEP` out of every
`GROUND_WINDOW`, the window opened by the first minute of a lie-down and
the allowance refilled when it closes, so an interrupted lie-down keeps
what it did not use — and **sore** for `SORE_LASTS` (12 h) after the last
minute of it, rest draining `needs::SORE_TIRING` (1.5×) faster,
multiplied into `tiring` beside malnutrition. All of it is in `tick_bim`
off `sleeps_on_ground() && restoring() == Some(Rest)`: `ground_left`
runs down and `task.wake()` cuts the doze at nothing. `Game::rest` caps
the minutes at the allowance and refuses under `GROUND_SLEEP_MIN`;
`queue_ready` holds a queued sleep for the window like one held for a
door, so the rest trigger and the timetable get a three-hour night when
the window allows and nothing when it does not — and past its trigger
the Bim nods off standing like any other. `job_code` calls anything
longer than the menu's nap a sleep, so the three hours read as one.
`a_bunk_is_one_bim_s_and_a_bim_with_none_sleeps_on_the_deck_three_hours_in_six`
and `a_bunk_goes_with_its_bim_from_one_room_to_the_next…` pin it all.
Adding a field to `Bim` or `Room` is a `SAVE_VERSION` bump (4).

## Borrowing one Bim and the room at once

`self.bims[who].task = Some(Task::rest(who, m, &mut self.bims[who].character,
&mut self.room, &self.maps))` does not compile, and neither does anything that
holds `&mut self.bims[who]` across a call to another `&mut self` method. Split
the fields locally first:

```rust
let (bims, room, maps) = (&mut self.bims, &mut self.room, &self.maps);
let bim = &mut bims[who];
```

Disjoint field borrows are fine *within one function body*. That is why the
per-Bim half of the frame is written as `&mut self` methods taking `who` and
re-indexing each time, rather than as one long-lived borrow.

## Every fixture is a list, and a chain picks the closest free one

`crates/game/src/galley.rs` is the galley as structs — `Worktop` (board,
drawer, knife, slices), `Hob` (heat, idle clock, the pot, the serving
plate, the bad-food judgement), `Fridge` (a door), `Locker` (a broom) —
and `Room` holds `worktops`, `hobs`, `fridges`, `dishwashers`, `lockers`,
`showers`, `bays`, and `bath` + `more_baths` (`Room::bath_at(i)`, nought
the room's own, which in the classic room has the walls and the door).
`Layout::more` (`room::More`) is every fixture past the first of its kind,
from `aboard::layout_of`; a toilet is paired with the basin nearest it.
What is *shared* stays on the room — the cold store's counts, the plate
count, `dish`: a cold store is a class of storage, not a box.

A chain **picks** (`task::Picks`, one `Option<usize>` a kind) the moment a
step first walks to a kind of fixture — `pick_for` in `Task::enter`, the
closest one nobody else has (`galley::closest_free`), with two extra
asks: leftovers want a hob with something in the pot, a broom wants a
locker with a broom in it — and keeps it for the rest of the errand and
across a suspend, since a half-cooked meal's pot is on its hob.
`destination` reads the picks; the standing steps index by them
(`Task::hob()` etc., nought before a pick). A cooking chain clears the
worktop and the hob's serving spot as it picks them (`reset_worktop`,
`reset_hob`) rather than at the start, since it does not know which yet.

What the *others* hold is `task::Taken`, gathered by `Game::taken_for(who)`
from every other Bim's errand **and queue**, and handed to every `Task`
constructor and `update`. Two places have to ask it, and forgetting the
second is the trap: `can_begin` (`task::can_pick_all` — one of
*everything* the kind walks to, `fixtures_used`, so a Bim does not set out
for a galley with no free hob and drop its slices there) covers errands
that *start*, and `queue_ready` (`Picks::clashes`) covers ones that
**resume**. `Exclusive` in `game.rs` keeps only what is not a fixture list:
a bench, the airlock, a site. Nothing free at all is `blocked`, the way a
shut door is. With one of everything this is exactly the old one-galley
rule — `REFERENCE_CHECKSUM` did not move — and with two it is two cooks:
`a_second_galley_is_cooked_in_at_the_same_time` in the world tests.

The menus act on the fixture under the click: `Game::hit_at` records
`hit_hob`, `hit_fridge`, `hit_dishwasher`, `hit_locker`, `hit_shower`,
`hit_bath` beside `hit_bay` and `hit_door`, `Switch::Hob(i)`,
`FridgeDoor(i)` and `Dishwasher(i)` carry the index, and every accessor
the app reads takes one; `galley_busy_by(who)` is "no galley free at all",
which is what greys **Make food** now, not one Bim cooking.

Stations aboard are per fixture (`Room::station_at(frame, x)`): a Bim
stands below the fixture it is working. The classic room keeps every
galley station on the worktop's line, whatever the fixture's depth,
because the probes are seeded on where the Bim stands.

`reset_worktop` deliberately does not clear the table: the other Bim may
well be sitting there eating what it cooked twenty minutes ago, and
wiping its plate would be the cook reaching across the room.

## Crew separation runs after both have moved

`Game::separate_crew` is its own pass at the end of the frame, not part of
either Bim's update. Run per-Bim it would give the one updated second the last
word and the pair would creep across the deck. It shoves each half the overlap,
and skips anyone seated, in bed or dead — they are where a chain put them —
pushing the other the whole way instead.

Knock-on worth remembering: **a recruited Bim drifts.** It never sets off
anywhere, but the other one walking into it moves it a body's width at a time.
A probe asserting "a recruited Bim stays put" by distance is asserting the
wrong thing; assert `is_walking()` is never true.

## The bed is a bed, and its footprint is still the bunk's

`Berth` in `crates/game/src/room.rs` draws a single bed now — headboard,
footboard, mattress, pillow, duvet — where it used to draw a bunk bed with a
ladder. The **footprint did not change**: the classic room's is still
94×172, the old upper deck plus the strip the lower one stuck out by, and
`station()` and `lie_pos()` are the bunk's numbers expressed off the frame.
That is deliberate. The footprint is what the nav grid is built on, and a
bed one pixel smaller is every route re-lengthened and every seed the probes
pin re-rolled — see "Adding furniture moves everything". Make the bed
smaller and expect `probe`, `crew`, `sweep`, `neglect` and `social` to move.

`social.rs` already fails at HEAD — seeds 1 and 7, "the bar hovers about its
trigger" — before and after the bed changed; it is not the bed.

## A per-Bim clock must not live on a shared object

`Filth` is the *deck*, and there is one deck. It used to also hold
`bursting_for`, `filthy_for` and `since_sick` — which are clocks on a **body**:
how long *this* Bim has been at the extreme urge, how long *it* has stood in
the mess. With one Bim that made no difference. With two it made every
difference: `Filth::update` ran once per crew member per frame on the same
object, so whichever Bim was comfortable zeroed the other's clock on the way
past. The hour was never reached, nobody ever had an accident, and nobody was
ever sick. The needs panel looked fine throughout — the *levels* were right,
and only the stages built on top of them were dead.

They are `filth::Ordeal` now, held by `Bim`. The rule the bug came from is
worth keeping: **when adding a second of something, every mutable clock has to
be asked whose it is.** A level that looks right is not evidence that the
machinery hanging off it does.

`scratchpad/neglect.rs` is the probe: it drives both crew to the far end of
every neglect and checks each reaches stage 3 and actually has the accident.

## Autonomy off does not stop a scheduled night

`set_autonomous(false)` stops a Bim *deciding* to sleep. The timetable still
queues one, because a scheduled night is a standing instruction from the player
rather than the Bim's own idea, and `pump_queue` is not gated on autonomy.

So a probe measuring "what going without sleep looks like" has to wipe the
schedule as well, or it measures a well-rested Bim and concludes the last stage
of drowsiness is unreachable. That cost a wrong diagnosis once already.

## Selecting is not commanding

Any of the crew can be selected; only `bim::PLAYER` takes orders. The two are
deliberately separate questions, and the split is what lets the right-hand side
show whichever Bim you clicked while `order_move` still refuses anybody but
James in peace (a crewmate under the alarm, see the fight). **A click
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
the live order goes through (`begin_ordered`: `cook`, `sweep_up`,
`send_to_switch`, `bandage`…), so every check the row makes is made
then, against the room as it is then, and what cannot be begun is
*dropped* rather than walked through into an empty cold store.
`queue_ready` asks what beginning asks — `can_begin` (one of everything
free) rather than `resume_station` — so a galley in use is waited for;
what could not begin with nobody else aboard (`can_pick_all` against an
empty `Taken`: leftovers with every pot empty) is let through to be
dropped, or it would sit on the agenda for a pot nobody is filling.

**A walk is `Kind::Walk { post }` with the spot on `Saved::target`**,
and it is never run as a chain: `pump_queue` hands it to `walk_order` —
`order_move_for`'s body, the selection check taken off — so the door is
opened on the way, the plain's windows are walked leg by leg and the
refusals are the right-click's; `post` is a crewmate's under the alarm
(`queue_squad` sets it for everybody but a player's own), so the Bim
holds the spot the way `order_one` posts it. `Step::GoToSpot` exists
only so the entry has a first step and `Kind::steps` a length.
`queue_walk` refuses at the click, with the cross, when there is no way
there even with the door open (`can_reach_through_door` of the snapped
stand — the deck is one piece or it is not, wherever the Bim will be by
then), and `walk_ready` holds a queued walk for a locked door like an
errand. `huddle` and `formation` are the spots a click and a drag deal
out, shared by the live order and the queued one; `queue_squad` is
`order_squad` for the queue.

**A plain order calls the queue off**: `drop_ordered(who)` throws away
the `ordered` entries and keeps the chains put down. `Game::order` calls
it for every errand variant (`CrewOrder::errand_for`) and
`order_move_for` for a walk — never `walk_order`, which is also how a
queued walk is begun, and never the bots' own `take_over`, so a Bim
deciding to eat does not lose what the player queued. A Shift on
anything that is not an errand — a selection, a Management box —
is the plain order.

**Nobody takes a Bim off a walk the player gave it.** A chain that is
interrupted goes onto the queue and is picked up again (`interrupt` →
`pump_queue`); a *walk* has no chain behind it, so an interruption does
not put anything anywhere — the route is simply gone, and the Bim ends
up wherever the errand started it with every leg still to come walked
from the wrong place. `on_a_given_walk(who)` is the question (a route in
hand and no task behind it: a right-click's walk, a Shift-click's when
its turn came, the walk back to a post), and the one errand that reaches
*across* a Bim's own agenda asks it: `free_to_talk`, a crewmate wanting
a word, which walks the other one to a meeting spot — and which leaves
alone a Bim with `ordered_count(who) > 0` still waiting as well, since
the next leg would then be walked from the talking spot. Everything a
Bim starts on *itself* already asks `character.arrived()`
(`consider_errand`, `flee_filth`, `return_to_post`), which is the same
rule said the other way round. The urgent-wound doctoring at the top of
`simulate_bim` is the deliberate exception and stays one: a Bim binding
its own wound as it runs from a fight has a route in hand and no chain
behind it too, and that is the one interruption the medical row is for.
`a_crewmate_s_word_does_not_take_a_shift_chain_off_a_bim` in
`order::tests` pins it, both halves: the chain is walked to its end, and
a Bim standing about with nothing of the player's on it is still
somebody to talk to.

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
place — a door's leaves starting to slide, a knife stroke on the board, a
shot leaving a gun, a bolt landing on a body or a bulkhead, a blow landing
— pushed onto `Room::cues` or `Combat::cues` the step it happens and
drained by the app through `Game::take_cues`. What each is played as is
`crates/app/src/sound.rs`'s; a server drops them.

- **A door is heard when its leaves change direction**, not when a target
  changes. `cue::door_motion` keeps a `moving` memory on the door (the
  ship's `Door` and the heads' `Bath` alike) and compares `open` before
  and after the step, so a door reversing part way is heard shutting, a
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
- `scratchpad/cues.rs` is the probe: a stew's ten strokes on the board, a
  day's heads' door opening and shutting turn and turn about, and nothing
  at all in a quiet first second.

## The diary keeps no words, and almost no entries

`memory.rs` stores `(day, minutes, code, one number)` and nothing else;
`MEMORY_LINES` in `crates/app/src/names.rs` is where every sentence lives.

**It only records what went wrong.** The day's work is not written down at all
— it used to be, and the page filled with "Went to the heads." three times a
day. So a quiet week leaves the diary *empty*, and `scratchpad/diary.rs`
asserts exactly that: a clean four days must produce nought entries. If you
find yourself adding a `What` for something that goes right, that is the rule
saying no.

An entry with no line in `MEMORY_LINES` is **dropped from the page** rather
than rendered as a placeholder. There used to be a "Something happened."
fallback and a harness check for that string; both are gone. What catches a
missing line now is `smoke.mjs`'s `lines.length === bims_memory_len(0)` — a
code with no words is a row that never appears, so the count comes up short.

Two knock-ons worth knowing, both of which bit when this changed:

- **A Bim had nothing to say.** Conversation topics were drawn from the
  speaker's own diary, so trimming the diary left both crew talking about
  "nothing much" for ever. `Bim::lately` now holds the last few finished
  errands as `job_code`s purely for small talk. `Game::chat_topic` therefore
  returns **two code spaces** — `JOB_` codes from 1, `What` codes from 20 —
  and `CHAT_TOPICS` is indexed by both. They do not overlap; keep it that way.
- **`Task::stowed` and `Task::swept()` existed only for the diary** and were
  dead the moment it stopped recording errands. Both are gone. If a diary
  entry ever wants that detail back, it has to be carried again.

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
recruited, so a recruited James standing in the classic room is never
moved by a Kate walking past — `scratchpad/crew.rs` relies on that. The
other half of the same picture (a squad stacked on one tile) is
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
with it and read that history first.

A seated Bim crowds nobody: `crowding` skips anyone `is_seated()`, because a
body tucked into a bunk or a chair is not in the gangway.

## `Filth` lives on `Room` now

The deck used to hang off `Game`. It moved because `task.rs` needs it: the
sweeping chain has to ask where the dirt is, and a chain is handed the room and
nothing else. `Game::render` still draws it separately — `self.room.filth.draw`
— because the layering matters and only `Game` knows the order.

## Sweeping: two corners that will bite again

- **Sweep the tile the Bim set out for, not the one under its boots.** They
  differ whenever the dirt is somewhere a body cannot quite stand, and a broom
  has the reach. Sweeping underfoot leaves those tiles filthy for ever *and*
  loops, because the unswept tile is still the worst on the deck next time.
- **`worst_tile` takes a reachability predicate, and it is not optional.**
  Parts of the deck read as `SPOT_DECK` and have no route to them — the corner
  past the end of the counter, hemmed in by the second bunk. Without the
  predicate a mess there is chosen as the worst tile for ever: fetch broom,
  fail the walk, give up, start again, three tiles left on the deck after four
  simulated days. `Filth` has no idea where a body fits, so the question goes
  to `next_dirty` in `task.rs`, which has the nav grid.

`Task::enter` and `Task::next_step` must ask that question *the same way* —
both go through `next_dirty`. If `next_step` says "more to sweep" and `enter`
then finds nowhere to go, `CarryBroomTo` is a step of no length and the chain
spins between it and `Sweep` for ever.

## Anything held has to survive the chain being given up

`Task::let_go` puts the broom back the same way it banks a carried harvest. A
Clean chain abandoned mid-sweep would otherwise leave the Bim holding the broom
for good — and since the locker door is drawn from whose hands it is in, the
cupboard would stand empty and nobody could ever take a broom that was never
returned. Add a held item and this is the second place to touch.

## A new `What` needs four edits, not one

`memory.rs` for the code, `MEMORY_LINES` in `crates/app/src/names.rs` for the sentence,
`CHAT_TOPICS` beside it for the short form a Bim says out loud, and the range
in `scratchpad/diary.rs` that checks every entry is nameable.

Miss the sentence and the entry is silently **dropped from the page** — no
placeholder any more — which `smoke.mjs` catches only through its row count.
Miss the range and the probe fails on a perfectly good entry, which is what
`Swept = 9` did.

And before adding one at all: the diary keeps only what went wrong. A `What`
for something that went right does not belong in it.

## Adding a job to the work list needs four edits

`work.rs` for the variant, **appended** — the code is the row's identity;
`WORK_NAMES` in `crates/app/src/names.rs` for the word and `WORK_SPOTS` in
`crates/app/src/crew.rs` for the fixture the row rings (`SPOT_NOTHING` for
a job with no fixed place, like building and doctoring), both pinned
against `Job::ALL` by length tests; and the range in
`scratchpad/priority.rs` that checks every code names a job. Same shape as
`memory.rs`'s `What` and for the same reason — no strings cross the
boundary, so the ship knows `Job::Clean` and only the host knows
"Cleaning". Miss the name and the row comes up blank, and a blank row is a
row the player cannot use that nothing else would notice.

The host builds its rows off `work_count()` rather than off the length of
its own name table, so the two disagreeing shows up as a blank row rather
than as a job silently missing from the panel. Keep it that way.

Then `work_on_offer` says when the row is on offer and `do_some_work` what
taking it starts. `Job::Medical` (9, the last) is the one row that is also
an **interruption**: `medical_on_offer(who)` — a bandage to hand, the Bim
itself while it bleeds, else the crewmate with the most open wounds, and
that one's worst part — is asked in `tick_bim` as well, before the errand
is stepped, and at `HIGHEST` it `bandage`s on the spot, which puts the
errand on the queue the way a need does; at any other number it waits its
turn like the rest, and it is offered **first** among equals, code
notwithstanding, so an untouched list dresses a wound before it sweeps the
blood up. Two things it asks that the menu's `bandage` does not, because
the menu is a player who can see and this runs every step: a patient the
helper has no route to is not offered (`task::patient_stand`, the same
question the walk asks — a chain that starts, finds no route and is given
up would be offered again next step, for ever), and a part somebody else
is already walking over to dress is left to them.
`medical_at_the_top_drops_the_sweep_to_dress_its_own_wound` and
`a_crewmate_bleeding_is_dressed_by_whoever_is_free` in `game::tests` pin
it, and `scratchpad/priority.rs` has a section.

**`Job::Craft` is the one row a bot never takes** (feature 89).
`craft_on_offer` asks `Game::is_bot(who)` — `hostile_bodies || !is_player`,
the same question `medical_on_offer` asks — and an **open** order (one
with no `only`) is offered to a player's own Bim alone, whatever the
Craft row is set to. Standing at a bench is the player's work: the
smelter, the workbench, the armoury and the drug lab all go the same
way, since the room knows a bench only by `Bench::kind` and the rule is
about who is standing there rather than about which bench it is. An
order *named* for one Bim (`Order::only`, an engineer's armour repair at
the workbench) is still that Bim's, bot or not — the world keeps such an
order posted until it is finished, so a bot refusing its own would leave
the bench held for ever. Everything else a bot did it still does: it
plants, cuts, sweeps, cooks, hauls, ferries, mines, builds, mans the
helm, doctors and shoots. `a_bot_never_stands_at_a_bench_and_a_player_s_bim_does`
in `game::tests` pins both halves.

An **activity code** (`JOB_*` in `game.rs`, what `activity()` and the
agenda say a Bim is doing) is the same shape one table over: appended after
the last, and named in `job_name` and `activity_line` in `names.rs`, both
`match`es with a `Busy`/`None` fallback — so a code left out is a Bim
reading "Busy", which nothing but a test notices.
`the_bandage_job_has_a_name_and_a_line` pins `JOB_BANDAGE` (22, the
last) against both; a new one wants the same line added to that test.

## Health mends every frame, so a sudden nothing is not nothing

`Health::update` runs before `Game` looks at whether the Bim is dead, and a fed
Bim's health climbs. Anything that takes the bar to zero *later* in the frame —
a Bim hurting itself, a Bim giving up — was therefore back above zero by the
time the check came round, and the death simply never happened: the run ended
with a Bim reading nought health and still walking about. `update` now returns
immediately when `is_dead()`. Anything new that damages health in one go
depends on that, so do not "tidy" it away.

**The bar is three parts and the blood is a fourth number**
(`crates/game/src/health.rs`). `Part::Head`, `Body`, `Legs` — codes 0–2,
the armour slots' order — with `Part::max` 5, 75 and 20, and `points()` is
the three added up, so the panel's bar and the starvation arithmetic are
what they were: hunger drains and mending regrows all three **in
proportion** to their size. `is_dead` is the head or the body at nothing
*or* the blood at nothing; `hurt` comes off the body and `give_up` zeroes
all three. A shot (`Health::shot(part, damage)`) is rolled onto a part by
`Part::hit_by(unit)` off `HIT_ODDS` (one in twenty, three in four, one in
five), takes the damage off that part and opens a **wound** there; the
legs at nothing is a leg lost and the leg health started again, twice is
none left and `NO_LEGS_PACE`. Every open wound bleeds `BLEED_PER_WOUND`
(10) of `MAX_BLOOD` (100) an hour until `bandage(part)` closes every wound
on that part; under `SLOWED_AT` (three quarters) the Bim walks at half pace, under
`OUT_AT` (**half**, feature 89) it is `unconscious()`, at nothing it is dead, and the blood
comes back over two days once nothing is open. A head shot at pistol
damage kills — not at the hit but at the top of the body's next tick, with
`points()` still in the sixties, which is why the world reads a death off
`is_dead`/`points() <= 0` per body and not off the hit (see
`crates/world/CLAUDE.md`). `the_parts_add_to_a_hundred_and_the_odds_to_one`
and the tests beside it in `health.rs` pin the arithmetic; the fight that
inflicts it is "The fight" below.

## Two Bims standing together is a layout problem

Names are painted a body's height above each head. A pair who meet walking
north–south end up one above the other, and the lower one's name lands on the
upper one's body — `./layout talk` shows it immediately and nothing else does.
`Game::chat` therefore always stands them **left and right**, `TALKING_GAP`
apart, and that gap is set by the *names* rather than by the bodies: a body is
`BODY_MARGIN` across the radius, but two labels need a good deal more.

## Aboard, the nav grid is phased to the tiles, and that is what makes a one-tile corridor walkable

`nav.rs` inflates every obstacle by a `BODY_MARGIN` of 23 over a 52-unit
tile, so a one-tile gap leaves a six-unit strip down its middle — narrower
than a cell. Whether any cell centre lands in it used to be luck: the
classic grid (`Nav::new`, 10-unit cells) starts from the walkable area's
edge, and the phase against the tiles drifted two units a tile, so one
corridor walked and the next did not. Aboard the room now builds
`Nav::tiled` — `CELLS_PER_TILE = 5`, origin half a cell in from the
interior's tile-aligned corner — so every tile's middle is a cell's middle
and the strip always holds one. `Room::nav_tile()` says which grid a room
gets (`None` for the classic room, which keeps its grid and its probe
seeds byte for byte; `Some(TILE)` aboard), and `Maps::new` takes it.
`a_one_tile_corridor_can_be_walked` in the world tests pins a straight gap
and an L, and fails on the old grid. That moved `REFERENCE_CHECKSUM`.

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
number). The bathroom door is worked by hand and is a
solid when shut, with a grid per state in `Maps`; a ship has a door in
every bulkhead and a grid per combination is not a thing, so **an unlocked
door is never a solid** — the pathfinder plans through it open or shut, and
the leaves are open by the time the body arrives. `Room::doors` holds them
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

The player's four words are the bathroom door's — `door::Order` Open (hold),
Close (let go), Lock, Unlock — and each is an errand through
`Switch::Door(index, order)` that walks James to the panel
(`Door::station`, a `STAND_OFF` out of the opening on his side). The host
asks `bims_hit_door()` after `Game::hit_at` said `HIT_SHIP_DOOR = 9`; the
readout's `SPOT_SHIP_DOOR = 17` carries its state through
`bims_ship_door_at(x, y)`. `a_locked_door_is_a_wall_and_an_unlocked_one_is_not`
in `crates/world/src/tests.rs` pins the routing half and
`simulation-check.mjs`'s door section the menu half. Two things that bit:

- **The camera follows James.** A pixel a harness found a door at is
  stale once he has walked to its panel — `findDoor` in the section looks
  again, by asking the room what is under each tile, not by reusing pixels.
- **A station's room draws no doors while docked**
  (`Game::set_doors_drawn(false)` in `World::join_rooms`). The joined deck
  has the same doors and draws them, and two pictures of one door would be
  a door in two states; the residents, who walk about in the station's
  room, are put on the joined deck as *visitors* (`Game::set_visitors`,
  from `Aboard::visit` every step) so its doors open for them too. Visitors
  are bodies for the doors and nothing else — not crew, not solid, not
  selectable.

## The bay is six tiles, and which side it is worked from is data

`PartKind::HydroBay` is `(6, 1)` with six use spots along its north side,
one a tray; the room's `Bay` already had `SPOTS = 6` trays, so each tile is
one of them. `Bay::at(frame, side)` lays the trays along the long axis and
stands the Bim on `side` — `Layout::bay_side`, which `aboard.rs` reads off
the part's first use spot — so a bay turned to `R90` or `R270` is six trays
down and worked from the east or the west, rather than "used from the wrong
side" like the rest. The pictures still grow plants upwards in every tray.

**The side is which edge of the frame the spot lies beyond, never which
way it is off the centre.** The first spot is at the *end* of the run, and
measured from the middle of six tiles it reads as off the end — which laid
the trays *across* the bay, six strips one tile long, with the Bim working
them from the west. `the_bay_aboard_is_a_tray_a_tile_worked_from_the_spots_side`
in `crates/world/src/tests.rs` pins a tray a tile for a bay lying and one
standing, through `Bay::station`. Fixing it moved where the Bim stands to
tend, which re-rolled the solo session in `ship-check.mjs` enough that the
crew member was off the helm at the redirect; the harness now takes the
helm again before it, the page's way.

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

## Stew for the store is a third count, a third target, and the cook row's

`Room::stew` is pots of stew on the shelf, beside `veg` and `tofu`, and
`manager::Stock` is the targets — `Veg`, `Tofu`, `Stew`, codes 0–2, and
since the bandage `Fibre`, 3 (see "Fibre is a crop" below),
through `bims_target(which)`/`bims_set_target(which, n)`. The food-units
dial and its 2:1 split are gone: `Game::target`, `Game::target_veg` and
friends no longer exist, and the management tab has three inputs
(`#veg-target`, `#tofu-target`, `#stew-target`, in `#keeps`) on both pages.
**The stew target starts at nought** on purpose: a default above it would
have the crew cook the store down from the first morning and re-roll every
probe that pins where they stand.

Two chains in `task.rs` hang off it, both sharing the meal chain's steps
where they can:

- **`Kind::Batch`** is the cook row's stew errand and the hob menu's "Cook a
  stew for the store" (`Game::make_stew`, not `Game::cook`, which is
  the table stew). One vegetable, then one block of tofu, each through
  the fridge–board–knife loop — `laps(kind)` is what says twice, and
  `TakeVegetable` picks the crop off `chopped` — then the pot, and instead
  of a plate `PackStew` empties the pot into a tub (`Held::Stew`) and
  `CarryStewToStore … StowStew` puts it away. The store's door is
  `OpenStoreForStew`/`ShutStoreOnStew`, **not** `OpenFridge`/`CloseFridge`:
  the chain has already been through those on the way to the board, and a
  step that appears twice in one chain is one `rewind` and `progress_of`
  cannot tell apart.
- **`Kind::Reheat`** is what a hungry Bim does when `room.stew > 0`:
  `make_food` tries leftovers, then the shelf, then the knife. `TakeStew`
  takes the count down as the tub leaves the shelf, `TipStewIntoPot`
  fills the pot most of the way to cooked, and from `TurnStoveOn` on it is
  the meal chain.

A tub in the hands is banked like a harvest — `let_go(.., for_good, ..)`
puts it back on the shelf only when the chain is given up for good, since
a suspended chain keeps it on `Saved.main`, and `take_crew` asks
`Saved::holds_stew()`. **There is no stew row on the work list**: stew for
the store is `Job::Cook`'s, beside a meal — `work_on_offer` offers Cook
while the Bim is hungry *or* `wants_stew()` (short of the target **and**
`can_make_stew()`, so a target with nothing to make it of is not a job that
comes round every frame), and `do_some_work` has the hungry one eat first
and only a Bim that is not cook for the shelf. That means the row being on
offer says nothing about which half wants it: `scratchpad/stew.rs` reads
`wants_stew_for_probe()` for the shelf's half, because a Bim that happens
to be hungry when the probe looks would otherwise read as the shelf
asking. It is the probe: target → shelf asks → stew on the shelf → stops
at the target → a hungry Bim warms one up, measured from the moment the
warming begins, since the Bim may be halfway through a pot *for* the shelf
when hunger bites and finishes that first. Job codes 15 and 16
(`JOB_STEW`, `JOB_REHEAT`) are in `JOB_NAMES` and `ACTIVITY`; the ship's
items panel lists it under `MADE_ABOARD` with its own `data-made="stew"`
icon rule, because it is not a `ResourceId` and the manifest has never
heard of it.

The management tab's three targets are a **Target column** of the `#stock`
table, one input in the row of the thing it is a target for
(`targetInput` in `crates/app/src/crew.rs`; the ids `veg-target`/`tofu-target`/
`stew-target` and the cells `#veg-control`… survive, since the harness
types into them), and the one explanation is a `?` on the column heading.
The `#keeps` row is gone. The table has three rows — vegetables, tofu and
stew, all in the cold store; the pot on the hob has no row, a meal in the
making being the agenda's business. A target cell rings the bay or the
hob while its row rings the cold store, and `pointerenter`/`leave` do not
bubble, so `points(el, spot, back)` takes a third argument: what to ring
when the pointer leaves — the row's spot for a cell inside a row, nothing
otherwise. Without it, leaving the cell for the row put the ring out while
the row was still under the pointer, and `smoke.mjs` says so. The table
has a **fourth row now, fibre** (`Stock::Fibre`, `Game::store_fibre`), in
the cold store like the food; the `?` on the Target heading is about food,
so the fibre row's own name is the underlined word (`FIBRE_TIP`) instead.

Never pipe `rustc` into `head`: `head` closing the pipe kills the compiler
with SIGPIPE before it writes the binary, and the "no such file" that
follows looks like a compile error that is not there.

## The helm is a job, and the room only knows it through the world

`Job::Helm` — "Controlling the ship", the sixth row — is on offer while
`Game::helm` is `Some` and nobody is *posted* within `HELM_SLACK` of the
seat, by the job or by the player's own walk to Confirm alike. The room has
no idea where the helm is or whether the ship wants anybody at it: the
world says, **every step**, through `Aboard::set_helm` → `Game::set_helm`
in stage 5 — the seat while the ship is anywhere but `Docked` or
`CastingOff` (the crew are being walked home then), `None` otherwise. The
classic room never gets one and never offers the row. Whoever takes the
job is `send_to` the seat — a post, exactly like the button — and recorded
as `helmsman`, so that `set_helm(None)` at the berth lifts *that* post and
no other; a helmsman ordered elsewhere loses the post and the record with
it, and the job comes round for whoever is free. `SPOT_HELM = 18` exists
only so the row can ring the helm's footprint (`Layout::helm`, off the
first `Helm` part); `Room::spot` never returns it, the ship's readout names
the part itself. `under_way_the_helm_is_a_job_and_somebody_takes_it` in
`crates/world/src/tests.rs` pins it — and note it has to `select_group(1)`
before `order_move`, which refuses an unselected James.

## Washing is its own need, and it is not Surroundings

`Need::Hygiene` — "Washing" on the panel, index 5, with a trigger row of
its own in `TRIGGER_NEEDS` — drains on the waking day like food and comes
round about once a day (`SHOWERS_PER_DAY`, `SHOWER_COST`), and the errand
is `Kind::Shower`: `GoToShower`, then `Shower` for `SHOWER_MINUTES` under
`Exclusive::Shower`, restoring the need and `ch.wash(1.0)` — the whole of
the Bim's own filth off, which the basin never managed. `JOB_SHOWER = 17`.

It is deliberately **not** a clock on `Need::Surroundings`. That one is the
*deck* — it follows the mess round the Bim — and the stages of being sick
in `filth::Ordeal` hang off it reaching nothing, so a time drain on it
would have a crew with nowhere to wash falling ill on a spotless deck.
The classic room has no shower (`Room::shower` is `None`;
`Layout::shower` comes off the first `Shower` part's use spot aboard), and
there a Bim simply goes on wanting one: `take_shower` refuses, and nothing
else comes of it. `first_station` returns `None` for a chain with nowhere
to go and `can_begin` reads that as "starts on the spot", so **`take_shower`
asks `room.shower.is_some()` itself** — any other errand whose station is
optional needs the same guard.
`a_bim_aboard_takes_a_shower_when_a_day_s_grime_has_caught_up_with_it` in
the world tests empties the need by hand rather than waiting a day for it.

**An accident empties it.** `Needs::soiled(on_bim)` in `mind_the_mess`,
beside `Character::soil`: the washing need drops to whatever share of
the Bim the mess left clean — nought for a fouling, just over half for a
wetting — and never rises for it, so a Bim that has soiled itself goes
for the shower next rather than at the end of the day, and comes out
with its own filth off. The deck under it is still the broom's.
`an_accident_empties_the_washing_need_along_with_covering_the_bim` in
`game::tests` and `a_bim_that_soils_itself_goes_for_a_shower` in the
world's pin it (a poisoned Bim at the extreme urge goes at once, which is
how both force the accident).

## Surroundings is the deck round the Bim, and a comfort lifts it

`Need::Surroundings` (index 3, "Surroundings" on the panel; it was
`Cleanliness` until September 2026) has no clock: `Filth::around(at)` is
the average of the tiles within `REACH` of the Bim **plus the lift of
the tile it stands on**, and `Filth::grinding` runs the need off that
and the Bim's own filth as before. The lift is `Filth::set_comforts`, a
`Vec<f32>` a tile beside the scores: every `filth::Comfort { at, lift,
tiles }` in the layout adds its lift to the square block round it,
summed where two overlap and capped at `LIFT_CAP` (a clean tile's 10),
laid afresh from the whole list on `Room::from_layout` and every
`relayout` — a plant built is a lift gained, one taken down a lift gone
— and carried across a `resized` deck by position like the stains. It is
a layer *over* the mess, not a change to it: no broom sweeps it, no
spatter dents it, `at`, `depth_at`, `dirty_share` and `worst_tile` never
see it, and a fouled tile under a plant is still fouled — only the
average the need reads is higher there. `aboard::layout_of` makes the
list off `shipdesign::comfort` (the small plant, the big plant, the
picture; `crates/shipdesign/CLAUDE.md`), at each part's centre; the
classic room has none. `somewhere_cleaner` reads `around`, so a Bim
fleeing a mess drifts towards the plants.
`a_plant_lifts_the_surroundings_and_the_mess_stays_underneath` in
`game::tests` pins the reach, the cap and the mess underneath.

## A meal is judged by the mess round the hob, not by the stains it makes

`Game::judge_the_food` runs the frame the pot finishes cooking
(`Room::update` flags it) or a bowl is filled (`FillBowl` calls
`made_a_bowl`): every tile within `GALLEY_REACH` (two) of the hob at or
below `filth::SPOILS_FOOD` is one roll at `BAD_FOOD_PER_DIRTY_TILE` (a
fifth), one bad roll is `Room::food_bad`, and a Bim `restoring`
`Need::Food` off a bad galley is `poison`ed — `Bim::poisoned_for`, two
days, the restroom drain trebled through `Needs::update`'s `purging` and
the hour of holding on skipped in `Ordeal::update`, so the need reaching
nothing *is* the accident. `What::FoodPoisoning = 31`, `Game::poisoning`
is the hours left for the health panel's `poisoning` line. A bowl is
judged because it is made in the same galley, and a reheated stew is
judged again because it is the hob it is warmed on — the shelf keeps no
record of a bad tub. `scratchpad/poison.rs` is the probe.

**`SPOILS_FOOD` is the wetting line, not `WORTH_SWEEPING`**, and that is
the trap: the cook's own chopping flicks a stain or two onto the deck on
the way (`JOB_MESSES`, `GRIME_COST`), a single stain is already past
"worth sweeping", and judged by that the galley poisoned the cook every
meal on a deck nobody had touched — `diary.rs`'s quiet four days came
back a page of accidents and `crew.rs` lost Kate. A clean galley draws
nothing from the RNG, so a clean run is unchanged; a fouled one re-rolls
every seed from the first meal.

## Aboard, a fixture is drawn inside its tile

The room's pictures were drawn at the classic room's offsets, and on a
ship's one-tile part they ran out of the tile: the pot overhung its
neighbours, the fridge's shelves ran out through its side, the chair was
wider than its tile and the dishwasher was only its door. The rule now is
that everything the room draws for a part is laid out **off the part's
rect**, not at fixed offsets: `Room::hob_scale` fits the burner inside a
one-tile hob and the pot is `POT_SIZE` of that; the fridge's shelves and
stock are placed in fractions of its interior; `CHAIR_SIZE` is 48×42 with
the backrest a unit inside the tile; and `Dishwasher::body` is the whole
tile aboard (`None` in the classic room, where the counter is the body).
`ship-layout.mjs simulation` rendered to PNG is how to check — there is no
assertion that can see a picture spill.

The shower has a menu, like the pan: `HIT_SHOWER = 10` → "Take a shower"
(`Game::can_shower`, `Game::take_shower`, `Game::shower_held_by`), and
`SPOT_SHOWER = 21` names it in the readout and rings it. The simulation
harness has a section for it. Codes 19 and 20 are the bench and the suit
locker, added beside these; `SPOT_NAMES` in `crates/app/src/names.rs` is indexed by code,
so every one of them needs its entry there in order, whoever adds it.

## The hob is one burner, left of centre

`Room::burner` is where the pot stands and the only ring that lights. It
sits at `centre - 30`, where the left of the old pair was, because the
cooking station is measured off `pot_pos` and centring it would move where
the Bim stands and re-roll every probe seed for a picture.

## The chopping board has two sides, and the tofu's is the left

`Room::board_sides` is two `room::Cut`s — what was put down, how much is
still whole, how many pieces it is in — the left one first. `put_on_board`
puts tofu on the left and a vegetable on the right, and the second thing
chopped for one meal takes whichever is free, so a shelf stew (vegetable,
then tofu) and a table stew (two vegetables) both end with both sides full
and everything goes into the pot together: `GatherSlices` takes
`board_pieces()` into `Held::Chopped { rounds, cubes }`, which is drawn as
a handful of both, and `clear_board`. `chop(done, of)` works on the side
the last `put_on_board` named. `Held::Fork` is the fork; it used to be
`Held::Slices` standing in for one. `target/probes/layout board` is the
picture, caught mid-stew with the tofu half cut.

## A berth has an axis

`Berth::lying` is a bunk turned a quarter — a frame wider than tall — and
everything about the bed goes through `Berth::at(across, along)`,
`size(across, along)`, `breadth()` and `length()`: the picture, the pillow,
where the Bim lies (`lie_pos`), which way it faces lying there
(`lie_facing`, which `task.rs` reads instead of the old `FACE_PILLOW`), and
where it stands to get in. For a bed standing up the arithmetic is the
classic room's to the number, so no probe seed moved; a bed lying down is
the same bed on its side rather than a blanket drawn off the end of it.

## A frozen walk is caught, and the two ways it happened

`Game::unstick` is the watchdog: a Bim marching with a destination that
has moved less than `STUCK_STEP` a second for `STUCK_AFTER` is stopped
dead, and nothing else aboard looks like that. It plans a fresh route to
the same destination from where the body is; if there is none, it clears
the walk (`follow_path(Vec::new())`, so `arrived()` comes true and the Bim
can start something else) and `interrupt`s the chain onto the queue,
where `queue_ready` holds it until the way is open again.

The two cases the social probe found, seeds 7 and 1 — the ones the notes
above said "are left":

- **The turn is an arc.** A body turns at `TURN_RATE` while moving at
  `MARCH_SPEED`, a radius of some seventeen units, so a walk that starts
  facing the wrong way leaves the line it was given by up to a body's
  width. Off the line against the inflated face of the table with the
  next waypoint straight through it, the push-out undid every step
  exactly: Kate stood at (315, 320) for days. The replan takes her round.
- **The heads' door shut across a walk into it.** `ShutDoorBehind` on the
  other Bim's chain is by hand, and `tick_door_closer`'s crossing check
  only holds the *closer*. James, sent by the sweep to a tile inside while
  the door stood open, pressed against the panels all night while Kate
  slept — and with the walk never arriving, `consider_errand` refused him
  a bed. Now the chain is put down and he goes to bed; the sweep resumes
  from the queue when the door is open again, re-picking its tile.

Both were the company bar collapsing to nothing in `social.rs`, not
anything about company: the drain runs while the other one is asleep or
walled in, and a Bim that cannot move cannot be talked to. A need bar that
empties on one seed is a frozen Bim until proved otherwise.

## Outside, the body is on a second grid, and a rock is mined straight on

A walk outside (`Kind::Eva`) is a walk now, not a clock: `StepOut`'s
`leave` stands the body a tile beyond the collar (`Character::go_outside`
— standing, no longer seated, with the suit on and `Held::Pick` in the
tool hand, both off again in `come_inside`), and from there `PickRock` →
`WalkToRock` → `Mine` go round until `next_rock` finds nothing, then
`WalkToPort` → `StepIn`. `Kind::steps()` walks that loop **once**
(`(Kind::Eva, Mine) => WalkToPort`), or `rewind` and `progress_of` never
return; `next_step` is what goes round.

The grid it walks is `Maps::outside`, `Nav::outside`: one cell a **tile**
(not five), `OUTSIDE_RADIUS` (100) tiles every way about the body, over
`Room::hull` (every structure tile, from `aboard.rs`) and `Room::rocks`
(what the world handed over in `Game::set_eva`). `Game::refresh_outside`
at the top of every `simulate` rebuilds it when `Room::rocks_version`
moves or the body is `OUTSIDE_RECENTRE` tiles off its middle, centred on
whoever is out or on `Room::outside` when nobody is, and drops it when
there is no outside and nobody in it. Everything that plans or moves a
body asks which grid by `Character::is_outside()`: `Task::enter` and
`unstick` through `Maps::for_body`, and `Game::move_body` — the **one**
call that moves a Bim now, replacing four `character.update` sites —
hands an outside body the grid's span and `outside_blockers` instead of
the deck and the furniture, so nothing shoves it back through the hull.
`crowding` skips a body outside; `order_move` refuses one (the walk
brings it in).

Two things that bit, in one afternoon:

- **`PickRock` is a step of no length, and it is not decoration.**
  `Mine`'s `leave` writes the rock on `Room::mined`; the world takes the
  tile out *after* the room's step and hands the rocks back on the next
  one, and the grid is rebuilt at the top of that step. Without a step
  between, `next_rock` ran on the old grid with the mined tile still
  solid, and the rock behind it — whose stand spot *is* the mined tile —
  read as unreachable, so a dig stopped after every tile. `Mine`'s
  `leave` also takes the rock off the room's own `rock_targets` and
  `rocks`, or the same rock is picked again before the world has spoken.
- **A rock is mined from the tile beside it, four ways, never a corner.**
  `next_rock` tried eight neighbours first. A rock reachable only
  diagonally was mined from its corner, and the pocket it left was a
  tile the grid rightly refuses to squeeze into diagonally between two
  rocks — so the rock behind it was marked for nothing and the walk went
  home. Straight on, the tile mined is always the tile stood in next.
  `Game::rocks_reachable` (from the port, once per rebuild, since a route
  to every mark is a search of the whole grid when there is none) is
  what keeps `Job::Mine` from being offered to marks nobody can reach,
  and what the Actions tab says are "out of reach".

A walk put down out there — hunger, an order — restarts its outside half
from the gangway: `rewind` sends the `outside_half` steps back to
`GoToGangway` and `resume` drops nothing back in, so it goes out again and
picks its rock afresh rather than dropping into `Mine` at a rock it is no
longer beside. `walks_done` is counted at `StepIn`'s **enter**, the same
step the world reads it. `JOB_EVA` is still 19; "Mining outside".

## Plates are counted, and the count is the drawer

`Room::plates` is the clean plates in the chopping board's drawer —
`START_PLATES` (20) in every room, `PLATE_DRAWER` (40) at most; nothing on
a manifest carries plates, so a designed ship starts with the same twenty
as the playtest one. `TakePlateAndSpoon` and `TakeBowl` take one
(`Room::take_plate`, saturating — an empty drawer serves an imaginary
plate rather than starving anybody), the rack gives them back when a
dishwasher cycle finishes (`Dishwasher::update` now *returns* the washed
count and `Room::update` puts it in the drawer), and the two places a
plate used to vanish put it back instead: `let_go(.., for_good, ..)` with
one in hand, and `reset_for_cooking` with one left on the counter. A plate
on the table stays out until `ClearTable`. The open drawer draws a pip per
plate up to what its face has room for; `Game::plates` and
`plate_drawer_capacity` are the readout (the Aboard panel's "Drawer" row,
and the board's spot state).

## A site is worked from the deck or from outside, and the room is relaid under the crew

`Kind::Haul { site, outside }` and `Kind::Build { site, outside }` are
the construction chains — `GoToShelf → TakeMaterials → CarryToSite →
DropMaterials`, and `GoToSite → Construct` — with, for a site beyond the
hull, the walk outside's own steps either side (`GoToSuitLocker` … `StepOut`
before, `WalkToPort` … `PutSuitBack` after). `Kind::fork` is where every
chain's turn-offs live now, shared by `Kind::steps` and `Task::next_step`
so the agenda and the chain cannot disagree; `outside_half(kind, step)`
is the generalised Eva rule for `rewind` and `resume`. The world says
what there is to build (`Game::set_build_orders`, `Room::builds`, one
`Build` a site with its tiles in room units, `haul` and `minutes`) and
who may suit up (`Room::suit_ok`); the room reports on `Room::picked`,
`dropped`, `returned` and `built`, and **moves no materials** — a crate
in the hands (`Held::Crate`) is a picture, the count is the world's.
`JOB_HAUL = 20`, `JOB_BUILD = 21`; `Job::Build` is the ninth row, and
`Job::Haul`'s row now starts an errand of its own.

- **Inside or outside is decided by `task::site_stand`**, one place: the
  nearest tile beside the footprint — four ways, never a corner, like a
  rock — that the grid has a route to, else a tile *of* the footprint
  (plating goes under the Bim's own feet). Asked of the deck's grid from
  the Bim, and failing that of the outside grid from `Room::outside`, in
  `Game::site_reach`; asked again as the walk is entered, from wherever
  the body is on whichever grid it is on. A site the world has dropped
  has nowhere to stand and the walk is `blocked`, the way a shut door
  blocks one. `nearest_shelf` is the other end of a haul, and both are
  checked before the errand is offered, since `first_station` has no
  answer for a chain that picks its spot on entry.
- **`refresh_outside` keeps the outside grid while there are sites**, not
  only while there is a mining site — the decision above needs it.
- **`go_outside` takes `pick: bool`**: a walk to mine takes the pick, a
  trip to a site does not, and `StepIn` counts a walk (`walks_done`) only
  for `Kind::Eva`, or the world says "back from outside with nothing"
  after every build.
- **A load given up for good is handed back** in `let_go`: a `Haul` past
  `TakeMaterials` pushes `Room::returned`, whatever the hands happen to
  show — a walk resumed after an interruption starts with empty hands and
  `CarryToSite`'s `enter` puts the crate back in them, so the picture is
  not the record.
- **`Exclusive::Site(id)`** is one pair of hands per site aboard;
  outside chains take `Exclusive::Airlock` like a walk.
- **`Room::relayout` and `Game::relayout`** lay the room out again under
  the crew for a design that changed: geometry from the new layout,
  state kept — `Filth::resized` carries the dirt across by position, the
  doors match by opening, the beds by frame (index is identity, and new
  ones go on the end), the bay and the heads only if they moved. The
  grids and blockers are rebuilt and `rocks_version` bumped for the
  outside grid; a Craft chain whose bench index now names a different
  bench is abandoned. `from_layout` and `relayout` share `bed_side`,
  `layout_chairs` and `galley_faces` so the two cannot lay the board out
  differently. `Layout::shelves`/`Room::shelves` (every `Shelf`, worked
  from its use spot) is what a haul fetches from.

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
for an airlock and the ship painter's `airlock_ajar` is a picture; plus
the heads' `closed_door()`. The bodies are the crew and the visitors, as
for the doors themselves. The classic room's opaque is the heads' walls.

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
- **The fog is drawn over `Layout::hull`** (every structure tile) and,
  in the classic room, over the whole box; unseen tiles are merged into
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
  `Character::set_armed(Option<WeaponKind>)` — recruited, alive, armed,
  not outside, seated, napping or out cold — which is drawing only:
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
  aim goes on the picture first, and the shot is read off it, in the
  aiming branch and in an execution alike, `eye` (the body, or its peek)
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
  says it sees, faces it, and fires when `reload` is out — not while
  walking. The crew's goes nowhere of its own: a recruited Bim stands
  where it was put. **Recruiting is per Bim**: `pump_queue`,
  `consider_errand` and `flee_filth` ask
  `bims[who].character.is_recruited()`, not `Game::is_recruited()` (the
  player's) — they used to ask the player's, and recruiting James froze
  Kate's errands with his. `free_to_talk` asks it too, and
  `is_unconscious()` beside it: once recruiting was per Bim, Kate could
  start a chat with a recruited James and walk him to the talking spot,
  which is what broke `scratchpad/crew.rs` ("a recruited James never sets
  off"), and nobody talks to a Bim that is out cold.
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
  Each gun's bolt has a look of its own in `Combat::draw` — the pistol's
  dash as before, the shotgun a fan of five short orange pellets
  (`PELLETS`, `PELLET_SPREAD` ±6°, one bolt drawn five times), the rifle
  a short thin yellow tracer, the sniper a long thin white-blue streak
  with a bright head — each tinted towards the side's colour with
  `Color::mix` (added to `draw.rs` for it), so red still says whose.
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
  `strike(.., false)`, because `scratchpad/priority.rs` calls it. The
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
  splashes**, `Filth::splash_blood(at, rng, can_get_to)`: the tile under
  the body always, plus `SPLASH_TILES` (3 to 5, counting it) of the nine
  round it, each picked out of what is left so no tile is soiled twice,
  through the same `can_get_to` filter as `spatter` (blood flung under a
  counter's lip is blood the broom never reaches; `strike` passes
  `nav.can_reach`). `part_health`, `blood`, `wounds(who, part)`,
  `bleeding`, `legs_lost`, `trauma(who, part)`, `is_dying`, `lasting`,
  `is_unconscious` are the readouts. `tick_bim`
  multiplies the pace by `Health::pace()` (the legs lost, the blood
  under half, and every trauma on it) and the effort by
  `Health::works_at()`; after the death check, `Health::unconscious()`
  differing from `Character::is_unconscious()` is `knock_out(out)` —
  going out `interrupt`s the errand onto the queue first **and drops
  the gun** (`drop_weapon`, below), and the frame stops there for that
  Bim, like a nap, until the blood comes back (`OUT_AT` is **half its
  blood** since feature 89, with `SLOWED_AT`'s half pace moved up to
  three quarters so the band above it is still walked). Out cold
  it is drawn by `draw_lying`: the fallen figure in the **live** colours,
  **`OUT_COLD_SCALE` (a tenth) bigger** than the dead one beside it, with
  a slow breath and no Zs — size, colour and breath are the three things
  that tell a body out cold from a body gone, since the shape is the one
  figure. The fallen figure (`draw_flat`, shared
  with the dead one's `draw_fallen`) is a body **stretched out along its
  heading**, head forward: legs and boots trailing behind the hips, one
  arm up beside the head, the other along the side, the head turned
  onto its cheek — drawn at `FLAT_SCALE` (half) of the standing
  figure since the user asked, about a tile long and inside two
  whichever way it lies; it is the shape that says "down" at a
  glance. The **limbs are short** — arms and legs drawn in towards the
  body — because seen from directly above an arm or a leg on the deck is
  foreshortened away, and at full reach the sprawl read as a figure
  standing up. A click on a body down is tested against that line
  (`Character::picked_at`), boots to head at the same scale, not the
  standing circle. `scratchpad/layout.rs dead` puts both
  figures side by side. `Bim::tick_drips` drips **blood on the
  deck** every `DRIP_EVERY / wounds` seconds while it bleeds and lives,
  scattered ±10 off the body from the room's stream (a bleeding Bim is a
  fight, and no seed-pinned probe has one) — and a drop is **filth**:
  `room.filth.soil(at, BLOOD_COST, Mess::Blood)`, sixty off the tile,
  between a wetting and a ruined one, so a Bim standing still bleeding
  fouls the tile under it in a few drops. There is no picture of a drop of
  its own; `Filth::draw` paints a blood tile in a dark red and the rest in
  the brown, the broom takes it up like any stain, `dirty_tiles` counts it
  and `Job::Clean` comes round for it. `Mess::Blood` is the last kind (5)
  and last in the ordering on purpose: blood dripped onto, or walked off a
  tile onto (`Filth::track`), any other mess reads as blood, since a pool
  of it is what the player is looking for after a fight.
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
  end of it, and under arms nobody eats or sleeps (`consider_errand` and
  `pump_queue` both bow out for `is_recruited()`). Thirty is half again
  `CALM_RANGE`, so between the two a crewmate is under arms *and*
  doctors, which three tests lean on with an enemy twenty-five tiles
  off beyond the classic room's walls;
  `after_a_fight_at_a_hostile_dock_the_alarm_comes_down_and_the_crew_sleep`
  in the world's tests is the dock. The other thing that keeps a crew
  from bed after a fight is not the alarm at all: a body that bled past
  the line is **out cold** on the deck until somebody bandages it — for
  days, if everybody is — and that is the medical system, not sleep. Going up, `muster_crew`
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
  player's (`toggle_recruited`), and a hired mercenary is a bot like Kate. Consequences for tests: at a hostile dock
  every crew member fights now, so a test that counts one Bim's hits or
  bolts issues the others `Gear::default()` (no weapon) — the blade lock,
  the bandage-holsters and the shoot-on-the-move tests in `game::tests`,
  the sniper's long shot in the world's. `the_crew_take_arms_when_an_enemy_comes_within_range_and_stand_down_after`
  pins the range, the pack, the stand-down and the hold;
  `under_the_alarm_the_crew_gather_round_the_player_and_take_orders` the
  ring, the following, the order and its post. The header says
  `ALARM_STATUS` while it is up. Mind that James wanders in the classic
  room under `set_autonomous(false)` — the chat walks him — so a test
  reads `bim_pos(0)` rather than where it put him. **Both musters are
  on an edge, and a room is thrown away at every dock and undock**, so
  `take_crew` stands the war and the alarm down first: a body carried
  into a fresh room — which starts at peace — still recruited had
  nothing to let it go, and the crew flew home from a hostile dock in
  combat mode. The new room musters them again the step an enemy is in
  range. `casting_off_from_a_hostile_station_stands_the_crew_down` in
  the world's tests.
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
  pins it; the chase onto the ship is the world's
  (`an_enemy_follows_the_crew_member_it_saw_onto_the_ship_and_is_put_ashore_when_it_leaves`).
  **And the airlock is watched** (since September 2026): `Game::watched`
  is a spot the room's people have eyes on whatever they are doing —
  `set_watched(Some(p))`, which the world sets to the tile just inside
  the station's door while the ship is docked (`Residents::join`) and
  the room otherwise has none — and `set_hostiles` counts a target
  within `AIRLOCK_WATCH` (2.5) tiles of it as *seen*, not stale, with
  nobody looking. A boarding is therefore what puts a station's people
  at war, at the door, rather than the moment one of them happens to
  look down the right corridor; a crew that stays aboard its own ship
  (the ship's airlock is three and a half tiles from the spot) is still
  nobody to them. A shot still wants real sight, so nobody fires at the
  door from across the station. `a_hostile_room_s_airlock_is_watched_and_a_boarder_at_it_is_seen_by_nobody_in_particular`
  (game) and `a_hostile_station_notices_a_boarding_at_its_airlock_with_nobody_looking`
  (world, every resident dead so no eye but the airlock's) pin it.
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
    **A room with no foreign half is not a last stand**: the classic
    test room and a ship flying alone have nowhere else in them, so
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
  fight rolls nothing the rest of the room rolls, so a probe's seed is
  the same whether or not somebody shot. The one exception is the blood
  on the deck — the drips' scatter and a cut's splash come off the room's
  stream — which is fine only because no seed-pinned probe bleeds.
- **`Character::hostile`** rings a body in red; the world sets it on every
  Bim of a hostile station's room (`Game::set_hostile_bodies`).
- **`HIT_BIM = 11`**: `Game::hit_at` answers it, after noting the
  fixtures and before asking the room, for a click within `pick_radius`
  of a living Bim, and `Game::hit_bim()` says which — the bandage menu.

Two things about the seam that bite, found while the world's and the
app's halves were built and not fixed here:

- **The crew's room steps before the residents'** (`World::step`), and
  each room reads the *other's* positions as they were handed over last
  step. So the step a charging blade arrives, the crew member's
  `melee_with` still sees it a tile off and does not lock, while the
  blade's own `melee_with` — in the residents' room, stepped after — sees
  the crew member in reach, lands its first blow that same step through
  `visit`, and only next step does the crew member's lock form. An
  unarmoured crew member (a body of 75) is half gone to the first
  thirty-five before `Locked` is ever said; the world's blade test wears the
  kevlar for exactly that, and the first cut of every melee is
  unanswered. Since the blow lands `SWING_TIME` after the swing starts,
  that test runs on step by step until the fist's twenty comes off the
  resident rather than reading it the step of the lock.
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
  both halves on a hand-laid room, and the world's sniper test has the
  resident's rifle land from twenty tiles and more.

`combat::tests` pin the tactics on a hand-laid walled room, which bodies
each kind of bolt finds, the curves
(`the_curves_pin_the_numbers_the_guns_were_asked_for`,
`damage_falls_off_with_the_distance_the_bolt_flew`), the dodge, the
issue (`an_issued_gear_rolls_every_kind_across_seeds_and_the_same_for_a_seed`)
and the charge and lock rule
(`a_blade_charges_the_nearest_target_and_a_gun_is_locked_only_by_a_blade`);
`game::tests` pin the war, an enemy's shot wounding and the tile under
the bleeding body reading `Blood`, the burst, the lock, the charge and
the cut's splash, the knock-out and `HIT_BIM` in the classic room.

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
  has a light diagonal stroke (`CRACK`) across it. The lying figure
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
  whether it is the armoury; the cold store is `HIT_FRIDGE` as before.
  `Game::container_spot(Container::Bench(i) | Shelf(i) | Fridge(i))` is
  where the Bim stands to use it, for `send_to`. **A left click notes
  the fixture indices now too**: `hit_at` and `drag_end` share
  `note_fixtures`, so the grid that opens off a left click is the
  fixture under *that* click and not the last right-clicked one.

`a_vest_takes_a_hit_first_and_its_protection_lifts_when_it_breaks`,
`equipping_swaps_with_what_is_worn_and_a_weapon_with_the_weapon` and
`give_and_take_round_trip_and_a_broken_piece_is_only_ever_discarded` in
`game::tests` pin it; the world's tests pin the hold's side.
`a_recruited_bim_shoots_the_enemies_it_can_see_and_they_are_hurt` in the
world tests stands James beside a resident of a station made hostile and
runs until `EnemyDown`; `BIMS_FIGHT=1 bims combat` is the picture
(`World::stage_fight_for_probe`), and `BIMS_WEAPON=schword` (or
`pistol|shotgun|rifle|sniper`) in the crew member's hand,
`BIMS_ENEMY_WEAPON=…` in every resident's and `BIMS_ARMOURED=1` for a
fresh helm, kevlar and leg guards on the crew member are how a swing, a
burst or a lock is looked at without walking the station for one —
without the armour a crew member charged by a schword is dead before it
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

## A carry between benches is a haul with a bench at each end

Feature 56 (September 2026): the world wants a gun or a piece of armour
walked from the lockers to the workbench's slots and the upgraded one
walked back, and the room's half is `Kind::Ferry { from, to }` — both
indices into `Room::benches` — modelled on `Kind::Haul`: `GoToStore`
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
`let_go(for_good)` past `TakeGear`, the hands emptied, the way a haul's
load goes back on the shelf. There is no new `Job`: the carry is
offered under `Job::Haul` ("Carrying things" already) — `consider_errand`
pushes `Haul` when `haul_on_offer` *or* `ferry_on_offer` has something,
and `do_some_work` tries `haul` then `ferry` — with its own readout code
`JOB_FERRY = 26`. `ferry_on_offer` wants both benches in the room, a
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
`CrewPanels::body_under_click`: `HIT_BODY`, `HIT_BIM` with the Bim down,
or `HIT_VISITOR` with the visitor down, and the click goes to `open_loot`
(which also walks the Bim shown over) instead of a menu; the right-click
keeps the rows, since a Bim out cold has the bandages on it too.

**A downed enemy is finished off** (RimWorld's execution). `Kind::Execute
{ visitor, blade }` — `GoToVictim` then `Execute` (`EXECUTE_SECONDS`, 3) —
walks to `task::victim_stand`: a gun to the free cell nearest it within
`EXECUTE_RANGE` (3 tiles) of the body with `Sight::clear_line` to it, a
blade (or a gun with no such cell) to a tile from the body towards it, as
for a patient. The body's place is `Room::bodies_down`, written from the
visitors and `visitors_down` in `tell_the_room_where_the_crew_are` every
step; a body that came round by the time the Bim arrives blocks the chain
at `Execute`'s enter. The picture is `tick_combat`'s: `Task::executing_at`
says where the body is while the hands are at it, the weapon is drawn
whether or not the Bim is under arms, and it fires at where the body lies
every `1 / fire_rate` (the bolt flies over it — a body down is nobody's
target) or swings every `MELEE_PERIOD` (`set_action(Swing)` outright: a
body on a chain is scripted and `antic` defers to the script); nothing
else until the chain is done. `Execute`'s leave pushes `(who, visitor)`
onto `Room::executed` (`Game::take_executed`), and the world kills the
body in its own room through `Game::execute_body` — `Health::give_up`,
which is dead at the top of the next tick, if it was still down and
alive. `Game::execute(who, visitor)` starts it (dead, out cold, outside
or empty-handed refused; a visitor not down refused); whether it is an
*enemy* is the world's check. `JOB_EXECUTE = 25`.
`an_execution_walks_to_the_body_and_the_room_says_who_was_finished` pins
both weapons, the body coming round, and the killing.

**A visitor on its feet is hit only when the world says it may be
spoken to.** `Game::set_visitors_hailable(&[bool])`, told right after
`set_visitors` every step like `set_visitors_down` (and cleared by it),
marks the world's mercenaries for hire; `hit_at` answers `HIT_VISITOR`
for one of those on its feet as well as for a body down, and
`Game::visitor_down(i)` says which the app's menu is on — the Loot row
or the Hire row. The room knows nothing of what is said; the fee, the
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
`can_reach`: `Game::can_reach`, `can_reach_through_door`, the seat pair in
`chat`, `nearest_shelf`, `site_stand`, `next_dirty`, `reachable_rocks`
and the spatter's filter. `path` is for a route that will be walked.

Why it is a rule and not a tidy-up: `work_on_offer` runs for every idle
Bim every step, and it asks about every shelf, every site, the bay and
the benches. When those asked `!nav.path(..).is_empty()`, an A* that
finds nothing walks every cell of its patch first — 40 000 cells on a
station's grid — and a station's room cost **900 µs a step**, which at
24x is the whole frame. `haul_on_offer` also looks for a site wanting a
load *before* it looks for a shelf, for the same reason. A docked step is
about 7 µs now; measure a new per-step question with the scratch bench
before adding it, and if it is a route, ask whether it is asked only when
the answer is used.

## Every bay is worked; a second table or a lone basin is drawn as itself, standing still

`Room::bays` is every bay aboard, the layout's own first and
`Layout::more_bays` after it, each with the side it is worked from.
`Kind::Tend { bay, spot }` names the bay; `Game::bay_work` asks every
bay in order and `tend_bay` walks to the first tray wanting a hand;
`update` grows them all against the one manager target. A click is
`Room::bay_at` → `HIT_HYDRO`, with the index kept as `Game::hit_bay`
exactly as `hit_door` is, and every `hydro_*` accessor takes the bay, so
the menu on a bay the crew built has that bay's switch and standing
order. `relayout` keeps a bay's trays by frame, like the beds.
`every_bay_aboard_is_worked_and_has_its_own_menu` in the world tests
pins it. **On a joined deck the station's bays are not the crew's**:
`Aboard::leave_the_station_s` drops every `more_bays` and `extras` entry
standing inside `Aboard::station_box` — at the join and at every
`Aboard::relayout` after it, since a part built relays the whole deck —
because the residents work and draw those in their own room, and a still
painted over a live fridge is two pictures of one door again.
`a_bay_the_crew_build_is_a_bay_like_the_first` lays a bay out as a site,
has the crew build it, and clicks it.

Every worktop, hob, cold store, dishwasher, locker, shower and toilet is
worked now — see the picks section above. What is left for the room to
only *draw* is a second table (every chair is seated already) and a basin
no toilet pairs with. The room used to leave every second fixture to the
ship painter, which has no picture for
those kinds and drew a coloured block: a station's hydroponics was one
bay of trays and three green slabs, and a hob the crew built was a
square. Now `layout_of` lists them as `Layout::extras` (a `room::Still` kind and the
rect), `Room::stills` (`Stills::from_extras`) holds them ready — a `Bath`
for a basin, a rect for a table — and
`Room::draw_stills` draws them after the fixtures, in `draw` and in the
designer's `draw_fixtures` alike. `drawn_by_room` now returns **every**
part of those kinds, so the painter draws none of them.

The pictures are functions of the fixture's index or rect:
`draw_worktop(list, i)`, `draw_hob(list, i)`, `draw_fridge(list, i)`,
`draw_locker(list, i)`, `draw_table_top(list, rect)`, with
`burner_of`/`hob_scale_of`/`knob_of` as functions of the hob's rect. A new picture for a fixture
goes in as a function of its rect first, and the room's own state on top.

## Fibre is a crop, a bandage is a thing in a pack, and the dressing is a walk to a crewmate

`hydro::Crop::Fibre = 3` is the third thing the bay grows — a paler,
taller stalk in `STRAW`, ripe in a `DAY` — and the one nobody eats: the
drug lab makes a bandage out of two of it (`shipdesign::recipes`, the
world's business). `manager::Stock::Fibre = 3` is its target,
`stock_target()` is a triple now, and `Bay::update`/`wants_work`/`wanted`
take `fibre` beside `veg` and `tofu`. **A fibre target of nought keeps
fibre out of `wanted` altogether.** The share arithmetic reads a target of
nought as a shortfall of nought, and a store over-committed on food —
greens short by one with two coming up — as *less* than nought, so
without the guard a bay nobody asked for fibre would plant it the moment
the food was in hand and every seed pinned on the bay would move. With
it the probes did not: `probe`, `priority` and `crew` run unchanged.

The counts: `Room::fibre` is the shelf, `Room::harvested_fibre` the tally
the world drains (`Game::take_harvested_fibre`) into the hold — `store`
bumps both, `let_go` included, since a harvest given up short of the
store is still a harvest — and `Game::set_stock(veg, tofu, stew, fibre)`
puts the hold's number back on the shelf every step. A sheaf in the hands
is drawn as greens (`Held::Vegetable`); `Task::lifted` is what the store
goes by.

**A bandage is a thing in a pack, and there is no count anywhere else**
(feature 87). `Room::bandages`/`bandages_used` are **gone**: a dressing
is `bims::game::BANDAGE` — `Item::Stack(combat::BANDAGE_CODE)`, code 13
said here because this crate does not know `physics` — kept in a Bim's
own `Gear`, **five to a box** (`Item::stack_limit`,
`combat::BANDAGES_A_BOX`) over a two-by-two footprint. `Game::bandages_of(who)`
is how many that Bim carries, `Game::give_stack`/`take_stack` put them
in and out by the unit, and `spend_bandage` is the one place one is
spent: the **emptiest** box first, so a pack tidies itself by being
used. The classic room deals every Bim `BANDAGES_AT_DAWN` (3) at
`Game::new` so `bims room` can try the chain; aboard it is
`World::restock_bandages` that fills a pack out of the hold, and a
station's people are dealt `data::RESIDENT_BANDAGES` when their room
opens. Nothing crosses the seam as a number any more — the world hands
the room no bandage count and reads none back.

**The pack stacks now, and the count is the cell's.** `Gear::count` is
a `u32` a cell beside `pack` and `turned` — **nought reads as one**, so
every `gear.pack[c] = Some(item)` written by hand is still one of it —
and `Gear::units(cell)` is the one accessor. `put_many`, `take_one`,
`room_in`, `stack_with_room`, `stack_to_spend` and `units_of` are the
rest; `put` is `put_many(.., 1)`, `take_out` takes the **whole** stack
and `rearrange` pours one box into another where the two are the same
thing with room, whatever is left staying where it was. A stack is the
one thing in a pack that stacks: the materials and the food stack on a
shelf and never on a back.

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

`a_bandage_is_walked_over_and_closes_the_wounds_on_one_part`,
`a_dressing_comes_out_of_the_pack_and_a_bim_out_of_sight_binds_every_wound`
and `a_fibre_target_has_the_bay_grow_fibre_into_the_store` in `game::tests`
pin both — the second at frame rate, because **`simulate(1.0)` cannot
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
either — starvation and `give_up`, which clears the traumas. The
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
  `take_medkits_used`, `MEDKITS_AT_DAWN` = 2 in the classic room; the
  hold's aboard). `JOB_TREAT = 23`. The world is told through
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
  so a wound is dressed the moment there is a bandage; no seeded probe
  bleeds, so nothing moved. **A bot under arms with nothing in sight
  doctors** — recruited, no target in `sees_any`, no lock and no blow —
  and takes its weapon up again the moment `aim` sees something; the
  player's own recruited Bim never does, being the player's. The whole
  doctoring errand is holstered (`dressing` in `tick_combat` covers the
  walk now), which is also what keeps the tactics off it.
  **A crewmate is doctored only in the calm** (since September 2026):
  `Game::calm` is three things at once — `Combat::lull()` (seconds
  since anything was fired or landed in this room, either side's: `fire`,
  `shoot`, `brawl` and `struck` set it to nought, `Combat::age` counts
  it up every step of `tick_combat`, `f32::MAX` in a fresh room) at
  least `CALM_AFTER` (20 s); `enemy_unseen_for` (seconds since any of the
  room's living, waking people on the deck had a target in sight — in a
  hostile room, since any target was a sighting rather than stale, the
  airlock's eyes counting) at least `CALM_AFTER`; and no enemy within
  `CALM_RANGE` (20) tiles of any of them (`enemy_within`, against the
  targets as handed — the crew's room knows every enemy's position, a
  hostile room its beliefs). `medical_on_offer` asks it **after** the
  helper's own bandage and before either kind of care for anybody else,
  so a wound of its own is still dressed on the spot whenever it has
  nothing in its own sight, and a crewmate is walked over to only when
  the fight has gone quiet — not the fight's end, which nobody in the
  room can know, but twenty seconds of it. Both rooms' people, so a
  hostile station's stop patching each other under fire too. The
  player's explicit `bandage`/`treat` orders are not gated: the menu is
  the player's call.
  `a_crewmate_is_doctored_only_twenty_seconds_after_the_last_shot_sighting_and_enemy_near`
  pins the three clocks one at a time — an enemy hidden in the heads
  (the room is under twenty tiles across), a hostile bolt fired at the
  floor, a sighting that ends — and the own wound bound regardless.
  **Its own kit before a new one.** The world says every step how many
  medkits each Bim carries in its **own pack** (`Game::set_pack_kits`,
  off the packs; `Room::pack_kits`, `Room::carries_kit`), and a helper
  that has one opens it where it stands: `GoToKit`'s target is the Bim's
  own position — no walk to a cabinet at all — and `TakeKit` takes it
  out of the pack rather than off the shelf, saying whose on
  `Room::pack_kits_used` (`Game::take_pack_kits_used`). The world then
  takes the medkit out of that pack and puts it **on the hold's count**,
  because from the moment a kit is in a hand the counting is the shelf
  kit's: `medkits_used` charges the hold for it when the treatment is
  done, and `let_go` puts it back on the shelf when the chain is given
  up for good — so a medic's own kit spent is the hold untouched and the
  pack one down, and a treatment abandoned leaves the kit in the hold. A
  kit in the pack counts as a kit everywhere the shelf's does:
  `medical_on_offer` offers the treatment for it and `Game::treat` is
  bare-handed only with neither.
  **Otherwise the kit is fetched.** `Kind::Treat` is `GoToKit → TakeKit →
  GoToPatient → Dress`: `task::kit_stand` picks the nearest of
  `Room::kit_stands` — the use spots of every container that takes a
  medkit, `Game::set_kit_stands` from the world every step
  (`hand_the_room_the_hold_s_medicine`, empty while the hold has none) —
  and a room with none (the classic room, a station's) takes the kit on
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
  `a_treatment_walks_to_the_kit_first_and_carries_it_to_the_patient`,
  `under_the_alarm_a_crewmate_with_nothing_in_sight_doctors…` and the
  world's `a_treatment_aboard_fetches_the_kit_from_a_cabinet…` pin it. **A patient holds still**: `tick_bim`
  stops the frame for a Bim that `is_being_seen_to` (somebody else's
  `Bandage` or `Treat` names it), its errand interrupted onto the queue
  and `Character::halt`ed — a dying Bim walking its errands had the
  helper arrive at an empty spot, twenty minutes lost and the walk
  begun again, for as long as it kept walking. Not while it flees.
- **A Bim dying runs from the fight, and does not shoot.**
  `Game::is_fleeing(who)`: dying, on its feet on the deck, and any target
  the world named is `Some`. In `tick_combat` that comes before arming:
  the weapon stays in the hand but is holstered (`set_armed(None)`), the
  burst, lock, blow and peek are dropped, and `flee(who, dt)` runs on the
  `plan_wait` clock — `Tactics::flee(nav, from, targets)`: the enemy is
  one place, the **average of every target up**, and the best cell is the
  reachable one within `FLEE_LOOK` (10) tiles furthest from it less
  `WALK_COST_PER_TILE` a tile, so it runs the opposite way and round a
  wall if it has to; a route there, the errand interrupted first. It
  goes for **the player's own Bim too**, recruited or not, and for a
  hostile room's people (a resident shot to a dying state runs from the
  crew). `pump_queue`, `consider_errand` and `flee_filth` skip a fleeing
  Bim like a recruited one; the enemy gone, it stops where it is and the
  queue picks up. `a_dying_bim_runs_from_where_the_enemy_are_and_does_not_shoot`
  pins both Bims.
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
  alone, and dies of the blood alone
  — which is how most of a fight's dead die now, and why
  `one_on_one` in the world tests uses `Game::kill_for_probe` (health
  `give_up`) rather than a head shot.
- **Asleep is `Action::Sleep`, and it is a different thing from out
  cold.** `Game::is_asleep(who)` reads the hands — a doze in a bunk and a
  nap on the feet both set it — and is what the world asks, beside
  `is_unconscious`, before a crew member may fly or trade
  (`World::fit_to_act`, b-next, September 2026: a body dead beside the
  helm used to fly the ship). `nod_off_for_probe(who, minutes)` is the
  nap given rather than rolled, for a test that wants one asleep on the
  spot.
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
`SPOT_RESEARCH = 22` is the ringing code, so `SPOT_NAMES` in the app is
twenty-three long. The room knows nothing of what a key opens.

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
take to part lost a hunter its quarry: the chase in
`an_enemy_follows_the_crew_member_it_saw_onto_the_ship…` turns on the
resident seeing James *in* the ship's airlock the instant it reaches the
station's, and read off the leaves the belief stuck in the doorway.

**Smashing.** `Door::locked_by` is a `Locker` — `Crew` from the panel,
`Body(who)` for one of the room's own — and `Door::smash(by, dt)` is a
body heaving at a locked door: one at a time (`Smash { by, done,
since_heave }`), `SMASH_DOOR` 15 s for a bulkhead door and
`SMASH_AIRLOCK` 30 s for an airlock, a `Cue::DoorSmash` every two seconds
and `Cue::DoorForced` when the lock gives (`unlock`), drawn as a bar over
the door across the opening in `WARN` (`smash_progress`). The app plays
`Force_opening_Door_and_Arilock.mp3` cut to one heave (`door_force.ogg`,
`Kind::Smash`) for both. **Only a hostile room's people smash**:
`Game::breach(who, dt)`, after `plan_stand` for a body at war, on its own
`breach_wait` clock — nothing while a route to any target is open;
otherwise the nearest locked door whose panel (`station`, `DOOR_STAND_OFF`)
it can reach, its own locks first, and it walks there; at the panel it
unlocks its own lock or sets `Bim::smashing` and heaves every frame until
the door gives or it moves off (`drop_smash`). The crew's bots never do
— the crew are the player's to send.

**The hunter and the post** (September 2026, for the raiders in
`crates/world/CLAUDE.md`). Two things the raid found wanting, both in
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
  locked against them smash it and walk on. `Game::post_at(who, to)` is
  `send_to` that keeps the post when there is no route yet —
  `return_to_post` plans the walk again as the way opens (a `path` with
  no route is a region check, not a search) — and `has_post` is what the
  world re-posts by after a fight, since `muster` drops every post.

`enemies_follow_a_crew_that_retreats_aboard_and_force_the_ship_s_locked_airlock`
(world, a blade and a pistol) pins the whole of it across the two rooms.

**Sealing in.** In `flee` a hostile body's run notes the first unlocked
door its route passes and which side it set out from (`Bim::seal`);
`seal_and_bind`, in the fleeing branch of `tick_combat` for hostile rooms
only, locks that door `Locker::Body(who)` once the body is through and
clear of the opening (`Bim::sealed_in`), and while its lock stands binds
its wounds every `BIND_EVERY` (10 s): the part bleeding most bandaged,
else a trauma treated — a field dressing out of its own pockets — so with
nothing bleeding it is dying no more, `is_fleeing` is false, `breach`
finds its target behind its own lock, unlocks it, and it fights again.
The crew's fleeing Bims do neither. `an_airlock_is_a_door_that_locks`,
`an_enemy_smashes_through_a_locked_door_in_fifteen_seconds` and
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
  tells (the test room) keeps them so. A room *never
  handed* lights is lit throughout (`Room::new`, the classic room); a
  designed deck handed none is dark everywhere. **In the dark a Bim sees
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
  brownout is dark, asleep and spoiling"). The bay has the same switch:
  `hydro::Bay::powered` / `Game::set_hydro_powered`, and unpowered it
  hibernates whatever the store or a standing order says.
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
  uploads the changed box with `set_partial` when it holds the version
  before (else the lot) and draws one textured quad, filtered, over the
  shapes and under the words, its corners the map's through the screen's
  transform — the room's scale on the room screen,
  `world_paint::light_map_on_screen` (the crew's names' arithmetic) on the
  game's. The lamps' own pictures (`fittings::wall_light`, flush to its
  wall; `standing_light`) draw no halo: the map is the light. `Fog::All`
  is unchanged: tiles, black.

## A field is a bay at half pace, outdoors

Feature 54, `hydro.rs`. A town on a planet eats off **fields**:
`PartKind::Field` is six tiles long with use spots along its north like
the hydro bay, and the room makes one a `Bay` — `Bay::field(frame, side)`,
`Bay::at` with `pace: FIELD_PACE` (0.5) and `outdoor: true`, both
serialised — so every errand, menu and standing order that works a bay
works a strip unchanged. Three things differ:

- **The pace.** `update` grows a tray by `minutes * pace`: a field takes
  two days to a bay's one, so a town needs twice the strips a ship needs
  bays, which is why a town is big. `is_field()` says which it is;
  `Game::hydro_is_field(bay)` hands it to the app for the menu's title.
- **The plug.** `set_powered` is a **no-op** on an outdoor bay — there is
  nothing to plug in — so `powered()` stays true through a brownout and
  `Game::set_hydro_powered`, which switches every bay aboard at once,
  needs no change; `Game::hydro_powered` reads true for a field.
- **The picture.** `draw` hands an outdoor bay to `draw_field`: turned
  earth (`EARTH`) the frame's size with a darker edge, a furrow along
  each tray (`FURROW`), and the crop through the same `draw_plant` at
  full light — no panel, no grow lights, no glow, the stir ring as
  before. The indoor picture is byte for byte what it was.

The layout carries them as `More::fields` (a frame and the side it is
worked from, `aboard::layout_of` off the part's first use spot like a
bay's); `Room::from_layout` and `relayout` chain them after `more.bays`
into `Room::bays`, and `relayout` keeps a strip's trays by frame **and
kind** — a field on a bay's old frame starts bare. None is ever the
layout's own bay (`Field` is not in `MAPPED`), and every one is a solid
through `others` and `Room::solids`. `drawn_by_room` lists them so the
ship painter leaves them to the room.

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
  need's chain take a body home from the plain.
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
- The classic room and a ship on its own have `plane: None` and none of
  this runs: `refresh_afield` returns at once, `plan_route` is the
  deck's grid, `walk_to(.., false)` is `nearest_free` and `path` as
  `dispatch` always was, planned before the interrupt so the seeded
  probes see the same routes.
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
one: `Look::CLASSIC` for the first two, so the room's pair are the pair
they were, and for every index after a hair, a shade and a build off a
multiplicative hash of the index. **Not off the RNG, on purpose**: a
draw in `Bim::new` would move every roll after it and re-seed every
probe ("Adding furniture moves everything"). `Game::look(who)` and
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
  and `draw_hair_lying` (the head on its cheek in `draw_flat`, called
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
`hold` when not, `pull_single` for finishing a body off — and `Bim::trigger`
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
cut, then `Filth::splash_blood` the way a cut splashes), a target as a
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
which is the wall working, not the taunt failing. And `Room::picked` is
`(site, resource, units, who)` now: the hauler goes with the load,
because how big a load a trip carries is that crew member's (*pack
mule*) and the world works it out again as the load is taken.

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
has no `Character`. No needs, no sleep, no bunk, no social life, no
memory, no schedule, no blood, no wounds, no traumas, no gear and no
pack. What it has is a position, a heading, four parts that break, an
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
`take_from_body`, `execute_body`, and `strike_droid` in place of
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
  `take_from_body` refuses one, wreck or not; `execute_body` refuses it
  too, there being no down-and-alive state to finish off. What keeps the
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
a crewmate wants the calm (`Game::calm`), which is exactly what it has
walked to. Carrying and still in it, it runs `Tactics::flee` with the
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
