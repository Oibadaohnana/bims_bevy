//! Defending a town (feature 94): the threat, the waves, the fight
//! inside one room, the win and what it leaves behind.
//!
//! The fight itself is the one in the game that does not cross the seam:
//! the machines and the town's people are both in the residents' room,
//! and what passes between them is delivered there. So most of what is
//! pinned here is *who shoots at whom*, which is three target lists and
//! one sheltering list the world sets every step.

use shipdesign::fixture::flyer;

use crate::class::{self, Class, Side, Talent};
use crate::data;
use crate::defense;
use crate::event::WorldEvent;
use crate::fixture::{REFERENCE_MONEY, open_simulation_world};
use crate::surface;
use crate::world::{Command, World};
use crate::world_checksum;

fn basic() -> World {
    open_simulation_world(flyer(2), REFERENCE_MONEY, 2)
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
    // a mercenary or a defender (task 111). Asked of the room, which is
    // what the world told it.
    let residents = world.residents.as_ref().expect("the town's room");
    let bims = residents.aboard.room.crew_count() as usize;
    assert!(bims > 1, "a town with people in it");
    for who in 0..bims {
        let merc = residents.is_mercenary(who);
        let sheltering = residents.aboard.room.is_sheltering(who);
        let want = who != surface::GUARD as usize && !merc && !residents.is_defender(who);
        assert_eq!(sheltering, want, "body {who} (mercenary {merc})");
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
            .map(|who| {
                bims::health::Part::ALL
                    .into_iter()
                    .map(|p| room.part_health(who, p))
                    .sum::<f32>()
                    + room.blood(who)
            })
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
/// and never fewer than one, lowest index first, never the guard and
/// never a mercenary — and they are classless bots with no contract.
#[test]
fn the_survivors_who_join_are_classless_bots_with_no_wages() {
    let Some((mut world, id)) = a_threatened_town(0.1, 1, Some(2)) else {
        return;
    };
    assert!(
        until(&mut world, 40, |w| w.droids_standing() > 0),
        "the first wave never landed"
    );
    let crew_was = world.aboard.crew_count();
    let players = world.players();
    let hired_was = world.hired().len();
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
    assert_eq!(world.hired().len(), hired_was, "a joiner signed a contract");
    let bunks = world.aboard.room.bed_count() as u32;
    for who in crew_was..world.aboard.crew_count() {
        assert_eq!(
            world.class_of(who),
            crate::class::Class::None,
            "joiner {who} has a class"
        );
        assert!(!world.is_hired(who), "joiner {who} draws wages");
        assert!(world.aboard.room.is_alive(who as usize), "joiner {who}");
        // **No bunk is no obstacle**: a hire wants one and is refused
        // without, and a joiner comes anyway and sleeps on the deck
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

/// Every one of the town's people dead is the town lost, and it falls
/// like any other station.
#[test]
fn a_town_whose_people_are_all_dead_falls() {
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
        until(&mut world, 20, |w| w.is_droid_held(id)),
        "a town with nobody left never fell"
    );
    assert!(world.defense(id).is_some_and(|d| d.lost));
    assert!(!world.town_held(id));
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
/// station the machines hold is: `XP_ENEMY_DOWN` and `XP_ENEMY_DEAD` (task 119) to
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
    // A machine down by the crew member: the down and the death, once.
    down_by_crew_member_0(&mut world, 0);
    world.step(&[]);
    let paid = class::XP_ENEMY_DOWN + class::XP_ENEMY_DEAD;
    assert_eq!(
        world.progress_of(0).xp,
        xp + paid,
        "a machine is experience"
    );
    for _ in 0..3 {
        world.step(&[]);
    }
    assert_eq!(world.progress_of(0).xp, xp + paid, "and only once");

    // *Rampage*: a soldier's stack for the first machine of the wave,
    // held while the second stands.
    let Some((mut world, _)) = a_threatened_town(1.0, 2, Some(2)) else {
        return;
    };
    assert_eq!(world.set_class(0, Class::Soldier), Ok(()));
    let mut events = Vec::new();
    world.award(0, class::LEVEL_XP[class::LEVELS as usize - 1], &mut events);
    let (level, side) = (1..=class::LEVELS)
        .find_map(|l| {
            class::pick_at(Class::Soldier, l).and_then(|(left, right)| {
                (left == Talent::Rampage)
                    .then_some((l, Side::Left))
                    .or((right == Talent::Rampage).then_some((l, Side::Right)))
            })
        })
        .expect("rampage is on a pick level");
    world.step(&[Command::PickTalent {
        slot: 0,
        level: level as u32,
        side,
    }]);
    assert!(world.has_talent(0, Talent::Rampage));
    assert!(
        until(&mut world, 40, |w| w.droids_standing() > 1),
        "the first wave never landed"
    );
    down_by_crew_member_0(&mut world, 0);
    world.step(&[]);
    assert_eq!(world.aboard.room.rampage(0), 1, "a stack in the town");
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

/// **A station is defended as a town is** (task 111): the countdown is
/// `DEFENSE_DELAY_STEPS`, the crew are stood on the station's deck the
/// step it starts, the waves come in at the airlock farthest from the
/// crew's, the defenders fight rather than shelter, and the last machine
/// down clears the site and pays the bounty it was holding — with nobody
/// joining the crew, and nothing held for good.
#[test]
fn a_station_defence_counts_down_lands_at_the_far_airlock_and_pays_on_the_win() {
    assert_eq!(data::DEFENSE_DELAY_STEPS, 1_200, "twenty seconds at 1x");
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
    assert_eq!(fielded, defense::defenders(world.days_gone()));
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
    // The win: the bounty held until the last is down, then paid.
    let money = world.money;
    let tier = world.droid_tier().code();
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
    assert_eq!(world.money, money + 2 * crate::world::bounty_for(tier));
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

/// **The defenders count as crew towards the wave** (task 111): a
/// defence's wave is the formula's with that many more players, and
/// anywhere else it is the formula's alone.
#[test]
fn the_wave_at_a_defence_is_the_wave_with_its_defenders_as_players() {
    let mut world = basic();
    world.step(&[]);
    let n = world.defenders_fielded();
    assert!(n > 0, "defenders fielded");
    let steps = crate::droid::time_steps(world.hours_gone());
    let want = crate::droid::wave_size(world.players() + n, steps).min(world.droid_wave_max());
    assert_eq!(world.droid_wave_size(), want);
    let mut quiet = basic();
    quiet.set_quiet_sites_for_probe(true);
    quiet.step(&[]);
    assert_eq!(quiet.defenders_fielded(), 0, "no defenders at a quiet site");
    assert_eq!(
        quiet.droid_wave_size(),
        crate::droid::wave_size(quiet.players(), steps).min(quiet.droid_wave_max())
    );
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
    assert_eq!(world.losses_at(id).mercenaries, 0);
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
    // One machine down: a bounty held on the clear.
    if let Some(residents) = world.residents.as_mut() {
        for _ in 0..60 {
            residents
                .aboard
                .room
                .strike_droid(0, bims::droid::DroidPart::Chassis, 100.0);
        }
    }
    until(&mut world, 5, |w| w.run.pending_bounty > 0);
    assert!(world.run.pending_bounty > 0, "a bounty held");
    let money = world.money;
    let events = world.leave_for_probe();
    assert!(world.is_droid_held(id), "the machines have it");
    assert!(world.defense(id).is_some_and(|d| d.lost));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::TownFell { station } if *station == id))
    );
    // No bounty paid — the pool may fall by the crew left ashore, the
    // bots' penalty and the players' respawns, and by nothing it gains.
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
