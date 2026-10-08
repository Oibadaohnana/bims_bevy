//! A world read back from its save steps exactly as the one it was saved
//! from — mid-fight, which is when a guest asks for it.
//!
//! A run with company is lockstep: every machine steps the same world on
//! the same orders, and a guest whose `world_checksum` parts from the
//! host's is handed the host's save (`Session::save`) and stands it up in
//! place of its own (`Session::restore_as`). That only mends anything if
//! the world read back steps on **exactly** as the host's does: a field a
//! save leaves out that the fight reads is a guest parted again the step
//! after it was mended, and a run with company that never comes back
//! together. The existing round trip (`tests.rs`,
//! `the_host_s_save_read_back_as_a_guest_is_the_same_world_steered_from_its_own_slot`)
//! is at the dock, where nothing is fighting; these are fights — two
//! players steering their own Bims (the keys, the trigger, a roll, a
//! reload and every ability of their class), bots, field medics, waves of
//! machines — saved and read back over and over, the copy stepped beside
//! the original on the same orders and the two compared at every step.
//!
//! [`play`] is the harness: every [`EVERY`] steps the host's save is read
//! back as the guest in slot 1 and stepped beside the host for
//! [`WINDOW`] steps on the host's own orders ([`orders`], read off the
//! host's world), compared by `world_checksum` at every step and whole —
//! the save's text, which is every saved field — at [`WHOLE_AT`]. A
//! failure names the step it was read back at, how far on it parted, and
//! the path of fields down to the first line of the save that differs.
//! The host and every guest read back are drawn every [`DRAWS`] steps,
//! alike, so what a picture keeps in the world is compared too; a clone
//! is never drawn — the guest's rollback never draws the host's timeline
//! — and is held to the drawn host by the checksum, so a picture that
//! wrote what a step reads would part it.
//! The fights: the `droids` arena (a soldier and a medic; a tank and an
//! engineer on a dark station), a run's mission abroad on the floor, the
//! Machine Heart with ten bots, a town's Area defend on a planet at night
//! and the plain beyond it (every body on a window of its own), and every
//! mission of the map.
//!
//! The same is asked of `World::clone`, which the guest's rollback lives
//! on (task 156): a copy stepped beside the original is the original.

use bims::combat::{Gear, Tier, WeaponKind};
use bims::math::Vec2;
use bims::module::{Module, ModuleKind};
use bims::order::{CrewOrder, angle_code};
use world::{Class, Command, World};

use crate::Session;
use crate::game::Game;
use crate::session::galaxy_type;

const W: f32 = 1400.0;
const H: f32 = 900.0;
const TILE: f32 = 52.0;

/// How often a fight is saved and read back, in steps.
const EVERY: u64 = 137;
/// How long each copy read back is stepped beside the original.
const WINDOW: u64 = 300;
/// The steps into a window at which the two are compared whole (their
/// texts) as well as by the checksum, which is every step: the first
/// step, which is where a field a save left out shows, then a while on,
/// and the window's end. And as read back, before any.
const WHOLE_AT: [u64; 4] = [1, 10, 75, WINDOW];
/// How often the host's session and a guest's read back are drawn
/// (`Session::render`), in steps: a window's frame, not the step's.
const DRAWS: u64 = 12;

/// The combat ship's sixteen for **two players** (`Session::combat`, a
/// player more): the first two the players' own in `classes` at the top
/// level with every rank bought and six active items each, the last four
/// field medics, a gun of every kind dealt down them and the kit at
/// `tier`, docked at the arena. Nobody's fight yet.
fn crew(seed: u64, classes: [Class; 2], tier: Tier) -> Session {
    use shipdesign::fixture::{COMBAT_CREW, combat_ship};
    let galaxy = 0;
    let spawn = world::spawn(&worldgen::Galaxy::new(seed, galaxy_type(galaxy)));
    let (star, station) = spawn.expect("the seed has a dock");
    let mut game = Game::start_with_crew(
        combat_ship(),
        world::data::SIMULATION_MONEY,
        2,
        COMBAT_CREW,
        0,
        seed,
        galaxy_type(galaxy),
        star,
        station,
        W,
        H,
    )
    .expect("a world");
    assert!(game.world.arena_dock_for_probe(), "the arena");
    let room = &mut game.world.aboard.room;
    let kinds = WeaponKind::ALL.iter().copied().cycle();
    for (who, kind) in kinds.take(room.crew_count() as usize).enumerate() {
        let gear = room.gear(who);
        room.issue(
            who,
            Gear {
                weapon: Some(kind.basic()),
                ..gear
            },
        );
    }
    let mut session = Session::resumed(2, 0, game, seed, galaxy, spawn);
    session.field_medics_for_probe(crate::session::COMBAT_MEDICS);
    session.dress_crew();
    class_the_players(&mut session, classes, tier);
    session
}

