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

use crate::Speed;
use crate::data;
use crate::run::Phase;
use crate::world::{Command, World};

/// What the reference crew have left over from the design phase.
pub const REFERENCE_MONEY: economy::Money = 40_000;

/// Every player's own crew member walked back aboard — to the deck just
/// inside the ship's airlock (`Aboard::gangway`) — as a player would walk
/// it: since task 111 a mission at a site the machines are coming for
/// opens with the crew ashore, and a scripted run with nobody at the
/// keyboard has to walk them back before *Back to ship* can carry. Empty
/// while the rooms are not joined.
pub fn walk_the_players_aboard(world: &World) -> Vec<Command> {
    let Some(at) = world.aboard.gangway else {
        return Vec::new();
    };
    (0..world.players())
        .map(|slot| Command::Crew {
            slot,
            order: bims::order::CrewOrder::SendTo {
                who: slot,
                x: at.x as f32,
                y: at.y as f32,
            },
        })
        .collect()
}

/// How many steps each mission of [`reference_run`] takes. Ten game minutes
/// at 1x, which is long enough for the crew to be about their business and
/// short enough that the check does not hold up a start.
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
/// (`crate::class::Charge::{Medkit, Bandage}` until task 120 took them
/// away): the hold no longer fills anybody's pack.
/// And again for a run (feature 102): the world's four switches hashed at
/// the end, the hostile list empty with every human friendly, the crisis
/// there from day nought, and the room's needs standing still.
/// And again for the run's loop (feature 103): `World::run` hashed at the
/// very end, the droid and town clocks in mission steps, and the scenario
/// itself grown a second half — back to the ship, the map, a trip resolved
/// and a mission at the far end of it.
/// And again when the old game was deleted (feature 104): nothing is
/// flown, so the scenario's first half is a mission at the spawn rather
/// than a trip under the free clock, and the flight's state, the helm's
/// destination, the radiation dose, the cold store's spoiling clock and
/// the needs' and the dose's switches are out of the hash.
/// And again when every human enemy went with it (feature 104): the
/// hostile list and the arena's reinforcements, the raids' schedule and
/// state, an enemy's shelves (on the world and in every system's memory)
/// and the human foes' switch are out of the hash.
/// And again when research left the game for relics (feature 106): the
/// research tree and every station's key tier — on the world and in
/// every system's memory — are out of the hash, and the run's relics, a
/// held site's cache and each crew member's shots fired are in it.
/// And again for the Machine Heart (feature 108): the run's summary — the
/// machines destroyed, the sites cleared and the systems liberated — is
/// in the hash, where the reference run's machines count. The Heart's own
/// fight is hashed only at its fortress, so nothing else moved.
/// And again for procedural stations (feature 112): the scenario's second
/// mission is at station 0 of star 1, an orbital (seed
/// `6020119263586756475`) that rolled the drawn hub and is a generated
/// station now, so the crew come ashore onto another deck and every body's
/// place — which the hash holds — is another. Was `0x_d534_2dda_6563_babe`;
/// with every station on the drawn plan its seed rolled and every town on
/// the template (`station::set_legacy_layouts`) that number came back
/// (`tests_layoutgen::the_pins_come_back_under_the_old_layouts`), so
/// nothing but the layout moved it. The spawn is a hub still and its
/// design did not move.
/// And again for task 113 (nothing stored): the hold's pieces, guns,
/// grids, workbench, craft targets and tick box are out of the hash, and
/// the holdings (the armory, the keys, the offers) and every crew
/// member's loadout and charges are in it; the design's gear is the
/// armory's from the start, so the ship is lighter and its trips are
/// other lengths. Was `0x_eb8a_5e06_aeca_832b`.
/// And for task 115 (the minigun and the rail lance): two more resources
/// are two more empty cargo slots in the ship's `design_hash`, which the
/// checksum eats — the ship, its mass and the run are what they were
/// (`SURVIVORS` did not move). Was `0x_2e8b_94f3_4a74_debc`.
/// And for task 116 (the arc greaves and the Reflective plate), the same
/// way: two more resources, two more empty cargo slots in the design hash,
/// and nothing else. Was `0x_24c4_416e_929c_d093`.
/// And for task 111 (every site an attack, a defence or a trader), **a
/// change meant to alter how a run plays**: the spawn is a defence from
/// the first step, so its `Defense` is hashed and the crew are stood
/// ashore — the scenario now walks the players back aboard before the
/// ship can leave (`walk_the_players_aboard`), and the site at the far
/// end is a defence too, its room opened with its defenders; and no
/// mining outpost is built, so the galaxy's stations are others. Was
/// `0x_7135_e4b4_5415_2e9f`.
/// And for *Back to ship* walking the player's own Bim home
/// (`World::walk_the_player_home`), **a change meant to alter how a run
/// plays**: the scenario presses it with both players ashore, so they set
/// off for the gangway the step it is pressed rather than the step after,
/// when `walk_the_players_aboard` sends them. Was `0x_36d8_8939_97a9_819b`.
/// And for worldgen's `GENERATOR_VERSION` 8 (six hundred stars, every
/// system a station and a planet to land on) with a trader in one system
/// in ten: another galaxy, so another spawn, another crew's day and
/// another site at the far end. Was `0x_86d1_6b33_0409_b543`.
/// And for the relics of tasks 117 and 118: a new profile's pool holds
/// fifteen more (task 118's tier ones and twos), and a choice no longer
/// carries a tier (task 117) — the pool and the choice are hashed. The run
/// plays as it did (`SURVIVORS` and the ship's `PINNED` did not move).
/// Was `0x_3222_89f6_e882_3fce`.
/// And for one speed (task 119): the scenario's two speed requests are 1×
/// where they were 24× (`Speed::Day` is gone), and a request's code is
/// hashed. Nothing else of the task moved it — with the speed alone, and
/// the experience left as it was, the new number came out the same.
/// Was `0x_1015_15d4_3b34_cfaa`.
/// And for the health rework (task 120), **a change meant to alter how a
/// run plays**: one bar of hit points where there was blood, wounds and
/// traumas, a downed body's countdown and the slow a downing leaves hashed
/// where the fear was, no medicine charges on anybody (their timers gone),
/// no drug lab or medicine aboard the playtest ship (another design hash),
/// and every hit that takes hit points a splash on the room's stream.
/// Was `0x_bdf6_e347_2c87_400a`.
/// And for the engineer's ranked kit (task 127): every class charge is a
/// counter on the world (`World::charges_held`), hashed whole where the
/// re-used kits were, and a charge's cooldowns are five codes long.
/// Was `0x_ee0a_ea07_0be0_41fb`.
/// And for five changes landed together on 28 September 2026 and
/// re-pinned as one tree, **each meant to alter how a run plays**: nobody
/// walks about at random and the player's own Bim is moved by its player
/// alone (no errand of its own, never shoved by a bot, under arms with the
/// alarm); task 131 (#16: the Manufacturers attack a defence before day
/// ten, station waves thirty seconds apart — `droid_reinforce` is hashed —
/// a field medic sets a carried body down, arrival spots on reachable
/// deck); task 131 (#20: every generated station another building, the
/// waves in at every airlock but the port); task 130 (the medic's ranked
/// kit, its timers and cloaks hashed where set); and task 132 (the wave's
/// cap, hashed as sixteen, is a forced size, nought unforced). Their
/// shares were not taken apart. Was `0x_240c_b1c5_daa2_167b`.
/// And for task 136, on purpose: the machines hold every other site of
/// a system from the first day (`outposts.rs`), so the reference run's
/// second mission meets an outpost's hold. Task 135 (one station and one
/// town a system, one fight a system) is not in it: the fixture keeps
/// whole systems (`World::set_whole_systems_for_probe`), and with the
/// outposts switched off the run came out at the old number. Was
/// `0x_4284_750b_5249_4340`.
/// And for the elites (079ffe0, no task number), on purpose: one system in
/// ten holds an elite, a Guardian in its second wave, and only elites drop
/// relics — a plain cleared site no longer offers them, the Manufacturers'
/// sites keep no cache and none stand in an elite system. Measured on that
/// commit alone: 5175dd8 before it came out at the old number. Task 138
/// (a revive under way stands the countdown) did not move it. Was
/// `0x_c6be_4221_4565_e460`.
/// And for three changes of 29 September 2026 in the tree together,
/// **each meant to alter how a run plays**: task 137's follow-up (a04aade:
/// a machine has one health, every hit comes off it, wrecked at nought),
/// the map rework (5ce3038: a jump is a day and a trip within a system
/// nothing, so the reference run's trip moves the clock another way) and
/// the bots' deaths (7b97be5: a bot lost costs the pool nothing). Their
/// shares were not taken apart. The empty-handed crewmate's pistol
/// (e5d8655) does not move it: with that rule switched off the run came
/// out at this same number. Was `0x_3e51_51c8_9fc9_08a0`.
/// And for a site's waves by its tier (no task number), on purpose: one
/// wave at tier one, two at tier two, four at tier three
/// (`data::DROID_TIER_WAVES`), where it was two and a wave every second
/// step — the reference run's sites are tier one, so their fight is one
/// wave now. With the count put back to two at every tier the run came
/// out at the old number. Was `0x_df17_c099_88c7_21d1`.
/// And for individual money (no task number), on purpose: the pool is
/// shared out into every player's wallet when the world opens and at the
/// end of every mission (`World::share_out`), and the wallets are
/// hashed — and, taken in the same tree, every
/// relic unlocked from the start (`relic.rs`, agent #12: the run's pool
/// is hashed). Was `0x_aca1_40d8_3ab7_360d`.
/// And re-pinned on 5 October 2026 (agent #17) after a week red: from
/// about 30 September every task's commit said "the reference checksum
/// red as before", and some two dozen changes **each meant to alter play**
/// landed on top of it (the weapons' ranges at 0.7, the electricity gone,
/// the smaller galaxy, the crew round the gangway, the movement's speed,
/// the defences' timings, the levels' experience, the Lancer's rail, the
/// six item slots, the floor's 32 rows). Their shares were not taken
/// apart. Measured on 8e53f94 (the tree's tests only besides). Was
/// `0x_6996_9850_bcab_0e80`.
pub const REFERENCE_CHECKSUM: u64 = 0x_6b64_4af0_7709_4b89;

