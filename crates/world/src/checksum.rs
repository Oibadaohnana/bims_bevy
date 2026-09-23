//! One number that says whether two copies of the world agree.
//!
//! Nothing uses it yet. It is here for the same reason `design_hash` was here
//! before there was a lobby: the moment there is a transport, a client and a
//! server will each be running [`crate::World::step`] and the only cheap way
//! to find out that they have drifted is to compare a number every so often.
//! Writing it now means the *loop* is designed to be comparable rather than
//! being made comparable later.
//!
//! # Why the floats are rounded
//!
//! FNV-1a over the raw bits of an `f64` would be exact, and exactness is
//! precisely the problem. Everything here that is not arithmetic is `sin`,
//! `cos`, `atan2` and `sqrt`; the first three come out of the platform's libm
//! on native and out of Rust's own on `wasm32-unknown-unknown`, and those two
//! are allowed to differ in the last bit. A checksum that noticed a
//! last-bit disagreement would fire constantly and say nothing.
//!
//! So every float is rounded onto a grid on the way in: positions to a
//! thousandth of a world unit, which is a hundred-thousandth of a tile, and
//! angles and speeds to a millionth. A real divergence — a ship that set off
//! and one that did not, a purchase one side made and the other did not — is
//! orders of magnitude larger than either grid. A rounding difference is
//! invisible. That is the trade, and it is the right way round.
//!
//! The one thing that is **not** rounded is the design: `shipdesign` is
//! integers throughout precisely so its hash is exact on both targets, and
//! that hash goes in whole.

use crate::armour::Where;
use crate::world::{ShipState, World, node_key};

/// FNV-1a, written out by hand.
///
/// Not a `Hash` derive and not `DefaultHasher`, for the reason `design_hash`
/// gives at more length: those are explicitly allowed to differ between
/// builds, and this number crosses between machines.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct Fnv(u64);

const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;

impl Fnv {
    fn new() -> Fnv {
        Fnv(OFFSET)
    }

    fn eat(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.0 ^= byte as u64;
            self.0 = self.0.wrapping_mul(PRIME);
        }
    }

    /// A float, onto a grid first. See the module note.
    fn eat_rounded(&mut self, value: f64, scale: f64) {
        let quantised = if value.is_finite() {
            (value * scale).round()
        } else {
            // A NaN or an infinity is a divergence in itself, and it has to
            // hash to *something* rather than to whatever `as i64` does with
            // it on this target.
            f64::MAX
        };
        self.eat(quantised.to_bits());
    }
}

/// A thousandth of a world unit. A tile is 52 units, so this is well under a
/// millimetre in a game whose distances run to a hundred million.
const POSITION_GRID: f64 = 1_000.0;

/// A millionth of a radian, and of a unit per minute.
const FINE_GRID: f64 = 1_000_000.0;

/// A hundredth of a health point, for a piece of armour: a hit takes
/// whole points off it.
const HEALTH_GRID: f64 = 100.0;