/// The two players at the top level of `classes` with every rank bought,
/// their kit at `tier`, and six active items each, so the items' clocks
/// are in the fight.
fn class_the_players(session: &mut Session, classes: [Class; 2], tier: Tier) {
    let world = &mut session.game.as_mut().expect("a game").world;
    for (slot, class) in classes.into_iter().enumerate() {
        world.pick_class_for_probe(slot as u32, class, tier);
        world.set_ranks_for_probe(slot as u32, [world::class::MAX_RANK; world::class::SLOTS]);
    }
    for slot in 0..2 {
        world.give_items_for_probe(
            slot,
            &[
                item(ModuleKind::BlinkDrive, tier),
                item(ModuleKind::FieldMender, tier),
                item(ModuleKind::SmokeLauncher, tier),
                item(ModuleKind::TetherLink, tier),
                item(ModuleKind::DecoyProjector, tier),
                item(ModuleKind::AblativeShell, tier),
            ],
        );
    }
}

/// The `droids` command's fight (`Session::droids`) for two: [`crew`]'s
/// arena the machines', `wave` machines a wave at `tier`.
fn arena(seed: u64, classes: [Class; 2], tier: Tier, wave: u32, waves: u32) -> Session {
    let mut session = crew(seed, classes, tier);
    let world = &mut session.game.as_mut().unwrap().world;
    world.set_droid_wave_for_probe(wave);
    assert!(session.infest_the_dock_for_probe(Some(tier), 0.5, waves));
    session
}

/// A mission's command (`Session::mission`) for two: an attack is
/// [`arena`], a defence the arena under threat from the first step, its
/// waves the machines'; either laid out for the mission.
fn mission(seed: u64, mission: world::run::Mission, classes: [Class; 2], tier: Tier) -> Session {
    let mut session = if mission.is_attack() {
        arena(seed, classes, tier, 10, 3)
    } else {
        let mut session = crew(seed, classes, tier);
        let world = &mut session.game.as_mut().unwrap().world;
        world.set_droid_wave_for_probe(10);
        world.set_droid_tier_for_probe(Some(tier));
        world.set_machines_only_for_probe();
        session
    };
    let world = &mut session.game.as_mut().unwrap().world;
    world.set_mission_for_probe(Some(mission));
    world.fit_dock_for_probe();
    session
}

/// A run as the lobby's Start opens it (`screens::run::build_run`) for
/// two players and four bots, the floor on and the ready check as the
/// game screen puts them, the first mission left and the floor walked
/// on — a trip proposed and accepted, *Ready* pressed — to its first
/// fight away from home: a mission in another system, the way a run's
/// are.
fn abroad(seed: u64, classes: [Class; 2]) -> Session {
    let spawn = world::spawn(&worldgen::Galaxy::new(seed, galaxy_type(0)));
    let mut session = Session::run_with_bots(
        world::data::START_MONEY_PER_BIM,
        2,
        4,
        0,
        seed,
        0,
        spawn,
        &classes,
        W,
        H,
    );
    class_the_players(&mut session, classes, Tier::Two);
    let world = &mut session.game.as_mut().unwrap().world;
    world.set_floor(true);
    world.set_ready_check(true);
    for _ in 0..6 {
        world.leave_for_probe();
        let site = *world.floor_next().first().expect("the floor goes on");
        world.step(&[Command::Propose {
            slot: 0,
            star: site.star,
            station: site.station,
        }]);
        world.step(&[Command::Accept { slot: 1, yes: true }]);
        world.step(&[]);
        assert_eq!(
            (world.star_id, world.ship.state.station()),
            (site.star, Some(site.station)),
            "the trip taken"
        );
        world.step(&[
            Command::Ready { slot: 0, yes: true },
            Command::Ready { slot: 1, yes: true },
        ]);
        if matches!(
            world.site_kind(site.station),
            world::SiteKind::Attack | world::SiteKind::Defend
        ) {
            return session;
        }
    }
    panic!("no fight on the floor's first rows");
}

