//! The health bar over every body on the deck (task 137), the way Dota
//! draws one: a short bar over the head, green for a friend and red for
//! an enemy, the crew's and the station's people's and the machines'
//! alike. What a body just lost stays on the bar in white for a moment
//! and then drains away, and what it just got back shows in a lighter
//! tone of its colour before the bar fills in behind it — so a glance
//! across a fight says who is being hit and who is being mended.
//!
//! # A picture, kept by the screen
//!
//! Nothing here is the world's. A bar is read off the room every frame —
//! a Bim's hit points out of [`bims::health::MAX_HEALTH`] with its worn
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

/// How long the white of a hit, or the light of a heal, stands still
/// before it starts to go, in real seconds.
const HOLD: f32 = 0.45;
/// How fast it goes once it does, in bars a second: a whole bar lost
/// drains in a little over half a second.
const DRAIN: f32 = 1.6;

/// The bar at 100% zoom, in points, and the least and most it is drawn
/// at as the camera zooms: readable far out, not a banner close in.
const WIDTH: f32 = 50.0;
const WIDTH_RANGE: (f32, f32) = (40.0, 70.0);
const HEIGHT: f32 = 7.0;
const HEIGHT_RANGE: (f32, f32) = (6.0, 10.0);

const ALLY: egui::Color32 = egui::Color32::from_rgb(0x3f, 0xc2, 0x4a);
const ALLY_HEALED: egui::Color32 = egui::Color32::from_rgb(0xa8, 0xf0, 0xae);
const ENEMY: egui::Color32 = egui::Color32::from_rgb(0xd9, 0x33, 0x2b);
const ENEMY_HEALED: egui::Color32 = egui::Color32::from_rgb(0xff, 0xa3, 0x9c);
const DAMAGE: egui::Color32 = egui::Color32::from_rgb(0xf4, 0xf4, 0xf0);
const TRACK: egui::Color32 = egui::Color32::from_rgb(0x14, 0x17, 0x16);
/// The thin ticks cutting the bar into quarters, as Dota's cut a bar into
/// chunks so how much is left can be counted at a glance.
const TICK: egui::Color32 = egui::Color32::from_black_alpha(90);
const CHUNKS: u32 = 4;

/// Which room a body is of: the crew's, or the station's people's.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Room {
    Crew,
    Residents(u32),
}

/// One body's bar as this window remembers it, every share nought to one.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Trail {
    /// The bar as it stands.
    now: f32,
    /// The top of the white: what the bar was before the hits of the last
    /// moment. Never below `now`.
    lost: f32,
    /// The top of the solid fill: what the bar was before the heals of
    /// the last moment, the light part running from here to `now`. Never
    /// above it.
    kept: f32,
    lost_hold: f32,
    kept_hold: f32,
}

impl Trail {
    fn new(share: f32) -> Trail {
        Trail {
            now: share,
            lost: share,
            kept: share,
            lost_hold: 0.0,
            kept_hold: 0.0,
        }
    }

    /// The bar reads `share` now, `dt` real seconds after it last did.
    fn step(&mut self, share: f32, dt: f32) {
        if share < self.now {
            // A hit: the white runs from here up to the highest the bar
            // has been lately, and a heal still showing light is the
            // first thing it eats into.
            self.lost = self.lost.max(self.now);
            self.kept = self.kept.min(share);
            self.lost_hold = HOLD;
        } else if share > self.now {
            // A heal: the solid stays where it was and the light fills
            // the gap; white above it is left to drain on its own.
            self.kept = self.kept.min(self.now);
            self.kept_hold = HOLD;
        }
        self.now = share;
        if self.lost_hold > 0.0 {
            self.lost_hold -= dt;
        } else {
            self.lost -= DRAIN * dt;
        }
        if self.kept_hold > 0.0 {
            self.kept_hold -= dt;
        } else {
            self.kept += DRAIN * dt;
        }
        self.lost = self.lost.max(share);
        self.kept = self.kept.min(share);
    }
}

/// What a body has, each a share of its whole bar: its hit points, and
/// the armour it wears on the end of them.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Reading {
    health: f32,
    armour: f32,
}

/// A body's two trails: its health alone, for the light of a heal —
/// only hit points are ever put back — and its health and armour
/// together, for the white of a hit, which takes the armour first.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Trails {
    health: Trail,
    all: Trail,
}

impl Trails {
    fn new(r: Reading) -> Trails {
        Trails {
            health: Trail::new(r.health),
            all: Trail::new(r.health + r.armour),
        }
    }

