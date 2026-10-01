//! The Manufacturers (feature 109): a human faction holding sites from the
//! first day of a run — a garrison with the machines beside it until day
//! ten, their own people in waves after — that the crisis never takes,
//! that is never a jammer, that nobody loots, and that bleeds out once
//! down.

use bims::combat::{Tier, WeaponKind};
use bims::droid::DroidKind;
use bims::sight::Stance;
use shipdesign::fixture::flyer;

use crate::armour::LootSource;
use crate::class::{self, Class};
use crate::data;
use crate::event::WorldEvent;
use crate::fixture::{REFERENCE_MONEY, crewed_world};
use crate::manufacturer;
use crate::world::World;

/// One player and a bot, docked at the nearest site of the
/// Manufacturers' on `day`, with the garrison laid.
fn at_their_site(day: u32) -> (World, u32) {
    at_their_site_with(day, |_| {})
}

/// [`at_their_site`] with something done to the world before the dock.
fn at_their_site_with(day: u32, before: impl FnOnce(&mut World)) -> (World, u32) {
    let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 1, 2);
    before(&mut world);
    // A class, so the fight pays experience: chosen at home, where it may be.
    world.set_class(0, Class::Soldier).unwrap();
    let station = world
        .manufacturer_dock_for_probe(day)
        .expect("a site of theirs within reach of home");
    assert_eq!(world.days_gone(), day);
    // The first step photographs the site, the next settles and lays.
    for _ in 0..3 {
        world.step(&[]);
    }
    assert!(world.is_manufacturer_held(station));
    (world, station)
}

/// The residents' room's Manufacturers, by index.
fn theirs(world: &World) -> Vec<usize> {
    let room = &world.residents.as_ref().unwrap().aboard.room;
    (0..room.crew_count() as usize)
        .filter(|&who| room.is_manufacturer(who))
        .collect()
}

/// Every Manufacturer downed where it stands, the countdown ahead of it.
fn knock_them_all_out(world: &mut World) {
    for who in theirs(world) {
        let room = &mut world.residents.as_mut().unwrap().aboard.room;
        if room.is_alive(who) {
            room.knock_out_for_probe(who);
        }
    }
}

fn wreck_the_machines(world: &mut World) {
    let room = &mut world.residents.as_mut().unwrap().aboard.room;
    for i in 0..room.droid_count() as usize {
        room.strike_droid(i, bims::droid::DroidPart::Chassis, 1e6);
    }
}

/// **A new run has somewhere to fight on day nought**: at least two sites
/// of theirs within two lanes of home and none in the home system, the
/// nearest reachable by the lanes, and a system of theirs next door says so
/// on the map.
#[test]
fn a_new_run_has_their_sites_within_two_lanes_and_none_at_home() {
    let world = crewed_world(flyer(2), REFERENCE_MONEY, 1, 2);
    for s in &world.stations {
        assert!(!world.is_manufacturer_station(s.id), "none at home");
        assert!(!world.is_manufacturer_held(s.id));
    }
    let galaxy = world.galaxy();
    let hops = galaxy.hops_from(world.star_id);
    let mut near = 0;
    for (star, &h) in hops.iter().enumerate() {
        if h == 0 || h > data::MANUFACTURER_NEAR_HOPS {
            continue;
        }
        let system = galaxy.system(star as u32).unwrap();
        near += system
            .stations
            .iter()
            .filter(|s| world.is_manufacturer_site(star as u32, s))
            .count();
    }
    assert!(near >= data::MANUFACTURER_NEAR_SITES, "{near} near home");
    let (star, _) = world.nearest_manufacturer_site().unwrap();
    assert!(world.route_to(star).is_some(), "the lanes reach it");
    // The map marks one a lane away, and never as the machines'.
    for (site, quote) in world.travel_quotes() {
        let Ok(quote) = quote else { continue };
        let theirs = galaxy
            .system(site.star)
            .and_then(|s| s.station(site.station).cloned())
            .is_some_and(|s| world.is_manufacturer_site(site.star, &s));
        assert_eq!(quote.manufacturers, theirs, "{site:?}");
        if theirs {
            assert!(!quote.infested && !quote.jammer);
            assert_eq!(quote.tier, Tier::One, "the pistol days are tier one");
        }
    }
}

