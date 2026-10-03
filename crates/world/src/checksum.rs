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
    // And every player's own money.
    hash.eat(world.wallets.len() as u64);
    for &wallet in &world.wallets {
        hash.eat(wallet);
    }

    let ship = &world.ship;
    hash.eat(world.design_hash());
    hash.eat(ship.crew_count as u64);
    hash.eat_rounded(ship.anchor.x, POSITION_GRID);
    hash.eat_rounded(ship.anchor.y, POSITION_GRID);
    hash.eat_rounded(ship.heading, FINE_GRID);
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

    // Whose side each station is on: home, and the stations the machines
    // hold. A stance is what decides whether anybody shoots, so two
    // worlds that disagree about one are already two different fights.
    // The station's people themselves — the residents' room — are not in
    // here, for the reason the crew's room is only in by its positions.
    hash.eat(world.home as u64);
    hash.eat(world.home_star as u64);
    // Which stations the machines hold, in id order, with the state of
    // each (feature 83): how many waves are left, which is aboard, when
    // the next is due and whether it has been settled or cleared. It is
    // the size of the fight, so two worlds that disagree about it are two
    // different fights. The tier they come at and the reinforcement clock
    // go in with them, since the probes move both.
    hash.eat(world.infested.len() as u64);
    for it in &world.infested {
        hash.eat(it.station as u64);
        hash.eat(it.waves_left as u64);
        hash.eat(it.wave as u64);
        hash.eat(u64::from(it.next_wave.is_some()));
        hash.eat(it.next_wave.unwrap_or(0));
        hash.eat(u64::from(it.settled));
        hash.eat(u64::from(it.cleared));
        eat_heart(&mut hash, it.heart.as_ref());
        eat_manufacturers(&mut hash, it);
    }
    hash.eat(u64::from(world.droid_tier().code()));
    hash.eat(world.droid_reinforce_steps());
    // The probes' forced wave size, nought for none (task 132: it was
    // the cap, sixteen unforced, until the cap went).
    hash.eat(u64::from(world.droid_wave_forced().unwrap_or(0)));
    // And the crisis (feature 92): the star the machines began at and the
    // day the first one turns. The hop table is *not* in here — it is
    // derived from the origin and the galaxy, so two clients that agree
    // about those two numbers agree about every star in the galaxy.
    hash.eat(u64::from(world.droid_origin()));
    hash.eat(u64::from(world.crisis_first_day()));
    // And the towns the crew are defending (feature 94): which wave is on
    // the ground, how many are still to come, how long until the next
    // lands, how many machines are standing, and whether it was held or
    // lost. It is a fight the same way an infestation is, and the wait
    // before the first wave goes in beside it, the way the reinforcement
    // clock goes in beside the machines' own.
    eat_defenses(&mut hash, world.defenses());
    hash.eat(world.defense_delay_steps());
    // And the towns they held: a held town stays friendly for good, so
    // two worlds that disagree about one disagree about whether a
    // settlement inside the infection still trades.
    eat_held_towns(&mut hash, world.held_towns());
    // The hired hands: who, what a month costs, when it is next due and
    // whether one is owed. A crew member that costs money is a different
    // crew from one that does not.
    hash.eat(world.hired.len() as u64);
    for hired in &world.hired {
        hash.eat(hired.who as u64);
        hash.eat(u64::from(hired.by));
        hash.eat(hired.fee);
        hash.eat_rounded(hired.due, FINE_GRID);
        hash.eat(hired.owed as u64);
        // And what was hired (feature 86): a field medic fights and
        // walks differently, so two clients that disagree about it
        // disagree about where a body is standing.
        hash.eat(hired.medic as u64);
    }

    // The holdings (task 113): the armory — every thing's id, what it is
    // and, for a piece, what it has left to a hundredth, the way a piece
    // anywhere else goes in — the keys, the offers standing and the next
    // id, for the reason `next_site` is in.
    let holdings = &world.holdings;
    hash.eat(holdings.next_id as u64);
    hash.eat(holdings.keys as u64);
    hash.eat(holdings.armory.len() as u64);
    for stored in &holdings.armory {
        hash.eat(stored.id as u64);
        eat_item(&mut hash, &stored.item);
    }
    hash.eat(holdings.offers.len() as u64);
    for offer in &holdings.offers {
        hash.eat(offer.from as u64);
        hash.eat(offer.slot.code() as u64);
        hash.eat(offer.to as u64);
    }
    // And every crew member's loadout: what it wears and holds and its
    // charges. A crew whose rifle is on James and one whose is on Kate
    // are two different games.
    for who in 0..world.aboard.room.crew_count() as usize {
        eat_gear(&mut hash, &world.aboard.room.gear(who));
    }

    for node in &world.discovered {
        let (kind, id) = node_key(node);
        hash.eat(kind as u64);
        hash.eat(id as u64);
    }
    for request in &world.speed_requests {
        hash.eat(request.code() as u64);
    }

    // The construction sites: what is to be built where. Integers
    // throughout, and nothing is carried to one since the money rework
    // (feature 95). The next id is in too: two worlds with the same sites
    // and a different next id would hand the next site different names.
    hash.eat(world.next_site as u64);
    hash.eat(world.builds.len() as u64);
    for site in &world.builds {
        hash.eat(site.id as u64);
        hash.eat(site.kind.code() as u64);
        hash.eat(site.origin.0 as u64);
        hash.eat(site.origin.1 as u64);
        hash.eat(site.rotation.code() as u64);
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

    // And whether the run is over.
    hash.eat(u64::from(world.lost));

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
        // Which of that system's stations the machines hold, the same way
        // the current system's go in above — a station the crew cleared is
        // in here with `cleared` set, and that is what the crisis reads.
        hash.eat(memory.infested.len() as u64);
        for it in &memory.infested {
            hash.eat(it.station as u64);
            hash.eat(it.waves_left as u64);
            hash.eat(it.wave as u64);
            hash.eat(u64::from(it.next_wave.is_some()));
            hash.eat(it.next_wave.unwrap_or(0));
            hash.eat(u64::from(it.settled));
            hash.eat(u64::from(it.cleared));
            eat_heart(&mut hash, it.heart.as_ref());
            eat_manufacturers(&mut hash, it);
        }
        // And that system's defences and held towns, which are its own
        // since station ids are: only where it has any, so a memory of a
        // system nobody defended hashes what it always did.
        if !memory.defenses.is_empty() || !memory.held_towns.is_empty() {
            hash.eat(0x_4445_4645);
            eat_defenses(&mut hash, &memory.defenses);
            eat_held_towns(&mut hash, &memory.held_towns);
        }
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
        // A nought where the talents picked were counted: they went with
        // task 139, and a crew that had picked none hashes as it did.
        hash.eat(0);
        // And the ranks bought of a ranked kit (task 124), eaten only where
        // there is one: a crew with none hashes as it always did.
        if progress.ranks != [0; crate::class::SLOTS] {
            for rank in progress.ranks {
                hash.eat(u64::from(rank));
            }
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
    // Every crew member's charges held (task 127): counters, by code.
    hash.eat(world.charges_held.len() as u64);
    for held in &world.charges_held {
        for &n in held {
            hash.eat(n as u64);
        }
    }
    // The engineers' ultimate cooldowns (task 127), only where one runs.
    for (who, e) in world.engineers.iter().enumerate() {
        if let Some(laid) = e.sentry_laid {
            hash.eat(who as u64);
            hash.eat_rounded(laid, FINE_GRID);
        }
    }
    // And every machine an EMP has stunned (task 127), only where one is.
    if let Some(residents) = &world.residents {
        let room = &residents.aboard.room;
        for i in 0..room.droid_count() as usize {
            if let Some(d) = room.droid(i).filter(|d| d.is_stunned()) {
                hash.eat(i as u64);
                hash.eat_rounded(d.stunned as f64, FINE_GRID);
                hash.eat(u64::from(d.exposed));
            }
        }
        // And what each Guardian's plate has stopped, only where it has.
        for i in 0..room.droid_count() as usize {
            if let Some(d) = room.droid(i).filter(|d| d.plate_taken > 0.0) {
                hash.eat(i as u64);
                hash.eat_rounded(d.plate_taken as f64, FINE_GRID);
            }
        }
    }
    // And when each crew member's next charge of each kind is due, on
    // the clock's grid (features 88 and 90): a charge in the pack is a
    // sentry that can be laid or a grenade that can be thrown, and one
    // still cooling down is not.
    hash.eat(world.charge_timers.len() as u64);
    for timers in &world.charge_timers {
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

    // The soldiers (feature 75): who is planted — a Stun Shot charging
    // since October 2026, the brace before it — the room's, read off it
    // like the positions — and a nought where the *rampage* stacks were
    // (the talent went with task 124), so the layout is what it was. A
    // soldier planted is a different fight from one walking. When
    // its next grenade is due went in with the charges above (feature
    // 90), where the engineer's kits' cooldowns are.
    let crew = world.aboard.crew_count() as usize;
    hash.eat(crew as u64);
    for who in 0..crew {
        hash.eat(u64::from(world.aboard.room.is_braced(who)));
        hash.eat(0);
        // And the shots it has fired, which an *Overcharge Cell* counts
        // (feature 106).
        hash.eat(u64::from(world.aboard.room.shots(who)));
    }

    // The medics (feature 76): who each beam holds, a nought where the
    // surge's charge was until the surge went (task 130), and — the
    // room's, read off it like the brace — the seconds each body's surge
    // has left, to a hundredth (nothing sets it since the relics were
    // rebuilt in October 2026, and it reads nought).
    hash.eat(world.medics.len() as u64);
    for medic in &world.medics {
        hash.eat(medic.patients.len() as u64);
        for &p in &medic.patients {
            hash.eat(p as u64);
        }
        hash.eat_rounded(0.0, FINE_GRID);
    }
    for who in 0..crew {
        hash.eat(u64::from(world.aboard.room.is_surging(who)));
        hash.eat_rounded(world.aboard.room.surge_left(who) as f64, HEALTH_GRID);
    }
    // *Lifeline*'s shields (task 142), only where one stands: the hit
    // points each still takes and its seconds, to a hundredth.
    for who in 0..crew {
        let hp = world.aboard.room.shield_hp(who);
        if hp <= 0.0 {
            continue;
        }
        hash.eat(who as u64);
        hash.eat_rounded(hp as f64, HEALTH_GRID);
        hash.eat_rounded(world.aboard.room.shield_left(who) as f64, HEALTH_GRID);
    }
    // And the medic's ranked kit (task 130, reworked by task 153), only
    // where there is any: each medic's last drone, the drone in the air
    // (where, whom it is over, when it goes) and the circle on with its
    // last burn — what a step heals, drains and burns by.
    for (who, medic) in world.medics.iter().enumerate() {
        if medic.last_drone.is_none() && medic.drone.is_none() && !medic.circle {
            continue;
        }
        hash.eat(who as u64);
        match medic.last_drone {
            Some(minutes) => {
                hash.eat(1);
                hash.eat_rounded(minutes, FINE_GRID);
            }
            None => hash.eat(0),
        }
        match medic.drone {
            Some(drone) => {
                hash.eat(1);
                hash.eat_rounded(drone.x as f64, FINE_GRID);
                hash.eat_rounded(drone.y as f64, FINE_GRID);
                hash.eat(drone.patient.map_or(u64::MAX, u64::from));
                hash.eat_rounded(drone.until, FINE_GRID);
            }
            None => hash.eat(0),
        }
        hash.eat(u64::from(medic.circle));
        hash.eat_rounded(medic.last_burn.unwrap_or(-1.0), FINE_GRID);
    }

    // The tanks (feature 77; task 155): when each last raised his Reflect
    // Barrier, on the clock's grid, where the taunt's minute was, and —
    // only where any is set — the barrier's window, the Bastion's
    // cooldown and haste and the Riot Shield's hit points spent, its last
    // hit and whether it broke; then who holds a shield up, where who
    // stood as a wall was, and how many enemy hits each body has taken.
    // A world with no tank in it hashes what it did.
    hash.eat(world.tanks.len() as u64);
    for tank in &world.tanks {
        match tank.last_reflect {
            Some(minutes) => {
                hash.eat(1);
                hash.eat_rounded(minutes, FINE_GRID);
            }
            None => hash.eat(0),
        }
        if tank.reflect != crate::tank::Window::default()
            || tank.last_bastion.is_some()
            || tank.hasted != crate::tank::Window::default()
            || tank.shield_spent > 0.0
            || tank.shield_struck.is_some()
            || tank.shield_broken
        {
            for window in [tank.reflect, tank.hasted] {
                hash.eat_rounded(window.began, FINE_GRID);
                hash.eat_rounded(window.until, FINE_GRID);
            }
            hash.eat_rounded(tank.last_bastion.unwrap_or(-1.0), FINE_GRID);
            hash.eat_rounded(tank.shield_spent as f64, HEALTH_GRID);
            hash.eat_rounded(tank.shield_struck.unwrap_or(-1.0), FINE_GRID);
            hash.eat(u64::from(tank.shield_broken));
        }
    }
    for who in 0..crew {
        let up = world.tanks.get(who).is_some_and(|t| t.shield_up);
        hash.eat(u64::from(up));
        hash.eat(world.aboard.room.hits_taken(who) as u64);
    }

    // The commanders (feature 78): when each last rallied, on the
    // clock's grid. The aura is not here: it is worked out afresh
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
    // And the commander's ranked kit (task 129), only where there is any:
    // whom his last Rally reached, his last Battle Cry and whom it
    // reached — whose fire rate and whose hits a step reads — and the
    // reinforcements of the mission.
    for (who, commander) in world.commanders.iter().enumerate() {
        if commander.rallied.is_empty() && commander.last_battle_cry.is_none() {
            continue;
        }
        hash.eat(who as u64);
        hash.eat(commander.rallied.len() as u64);
        for &r in &commander.rallied {
            hash.eat(r as u64);
        }
        match commander.last_battle_cry {
            Some(minutes) => {
                hash.eat(1);
                hash.eat_rounded(minutes, FINE_GRID);
            }
            None => hash.eat(0),
        }
        hash.eat(commander.cried.len() as u64);
        for &c in &commander.cried {
            hash.eat(c as u64);
        }
    }
    // When each commander last called reinforcements in, only where one
    // has this mission.
    for (who, commander) in world.commanders.iter().enumerate() {
        if let Some(minutes) = commander.last_reinforcement {
            hash.eat(who as u64);
            hash.eat_rounded(minutes, FINE_GRID);
        }
    }
    if !world.reinforcements.is_empty() {
        hash.eat(world.reinforcements.len() as u64);
        for r in &world.reinforcements {
            hash.eat(r.who as u64);
            hash.eat(r.by as u64);
            // A Medivac's medic marked, only where it is one.
            if r.medic {
                hash.eat(1);
            }
        }
    }
    // When each commander last called a medic in (his C), only where one
    // has this mission.
    for (who, commander) in world.commanders.iter().enumerate() {
        if let Some(minutes) = commander.last_medivac {
            hash.eat(who as u64);
            hash.eat_rounded(minutes, FINE_GRID);
        }
    }
    // A nought where the squad order was hashed, so no number moved when
    // the commander's squad orders were removed.
    hash.eat(0);
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
    // And each body's downed countdown and the slow a downing left
    // (task 120): what decides when a body dies and how fast it walks.
    for who in 0..crew {
        let left = world.aboard.room.down_left(who as usize).unwrap_or(-1.0);
        hash.eat_rounded(left as f64, HEALTH_GRID);
        hash.eat(u64::from(world.aboard.room.was_downed(who as usize)));
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

    // What the run has switched off (feature 102): two clients that
    // disagreed about it would be playing two games.
    hash.eat(u64::from(world.shipyard_enabled()));

    // The run (feature 103), whole: where it stands, the mission clock,
    // the bounty waiting on the site, the vote on the table, who is going
    // home, the departure check, who is out and since when — every one of
    // them something two clients must agree on or they part at the next
    // trip.
    let run = &world.run;
    hash.eat(u64::from(run.phase.code()));
    hash.eat(run.mission_steps);
    hash.eat(u64::from(run.missions));
    hash.eat(run.site.map_or(u64::MAX, u64::from));
    hash.eat(u64::from(run.snapped));
    hash.eat(u64::from(run.snapshot.is_some()));
    if let Some(snapshot) = &run.snapshot {
        hash.eat(u64::from(snapshot.station));
        hash.eat(u64::from(snapshot.infestation.is_some()));
        hash.eat(u64::from(snapshot.defense.is_some()));
        hash.eat(snapshot.graves.len() as u64);
        hash.eat(snapshot.lamps.len() as u64);
    }
    hash.eat(run.pending_bounty);
    match &run.proposal {
        None => hash.eat(u64::MAX),
        Some(p) => {
            hash.eat(u64::from(p.site.star));
            hash.eat(u64::from(p.site.station));
            hash.eat(u64::from(p.by));
            for &yes in &p.accepted {
                hash.eat(u64::from(yes));
            }
        }
    }
    hash.eat(run.returning.len() as u64);
    for &r in &run.returning {
        hash.eat(u64::from(r));
    }
    hash.eat(u64::from(run.recalled));
    match &run.departure {
        None => hash.eat(0),
        Some(crate::run::Departure::Asking { behind, answers }) => {
            hash.eat(1);
            for &who in behind {
                hash.eat(u64::from(who));
            }
            for answer in answers {
                hash.eat(answer.map_or(2, u64::from));
            }
        }
        Some(crate::run::Departure::Declined { behind }) => {
            hash.eat(3);
            for &who in behind {
                hash.eat(u64::from(who));
            }
        }
    }
    for &c in &run.connected {
        hash.eat(u64::from(c));
    }
    hash.eat(run.fallen.len() as u64);
    for fallen in &run.fallen {
        hash.eat(u64::from(fallen.slot));
        hash.eat(fallen.order);
    }
    hash.eat(run.deaths);

    // The relics (feature 106), whole: what the crew hold, the choice on
    // the table and who has said yes, and how many offers came before —
    // and the clear and the win. A relic on one client and not the other
    // is a different crew.
    let relics = &run.relics;
    let eat_list = |hash: &mut Fnv, list: &[crate::relic::Relic]| {
        hash.eat(list.len() as u64);
        for r in list {
            hash.eat(u64::from(r.code()));
        }
    };
    eat_list(&mut hash, &relics.held);
    match &relics.choice {
        None => hash.eat(u64::MAX),
        Some(choice) => {
            eat_list(&mut hash, &choice.options);
            match &choice.proposal {
                None => hash.eat(u64::MAX),
                Some(p) => {
                    hash.eat(p.relic.map_or(u64::MAX, |r| u64::from(r.code())));
                    hash.eat(u64::from(p.by));
                    for &yes in &p.accepted {
                        hash.eat(u64::from(yes));
                    }
                }
            }
        }
    }
    hash.eat(u64::from(relics.offers));
    hash.eat(u64::from(run.fought));
    hash.eat(u64::from(run.cleared_here));
    hash.eat(u64::from(run.won));
    hash.eat(u64::from(run.win_on_clear));
    // The run in numbers (feature 108): what the victory screen says.
    hash.eat(u64::from(run.machines_destroyed));
    hash.eat(u64::from(run.sites_cleared));
    hash.eat(u64::from(run.systems_liberated));
    // The ready check: eaten only while its switch is on, so a run
    // without it — every test's, the reference run's — hashes as it
    // always did.
    if run.ready_check {
        hash.eat(u64::from(run.briefing));
        hash.eat(run.ready.len() as u64);
        for &r in &run.ready {
            hash.eat(u64::from(r));
        }
    }
    // The traders (task 114): every one met, whose it is, what is left on
    // its shelf.
    // Eaten only where there is any, so a run that has met none hashes as
    // it always did.
    if !run.traders.is_empty() {
        hash.eat(run.traders.len() as u64);
        for trader in &run.traders {
            hash.eat(u64::from(trader.site.star));
            hash.eat(u64::from(trader.site.station));
            hash.eat(u64::from(trader.owner));
            hash.eat(trader.shelf.len() as u64);
            for item in &trader.shelf {
                match item {
                    None => hash.eat(u64::MAX),
                    Some(item) => {
                        hash.eat(item.resource as u64);
                        hash.eat(u64::from(item.tier.code()));
                    }
                }
            }
            // The items sold this visit (October 2026), only where any are.
            if !trader.items_sold.is_empty() {
                hash.eat(0x_534F_4C44);
                for &kind in &trader.items_sold {
                    hash.eat(u64::from(kind));
                }
            }
        }
    }
    // The fight chosen in each system (task 135). Eaten only where there
    // is any, so a run that has begun no mission since the start hashes
    // as it always did.
    if !run.chosen.is_empty() {
        hash.eat(run.chosen.len() as u64);
        for site in &run.chosen {
            hash.eat(u64::from(site.star));
            hash.eat(u64::from(site.station));
        }
    }

    // The soldiers' ranked kit (task 124): every Rampage's clock, and
    // Weak Spot's own stream. Eaten only where there is any — a Rampage
    // gone on, the stream drawn on — so a run with neither hashes as it
    // always did.
    if world.soldiers.iter().any(|s| s.began.is_some()) {
        hash.eat(world.soldiers.len() as u64);
        for s in &world.soldiers {
            match s.began {
                Some(b) => hash.eat_rounded(b, FINE_GRID),
                None => hash.eat(u64::MAX),
            }
            hash.eat_rounded(s.until, FINE_GRID);
            hash.eat_rounded(s.extended, FINE_GRID);
        }
    }
    // And the Stun Shot (October 2026): every charge running and when
    // the last shot was fired, eaten only where there is any.
    if world
        .soldiers
        .iter()
        .any(|s| s.charging.is_some() || s.last_shot.is_some())
    {
        hash.eat(world.soldiers.len() as u64);
        for s in &world.soldiers {
            match s.charging {
                Some(c) => {
                    hash.eat_rounded(c.until, FINE_GRID);
                    hash.eat(c.tile.0 as i64 as u64);
                    hash.eat(c.tile.1 as i64 as u64);
                }
                None => hash.eat(u64::MAX),
            }
            match s.last_shot {
                Some(f) => hash.eat_rounded(f, FINE_GRID),
                None => hash.eat(u64::MAX),
            }
        }
    }
    // A throw walked out to (`Command::ThrowAt`): who, which, the tile and
    // the spot. Eaten only where there is any, so a run that never walks
    // to throw hashes as it always did.
    if !world.throws.is_empty() {
        hash.eat(world.throws.len() as u64);
        for p in &world.throws {
            hash.eat(u64::from(p.who));
            hash.eat(u64::from(p.satchel));
            for n in [p.tile.0, p.tile.1, p.stand.0, p.stand.1] {
                hash.eat(n as u64);
            }
        }
    }
    if world.crit_rng != world.fresh_crit_rng() {
        let (state, inc) = world.crit_rng.state();
        hash.eat(state);
        hash.eat(inc);
    }

    hash.0
}

/// Whether a held site is the Manufacturers' (feature 109): eaten only
/// where it is, so a machines' site hashes as it always did.
/// The towns under attack (feature 94), in station order: the schedule of
/// each fight and how it ended.
fn eat_defenses(hash: &mut Fnv, defenses: &[crate::defense::Defense]) {
    hash.eat(defenses.len() as u64);
    for d in defenses {
        hash.eat(u64::from(d.station));
        hash.eat(u64::from(d.waves_left));
        hash.eat(u64::from(d.wave));
        hash.eat(u64::from(d.next_in.is_some()));
        hash.eat(d.next_in.unwrap_or(0));
        hash.eat(u64::from(d.standing));
        hash.eat(u64::from(d.settled));
        hash.eat(u64::from(d.won));
        hash.eat(u64::from(d.lost));
    }
}

/// The towns the crew held, in station order.
fn eat_held_towns(hash: &mut Fnv, towns: &[u32]) {
    hash.eat(towns.len() as u64);
    for &station in towns {
        hash.eat(u64::from(station));
    }
}

fn eat_manufacturers(hash: &mut Fnv, it: &crate::droid::Infestation) {
    if it.manufacturers {
        hash.eat(0x_4D41_4E55);
    }
}

/// The Machine Heart's fight on its fortress's infestation (feature 108):
/// nothing at all at every other station, so no other fight's number
/// moved when it went in.
fn eat_heart(hash: &mut Fnv, fight: Option<&crate::heart::HeartFight>) {
    let Some(f) = fight else {
        return;
    };
    hash.eat(u64::from(f.phase.code()));
    hash.eat(u64::from(f.conduits));
    hash.eat_rounded(f64::from(f.core_health), HEALTH_GRID);
    hash.eat(u64::from(f.laid));
    hash.eat(f.next_build.map_or(u64::MAX, |s| s));
    hash.eat(u64::from(f.built));
    hash.eat(u64::from(f.links_down));
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

/// What is on a body: what it wears, the gun in its hand and its charges
/// — integers throughout but a piece's health, to a hundredth as a piece
/// of armour's goes in anywhere else.
fn eat_gear(hash: &mut Fnv, gear: &bims::combat::Gear) {
    for piece in [gear.armour] {
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
    for (code, &n) in gear.charges.iter().enumerate() {
        if n > 0 {
            hash.eat(code as u64);
            hash.eat(n as u64);
        }
    }
    // The items (October 2026), only where one is carried, so a loadout
    // with none hashes what it always did.
    for (slot, item) in gear.items.iter().enumerate() {
        if let Some(item) = item {
            hash.eat(0x_4954_454D + slot as u64);
            hash.eat(item.kind.code() as u64);
            hash.eat(item.tier.code() as u64);
        }
    }
}

/// One thing of the armory: which of the three, what it is, and a
/// piece's id and health.
fn eat_item(hash: &mut Fnv, item: &bims::combat::Item) {
    use bims::combat::Item;
    match item {
        Item::Armour(piece) => {
            hash.eat(0);
            hash.eat(piece.id as u64);
            hash.eat(piece.kind.code() as u64);
            hash.eat(piece.tier.code() as u64);
            hash.eat_rounded(piece.health as f64, HEALTH_GRID);
        }
        Item::Weapon(weapon) => {
            hash.eat(1);
            hash.eat(weapon.kind.code() as u64);
            hash.eat(weapon.tier.code() as u64);
        }
        Item::Stack(code) => {
            hash.eat(2);
            hash.eat(*code as u64);
        }
        Item::Module(item) => {
            hash.eat(3);
            hash.eat(item.kind.code() as u64);
            hash.eat(item.tier.code() as u64);
        }
    }
}
