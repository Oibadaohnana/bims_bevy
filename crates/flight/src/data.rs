//! The numbers a trip is flown by, kept apart from the arithmetic.
//!
//! Placeholders, like every other table in this workspace — but one of them
//! is a placeholder **against a scenario** rather than against nothing, and
//! that is the only thing that makes it checkable: `PartDef::torque_thrust`
//! — which lives next door in `shipdesign`, because it is a fact about a
//! part — is set so that four thrusters on the flyable fixture
//! (`shipdesign::fixture::flyer`) turn it through half a circle inside two
//! game hours. A flip that takes a day would make braking by turning round
//! a worse deal than a backward engine in every case, and the choice between
//! them would stop being a choice. It is pinned by a test rather than by a
//! comment: `four_thrusters_flip_the_reference_inside_two_hours`.
//!
//! There used to be a second, the fuel a unit of thrust burnt a minute, set
//! so one tank crossed the longest reference hop. There is no fuel now: the
//! engines run on the reactor, and what bounds a trip is how hard the
//! reactor lets them push — `shipdesign::power::thrust` — which is in the
//! `Dynamics` before a plan is made.

/// How close to a station a trip finishes.
///
/// The arrival point is this far **short** of the target along the approach
/// line, so a ship that arrives has not flown into the thing it was aiming at.
pub const ARRIVAL_RADIUS_STATION: f64 = 3_000.0;

/// The same for a body, which is a great deal bigger than a station even
/// though the generator stores it as a point. Flying to a gas giant means
/// flying to somewhere near it.
pub const ARRIVAL_RADIUS_BODY: f64 = 15_000.0;

/// How near the bearing counts as pointing at it.
///
/// A trip whose target is already within this of the ship's nose skips the
/// align phase entirely rather than turning through a thousandth of a radian,
/// which would be a rotation phase of no length that the plan walker would
/// have to special-case anyway.
pub const ALIGN_TOLERANCE: f64 = 0.01;

/// The floor under a ship's moment of inertia.
///
/// Strictly greater than zero because [`crate::Dynamics::alpha`] divides by
/// it. A ship small enough for this to matter is one part welded to nothing,
/// which is not a ship — but a division by zero is an infinite angular
/// acceleration and a heading of NaN, and a NaN heading is a ship that
/// disappears rather than an error anybody can read.
pub const INERTIA_FLOOR: f64 = 1.0;

/// Below this, a distance is not a trip and a speed is not motion.
///
/// One world unit is about a fiftieth of a tile, so this is well under the
/// width of a bulkhead — it is here to keep a plan of no length out of the
/// segment walker, not to be a tolerance anybody flies to.
pub const STILL: f64 = 1e-6;