/// The `end` command's run (`screens::run::build_run`) for two players
/// and ten bots: the Machine Heart's fortress on the Heart's day, tier
/// three, the players' every rank bought.
fn heart(seed: u64, classes: [Class; 2]) -> Session {
    let spawn = world::spawn(&worldgen::Galaxy::new(seed, galaxy_type(0)));
    let mut session = Session::run_with_bots(
        world::data::START_MONEY_PER_BIM,
        2,
        10,
        0,
        seed,
        0,
        spawn,
        &classes,
        W,
        H,
    );
    assert!(session.end_for_probe(32), "the fortress");
    class_the_players(&mut session, classes, Tier::Three);
    session
}

/// The `defense` command (`Session::defense_for_probe`) for two players
/// and six bots on a run's ship: a town threatened, the crew on its pad,
/// its waves an Area defend's.
fn town(classes: [Class; 2]) -> Session {
    let (seed, roll) = (12_345, 678);
    let spawn = crate::session::pick_ground(seed, 0, roll);
    let mut session = Session::run_with_bots(
        world::data::START_MONEY_PER_BIM,
        2,
        6,
        0,
        seed,
        0,
        spawn,
        &classes,
        W,
        H,
    );
    // Every town of the system kept (the tests' dial, saved): the probe
    // lands at the lowest landable body's town, which a run's system at
    // home does not offer, and a load trims a system to what it offers
    // (`World::settle_offered`) — the town gone from under the crew.
    // And night, so the plain's sight is the short one.
    let world = &mut session.game.as_mut().unwrap().world;
    world.set_whole_systems_for_probe(true);
    world.set_night_for_probe(Some(true));
    assert!(session.land_for_probe(), "the town");
    assert!(session.defense_for_probe(0.25, 0.5, 4), "threatened");
    class_the_players(&mut session, classes, Tier::Two);
    // A step on the pad, so the town's defence stands: before it, a load
    // deals the system's outposts again (`World::settle_outposts`) and
    // takes back the town the probe's landing gave its people
    // (`World::give_back_outposts`), which no run's landing does.
    for _ in 0..2 {
        session.world_step();
    }
    assert!(
        session
            .game
            .as_ref()
            .unwrap()
            .world
            .defense_here()
            .is_some(),
        "the town's defence under way"
    );
    session
}

fn item(kind: ModuleKind, tier: Tier) -> Module {
    Module {
        kind,
        tier,
        paid: 0,
    }
}

fn tile_of(p: Vec2) -> (i32, i32) {
    ((p.x / TILE).floor() as i32, (p.y / TILE).floor() as i32)
}

/// Who the two players are and where they take the fight.
#[derive(Clone, Copy)]
struct How {
    classes: [Class; 2],
    /// Out onto a planet's plain: the players sent off the deck to the
    /// west, walked leg by leg, and their bots after banners far out on
    /// the ground — every body on a window of its own
    /// (`bims::nav::Maps::afield`).
    afield: bool,
}

impl How {
    fn fight(classes: [Class; 2]) -> How {
        How {
            classes,
            afield: false,
        }
    }
}

