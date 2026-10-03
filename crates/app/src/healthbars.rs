//! The health bar over every body on the deck (task 137), the way Dota
//! draws one: a short bar over the head, green for a friend and red for
//! an enemy, the crew's and the station's people's and the machines'
//! alike. What a body lost in the last second stays on the bar in white
//! and then drains away, and what it got back in the last second shows
//! in a lighter tone of its colour, the solid filling in behind it — so a
//! steady bleed or mend is a short white or light edge riding the bar,
//! never a stretch from where it began, and a glance across a fight says
//! who is being hit and who is being mended.
//!
//! # A picture, kept by the screen
//!
//! Nothing here is the world's. A bar is read off the room every frame —
//! a Bim's hit points out of its whole bar (`Game::max_health`) with its worn
//! armour's health in the armour's blue on the end, as the HUD draws it;
//! a machine's one health, its four parts' added together — and the white
//! and the light parts are this window's memory of what the bar was a
//! moment ago, aged on its own clock: real seconds at every speed, held
//! still while the game is paused, like the fight's passing lights.
//! Only a body standing gets one: the downed wear their countdown ring,
//! and the dead and the wrecks nothing.

use std::collections::HashMap;

use bevy_egui::egui;
use ship::game::Game;
use ship::world_paint;

/// How fast the white of a hit drains once it is out of the window, in
/// bars a second — a whole bar lost drains in a little over half a
/// second — and how fast the solid catches up with the light of a heal.
const DRAIN: f32 = 1.6;
/// The hits the white shows and the heals the light shows: those of the
/// last second, as Dota's bar does, kept in [`SLICES`] slices of a tenth
/// of a second each, the newest first.
const SLICES: usize = 10;
const SLICE: f32 = 0.1;

/// The bar at 100% zoom, in points, and the least and most it is drawn
/// at as the camera zooms: readable far out, not a banner close in.
const WIDTH: f32 = 50.0;
const WIDTH_RANGE: (f32, f32) = (40.0, 70.0);
const HEIGHT: f32 = 7.0;
const HEIGHT_RANGE: (f32, f32) = (6.0, 10.0);
/// A sentry's bar is this share of a body's, and sits this share as
/// high: a thing laid on the deck, not somebody standing there.
const SMALL: f32 = 0.7;
const SMALL_LIFT: f32 = 0.8;

const ALLY: egui::Color32 = egui::Color32::from_rgb(0x3f, 0xc2, 0x4a);
const ALLY_HEALED: egui::Color32 = egui::Color32::from_rgb(0xa8, 0xf0, 0xae);
const ENEMY: egui::Color32 = egui::Color32::from_rgb(0xd9, 0x33, 0x2b);
const ENEMY_HEALED: egui::Color32 = egui::Color32::from_rgb(0xff, 0xa3, 0x9c);
const DAMAGE: egui::Color32 = egui::Color32::from_rgb(0xf4, 0xf4, 0xf0);
const TRACK: egui::Color32 = egui::Color32::from_rgb(0x14, 0x17, 0x16);
/// The ticks cutting the bar into chunks of hit points, as Dota's cut a
/// bar, so how much is left — and how big a bar is — can be counted at a
/// glance: a thin black line every [`TICK_HP`], a thicker one every
/// [`BIG_TICK_HP`]. A tick closer to the next than [`TICK_ROOM`] points is
/// left out, the thin ones first, so a machine's long bar is not solid
/// black.
const TICK: egui::Color32 = egui::Color32::from_black_alpha(150);
const BIG_TICK: egui::Color32 = egui::Color32::from_black_alpha(235);
const TICK_HP: f32 = 25.0;
const BIG_TICK_HP: f32 = 100.0;
/// The thin tick's and the thick one's width, in physical pixels.
const TICK_PX: f32 = 1.0;
const BIG_TICK_PX: f32 = 2.0;
const TICK_ROOM: f32 = 2.5;

/// Which room a body is of: the crew's, or the station's people's.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Room {
    Crew,
    Residents(u32),
    /// One of the crew's sentries, by the world's id.
    Sentry,
}

