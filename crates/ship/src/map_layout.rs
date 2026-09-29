//! Which system the map shows and where each thing of it is drawn (the map
//! rework): the ship's own, or any star picked on the galaxy chart or off the
//! world map's list; fitted so every site of it is in view; and the sites
//! spread apart on the screen so no two icons lie on one another.
//!
//! A child of `crate::game`, so it keeps the map's own state on `Game`.
//! Nothing here decides anything: what a site is, is the world's
//! (`World::system_look`), and where it is drawn is the picture's.

use world::{Site, SiteKind, SystemLook};
use worldgen::math::{DVec2, dvec2};
use worldgen::{Node, StarSystem};

use super::{Game, MAP_FIT, turned};

/// How far apart, in screen pixels, the middles of two sites on the map
/// are held at the least: a planet's ring is thirty-four across with its
/// pad at the shoulder, an elite station's crown and outer ring about as
/// much, and the word under each is some sixty wide — at seventy-two the
/// rings, the marks and the words of two sites never touch.
pub const MAP_SEPARATION: f32 = 72.0;

/// How many times the icons are pushed apart. A handful of sites a
/// system; they settle long before this.
const SPREAD_PASSES: usize = 8;

/// A system other than the ship's own, looked at on the map, and what it
/// was worked out against — the star the ship is in and the world clock —
/// so it is looked at again once either moves.
pub(super) struct Shown {
    star: u32,
    key: (u32, u64),
    look: SystemLook,
}

/// A site of the system the map shows, and what the map says of it: the
/// ring round it and the word under it.
#[derive(Clone, Copy, Debug)]
pub struct MapSite {
    pub site: Site,
    /// The icon it is drawn as: a station, or the planet its town is on.
    pub node: Node,
    pub kind: SiteKind,
    /// A defence starts on arrival.
    pub threatened: bool,
    /// The fight there is over and won.
    pub cleared: bool,
    /// A trader shut while its system is the machines'.
    pub closed: bool,
    /// The fight the crew passed over (task 135): this system's other one
    /// was fought.
    pub passed: bool,
    /// The system's elite.
    pub elite: bool,
}

impl Game {
    /// Show `star`'s system on the map, or the ship's own with `None` —
    /// once a frame from the app, so it is cheap while nothing changed.
    /// Another system is looked at through the world once, and again
    /// only when the ship has jumped or the clock has moved; a change of
    /// system fits the map to it afresh.
    pub fn show_system(&mut self, star: Option<u32>) {
        let own = self.world.star_id;
        let key = (own, self.world.clock_minutes.to_bits());
        let want = star.filter(|&s| s != own);
        let before = self.shown_star();
        match want {
            None => self.shown = None,
            Some(star) => {
                let fresh = self
                    .shown
                    .as_ref()
                    .is_some_and(|s| s.star == star && s.key == key);
                if !fresh {
                    self.shown = self
                        .world
                        .system_look(star)
                        .map(|look| Shown { star, key, look });
                }
            }
        }
        if self.shown_star() != before || self.shown_own != Some(own) {
            self.shown_own = Some(own);
            self.fit_map();
        }
    }

    /// The star whose system the map is showing.
    pub fn shown_star(&self) -> u32 {
        self.shown
            .as_ref()
            .map(|s| s.star)
            .unwrap_or(self.world.star_id)
    }

    /// Whether the map shows the system the ship is in — the only one it
    /// draws the ship, the sensors' reach and the ticks of where the crew
    /// have been in.
    pub fn shows_own_system(&self) -> bool {
        self.shown.is_none()
    }

    /// The system the map draws.
    pub fn map_system(&self) -> &StarSystem {
        match &self.shown {
            Some(s) => &s.look.system,
            None => &self.world.system,
        }
    }

    /// What the map's camera is measured from: the ship in its own system,
    /// the star in any other.
    pub(super) fn map_origin(&self) -> DVec2 {
        match &self.shown {
            Some(_) => DVec2::ZERO,
            None => self.world.ship.position(),
        }
    }

    /// Every node the map draws, bodies first and stations after: in the
    /// ship's own system what the crew have found; in another, every body
    /// and every site — a derived jammer or the fortress a station of its
    /// own, though the generated system has no such station.
    pub fn map_nodes(&self) -> Vec<Node> {
        let Some(shown) = &self.shown else {
            let mut nodes: Vec<Node> = self
                .world
                .discovered
                .iter()
                .filter(|n| matches!(n, Node::Body(_)))
                .copied()
                .collect();
            nodes.extend(
                self.world
                    .discovered
                    .iter()
                    .filter(|n| matches!(n, Node::Station(_))),
            );
            return nodes;
        };
        let mut nodes: Vec<Node> = shown
            .look
            .system
            .bodies
            .iter()
            .map(|b| Node::Body(b.id))
            .collect();
        nodes.extend(
            shown
                .look
                .sites
                .iter()
                .filter(|s| world::surface_body(s.site.station).is_none())
                .map(|s| Node::Station(s.site.station)),
        );
        nodes
    }

