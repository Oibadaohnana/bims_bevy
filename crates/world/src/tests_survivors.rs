//! What the old game's deletion left alone (feature 104).
//!
//! The third step of the roguelike redesign deleted the needs, the
//! radiation, the human foes and the flown trip — everything the first
//! step had switched off. **Nothing a run does was meant to move**, and
//! this is what says it did not: a seeded run — a trip resolved, a
//! mission fought at the far end of it, a jump, and a town defended —
//! hashed over **only the state that outlived the deletion**: the world
//! clock and the mission clock, the pool, every body's position, health,
//! experience and gear, and every site's state. The number was taken off
//! the tree as it stood before anything was deleted (tag
//! `needs-sim-final`), so it is pinned rather than computed, the way
//! `REFERENCE_CHECKSUM` is.
//!
//! The hash is `crate::fixture::Survivors`, so `crates/ship` can take the
//! same reading of the commands it builds.
//!
//! `world_checksum` could not do this job: the switches and the needs
//! were *in* it, so the deletion had to move it. This hashes nothing the
//! deletion took away, and so it must not move — ever, until a change
//! meant to alter how a run plays says why in the constant's own note.

use bims::combat::{Gear, WeaponKind};
use bims::math::vec2;
use shipdesign::fixture::combat_ship;

use crate::LootSource;
use crate::class::Class;
use crate::data;
use crate::fixture::{REFERENCE_MONEY, Survivors, open_crewed_world};
use crate::orders::Standing;
use crate::run::{Phase, Site};
use crate::world::{Command, World};

const TILE: f32 = shipdesign::TILE as f32;