/// One body's bar as this window remembers it, every share nought to one.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Trail {
    /// The bar as it stands.
    now: f32,
    /// The top of the white: what the bar was before the hits of the last
    /// second. Never below `now`.
    lost: f32,
    /// The top of the solid fill: what the bar was before the heals of
    /// the last second, the light part running from here to `now`. Never
    /// above it.
    kept: f32,
    /// What was taken and what was healed in each slice of the last
    /// second, the newest first, and how far into the newest slice the
    /// clock is.
    hurt: [f32; SLICES],
    healed: [f32; SLICES],
    slice_age: f32,
}

/// Take `amount` off `slices`, the newest first: a hit eats the light
/// of the latest heals, a heal fills in the white of the latest hits.
fn eat(slices: &mut [f32; SLICES], mut amount: f32) {
    for slice in slices {
        let eaten = slice.min(amount);
        *slice -= eaten;
        amount -= eaten;
    }
}

impl Trail {
    fn new(share: f32) -> Trail {
        Trail {
            now: share,
            lost: share,
            kept: share,
            hurt: [0.0; SLICES],
            healed: [0.0; SLICES],
            slice_age: 0.0,
        }
    }

    /// The same bar read against a whole `factor` times what it was read
    /// against: every share times it, so a bar whose whole grew or
    /// shrank — a shield thrown over the body, or draining away — shows
    /// no hit or heal for it.
    fn rescale(&mut self, factor: f32) {
        self.now *= factor;
        self.lost *= factor;
        self.kept *= factor;
        for slice in self.hurt.iter_mut().chain(self.healed.iter_mut()) {
            *slice *= factor;
        }
    }

    /// The bar reads `share` now, `dt` real seconds after it last did.
    fn step(&mut self, share: f32, dt: f32) {
        if share < self.now {
            // A hit: the white shows it for a second, and a heal still
            // showing light is the first thing it eats into.
            self.hurt[0] += self.now - share;
            eat(&mut self.healed, self.now - share);
        } else if share > self.now {
            // A heal: the light shows it for a second, and it fills in
            // the white of a hit still showing first.
            self.healed[0] += share - self.now;
            eat(&mut self.hurt, share - self.now);
        }
        self.now = share;
        // The slices age, the oldest falling out of the window.
        self.slice_age += dt;
        let mut turns = 0;
        while self.slice_age >= SLICE {
            self.slice_age -= SLICE;
            if turns < SLICES {
                for slices in [&mut self.hurt, &mut self.healed] {
                    slices.rotate_right(1);
                    slices[0] = 0.0;
                }
                turns += 1;
            }
        }
        // The white's top and the solid's jump at once to take in a new
        // hit or heal, and go back to the bar at DRAIN's pace once it is
        // out of the window, so neither snaps away.
        let white = share + self.hurt.iter().sum::<f32>();
        let solid = share - self.healed.iter().sum::<f32>();
        if self.lost < white {
            self.lost = white;
        } else {
            self.lost = (self.lost - DRAIN * dt).max(white);
        }
        if self.kept > solid {
            self.kept = solid;
        } else {
            self.kept = (self.kept + DRAIN * dt).min(solid);
        }
        self.lost = self.lost.max(share);
        self.kept = self.kept.clamp(0.0, share);
    }
}

/// What a body has, each a share of its whole bar: its hit points, the
/// armour it wears on the end of them, and a shield on the end of that
/// (a tank's Bastion, a relic's *Lifeline*); and how many hit points the
/// whole bar is, for the ticks.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Reading {
    health: f32,
    armour: f32,
    shield: f32,
    whole: f32,
}

/// A body's two trails: its health alone, for the light of a heal —
/// only hit points are ever put back — and its health, armour and
/// shield together, for the white of a hit, which takes the shield and
/// the armour first. The shield's share as it stands, and the whole the
/// shares were last read against.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Trails {
    health: Trail,
    all: Trail,
    shield: f32,
    whole: f32,
}

impl Trails {
    fn new(r: Reading) -> Trails {
        Trails {
            health: Trail::new(r.health),
            all: Trail::new(r.health + r.armour + r.shield),
            shield: r.shield,
            whole: r.whole,
        }
    }

    fn step(&mut self, r: Reading, dt: f32) {
        // A whole that moved — a shield come or going — is the bar
        // redrawn to a new scale, not a hit or a heal.
        if self.whole > 0.0 && r.whole > 0.0 && self.whole != r.whole {
            let factor = self.whole / r.whole;
            self.health.rescale(factor);
            self.all.rescale(factor);
        }
        self.whole = r.whole;
        self.shield = r.shield;
        self.health.step(r.health, dt);
        self.all.step(r.health + r.armour + r.shield, dt);
    }
}