/// **Day nought: their people with pistols and nothing on**, no machine
/// beside them and nothing to come after. The site is hostile, nobody
/// lives there, and it is cleared the moment the last of them is down —
/// out cold, not dead — with the bounty paid for each, and the crew's
/// experience fifteen a head.
#[test]
fn a_day_nought_site_is_pistols_and_no_armour_and_clears_on_the_last_down() {
    let (mut world, station) = at_their_site(0);
    assert_eq!(world.stance(station), Stance::Hostile);
    let people = world.station(station).map(|s| world.people_of(s));
    assert_eq!(people, Some(0), "nobody lives there");
    let it = world.infestation(station).unwrap().clone();
    assert!(it.settled && it.manufacturers);
    assert_eq!(it.waves_left, 0, "a fixed garrison: nothing to come");
    let room = &world.residents.as_ref().unwrap().aboard.room;
    assert_eq!(room.droid_count(), 0, "no machine before day five");
    let them = theirs(&world);
    assert_eq!(them.len() as u32, world.droid_wave_size());
    for &who in &them {
        let gear = room.gear(who);
        assert_eq!(gear.weapon, Some(WeaponKind::LaserPistol.basic()));
        assert!(gear.armour.is_none());
        assert_eq!(room.uniform(who), bims::character::Uniform::Manufacturer);
    }
    // Everybody down but one: not cleared.
    let money = world.money;
    let xp = world.progress_of(0).xp;
    let last = *them.last().unwrap();
    for &who in &them[..them.len() - 1] {
        world
            .residents
            .as_mut()
            .unwrap()
            .aboard
            .room
            .knock_out_for_probe(who);
    }
    // The crew member stood beside the fight, so every down is in reach.
    beside(&mut world, them[0]);
    world.step(&[]);
    assert!(!world.droid_station_cleared(station));
    assert_eq!(
        world.run.pending_bounty,
        (them.len() as u64 - 1)
            * crate::world::bounty_share(
                crate::world::bounty_for(1),
                100 - data::BOUNTY_SPREAD_PERCENT
            ),
        "owed for each down, at tier one, a pistol and nothing worn the least"
    );
    // The last one down clears it, there and then, and pays.
    world
        .residents
        .as_mut()
        .unwrap()
        .aboard
        .room
        .knock_out_for_probe(last);
    let mut cleared = false;
    let mut rewarded = false;
    let worth = crate::world::bounty_share(
        crate::world::bounty_for(1),
        100 - data::BOUNTY_SPREAD_PERCENT,
    );
    for _ in 0..3 {
        let events = world.step(&[]);
        cleared |= events
            .iter()
            .any(|e| matches!(e, WorldEvent::DroidStationCleared { .. }));
        // And its pay said for the numbers over the body.
        rewarded |= events.iter().any(|e| {
            matches!(e, WorldEvent::EnemyRewarded { who, xp, money, .. }
                if *who == last as u32 && *xp == class::XP_ENEMY_DOWN && *money == worth)
        });
    }
    assert!(rewarded, "the last one's pay said");
    assert!(cleared && world.droid_station_cleared(station));
    let room = &world.residents.as_ref().unwrap().aboard.room;
    assert!(
        them.iter().all(|&w| room.is_alive(w)),
        "down, not dead: the clear waits on nobody bleeding out"
    );
    assert_eq!(world.money, money + them.len() as u64 * worth);
    assert_eq!(world.run.pending_bounty, 0);
    assert_eq!(
        world.progress_of(0).xp,
        xp + them.len() as u32 * class::XP_ENEMY_DOWN,
        "fifteen a head"
    );
}

/// Crew member 0 stood in the joined room where resident `who` is.
fn beside(world: &mut World, who: usize) {
    let at = world
        .body_position(LootSource::Resident(who as u32))
        .expect("the rooms joined");
    world.aboard.room.put_for_probe(0, at);
}

