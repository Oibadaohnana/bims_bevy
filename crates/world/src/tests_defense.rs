//! Defending a town (feature 94): the threat, the waves, the fight
//! inside one room, the win and what it leaves behind.
//!
//! The fight itself is the one in the game that does not cross the seam:
//! the machines and the town's people are both in the residents' room,
//! and what passes between them is delivered there. So most of what is
//! pinned here is *who shoots at whom*, which is three target lists and
//! one sheltering list the world sets every step.

use shipdesign::fixture::flyer;

use crate::class::{self, Class};
use crate::data;
use crate::defense;
use crate::event::WorldEvent;
use crate::fixture::{REFERENCE_MONEY, open_simulation_world};
use crate::surface;
use crate::world::{Command, World};
use crate::world_checksum;

/// The world these tests stand in. **The machines' fight whatever the
/// day** (task 131): before day ten a defence's waves are the
/// Manufacturers' in the game, and these run at day nought — the
/// Manufacturers' own tests are [`manufacturers_attack`] below.
fn basic() -> World {
    let mut world = open_simulation_world(flyer(2), REFERENCE_MONEY, 2);
    world.set_machines_only_for_probe();
    // A town's defence as it was before the Area defend (October 2026):
    // every wave down and won. `tests_area.rs` is the Area defend.
    world.set_area_defense_off_for_probe();
    world
}

/// Put the machines' origin one lane hop from the crew's own star and
/// wind the crisis to day nought: the star next door is theirs, and this
/// system's front is one — which is what a **threatened** town is.
/// `false` in a galaxy with no star one hop off, which no real one is.
fn threaten(world: &mut World) -> bool {
    let hops = world.start_star_hops_for_probe();
    let Some(star) = (0..hops.len() as u32).find(|&s| hops[s as usize] == 1) else {
        return false;
    };
    world.set_crisis_first_day_for_probe(0);
    world.set_droid_origin_for_probe(star);
    world.set_day_for_probe(0);
    assert_eq!(world.front(world.star_id), Some(1));
    true
}

/// The town the ship would land at, threatened, with both clocks cut so
/// a wave is a handful of steps away rather than an hour — and with the
/// fight's size forced, since a town's fight run to its end at the
/// formula's numbers is tens of thousands of steps. `waves` is how many
/// there are all told and `wave_size` how many machines each is, both
/// read at the landing.
fn a_threatened_town(reinforce: f64, waves: u32, wave_size: Option<u32>) -> Option<(World, u32)> {
    let mut world = basic();
    if !threaten(&mut world) {
        return None;
    }
    world.set_defense_delay_for_probe(data::STEP_MINUTES * 4.0);
    world.set_droid_reinforce_minutes_for_probe(reinforce);
    world.set_droid_waves_for_probe(waves);
    if let Some(n) = wave_size {
        world.set_droid_wave_for_probe(n);
    }
    if !world.land_for_probe() {
        return None;
    }
    let id = world.ship.state.alongside().expect("landed");
    // The town has to be a friendly one for any of this: the roll can put
    // an enemy's settlement on the first planet with ground.
    if !world.site_threatened(id) {
        return None;
    }
    Some((world, id))
}

/// Step until the predicate holds, or give up after `steps` and say so.
fn until(world: &mut World, steps: u32, mut done: impl FnMut(&World) -> bool) -> bool {
    for _ in 0..steps {
        world.step(&[]);
        if done(world) {
            return true;
        }
    }
    false
}

/// Every town is threatened from the first day (task 111), wherever the
/// machines are, and the first wave lands `DEFENSE_DELAY_STEPS` after the
/// crew set down — not before; and under the tests' quiet dial nothing is
/// coming for anybody.
#[test]
fn a_threatened_town_s_first_wave_lands_after_the_delay() {
    // A town well away from the machines is threatened all the same.
    let mut far = basic();
    assert!(far.land_for_probe(), "somewhere to land");
    let id = far.ship.state.alongside().expect("landed");
    assert!(
        far.site_threatened(id),
        "every town is threatened from day one"
    );
    // And under the quiet dial nothing is, and landing starts nothing.
    let mut quiet = basic();
    quiet.set_quiet_sites_for_probe(true);
    assert!(quiet.land_for_probe(), "somewhere to land");
    let id = quiet.ship.state.alongside().expect("landed");
    assert!(
        !quiet.site_threatened(id),
        "nothing is coming under the dial"
    );
    for _ in 0..20 {
        quiet.step(&[]);
    }
    assert!(quiet.defense(id).is_none(), "no attack without a threat");
    assert_eq!(quiet.droids_standing(), 0);

    let Some((mut world, id)) = a_threatened_town(1.0, 2, None) else {
        return;
    };
    // The attack is laid down at the first step on the pad, with the
    // whole delay still to run and nothing on the ground.
    world.step(&[]);
    let d = world.defense(id).expect("an attack").clone();
    assert_eq!(d.wave, 0, "nothing has landed yet");
    assert!(d.settled, "the wave count is fixed at the landing");
    assert!(d.next_in.is_some_and(|left| left > 0));
    assert_eq!(world.droids_standing(), 0);
    assert!(world.defense_wave_standing().is_none());
    // And it lands once the delay has run out.
    assert!(
        until(&mut world, 20, |w| w.droids_standing() > 0),
        "the first wave never landed"
    );
    let d = world.defense(id).expect("an attack");
    assert_eq!(d.wave, 1);
    assert_eq!(world.defense_wave_standing().map(|(w, _)| w), Some(1));
}

/// The fight inside the one room: the machines shoot the town's people
/// and the town's people shoot back, the crew never aim at a
/// townsperson, and everybody who is neither the guard nor a mercenary
/// goes indoors and stays there.
#[test]
fn the_town_fights_the_machines_and_the_crew_never_aim_at_a_townsperson() {
    let Some((mut world, id)) = a_threatened_town(1.0, 2, None) else {
        return;
    };
    assert!(
        until(&mut world, 40, |w| w.droids_standing() > 0),
        "the first wave never landed"
    );
    // Who shelters: everybody of the town's own but the guard, and never
    // a defender (task 111). Asked of the room, which is
    // what the world told it.
    let residents = world.residents.as_ref().expect("the town's room");
    let bims = residents.aboard.room.crew_count() as usize;
    assert!(bims > 1, "a town with people in it");
    for who in 0..bims {
        let sheltering = residents.aboard.room.is_sheltering(who);
        let want = who != surface::GUARD as usize && !residents.is_defender(who);
        assert_eq!(sheltering, want, "body {who}");
    }

    // The crew's targets are the machines alone: every one of the room's
    // Bims is handed over as nobody's target, and every machine standing
    // is somebody's.
    let targets = world.aboard.room.combat_targets_for_probe();
    for who in 0..bims {
        assert!(targets[who].is_none(), "the crew aim at townsperson {who}");
    }
    let machines = world.residents.as_ref().unwrap().aboard.room.droid_count() as usize;
    assert!(machines > 0);
    assert!(
        (bims..bims + machines).any(|i| targets[i].is_some()),
        "the crew aim at no machine"
    );

    // And the hits land both ways, inside the room. Run the fight until
    // somebody has been hurt on each side.
    let droid_health = |w: &World| -> f32 {
        let room = &w.residents.as_ref().unwrap().aboard.room;
        (0..room.droid_count() as usize)
            .filter_map(|i| room.droid(i))
            .map(|d| {
                bims::droid::DroidPart::ALL
                    .into_iter()
                    .map(|p| d.body.health(p))
                    .sum::<f32>()
            })
            .sum()
    };
    let town_health = |w: &World| -> f32 {
        let room = &w.residents.as_ref().unwrap().aboard.room;
        (0..room.crew_count() as usize)
            .map(|who| room.health(who))
            .sum()
    };
    let (droids_were, town_was) = (droid_health(&world), town_health(&world));
    let hurt = until(&mut world, 8_000, |w| {
        droid_health(w) < droids_were && town_health(w) < town_was
    });
    assert!(
        hurt,
        "the machines and the town never hurt each other: droids {} of {droids_were}, town {} of {town_was}",
        droid_health(&world),
        town_health(&world)
    );
    assert!(world.defense(id).is_some());
}