/// One bar to draw: where the body is in the camera's units, whose side
/// it is on, and its trails.
struct Bar {
    at: (f32, f32),
    friendly: bool,
    trails: Trails,
    /// The hit points the whole bar stands for.
    whole: f32,
    /// A sentry's: the smaller bar, lower down.
    small: bool,
}

/// Every bar on the deck and what each remembers. Kept by the game
/// screen.
#[derive(Default)]
pub struct HealthBars {
    trails: HashMap<(Room, u32), Trails>,
    shown: Vec<Bar>,
}

/// What a body of `room` has, or `None` for one that is down: a Bim
/// downed or dead, a machine a wreck. A Bim's bar is its hit points and
/// its worn armour's health laid end to end, the HUD's bar; a machine's
/// is its one health (task 137), and it wears nothing.
fn reading(room: &bims::game::Game, who: usize) -> Option<Reading> {
    if room.is_down(who) {
        return None;
    }
    let bims = room.crew_count() as usize;
    match who.checked_sub(bims) {
        None => {
            let armour = room.armour_health(who).max(0.0);
            let shield = room.shield_hp(who);
            let whole = room.max_health(who) + armour + shield;
            Some(Reading {
                health: (room.health(who) / whole).clamp(0.0, 1.0),
                armour: armour / whole,
                shield: shield / whole,
                whole,
            })
        }
        Some(i) => {
            let body = &room.droid(i)?.body;
            Some(Reading {
                health: body.life_share(),
                armour: 0.0,
                shield: 0.0,
                whole: body.life_max(),
            })
        }
    }
}

/// The bar's height at the view's `scale`, in points.
fn height(scale: f32) -> f32 {
    (HEIGHT * scale).clamp(HEIGHT_RANGE.0, HEIGHT_RANGE.1)
}

/// How much further up than `theme::NAME_LIFT` a name goes, so it sits
/// over the bar rather than on it.
pub fn name_room(scale: f32) -> f32 {
    height(scale) + 3.0
}

impl HealthBars {
    /// Read every bar off the rooms, `dt` real seconds after the last
    /// frame — nought while the game is paused. Called every frame the
    /// bars are drawn, just before [`HealthBars::paint`].
    pub fn update(&mut self, game: &Game, dt: f32) {
        let mut next: HashMap<(Room, u32), Trails> = HashMap::new();
        self.shown.clear();
        let mut take =
            |key: (Room, u32), r: Reading, at: (f32, f32), friendly: bool, small: bool| {
                let mut trails = self
                    .trails
                    .get(&key)
                    .copied()
                    .unwrap_or_else(|| Trails::new(r));
                trails.step(r, dt);
                next.insert(key, trails);
                self.shown.push(Bar {
                    at,
                    friendly,
                    trails,
                    whole: r.whole,
                    small,
                });
            };
        let crew = &game.world.aboard;
        for who in 0..crew.count() {
            let room = &crew.room;
            if !room.body_seen(who as usize) {
                continue;
            }
            if let Some(s) = reading(room, who as usize) {
                let at = world_paint::crew_on_screen(game, who);
                take(
                    (Room::Crew, who),
                    s,
                    at,
                    room.is_friendly_body(who as usize),
                    false,
                );
            }
        }
        if let Some(residents) = &game.world.residents {
            let room = &residents.aboard.room;
            for who in 0..residents.aboard.count() {
                if !room.body_seen(who as usize) {
                    continue;
                }
                if let Some(s) = reading(room, who as usize) {
                    let at = world_paint::resident_on_screen(game, who);
                    take(
                        (Room::Residents(residents.station), who),
                        s,
                        at,
                        room.is_friendly_body(who as usize),
                        false,
                    );
                }
            }
        }
        // The crew's sentries, the shooting one and the Healing Sentry:
        // what they have left of what they were laid with.
        for (d, at) in game.world.deployables_in_room() {
            if !d.kind.is_sentry() {
                continue;
            }
            let whole = game.world.laid_health(d.kind, d.owner_slot).max(1.0);
            let r = Reading {
                health: (d.health / whole).clamp(0.0, 1.0),
                armour: 0.0,
                shield: 0.0,
                whole,
            };
            let at = world_paint::room_point_on_screen(game, at);
            take((Room::Sentry, d.id), r, at, true, true);
        }
        self.trails = next;
    }