/// What the two players do this step: read off the world as it stands,
/// the same on every copy so long as the copies agree. Each walks at the
/// nearest machine it can see or away from it when close, aims and fires
/// at it, sprints, rolls and reloads now and then, and goes through its
/// class's abilities and its items in turn at the machine.
fn orders(world: &World, step: u64, how: How) -> Vec<Command> {
    let room = &world.aboard.room;
    let targets = room.hostiles_for_probe();
    let mut out = Vec::new();
    for slot in 0..2u32 {
        let who = slot as usize;
        let phase = step + slot as u64 * 37;
        let me = room.bim_pos(who);
        let nearest = targets
            .iter()
            .flatten()
            .filter(|t| !t.stale)
            .map(|t| t.at)
            .min_by(|a, b| (*a - me).len().total_cmp(&(*b - me).len()));
        let plain = room.interior();
        if how.afield {
            // Sent off the deck onto the plain, walked there leg by leg on
            // the body's own window, firing at whatever is near.
            if phase % 300 == 0 {
                let leg = (phase / 300) as f32;
                let out_there = bims::math::vec2(
                    plain.min.x - (30.0 + 35.0 * (leg % 4.0) + 5.0 * slot as f32) * TILE,
                    me.y + ((leg % 5.0) - 2.0) * 9.0 * TILE,
                );
                out.push(Command::Crew {
                    slot,
                    order: CrewOrder::Move {
                        x: out_there.x,
                        y: out_there.y,
                    },
                });
            } else if phase % 15 == 0 {
                let aim = nearest.map_or(std::f32::consts::PI, |at| (at - me).angle());
                out.push(Command::Crew {
                    slot,
                    order: CrewOrder::Control {
                        walk: None,
                        aim: angle_code(aim),
                        fire: nearest.is_some(),
                        sprint: false,
                    },
                });
            }
        } else if phase % 15 == 0 {
            let (aim, fire, walk) = match nearest {
                Some(at) => {
                    let d = at - me;
                    let aim = d.angle();
                    let walk = if d.len() > 7.0 * TILE {
                        Some(aim + 0.3 * ((phase / 15) % 3) as f32 - 0.3)
                    } else if d.len() < 3.0 * TILE {
                        Some(aim + std::f32::consts::PI)
                    } else if (phase / 15) % 4 == 0 {
                        None
                    } else {
                        Some(aim + std::f32::consts::FRAC_PI_2)
                    };
                    (aim, true, walk)
                }
                None => {
                    let aim = (phase / 15) as f32 * 0.7;
                    (aim, (phase / 15) % 5 == 0, Some(aim))
                }
            };
            out.push(Command::Crew {
                slot,
                order: CrewOrder::Control {
                    walk: walk.map(angle_code),
                    aim: angle_code(aim),
                    fire,
                    sprint: phase % 400 < 30,
                },
            });
        }
        if phase % 260 == 100 && !how.afield {
            out.push(Command::Crew {
                slot,
                order: CrewOrder::Dodge,
            });
        }
        if phase % 330 == 200 {
            out.push(Command::Crew {
                slot,
                order: CrewOrder::Reload,
            });
        }
        // The site's own work, whatever there is of it in reach: a weld,
        // a charge, a flag, a terminal.
        // Out on the plain the walk is the players' and nothing else's:
        // nothing below that sends them anywhere.
        if phase % 160 == 60 && !how.afield {
            out.push(match (phase / 160) % 4 {
                0 => Command::Weld { slot },
                1 => Command::Plant { slot },
                2 => Command::Flag { slot },
                _ => Command::Interact { slot },
            });
        }
        // The medkit in hand a while, and the gun again.
        if (phase % 410 == 205 || phase % 410 == 290) && !how.afield {
            let hand = if phase % 410 == 205 {
                bims::bim::Hand::Medkit
            } else {
                bims::bim::Hand::Weapon
            };
            out.push(Command::Crew {
                slot,
                order: CrewOrder::Hand { hand },
            });
        }
        // A crewmate down carried, and put down again.
        if phase % 600 == 300 && !how.afield {
            let down = (2..room.crew_count()).find(|&i| room.is_downed(i as usize));
            out.push(Command::Carry { slot, who: down });
        }
        if phase % 600 == 450 && !how.afield {
            out.push(Command::Carry { slot, who: None });
        }
        // The bots told to follow, to fall back to the ship, or to attack
        // where the nearest machine stands.
        if phase % 520 == 10 {
            let (_, my) = tile_of(me);
            let order = match ((phase / 520) % 3, nearest) {
                (0, _) if how.afield => world::Standing::Attack {
                    tile: (tile_of(plain.min).0 - 160, my + 20 * slot as i32 - 10),
                },
                (1, _) => world::Standing::Retreat,
                (2, Some(at)) => world::Standing::Attack { tile: tile_of(at) },
                _ => world::Standing::Follow,
            };
            out.push(Command::Orders { slot, order });
        }
        let Some(at) = nearest else {
            continue;
        };
        let (x, y) = tile_of(at);
        let (mx, my) = tile_of(me);
        // An attack-move on the machine, now and then, which the keys
        // drop again at their next word.
        if phase % 230 == 115 && !how.afield {
            out.push(Command::Crew {
                slot,
                order: CrewOrder::AttackMove { x: at.x, y: at.y },
            });
        }
        if phase % 70 == 35 {
            let turn = (phase / 70) as usize;
            let abilities = class_abilities(how.classes[who], slot, (x, y), (mx, my));
            if !abilities.is_empty() {
                out.push(abilities[turn % abilities.len()]);
            }
        }
        if phase % 190 == 90 && !how.afield {
            let item = ((phase / 190) % 6) as u32;
            // The blink and the smoke at the machine, the tether at the
            // other player, the rest where the Bim stands.
            let there = match item {
                0 | 2 => at,
                3 => room.bim_pos(1 - who),
                _ => me,
            };
            out.push(Command::UseItem {
                slot,
                item,
                x: there.x as i32,
                y: there.y as i32,
            });
        }
    }
    out
}

