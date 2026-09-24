//! One world, opened the same way every time.
//!
//! Here rather than in the tests for the reason `shipdesign::fixture` is:
//! **two targets have to agree about it.** The browser runs
//! [`World::step`](crate::World::step) in wasm and the native server that will
//! one day be authoritative runs the same loop for x86, and the only way to
//! find out whether they agree is to run a fixed scenario on both and compare
//! each against the same written-down number.
//!
//! The native end is `tests.rs`; the wasm end is `ship_self_check` in
//! `crates/ship`, which the node harness reads. Both compare against
//! [`REFERENCE_CHECKSUM`]. If one target's arithmetic ever drifts from the
//! other's, exactly one of those two fails.

use shipdesign::fixture::flyer;
use worldgen::GalaxyType;

use crate::data;
use crate::world::{Command, World};
use crate::{Speed, Target};

/// What the reference crew have left over from the design phase.
pub const REFERENCE_MONEY: economy::Money = 40_000;

/// How many steps [`reference_run`] takes. Ten game minutes at 1x, which is
/// long enough to get a trip planned, confirmed and turning and short enough
/// that the wasm half of the check does not hold up a page load.
pub const REFERENCE_STEPS: u32 = 600;

/// What [`reference_run`] comes out at.
///
/// Pinned rather than computed, for the same reason `REFERENCE_HASH` is: a
/// test comparing two computed values would pass happily while both were
/// wrong. Update it only when the scenario below is meant to change — or
/// the checksum's shape does: it moved when the raids went in (September
/// 2026), since the schedule is hashed from the first step, and again
/// when an enemy's shelf became loot (`crate::plunder`), since the list
/// of them is hashed whole, and again when the research grew its queue
/// (feature 64), hashed the same way, and again when the tier-two key
/// went in: the cargo is one slot longer, the research tree one node,
/// and a station's key is a tier (`World::station_keys`), and again
/// when a system got a memory (feature 71, `crate::memory`): the losses
/// and the systems left are hashed whole after the plunder, and again
/// when the machines went in (feature 83, `crate::droid`): which
/// stations they hold and the state of each is hashed after the
/// reinforcements, with the tier they come at, the reinforcement clock
/// and the wave cap.
/// And again when every player got two standing orders for the bots
/// that follow them (feature 84, `crate::orders`): one `Standing` a
/// player slot, hashed after the squad order — an order moves bodies,
/// so two clients that disagree about it disagree about where the crew
/// are standing.
/// And again when the dead started staying where they fell (feature 85,
/// `crate::memory`): the graves after the losses — whose deck, where on
/// it and what is still on the body — and the nodes the ship has been
/// at after them, both for the live system and inside every memory.
/// And again when the engineer's kits became **charges on a cooldown**
/// (feature 88, `crate::deploy`): a deployable's `shots` are gone from
/// the hash — nothing in the game carries ammunition — and
/// `World::kit_timers`, when each engineer's next charge of each kit is
/// due, is hashed after the re-used kits, since a charge in the pack is a
/// sentry that can be laid and one still cooling down is not.
/// And again when **every** class's ability became a charge on a
/// cooldown (feature 90, `crate::class::Charge`): those kit timers and
/// the soldier's `last_throw` are one `World::charge_timers` now — the
/// two kits and the grenade, in charge order, in the kits' old place —
/// so the soldiers' block is the brace and the *rampage* stacks alone.
/// And again when the crisis went in (feature 92, `crate::droid`): the
/// star the machines began at and the day the first one turns, hashed
/// with the droids' other dials — the hop table is derived from the
/// origin and the galaxy and is not in there — and the machines' hold on
/// a system's stations filed with the rest of that system's memory.
/// And again when the medicine became everybody's charges on a cooldown
/// (`crate::class::Charge::{Medkit, Bandage}`): `World::charge_timers` is
/// five a crew member, and the hold no longer fills anybody's pack.
/// And again for a run (feature 102): the world's four switches hashed at
/// the end, the hostile list empty with every human friendly, the crisis
/// there from day nought, and the room's needs standing still.
/// And again for the run's loop (feature 103): `World::run` hashed at the
/// very end, the droid and town clocks in mission steps, and the scenario
/// itself grown a second half — back to the ship, the map, a trip resolved
/// and a mission at the far end of it.
pub const REFERENCE_CHECKSUM: u64 = 0x_3b79_4d55_6b05_0f5d;

/// A world with the flyable fixture docked at the simulation's spawn: the
/// default seed's first dock, which is where every fixture world starts.
pub fn reference_world() -> World {
    simulation_world(flyer(2), REFERENCE_MONEY, 2)
}