    /// Draw every bar read by the last [`HealthBars::update`], over its
    /// body's head and under its name. `to_screen` puts a point in the
    /// camera's units on the canvas; `scale` is the view's.
    pub fn paint(
        &self,
        painter: &egui::Painter,
        scale: f32,
        to_screen: impl Fn((f32, f32)) -> egui::Pos2,
    ) {
        let w = (WIDTH * scale).clamp(WIDTH_RANGE.0, WIDTH_RANGE.1);
        let h = height(scale);
        for bar in &self.shown {
            let (w, h, lift) = if bar.small {
                (w * SMALL, h * SMALL, SMALL_LIFT)
            } else {
                (w, h, 1.0)
            };
            let head = to_screen(bar.at);
            let middle = egui::pos2(
                head.x,
                head.y - crate::theme::NAME_LIFT * lift * scale - h * 0.5,
            );
            let track = egui::Rect::from_center_size(middle, egui::vec2(w, h));
            let (solid, healed) = if bar.friendly {
                (ALLY, ALLY_HEALED)
            } else {
                (ENEMY, ENEMY_HEALED)
            };
            paint_bar(painter, track, bar.trails, bar.whole, solid, healed);
        }
    }
}

/// One bar in `track`: the dark track with an outline, the solid fill up
/// to the hit points the body kept, the light up to the ones it has now,
/// the armour's blue after them, the shield's pale cyan after that, and
/// the white up to what it had before its last hits.
fn paint_bar(
    painter: &egui::Painter,
    track: egui::Rect,
    trails: Trails,
    whole: f32,
    solid: egui::Color32,
    healed: egui::Color32,
) {
    painter.rect_filled(track.expand(1.0), 1.5, egui::Color32::from_black_alpha(220));
    painter.rect_filled(track, 1.0, TRACK);
    let span = |from: f32, to: f32| {
        egui::Rect::from_min_max(
            egui::pos2(track.min.x + track.width() * from, track.min.y),
            egui::pos2(track.min.x + track.width() * to, track.max.y),
        )
    };
    let (health, kept) = (trails.health.now, trails.health.kept);
    // The white of a draining shield can run past the end of the bar;
    // it is the bar's to draw and no further.
    let (all, lost) = (trails.all.now.min(1.0), trails.all.lost.min(1.0));
    let armoured = (all - trails.shield).max(health);
    if kept > 0.0 {
        painter.rect_filled(span(0.0, kept), 1.0, solid);
    }
    if health > kept {
        painter.rect_filled(span(kept, health), 0.0, healed);
    }
    if armoured > health {
        painter.rect_filled(span(health, armoured), 0.0, crate::theme::ARMOUR);
    }
    if all > armoured {
        painter.rect_filled(span(armoured, all), 0.0, crate::theme::SHIELD);
    }
    if lost > all {
        painter.rect_filled(span(all, lost), 0.0, DAMAGE);
    }
    // A sheen along the top of the fill, as Dota's bars have.
    if lost > 0.0 {
        let top = span(0.0, lost);
        let sheen = egui::Rect::from_min_max(
            top.min,
            egui::pos2(top.max.x, top.min.y + track.height() * 0.35),
        );
        painter.rect_filled(sheen, 0.0, egui::Color32::from_white_alpha(38));
    }
    // The ticks, snapped to whole pixels so a thin one is one sharp
    // pixel and not two grey ones.
    let px = 1.0 / painter.pixels_per_point();
    for (share, big) in ticks(whole, track.width()) {
        let x = track.min.x + track.width() * share;
        let (wide, colour) = if big {
            (BIG_TICK_PX * px, BIG_TICK)
        } else {
            (TICK_PX * px, TICK)
        };
        let left = ((x - wide * 0.5) / px).round() * px;
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(left, track.min.y),
                egui::pos2(left + wide, track.max.y),
            ),
            0.0,
            colour,
        );
    }
}