/// **Nothing of theirs is taken, and nobody saves one**: a Manufacturer
/// downed is no body to loot and no patient — it lets go of no gun, and
/// nobody revives it, its own people least of all — and it dies where it
/// lies when its countdown runs out (task 120). Its death, down first, is
/// worth nothing: the down paid `XP_ENEMY_DOWN` already.
#[test]
fn a_manufacturer_downed_is_never_revived_and_nothing_of_it_is_taken() {
    let (mut world, _) = at_their_site(0);
    let them = theirs(&world);
    let who = them[0];
    // The medkit in hand: the player's own Bim takes arms by itself among
    // them (September 2026), and a second of them shot down would be
    // fifteen more than the nothing this reads. Emptying its hands no longer
    // does — an empty hand under arms is given a pistol — so it holds its
    // fire with the medkit (task 138).
    world.aboard.room.order_hand(0, bims::bim::Hand::Medkit);
    beside(&mut world, who);
    {
        let room = &mut world.residents.as_mut().unwrap().aboard.room;
        room.strike(who, 1_000.0, false);
    }
    world.step(&[]);
    let xp = world.progress_of(0).xp;
    {
        let room = &world.residents.as_ref().unwrap().aboard.room;
        assert!(room.is_downed(who));
        assert!(!room.needs_rescue(who), "nobody's patient");
        assert!(room.down_left(who).is_some(), "the countdown running");
        assert!(
            room.gear(who).weapon.is_some(),
            "the gun stays with the body"
        );
        assert!(!world.aboard.room.visitor_down(who), "not a body to click");
    }
    // One of its own beside it, told to revive it: refused.
    if let Some(&other) = them.get(1) {
        let room = &mut world.residents.as_mut().unwrap().aboard.room;
        let at = room.body_pos(who);
        room.put_for_probe(other, at + bims::math::vec2(40.0, 0.0));
        assert!(!room.revive_crewmate(other, who), "never revived");
    }
    let mut dead = false;
    for _ in 0..(bims::health::DOWNED_SECONDS as u32 * 60 + 60) {
        world.step(&[]);
        if !world.residents.as_ref().unwrap().aboard.room.is_alive(who) {
            dead = true;
            break;
        }
    }
    assert!(dead, "it died when its countdown ran out");
    assert_eq!(
        world.progress_of(0).xp,
        xp,
        "and its death is worth nothing more"
    );
}

/// **Day eight: about half the garrison is Troopers** — the machines they
/// still command, at the machines' own tier — beside their people in
/// tier-one guns and armour. Still one wave, and the Troopers have to be
/// destroyed for it to clear.
#[test]
fn a_day_eight_garrison_is_about_half_troopers_fighting_beside_them() {
    let mut troopers = 0;
    let mut people = 0;
    for day in [8u32] {
        let mut world = crewed_world(flyer(2), REFERENCE_MONEY, 1, 2);
        world.set_droid_wave_for_probe(16);
        let station = world.manufacturer_dock_for_probe(day).unwrap();
        for _ in 0..3 {
            world.step(&[]);
        }
        let room = &world.residents.as_ref().unwrap().aboard.room;
        for i in 0..room.droid_count() as usize {
            let d = room.droid(i).unwrap();
            assert_eq!(d.kind, DroidKind::Trooper);
            assert_eq!(d.tier, world.droid_tier());
            troopers += 1;
        }
        for who in theirs(&world) {
            let gear = room.gear(who);
            assert_eq!(gear.weapon.unwrap().tier, Tier::One);
            assert_eq!(gear.armour.unwrap().tier, Tier::One);
            people += 1;
        }
        assert_eq!(world.infestation(station).unwrap().waves_left, 0);
        // Their people down and the machines standing: not cleared.
        knock_them_all_out(&mut world);
        world.step(&[]);
        assert!(!world.droid_station_cleared(station));
        wreck_the_machines(&mut world);
        for _ in 0..3 {
            world.step(&[]);
        }
        assert!(world.droid_station_cleared(station));
    }
    assert_eq!(troopers + people, 16);
    assert!(
        (3..=13).contains(&troopers),
        "{troopers} of 16 at day eight"
    );
    // And over many garrisons the share is the day's.
    let many: usize = (0..400u64)
        .map(|seed| {
            manufacturer::garrison(10, 8, seed)
                .iter()
                .filter(|&&t| t)
                .count()
        })
        .sum();
    assert!((1_700..2_300).contains(&many), "{many} of 4000");
}

