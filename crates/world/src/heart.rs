//! The Machine Heart (feature 108): the fortress at the crisis's origin,
//! and the one way to **win** a run.
//!
//! # The fortress is a site like any other
//!
//! The origin star's system holds one more station than the generator
//! numbered: the machines' fortress, rolled off the star's own stream the
//! way a derived jammer is ([`crate::jammer`]) — its map seed and the point
//! it stands on — so two clients put the same fortress in the same place
//! without a word. Its id is [`heart_id`], in a range of its own. It is
//! laid out on [`crate::station::Plan::Fortress`] — the hub at the arena's
//! size — and it is **never saved**: `World::settle_heart` strips it and
//! lays it again wherever a system is settled, as `settle_jammer` does the
//! jammer. What is saved is what happened to it, on its
//! [`crate::droid::Infestation`] like any held station's, with the fight's
//! own state beside the waves ([`HeartFight`]). It is always the machines'
//! and always tier three, and it is never the jammer.
//!
//! # The fight is three phases, and the world says which
//!
//! The core, its conduits and its fabricators are machines on the deck —
//! `bims::droid::DroidKind::HEART`, one health each, stood where they were
//! built — so every bolt, blow, grenade, bounty and wreck the fight
//! already has serves them. What the world adds is the phase, read off the
//! room every step (`World::heart_step`):
//!
//! 1. **Sealed** while any conduit stands: the core takes nothing and
//!    fires nothing.
//! 2. **Exposed** once the last conduit is down: the core sweeps one beam
//!    and every fabricator still standing builds a tier-three machine every
//!    [`data::HEART_FABRICATOR_INTERVAL`] of the mission clock.
//! 3. **Overload** under [`data::HEART_OVERLOAD_FRACTION`] of its health:
//!    two beams, [`data::HEART_OVERLOAD_SWEEP_FACTOR`] times faster, and
//!    the fabricators building [`data::HEART_OVERLOAD_SPAWN_FACTOR`] times
//!    as often.
//!
//! The core at nothing is `World::run_won` — the one place a run is won —
//! whoever is dead. Leaving before then puts the whole fortress back
//! (`crate::run::SiteSnapshot`), since the fight's state is on the
//! station's `Infestation`, which the snapshot keeps.

use shipdesign::{Layer, PartKind, ShipDesign};
use worldgen::rng::{Purpose, Rng, seed_for};
use worldgen::{Name, StarSystem, StationBlueprint, StationKind, Stock};

use crate::data;
use crate::jump;

/// The bit that marks a station id as the Machine Heart's fortress. Clear
/// of the generator's ids, of [`crate::jammer::JAMMER_BASE`] and of
/// [`crate::surface::SURFACE_BASE`].
pub const HEART_BASE: u32 = 0x2000_0000;

/// The station id of the fortress in a star's system — only ever the
/// origin's.
pub fn heart_id(star: u32) -> u32 {
    HEART_BASE | star
}

/// Which star's fortress a station id names, if it names one.
pub fn heart_star(id: u32) -> Option<u32> {
    (id & HEART_BASE != 0
        && id & crate::surface::SURFACE_BASE == 0
        && id & crate::jammer::JAMMER_BASE == 0)
        .then_some(id & !HEART_BASE)
}

/// Whether this station id is the Machine Heart's fortress.
pub fn is_heart(id: u32) -> bool {
    heart_star(id).is_some()
}

/// The kind the fortress is built as. An orbital's, for its size and its
/// airlocks; nobody lives there whatever its kind says
/// ([`crate::station::Plan::residents`] is nought for a fortress).
pub const HEART_KIND: StationKind = StationKind::Orbital;

/// The first ring of [`jump::clear_point`] the fortress is looked for on:
/// beyond every ring a derived jammer is rolled on, so the two stand well
/// apart however empty the system.
const HEART_FIRST_RING: u32 = 8;
/// How many rings out from [`HEART_FIRST_RING`] the roll may push it.
const HEART_RINGS: u32 = 4;

/// How many conduits seal the core for a crew of `players` player Bims.
pub fn conduits_for(players: u32) -> u32 {
    data::HEART_CONDUITS_BASE
        .saturating_add(data::HEART_CONDUITS_PER_PLAYER.saturating_mul(players))
}

/// How much health the core has for a crew of `players` player Bims.
pub fn core_health_for(players: u32) -> f32 {
    data::HEART_CORE_HEALTH_BASE + data::HEART_CORE_HEALTH_PER_PLAYER * players as f32
}