/// Taking off pauses the attack and landing again resumes it where it
/// stood: the same wave, the same waves to come, and the machines still
/// standing rather than a fresh wave at full strength.
#[test]
fn a_take_off_pauses_the_attack_and_a_landing_resumes_it() {
    let Some((mut world, id)) = a_threatened_town(1.0, 2, None) else {
        return;
    };
    assert!(
        until(&mut world, 40, |w| w.droids_standing() > 0),
        "the first wave never landed"
    );
    // Let the fight run a little, so some of the wave is down.
    for _ in 0..600 {
        world.step(&[]);
        if world.droids_standing() == 0 {
            break;
        }
    }
    let was = world.defense(id).expect("an attack").clone();
    let standing = world.droids_standing();
    if standing == 0 {
        // The wave was destroyed in the time given; the pause is then
        // about the countdown, which the next test covers.
        return;
    }
    // Off the pad, and a long way off, so the room closes.
    world.undock_for_probe();
    let far = data::RESIDENTS_RANGE * 50.0;
    world.put_for_probe(worldgen::math::dvec2(far, far));
    for _ in 0..200 {
        world.step(&[]);
    }
    let away = world.defense(id).expect("an attack").clone();
    assert_eq!(
        away.wave, was.wave,
        "the wave moved while the crew were away"
    );
    assert_eq!(away.waves_left, was.waves_left);
    assert_eq!(away.standing, standing, "the machines left the ground");
    assert_eq!(
        away.next_in, was.next_in,
        "the clock ran while the crew were away"
    );

    // And down again: the same wave, and the machines that were still up.
    world.dock_for_probe(id);
    world.step(&[]);
    assert_eq!(world.droids_standing(), standing, "a fresh wave was laid");
    let back = world.defense(id).expect("an attack");
    assert_eq!(back.wave, was.wave);
    assert_eq!(back.waves_left, was.waves_left);
}

/// The town's own guard against a wave of machines is a long fight and
/// most often a losing one — the crew are what tips it, and a test that
/// waited for one either way would be tens of thousands of steps of
/// somebody else's fight. So the wave is destroyed outright, and what is
/// pinned is what the **win** does.
fn destroy_the_wave(world: &mut World) {
    let Some(residents) = world.residents.as_mut() else {
        return;
    };
    let room = &mut residents.aboard.room;
    for i in 0..room.droid_count() as usize {
        for _ in 0..60 {
            room.strike_droid(i, bims::droid::DroidPart::Chassis, 100.0);
        }
    }
    assert_eq!(world.droids_standing(), 0, "a machine survived the lot");
}

/// The whole fight run to its end: the town is held, it says so once, it
/// stays friendly past its system's own infestation, and its desk is
/// still there.
#[test]
fn a_town_held_stays_friendly_past_its_system_s_day() {
    let Some((mut world, id)) = a_threatened_town(0.1, 1, Some(2)) else {
        return;
    };
    assert!(
        until(&mut world, 40, |w| w.droids_standing() > 0),
        "the first wave never landed"
    );
    destroy_the_wave(&mut world);
    assert!(
        until(&mut world, 20, |w| w.town_held(id)),
        "the last wave destroyed and the town not held"
    );
    assert!(world.town_held(id));
    assert_eq!(world.held_towns(), &[id]);
    assert!(!world.site_threatened(id), "held is not threatened again");
    assert!(
        world.station(id).unwrap().market().is_some(),
        "still a desk"
    );

    // Its system falls, and the town does not. `infest` is the crisis's
    // own door, so calling it is what the crisis would do.
    world.infest(id);
    assert!(!world.is_droid_held(id), "the crisis took a held town");
    assert_ne!(
        world.stance(id),
        bims::sight::Stance::Hostile,
        "a held town turned"
    );
    assert!(
        world.people_of(world.station(id).unwrap()) > 0,
        "nobody left"
    );
    // And its prices are the front's, since it is a friendly desk inside
    // the infection.
    assert_eq!(world.front_at(id), Some(1));
}

/// A win puts some of the survivors on the crew: a fifth rounded down
/// and never fewer than one, lowest index first, never the guard — and
/// they are classless bots.
#[test]
fn the_survivors_who_join_are_classless_bots() {
    let Some((mut world, id)) = a_threatened_town(0.1, 1, Some(2)) else {
        return;
    };
    assert!(
        until(&mut world, 40, |w| w.droids_standing() > 0),
        "the first wave never landed"
    );
    let crew_was = world.aboard.crew_count();
    let players = world.players();
    // Who is left in the town, and who the guard is, before the win.
    let residents = world.residents.as_ref().expect("the town's room");
    let bims = residents.aboard.room.crew_count() as usize;
    let townsfolk: Vec<u32> = (0..bims)
        .filter(|&who| residents.is_own(who))
        .filter(|&who| residents.aboard.room.is_alive(who))
        .map(|who| who as u32)
        .collect();
    let guard_alive = townsfolk.first() == Some(&surface::GUARD);
    let want = defense::joiners(townsfolk.len() as u32, guard_alive);

    destroy_the_wave(&mut world);
    let mut joined = 0u32;
    let mut held = false;
    for _ in 0..20 {
        for event in world.step(&[]) {
            match event {
                WorldEvent::TownHeld { station } => {
                    assert_eq!(station, id);
                    held = true;
                }
                WorldEvent::TownsfolkJoined { count } => joined = count,
                _ => {}
            }
        }
        if held {
            break;
        }
    }
    assert!(held, "the last wave destroyed and the town not held");
    // Exactly what the arithmetic says, and the crew grew by that many.
    assert_eq!(joined, want, "the share of {} survivors", townsfolk.len());
    assert!(joined > 0, "nobody joined a town held with people in it");
    assert_eq!(world.aboard.crew_count(), crew_was + joined);
    assert_eq!(world.players(), players, "a joiner took a player slot");
    let bunks = world.aboard.room.bed_count() as u32;
    for who in crew_was..world.aboard.crew_count() {
        assert_eq!(
            world.class_of(who),
            crate::class::Class::None,
            "joiner {who} has a class"
        );
        assert!(world.aboard.room.is_alive(who as usize), "joiner {who}");
        // **No bunk is no obstacle**: a joiner comes anyway and sleeps on the deck
        // under the room's own rule.
        if who >= bunks {
            assert!(
                world.aboard.room.bed_of(who as usize).is_none(),
                "joiner {who} took a bunk there is not"
            );
        }
    }
    // The guard never goes: it is still in the town, and the town is
    // not emptied.
    let residents = world.residents.as_ref().expect("the town's room");
    assert!(
        residents.aboard.room.crew_count() > 0,
        "the town was emptied"
    );
    if guard_alive {
        assert!(
            residents.aboard.room.is_alive(surface::GUARD as usize),
            "the guard went with the crew"
        );
    }
    // And the joiners are the lowest indices the town had, never the
    // guard: each one's gear went with it, so they are the bodies that
    // stood where they stood.
    assert_eq!(
        joined as usize,
        (world.aboard.crew_count() - crew_was) as usize
    );
}

/// **Every one of the town's people dead is no loss** (October 2026, the
/// player's word): it once infested the town mid-fight. The defence goes
/// on.
#[test]
fn a_town_whose_people_are_all_dead_does_not_fall() {
    let Some((mut world, id)) = a_threatened_town(1.0, 2, None) else {
        return;
    };
    world.step(&[]);
    // Kill the town outright — the machines would take a while about it.
    let bims = world
        .residents
        .as_ref()
        .expect("the town's room")
        .aboard
        .room
        .crew_count() as usize;
    if let Some(residents) = world.residents.as_mut() {
        for who in 0..bims {
            residents.aboard.room.kill_for_probe(who);
        }
    }
    assert!(
        !until(&mut world, 20, |w| w.is_droid_held(id)),
        "a town with nobody left fell"
    );
    assert!(world.defense(id).is_some_and(|d| !d.lost));
}

/// The system's day coming while the crew are away with waves left takes
/// the town like any other station.
#[test]
fn a_town_left_to_itself_falls_with_its_system() {
    let Some((mut world, id)) = a_threatened_town(1.0, 2, None) else {
        return;
    };
    world.step(&[]);
    assert!(world.defense(id).is_some());
    // Off the pad and far away, then the day comes.
    world.undock_for_probe();
    let far = data::RESIDENTS_RANGE * 50.0;
    world.put_for_probe(worldgen::math::dvec2(far, far));
    let day = world.infested_on(world.star_id);
    world.set_day_for_probe(day);
    assert!(
        until(&mut world, 20, |w| w.is_droid_held(id)),
        "the system's day never took the town"
    );
    assert!(world.defense(id).is_some_and(|d| d.lost));
    assert!(!world.town_held(id));
}