/// A world with the flyable fixture docked at the simulation's spawn: the
/// default seed's first dock, which is where every fixture world starts.
/// **Not quiet** (task 111): the reference run is the game, so the
/// machines come for the spawn as they come for every site.
pub fn reference_world() -> World {
    open_simulation_world(flyer(2), REFERENCE_MONEY, 2)
}

/// A world opened the way the simulation opens one: the default seed, a
/// two-arm spiral, and [`crate::spawn`]'s dock. What `nix run .#simulation`
/// does with [`shipdesign::fixture::playtest_ship`] and
/// [`data::SIMULATION_MONEY`], and what every fixture here does with its own
/// ship and purse.
///
/// **Whole** (task 135, [`World::set_whole_systems_for_probe`]): every
/// system keeps every station and town, as every test here was written
/// against; `tests_offered.rs` opens its own worlds without the dial.
///
/// **Quiet** (task 111, [`World::set_quiet_sites_for_probe`]): every site
/// neither a trader nor an enemy's is a peaceful stop, since the tests
/// that open one are about something other than the fight a site's
/// defence is. A test of the defence takes the dial off again.
///
/// **By day, lamps lit** (task 152, [`World::set_night_for_probe`],
/// [`World::set_dark_for_probe`]): every town is landed at under its sky
/// and every enemy's station lit, as every test here was written for; a
/// test of the dark says so.
pub fn simulation_world(
    design: shipdesign::ShipDesign,
    money: economy::Money,
    players: u32,
) -> World {
    let mut world = open_simulation_world(design, money, players);
    world.set_quiet_sites_for_probe(true);
    world.set_night_for_probe(Some(false));
    world.set_dark_for_probe(Some(false));
    world
}

