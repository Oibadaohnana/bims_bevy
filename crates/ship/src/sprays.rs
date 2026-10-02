//! The particles over the fight, for the host's GPU: what the classes'
//! abilities look like, and every room's sprays (`bims::fx::Spray`)
//! turned into the camera's units.
//!
//! **Drawing only**, like the fight's passing lights it rides on: the
//! sprays are spawned into the crew's room's `Fx`, which records nothing
//! until a host ages it (`Session::age_effects`), so a test, a probe or a
//! server spawns none. Nothing the world reads is touched, and which way a
//! particle flies is the GPU's hash of a spray's seed — never a roll.
//!
//! Two kinds of ability picture:
//!
//! - **The moment it is used** ([`abilities`]), off the world's events as
//!   [`crate::game::Game::step`] and `send` collect them — every peer steps
//!   the same world, so every window sees every player's: a Rally's and a
//!   Battle Cry's ring of light running out over the whole of their
//!   reach with motes rising in it (the commander's area), a Nanite
//!   Burst's green, a Taunt's red shockwave, a Bulwark's and a Stun Shot's
//!   sparks, a Rampage's and a Juggernaut's flare, a cloak drawn in, a
//!   reinforcement beamed down.
//! - **While it runs** ([`running`]), a beat of [`BEAT`] real seconds at a
//!   time: embers off a Rampage or a Juggernaut, gold motes over a Bim a
//!   Rally reaches, sparks off a Battle Cry's crier, a shimmer off a
//!   cloaked body, green motes along a medic's beam and a Healing
//!   Sentry's lines.

use bims::draw::Color;
use bims::fx::{Spray, SprayKind};
use bims::math::{Vec2, vec2};
use shipdesign::TILE;
use world::WorldEvent;

use crate::game::Game;

/// The colours of the classes' lights, each its class's family
/// (`ability_icons.rs` in the app): the commander's gold and his cry's
/// orange, the medic's green, the tank's red and steel blue, the
/// soldier's blue and his rampage's fire, the cloak's pale cyan.
pub const RALLY_GOLD: Color = Color::rgb(1.0, 0.8, 0.32);
pub const CRY_ORANGE: Color = Color::rgb(1.0, 0.52, 0.2);
pub const NANITE_GREEN: Color = Color::rgb(0.42, 1.0, 0.58);
pub const TAUNT_RED: Color = Color::rgb(1.0, 0.3, 0.24);
pub const JUGGERNAUT_RED: Color = Color::rgb(0.85, 0.12, 0.1);
pub const WALL_BLUE: Color = Color::rgb(0.55, 0.76, 1.0);
pub const BRACE_BLUE: Color = Color::rgb(0.45, 0.68, 1.0);
pub const RAMPAGE_FIRE: Color = Color::rgb(1.0, 0.42, 0.16);
pub const CLOAK_CYAN: Color = Color::rgb(0.72, 0.92, 1.0);
pub const BEAM_DOWN: Color = Color::rgb(0.6, 0.85, 1.0);
pub const KIT_AMBER: Color = Color::rgb(1.0, 0.76, 0.32);

/// How often a running ability gives off its next few particles, in real
/// seconds.
pub const BEAT: f32 = 1.0 / 12.0;

/// Where crew member `who` stands in the crew's room, if alive there.
fn crew_at(game: &Game, who: u32) -> Option<Vec2> {
    let room = &game.world.aboard.room;
    (who < room.crew_count() && room.is_alive(who as usize)).then(|| room.bim_pos(who as usize))
}

fn tiles(n: f32) -> f32 {
    n * TILE as f32
}

/// A ring of light running out to `reach`, and motes rising all over the
/// ground it covers: an aura called.
fn aura(game: &mut Game, at: Vec2, reach: f32, colour: Color) {
    let room = &mut game.world.aboard.room;
    let up = vec2(0.0, -1.0);
    room.spray(
        Spray::along(SprayKind::Nova, at, up)
            .reach(reach)
            .life(0.7)
            .colour(colour.glowing(2.2))
            .count(64)
            .dot(6.0),
    );
    room.spray(
        Spray::along(SprayKind::Nova, at, up)
            .reach(reach * 0.55)
            .life(0.5)
            .colour(colour.glowing(1.6))
            .count(32)
            .dot(4.0),
    );
    room.spray(
        Spray::along(SprayKind::Motes, at, up)
            .reach(reach)
            .life(1.6)
            .colour(colour.glowing(1.5))
            .count(48)
            .dot(3.2),
    );
    room.spray(
        Spray::along(SprayKind::Sparks, at, up)
            .reach(tiles(1.6))
            .life(0.4)
            .colour(colour.glowing(2.4))
            .count(16)
            .spread(core::f32::consts::TAU)
            .dot(1.6),
    );
}