/// The arithmetic the joiners are counted by, said again against the
/// module's own table, and the checksum notices an attack.
#[test]
fn the_checksum_notices_an_attack_and_two_worlds_fight_alike() {
    assert_eq!(defense::joiners(10, true), 2);
    let Some((mut world, id)) = a_threatened_town(1.0, 2, None) else {
        return;
    };
    let mut twin = {
        let Some((twin, _)) = a_threatened_town(1.0, 2, None) else {
            return;
        };
        twin
    };
    assert_eq!(world_checksum(&world), world_checksum(&twin));
    let quiet = world_checksum(&world);
    // The attack laid down is a different world.
    world.step(&[]);
    assert_ne!(world_checksum(&world), quiet, "the attack is not hashed");
    // And the twin, stepped the same way, agrees with it all through the
    // landing of the first wave and the fight after it.
    twin.step(&[]);
    assert_eq!(world_checksum(&world), world_checksum(&twin));
    for _ in 0..400 {
        world.step(&[]);
        twin.step(&[]);
        assert_eq!(
            world_checksum(&world),
            world_checksum(&twin),
            "two worlds parted during the attack"
        );
    }
    assert!(world.defense(id).is_some());
}

/// Machine `i` of the town's wave moved to where crew member 0 stands —
/// well within the vicinity — hit last by crew member 0, and destroyed.
fn down_by_crew_member_0(world: &mut World, i: usize) {
    let at = world.aboard.room.bim_pos(0);
    let station = world
        .aboard
        .to_station(worldgen::math::dvec2(at.x as f64, at.y as f64))
        .expect("the rooms joined");
    let residents = world.residents.as_mut().expect("the town's room");
    let bims = residents.aboard.room.crew_count() as usize;
    let there = residents.aboard.to_room(station);
    let droid = residents
        .aboard
        .room
        .droid_mut_for_probe(i)
        .expect("a machine of the wave");
    droid.pos = there;
    residents.last_hit_by[bims + i] = Some(0);
    residents
        .aboard
        .room
        .strike_droid(i, bims::droid::DroidPart::Chassis, 1e6);
}

/// A machine taken down in a town's defence is experience, as one at a
/// station the machines hold is: `XP_ENEMY_DOWN` (fifteen) to
/// every classed crew member within the vicinity, once. A townsperson
/// going down is nobody's. And a soldier's *rampage* counts it while the
/// rest of the wave stands, since a defended town's machines are the
/// crew's enemies. (Until the fix after feature 104 the experience was
/// asked of a hostile station alone, and a defended town is friendly.)
#[test]
fn a_machine_downed_in_a_town_s_defence_is_experience_and_a_townsperson_is_not() {
    let Some((mut world, _)) = a_threatened_town(1.0, 2, Some(2)) else {
        return;
    };
    assert_eq!(world.set_class(0, Class::Commander), Ok(()));
    assert!(
        until(&mut world, 40, |w| w.droids_standing() > 0),
        "the first wave never landed"
    );
    // A townsperson down, and not the guard: nothing.
    let xp = world.progress_of(0).xp;
    let bims = world.residents.as_ref().unwrap().aboard.room.crew_count() as usize;
    assert!(bims > 1, "a town with people in it");
    let townsperson = (0..bims)
        .rev()
        .find(|&who| who != surface::GUARD as usize)
        .expect("somebody but the guard");
    world
        .residents
        .as_mut()
        .unwrap()
        .aboard
        .room
        .kill_for_probe(townsperson);
    for _ in 0..3 {
        world.step(&[]);
    }
    assert_eq!(world.progress_of(0).xp, xp, "a townsperson is nobody's");
    // A machine down by the crew member: the down, once; its death nothing more.
    down_by_crew_member_0(&mut world, 0);
    world.step(&[]);
    let paid = class::XP_ENEMY_DOWN;
    assert_eq!(
        world.progress_of(0).xp,
        xp + paid,
        "a machine is experience"
    );
    for _ in 0..3 {
        world.step(&[]);
    }
    assert_eq!(world.progress_of(0).xp, xp + paid, "and only once");

    // A soldier's Rampage at its fourth rank (task 124): a machine it
    // downs in the town is a second more of it, as it is anywhere.
    let Some((mut world, _)) = a_threatened_town(1.0, 2, Some(2)) else {
        return;
    };
    assert_eq!(world.set_class(0, Class::Soldier), Ok(()));
    let mut events = Vec::new();
    world.award(0, class::LEVEL_XP[15], &mut events);
    world.set_ranks_for_probe(0, [0, 0, 0, 4]);
    assert!(
        until(&mut world, 40, |w| w.droids_standing() > 1),
        "the first wave never landed"
    );
    world.step(&[Command::Rampage { slot: 0 }]);
    assert!(world.is_rampaging(0));
    down_by_crew_member_0(&mut world, 0);
    world.step(&[]);
    assert_eq!(
        world.soldier_of(0).extended,
        1.0,
        "a second more in the town"
    );
}

/// **A town held is its own system's** (feature 111's first fix): station
/// ids are only unique within a system, so a held town carried across a
/// jump held the next system's town of the same id — friendly for good,
/// never threatened, never taken. The defences and the towns held go onto
/// the system's memory with the machines' own hold, and come back with it.
#[test]
fn a_town_held_in_one_system_is_not_held_in_the_next() {
    let Some((mut world, id)) = a_threatened_town(0.1, 1, Some(2)) else {
        return;
    };
    assert!(
        until(&mut world, 40, |w| w.droids_standing() > 0),
        "the first wave never landed"
    );
    destroy_the_wave(&mut world);
    assert!(
        until(&mut world, 20, |w| w.town_held(id)),
        "the last wave destroyed and the town not held"
    );
    assert!(world.defense(id).is_some_and(|d| d.won));
    let home = world.star_id;

    // Away down a lane: the same id there is nobody's town the crew held,
    // and no fight was ever fought over it.
    let next = *world
        .galaxy()
        .lanes(home)
        .first()
        .expect("a star with no lane");
    world.undock_for_probe();
    let mut events = Vec::new();
    assert!(world.jump(next, &mut events), "the ship never jumped");
    assert!(!world.town_held(id), "a town held next door was held here");
    assert!(world.held_towns().is_empty());
    assert!(
        world.defense(id).is_none(),
        "the last system's fight came along"
    );
    assert!(world.defenses().is_empty());
    // And the map's quote back home reads that system's memory.
    let quote = world
        .travel_quote(crate::run::Site {
            star: home,
            station: id,
        })
        .expect("a trip back down the lane");
    assert!(quote.cleared, "the town held there is cleared on the map");
    assert!(!quote.threatened, "and not threatened");

    // Back again: the town is held, as it was left.
    assert!(world.jump(home, &mut events), "the ship never jumped back");
    assert!(world.town_held(id), "the held town was forgotten");
    assert!(world.defense(id).is_some_and(|d| d.won));
    assert_eq!(world.held_towns(), &[id]);
}

// --- every site a defence from day one (task 111) ---------------------------

/// The spawn with the game's own rules: not a trader and nobody's enemy,
/// so a defence from the first step. A wave of `size`, `waves` of them.
fn a_station_defence(waves: u32, size: u32) -> (World, u32) {
    let mut world = basic();
    world.set_droid_waves_for_probe(waves);
    world.set_droid_wave_for_probe(size);
    let id = world.ship.state.station().expect("docked at the spawn");
    assert_eq!(world.site_kind(id), crate::run::SiteKind::Defend);
    assert!(world.site_threatened(id), "the machines come for the spawn");
    (world, id)
}

/// A site's defenders are its militia (October 2026): the world hands
/// their room `Outfit::Defender` every step, and nobody else wears it.
#[test]
fn a_site_s_defenders_wear_the_militia_s_kit_and_its_own_people_do_not() {
    use bims::character::Outfit;
    let (mut world, _) = a_station_defence(1, 2);
    world.step(&[]);
    let residents = world.residents.as_ref().expect("the station's room");
    let room = &residents.aboard.room;
    let mut defenders = 0;
    for who in 0..room.crew_count() as usize {
        if residents.is_defender(who) {
            defenders += 1;
            assert_eq!(room.outfit(who), Outfit::Defender, "defender {who}");
        } else {
            assert_ne!(room.outfit(who), Outfit::Defender, "{who} is no defender");
        }
    }
    assert!(defenders > 0);
    // And none of the crew.
    for who in 0..world.aboard.crew_count() as usize {
        assert_ne!(world.aboard.room.outfit(who), Outfit::Defender);
    }
}