/// [`simulation_world`] as the game opens it: the machines coming for the
/// spawn from the first step.
pub fn open_simulation_world(
    design: shipdesign::ShipDesign,
    money: economy::Money,
    players: u32,
) -> World {
    let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (star, station) = crate::spawn(&galaxy).expect("the default seed has a dock somewhere");
    let mut world = World::start(
        design,
        money,
        players,
        data::DEFAULT_SEED,
        GalaxyType::SpiralTwoArm,
        star,
        station,
    )
    .expect("the default seed should have somewhere to spawn");
    world.set_whole_systems_for_probe(true);
    world
}

/// [`simulation_world`] with `crew` aboard, of whom the first `players`
/// are players — `World::start_with_crew`. A world of one player and two
/// crew is one where the second is a crewmate nobody steers: a bot, under
/// the alarm, since feature 59 gave every player's own to its player.
/// **Quiet** (task 111) and **by day** (task 152), as [`simulation_world`]
/// is.
pub fn crewed_world(
    design: shipdesign::ShipDesign,
    money: economy::Money,
    players: u32,
    crew: u32,
) -> World {
    let mut world = open_crewed_world(design, money, players, crew);
    world.set_quiet_sites_for_probe(true);
    world.set_night_for_probe(Some(false));
    world.set_dark_for_probe(Some(false));
    world
}