    fn step(&mut self, r: Reading, dt: f32) {
        self.health.step(r.health, dt);
        self.all.step(r.health + r.armour, dt);
    }
}

/// One bar to draw: where the body is in the camera's units, whose side
/// it is on, and its trails.
struct Bar {
    at: (f32, f32),
    friendly: bool,
    trails: Trails,
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
            let whole = bims::health::MAX_HEALTH + armour;
            Some(Reading {
                health: (room.health(who) / whole).clamp(0.0, 1.0),
                armour: armour / whole,
            })
        }
        Some(i) => Some(Reading {
            health: room.droid(i)?.body.life_share(),
            armour: 0.0,
        }),
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
        let mut take = |key: (Room, u32), r: Reading, at: (f32, f32), friendly: bool| {
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
                    );
                }
            }
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
            let head = to_screen(bar.at);
            let middle = egui::pos2(head.x, head.y - crate::theme::NAME_LIFT * scale - h * 0.5);
            let track = egui::Rect::from_center_size(middle, egui::vec2(w, h));
            let (solid, healed) = if bar.friendly {
                (ALLY, ALLY_HEALED)
            } else {
                (ENEMY, ENEMY_HEALED)
            };
            paint_bar(painter, track, bar.trails, solid, healed);
        }
    }
}

/// One bar in `track`: the dark track with an outline, the solid fill up
/// to the hit points the body kept, the light up to the ones it has now,
/// the armour's blue after them, and the white up to what it had before
/// its last hits.
fn paint_bar(
    painter: &egui::Painter,
    track: egui::Rect,
    trails: Trails,
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
    let (all, lost) = (trails.all.now, trails.all.lost);
    if kept > 0.0 {
        painter.rect_filled(span(0.0, kept), 1.0, solid);
    }
    if health > kept {
        painter.rect_filled(span(kept, health), 0.0, healed);
    }
    if all > health {
        painter.rect_filled(span(health, all), 0.0, crate::theme::ARMOUR);
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
    for i in 1..CHUNKS {
        let x = track.min.x + track.width() * i as f32 / CHUNKS as f32;
        painter.line_segment(
            [egui::pos2(x, track.min.y), egui::pos2(x, track.max.y)],
            egui::Stroke::new(1.0, TICK),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hit_leaves_white_that_holds_and_then_drains() {
        let mut t = Trail::new(1.0);
        t.step(0.6, 0.016);
        assert_eq!(t.now, 0.6);
        assert_eq!(t.lost, 1.0, "the white runs up to what the bar was");
        assert_eq!(t.kept, 0.6);
        // Still held a moment later.
        t.step(0.6, HOLD * 0.5);
        assert_eq!(t.lost, 1.0);
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
        assert_eq!(t.lost, 1.0);
        assert_eq!(t.now, 0.5);
    }

    #[test]
    fn a_heal_shows_light_that_the_fill_catches_up_to() {
        let mut t = Trail::new(0.4);
        t.step(0.7, 0.016);
        assert_eq!(t.kept, 0.4, "the solid waits where it was");
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
        assert_eq!(t.kept, 0.4, "the light that is left runs 0.4 to 0.5");
        assert_eq!(t.lost, 0.7);
        t.step(0.3, 0.016);
        assert_eq!(t.kept, 0.3, "and a hit below it takes the solid");
    }

    #[test]
    fn a_hit_the_armour_takes_whitens_the_blue_and_a_heal_lights_the_health() {
        // Sixty hit points and forty of armour on a bar of a hundred and
        // forty.
        let r = |health: f32, armour: f32| Reading {
            health: health / 140.0,
            armour: armour / 140.0,
        };
        let mut t = Trails::new(r(60.0, 40.0));
        // The armour takes a hit of ten: the health is untouched and the
        // white runs from the end of the blue to where the blue was.
        t.step(r(60.0, 30.0), 0.016);
        assert_eq!(t.health.now, 60.0 / 140.0);
        assert_eq!(t.health.kept, 60.0 / 140.0);
        assert_eq!(t.all.now, 90.0 / 140.0);
        assert_eq!(t.all.lost, 100.0 / 140.0);
        // A heal of twenty: light on the health, the solid where it was.
        t.step(r(80.0, 30.0), 0.016);
        assert_eq!(t.health.kept, 60.0 / 140.0);
        assert_eq!(t.health.now, 80.0 / 140.0);
    }

    #[test]
    fn a_paused_bar_holds_still() {
        let mut t = Trail::new(1.0);
        t.step(0.2, 0.016);
        for _ in 0..100 {
            t.step(0.2, 0.0);
        }
        assert_eq!(t.lost, 1.0);
    }
}