/// **A station is defended as a town is** (task 111): the countdown is
/// `DEFENSE_DELAY_STEPS`, the crew are stood on the station's deck the
/// step it starts, the waves come in at the airlock farthest from the
/// crew's, the defenders fight rather than shelter, and the last machine
/// down clears the site — paying its bounty, which task 136 had taken
/// away and the player asked back (`data::DEFENSE_BOUNTY_PERCENT`) — with nobody joining the crew, and nothing held for good.
#[test]
fn a_station_defence_counts_down_lands_at_the_far_airlock_and_pays_on_the_win() {
    assert_eq!(data::DEFENSE_DELAY_STEPS, 300, "five seconds at 1x");
    let (mut world, id) = a_station_defence(1, 2);
    let crew = world.aboard.crew_count();
    world.step(&[]);
    let d = world
        .defense(id)
        .expect("the defence starts at the first step");
    assert_eq!(d.wave, 0);
    assert_eq!(d.next_in, Some(data::DEFENSE_DELAY_STEPS - 1));
    // Every crew member on its feet is ashore, on the station's deck.
    for who in 0..crew {
        assert!(!world.inside_ship(who), "{who} still aboard");
    }
    // The defenders: the day's number, after the station's own people.
    let residents = world.residents.as_ref().expect("the station's room");
    let fielded = residents.defender.iter().filter(|&&d| d).count() as u32;
    assert_eq!(fielded, world.area_now().1.defenders);
    assert!(fielded > 0);
    // The countdown runs out on the step it says.
    let mut steps = 1;
    while world.droids_standing() == 0 {
        world.step(&[]);
        steps += 1;
        assert!(steps <= data::DEFENSE_DELAY_STEPS + 2, "no wave by {steps}");
    }
    assert!(steps >= data::DEFENSE_DELAY_STEPS, "a wave at {steps}");
    // At the far airlock.
    let residents = world.residents.as_ref().unwrap();
    let station = world.station(id).unwrap();
    let port = crate::droid::arrival_airlock_at(&station.design, id, 1).expect("an airlock");
    let spot = crate::droid::inside_of(&port, data::ASHORE_TILES);
    let spot = residents
        .aboard
        .to_room(worldgen::math::dvec2(spot.0, spot.1));
    let room = &residents.aboard.room;
    let nearest = (0..room.droid_count() as usize)
        .filter_map(|i| room.droid(i))
        .map(|d| (d.pos - spot).len())
        .fold(f32::MAX, f32::min);
    assert!(
        nearest < 6.0 * shipdesign::TILE as f32,
        "the wave landed {nearest} from the far airlock"
    );
    // The defenders take arms; the station's own people shelter.
    let bims = room.crew_count() as usize;
    for who in 0..bims {
        if residents.is_defender(who) {
            assert!(!room.is_sheltering(who), "defender {who} sheltering");
        }
    }
    // The win, and the bounty held for it paid.
    let money = world.money;
    destroy_the_wave(&mut world);
    let mut events = Vec::new();
    for _ in 0..20 {
        events.extend(world.step(&[]));
        if world.defense(id).is_some_and(|d| d.won) {
            break;
        }
    }
    events.extend(world.step(&[]));
    assert!(world.defense(id).is_some_and(|d| d.won), "the defence won");
    assert!(world.site_cleared(id));
    assert!(!world.site_threatened(id), "held is threatened no more");
    assert!(world.money > money, "a defence pays its bounty");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Bounty { .. })),
        "no bounty said at a defence"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::TownHeld { station } if *station == id))
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::TownsfolkJoined { .. })),
        "nobody joins from a station"
    );
    assert_eq!(world.aboard.crew_count(), crew);
    assert!(!world.town_held(id), "a station held is not held for good");
    // **And the crisis may take it later**: a won defence is not immune,
    // and its fight stays won.
    world.infest(id);
    assert!(world.is_droid_held(id), "the crisis took the held station");
    assert_eq!(world.site_kind(id), crate::run::SiteKind::Attack);
    assert!(world.defense(id).is_some_and(|d| d.won && !d.lost));
}

/// **A defence's waves are ten seconds apart**: the last machine of one
/// down, the next lands `DEFENSE_REINFORCE_STEPS` later — not the fifteen
/// seconds (`DROID_REINFORCE_STEPS`) of an attacked station's.
#[test]
fn a_defence_s_next_wave_lands_ten_seconds_after_the_last_is_down() {
    assert_eq!(data::DEFENSE_REINFORCE_STEPS, 600, "ten seconds at 1x");
    let (mut world, id) = a_station_defence(2, 2);
    let landed = until(&mut world, data::DEFENSE_DELAY_STEPS as u32 + 2, |w| {
        w.droids_standing() > 0
    });
    assert!(landed, "no first wave");
    destroy_the_wave(&mut world);
    let mut steps = 0;
    while world.droids_standing() == 0 {
        world.step(&[]);
        steps += 1;
        if steps == 2 {
            let left = world.defense(id).and_then(|d| d.next_in);
            assert!(
                left.is_some_and(|left| left <= data::DEFENSE_REINFORCE_STEPS),
                "the countdown reads {left:?}"
            );
        }
        assert!(
            steps <= data::DEFENSE_REINFORCE_STEPS + 2,
            "no wave by {steps}"
        );
    }
    assert!(steps >= data::DEFENSE_REINFORCE_STEPS, "a wave at {steps}");
    assert_eq!(world.defense(id).map(|d| d.wave), Some(2));
}

/// **Every bot brings its machines** (task 147): a defence's wave is the
/// players' share with `⌈enemies per bot × bots⌉` on top — the crew's
/// bots and the site's defenders, a decimal rounded up — and anywhere
/// else the players' share and the crew's bots alone.
#[test]
fn the_wave_at_a_defence_is_the_wave_with_a_machine_for_each_defender() {
    let mut world = basic();
    world.step(&[]);
    let n = world.defenders_fielded() + world.crew_bots();
    assert!(world.defenders_fielded() > 0, "defenders fielded");
    let day = world.run_day();
    let players = |w: &World| w.scaling().size(w.players(), 0, day);
    assert_eq!(world.droid_wave_size(), players(&world) + n);
    world.set_wave_scaling(crate::droid::WaveScaling {
        enemies_per_bot: 1.5,
        ..crate::droid::WaveScaling::DEFAULT
    });
    assert_eq!(
        world.droid_wave_size(),
        players(&world) + (3 * n).div_ceil(2)
    );
    let mut quiet = basic();
    quiet.set_quiet_sites_for_probe(true);
    quiet.step(&[]);
    assert_eq!(quiet.defenders_fielded(), 0, "no defenders at a quiet site");
    assert_eq!(
        quiet.droid_wave_size(),
        (players(&quiet) + quiet.crew_bots()).max(1)
    );
}

/// **The defenders keep pace with the machines**: each is armed at the
/// tier the run day deals the enemies (`World::defender_tiers`, the
/// machines' own shares) — a hired hand's kit lifted to it — so at the
/// first day's tier one they carry exactly what they always did; in the
/// tier-two zone every one carries a tier-two gun and wears tier-two
/// armour, the kit's one-in-two roll for armour or not (October 2026), and
/// with every tier timing at nought the same at tier three.
#[test]
fn the_defenders_are_armed_at_the_day_s_tier() {
    use bims::combat::{Gear, Tier};
    let defenders = |world: &World| -> Vec<(usize, Gear)> {
        let residents = world.residents.as_ref().expect("the station's room");
        (0..residents.defender.len())
            .filter(|&who| residents.is_defender(who))
            .map(|who| (who, residents.aboard.room.gear(who)))
            .collect()
    };
    let mut world = basic();
    world.step(&[]);
    let first = defenders(&world);
    assert!(!first.is_empty(), "defenders fielded");
    assert!(
        world
            .defender_tiers(first.len() as u32)
            .iter()
            .all(|&t| t == Tier::One)
    );
    for (who, gear) in &first {
        assert_eq!(
            gear.weapon.map(|w| w.tier),
            Some(gear.weapon.unwrap().kind.min_tier()),
            "{who}"
        );
    }
    // The same site in the tier-two zone and then the tier-three one, the
    // room opened again under the new dials before the defence begins (a
    // defence running keeps its room).
    assert!(
        first.iter().any(|(_, gear)| gear.armour.is_none()),
        "a tier-one defender may go unarmoured, so the test shows the lift"
    );
    for tier in [Tier::Two, Tier::Three] {
        let mut world = basic();
        if tier == Tier::Two {
            // Tier two's door on day one.
            world.set_wave_scaling(crate::droid::WaveScaling::with_tier_days(1, 1_000));
        } else {
            world.set_droid_tier_for_probe(Some(Tier::Three));
        }
        world.set_quiet_sites_for_probe(true);
        world.set_quiet_sites_for_probe(false);
        world.step(&[]);
        assert_eq!(world.zone_tier(), tier);
        // The area's defenders, the first of them the same kits lifted.
        let lifted = defenders(&world);
        assert!(lifted.len() >= first.len());
        for ((who, before), (_, after)) in first.iter().zip(&lifted) {
            let weapon = after.weapon.expect("armed");
            assert_eq!(weapon.kind, before.weapon.unwrap().kind, "{who}'s gun");
            assert_eq!(weapon.tier, tier.max(weapon.kind.min_tier()), "{who}'s gun");
            let piece = after.armour.expect("armoured from tier two");
            assert_eq!(piece.tier, tier, "{who}'s armour");
        }
    }
}