/// A flare off one body: sparks every way and embers.
fn flare(game: &mut Game, at: Vec2, colour: Color, reach: f32) {
    let room = &mut game.world.aboard.room;
    let up = vec2(0.0, -1.0);
    room.spray(
        Spray::along(SprayKind::Sparks, at, up)
            .reach(reach)
            .life(0.45)
            .colour(colour.glowing(2.4))
            .count(20)
            .spread(core::f32::consts::TAU)
            .dot(1.8),
    );
    room.spray(
        Spray::along(SprayKind::Embers, at, up)
            .reach(reach * 0.7)
            .life(0.8)
            .colour(colour.glowing(1.9))
            .count(16)
            .dot(3.0),
    );
}

/// Motes drawn in to a body from round it: a cloak closing, a beam down.
fn gather(game: &mut Game, at: Vec2, colour: Color, reach: f32, count: u32) {
    game.world.aboard.room.spray(
        Spray::along(SprayKind::Swirl, at, vec2(0.0, -1.0))
            .reach(reach)
            .life(0.6)
            .colour(colour.glowing(1.8))
            .count(count)
            .dot(2.6),
    );
}

/// A puff of dust where a body set something down.
fn dust(game: &mut Game, at: Vec2) {
    game.world.aboard.room.spray(
        Spray::along(SprayKind::Smoke, at, vec2(0.0, -1.0))
            .reach(tiles(0.7))
            .life(1.0)
            .colour(bims::fx::DUST)
            .count(10)
            .dot(10.0),
    );
}

/// The particles for the abilities the world says were used this step.
pub fn abilities(game: &mut Game, events: &[WorldEvent]) {
    if !game.world.aboard.room.fx_on() {
        return;
    }
    for event in events {
        ability(game, *event);
    }
}