/// What [`survivors`] comes out at. Taken off the tree before the
/// deletion (the `needs-sim-final` tag), with the same scenario, as
/// `0x_9aad_713a_e8d2_95ba`.
///
/// **Moved once since, on purpose**: the fix after feature 104 that pays
/// experience for a machine downed in a **town's defence**
/// (`World::first_enemy_body`) — the town run's engineer and tank earn
/// it now. Nothing else moved: the hash after the first two worlds (the
/// held station's fight and the jump) is the same with the fix and
/// without it, and with the defence's half of the fix taken out the old
/// number came back.
///
/// **And once more, on purpose**: the Guardian (feature 100). The town run
/// is one hop from the machines' origin, so its waves come at tier three,
/// and a tier-three wave of six has a Guardian in it out of the Troopers'
/// share (`bims::droid::guardians_of`), and it sweeps its beam at the
/// town's defenders. Was `0x_1ac2_9e51_d456_7c7a`; with the Guardians
/// switched off in `guardians_of` and everything else of the feature in
/// — the shield, the turn, the beam — that number came back, so nothing
/// else moved.
///
/// **And once more, on purpose**: the least a trip may be (feature 105,
/// `data::MIN_TRAVEL_HOURS`). A trip under a day is a day now, and the
/// run has one — the world clock is in the hash, and the crisis and the
/// wages are read off it. Was `0x_a382_6e88_a4a0_8f4c`; with the least
/// set to nought and everything else of the feature in — the waves
/// scaled on the players and the clock alone, the site the crew are at
/// refused — that number came back: both worlds force the wave to six,
/// and the count is two at day nought under the old rule and the new.
///
/// **And once more, on purpose**: the experience rule of feature 109 — an
/// enemy is worth `XP_ENEMY_DOWN` once, at its first down or death, and
/// its death after a down nothing more, where a machine destroyed used to
/// pay `XP_ENEMY_DEAD` (five) on top. The crew's experience is in the
/// hash. Was `0x_062f_ddde_23e9_2bfb`; with the five put back on a death
/// and everything else of the feature in — the Manufacturers' sites
/// rolled across the galaxy, the jammer rule round them — that number
/// came back, so nothing else moved: the run's jump and its town meet no
/// site of theirs.
///
/// **And once more, on purpose**: procedural stations and towns (feature
/// 112). Every station but the spawn is generated from its seed and every
/// town's streets, gates and lots are drawn, so the run's held station,
/// the station it jumps to and the town it defends are other buildings,
/// and every body in them stands somewhere else. Was
/// `0x_024e_0982_c931_c4bb`; with every station on the drawn plan its
/// seed rolled and every town on the template
/// (`station::set_legacy_layouts`) that number came back
/// (`tests_layoutgen::the_pins_come_back_under_the_old_layouts`), so
/// nothing but the layouts moved it.
///
/// **And once more, on purpose**: nothing stored (task 113). The reading
/// takes a body's gear by its `Debug`, and `Gear` lost the pack and
/// gained the charges, so the number could not have stayed; and the run
/// plays differently by the task's own rules — every piece mended and
/// every charge **set** at each mission's start, the dead players back
/// at the mission's end rather than bought at the next one's start, a
/// body out cold keeping its gun rather than dropping it and fetching it,
/// and the design's gear in the armory rather than the hold (a lighter
/// ship, other trip lengths). With the reading itself changed there is
/// no old number to bring back by switching one rule off, so none of
/// those was isolated. Was `0x_81b4_5bae_e33d_8446`.
///
/// **And once more, on purpose**: every site an attack, a defence or a
/// trader (task 111), a change meant to alter how a run plays. The
/// machines come for every site that is neither a trader nor an enemy's
/// from the first day — the spawn both worlds open at, the station the
/// run jumps to and its town — twenty seconds after the crew arrive, the
/// crew stood ashore when it starts and armed defenders beside the site's
/// people, the wave sized with them as players; the scenario walks the
/// players back aboard before *Back to ship* can carry
/// (`fixture::walk_the_players_aboard`) and answers the departure check
/// for whoever is left out, and the town is the first **town** threatened
/// rather than the first site. A defence is won only once every wreck is
/// counted, so the last machine's bounty is paid. And no mining outpost is
/// built, so the galaxy's stations are others. None of it isolated: the
/// rule is the run. Was `0x_14e4_ee7e_31d6_a769`.
///
/// **And once more, on purpose**: worldgen's `GENERATOR_VERSION` 8 — six
/// hundred stars where there were a thousand, every system given a
/// station and a planet to land on — with a trader in one system in ten.
/// Every seed is another galaxy, so the spawn, the site the run fights
/// at, the star it jumps to and the town it defends are all others; no
/// rule of the room moved. Was `0x_04ed_2f68_873a_b95b`.
///
/// **And once more, on purpose**: experience the same for every class
/// (task 119). An enemy's death is worth `XP_ENEMY_DEAD` (five) on top of
/// its down again, and nothing a class does — building, a kit laid,
/// healing, hits taken, a hire — is worth anything; the crew's experience
/// is in the reading, and a level reached is a fixed talent. The task's
/// other half, one speed, moved nothing here: with it alone on HEAD the
/// old number came back (the scenario sends no speed request, and the
/// wave-landing reset it dropped only ever set 1× on requests already at
/// 1×). Was `0x_7a04_c23f_0efb_0d9f`.
///
/// **And once more, on purpose**: the health rework (task 120), a change
/// meant to alter how a run plays. One bar of hit points and nothing else
/// — no blood, no wounds, no traumas, no bandages or medkits — a body at
/// nought downed for thirty seconds and then dead unless a crewmate
/// revives it, the bots reviving where they doctored, no dying crew member
/// running from the fight, and the blood on the deck thrown by every hit
/// that takes hit points and dripped under twenty, which draws differently
/// on the room's stream. The reading changed shape with it (a body's bar
/// and its countdown where its blood and its parts were), so there is no
/// old number to bring back by switching one rule off. Was
/// `0x_002a_da6c_00c6_d69f`.
///
/// **And once more, on purpose**: the soldier's and the engineer's ranked
/// kits (tasks 124 and 127, landed together). The first run's soldier
/// climbs a sixteen-level table and starts with no grenade at rank nought
/// where it had two; the town run's engineer starts with no sandbags and
/// no sentry kit where it had three and one; and every class charge is a
/// counter on the world now, so no charge is in any gear the reading takes.
/// Nobody in either run buys a rank, lays a deployable or throws anything.
/// The two tasks' shares were not taken apart: their changes went into
/// the tree together. Was `0x_7535_9d7e_fddc_80af`.
///
/// **And once more, on purpose**, for five changes landed together on 28
/// September 2026 and tested as one tree, each meant to alter how a run
/// plays: nobody walks about at random any more and the player's own Bim
/// is moved by its player alone — no errand of its own, never shoved by a
/// bot under arms, under arms with the alarm (the wander drew on the
/// room's stream every step); task 131 (#16: the town run's defence is
/// the Manufacturers' before day ten, waves thirty seconds apart, a field
/// medic sets a carried body down, arrival spots on reachable deck); task
/// 131 (#20: the first run's station is another building, the waves in at
/// every airlock but the port); task 130 (the first run's medic revives to
/// two fifths, a Healing Sentry heals four times as much); and task 132's
/// uncapped wave. Their shares were not taken apart. Was
/// `0x_5051_196b_e073_7b74`.
///
/// **And once more, on purpose**, for the tasks of 29 September 2026 in
/// the tree together: task 133 (a fight won freezes the deck, the downed
/// do not die and go home with the rest), and task 136 (the machines hold
/// every other site from the first day, a defence pays no money). With
/// 136's outposts switched off the run read `0x_d6d3_1a11_58ab_2616`, so
/// both move it. Task 135 does not: the fixture keeps whole systems
/// (`World::set_whole_systems_for_probe`), and a start that never trims
/// read the same as one that trims and puts back. Was
/// `0x_1278_0108_e96e_dc55`.
///
/// **And again, on purpose**, for three changes of 29 September 2026 in
/// the tree together: task 137's follow-up (a machine has one health that
/// every hit comes off, wrecked at nought and not by its head), the map
/// rework (a jump is a day, a trip within a system nothing) and the bots'
/// deaths (a bot lost costs the pool nothing). Their shares were not taken
/// apart. The empty-handed crewmate's pistol does not move it: with that
/// rule switched off the run read the same. Was `0x_dd01_665e_6048_4ba8`.
///
/// **And again, on purpose**: an enemy down is twenty experience and its
/// death nothing more (`XP_ENEMY_DEAD` gone, `XP_ENEMY_DOWN` ten to
/// twenty), and the crew's experience is in the reading. Taken on a clean
/// worktree of f749c42 with that change alone, since other agents'
/// uncommitted work in the tree moved it as well (the tree read
/// `0x_702c_7b50_cfc9_f862` with theirs in). Was `0x_0276_3b8b_1c2e_7309`.
///
/// **Not in the constant yet, and moving it on purpose**: the bots under
/// an attack banner measure the ground they make good by the walk rather
/// than as the crow flies (`Tactics::advance`), and this run puts a
/// banner down. On a clean worktree of f749c42 with that change alone it
/// read `0x_6590_3c89_ab90_c124` where that tree pinned
/// `0x_0276_3b8b_1c2e_7309`. The shared tree read `0x_702c_7b50_cfc9_f862`
/// without it (other agents' uncommitted work) and `0x_cc3d_8f02_97f0_2873`
/// with it, so the constant is left for the sweep to take.
///
/// **Not in the constant yet either, and moving it on purpose**: a shot
/// out of a peek leaves the eye rather than the muzzle when the muzzle's
/// line to the target clips the corner or a lamp (`Game::shot_from`),
/// so the gun held on the right no longer puts bolts into the wall a
/// corner on that side. On 3adce24 plus the working tree's `crates/game`
/// (and HEAD's `earn_bounty` hunk put right) it read
/// `0x_8d00_61ed_88c8_6121`, and `0x_cc3d_8f02_97f0_2873` with that one
/// check taken out. Left for the sweep as well.
///
/// **And a site's waves by its tier, on purpose** (no task number: one
/// wave at tier one, two at tier two, four at tier three,
/// `data::DROID_TIER_WAVES`, where it was two and a wave every second
/// step): the tree read `0x_8d00_61ed_88c8_6121` with the count put back
/// to two at every tier and `0x_c91e_9026_9f7f_3d90` with it. Left for
/// the sweep with the two above.
///
/// **And taken, on purpose, for individual money and a bounty by the
/// enemy's strength** (no task number): the pool shared out into every
/// player's wallet at the opening and at every mission's end (the
/// wallets are in `Survivors::eat_world`), and a machine's bounty its
/// kind's share of its tier's (a Husk 90 %, a Warden 110 %). Taken on
/// the tree as it stood — the three moves above that were left for the
/// sweep, and agent #12's relic rebalance (task 142, no relic in this
/// run), with it. Was `0x_11a9_33d1_6eeb_9209`.
const SURVIVORS: u64 = 0x_eacb_4bf8_f39d_85b2;