/// [`crewed_world`] as the game opens it, the machines coming for the
/// spawn from the first step: what `SURVIVORS` opens its worlds with,
/// since it pins the game as it plays.
pub fn open_crewed_world(
    design: shipdesign::ShipDesign,
    money: economy::Money,
    players: u32,
    crew: u32,
) -> World {
    let galaxy = worldgen::Galaxy::new(data::DEFAULT_SEED, GalaxyType::SpiralTwoArm);
    let (star, station) = crate::spawn(&galaxy).expect("the default seed has a dock somewhere");
    let mut world = World::start_with_crew(
        design,
        money,
        players,
        crew,
        data::DEFAULT_SEED,
        GalaxyType::SpiralTwoArm,
        star,
        station,
    )
    .expect("the default seed should have somewhere to spawn");
    world.set_whole_systems_for_probe(true);
    world
}

/// The scenario: open a world, run a mission at the spawn for
/// [`REFERENCE_STEPS`], and then the run's own loop (feature 103): both
/// players back to the ship, the map, a destination put and accepted, the
/// trip resolved, and [`REFERENCE_STEPS`] of the mission that begins there.
///
/// Everything a checksum is meant to catch is in it — the crew walking
/// about aboard and on the joined deck, a trip's length worked out from a
/// square root and put on the clock, the ship set down at a berth through
/// a trigonometric function, and the site at the far end opened, its
/// people with it.
pub fn reference_run() -> u64 {
    reference_run_world().checksum()
}

/// The world [`reference_run`] ends on, for a test that wants to see the
/// scenario did what it says: went back to the ship, travelled, and is on
/// a mission at the far end.
pub fn reference_run_world() -> World {
    let mut world = reference_world();
    // Both players ask for 1× — the only speed there is since task 119,
    // and what they already had — so the step that carries the requests
    // is a decision taken rather than the default. The code is in the
    // checksum.
    world.step(&[
        Command::SetSpeed {
            slot: 0,
            speed: Speed::Real,
        },
        Command::SetSpeed {
            slot: 1,
            speed: Speed::Real,
        },
    ]);
    for _ in 1..REFERENCE_STEPS {
        world.step(&[]);
    }
    // Both players pressing Back to ship — aboard, so the ship leaves —
    // the first destination of the map put and accepted, and a mission
    // there.
    world.step(&[Command::Return { slot: 0 }, Command::Return { slot: 1 }]);
    // The crew start the mission ashore since task 111 — the machines
    // are coming for the spawn — so the players walk back aboard, and
    // whoever the departure check asks about is left behind.
    let aboard = walk_the_players_aboard(&world);
    world.step(&aboard);
    for _ in 0..REFERENCE_STEPS * 10 {
        if world.run.phase != Phase::Mission {
            break;
        }
        if matches!(
            world.run.departure,
            Some(crate::run::Departure::Asking { .. })
        ) {
            world.step(&[
                Command::LeaveBehind { slot: 0, yes: true },
                Command::LeaveBehind { slot: 1, yes: true },
            ]);
        } else {
            world.step(&[]);
        }
    }
    if world.run.phase == Phase::Map {
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
            // One bar and the downed countdown since task 120, where the
            // blood and each part's health, wounds and trauma were.
            self.eat_f(room.health(who) as f64);
            self.eat_debug(&room.down_left(who));
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
        for &wallet in &world.wallets {
            self.eat(wallet);
        }
        self.eat(world.star_id as u64);
        self.eat_debug(&world.ship.state.alongside());
        // Experience and the class it is spent in, crew member by crew
        // member.
        for (class, progress) in world.classes.iter().zip(&world.progress) {
            self.eat_debug(class);
            self.eat(progress.xp as u64);
            // An empty list where the talents picked were (task 139 took
            // them away): no run ever picked one, so the reading is as it
            // was.
            self.eat_debug(&[(); 0]);
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
            self.eat(world.site_threatened(site.station) as u64);
        }
    }
}

impl Default for Survivors {
    fn default() -> Survivors {
        Survivors::new()
    }
}