/// A world opened the way the simulation opens one: the default seed, a
/// two-arm spiral, and [`crate::spawn`]'s dock. What `nix run .#simulation`
/// does with [`shipdesign::fixture::playtest_ship`] and
/// [`data::SIMULATION_MONEY`], and what every fixture here does with its own
/// ship and purse.
pub fn simulation_world(
    design: shipdesign::ShipDesign,
    money: economy::Money,
    players: u32,
) -> World {
    let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (star, station) = crate::spawn(&galaxy).expect("the default seed has a dock somewhere");
    World::start(
        design,
        money,
        players,
        data::DEFAULT_SEED,
        GalaxyType::SpiralTwoArm,
        star,
        station,
    )
    .expect("the default seed should have somewhere to spawn")
}

/// [`simulation_world`] with `crew` aboard, of whom the first `players`
/// are players — `World::start_with_crew`. A world of one player and two
/// crew is one where the second is a crewmate nobody steers: a bot, under
/// the alarm, since feature 59 gave every player's own to its player.
pub fn crewed_world(
    design: shipdesign::ShipDesign,
    money: economy::Money,
    players: u32,
    crew: u32,
) -> World {
    let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (star, station) = crate::spawn(&galaxy).expect("the default seed has a dock somewhere");
    World::start_with_crew(
        design,
        money,
        players,
        crew,
        data::DEFAULT_SEED,
        GalaxyType::SpiralTwoArm,
        star,
        station,
    )
    .expect("the default seed should have somewhere to spawn")
}

/// Somewhere in the spawn system that is not where the ship is standing.
///
/// The lowest-numbered node that is not the dock, not the dock's own
/// parent body — which body that is depends on the generator, and a trip
/// to it from its orbit is a hop to the point over it rather than the
/// burn the checksum was pinned on — and not already there, so the
/// scenario does not depend on which of them the generator happened to
/// put nearest.
pub fn reference_target(world: &World) -> Target {
    let docked = match &world.ship.state {
        crate::ShipState::Docked { station } => Some(*station),
        _ => None,
    };
    let parent = docked.and_then(|id| world.system.station(id)?.parent_body);
    for node in world.system.nodes() {
        let target = match node {
            worldgen::Node::Station(id) if Some(id) == docked => continue,
            worldgen::Node::Body(id) if Some(id) == parent => continue,
            worldgen::Node::Body(id) => Target::Body(id),
            worldgen::Node::Station(id) => Target::Station(id),
        };
        if world.preview(target) != Err(flight::PlanError::AlreadyThere) {
            return target;
        }
    }
    Target::Point(worldgen::math::dvec2(0.0, 0.0))
}

/// The scenario: open a world, confirm a trip, and run for
/// [`REFERENCE_STEPS`] — with the old game's free clock, since a trip is
/// flown in it — and then the run's own loop (feature 103): both players
/// back to the ship, the map, a destination put and accepted, the trip
/// resolved, and [`REFERENCE_STEPS`] of the mission that begins there.
///
/// Everything a checksum is meant to catch is in it — a plan made, a burn
/// drawing on the reactor, a heading turning through a trigonometric function, the local
/// frame changing as the ship leaves the dock, and a trip's length worked
/// out from a square root and put on the clock.
pub fn reference_run() -> u64 {
    reference_run_world().checksum()
}

/// The world [`reference_run`] ends on, for a test that wants to see the
/// scenario did what it says: flew, went back to the ship, travelled.
pub fn reference_run_world() -> World {
    let mut world = reference_world();
    world.set_free_clock(true);
    let target = reference_target(&world);

    // The second player at the helm, since the ship is flown from there;
    // both ask for a day a minute, so the effective speed is a decision that
    // was actually taken rather than the default. `Day` rather than `Top`:
    // the request's code is in the checksum, and code 4 is what the
    // reference was pinned with before 48x went in above it.
    world.man_the_helm_for_probe(1);
    world.step(&[
        Command::SetSpeed {
            slot: 0,
            speed: Speed::Day,
        },
        Command::SetSpeed {
            slot: 1,
            speed: Speed::Day,
        },
        Command::Confirm { slot: 1, target },
    ]);
    for _ in 1..REFERENCE_STEPS {
        world.step(&[]);
    }
    // And the run (feature 103): the world clock standing still, both
    // players pressing Back to ship — aboard, so the ship leaves — the
    // first destination of the map put and accepted, and a mission there.
    world.set_free_clock(false);
    world.step(&[Command::Return { slot: 0 }, Command::Return { slot: 1 }]);
    world.step(&[]);
    let site = world
        .travel_quotes()
        .into_iter()
        .find(|(site, quote)| quote.is_ok() && Some(*site) != world.current_site())
        .map(|(site, _)| site);
    if let Some(site) = site {
        world.step(&[Command::Propose {
            slot: 0,
            star: site.star,
            station: site.station,
        }]);
        world.step(&[Command::Accept { slot: 1, yes: true }]);
    }
    for _ in 1..REFERENCE_STEPS {
        world.step(&[]);
    }
    world
}