/// Where the ticks go on a bar of `whole` hit points drawn `width` points
/// wide: each a share of the bar along it, and whether it is a thick one
/// (every [`BIG_TICK_HP`]) or thin (every [`TICK_HP`] between). None at
/// either end, and none of a kind packed closer than [`TICK_ROOM`].
fn ticks(whole: f32, width: f32) -> Vec<(f32, bool)> {
    let mut out = Vec::new();
    if whole <= TICK_HP || width <= 0.0 {
        return out;
    }
    let thin = width * TICK_HP / whole >= TICK_ROOM;
    let thick = width * BIG_TICK_HP / whole >= TICK_ROOM;
    let per_big = (BIG_TICK_HP / TICK_HP).round() as u32;
    let mut n = 1;
    while (n as f32) * TICK_HP < whole - 0.5 {
        let big = n % per_big == 0;
        if (big && thick) || (!big && thin) {
            out.push((n as f32 * TICK_HP / whole, big));
        }
        n += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn a_hit_leaves_white_that_holds_and_then_drains() {
        let mut t = Trail::new(1.0);
        t.step(0.6, 0.016);
        assert_eq!(t.now, 0.6);
        assert_eq!(t.lost, 1.0, "the white runs up to what the bar was");
        assert_eq!(t.kept, 0.6);
        // Still held most of a second later.
        t.step(0.6, 0.8);
        assert!(near(t.lost, 1.0));
        // Then it drains, and never below the bar.
        for _ in 0..200 {
            t.step(0.6, 0.016);
        }
        assert_eq!(t.lost, 0.6);
    }

    #[test]
    fn hits_in_a_row_keep_the_white_from_the_first() {
        let mut t = Trail::new(1.0);
        t.step(0.8, 0.016);
        t.step(0.5, 0.016);
        assert!(near(t.lost, 1.0));
        assert_eq!(t.now, 0.5);
    }

    #[test]
    fn a_steady_bleed_whitens_only_its_last_second() {
        // An eighth of a bar a second for three seconds, from the top.
        let mut t = Trail::new(1.0);
        let mut share = 1.0;
        for _ in 0..(3.0 / 0.016) as usize {
            share -= 0.125 * 0.016;
            t.step(share, 0.016);
        }
        let white = t.lost - t.now;
        assert!(
            (0.1..=0.13).contains(&white),
            "a second's worth of white, not three: {white}"
        );
        for _ in 0..100 {
            t.step(share, 0.016);
        }
        assert_eq!(t.lost, t.now);
    }

    #[test]
    fn a_heal_fills_in_the_white_of_a_hit() {
        let mut t = Trail::new(1.0);
        t.step(0.5, 0.016);
        t.step(0.7, 0.016);
        assert!(near(t.lost, 1.0), "the white's top stays: {}", t.lost);
        assert!(near(t.kept, 0.5), "the heal shows light: {}", t.kept);
    }

    #[test]
    fn a_heal_shows_light_that_the_fill_catches_up_to() {
        let mut t = Trail::new(0.4);
        t.step(0.7, 0.016);
        assert!(near(t.kept, 0.4), "the solid waits where it was");
        assert_eq!(t.now, 0.7);
        assert_eq!(t.lost, 0.7, "no white for a heal");
        for _ in 0..200 {
            t.step(0.7, 0.016);
        }
        assert_eq!(t.kept, 0.7);
    }

    #[test]
    fn a_hit_eats_the_light_first() {
        let mut t = Trail::new(0.4);
        t.step(0.7, 0.016);
        t.step(0.5, 0.016);
        assert!(near(t.kept, 0.4), "the light that is left runs 0.4 to 0.5");
        assert!(near(t.lost, 0.7));
        t.step(0.3, 0.016);
        assert!(near(t.kept, 0.3), "and a hit below it takes the solid");
    }

    #[test]
    fn a_hit_the_armour_takes_whitens_the_blue_and_a_heal_lights_the_health() {
        // Sixty hit points and forty of armour on a bar of a hundred and
        // forty.
        let r = |health: f32, armour: f32| Reading {
            health: health / 140.0,
            armour: armour / 140.0,
            shield: 0.0,
            whole: 140.0,
        };
        let mut t = Trails::new(r(60.0, 40.0));
        // The armour takes a hit of ten: the health is untouched and the
        // white runs from the end of the blue to where the blue was.
        t.step(r(60.0, 30.0), 0.016);
        assert_eq!(t.health.now, 60.0 / 140.0);
        assert_eq!(t.health.kept, 60.0 / 140.0);
        assert_eq!(t.all.now, 90.0 / 140.0);
        assert!(near(t.all.lost, 100.0 / 140.0));
        // A heal of twenty: light on the health, the solid where it was.
        t.step(r(80.0, 30.0), 0.016);
        assert!(near(t.health.kept, 60.0 / 140.0));
        assert_eq!(t.health.now, 80.0 / 140.0);
    }

    #[test]
    fn a_shield_coming_and_draining_is_no_hit_and_no_heal() {
        // A hundred hit points, then a Bastion's six hundred on the end,
        // then the shield half drained, then gone.
        let r = |health: f32, shield: f32| {
            let whole = 100.0 + shield;
            Reading {
                health: health / whole,
                armour: 0.0,
                shield: shield / whole,
                whole,
            }
        };
        let mut t = Trails::new(r(100.0, 0.0));
        for shield in [600.0, 300.0, 0.0] {
            t.step(r(100.0, shield), 0.016);
            assert!(near(t.health.kept, t.health.now), "no light: {shield}");
            assert!(near(t.health.lost, t.health.now), "no white: {shield}");
            assert!(near(t.all.now, 1.0), "{shield}");
        }
        // A hit on the shield is white on its end, as on the armour.
        let mut t = Trails::new(r(100.0, 600.0));
        t.step(r(100.0, 500.0), 0.016);
        assert!(near(t.health.now, 100.0 / 600.0));
        assert!(t.all.lost > t.all.now, "the white of the hit");
        assert!(near(t.shield, 500.0 / 600.0));
    }

    #[test]
    fn a_tick_every_twenty_five_hit_points_and_a_thick_one_every_hundred() {
        // A sixteenth-level player: 260 hit points, ten ticks, the
        // hundredth and the two hundredth thick.
        let t = ticks(260.0, WIDTH);
        assert_eq!(t.len(), 10);
        let thick: Vec<f32> = t.iter().filter(|(_, b)| *b).map(|(s, _)| *s).collect();
        assert_eq!(thick, vec![100.0 / 260.0, 200.0 / 260.0]);
        assert_eq!(t[0], (25.0 / 260.0, false));
        // A plain hundred: three thin ticks, and none at the end.
        assert_eq!(
            ticks(100.0, WIDTH),
            vec![(0.25, false), (0.5, false), (0.75, false)]
        );
        // A bar of a thousand on fifty points keeps only the hundreds.
        let t = ticks(1000.0, WIDTH);
        assert_eq!(t.len(), 9);
        assert!(t.iter().all(|(_, b)| *b));
        // Nothing to cut on a bar of twenty.
        assert!(ticks(20.0, WIDTH).is_empty());
    }

    #[test]
    fn a_steady_mend_lights_only_its_last_second() {
        // An eighth of a bar a second for three seconds, from a fifth.
        let mut t = Trail::new(0.2);
        let mut share = 0.2;
        for _ in 0..(3.0 / 0.016) as usize {
            share += 0.125 * 0.016;
            t.step(share, 0.016);
        }
        let light = t.now - t.kept;
        assert!(
            (0.1..=0.13).contains(&light),
            "a second's worth of light, not three: {light}"
        );
        // And it goes once the mend stops.
        for _ in 0..100 {
            t.step(share, 0.016);
        }
        assert_eq!(t.kept, t.now);
    }

    #[test]
    fn a_heal_holds_its_light_a_second() {
        let mut t = Trail::new(0.4);
        t.step(0.7, 0.016);
        for _ in 0..50 {
            t.step(0.7, 0.016);
        }
        assert!(near(t.kept, 0.4), "still light at 0.8 s");
        for _ in 0..20 {
            t.step(0.7, 0.016);
        }
        assert!(t.kept > 0.4, "filling in after a second: {}", t.kept);
    }

    #[test]
    fn a_paused_bar_holds_still() {
        let mut t = Trail::new(1.0);
        t.step(0.2, 0.016);
        for _ in 0..100 {
            t.step(0.2, 0.0);
        }
        assert!(near(t.lost, 1.0));
        let mut t = Trail::new(0.2);
        t.step(0.6, 0.016);
        for _ in 0..100 {
            t.step(0.6, 0.0);
        }
        assert!(near(t.kept, 0.2), "a heal's light held too");
    }
}