/// The fortress in the origin star's system, rolled off the star's own
/// stream. `system` is read **without** any derived jammer or fortress
/// in it, so the answer is the same whether the crew are in the system or
/// quoting a trip to it from next door.
pub fn blueprint(system: &StarSystem, galaxy_seed: u64, star: u32) -> StationBlueprint {
    let mut bare = system.clone();
    bare.stations
        .retain(|s| !crate::jammer::is_derived(s.id) && !is_heart(s.id));
    let base = seed_for(
        galaxy_seed,
        star,
        worldgen::GENERATOR_VERSION,
        Purpose::Jammer,
    );
    // Off the jammer's stream but on a branch of its own ("HEART"), so
    // nothing about the jammer moves because the fortress is there.
    let stream = Rng::new(base).branch(0x_4845_4152_5400_0000);
    let map_seed = stream.branch(0x_4d41_5000_0000_0000).next_u64();
    let stock = Stock::roll(HEART_KIND, &mut stream.branch(0x_5354_4f43_4b00_0000));
    let bias = worldgen::data::price_bias(&mut stream.branch(0x_4249_4153_0000_0000));
    let mut place = stream.branch(0x_504c_4143_4500_0000);
    let ring = HEART_FIRST_RING + place.below(HEART_RINGS);
    // A quarter-step off the search's bearings, where a jammer takes a
    // half-step: the two never try the same point.
    let turn = (place.below(16) as f64 + 0.25) * core::f64::consts::TAU / 256.0;
    let position = jump::clear_point(&bare, ring, turn);
    let name = Name::station(
        (map_seed % worldgen::name::STATION_WORDS as u64) as u16,
        ((map_seed >> 16) % 1000) as u16,
        0,
    );
    StationBlueprint {
        id: heart_id(star),
        kind: HEART_KIND,
        parent_body: None,
        position,
        name,
        salvage_sites: 0,
        hazard_sites: Vec::new(),
        map_seed,
        stock,
        bias,
        hostile: true,
    }
}

/// Where the fight is. The codes are what the checksum eats, written out.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HeartPhase {
    /// A conduit still stands: the core takes nothing and fires nothing.
    #[default]
    Sealed = 1,
    /// The conduits are down: one beam, and the fabricators building.
    Exposed = 2,
    /// Under the overload fraction: two beams, faster, and faster building.
    Overload = 3,
    /// The core is destroyed, and the run won.
    Destroyed = 4,
}

impl HeartPhase {
    pub fn code(self) -> u8 {
        self as u8
    }

    /// The phase with a number, for the probes' `BIMS_HEART_PHASE`: 1 to 3.
    pub fn from_number(n: u32) -> Option<HeartPhase> {
        match n {
            1 => Some(HeartPhase::Sealed),
            2 => Some(HeartPhase::Exposed),
            3 => Some(HeartPhase::Overload),
            _ => None,
        }
    }
}

/// The Machine Heart's fight as the world keeps it, on the fortress's
/// [`crate::droid::Infestation`] beside its waves. **Saved and in
/// `world_checksum`**, and put back with the rest of the station when the
/// crew leave before the core is down.
///
/// How much health each machine has left is the room's — they are bodies
/// on the deck, carried from room to room like any machine — and this is
/// what the room cannot say: which phase the fight is in, how many were
/// built, and when the fabricators build next.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HeartFight {
    pub phase: HeartPhase,
    /// How many conduits were built, and how much health the core was
    /// built with: fixed when the crew first dock, off how many players
    /// there are then.
    pub conduits: u32,
    pub core_health: f32,
    /// Whether the core, the conduits and the fabricators have been put on
    /// the deck. A room built afresh without them — the first dock, or a
    /// load — has them laid again from here and nothing else.
    pub laid: bool,
    /// The step of the mission clock the fabricators build at next, once
    /// the core is exposed.
    pub next_build: Option<u64>,
    /// How many machines the fabricators have built: what each one's
    /// seed, kind and place are read off, so every client builds alike.
    pub built: u32,
}

impl HeartFight {
    /// A fortress's fight for `players` player Bims, nothing laid yet.
    pub fn new(players: u32) -> HeartFight {
        HeartFight {
            phase: HeartPhase::Sealed,
            conduits: conduits_for(players),
            core_health: core_health_for(players),
            laid: false,
            next_build: None,
            built: 0,
        }
    }
}

/// The fortress's strength on arrival (feature 108): what the map shows
/// under it, off the arrival day — the same numbers the fight is built
/// with, so waiting is seen to make it harder.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct HeartPreview {
    pub conduits: u32,
    pub core_health: f32,
    pub wave_size: u32,
    pub wave_count: u32,
}

/// Where the Machine Heart's machines stand in its fortress, in the
/// design's tiles: the core, the fabricators round it, and a conduit a
/// room — the rooms and the lobbies in [`crate::station::fortress_rooms`]'
/// order, so no two conduits share a room until every room has one.
#[derive(Clone, PartialEq, Debug)]
pub struct Places {
    pub core: (u32, u32),
    pub fabricators: Vec<(u32, u32)>,
    pub conduits: Vec<(u32, u32)>,
}