    /// Where a node of the system shown lies in it.
    pub fn map_position(&self, node: Node) -> Option<DVec2> {
        let Some(shown) = &self.shown else {
            return self.world.system.absolute_position(node);
        };
        if let Node::Station(id) = node
            && let Some(site) = shown.look.sites.iter().find(|s| s.site.station == id)
        {
            return Some(site.at);
        }
        shown.look.system.absolute_position(node)
    }

    /// Every site of the system shown, with what the map says of it. The
    /// ship's own system is read off the world as it stands; another off
    /// its look, which is what it would be on arrival.
    pub fn map_sites(&self) -> Vec<MapSite> {
        let world = &self.world;
        let node_of = |station: u32| match world::surface_body(station) {
            Some(body) => Node::Body(body),
            None => Node::Station(station),
        };
        let Some(shown) = &self.shown else {
            let closed = world.trader_closed_on(world.star_id, world.days_gone());
            return world
                .discovered
                .iter()
                .filter_map(|&node| {
                    let id = match node {
                        Node::Station(id) => id,
                        Node::Body(body) => world.surface(body)?.id,
                    };
                    let kind = world.site_kind(id);
                    Some(MapSite {
                        site: Site {
                            star: world.star_id,
                            station: id,
                        },
                        node,
                        kind,
                        threatened: world.site_threatened(id),
                        cleared: world.site_cleared(id) && !world.site_threatened(id),
                        closed: kind == SiteKind::Trader && closed,
                        passed: world.passed_over(id),
                        elite: world.is_elite_here(id),
                    })
                })
                .collect();
        };
        let closed = world.trader_closed_on(shown.star, world.days_gone());
        shown
            .look
            .sites
            .iter()
            .map(|s| MapSite {
                site: s.site,
                node: node_of(s.site.station),
                kind: s.quote.kind,
                threatened: s.quote.threatened,
                cleared: s.quote.cleared,
                closed: s.quote.trader && closed,
                passed: false,
                elite: s.quote.elite,
            })
            .collect()
    }

    /// Where the map would put a place of the system shown, in the
    /// camera's units about its origin — before any icon is moved aside.
    fn raw_spot(&self, at: DVec2) -> (f32, f32) {
        let offset = at.sub(self.map_origin());
        turned(offset.x as f32, -offset.y as f32, self.camera_turn() as f32)
    }

    /// Where the map draws every node, in the camera's units about its
    /// origin: where it lies, and then every **site** pushed clear of the
    /// star and of every site laid before it until the two are
    /// [`MAP_SEPARATION`] pixels apart at the camera's scale — the site
    /// the crew are at first and never moved, so the ship's reticle stays
    /// on it; then the towns' planets; then the stations. So a station in
    /// orbit of the planet whose town is the other site stands beside it
    /// rather than on it. A body with nothing on it is drawn where it lies,
    /// and a station is drawn over its planet as it always was.
    pub fn map_spots(&self) -> Vec<(Node, (f32, f32))> {
        let sites = self.map_sites();
        let here = self
            .shows_own_system()
            .then(|| self.world.current_site())
            .flatten();
        let rank = |node: &Node| match sites.iter().find(|s| s.node == *node) {
            None => 0,
            Some(s) if Some(s.site) == here => 1,
            Some(_) if matches!(node, Node::Body(_)) => 2,
            Some(_) => 3,
        };
        let star = self.raw_spot(DVec2::ZERO);
        let mut fixed: Vec<(f32, f32)> = vec![star];
        let nodes = self.map_nodes();
        let mut ordered: Vec<Node> = nodes.clone();
        ordered.sort_by_key(|n| rank(n));
        let scale = self.map_view.scale().max(1e-30);
        let apart = MAP_SEPARATION / scale;
        let mut spots: Vec<(Node, (f32, f32))> = Vec::new();
        for node in ordered {
            let Some(at) = self.map_position(node) else {
                continue;
            };
            let mut spot = self.raw_spot(at);
            match rank(&node) {
                0 => {}
                1 => fixed.push(spot),
                _ => {
                    spot = spread(spot, &fixed, apart, star);
                    fixed.push(spot);
                }
            }
            spots.push((node, spot));
        }
        // Back in the map's own order.
        spots.sort_by_key(|(n, _)| nodes.iter().position(|m| m == n));
        spots
    }