/// Everything about the world that two clients have to agree on.
pub fn world_checksum(world: &World) -> u64 {
    let mut hash = Fnv::new();

    hash.eat(world.steps);
    hash.eat_rounded(world.clock_minutes, FINE_GRID);
    hash.eat(world.galaxy_seed);
    hash.eat(world.galaxy_type as u64);
    hash.eat(world.star_id as u64);
    hash.eat(world.money);
    for &target in world.craft_targets.iter() {
        hash.eat(target as u64);
    }
    for body in &world.health {
        hash.eat_rounded(body.points, FINE_GRID);
        hash.eat_rounded(body.dose, FINE_GRID);
        hash.eat(u64::from(body.dead));
    }

    let ship = &world.ship;
    hash.eat(world.design_hash());
    hash.eat(ship.crew_count as u64);
    hash.eat_rounded(ship.anchor.x, POSITION_GRID);
    hash.eat_rounded(ship.anchor.y, POSITION_GRID);
    hash.eat_rounded(ship.heading, FINE_GRID);
    hash.eat_rounded(ship.charge, FINE_GRID);
    hash.eat(ship.destination_set_by.map(u64::from).unwrap_or(u64::MAX));
    hash.eat(ship.frame.code() as u64);
    if let Some(node) = ship.frame.node() {
        let (kind, id) = node_key(&node);
        hash.eat(kind as u64);
        hash.eat(id as u64);
    }

    hash.eat(ship.state.code() as u64);
    match &ship.state {
        ShipState::Docked { station } => hash.eat(*station as u64),
        ShipState::Holding => {}
        ShipState::Travelling { plan, departed } => {
            // The plan is what the ship will *do*, so it goes in rather than
            // only where the ship has got to: two clients agreeing about a
            // position and disagreeing about the trip is the drift this is
            // for.
            hash.eat_rounded(*departed, FINE_GRID);
            hash.eat_rounded(plan.duration(), FINE_GRID);
            hash.eat_rounded(plan.distance, POSITION_GRID);
            hash.eat_rounded(plan.bearing, FINE_GRID);
            // What the burn draws: the throttle the trip was planned under,
            // which two clients could disagree about if one wired an engine
            // the other did not see.
            hash.eat_rounded(plan.dynamics.forward_power, FINE_GRID);
            hash.eat_rounded(plan.dynamics.backward_power, FINE_GRID);
            hash.eat(plan.target.code() as u64);
            hash.eat(u64::from(plan.docks));
            hash.eat(u64::from(plan.aborting));
            hash.eat(plan.segments.len() as u64);
        }
        ShipState::Charging { star, began } => {
            hash.eat(*star as u64);
            hash.eat_rounded(*began, FINE_GRID);
        }
        ShipState::CastingOff { station, since } => {
            hash.eat(*station as u64);
            hash.eat_rounded(*since, FINE_GRID);
        }
        ShipState::Undocking {
            station,
            from,
            along,
            began,
        } => {
            hash.eat(*station as u64);
            hash.eat_rounded(from.x, POSITION_GRID);
            hash.eat_rounded(from.y, POSITION_GRID);
            hash.eat_rounded(along.x, FINE_GRID);
            hash.eat_rounded(along.y, FINE_GRID);
            hash.eat_rounded(*began, FINE_GRID);
        }
        ShipState::Docking {
            station,
            from,
            from_heading,
            hold,
            began,
        } => {
            hash.eat(*station as u64);
            hash.eat_rounded(from.x, POSITION_GRID);
            hash.eat_rounded(from.y, POSITION_GRID);
            hash.eat_rounded(*from_heading, FINE_GRID);
            hash.eat_rounded(hold.x, POSITION_GRID);
            hash.eat_rounded(hold.y, POSITION_GRID);
            hash.eat_rounded(*began, FINE_GRID);
        }
    }

    // The crew: where each of them is, and the room's clock. Not the whole
    // of the room's state — that is a great deal of `f32` arithmetic that
    // two targets will disagree on in the last bit — but a Bim that went
    // somewhere different is a different world, and this is what says so.
    hash.eat(world.aboard.count() as u64);
    for who in 0..world.aboard.count() {
        let at = world.aboard.position(who);
        hash.eat_rounded(at.x, POSITION_GRID);
        hash.eat_rounded(at.y, POSITION_GRID);
    }
    hash.eat_rounded(world.aboard.minutes(), FINE_GRID);

    // Whose side each station is on: home, and the hostile list in id
    // order. A stance is what decides whether anybody shoots, so two
    // worlds that disagree about one are already two different fights.
    // The station's people themselves — the residents' room — are not in
    // here, for the reason the crew's room is only in by its positions.
    hash.eat(world.home as u64);
    hash.eat(world.home_star as u64);
    hash.eat(world.hostile.len() as u64);
    for &station in &world.hostile {
        hash.eat(station as u64);
    }
    hash.eat(world.reinforcements as u64);
    // And which stations the machines hold, in id order, with the state
    // of each (feature 83): how many waves are left, which is aboard,
    // when the next is due and whether it has been settled or cleared.
    // It is the size of the fight, so two worlds that disagree about it
    // are two different fights — the reason `reinforcements` is in here.
    // The tier they come at and the reinforcement clock go in with them,
    // since the probes move both.
    hash.eat(world.infested.len() as u64);
    for it in &world.infested {
        hash.eat(it.station as u64);
        hash.eat(it.waves_left as u64);
        hash.eat(it.wave as u64);
        hash.eat(u64::from(it.next_wave.is_some()));
        hash.eat_rounded(it.next_wave.unwrap_or(0.0), FINE_GRID);
        hash.eat(u64::from(it.settled));
        hash.eat(u64::from(it.cleared));
    }
    hash.eat(u64::from(world.droid_tier().code()));
    hash.eat_rounded(world.droid_reinforce_minutes(), FINE_GRID);
    hash.eat(u64::from(world.droid_wave_max()));
    // The hired hands: who, what a month costs, when it is next due and
    // whether one is owed. A crew member that costs money is a different
    // crew from one that does not.
    hash.eat(world.hired.len() as u64);
    for hired in &world.hired {
        hash.eat(hired.who as u64);
        hash.eat(hired.fee);
        hash.eat_rounded(hired.due, FINE_GRID);
        hash.eat(hired.owed as u64);
        // And what was hired (feature 86): a field medic fights and
        // walks differently, so two clients that disagree about it
        // disagree about where a body is standing.
        hash.eat(hired.medic as u64);
    }

    // The armour: every piece, what it has left and where it is. A piece
    // on a body or in a pack is the room's, and its health is `f32`
    // arithmetic under fire, so it goes in to a hundredth — a hit is
    // whole points, and a last-bit disagreement is nothing beside one.
    // The next id is in for the reason `next_site` is.
    hash.eat(world.next_piece as u64);
    hash.eat(world.pieces.len() as u64);
    for piece in &world.pieces {
        hash.eat(piece.id as u64);
        hash.eat(piece.kind.code() as u64);
        hash.eat(piece.tier.code() as u64);
        hash.eat_rounded(piece.health as f64, HEALTH_GRID);
        hash.eat(piece.at.code() as u64);
        match piece.at {
            Where::Hold => {}
            Where::Pack { who, cell } => {
                hash.eat(who as u64);
                hash.eat(cell as u64);
            }
            Where::Worn { who } => hash.eat(who as u64),
        }
    }
    // The weapons in the hold by tier, the tick box, and what is on the
    // workbench and how far along: integers throughout. A crew whose
    // pistol came off the bench at tier two and one whose did not are two
    // different games.
    hash.eat(world.guns.len() as u64);
    for gun in &world.guns {
        hash.eat(gun.kind.code() as u64);
        hash.eat(gun.tier.code() as u64);
    }
    // The grids — the shelves, the cold stores, the lockers: every slot,
    // what it holds and how many, where it lies and which way round, and
    // the next id — integers throughout. Two crews whose rifles lie in
    // different places have different armouries, and the fit that
    // refuses a stow depends on it.
    for grid in &world.grids {
        eat_grid(&mut hash, grid);
    }
    hash.eat(u64::from(world.auto_upgrade));
    // The workbench: its three slots and the thing in somebody's arms —
    // each a gun by kind and tier, or a piece by id, kind, tier and health
    // to the hundredth, the way a piece anywhere else goes in — and the
    // work under way on the pair.
    for item in world
        .bench
        .slots
        .iter()
        .chain(core::iter::once(&world.bench.carrying))
    {
        match item {
            None => hash.eat(u64::MAX),
            Some(bims::combat::Item::Weapon(gun)) => {
                hash.eat(1);
                hash.eat(gun.kind.code() as u64);
                hash.eat(gun.tier.code() as u64);
            }
            Some(bims::combat::Item::Armour(piece)) => {
                hash.eat(2);
                hash.eat(piece.id as u64);
                hash.eat(piece.kind.code() as u64);
                hash.eat(piece.tier.code() as u64);
                hash.eat_rounded(piece.health as f64, HEALTH_GRID);
            }
            // Never on the bench; hashed all the same rather than skipped.
            Some(bims::combat::Item::Stack(resource)) => {
                hash.eat(3);
                hash.eat(*resource as u64);
            }
            Some(bims::combat::Item::Key(tier)) => {
                hash.eat(4);
                hash.eat(*tier as u64);
            }
        }
    }
    hash.eat(u64::from(world.bench.back));
    match world.bench.work {
        None => hash.eat(u64::MAX),
        Some(upgrade) => {
            hash.eat(upgrade.resource as u64);
            hash.eat(upgrade.to.code() as u64);
            hash.eat(upgrade.done as u64);
        }
    }

    for node in &world.discovered {
        let (kind, id) = node_key(node);
        hash.eat(kind as u64);
        hash.eat(id as u64);
    }
    for request in &world.speed_requests {
        hash.eat(request.code() as u64);
    }

    // The mining sites: every rock still standing at each, and the marks.
    // Integers throughout, so they go in whole.
    for site in &world.sites {
        eat_site(&mut hash, site);
    }

    // The construction sites: what is to be built where, and what has
    // been carried to each. Integers throughout. The next id is in too:
    // two worlds with the same sites and a different next id would hand
    // the next site different names.
    hash.eat(world.next_site as u64);
    hash.eat(world.builds.len() as u64);
    for site in &world.builds {
        hash.eat(site.id as u64);
        hash.eat(site.kind.code() as u64);
        hash.eat(site.origin.0 as u64);
        hash.eat(site.origin.1 as u64);
        hash.eat(site.rotation.code() as u64);
        for &units in site.delivered.iter().chain(site.carrying.iter()) {
            hash.eat(units as u64);
        }
    }

    // What the crew know: every node done or not, every lock open or not,
    // what the AI is on and how far it has got, and what it goes onto
    // next — a crew that knows how to build a thing and one that does not
    // are two different games, and so are two that will know different
    // things tomorrow. And which tier of key each station still has on
    // its desk: a key taken is a key nobody else can take.
    for &done in world.research.done.iter() {
        hash.eat(u64::from(done));
    }
    for &open in world.research.unlocked.iter() {
        hash.eat(u64::from(open));
    }
    hash.eat(
        world
            .research
            .current
            .map(|n| n.code() as u64)
            .unwrap_or(u64::MAX),
    );
    hash.eat_rounded(world.research.progress, FINE_GRID);
    hash.eat(world.research.queue.len() as u64);
    for node in &world.research.queue {
        hash.eat(node.code() as u64);
    }
    hash.eat(world.station_keys.len() as u64);
    for &key in &world.station_keys {
        hash.eat(u64::from(key));
    }

    // The lamps a fight has damaged: where each hangs and what it has
    // left, to a hundredth like a piece of armour — a corridor shot dark
    // on one client and lit on the other is two different fights.
    hash.eat(world.lamps.len() as u64);
    for lamp in &world.lamps {
        hash.eat(lamp.station.map(u64::from).unwrap_or(u64::MAX));
        hash.eat(lamp.tile.0 as u64);
        hash.eat(lamp.tile.1 as u64);
        hash.eat_rounded(lamp.health as f64, HEALTH_GRID);
    }

    // How long the cold store has been without power, in steps: the clock
    // the food spoils by. An integer, so it goes in whole — two worlds a
    // step apart on it lose the next crate on different steps.
    hash.eat(world.cold_store_out);

    // The raids: the schedule and the one under way — two clients are
    // raided together, by the same boarders from the same bearing, or not
    // at all. The raider tied up is its id and its seed: its hull is a
    // function of the seed, like a station's.
    hash.eat(world.raids.next as u64);
    hash.eat(world.raids.due);
    hash.eat(u64::from(world.raids.left_home));
    match &world.raids.state {
        crate::raid::Raid::Quiet => hash.eat(0),
        crate::raid::Raid::Closing {
            n,
            boarders,
            from,
            at,
            began,
            arrives,
        } => {
            hash.eat(1);
            hash.eat(*n as u64);
            hash.eat(*boarders as u64);
            hash.eat_rounded(from.x, POSITION_GRID);
            hash.eat_rounded(from.y, POSITION_GRID);
            hash.eat_rounded(at.x, POSITION_GRID);
            hash.eat_rounded(at.y, POSITION_GRID);
            hash.eat_rounded(*began, FINE_GRID);
            hash.eat_rounded(*arrives, FINE_GRID);
        }
        crate::raid::Raid::Docked {
            station,
            boarders,
            breached,
            repelled,
        } => {
            hash.eat(2);
            hash.eat(station.id as u64);
            hash.eat(station.map_seed);
            hash.eat_rounded(station.anchor.x, POSITION_GRID);
            hash.eat_rounded(station.anchor.y, POSITION_GRID);
            hash.eat(*boarders as u64);
            hash.eat(u64::from(*breached));
            hash.eat(u64::from(*repelled));
        }
    }
    // And whether the run is over.
    hash.eat(u64::from(world.lost));

    // Every enemy's shelf the crew have been alongside, as they have left
    // it: whose, its size, and its grid the way the hold's grids go in —
    // two crews who plundered a raider differently have different worlds.
    hash.eat(world.plunder.len() as u64);
    for plunder in &world.plunder {
        hash.eat(plunder.station as u64);
        hash.eat(plunder.capacity as u64);
        eat_grid(&mut hash, &plunder.grid);
    }

    // What every station has lost to the crew, and every system the ship
    // has left as it was left — integers throughout, the sites and the
    // shelves the way they go in above: a crew that emptied a station and
    // one that did not are two different galaxies.
    eat_losses(&mut hash, &world.losses);
    eat_graves(&mut hash, &world.graves);
    eat_nodes(&mut hash, &world.visited);
    hash.eat(world.memories.len() as u64);
    for memory in &world.memories {
        hash.eat(memory.star as u64);
        hash.eat(memory.hostile.len() as u64);
        for &station in &memory.hostile {
            hash.eat(station as u64);
        }
        hash.eat(memory.reinforcements as u64);
        hash.eat(memory.station_keys.len() as u64);
        for &key in &memory.station_keys {
            hash.eat(u64::from(key));
        }
        hash.eat(memory.sites.len() as u64);
        for site in &memory.sites {
            eat_site(&mut hash, site);
        }
        hash.eat(memory.plunder.len() as u64);
        for plunder in &memory.plunder {
            hash.eat(plunder.station as u64);
            hash.eat(plunder.capacity as u64);
            eat_grid(&mut hash, &plunder.grid);
        }
        hash.eat(memory.lamps.len() as u64);
        for lamp in &memory.lamps {
            hash.eat(lamp.station.map(u64::from).unwrap_or(u64::MAX));
            hash.eat(lamp.tile.0 as u64);
            hash.eat(lamp.tile.1 as u64);
            hash.eat_rounded(lamp.health as f64, HEALTH_GRID);
        }
        eat_nodes(&mut hash, &memory.discovered);
        eat_losses(&mut hash, &memory.losses);
        eat_graves(&mut hash, &memory.graves);
        eat_nodes(&mut hash, &memory.visited);
    }

    // The classes and what each crew member has learnt, whether the
    // berth was ever left, every deployable standing and the kits taken
    // back (feature 74): a pick made is a different crew, a sentry laid a
    // different fight. Integers throughout but a deployable's health, to
    // a hundredth like a piece of armour's.
    hash.eat(world.classes.len() as u64);
    for class in &world.classes {
        hash.eat(class.code() as u64);
    }
    hash.eat(world.progress.len() as u64);
    for progress in &world.progress {
        hash.eat(progress.xp as u64);
        hash.eat(progress.picks.len() as u64);
        for &(level, side) in &progress.picks {
            hash.eat(u64::from(level));
            hash.eat(side.code() as u64);
        }
    }
    hash.eat(u64::from(world.undocked_once));
    hash.eat(world.deployables.len() as u64);
    for d in &world.deployables {
        hash.eat(d.id as u64);
        hash.eat(d.kind.code() as u64);
        hash.eat(d.owner_slot as u64);
        hash.eat(d.deck.code() as u64);
        hash.eat(d.tile.0 as u64);
        hash.eat(d.tile.1 as u64);
        hash.eat_rounded(d.health as f64, HEALTH_GRID);
    }
    hash.eat(world.next_deployable as u64);
    hash.eat(world.reused_kits.len() as u64);
    for &n in &world.reused_kits {
        hash.eat(n as u64);
    }
    // And when each engineer's next charge of each kit is due, on the
    // clock's grid (feature 88): a charge in the pack is a sentry that can
    // be laid and one still cooling down is not.
    hash.eat(world.kit_timers.len() as u64);
    for timers in &world.kit_timers {
        for began in timers {
            match began {
                Some(minutes) => {
                    hash.eat(1);
                    hash.eat_rounded(*minutes, FINE_GRID);
                }
                None => hash.eat(0),
            }
        }
    }

    // The soldiers (feature 75): who is braced and each one's *rampage*
    // stacks — the room's, read off it like the positions, integers — and
    // when each crew member last threw a grenade, on the clock's grid. A
    // soldier braced is a different fight from one standing easy.
    let crew = world.aboard.crew_count() as usize;
    hash.eat(crew as u64);
    for who in 0..crew {
        hash.eat(u64::from(world.aboard.room.is_braced(who)));
        hash.eat(world.aboard.room.rampage(who) as u64);
    }
    hash.eat(world.last_throw.len() as u64);
    for last in &world.last_throw {
        match last {
            Some(minutes) => {
                hash.eat(1);
                hash.eat_rounded(*minutes, FINE_GRID);
            }
            None => hash.eat(0),
        }
    }

    // The medics (feature 76): who each beam holds, each surge's charge
    // on the clock's grid, the field surgery this fight, and — the
    // room's, read off it like the brace — the seconds each body's surge
    // has left, to a hundredth. A patient held is a different fight
    // from one bleeding.
    hash.eat(world.medics.len() as u64);
    for medic in &world.medics {
        hash.eat(medic.patients.len() as u64);
        for &p in &medic.patients {
            hash.eat(p as u64);
        }
        hash.eat_rounded(medic.charge, FINE_GRID);
        hash.eat(u64::from(medic.field_surgery_used));
    }
    for who in 0..crew {
        hash.eat(u64::from(world.aboard.room.is_surging(who)));
        hash.eat_rounded(world.aboard.room.surge_left(who) as f64, HEALTH_GRID);
        hash.eat(u64::from(world.aboard.room.surge_closing(who)));
    }

    // The tanks (feature 77): when each last taunted, on the clock's
    // grid, and — the room's, read off it like the brace — who stands
    // as a wall and how many enemy hits each body has taken since its
    // last point of experience. A wall up is a different fight.
    hash.eat(world.tanks.len() as u64);
    for tank in &world.tanks {
        match tank.last_taunt {
            Some(minutes) => {
                hash.eat(1);
                hash.eat_rounded(minutes, FINE_GRID);
            }
            None => hash.eat(0),
        }
    }
    for who in 0..crew {
        hash.eat(u64::from(world.aboard.room.is_bulwark(who)));
        hash.eat(world.aboard.room.hits_taken(who) as u64);
    }

    // The commanders (feature 78): when each last rallied, on the
    // clock's grid, and the one squad order the crew are under — whose
    // it is, what it is and who is in it. A squad ordered somewhere is
    // a different fight. The aura is not here: it is worked out afresh
    // every step from where the commanders stand.
    hash.eat(world.commanders.len() as u64);
    for commander in &world.commanders {
        match commander.last_rally {
            Some(minutes) => {
                hash.eat(1);
                hash.eat_rounded(minutes, FINE_GRID);
            }
            None => hash.eat(0),
        }
    }
    match &world.squad {
        None => hash.eat(0),
        Some(order) => {
            hash.eat(1);
            hash.eat(order.by_slot as u64);
            hash.eat(u64::from(order.kind.code()));
            match &order.kind {
                crate::commander::SquadKind::Attack { enemies } => {
                    hash.eat(enemies.len() as u64);
                    for &enemy in enemies {
                        hash.eat(enemy as u64);
                    }
                }
                crate::commander::SquadKind::FallBack { tile } => {
                    hash.eat(tile.0 as i64 as u64);
                    hash.eat(tile.1 as i64 as u64);
                }
                crate::commander::SquadKind::StandGround => {}
            }
            hash.eat(order.members.len() as u64);
            for &who in &order.members {
                hash.eat(who as u64);
            }
        }
    }
    // And every player's standing order to the bots (feature 84,
    // `crate::orders`): it moves bodies, so two clients that disagree
    // about it disagree about where the crew are standing.
    hash.eat(world.standing.len() as u64);
    for order in &world.standing {
        hash.eat(u64::from(order.code()));
        if let crate::orders::Standing::Attack { tile } = order {
            hash.eat(tile.0 as i64 as u64);
            hash.eat(tile.1 as i64 as u64);
        }
    }
    // And the seconds each body has been dying with an enemy about,
    // which is what a commander's aura buys it (`bims::bim::Bim::fear`).
    for who in 0..crew {
        hash.eat_rounded(world.aboard.room.fear(who) as f64, HEALTH_GRID);
    }
    // And who has whom in their arms (feature 86): a carry stands one
    // body where another walks and holds both their fire, so it is as
    // much a position as a walk is. `u64::MAX` for empty arms, the way
    // an empty slot goes in everywhere else.
    for who in 0..crew {
        match world.aboard.room.carrying(who as usize) {
            None => hash.eat(u64::MAX),
            Some(patient) => hash.eat(patient as u64),
        }
    }

    hash.0
}