/// **A defender is nobody's loss** (task 111): one dead is not in the
/// station's losses, and a derelict — nobody of its own — defended by its
/// defenders alone is not lost when every one of them is dead.
#[test]
fn a_dead_defender_is_no_loss_and_a_derelict_is_not_lost_with_its_defenders() {
    let mut world = basic();
    let id = world.ship.state.station().expect("docked");
    assert!(
        world.regenerate_dock_for_probe(7, Some(worldgen::StationKind::Derelict)),
        "the dock made a derelict"
    );
    assert_eq!(world.site_kind(id), crate::run::SiteKind::Defend);
    world.step(&[]);
    assert!(world.defense(id).is_some_and(|d| !d.over()), "defended");
    let residents = world.residents.as_mut().expect("the derelict's room");
    let bims = residents.aboard.room.crew_count() as usize;
    assert!(bims > 0, "defenders stand on a derelict");
    for who in 0..bims {
        assert!(residents.is_defender(who), "{who} is somebody's own");
        residents.aboard.room.kill_now(who);
    }
    for _ in 0..10 {
        world.step(&[]);
    }
    assert!(
        world.defense(id).is_some_and(|d| !d.lost),
        "lost with nobody of its own to lose"
    );
    assert!(!world.is_droid_held(id));
    // Their deaths are nobody's loss when the room closes.
    world.leave_for_probe();
    assert_eq!(world.losses_at(id).dead, 0, "a defender counted a loss");
}

/// **Leaving before the last wave** (task 111): the station falls to the
/// machines as a town does, the bounty it was holding is dropped, and
/// nothing else is given.
#[test]
fn leaving_a_station_defence_early_gives_it_to_the_machines_and_pays_nothing() {
    // The delay cut before the first step, which is when the defence
    // starts and takes it.
    let (mut world, id) = a_station_defence(2, 2);
    world.set_defense_delay_for_probe(data::STEP_MINUTES * 4.0);
    assert!(
        until(&mut world, 40, |w| w.droids_standing() > 0),
        "the first wave never landed"
    );
    // One machine down, and its bounty held until the site is cleared.
    if let Some(residents) = world.residents.as_mut() {
        for _ in 0..60 {
            residents
                .aboard
                .room
                .strike_droid(0, bims::droid::DroidPart::Chassis, 100.0);
        }
    }
    until(&mut world, 5, |w| w.droids_standing() < 2);
    assert!(world.droids_standing() < 2, "the machine went down");
    assert!(world.run.pending_bounty > 0, "no bounty held at a defence");
    let money = world.money;
    let events = world.leave_for_probe();
    assert!(world.is_droid_held(id), "the machines have it");
    assert!(world.defense(id).is_some_and(|d| d.lost));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::TownFell { station } if *station == id))
    );
    // No bounty paid — the pool may fall by the players left ashore and
    // their respawns (a bot costs nothing), and by nothing it gains.
    assert!(world.money <= money);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::Bounty { .. })),
        "a bounty paid for a site left"
    );
    assert_eq!(world.run.pending_bounty, 0, "the bounty dropped");
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::TownsfolkJoined { .. }))
    );
}

/// A townsperson downed fighting for its town is picked up with the
/// medkit like a crewmate: the crew's room is told it may be revived
/// (`GUEST + i`), a crew member ordered to it walks over and kneels, its
/// countdown stands while the hands are on it, and it gets up in its own
/// room — said as `ResidentRevived`.
#[test]
fn a_townsperson_downed_is_picked_up_with_the_medkit() {
    let Some((mut world, id)) = a_threatened_town(1.0, 2, None) else {
        return;
    };
    world.step(&[]);
    world.aboard.room.set_autonomous(false);
    let guard = surface::GUARD as usize;
    // The guard down a tile from crew member 0.
    let near = world.aboard.room.bim_pos(0) + bims::math::vec2(shipdesign::TILE as f32, 0.0);
    let station = world
        .aboard
        .to_station(worldgen::math::dvec2(near.x as f64, near.y as f64))
        .expect("the rooms joined");
    {
        let residents = world.residents.as_mut().expect("the town's room");
        let there = residents.aboard.to_room(station);
        residents.aboard.room.put_for_probe(guard, there);
        residents.aboard.room.knock_out_for_probe(guard);
    }
    world.step(&[]);
    world.step(&[]);
    let patient = bims::game::GUEST + guard;
    let room = &world.residents.as_ref().unwrap().aboard.room;
    assert!(room.is_downed(guard), "the guard is down");
    assert!(
        world.aboard.room.is_revivable_guest(patient),
        "the crew's room may pick the guard up"
    );
    assert!(world.aboard.room.guest_pos(patient).is_some());

    world.step(&[Command::Crew {
        slot: 0,
        order: bims::order::CrewOrder::Revive {
            who: 0,
            patient: patient as u32,
        },
    }]);
    assert_eq!(world.aboard.room.reviving(0), Some(patient));
    // Hands on it: the countdown stands.
    assert!(
        until(&mut world, 400, |w| w
            .aboard
            .room
            .revive_share(patient)
            .is_some()),
        "crew member 0 never knelt at the guard"
    );
    world.step(&[]);
    let left = |w: &World| w.residents.as_ref().unwrap().aboard.room.down_left(guard);
    let was = left(&world).expect("still down");
    for _ in 0..10 {
        world.step(&[]);
    }
    assert_eq!(left(&world), Some(was), "the countdown ran under the hands");
    // And it gets up in its own room, said once.
    let mut said = false;
    for _ in 0..2_000 {
        let events = world.step(&[]);
        said |= events.iter().any(|e| {
            matches!(e, WorldEvent::ResidentRevived { station, who, by: 0 }
                if *station == id && *who == guard as u32)
        });
        if said {
            break;
        }
    }
    assert!(said, "no ResidentRevived");
    let room = &world.residents.as_ref().unwrap().aboard.room;
    assert!(
        !room.is_downed(guard) && room.is_alive(guard),
        "the guard is up"
    );
    // The crew's room hears it the next step, as it hears every visitor.
    world.step(&[]);
    assert!(!world.aboard.room.is_revivable_guest(patient));
}

/// **A defence is attacked by the day's mix** (task 131; the scaling's
/// areas since October 2026): their people alone through area 0, the
/// machines beside them through the tier-one area — about half halfway —
/// and from tier two's door the machines alone. A Manufacturer among the site's own people is an
/// *intruder*: the crew's target and the site's, never the machines'.
mod manufacturers_attack {
    use super::*;

    /// The spawn, a defence from the first step (task 111), on `day`, with
    /// its fight cut to one wave of `size` a few steps off — and the game's
    /// own rule for who comes, which `basic` takes away.
    fn at_the_spawn(day: u32, size: u32) -> (World, u32) {
        let mut world = open_simulation_world(flyer(2), REFERENCE_MONEY, 2);
        world.set_day_for_probe(day);
        world.set_droid_waves_for_probe(1);
        world.set_droid_wave_for_probe(size);
        world.set_defense_delay_for_probe(data::STEP_MINUTES * 4.0);
        let id = world.ship.state.station().expect("docked at the spawn");
        assert!(world.site_threatened(id), "the spawn is a defence");
        (world, id)
    }

    /// Step until the wave is on the deck, and answer how many of it are
    /// their people and how many machines.
    fn the_wave(world: &mut World) -> (usize, usize) {
        assert!(
            until(world, 60, |w| w.droids_standing() > 0),
            "the wave never landed"
        );
        let room = &world
            .residents
            .as_ref()
            .expect("the site's room")
            .aboard
            .room;
        let people = (0..room.crew_count() as usize)
            .filter(|&who| room.is_manufacturer(who))
            .inspect(|&who| assert!(room.is_intruder(who), "a Manufacturer among friends"))
            .count();
        (people, room.droid_count() as usize)
    }

    #[test]
    fn on_day_nought_the_wave_is_their_people_alone() {
        let (mut world, _) = at_the_spawn(0, 6);
        assert_eq!(world.machines_of(6, 1), 0, "area 0");
        assert_eq!(the_wave(&mut world), (6, 0), "no Trooper on day nought");
        assert_eq!(world.droids_standing(), 6, "their people count as standing");
    }