    /// Fit the map to the system shown: every site of it, the star, and
    /// in the ship's own system the ship, within [`MAP_FIT`] of the
    /// canvas, the middle of them in the middle.
    pub(super) fn fit_map(&mut self) {
        let mut points: Vec<(f32, f32)> = vec![self.raw_spot(DVec2::ZERO)];
        if self.shows_own_system() {
            points.push(self.raw_spot(self.world.ship.position()));
        }
        for site in self.map_sites() {
            if let Some(at) = self.map_position(site.node) {
                points.push(self.raw_spot(at));
            }
        }
        // Nothing but the star and the ship: the whole of what was found.
        if points.len() <= 2 {
            for node in self.map_nodes() {
                if let Some(at) = self.map_position(node) {
                    points.push(self.raw_spot(at));
                }
            }
        }
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for (x, y) in &points {
            x0 = x0.min(*x);
            y0 = y0.min(*y);
            x1 = x1.max(*x);
            y1 = y1.max(*y);
        }
        let (w, h) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
        // A margin of an icon and its words on every side, in pixels.
        let pad = MAP_SEPARATION;
        let room_w = (self.map_view.width * MAP_FIT - pad).max(1.0);
        let room_h = (self.map_view.height * MAP_FIT - pad).max(1.0);
        let scale = (room_w / w).min(room_h / h);
        self.map_view.set_scale(scale);
        // The camera's scale may have been clamped; the middle is the
        // middle either way.
        let middle = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        self.map_anchor = self.map_point_of(middle);
        let (fx, fy) = middle;
        self.map_view.set_focus(fx, fy);
    }

    /// A spot in the camera's units back into a place in the system
    /// shown: the turn undone, then the y-flip, then the origin added.
    pub(super) fn map_point_of(&self, spot: (f32, f32)) -> DVec2 {
        let (vx, vy) = turned(spot.0, spot.1, -self.camera_turn() as f32);
        self.map_origin().add(dvec2(vx as f64, -(vy as f64)))
    }
}

/// `spot` pushed away from every one of `fixed` nearer than `apart`:
/// straight away from the nearest each pass — away from `centre` (the
/// star) where the two lie on one point — until nothing is too near or the
/// passes run out.
fn spread(spot: (f32, f32), fixed: &[(f32, f32)], apart: f32, centre: (f32, f32)) -> (f32, f32) {
    let mut spot = spot;
    for _ in 0..SPREAD_PASSES {
        let near = fixed
            .iter()
            .map(|f| (f, (spot.0 - f.0).hypot(spot.1 - f.1)))
            .filter(|(_, d)| *d < apart * 0.999)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((from, d)) = near else {
            break;
        };
        let (mut dx, mut dy) = (spot.0 - from.0, spot.1 - from.1);
        if d < apart * 1e-3 {
            // On top of it: out from the star, or up where it is the star.
            (dx, dy) = (from.0 - centre.0, from.1 - centre.1);
            if dx.hypot(dy) < apart * 1e-3 {
                (dx, dy) = (0.0, -1.0);
            }
        }
        let len = dx.hypot(dy).max(1e-30);
        spot = (from.0 + dx / len * apart, from.1 + dy / len * apart);
    }
    spot
}

#[cfg(test)]
mod tests {
    use super::spread;

    #[test]
    fn a_spot_on_another_is_pushed_out_from_the_star() {
        let got = spread((10.0, 0.0), &[(0.0, 0.0), (10.0, 0.0)], 4.0, (0.0, 0.0));
        assert!((got.0 - 14.0).abs() < 1e-4 && got.1.abs() < 1e-4, "{got:?}");
    }

    #[test]
    fn a_spot_clear_of_everything_stays() {
        let got = spread((10.0, 10.0), &[(0.0, 0.0)], 4.0, (0.0, 0.0));
        assert_eq!(got, (10.0, 10.0));
    }

    #[test]
    fn a_spot_ends_apart_from_every_fixed_one() {
        let fixed = [(0.0, 0.0), (3.0, 0.0), (0.0, 3.0), (3.0, 3.0)];
        let got = spread((1.5, 1.5), &fixed, 2.0, (0.0, 0.0));
        for f in fixed {
            assert!((got.0 - f.0).hypot(got.1 - f.1) >= 2.0 * 0.999, "{got:?}");
        }
    }
}