/// A mining site whole: its belt, every rock still standing and the
/// marks. Integers throughout.
fn eat_site(hash: &mut Fnv, site: &crate::mining::MiningSite) {
    hash.eat(site.belt as u64);
    hash.eat(site.tiles.len() as u64);
    for tile in &site.tiles {
        hash.eat(tile.x as i64 as u64);
        hash.eat(tile.y as i64 as u64);
        hash.eat(tile.kind.code() as u64);
    }
    hash.eat(site.marked.len() as u64);
    for &(x, y) in &site.marked {
        hash.eat(x as i64 as u64);
        hash.eat(y as i64 as u64);
    }
}

/// What the stations have lost, station by station.
fn eat_losses(hash: &mut Fnv, losses: &[crate::memory::Losses]) {
    hash.eat(losses.len() as u64);
    for loss in losses {
        hash.eat(loss.station as u64);
        hash.eat(loss.dead as u64);
        hash.eat(loss.mercenaries as u64);
    }
}

/// A list of nodes — a chart, or where the crew have been — whole.
fn eat_nodes(hash: &mut Fnv, nodes: &[worldgen::Node]) {
    hash.eat(nodes.len() as u64);
    for node in nodes {
        let (kind, id) = node_key(node);
        hash.eat(kind as u64);
        hash.eat(id as u64);
    }
}