fn ability(game: &mut Game, event: WorldEvent) {
    use WorldEvent as E;
    let who = match event {
        E::Rallied { who }
        | E::BattleCried { who }
        | E::NaniteBurst { who, .. }
        | E::Taunted { who }
        | E::Juggernaut { who }
        | E::Rampaged { who }
        | E::Bulwarked { who, on: true }
        | E::ShotCharging { who }
        | E::Deployed { who, .. }
        | E::Reinforced { who, .. }
        | E::Medivac { who, .. }
        | E::Cloaked { who, .. } => who,
        _ => return,
    };
    let Some(at) = crew_at(game, who) else {
        return;
    };
    match event {
        E::Rallied { .. } => aura(game, at, tiles(world::class::RALLY_TILES), RALLY_GOLD),
        E::BattleCried { .. } => {
            aura(game, at, tiles(world::class::BATTLE_CRY_TILES), CRY_ORANGE);
            flare(game, at, CRY_ORANGE, tiles(1.4));
        }
        E::NaniteBurst { .. } => {
            let reach = tiles(game.world.nanite_burst_radius(who));
            aura(game, at, reach, NANITE_GREEN);
        }
        E::Taunted { .. } => {
            let reach = tiles(game.world.taunt_radius(who));
            let room = &mut game.world.aboard.room;
            room.spray(
                Spray::along(SprayKind::Nova, at, vec2(0.0, -1.0))
                    .reach(reach)
                    .life(0.55)
                    .colour(TAUNT_RED.glowing(2.2))
                    .count(64)
                    .dot(7.0),
            );
            flare(game, at, TAUNT_RED, tiles(1.2));
        }
        E::Juggernaut { .. } => {
            let room = &mut game.world.aboard.room;
            room.spray(
                Spray::along(SprayKind::Nova, at, vec2(0.0, -1.0))
                    .reach(tiles(5.0))
                    .life(0.6)
                    .colour(JUGGERNAUT_RED.glowing(2.4))
                    .count(48)
                    .dot(9.0),
            );
            flare(game, at, JUGGERNAUT_RED, tiles(2.0));
        }
        E::Rampaged { .. } => flare(game, at, RAMPAGE_FIRE, tiles(2.0)),
        E::Bulwarked { .. } => {
            let reach = tiles(game.world.bulwark_reach(who));
            let room = &mut game.world.aboard.room;
            room.spray(
                Spray::along(SprayKind::Nova, at, vec2(0.0, -1.0))
                    .reach(reach)
                    .life(0.45)
                    .colour(WALL_BLUE.glowing(2.0))
                    .count(32)
                    .dot(5.0),
            );
            flare(game, at, WALL_BLUE, tiles(1.0));
        }
        E::ShotCharging { .. } => {
            dust(game, at);
            game.world.aboard.room.spray(
                Spray::along(SprayKind::Sparks, at, vec2(0.0, -1.0))
                    .reach(tiles(0.8))
                    .life(0.3)
                    .colour(BRACE_BLUE.glowing(2.2))
                    .count(10)
                    .spread(core::f32::consts::TAU)
                    .dot(1.4),
            );
        }
        E::Deployed { .. } => {
            dust(game, at);
            game.world.aboard.room.spray(
                Spray::along(SprayKind::Sparks, at, vec2(0.0, -1.0))
                    .reach(tiles(0.7))
                    .life(0.3)
                    .colour(KIT_AMBER.glowing(2.0))
                    .count(8)
                    .spread(core::f32::consts::TAU)
                    .dot(1.3),
            );
        }
        E::Cloaked { target, .. } => {
            let at = crew_at(game, target).unwrap_or(at);
            gather(game, at, CLOAK_CYAN, tiles(1.4), 32);
            game.world.aboard.room.spray(
                Spray::along(SprayKind::Smoke, at, vec2(0.0, -1.0))
                    .reach(tiles(0.5))
                    .life(0.9)
                    .colour(CLOAK_CYAN.alpha(0.18))
                    .count(8)
                    .dot(14.0),
            );
        }
        E::Reinforced { count, .. } => {
            // Each soldier called in, beamed down where he stands.
            let fresh: Vec<u32> = game
                .world
                .reinforcements
                .iter()
                .filter(|r| r.by == who && !r.medic)
                .map(|r| r.who)
                .collect();
            let skip = fresh.len().saturating_sub(count as usize);
            for r in fresh.into_iter().skip(skip) {
                if let Some(p) = crew_at(game, r) {
                    beam_down(game, p);
                }
            }
            aura(
                game,
                at,
                tiles(world::class::REINFORCEMENT_REACH_TILES),
                BEAM_DOWN,
            );
        }
        E::Medivac { medic, .. } => {
            if let Some(p) = crew_at(game, medic) {
                beam_down(game, p);
            }
        }
        _ => {}
    }
}

/// A body arriving out of the sky: light drawn down onto it and a ring
/// thrown off where it lands.
fn beam_down(game: &mut Game, at: Vec2) {
    gather(game, at, BEAM_DOWN, tiles(1.6), 32);
    game.world.aboard.room.spray(
        Spray::along(SprayKind::Nova, at, vec2(0.0, -1.0))
            .reach(tiles(1.4))
            .life(0.5)
            .colour(BEAM_DOWN.glowing(2.2))
            .count(24)
            .dot(4.0),
    );
    dust(game, at);
}