/// **From day ten, their own people in waves**: no machine among them,
/// gear at the machines' tier, and the next wave four hours of the
/// mission clock after the last is down — at the airlock — until none are
/// left.
#[test]
fn from_day_ten_they_come_in_waves_of_their_own_people_alone() {
    // A tier-one site has one wave; this is the waves after it.
    let (mut world, station) = at_their_site_with(12, |w| w.set_droid_waves_for_probe(3));
    let it = world.infestation(station).unwrap().clone();
    assert!(it.waves_left >= 1, "waves: {it:?}");
    let room = &world.residents.as_ref().unwrap().aboard.room;
    assert_eq!(room.droid_count(), 0, "they have lost the machines");
    let tier = world.droid_tier();
    for who in theirs(&world) {
        let gear = room.gear(who);
        assert_eq!(gear.weapon.unwrap().tier, tier);
        assert_eq!(gear.armour.unwrap().tier, tier);
    }
    // Down, and the next is due four hours on.
    knock_them_all_out(&mut world);
    world.step(&[]);
    world.step(&[]);
    let now = world.mission_steps();
    let due = world.infestation(station).unwrap().next_wave.unwrap();
    assert!(
        due >= now + data::MANUFACTURER_REINFORCE_STEPS - 2
            && due <= now + data::MANUFACTURER_REINFORCE_STEPS,
        "due {due}, now {now}"
    );
    // A minute's wait for the rest of the test.
    world.set_droid_reinforce_minutes_for_probe(1.0);
    world.infestation_mut_for_probe(station).unwrap().next_wave = Some(now + 30);
    let before = theirs(&world).len();
    let mut landed = false;
    for _ in 0..200 {
        let events = world.step(&[]);
        if events
            .iter()
            .any(|e| matches!(e, WorldEvent::DroidReinforcements { .. }))
        {
            landed = true;
            break;
        }
    }
    assert!(landed, "the next wave docked");
    world.step(&[]);
    let now_theirs = theirs(&world);
    assert_eq!(
        now_theirs.len() as u32,
        before as u32 + world.droid_wave_size()
    );
    let room = &world.residents.as_ref().unwrap().aboard.room;
    assert_eq!(room.droid_count(), 0, "their people alone");
    assert!(
        world.droid_ship(station).is_some(),
        "their ship at the airlock"
    );
    // Every wave down: cleared.
    for _ in 0..(it.waves_left + 2) * 200 {
        knock_them_all_out(&mut world);
        world.step(&[]);
        if world.droid_station_cleared(station) {
            break;
        }
    }
    assert!(world.droid_station_cleared(station));
}

/// **The crisis never takes a site of theirs, and it is never a jammer**:
/// the machines taking the whole system round it pass it by, and the
/// jammer is somewhere else — the machines' own where there is nowhere
/// else.
#[test]
fn the_crisis_passes_their_site_by_and_it_is_never_the_jammer() {
    let (mut world, station) = at_their_site(0);
    world.infest(station);
    assert!(world.is_manufacturer_held(station), "still theirs");
    world.leave_for_probe();
    world.set_droid_origin_for_probe(world.star_id);
    world.infest_here_for_probe();
    for s in world.stations.clone() {
        if world.is_manufacturer_station(s.id) {
            assert!(world.is_manufacturer_held(s.id));
        }
    }
    let jammer = world.jammer_station().expect("the system is theirs");
    assert!(!world.is_manufacturer_station(jammer));
    assert_ne!(jammer, station);
}

/// **Two clients meet the same garrison**: the same seed and the same trip
/// lay the same people with the same gear in the same places, and the
/// checksums agree through the fight.
#[test]
fn two_worlds_meet_the_same_garrison() {
    let (mut a, _) = at_their_site(7);
    let (mut b, _) = at_their_site(7);
    let read = |w: &World| {
        let room = &w.residents.as_ref().unwrap().aboard.room;
        (0..room.body_count() as usize)
            .map(|who| (room.body_pos(who), room.is_manufacturer(who)))
            .map(|(p, m)| (p.x.to_bits(), p.y.to_bits(), m))
            .collect::<Vec<_>>()
    };
    assert_eq!(read(&a), read(&b));
    for _ in 0..300 {
        a.step(&[]);
        b.step(&[]);
    }
    assert_eq!(a.checksum(), b.checksum());
    assert_eq!(read(&a), read(&b));
}

/// A site left uncleared is put back and met afresh on the next visit —
/// rolled again at the day of that visit, their people and not the dead
/// of the last.
#[test]
fn a_site_left_uncleared_is_met_afresh_and_their_dead_are_nobody_s_graves() {
    let (mut world, station) = at_their_site(1);
    let who = theirs(&world)[0];
    world
        .residents
        .as_mut()
        .unwrap()
        .aboard
        .room
        .kill_for_probe(who);
    world.step(&[]);
    world.leave_for_probe();
    let it = world.infestation(station).unwrap();
    assert!(!it.settled && !it.cleared, "put back as met: {it:?}");
    assert!(
        world.graves_at(station).is_empty(),
        "their dead are no station's"
    );
    assert_eq!(world.losses_at(station).dead, 0);
}

/// **They fight**: a crew member walking in among them is shot at — the
/// hostile room's own war, its people mustered and their shots flown on
/// the joined deck — and the crew's bolts reach them back.
#[test]
fn they_shoot_the_crew_and_the_crew_shoot_back() {
    let (mut world, _) = at_their_site(4);
    let them = theirs(&world);
    beside(&mut world, them[0]);
    let (mut hit, mut struck) = (false, false);
    for _ in 0..1_200 {
        world.aboard.room.patch_up_for_probe(0);
        let events = world.step(&[]);
        hit |= events
            .iter()
            .any(|e| matches!(e, WorldEvent::CrewHit { who: 0, .. }));
        let room = &world.residents.as_ref().unwrap().aboard.room;
        struck |= them
            .iter()
            .any(|&w| room.health(w) < 100.0 || !room.is_alive(w));
        if hit && struck {
            break;
        }
    }
    assert!(hit, "a Manufacturer's shot landed on the crew member");
    assert!(struck, "and the crew's landed on one of them");
}

