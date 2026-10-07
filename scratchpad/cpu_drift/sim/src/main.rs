// Plays the survivors test's seeded fight and prints, every 120 steps, the
// world checksum (rounded, what multiplayer compares) and an exact hash of
// every body's position bits. Run twice under two libm paths and diff.
use bims::combat::{Gear, WeaponKind};
use bims::math::vec2;
use shipdesign::fixture::combat_ship;
use world::class::Class;
use world::fixture::{REFERENCE_MONEY, open_crewed_world};
use world::orders::Standing;
use world::run::{Phase, Site};
use world::world::{Command, World};
use world::LootSource;
const TILE: f32 = shipdesign::TILE as f32;

fn arm(world: &mut World) {
    let crew = world.aboard.room.crew_count() as usize;
    for (who, kind) in WeaponKind::ALL[..5].iter().copied().cycle().take(crew).enumerate() {
        let gear = world.aboard.room.gear(who);
        world.aboard.room.issue(who, Gear { weapon: Some(kind.basic()), ..gear });
    }
}
fn travel_to(world: &mut World, site: Site) {
    world.step(&[Command::Propose { slot: 0, star: site.star, station: site.station }]);
    for slot in 1..world.players() { world.step(&[Command::Accept { slot, yes: true }]); }
    assert_eq!(world.run.phase, Phase::Mission);
}
fn back_to_ship(world: &mut World) {
    world.step(&[Command::Return { slot: 0 }, Command::Return { slot: 1 }]);
    let aboard = world::fixture::walk_the_players_aboard(world);
    world.step(&aboard);
    for _ in 0..3000 {
        if matches!(world.run.phase, Phase::Map | Phase::Reward) { return; }
        if matches!(world.run.departure, Some(world::run::Departure::Asking { .. })) {
            world.step(&[Command::LeaveBehind { slot: 0, yes: true }, Command::LeaveBehind { slot: 1, yes: true }]);
            continue;
        }
        world.step(&[]);
    }
    panic!("the ship never left");
}
fn banner_by(world: &World, body: LootSource) -> Option<(i32, i32)> {
    let room = &world.aboard.room;
    let at = world.body_position(body)?;
    let (cx, cy) = ((at.x / TILE) as i32, (at.y / TILE) as i32);
    for r in 0..12 { for dy in -r..=r { for dx in -r..=r {
        let tile = (cx + dx, cy + dy);
        if room.is_banner_tile(vec2((tile.0 as f32 + 0.5) * TILE, (tile.1 as f32 + 0.5) * TILE)) { return Some(tile); }
    }}}
    None
}
fn exact(world: &World) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut eat = |v: u64| for b in v.to_le_bytes() { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); };
    for room in std::iter::once(&world.aboard.room).chain(world.residents.as_ref().map(|r| &r.aboard.room)) {
        for who in 0..room.crew_count() as usize {
            let p = room.body_pos(who);
            eat(p.x.to_bits() as u64); eat(p.y.to_bits() as u64); eat(room.health(who).to_bits() as u64);
        }
        for d in room.droids() { eat(d.pos.x.to_bits() as u64); eat(d.pos.y.to_bits() as u64); }
    }
    h
}
fn report(tag: &str, world: &World, step: u32) {
    println!("{tag} {step:6} rounded={:016x} exact={:016x} droids={}", world::world_checksum(world), exact(world),
        world.residents.as_ref().map_or(0, |r| r.aboard.room.droid_count()) + world.aboard.room.droid_count());
}
fn run_for(tag: &str, world: &mut World, steps: u32, base: &mut u32) {
    for _ in 0..steps { world.step(&[]); *base += 1; if *base % 120 == 0 { report(tag, world, *base); } }
}

fn arena(players: u32, tier: bims::combat::Tier, kinds: Vec<bims::droid::DroidKind>, steps: u32) {
    use shipdesign::fixture::COMBAT_CREW;
    let seed = world::data::DEFAULT_SEED;
    let galaxy = worldgen::Galaxy::new(seed, worldgen::GalaxyType::SpiralTwoArm);
    let (star, station) = world::spawn(&galaxy).unwrap();
    let mut w = World::start_with_crew(combat_ship(), world::data::SIMULATION_MONEY, players, COMBAT_CREW, seed,
        worldgen::GalaxyType::SpiralTwoArm, star, station).unwrap();
    w.arena_dock_for_probe();
    let crew = w.aboard.room.crew_count() as usize;
    for (who, kind) in WeaponKind::ALL.iter().copied().cycle().take(crew).enumerate() {
        let gear = w.aboard.room.gear(who);
        w.aboard.room.issue(who, Gear { weapon: Some(kind.basic()), ..gear });
    }
    w.set_droid_wave_for_probe(14);
    w.set_droid_kinds_for_probe(kinds);
    w.set_droid_tier_for_probe(Some(tier));
    w.set_droid_reinforce_minutes_for_probe(0.5);
    w.set_machines_only_for_probe();
    w.set_droid_waves_for_probe(50);
    w.infest(station);
    w.outfit_for_probe(tier);
    let mut first: Option<u32> = None;
    for n in 1..=steps {
        if n == 10 {
            let tile = banner_by(&w, LootSource::Resident(0)).expect("ground by a machine");
            w.step(&[Command::Orders { slot: 0, order: Standing::Attack { tile } }, Command::Orders { slot: 1, order: Standing::Attack { tile } }]);
        } else { w.step(&[]); }
        if n % 120 == 0 {
            println!("arena{players} {n:6} rounded={:016x} exact={:016x} droids={} crew_alive={}", world::world_checksum(&w), exact(&w), w.droids_standing(),
                (0..w.aboard.room.crew_count() as usize).filter(|&i| w.aboard.room.is_alive(i)).count());
        }
        let _ = &mut first;
    }
}