/// A reading of **only the state that outlived the old game's deletion**
/// (feature 104): the world clock and the mission clock, the run's
/// phase, deaths and pending bounty, the pool, every body's position,
/// health and gear on the crew's deck and the site's, the crew's
/// experience and classes, the machines, and every site's state.
///
/// `world_checksum` could not carry the deletion across: the old game's
/// switches and the needs were in it, so the deletion had to move it.
/// This hashes nothing the deletion took away, so a seeded run read with
/// it on the tree before the deletion (the `needs-sim-final` tag) and
/// after it has to come out the same — `tests_survivors.rs` here, and
/// the commands `crates/ship` builds, are pinned against that. Values
/// go in by their names where they are enums, so a variant deleted or a
/// discriminant closed up moves nothing that did not itself move.
pub struct Survivors(u64);

impl Survivors {
    pub fn new() -> Survivors {
        Survivors(0xcbf2_9ce4_8422_2325)
    }

    /// The reading so far.
    pub fn value(&self) -> u64 {
        self.0
    }

    fn eat(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.0 ^= byte as u64;
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    /// A float onto a thousandth, as the world's own checksum grids one.
    fn eat_f(&mut self, value: f64) {
        self.eat(((value * 1_000.0).round() as i64) as u64);
    }

    /// A value by its name.
    fn eat_debug(&mut self, value: &impl std::fmt::Debug) {
        let s = format!("{value:?}");
        self.eat(s.len() as u64);
        for b in s.bytes() {
            self.eat(b as u64);
        }
    }

    /// Every body on one deck: where it is, its health and its gear, and
    /// the machines' where they are and what is left of them.
    fn eat_room(&mut self, room: &bims::game::Game) {
        self.eat(room.crew_count() as u64);
        for who in 0..room.crew_count() as usize {
            let at = room.body_pos(who);
            self.eat_f(at.x as f64);
            self.eat_f(at.y as f64);
            self.eat(room.is_alive(who) as u64);
            self.eat(room.is_down(who) as u64);
            self.eat_f(room.blood(who) as f64);
            for part in bims::health::Part::ALL {
                self.eat_f(room.part_health(who, part) as f64);
                self.eat(room.wounds(who, part) as u64);
                self.eat_debug(&room.trauma(who, part));
            }
            self.eat_debug(&room.gear(who));
        }
        self.eat(room.droid_count() as u64);
        for droid in room.droids() {
            self.eat_f(droid.pos.x as f64);
            self.eat_f(droid.pos.y as f64);
            self.eat_debug(&droid.kind);
            self.eat_debug(&droid.tier);
            self.eat_debug(&droid.body);
        }
    }

    /// Everything that survived about `world`, added to the reading.
    pub fn eat_world(&mut self, world: &World) {
        self.eat_f(world.clock_minutes);
        self.eat(world.mission_steps());
        self.eat(world.run.phase as u64);
        self.eat(world.run.missions as u64);
        self.eat(world.run.deaths);
        self.eat(world.run.pending_bounty);
        self.eat(world.money);
        self.eat(world.star_id as u64);
        self.eat_debug(&world.ship.state.alongside());
        // Experience and the class it is spent in, crew member by crew
        // member.
        for (class, progress) in world.classes.iter().zip(&world.progress) {
            self.eat_debug(class);
            self.eat(progress.xp as u64);
            self.eat_debug(&progress.picks);
        }
        for fallen in &world.run.fallen {
            self.eat(fallen.slot as u64);
            self.eat(fallen.order);
        }
        // The bodies: the crew's deck and the site's.
        self.eat_room(&world.aboard.room);
        match &world.residents {
            Some(residents) => {
                self.eat(1);
                self.eat_room(&residents.aboard.room);
            }
            None => self.eat(0),
        }
        // Every site's state: the machines' hold on each, a town's fight,
        // the towns held — and, for this system's sites, whether each is
        // cleared, held and threatened.
        for it in &world.infested {
            self.eat(it.station as u64);
            self.eat(it.waves_left as u64);
            self.eat(it.wave as u64);
            self.eat_debug(&it.next_wave);
            self.eat(it.settled as u64);
            self.eat(it.cleared as u64);
        }
        for d in world.defenses() {
            self.eat(d.station as u64);
            self.eat(d.waves_left as u64);
            self.eat(d.wave as u64);
            self.eat_debug(&d.next_in);
            self.eat(d.standing as u64);
            self.eat(d.settled as u64);
            self.eat(d.won as u64);
            self.eat(d.lost as u64);
        }
        for &town in world.held_towns() {
            self.eat(town as u64);
        }
        for site in world.sites_at(world.star_id) {
            self.eat(site.station as u64);
            self.eat(world.site_cleared(site.station) as u64);
            self.eat(world.is_droid_held(site.station) as u64);
            self.eat(world.town_threatened(site.station) as u64);
        }
    }
}

impl Default for Survivors {
    fn default() -> Survivors {
        Survivors::new()
    }
}