/// What the abilities still running give off over `real` seconds of the
/// host's frame — nought while paused, when nothing is given off.
pub fn running(game: &mut Game, real: f32) {
    if real <= 0.0 || !game.world.aboard.room.fx_on() {
        return;
    }
    game.spray_due += real;
    if game.spray_due < BEAT {
        return;
    }
    // One beat a frame at most: a long frame is not a burst of them.
    game.spray_due = (game.spray_due - BEAT).min(BEAT);
    let crew = game.world.aboard.room.crew_count();
    let up = vec2(0.0, -1.0);
    for who in 0..crew {
        let Some(at) = crew_at(game, who) else {
            continue;
        };
        let w = &game.world;
        let rampage = w.is_rampaging(who);
        let juggernaut = w.is_juggernaut(who);
        let rallied = w.rally_reaching(who).is_some();
        let crying = w.is_crying(who);
        let cloaked = w.cloak_left(who) > 0.0;
        let patients: Vec<Vec2> = w
            .patients_of(who)
            .into_iter()
            .filter_map(|p| crew_at(game, p))
            .collect();
        let room = &mut game.world.aboard.room;
        if rampage || juggernaut {
            let colour = if juggernaut {
                JUGGERNAUT_RED
            } else {
                RAMPAGE_FIRE
            };
            room.spray(
                Spray::along(SprayKind::Motes, at, up)
                    .reach(tiles(0.4))
                    .life(0.7)
                    .colour(colour.glowing(1.9))
                    .count(3)
                    .dot(2.4),
            );
        }
        if rallied {
            room.spray(
                Spray::along(SprayKind::Motes, at, up)
                    .reach(tiles(0.45))
                    .life(0.9)
                    .colour(RALLY_GOLD.glowing(1.5))
                    .count(1)
                    .dot(2.2),
            );
        }
        if crying {
            room.spray(
                Spray::along(SprayKind::Sparks, at, up)
                    .reach(tiles(0.6))
                    .life(0.3)
                    .colour(CRY_ORANGE.glowing(2.0))
                    .count(2)
                    .spread(1.2)
                    .dot(1.3),
            );
        }
        if cloaked {
            room.spray(
                Spray::along(SprayKind::Swirl, at, up)
                    .reach(tiles(0.6))
                    .life(0.5)
                    .colour(CLOAK_CYAN.alpha(0.5))
                    .count(1)
                    .dot(2.0),
            );
        }
        for p in patients {
            room.spray(
                Spray::new(SprayKind::Trail, at, p)
                    .reach(6.0)
                    .life(0.5)
                    .colour(NANITE_GREEN.glowing(1.7))
                    .count(3)
                    .dot(2.2),
            );
        }
    }
    // A Healing Sentry's lines to the crew it heals.
    let standing = game.world.deployables_in_room();
    let links = game.world.healing_links();
    for (id, who, _) in links {
        let Some(from) = standing.iter().find(|(d, _)| d.id == id).map(|s| s.1) else {
            continue;
        };
        let Some(to) = crew_at(game, who) else {
            continue;
        };
        game.world.aboard.room.spray(
            Spray::new(SprayKind::Trail, from, to)
                .reach(5.0)
                .life(0.5)
                .colour(NANITE_GREEN.glowing(1.5))
                .count(2)
                .dot(2.0),
        );
    }
}

/// Every room's sprays since the last call, turned into the camera's
/// units — the crew's room through the ship's turn, a station's room
/// through its own frame — for the host's GPU.
pub fn take(game: &mut Game) -> Vec<Spray> {
    let mut out = game.world.aboard.room.take_sprays();
    for s in &mut out {
        s.at = on_screen(crate::world_paint::room_point_on_screen(game, s.at));
        s.to = on_screen(crate::world_paint::room_point_on_screen(game, s.to));
    }
    let Some(residents) = game.world.residents.as_mut() else {
        return out;
    };
    let theirs = residents.aboard.room.take_sprays();
    if theirs.is_empty() {
        return out;
    }
    let Some(frame) = resident_frame(game) else {
        return out;
    };
    out.extend(theirs.into_iter().map(|mut s| {
        s.at = frame(s.at);
        s.to = frame(s.to);
        s
    }));
    out
}

fn on_screen((x, y): (f32, f32)) -> Vec2 {
    vec2(x, y)
}

/// A point of the residents' room into the camera's units: the turn and
/// the shift `world_paint::stations` lays their room's picture with.
fn resident_frame(game: &Game) -> Option<impl Fn(Vec2) -> Vec2 + use<>> {
    let residents = game.world.residents.as_ref()?;
    let station = game.world.station(residents.station)?;
    let turn = game.camera_turn() as f32;
    let offset = station.centre().sub(game.world.ship.position());
    let at = crate::game::turned(offset.x as f32, -offset.y as f32, turn);
    let side = station.design.build_area as f32 * TILE as f32;
    let shift = residents.aboard.offset;
    let pivot = (side / 2.0 + shift.x as f32, side / 2.0 + shift.y as f32);
    Some(move |p: Vec2| {
        let (x, y) = crate::game::turned(p.x - pivot.0, p.y - pivot.1, turn);
        vec2(x + at.0, y + at.1)
    })
}