/// The bodies lying on the stations' decks (feature 85): whose deck,
/// where on it to a thousandth like any position, and what is still on
/// each — a body is loot until somebody takes it, so what is on it is
/// worth as much agreement as what is on a shelf. What it *looked* like
/// is not in here, for the reason no other body's look is.
fn eat_graves(hash: &mut Fnv, graves: &[crate::memory::Grave]) {
    hash.eat(graves.len() as u64);
    for grave in graves {
        hash.eat(grave.station as u64);
        hash.eat_rounded(grave.x, POSITION_GRID);
        hash.eat_rounded(grave.y, POSITION_GRID);
        hash.eat(u64::from(grave.hired));
        eat_gear(hash, &grave.gear);
    }
}

/// What is on a body: what it wears, the gun in its hand and its pack,
/// cell by cell — integers throughout but a piece's health, to a
/// hundredth as a piece of armour's goes in anywhere else.
fn eat_gear(hash: &mut Fnv, gear: &bims::combat::Gear) {
    for piece in [gear.head, gear.body, gear.legs] {
        match piece {
            None => hash.eat(u64::MAX),
            Some(piece) => {
                hash.eat(piece.kind.code() as u64);
                hash.eat(piece.tier.code() as u64);
                hash.eat_rounded(piece.health as f64, HEALTH_GRID);
            }
        }
    }
    match gear.weapon {
        None => hash.eat(u64::MAX),
        Some(weapon) => {
            hash.eat(weapon.kind.code() as u64);
            hash.eat(weapon.tier.code() as u64);
        }
    }
    for (cell, item) in gear.pack.iter().enumerate() {
        let Some(item) = item else { continue };
        hash.eat(cell as u64);
        let (kind, a, b) = item_codes(item);
        hash.eat(kind);
        hash.eat(a);
        hash.eat(b);
        hash.eat(u64::from(gear.turned[cell]));
        // How many are in the cell's stack (feature 87): five dressings
        // in a box are not one, and `units` reads a count never set as
        // the one it is.
        hash.eat(gear.units(cell) as u64);
    }
}