/// Every ability a class has, at the machine at `at` (a room tile) or
/// about the Bim at `me`.
fn class_abilities(class: Class, slot: u32, at: (i32, i32), me: (i32, i32)) -> Vec<Command> {
    let (x, y) = at;
    match class {
        Class::Soldier => vec![
            Command::Throw { slot, x, y },
            Command::StunShot { slot, x, y },
            Command::Rampage { slot },
        ],
        Class::Medic => vec![
            Command::Beam {
                slot,
                patient: Some(1 - slot),
            },
            Command::HealDrone { slot },
            Command::HealingCircle { slot, on: true },
            Command::Beam {
                slot,
                patient: None,
            },
        ],
        Class::Tank => vec![
            Command::RiotShield { slot, on: true },
            Command::Reflect { slot },
            Command::Bastion { slot },
            Command::RiotShield { slot, on: false },
        ],
        Class::Engineer => vec![
            Command::Deploy {
                slot,
                kind: world::DeployKind::Mine,
                x: me.0 + 1,
                y: me.1,
            },
            Command::ThrowAt {
                slot,
                satchel: true,
                x,
                y,
            },
            Command::Detonate { slot },
            Command::Sentry {
                slot,
                tile: (me.0, me.1 + 1),
            },
            Command::Deploy {
                slot,
                kind: world::DeployKind::HealingSentry,
                x: me.0 - 1,
                y: me.1,
            },
        ],
        Class::Commander => vec![
            Command::Rally { slot },
            Command::BattleCry { slot },
            Command::Reinforce { slot },
            Command::Medivac { slot },
        ],
        Class::None => Vec::new(),
    }
}

/// The world as pretty text: every saved field on a line of its own, for
/// [`parting`] to say where two part.
fn text(world: &World) -> String {
    ron::ser::to_string_pretty(world, ron::ser::PrettyConfig::default()).expect("a world writes")
}

