# Flight

Notes on `crates/flight` — what a design does when pushed, and the closed-form
plan that flies a trip.

> **Since the port to Bevy (September 2026):** the browser host is gone.
> Every name table this file points at in `web/*.js` is now
> `crates/app/src/names.rs`; the pages are the screens in
> `crates/app/src/screens/`; the `bims_*` and `ship_*` exports are methods on
> `bims::game::Game` and `ship::Session`. The node harnesses
> (`scratchpad/*.mjs`, `smoke.mjs`, `stub.mjs`) are gone with the host — a
> check this file says lives in one of them lives nowhere now unless it says
> so in `crates/app`'s tests or `./check smoke`, and is worth restoring as a
> unit test if the thing it guarded moves again.

## A plan is read, never integrated

`flight::plan_trip` works a trip out **once** and `state_at(plan, minutes)`
evaluates it. Nothing accumulates: calling it with 0, then 3, then 3.5 gives
the same answers as calling it with 3.5. That is the only reason a browser at
24x, a browser at 1x and a server catching up on an hour of somebody's
disconnection can agree about where the ship is.

So: **do not add anything to a plan that has to be stepped.** A plan is a list
of segments with fixed durations, and inside a segment nothing changes rate.
If something new needs a rate change, it is a new segment.

Two things fall out of that and are easy to undo:

- **A trip keeps the `Dynamics` it was planned with.** Welding a wall on
  halfway does not move an arrival that has already been promised; it changes
  what the *next* plan will be like. `World::on_ship_changed` recomputes the
  live dynamics and deliberately leaves the active plan alone.
- **Nothing is burnt.** There is no fuel (September 2026): the engines run on
  the reactor, and what the reactor can feed them is a **throttle** on the
  thrust, worked out by `shipdesign::power::thrust` *before* the dynamics are
  — `a_forward` is already the fed push, and a dark engine (no live conduit
  under it) is no engine at all. Every burning segment carries the set's
  `power` — `Dynamics::forward_power`/`backward_power`, the throttled draw a
  minute — and `effort_at(...).power` is what the world's power stage charges
  the ship while it flies. Constant through a segment like everything else;
  the batteries are deliberately not in the throttle, since a burn that ran
  off them for a while would be a rate change inside a segment. A ship's
  mass is therefore fixed for the whole of a plan.

## The placeholder numbers are pinned to scenarios, not to taste

`torque_thrust` on the thruster in `shipdesign::parts` is chosen against
`flyer` and a stated outcome: four thrusters turn the ship through half a
circle inside two game hours. `REACTOR_OUTPUT` against `ENGINE_POWER` there
is the other: the flyer's one engine is fed **flat out** off its one reactor
with the ship's systems running, and crosses the generator's longest
reference hop. The tests that pin them are
`four_thrusters_flip_the_reference_inside_two_hours` and
`the_flyer_crosses_the_longest_reference_hop_on_its_reactor`;
`the_heavy_engine_is_faster_and_dearer_over_the_same_hop` pins the trade the
heavy engine is — throttled to under half on one reactor and still faster,
and power per unit of push the same for both — and
`what_the_fixture_actually_flies_like` beside them prints the numbers for
whoever has to move one next. Changing any without rerunning those is how a
flip becomes a worse deal than a backward engine in every case and the
choice between them stops being a choice. `FUEL_PER_THRUST_MINUTE` and the
`one_full_tank…` test are gone with the fuel.