    #[test]
    fn in_tier_one_machines_stand_beside_them_and_from_tier_two_the_machines_alone() {
        let (mut world, _) = at_the_spawn(12, 16);
        let (people, machines) = the_wave(&mut world);
        assert_eq!(people + machines, 16);
        assert!(
            machines > 0 && people > 0,
            "half and half in the tier-one area, near enough: {people} people, {machines} machines"
        );

        let door = crate::droid::WaveScaling::DEFAULT.tier_two_day();
        let (mut world, _) = at_the_spawn(door, 16);
        assert_eq!(world.machines_of(16, 1), 16);
        let (people, machines) = the_wave(&mut world);
        assert_eq!(people, 0, "the machines alone from tier two's door");
        assert!(machines >= 16, "with the area's on top: {machines}");
    }

    #[test]
    fn the_crew_and_the_site_aim_at_an_intruder_and_the_machines_never_do() {
        let (mut world, _) = at_the_spawn(8, 16);
        the_wave(&mut world);
        world.step(&[]);
        let residents = world.residents.as_ref().expect("the site's room");
        let room = &residents.aboard.room;
        let intruders: Vec<usize> = (0..room.crew_count() as usize)
            .filter(|&who| room.is_intruder(who))
            .collect();
        assert!(!intruders.is_empty());
        // None of them shelters, and none is the site's own.
        for &who in &intruders {
            assert!(!room.is_sheltering(who), "an intruder runs into no house");
        }
        // The crew's targets are the residents' whole index space: an
        // intruder on its feet is on it, a townsperson never.
        let targets = world.aboard.room.combat_targets_for_probe();
        assert!(targets.len() >= room.body_count() as usize);
        for who in 0..room.crew_count() as usize {
            assert_eq!(
                targets[who].is_some(),
                room.is_intruder(who) && room.is_alive(who) && !room.is_downed(who),
                "the crew's target {who}"
            );
        }
        // The site's own people aim at them too: their list is the same
        // index space.
        let theirs = room.combat_targets_for_probe();
        for &who in &intruders {
            assert!(theirs[who].is_some(), "the site aims at intruder {who}");
        }
    }

    #[test]
    fn a_wave_of_them_fought_down_clears_the_site_and_pays() {
        let (mut world, id) = at_the_spawn(0, 2);
        the_wave(&mut world);
        // Their people down where they stand: the win waits for each to be
        // counted at its first down, not for it to bleed out.
        let bims = world.residents.as_ref().unwrap().aboard.room.crew_count() as usize;
        for who in 0..bims {
            let room = &mut world.residents.as_mut().unwrap().aboard.room;
            if room.is_intruder(who) {
                room.set_health_for_probe(who, 0.0);
            }
        }
        assert!(
            until(&mut world, 60, |w| w.defense(id).is_some_and(|d| d.won)),
            "the site was never cleared"
        );
        assert_eq!(world.droids_standing(), 0);
    }

    #[test]
    fn two_worlds_meet_the_same_manufacturers() {
        let run = || {
            let (mut world, _) = at_the_spawn(8, 8);
            for _ in 0..600 {
                world.step(&[]);
            }
            world_checksum(&world)
        };
        assert_eq!(run(), run());
    }
}

/// **A defender on its feet keeps the run going**: every crew member
/// down is not the run lost at once while one of the site's defenders
/// still stands to hold the machines off; once none does, it is.
#[test]
fn the_run_is_not_lost_at_once_while_a_defender_stands() {
    let (mut world, _) = a_station_defence(1, 2);
    world.step(&[]);
    assert!(world.defender_standing(), "the site's defenders are up");
    for who in 0..world.aboard.crew_count() as usize {
        world.aboard.room.knock_out_for_probe(who);
    }
    for _ in 0..5 {
        world.step(&[]);
    }
    let crew = world.aboard.crew_count() as usize;
    assert!((0..crew).all(|who| world.aboard.room.is_downed(who)));
    assert!(!world.lost, "a defender stands");
    let residents = world.residents.as_mut().expect("the station's room");
    for who in 0..residents.aboard.room.crew_count() as usize {
        if residents.is_defender(who) {
            residents.aboard.room.kill_now(who);
        }
    }
    let lost = (0..5).any(|_| {
        world
            .step(&[])
            .iter()
            .any(|e| matches!(e, WorldEvent::CrewLost))
    });
    assert!(lost && world.lost, "nobody stands");
    assert!((0..crew).all(|who| world.aboard.room.is_alive(who)));
}

/// A defence's **bonus wave** (October 2026), chosen in the ready check:
/// the site's own wave, then one half as big again, and won only with
/// that down.
#[test]
fn a_defence_s_bonus_wave_comes_last_half_as_big_again() {
    let (mut world, id) = a_station_defence(1, 2);
    world.set_ready_check(true);
    assert!(world.awaiting_ready());
    world.step(&[crate::world::Command::BonusWave { slot: 0, on: true }]);
    world.step(&[crate::world::Command::Ready { slot: 0, yes: true }]);
    world.step(&[crate::world::Command::Ready { slot: 1, yes: true }]);
    assert!(!world.awaiting_ready(), "under way");
    assert!(until(&mut world, 4_000, |w| w.droids_standing() == 2));
    destroy_the_wave(&mut world);
    assert!(
        until(&mut world, 4_000, |w| w.droids_standing() > 0),
        "the bonus wave lands"
    );
    assert!(!world.defense(id).unwrap().won, "not won before it");
    assert_eq!(world.droids_standing(), 3, "half as big again, rounded up");
    assert_eq!(world.bonus_wave_here(), crate::run::BonusWave::Landed);
    destroy_the_wave(&mut world);
    assert!(until(&mut world, 200, |w| w.defense(id).unwrap().won));
    assert_eq!(world.bonus_wave_here(), crate::run::BonusWave::Done);
}

/// **A station whose own people all die mid-wave fights on** (October
/// 2026): it was the site lost, `infest` building the room afresh — the
/// wave being fought gone, and an attack's fresh first wave landed in its
/// place with nothing said. The machines standing stay where they are and
/// nothing lands while they do.
#[test]
fn a_station_whose_people_die_mid_wave_keeps_its_wave_and_its_defence() {
    let (mut world, id) = a_station_defence(3, 2);
    world.step(&[]);
    while world.droids_standing() == 0 {
        world.step(&[]);
    }
    let machines = |w: &World| {
        let room = &w
            .residents
            .as_ref()
            .expect("the station's room")
            .aboard
            .room;
        (0..room.droid_count() as usize)
            .filter_map(|i| room.droid(i))
            .map(|d| (d.wave, d.destroyed))
            .collect::<Vec<_>>()
    };
    let landed = machines(&world);
    let residents = world.residents.as_mut().expect("the station's room");
    for who in 0..residents.aboard.room.crew_count() as usize {
        if residents.is_own(who) {
            residents.aboard.room.kill_now(who);
        }
    }
    for _ in 0..30 {
        let events = world.step(&[]);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, WorldEvent::DroidReinforcements { .. })),
            "a wave landed while the first stood"
        );
    }
    assert!(!world.is_droid_held(id), "the station fell");
    assert_eq!(world.site_kind(id), crate::run::SiteKind::Defend);
    assert!(world.defense(id).is_some_and(|d| !d.lost && d.wave == 1));
    assert_eq!(machines(&world).len(), landed.len(), "the wave was swapped");
    assert!(world.droids_standing() > 0);
}

// --- Seal the breaches (October 2026) ---------------------------------------

/// The spawn's defence as **Seal the breaches**: its ways in but the port
/// are its breaches.
fn breaches(waves_of: u32) -> (World, u32) {
    breaches_after(waves_of, data::DEFENSE_DELAY_STEPS)
}

/// [`breaches`] with the first wave `delay` steps off.
fn breaches_after(waves_of: u32, delay: u64) -> (World, u32) {
    let (mut world, id) = a_station_defence(1, waves_of);
    world.set_defense_delay_for_probe(delay as f64 * data::STEP_MINUTES);
    world.set_mission_for_probe(Some(crate::run::Mission::Breaches));
    world.step(&[]);
    assert!(
        world.is_breaches(id),
        "the defence began as Seal the breaches"
    );
    (world, id)
}