/// Where two worlds' texts first part: the fields down to the line, list
/// entries by their index, and the two lines. `None` where they agree.
fn parting(a: &str, b: &str) -> Option<String> {
    let la: Vec<&str> = a.lines().collect();
    let lb: Vec<&str> = b.lines().collect();
    let i = (0..la.len().max(lb.len())).find(|&i| la.get(i) != lb.get(i))?;
    let indent = |s: &str| s.len() - s.trim_start().len();
    let mut path = Vec::new();
    let mut depth = la.get(i).or(lb.get(i)).map_or(0, |l| indent(l));
    let mut child = i;
    for j in (0..i).rev() {
        let line = la[j];
        if line.trim().is_empty() || indent(line) >= depth {
            continue;
        }
        // An entry of a list: which, by the entries opened before it at
        // its own depth under this line.
        let entry_depth = indent(la[child]);
        let index = (j + 1..child)
            .filter(|&k| {
                indent(la[k]) == entry_depth && !la[k].trim_start().starts_with([')', ']', '}'])
            })
            .count();
        let head = line
            .trim()
            .trim_end_matches(['(', '[', '{', ' '])
            .trim_end_matches(':');
        if line.trim_end().ends_with('[') {
            path.push(format!("{head}[{index}]"));
        } else if !head.is_empty() {
            path.push(head.to_string());
        }
        depth = indent(line);
        child = j;
        if depth == 0 {
            break;
        }
    }
    path.reverse();
    let differing = la.iter().zip(&lb).filter(|(x, y)| x != y).count();
    let lines = |l: &[&str]| {
        (i.saturating_sub(3)..(i + 4).min(l.len()))
            .map(|k| format!("  {} {}", if k == i { ">" } else { " " }, l[k]))
            .collect::<Vec<_>>()
            .join("\n")
    };
    Some(format!(
        "line {i}, {}\n  the original:\n{}\n  the copy:\n{}\n  ({differing} lines differ of {} and {})",
        path.join(" / "),
        lines(&la),
        lines(&lb),
        la.len(),
        lb.len()
    ))
}

/// The world in its one-line text, as a save writes it: what two worlds
/// are compared whole by, cheaply.
fn compact(world: &World) -> String {
    ron::to_string(world).expect("a world writes")
}

fn world_of(session: &Session) -> &World {
    &session.game.as_ref().expect("a game").world
}

/// One step of a session's game with these orders, the way the app takes
/// it: queued, then stepped.
fn step_with(session: &mut Session, commands: &[Command]) {
    let game = session.game.as_mut().expect("a game");
    game.queued.extend_from_slice(commands);
    session.world_step();
}

/// The host's world and a copy agree: by checksum, and, given the host's
/// text (`whole`), by every saved field. A panic that says where they
/// parted otherwise.
fn agree(
    name: &str,
    what: &str,
    from: u64,
    age: u64,
    host: &World,
    copy: &World,
    whole: Option<&str>,
) {
    let (a, b) = (host.checksum(), copy.checksum());
    if a == b && whole.is_none_or(|text| text == compact(copy)) {
        return;
    }
    let parted = parting(&text(host), &text(copy)).unwrap_or_else(|| {
        // The pretty texts agree: the one-line ones say where.
        let (one, other) = (whole.map(str::to_string).unwrap_or_default(), compact(copy));
        match one.bytes().zip(other.bytes()).position(|(x, y)| x != y) {
            Some(at) if whole.is_some() => {
                let near = |t: &str| {
                    let lo = t.floor_char_boundary(at.saturating_sub(300));
                    let hi = t.ceil_char_boundary((at + 300).min(t.len()));
                    t[lo..hi].to_string()
                };
                format!(
                    "the pretty texts agree, the one-line ones part at byte {at}:\n  the original: {}\n  the copy:     {}",
                    near(&one),
                    near(&other)
                )
            }
            _ => "every saved field agrees; the checksum alone parts".to_string(),
        }
    });
    let whole = whole.is_some();
    panic!(
        "{name}: the world {what} at step {from} parted from the host's {age} steps on \
         (checksum {a:#x} against {b:#x}{}):\n{parted}",
        if whole && a == b {
            let since = WHOLE_AT.iter().copied().filter(|&at| at < age).max();
            format!(", since {} steps on", since.unwrap_or(0))
        } else {
            String::new()
        }
    );
}