/// A gun in every hand, the kinds dealt round, as the fight's probes arm
/// a crew — the five there were when the reading was taken: the minigun
/// and the rail lance (task 115) joined `WeaponKind::ALL` after it, and a
/// sixth crew member with a minigun would be a different run rather than
/// the same run read again.
fn arm(world: &mut World) {
    let crew = world.aboard.room.crew_count() as usize;
    for (who, kind) in WeaponKind::ALL[..5]
        .iter()
        .copied()
        .cycle()
        .take(crew)
        .enumerate()
    {
        let gear = world.aboard.room.gear(who);
        world.aboard.room.issue(
            who,
            Gear {
                weapon: Some(kind.basic()),
                ..gear
            },
        );
    }
}

/// Every player's yes to `site`: the trip, resolved.
fn travel_to(world: &mut World, site: Site) {
    world.step(&[Command::Propose {
        slot: 0,
        star: site.star,
        station: site.station,
    }]);
    for slot in 1..world.players() {
        world.step(&[Command::Accept { slot, yes: true }]);
    }
    assert_eq!(world.run.phase, Phase::Mission, "the trip was taken");
    assert_eq!(world.ship.state.alongside(), Some(site.station));
}

/// Both players' *Back to ship* until the map is up — and since task 111
/// the crew start a mission ashore at a site the machines are coming
/// for, so they walk back aboard first (a longer wait), and whoever the
/// departure check asks about is left behind, both players saying yes.
fn back_to_ship(world: &mut World) {
    world.step(&[Command::Return { slot: 0 }, Command::Return { slot: 1 }]);
    let aboard = crate::fixture::walk_the_players_aboard(world);
    world.step(&aboard);
    for _ in 0..3000 {
        if matches!(world.run.phase, Phase::Map | Phase::Reward) {
            return;
        }
        if matches!(
            world.run.departure,
            Some(crate::run::Departure::Asking { .. })
        ) {
            world.step(&[
                Command::LeaveBehind { slot: 0, yes: true },
                Command::LeaveBehind { slot: 1, yes: true },
            ]);
            continue;
        }
        world.step(&[]);
    }
    panic!("the ship never left");
}