/// The waves come on a clock while a breach is open, stacking on those
/// still standing; each is the smaller for every breach welded; with
/// every one welded none comes, and the fight is won once what is aboard
/// is destroyed.
#[test]
fn the_waves_come_while_a_breach_is_open_and_the_fight_is_won_once_all_are_welded() {
    let (mut world, id) = breaches(6);
    let (open, total) = world.breaches_open().expect("breaches");
    assert!(total >= 2, "the spawn has two ways in or more but its port");
    assert_eq!(open, total);
    // The first wave after the delay.
    assert!(until(
        &mut world,
        data::DEFENSE_DELAY_STEPS as u32 + 5,
        |w| w.droids_standing() > 0
    ));
    let first = world.droids_standing();
    assert_eq!(first, 6);
    // The next lands on the clock with the first still standing.
    let wave = |w: &World| w.defense(id).unwrap().wave;
    assert!(until(
        &mut world,
        data::BREACH_WAVE_STEPS as u32 + 5,
        |w| wave(w) == 2
    ));
    let laid = |w: &World| w.residents.as_ref().unwrap().aboard.room.droid_count();
    assert_eq!(laid(&world), 12, "stacked on the first, wrecks and all");
    // One breach welded: the next wave is its share of the open ones.
    let one = world.entries()[0].index;
    world.weld_for_probe(one);
    world.step(&[]);
    assert_eq!(world.breaches_open(), Some((total - 1, total)));
    let before = laid(&world);
    assert!(until(
        &mut world,
        data::BREACH_WAVE_STEPS as u32 + 5,
        |w| wave(w) == 3
    ));
    assert_eq!(
        laid(&world) - before,
        (6 * (total - 1)).div_ceil(total),
        "a smaller wave"
    );
    // Every one welded: no wave comes, none burns through, and the fight is
    // won with the deck cleared.
    for e in world.entries() {
        world.weld_for_probe(e.index);
    }
    let mut events = Vec::new();
    for _ in 0..data::BREACH_WAVE_STEPS + 10 {
        events.extend(world.step(&[]));
    }
    assert_eq!(wave(&world), 3, "no wave with every breach welded");
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::WeldBurnt { .. }))
    );
    assert!(!world.defense(id).unwrap().won, "not while machines stand");
    destroy_the_wave(&mut world);
    assert!(until(&mut world, 20, |w| w
        .defense(id)
        .is_some_and(|d| d.won)));
}

/// A breach takes `BREACH_WELD_SECONDS` of hands on it, and two Bims
/// welding it together take half as long; the bar reads the share.
#[test]
fn two_bims_weld_a_breach_twice_as_fast() {
    let weld_with = |hands: u32| -> u32 {
        // No wave while they work.
        let (mut world, _id) = breaches_after(2, 100_000);
        let way = world.entries()[0];
        let t = shipdesign::TILE as f32;
        let at = way.at - way.outward * (2.0 * t);
        for slot in 0..hands {
            world.aboard.room.stand_at(slot as usize, at);
        }
        let orders: Vec<Command> = (0..hands).map(|slot| Command::Weld { slot }).collect();
        let events = world.step(&orders);
        assert!(
            events
                .iter()
                .filter(|e| matches!(e, WorldEvent::Welding { done: false, .. }))
                .count()
                == hands as usize,
            "{events:?}"
        );
        for step in 1..5_000 {
            let events = world.step(&[]);

            if step == 300 {
                let share = world.weld_share(way.index);
                assert!(share > 0.0 && share < 1.0, "half way: {share}");
            }
            if events
                .iter()
                .any(|e| matches!(e, WorldEvent::Welding { done: true, .. }))
            {
                assert!(
                    world
                        .entries()
                        .iter()
                        .any(|e| e.index == way.index && e.welded)
                );
                assert!(
                    (0..hands).all(|who| world.aboard.room.deploy_work(who as usize).is_none()),
                    "every hand let go"
                );
                return step;
            }
        }
        panic!("never welded with {hands}");
    };
    let one = weld_with(1);
    let two = weld_with(2);
    let full = data::BREACH_WELD_SECONDS * 60;
    assert!(one + 2 >= full && one <= full + 30, "one Bim: {one} steps");
    assert!(
        two * 2 + 4 >= full && two <= full / 2 + 30,
        "two Bims: {two} steps"
    );
}

// --- Evacuation (October 2026) ----------------------------------------------

/// The spawn's defence as an **Evacuation**, its first wave far off.
fn evacuation(delay: u64) -> (World, u32) {
    let (mut world, id) = a_station_defence(1, 2);
    world.set_defense_delay_for_probe(delay as f64 * data::STEP_MINUTES);
    world.set_mission_for_probe(Some(crate::run::Mission::Evacuation));
    world.step(&[]);
    assert!(
        world.defense(id).is_some_and(|d| d.evacuation.is_some()),
        "the defence began as an Evacuation"
    );
    (world, id)
}

fn flag_on_deck(world: &World) -> bims::math::Vec2 {
    let look = world.evacuation_look().expect("an Evacuation");
    let p = world.aboard.from_station(look.flag).expect("on the deck");
    bims::math::vec2(p.x as f32, p.y as f32)
}

/// The site's people and its refugees follow the flag a player carries to
/// the ship, and the defence is held with every one of them aboard.
#[test]
fn an_evacuation_follows_the_flag_to_the_ship_and_is_held() {
    let (mut world, id) = evacuation(100_000);
    let look = world.evacuation_look().unwrap();
    assert!(
        look.total >= data::EVACUEES,
        "the refugees and the site's own"
    );
    assert!(!look.carried);
    // Taken up from beside it.
    let flag = flag_on_deck(&world);
    world.aboard.room.stand_at(0, flag);
    let events = world.step(&[Command::Flag { slot: 0 }]);
    assert!(
        events.iter().any(|e| matches!(
            e,
            WorldEvent::FlagCarried {
                who: 0,
                taken: true
            }
        )),
        "{events:?}"
    );
    // Carried to the ship: they follow, and are aboard.
    let home = world.aboard.gangway.expect("the ship's gangway");
    let home = bims::math::vec2(home.x as f32, home.y as f32);
    let mut held = false;
    for _ in 0..6_000 {
        world.aboard.room.stand_at(0, home);
        let events = world.step(&[]);
        if events
            .iter()
            .any(|e| matches!(e, WorldEvent::TownHeld { station } if *station == id))
        {
            held = true;
            break;
        }
    }
    let look = world.evacuation_look().unwrap();
    assert!(
        held,
        "held: {} of {} aboard, {} alive",
        look.aboard, look.total, look.alive
    );
    assert_eq!(look.aboard, look.alive);
    assert!(world.defense(id).unwrap().won);
}

/// The flag put down holds them where it lies; a player who is not the
/// carrier cannot take it from another; and with every one of them dead
/// the Evacuation fails and the site falls.
#[test]
fn a_dropped_flag_holds_them_there_and_none_left_alive_loses_the_site() {
    let (mut world, id) = evacuation(100_000);
    let flag = flag_on_deck(&world);
    world.aboard.room.stand_at(0, flag);
    world.step(&[Command::Flag { slot: 0 }]);
    assert_eq!(world.can_flag(1), Err(crate::event::Refusal::NoFlagNear));
    // Carried a few tiles and put down.
    let t = shipdesign::TILE as f32;
    let there = flag + bims::math::vec2(0.0, 4.0 * t);
    world.aboard.room.stand_at(0, there);
    world.step(&[]);
    let events = world.step(&[Command::Flag { slot: 0 }]);
    assert!(events.iter().any(|e| matches!(
        e,
        WorldEvent::FlagCarried {
            who: 0,
            taken: false
        }
    )));
    world
        .aboard
        .room
        .stand_at(0, flag - bims::math::vec2(0.0, 6.0 * t));
    for _ in 0..1_200 {
        world.step(&[]);
    }
    let at = flag_on_deck(&world);
    assert!((at - there).len() < t, "it stays where it was put down");
    // Most of them by it.
    let residents = world.residents.as_ref().unwrap();
    let room = &residents.aboard.room;
    let by_it = (0..room.crew_count() as usize)
        .filter(|&who| residents.is_own(who) && room.is_alive(who))
        .filter(|&who| {
            let p = residents.aboard.position(who as u32);
            world
                .aboard
                .from_station(p)
                .is_some_and(|q| (bims::math::vec2(q.x as f32, q.y as f32) - at).len() < 4.0 * t)
        })
        .count();
    assert!(by_it * 2 >= data::EVACUEES as usize, "{by_it} by the flag");
    // Every one of them dead: failed.
    let residents = world.residents.as_mut().unwrap();
    let n = residents.aboard.room.crew_count() as usize;
    for who in 0..n {
        if residents.is_own(who) {
            residents.aboard.room.kill_now(who);
        }
    }
    let events = world.step(&[]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::TownFell { station } if *station == id))
    );
    assert!(world.defense(id).unwrap().lost);
}