/// One thing in a pack as three numbers, the way `grid::Kept::codes`
/// gives a slot's: which of the four, then what.
fn item_codes(item: &bims::combat::Item) -> (u64, u64, u64) {
    use bims::combat::Item;
    match item {
        Item::Armour(piece) => (0, piece.kind.code() as u64, piece.tier.code() as u64),
        Item::Weapon(weapon) => (1, weapon.kind.code() as u64, weapon.tier.code() as u64),
        Item::Stack(code) => (2, *code as u64, 0),
        Item::Key(tier) => (3, u64::from(*tier), 0),
    }
}

/// A grid — one of the hold's, or an enemy's shelf — whole: every slot,
/// what it holds and how many, where it lies and which way round, and
/// the next id — integers throughout.
fn eat_grid(hash: &mut Fnv, grid: &crate::grid::Grid) {
    hash.eat(grid.next as u64);
    hash.eat(grid.slots.len() as u64);
    for slot in &grid.slots {
        hash.eat(slot.id as u64);
        let (kind, a, b) = slot.kept.codes();
        hash.eat(kind);
        hash.eat(a);
        hash.eat(b);
        hash.eat(slot.count as u64);
        hash.eat(slot.x as u64);
        hash.eat(slot.y as u64);
        hash.eat(u64::from(slot.turned));
    }
}