fn main() {
    use bims::droid::DroidKind::*;
    if std::env::args().nth(1).as_deref() == Some("gen") {
        use worldgen::GalaxyType::*;
        for seed in 0..std::env::args().nth(2).unwrap().parse::<u64>().unwrap() {
            for ty in [SpiralTwoArm, Spiral, Elliptical] {
                let g = worldgen::Galaxy::new(seed, ty);
                let mut h: u64 = 0xcbf29ce484222325;
                for st in &g.stars { for v in [st.position.x.to_bits(), st.position.y.to_bits()] { h ^= v; h = h.wrapping_mul(0x100000001b3); } }
                println!("{seed} {} rounded={:016x} exact_stars={h:016x}", ty as u32, g.checksum());
            }
        }
        return;
    }
    if std::env::args().nth(1).as_deref() == Some("runs") {
        let (from, to): (u64, u64) = (std::env::args().nth(2).unwrap().parse().unwrap(), std::env::args().nth(3).unwrap().parse().unwrap());
        for seed in from..to {
            let ty = worldgen::GalaxyType::SpiralTwoArm;
            let g = worldgen::Galaxy::new(seed, ty);
            let Some((star, station)) = world::spawn(&g) else { println!("{seed} nospawn"); continue };
            let Ok(mut w) = World::start_with_crew(combat_ship(), REFERENCE_MONEY, 2, 6, seed, ty, star, station) else { println!("{seed} nostart"); continue };
            w.set_whole_systems_for_probe(true);
            arm(&mut w);
            for _ in 0..600 { w.step(&[]); }
            let a = (world::world_checksum(&w), exact(&w), w.clock_minutes.to_bits(), w.ship.anchor.x.to_bits() ^ w.ship.anchor.y.to_bits().rotate_left(1));
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                back_to_ship(&mut w);
                let next = w.destinations().into_iter().find(|s| s.star != w.star_id && w.travel_quote(*s).is_ok())?;
                travel_to(&mut w, next);
                for _ in 0..1200 { w.step(&[]); }
                Some(())
            }));
            let b = (world::world_checksum(&w), exact(&w), w.clock_minutes.to_bits(), w.ship.anchor.x.to_bits() ^ w.ship.anchor.y.to_bits().rotate_left(1));
            println!("{seed} ok={:?} a={:016x} {:016x} clock={:016x} anchor={:016x} b={:016x} {:016x} clock={:016x} anchor={:016x}", r.ok().flatten().is_some(), a.0, a.1, a.2, a.3, b.0, b.1, b.2, b.3);
        }
        return;
    }
    if std::env::args().nth(1).as_deref() == Some("arena") {
        arena(2, bims::combat::Tier::Two, vec![Trooper, Husk, Lancer, Bomber, Trooper, Warden, Guardian, Conductor, Husk, Trooper], 20000);
        return;
    }
    let mut n = 0;
    let mut world = open_crewed_world(combat_ship(), REFERENCE_MONEY, 2, 6);
    world.set_class(0, Class::Soldier).unwrap();
    world.set_class(1, Class::Medic).unwrap();
    arm(&mut world);
    world.set_droid_reinforce_minutes_for_probe(1.0);
    world.set_droid_wave_for_probe(6);
    let here = world.current_site();
    let held = world.sites_at(world.star_id).into_iter().find(|&s| Some(s) != here
        && world::surface::surface_body(s.station).is_none() && world.travel_quote(s).is_ok()).unwrap();
    world.infest(held.station);
    back_to_ship(&mut world);
    report("station", &world, 0);
    travel_to(&mut world, held);
    run_for("station", &mut world, 10, &mut n);
    let tile = banner_by(&world, LootSource::Resident(0)).unwrap();
    world.step(&[Command::Orders { slot: 0, order: Standing::Attack { tile } }, Command::Orders { slot: 1, order: Standing::Attack { tile } }]);
    run_for("station", &mut world, 6000, &mut n);

    let mut n = 0;
    let mut town = open_crewed_world(combat_ship(), REFERENCE_MONEY, 2, 5);
    town.set_class(0, Class::Engineer).unwrap();
    town.set_class(1, Class::Tank).unwrap();
    arm(&mut town);
    let hops = town.start_star_hops_for_probe();
    let star = (0..hops.len() as u32).find(|&s| hops[s as usize] == 1).unwrap();
    town.set_crisis_first_day_for_probe(0);
    town.set_droid_origin_for_probe(star);
    town.set_day_for_probe(0);
    town.set_defense_delay_for_probe(world::data::STEP_MINUTES * 4.0);
    town.set_droid_reinforce_minutes_for_probe(1.0);
    town.set_droid_waves_for_probe(2);
    town.set_droid_wave_for_probe(6);
    let threatened = town.sites_at(town.star_id).into_iter().find(|s| world::surface::surface_body(s.station).is_some()
        && town.site_threatened(s.station) && town.travel_quote(*s).is_ok()).unwrap();
    back_to_ship(&mut town);
    report("town", &town, 0);
    travel_to(&mut town, threatened);
    run_for("town", &mut town, 6000, &mut n);
}