/// The defences' missions of October 2026 (`defences.rs`): a defence at
/// the spawn made that mission, the dock laid out for it, and the first
/// wave a step after the defence begins.
fn guarded(mission: crate::run::Mission) -> (World, u32) {
    let (mut world, id) = a_station_defence(1, 4);
    world.set_defense_delay_for_probe(data::STEP_MINUTES);
    world.set_mission_for_probe(Some(mission));
    world.fit_dock_for_probe();
    world.step(&[]);
    assert!(world.guard_now().is_some(), "{mission:?} began");
    (world, id)
}

/// Every machine on the residents' deck destroyed where it stands.
fn wreck_them(world: &mut World) {
    if let Some(residents) = world.residents.as_mut() {
        let room = &mut residents.aboard.room;
        for i in 0..room.droid_count() as usize {
            room.strike_droid(i, bims::droid::DroidPart::Chassis, 1e6);
        }
    }
}

#[test]
fn a_charge_is_defused_by_hand_and_the_timer_out_blows_the_station_and_the_run() {
    use crate::objective::Guard;
    let (mut world, id) = guarded(crate::run::Mission::Bombs);
    let Some(Guard::Bombs(b)) = world.guard_now() else {
        panic!("bombs");
    };
    assert_eq!(b.charges.len() as u32, data::BOMB_CHARGES);
    assert!(
        b.left <= data::BOMB_STEPS && b.left > data::BOMB_STEPS - 60,
        "three minutes"
    );
    let look = world.objective_look().expect("a look");
    let at = look
        .marks
        .iter()
        .find(|m| matches!(m.kind, crate::MarkKind::Charge { defused: false }))
        .map(|m| world.aboard.from_station(m.at).unwrap())
        .unwrap();
    let at = bims::math::vec2(at.x as f32, at.y as f32);
    world
        .aboard
        .room
        .stand_at(0, at + bims::math::vec2(shipdesign::TILE as f32, 0.0));
    assert!(matches!(
        world.can_interact(0),
        Ok(crate::Interaction::Defuse(_))
    ));
    world.step(&[crate::world::Command::Interact { slot: 0 }]);
    let mut defused = false;
    for _ in 0..(data::DEFUSE_SECONDS * 60 + 120) {
        wreck_them(&mut world);
        if world
            .step(&[])
            .iter()
            .any(|e| matches!(e, WorldEvent::Objective { what: 20, .. }))
        {
            defused = true;
            break;
        }
    }
    assert!(defused, "a charge defused by hand");
    // The timer out with one left: the station goes up, and the run.
    world.set_guard_time_for_probe(2);
    let mut blew = false;
    for _ in 0..4 {
        if world
            .step(&[])
            .iter()
            .any(|e| matches!(e, WorldEvent::Objective { what: 21, .. }))
        {
            blew = true;
            break;
        }
    }
    assert!(blew);
    assert!(world.lost, "the run is lost");
    assert!(world.defense(id).unwrap().lost);
}

#[test]
fn the_vault_s_doors_are_sealed_broken_by_an_unbroken_stand_and_the_commander_down_loses_the_run() {
    use crate::objective::Guard;
    let (mut world, id) = guarded(crate::run::Mission::Doors);
    let Some(Guard::Doors(d)) = world.guard_now() else {
        panic!("doors");
    };
    assert_eq!(d.outer.len(), 2, "two outer doors");
    // The commander is in the core, posted, in the commander's kit.
    let vip = d.vip as usize;
    let fitted = world.station(id).unwrap().fitted.clone().unwrap();
    let core = fitted.rooms[1];
    let p = world.aboard.room.bim_pos(vip);
    let q = world
        .aboard
        .to_station(worldgen::math::dvec2(p.x as f64, p.y as f64))
        .unwrap();
    let t = shipdesign::TILE as f64;
    let (tx, ty) = ((q.x / t).floor() as u32, (q.y / t).floor() as u32);
    assert!(
        tx >= core[0] && tx <= core[2] && ty >= core[1] && ty <= core[3],
        "the commander stands in the core: ({tx}, {ty}) in {core:?}"
    );
    assert!(world.is_vip(vip as u32), "the commander");
    // Every door sealed in the crew's room.
    let mid = |world: &World, tile: (u32, u32)| {
        let p = world
            .aboard
            .from_station(worldgen::math::dvec2(
                (tile.0 as f64 + 0.5) * t,
                (tile.1 as f64 + 0.5) * t,
            ))
            .unwrap();
        bims::math::vec2(p.x as f32, p.y as f32)
    };
    for gate in d.outer.iter().chain(std::iter::once(&d.inner)) {
        let door = world
            .aboard
            .room
            .door_index_at(mid(&world, gate.door))
            .expect("a door there");
        assert!(world.aboard.room.door_sealed(door), "sealed");
    }
    // A machine at an outer door for twenty seconds unbroken: broken in.
    let gate = d.outer[0];
    let outside = world
        .residents
        .as_ref()
        .unwrap()
        .aboard
        .to_room(worldgen::math::dvec2(
            (gate.outside.0 as f64 + 0.5) * t,
            (gate.outside.1 as f64 + 0.5) * t,
        ));
    assert!(until(&mut world, 60, |w| w.droids_standing() > 0), "a wave");
    let mut broke = false;
    for _ in 0..(data::DOOR_BREAK_STEPS + 30) {
        if let Some(residents) = world.residents.as_mut()
            && let Some(d) = residents.aboard.room.droid_mut_for_probe(0)
        {
            d.pos = outside;
            d.destroyed = false;
        }
        if world
            .step(&[])
            .iter()
            .any(|e| matches!(e, WorldEvent::Objective { what: 22, .. }))
        {
            broke = true;
            break;
        }
    }
    assert!(broke, "broken in by an unbroken stand");
    let Some(Guard::Doors(d)) = world.guard_now() else {
        panic!();
    };
    assert!(d.outer[0].breached);
    let door = world
        .aboard
        .room
        .door_index_at(mid(&world, d.outer[0].door))
        .unwrap();
    assert!(!world.aboard.room.door_sealed(door), "let go");
    // The commander down: the run lost.
    world.aboard.room.kill_now(vip);
    let mut said = false;
    for _ in 0..3 {
        if world
            .step(&[])
            .iter()
            .any(|e| matches!(e, WorldEvent::Objective { what: 23, .. }))
        {
            said = true;
            break;
        }
    }
    assert!(said);
    assert!(world.lost, "the run is lost");
}

#[test]
fn hold_the_doors_waves_are_three_quarters_and_one_a_player_every_twenty_five_seconds() {
    let (mut world, id) = guarded(crate::run::Mission::Doors);
    assert!(until(&mut world, 60, |w| w.droids_standing() > 0));
    let players = world.players();
    assert_eq!(world.droids_standing(), 4 * 3 / 4 + players);
    let wave = |w: &World| w.defense(id).unwrap().wave;
    let was = wave(&world);
    assert!(until(
        &mut world,
        data::DOORS_WAVE_STEPS as u32 + 5,
        |w| wave(w) == was + 1
    ));
    assert!(world.droids_standing() > 4 * 3 / 4 + players, "stacked");
}

#[test]
fn the_commander_dead_loses_the_site_and_not_the_run() {
    use crate::objective::Guard;
    let (mut world, id) = guarded(crate::run::Mission::Chief);
    let Some(Guard::Chief(c)) = world.guard_now() else {
        panic!("chief");
    };
    assert!(c.spots.len() >= 2, "a round");
    world.aboard.room.kill_now(c.vip as usize);
    let mut fell = false;
    for _ in 0..3 {
        if world
            .step(&[])
            .iter()
            .any(|e| matches!(e, WorldEvent::Objective { what: 26, .. }))
        {
            fell = true;
            break;
        }
    }
    assert!(fell);
    assert!(world.defense(id).unwrap().lost, "the site falls");
    assert!(!world.lost, "the run goes on");
}

#[test]
fn the_hold_done_the_republic_s_soldiers_come_while_an_enemy_stands() {
    let (mut world, _) = guarded(crate::run::Mission::Doors);
    assert!(until(&mut world, 60, |w| w.droids_standing() > 0));
    let crew = world.aboard.crew_count();
    world.set_guard_time_for_probe(2);
    let mut up = false;
    let mut came = 0;
    for _ in 0..10 {
        for e in world.step(&[]) {
            match e {
                WorldEvent::Objective { what: 24, .. } => up = true,
                WorldEvent::Objective { what: 25, n, .. } => came = n,
                _ => {}
            }
        }
        if came > 0 {
            break;
        }
    }
    assert!(up, "the hold's time up");
    assert!(came > 0, "the Republic's soldiers in");
    assert_eq!(world.aboard.crew_count(), crew + came);
    for who in crew..crew + came {
        assert!(world.is_reinforcement(who), "gone at the mission's end");
    }
}