/// What a fight came to: enough to say it was one.
#[derive(Default, Debug)]
struct Fought {
    /// Machines down, all told (each wave's at its end).
    machines_down: u32,
    /// Steps a crew member spent downed.
    downed_steps: u32,
    /// Steps a crew member spent afield, on a planet's plain.
    afield_steps: u32,
    /// Copies read back and stepped beside the host.
    read_back: u32,
    /// The longest save written, in bytes: what a resync sends.
    largest_save: usize,
}

/// The host's fight played for `steps` steps, saved and read back as a
/// guest every [`EVERY`] steps and cloned beside it, each copy stepped
/// with the host on the same orders for [`WINDOW`] steps and compared at
/// every one.
fn play(name: &str, mut host: Session, how: How, steps: u64) -> Fought {
    struct Copy {
        from: u64,
        back: Session,
        clone: World,
    }
    let mut copies: Vec<Copy> = Vec::new();
    let mut fought = Fought::default();
    let mut standing = world_of(&host).droids_standing();
    for step in 0..steps {
        if step % EVERY == 0 {
            let saved = host.save().expect("a world to save");
            fought.largest_save = fought.largest_save.max(saved.len());
            let back = Session::restore_as(&saved, 1, W, H).expect("the save reads back");
            let clone = world_of(&host).clone();
            let text = compact(world_of(&host));
            let whole = Some(text.as_str());
            agree(
                name,
                "read back",
                step,
                0,
                world_of(&host),
                world_of(&back),
                whole,
            );
            agree(name, "cloned", step, 0, world_of(&host), &clone, whole);
            copies.push(Copy {
                from: step,
                back,
                clone,
            });
            fought.read_back += 1;
        }
        let commands = orders(world_of(&host), step, how);
        step_with(&mut host, &commands);
        // Drawn now and then, as a window draws it.
        if step % DRAWS == 0 {
            host.render();
        }
        let world = world_of(&host);
        let text = copies
            .iter()
            .any(|c| WHOLE_AT.contains(&(step + 1 - c.from)))
            .then(|| compact(world));
        for copy in &mut copies {
            step_with(&mut copy.back, &commands);
            if step % DRAWS == 0 {
                copy.back.render();
            }
            copy.clone.step(&commands);
            let age = step + 1 - copy.from;
            let whole = text.as_deref().filter(|_| WHOLE_AT.contains(&age));
            agree(
                name,
                "read back",
                copy.from,
                age,
                world,
                world_of(&copy.back),
                whole,
            );
            // A clone is a derive's copy, compared whole as it is made; it
            // is never drawn, so from then on it is the checksum's.
            agree(name, "cloned", copy.from, age, world, &copy.clone, None);
        }
        copies.retain(|c| step + 1 - c.from < WINDOW);
        let world = world_of(&host);
        let now = world.droids_standing();
        fought.machines_down += standing.saturating_sub(now);
        standing = now;
        let room = &world.aboard.room;
        fought.downed_steps += (0..room.crew_count() as usize)
            .filter(|&i| room.is_downed(i))
            .count() as u32;
        fought.afield_steps += (0..room.crew_count() as usize)
            .filter(|&i| room.is_afield_for_probe(i))
            .count() as u32;
    }
    fought
}

/// A fight that was one: machines went down and so did the crew.
fn a_fight(name: &str, fought: &Fought) {
    eprintln!("{name}: {fought:?}");
    assert!(
        fought.machines_down >= 4,
        "{name}: machines went down: {fought:?}"
    );
}

#[test]
fn a_soldier_and_a_medic_s_fight_read_back_steps_as_the_host_s() {
    let classes = [Class::Soldier, Class::Medic];
    let host = arena(world::data::DEFAULT_SEED, classes, Tier::Two, 14, 6);
    let fought = play("soldier and medic", host, How::fight(classes), 2_400);
    a_fight("soldier and medic", &fought);
}