/// **A reinforcement of theirs comes looking for the crew**, as the
/// machines' does (`a_reinforcement_wave_hunts_the_crew_and_the_first_wave_waits`):
/// the garrison stands about the station and knows only what it has
/// seen, and the wave their ship brings after it was told where the crew
/// are and walks at them from the airlock. It stood at the airlock
/// instead — "just standing behind the airlock they came from" — since
/// the telling was a machine's (`Droid::seeking`) and a reinforcement of
/// theirs is their people alone.
#[test]
fn a_reinforcement_of_theirs_hunts_the_crew_from_its_airlock() {
    reinforcement_hunts(false);
}

/// **And one that lands after the room has forgotten the crew** — the
/// garrison down a minute and more and nobody of theirs looking — was
/// told where they are all the same, as the machines' is.
#[test]
fn a_reinforcement_of_theirs_is_told_where_the_crew_are() {
    reinforcement_hunts(true);
}

/// The garrison down, crew member 0 ashore, the next wave landed —
/// `forgotten` a minute and more after, crew member 0 out where the
/// garrison stood — and the wave walked at it from its airlock.
fn reinforcement_hunts(forgotten: bool) {
    let (mut world, station) = at_their_site_with(12, |w| w.set_droid_waves_for_probe(3));
    let garrison = theirs(&world);
    if forgotten {
        // Where one of the garrison stood, off the watched airlock.
        beside(&mut world, garrison[0]);
    } else {
        // At the crew's own airlock, far from where the next wave comes in.
        let ashore = world.aboard.ashore.expect("docked, so there is a door");
        world
            .aboard
            .room
            .put_for_probe(0, bims::math::vec2(ashore.x as f32, ashore.y as f32));
    }
    world.aboard.room.recruit_for_probe(0, true);
    // The garrison down and gone, and the next wave a moment off.
    knock_them_all_out(&mut world);
    world.step(&[]);
    world.step(&[]);
    if forgotten {
        // Past `FORGET_AFTER`, with nobody of theirs on their feet to see,
        // and the next wave held off until then.
        let now = world.mission_steps();
        world.infestation_mut_for_probe(station).unwrap().next_wave = Some(now + 100_000);
        for _ in 0..(bims::game::FORGET_AFTER as u32 + 5) * 60 {
            world.aboard.room.patch_up_for_probe(0);
            world.step(&[]);
        }
    }
    let now = world.mission_steps();
    world.infestation_mut_for_probe(station).unwrap().next_wave = Some(now + 30);
    let mut landed = false;
    for _ in 0..200 {
        let events = world.step(&[]);
        if events
            .iter()
            .any(|e| matches!(e, WorldEvent::DroidReinforcements { .. }))
        {
            landed = true;
            break;
        }
    }
    assert!(landed, "the next wave docked");
    world.step(&[]);
    let wave: Vec<usize> = theirs(&world)
        .into_iter()
        .filter(|w| !garrison.contains(w))
        .collect();
    assert!(!wave.is_empty());
    // How far each of the wave is from the crew member, as the room
    // believes: a reinforcement believes in the crew from the start.
    let to_the_crew = |world: &World| -> Vec<f32> {
        let room = &world.residents.as_ref().unwrap().aboard.room;
        let believed: Vec<bims::math::Vec2> =
            room.believed_for_probe().into_iter().flatten().collect();
        wave.iter()
            .filter(|&&w| room.is_alive(w) && !room.is_downed(w))
            .map(|&w| {
                believed
                    .iter()
                    .map(|&at| (at - room.bim_pos(w)).len())
                    .fold(f32::MAX, f32::min)
            })
            .collect()
    };
    let landed_at = to_the_crew(&world);
    assert!(
        landed_at.iter().all(|&d| d < f32::MAX),
        "a reinforcement knows where the crew are: {landed_at:?}"
    );
    for _ in 0..600 {
        world.aboard.room.patch_up_for_probe(0);
        world.step(&[]);
    }
    let now_at = to_the_crew(&world);
    let tile = shipdesign::TILE as f32;
    let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len().max(1) as f32;
    assert!(
        mean(&now_at) < mean(&landed_at) - 4.0 * tile,
        "the wave closed on the crew: {:.1} tiles off on landing, {:.1} ten seconds on",
        mean(&landed_at) / tile,
        mean(&now_at) / tile
    );
}
