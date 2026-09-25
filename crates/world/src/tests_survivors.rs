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
use crate::fixture::{REFERENCE_MONEY, Survivors, crewed_world};
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
const SURVIVORS: u64 = 0x_1ac2_9e51_d456_7c7a;

/// A gun in every hand, the kinds dealt round, as the fight's probes arm
/// a crew.
fn arm(world: &mut World) {
    let crew = world.aboard.room.crew_count() as usize;
    for (who, kind) in WeaponKind::ALL
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

/// Both players' *Back to ship*, pressed aboard, until the map is up.
fn back_to_ship(world: &mut World) {
    world.step(&[Command::Return { slot: 0 }, Command::Return { slot: 1 }]);
    for _ in 0..600 {
        if world.run.phase == Phase::Map {
            return;
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
fn survivors() -> u64 {
    let mut hash = Survivors::new();

    // The trip to a held station, and the fight.
    let mut world = crewed_world(combat_ship(), REFERENCE_MONEY, 2, 6);
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
    let mut town = crewed_world(combat_ship(), REFERENCE_MONEY, 2, 5);
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
        .find(|s| town.town_threatened(s.station) && town.travel_quote(*s).is_ok())
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