#[test]
fn a_tank_and_an_engineer_s_fight_read_back_steps_as_the_host_s() {
    let classes = [Class::Tank, Class::Engineer];
    // A dark station, so the bolts' light is the rule's (`Sight::flare`).
    let mut host = crew(world::data::DEFAULT_SEED, classes, Tier::Three);
    let world = &mut host.game.as_mut().unwrap().world;
    world.set_dark_for_probe(Some(true));
    world.set_droid_wave_for_probe(16);
    assert!(host.infest_the_dock_for_probe(Some(Tier::Three), 0.5, 6));
    let fought = play("tank and engineer", host, How::fight(classes), 2_400);
    a_fight("tank and engineer", &fought);
}

#[test]
fn a_soldier_and_a_commander_abroad_read_back_step_as_the_host_s() {
    let classes = [Class::Soldier, Class::Commander];
    let host = abroad(world::data::DEFAULT_SEED, classes);
    let fought = play("abroad", host, How::fight(classes), 2_400);
    a_fight("abroad", &fought);
}

#[test]
fn a_commander_and_a_soldier_at_the_heart_read_back_step_as_the_host_s() {
    let classes = [Class::Commander, Class::Soldier];
    let host = heart(world::data::DEFAULT_SEED, classes);
    let fought = play("the Heart", host, How::fight(classes), 2_400);
    a_fight("the Heart", &fought);
}

#[test]
fn a_medic_and_a_soldier_out_on_the_plain_read_back_step_as_the_host_s() {
    let classes = [Class::Medic, Class::Soldier];
    let mut host = town(classes);
    // Everybody at the deck's west edge, the plain a few tiles off.
    let room = &mut host.game.as_mut().unwrap().world.aboard.room;
    let (edge, y) = (room.interior().min.x, room.bim_pos(0).y);
    for who in 0..room.crew_count() as usize {
        let at = bims::math::vec2(edge + 3.0 * TILE, y + (who as f32 - 4.0) * TILE);
        room.put_for_probe(who, at);
    }
    let how = How {
        classes,
        afield: true,
    };
    // The walk off the deck, unwatched: it is a long one.
    for step in 0..1_200 {
        let commands = orders(world_of(&host), step, how);
        step_with(&mut host, &commands);
    }
    let fought = play("the plain", host, how, 2_400);
    eprintln!("the plain: {fought:?}");
    assert!(fought.afield_steps > 3_000, "out on the plain: {fought:?}");
}

#[test]
fn an_engineer_and_a_medic_holding_a_town_read_back_step_as_the_host_s() {
    let classes = [Class::Engineer, Class::Medic];
    let host = town(classes);
    let fought = play("a town", host, How::fight(classes), 2_400);
    a_fight("a town", &fought);
}

/// Every mission's fight, a while of it, saved and read back as the rest.
fn a_mission_read_back(m: world::run::Mission, classes: [Class; 2]) {
    let name = format!("{m:?}");
    let host = mission(world::data::DEFAULT_SEED, m, classes, Tier::Two);
    let fought = play(&name, host, How::fight(classes), 600);
    eprintln!("{name}: {fought:?}");
}

macro_rules! missions {
    ($($test:ident: $mission:ident, $a:ident, $b:ident;)*) => {$(
        #[test]
        fn $test() {
            a_mission_read_back(world::run::Mission::$mission, [Class::$a, Class::$b]);
        }
    )*};
}

missions! {
    mission_plain_read_back: Plain, Soldier, Tank;
    mission_breaches_read_back: Breaches, Engineer, Soldier;
    mission_sabotage_read_back: Sabotage, Medic, Commander;
    mission_evacuation_read_back: Evacuation, Tank, Medic;
    mission_nests_read_back: Nests, Commander, Engineer;
    mission_overseer_read_back: Overseer, Soldier, Medic;
    mission_heist_read_back: Heist, Engineer, Tank;
    mission_prison_read_back: Prison, Commander, Soldier;
    mission_fuelrun_read_back: FuelRun, Medic, Engineer;
    mission_salvage_read_back: Salvage, Tank, Commander;
    mission_bombs_read_back: Bombs, Soldier, Engineer;
    mission_doors_read_back: Doors, Medic, Tank;
    mission_chief_read_back: Chief, Commander, Medic;
}