/// The nearest tile of the crew's deck a banner may stand on to where a
/// body lies, in the room's own tile frame.
fn banner_by(world: &World, body: LootSource) -> Option<(i32, i32)> {
    let room = &world.aboard.room;
    let at = world.body_position(body)?;
    let (cx, cy) = ((at.x / TILE) as i32, (at.y / TILE) as i32);
    for r in 0..12 {
        for dy in -r..=r {
            for dx in -r..=r {
                let tile = (cx + dx, cy + dy);
                let middle = vec2((tile.0 as f32 + 0.5) * TILE, (tile.1 as f32 + 0.5) * TILE);
                if room.is_banner_tile(middle) {
                    return Some(tile);
                }
            }
        }
    }
    None
}

fn run_for(world: &mut World, steps: u32) {
    for _ in 0..steps {
        world.step(&[]);
    }
}

/// The run: two players and four bots on the combat ship, armed. A
/// station of the spawn system handed to the machines, the trip there
/// resolved and the fight at the far end of it; then off whoever is
/// where, a jump to a star next door and a mission there; and a second
/// world whose own system is on the front, the trip to its town and the
/// town's fight. Each world hashed as it ends.
pub(crate) fn survivors() -> u64 {
    let mut hash = Survivors::new();

    // The trip to a held station, and the fight.
    let mut world = open_crewed_world(combat_ship(), REFERENCE_MONEY, 2, 6);
    world.set_class(0, Class::Soldier).unwrap();
    world.set_class(1, Class::Medic).unwrap();
    arm(&mut world);
    world.set_droid_reinforce_minutes_for_probe(1.0);
    world.set_droid_wave_for_probe(6);
    let here = world.current_site();
    let held = world
        .sites_at(world.star_id)
        .into_iter()
        .find(|&s| {
            Some(s) != here
                && crate::surface::surface_body(s.station).is_none()
                && world.travel_quote(s).is_ok()
        })
        .expect("the spawn system has another station");
    world.infest(held.station);
    back_to_ship(&mut world);
    travel_to(&mut world, held);
    // The bots sent at the machines, both players' banner by the first
    // of them, so the crew's own fight is in the run as well as theirs.
    run_for(&mut world, 10);
    let tile = banner_by(&world, LootSource::Resident(0)).expect("ground by the first machine");
    world.step(&[
        Command::Orders {
            slot: 0,
            order: Standing::Attack { tile },
        },
        Command::Orders {
            slot: 1,
            order: Standing::Attack { tile },
        },
    ]);
    run_for(&mut world, 2400);
    hash.eat_world(&world);

    // A jump next door, and the mission there.
    world.leave_for_probe();
    let next = world
        .destinations()
        .into_iter()
        .find(|s| s.star != world.star_id && world.travel_quote(*s).is_ok())
        .expect("some star next door has somewhere to go");
    travel_to(&mut world, next);
    run_for(&mut world, 1200);
    hash.eat_world(&world);

    // A town on the front, and its fight.
    let mut town = open_crewed_world(combat_ship(), REFERENCE_MONEY, 2, 5);
    town.set_class(0, Class::Engineer).unwrap();
    town.set_class(1, Class::Tank).unwrap();
    arm(&mut town);
    let hops = town.start_star_hops_for_probe();
    let star = (0..hops.len() as u32)
        .find(|&s| hops[s as usize] == 1)
        .expect("a star one hop off");
    town.set_crisis_first_day_for_probe(0);
    town.set_droid_origin_for_probe(star);
    town.set_day_for_probe(0);
    town.set_defense_delay_for_probe(data::STEP_MINUTES * 4.0);
    town.set_droid_reinforce_minutes_for_probe(1.0);
    town.set_droid_waves_for_probe(2);
    town.set_droid_wave_for_probe(6);
    let threatened = town
        .sites_at(town.star_id)
        .into_iter()
        .find(|s| {
            crate::surface::surface_body(s.station).is_some()
                && town.site_threatened(s.station)
                && town.travel_quote(*s).is_ok()
        })
        .expect("a threatened town in the spawn system");
    back_to_ship(&mut town);
    travel_to(&mut town, threatened);
    run_for(&mut town, 2400);
    hash.eat_world(&town);

    hash.value()
}

/// **The deletion changed no behaviour**: the seeded run hashes, over
/// what survived it, to the number it came to before anything was
/// deleted.
#[test]
fn the_run_plays_as_it_did_before_the_old_game_was_deleted() {
    let got = survivors();
    assert_eq!(
        got, SURVIVORS,
        "the run moved: {got:#018x} where {SURVIVORS:#018x} was pinned before the deletion"
    );
}