/// The tiles for `conduits` conduits and `fabricators` fabricators in the
/// fortress `design`. A spot is the free deck tile nearest the middle of
/// its room (nothing standing on it), so a conduit never stands in a
/// bunk or on a table. More conduits than rooms go round again, each a
/// further tile from the middle than the last one in that room.
pub fn places(design: &ShipDesign, conduits: u32, fabricators: u32) -> Places {
    let side = design.build_area;
    let (hub, rooms) = crate::station::fortress_rooms(side);
    let grid = design.grid();
    let floor: Vec<(u32, u32)> = design
        .parts
        .iter()
        .filter(|p| p.kind == PartKind::Floor)
        .map(|p| (p.origin.0, p.origin.1))
        .collect();
    let free = |tile: (u32, u32), taken: &[(u32, u32)]| {
        floor.contains(&tile)
            && grid.get(Layer::Object, (tile.0 as i32, tile.1 as i32)) == 0
            && !taken.contains(&tile)
    };
    // The nearest free tile to `(cx, cy)` inside `x0..=x1, y0..=y1`, ties
    // to the lower row and then the lower column so every build agrees.
    let nearest = |(x0, y0, x1, y1): (u32, u32, u32, u32),
                   (cx, cy): (u32, u32),
                   taken: &[(u32, u32)]|
     -> Option<(u32, u32)> {
        let mut best: Option<(u32, (u32, u32))> = None;
        for y in y0..=y1 {
            for x in x0..=x1 {
                if !free((x, y), taken) {
                    continue;
                }
                let d = x.abs_diff(cx).pow(2) + y.abs_diff(cy).pow(2);
                if best.is_none_or(|(b, _)| d < b) {
                    best = Some((d, (x, y)));
                }
            }
        }
        best.map(|(_, t)| t)
    };
    let mid = ((hub.0 + hub.2) / 2, (hub.1 + hub.3) / 2);
    // The core on the free deck nearest the middle of the hub, which is the
    // middle itself: a fortress has no big plant there
    // (`crate::station::Plan::Fortress`).
    let inner = (hub.0 + 1, hub.1 + 1, hub.2 - 1, hub.3 - 1);
    let core = nearest(inner, mid, &[]).unwrap_or(mid);
    let mut taken = vec![core];
    let corners = [(-3i32, -3i32), (3, 3), (3, -3), (-3, 3)];
    let fabricators: Vec<(u32, u32)> = (0..fabricators as usize)
        .filter_map(|i| {
            let (dx, dy) = corners[i % corners.len()];
            let want = (
                (mid.0 as i32 + dx).max(0) as u32,
                (mid.1 as i32 + dy).max(0) as u32,
            );
            let at = nearest(inner, want, &taken)?;
            taken.push(at);
            Some(at)
        })
        .collect();
    let conduits: Vec<(u32, u32)> = (0..conduits as usize)
        .filter_map(|i| {
            let room = *rooms.get(i % rooms.len().max(1))?;
            let inside = (room.0 + 1, room.1 + 1, room.2 - 1, room.3 - 1);
            let centre = ((room.0 + room.2) / 2, (room.1 + room.3) / 2);
            let at = nearest(inside, centre, &taken)?;
            taken.push(at);
            Some(at)
        })
        .collect();
    Places {
        core,
        fabricators,
        conduits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use worldgen::{Galaxy, GalaxyType};

    /// The fortress's id is its own: never a generated station's, a
    /// surface's or a derived jammer's.
    #[test]
    fn the_fortress_s_id_collides_with_nothing() {
        for star in [0u32, 1, 7, 512, 999] {
            let id = heart_id(star);
            assert_eq!(heart_star(id), Some(star));
            assert!(is_heart(id));
            assert!(!crate::jammer::is_derived(id));
            assert!(crate::surface::surface_body(id).is_none());
            assert!(!is_heart(star));
            assert!(!is_heart(crate::jammer::jammer_id(star)));
            assert!(!is_heart(crate::surface::surface_id(star)));
        }
    }

    /// It is the same fortress every time, and it stands clear of every
    /// node of its system and of where a jump lands.
    #[test]
    fn the_fortress_stands_clear_and_is_the_same_every_time() {
        let galaxy = Galaxy::new(0x1234_5678, GalaxyType::Round);
        for star in [0u32, 17, 400, 999] {
            let system = galaxy.system(star).unwrap();
            let one = blueprint(&system, galaxy.seed, star);
            let two = blueprint(&system, galaxy.seed, star);
            assert_eq!(one, two);
            assert_eq!(one.id, heart_id(star));
            for node in system.nodes() {
                let there = system.absolute_position(node).unwrap();
                assert!(there.distance(one.position) >= data::JUMP_CLEARANCE);
            }
            assert_ne!(one.position, jump::landing_point(&system));
            // And the derived jammer, where there is one, is elsewhere.
            let jammer = crate::jammer::blueprint(&system, galaxy.seed, star);
            assert!(jammer.position.distance(one.position) >= data::JUMP_CLEARANCE);
        }
    }

    /// The count scales on the players and nothing else.
    #[test]
    fn the_conduits_and_the_core_scale_on_the_players() {
        assert_eq!(conduits_for(0), data::HEART_CONDUITS_BASE);
        assert_eq!(
            conduits_for(3),
            data::HEART_CONDUITS_BASE + 3 * data::HEART_CONDUITS_PER_PLAYER
        );
        assert_eq!(
            core_health_for(2),
            data::HEART_CORE_HEALTH_BASE + 2.0 * data::HEART_CORE_HEALTH_PER_PLAYER
        );
    }
}
