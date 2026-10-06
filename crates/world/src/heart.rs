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
//! laid out on [`crate::station::Plan::Fortress`] — one hall at the
//! arena's size, the core in its middle — and it is **never saved**: `World::settle_heart` strips it and
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
//!    fires nothing, and every conduit shot down brings a wave of the
//!    machines and its Guardians in by the airlocks, whatever is still
//!    standing — and seals the other conduits until that wave is down.
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

/// How many Guardians the `link`-th conduit shot down sends in (October
/// 2026), counted from one: one for the first, two for the second, and so
/// on to the last ([`data::HEART_GUARDIANS_PER_LINK`] each step).
pub fn guardians_for_link(link: u32) -> u32 {
    data::HEART_GUARDIANS_PER_LINK.saturating_mul(link)
}

/// Every Guardian a fortress of `conduits` sends, all of them shot down.
pub fn guardians_for(conduits: u32) -> u32 {
    (1..=conduits).map(guardians_for_link).sum()
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
    /// How many conduits have been shot down and answered
    /// (`World::heart_step`): every conduit down past this brings a wave
    /// and its Guardians in by the airlocks ([`guardians_for_link`]), on
    /// top of whatever stands.
    #[cfg_attr(feature = "serde", serde(default))]
    pub links_down: u32,
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
            links_down: 0,
        }
    }
}

/// The fortress's strength on arrival (feature 108): what the map shows
/// under it — the same numbers the fight is built with: the players
/// decide them, the day does not.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct HeartPreview {
    pub conduits: u32,
    pub core_health: f32,
    /// Every Guardian its conduits shot down send ([`guardians_for`]).
    pub guardians: u32,
    /// The machines in the wave each conduit shot down sends, its
    /// Guardians aside (October 2026): the wave of the arrival day and
    /// the tier-three area's Bombers and Lancers on top.
    pub wave: u32,
}

/// Where the Machine Heart's machines stand in its fortress, in the
/// design's tiles: the core in the middle of the hall, the fabricators
/// round it, and the conduits on a ring round them.
#[derive(Clone, PartialEq, Debug)]
pub struct Places {
    pub core: (u32, u32),
    pub fabricators: Vec<(u32, u32)>,
    pub conduits: Vec<(u32, u32)>,
}

/// The tiles for `conduits` conduits and `fabricators` fabricators in the
/// fortress `design` (one hall since October 2026): the core on the free
/// deck tile nearest the middle, the fabricators three tiles off it on a
/// diagonal, and the conduits spread evenly over [`CONDUIT_RING`]'s twelve
/// places — a spot is the free deck tile nearest it, nothing standing on
/// it, so a conduit never stands on a lamp or a sandbag.
pub fn places(design: &ShipDesign, conduits: u32, fabricators: u32) -> Places {
    let side = design.build_area;
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
    // The nearest free tile to `(cx, cy)` inside the hall's deck, ties to
    // the lower row and then the lower column so every build agrees.
    let hall = (2, 2, side.saturating_sub(3), side.saturating_sub(3));
    let nearest = |(cx, cy): (u32, u32), taken: &[(u32, u32)]| -> Option<(u32, u32)> {
        let (x0, y0, x1, y1) = hall;
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
    let mid = (side / 2, side / 2);
    let off = |(dx, dy): (i32, i32)| {
        (
            (mid.0 as i32 + dx).max(0) as u32,
            (mid.1 as i32 + dy).max(0) as u32,
        )
    };
    // The core in the middle of the hall.
    let core = nearest(mid, &[]).unwrap_or(mid);
    let mut taken = vec![core];
    let corners = [(-3i32, -3i32), (3, 3), (3, -3), (-3, 3)];
    let fabricators: Vec<(u32, u32)> = (0..fabricators as usize)
        .filter_map(|i| {
            let at = nearest(off(corners[i % corners.len()]), &taken)?;
            taken.push(at);
            Some(at)
        })
        .collect();
    // The conduits on a ring round it, spread evenly over its places.
    let n = conduits.max(1) as usize;
    let conduits: Vec<(u32, u32)> = (0..conduits as usize)
        .filter_map(|i| {
            let spot = CONDUIT_RING[(i * CONDUIT_RING.len() / n) % CONDUIT_RING.len()];
            let at = nearest(off(spot), &taken)?;
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

/// Where a conduit may stand, in tiles from the core: a ring a little under
/// twenty tiles out, a place every thirty degrees from fifteen — off the
/// airlocks' four ways in — written out so no build works out a sine.
const CONDUIT_RING: [(i32, i32); 12] = [
    (19, 5),
    (14, 14),
    (5, 19),
    (-5, 19),
    (-14, 14),
    (-19, 5),
    (-19, -5),
    (-14, -14),
    (-5, -19),
    (5, -19),
    (14, -14),
    (19, -5),
];

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

    /// The fortress is one hall (October 2026): no partition and no door
    /// in it, the core in its middle, the fabricators beside it and every
    /// conduit on the ring round it, none two places apart standing within
    /// eight tiles of another.
    #[test]
    fn the_fortress_is_one_hall_with_the_core_in_its_middle() {
        use crate::station::{Plan, layout};
        let design = layout(HEART_KIND, Plan::Fortress, 7);
        assert!(
            design
                .parts
                .iter()
                .all(|p| !matches!(p.kind, PartKind::Wall | PartKind::Door)),
            "no partition in the hall"
        );
        let mid = design.build_area / 2;
        let d2 = |(x, y): (u32, u32), (cx, cy): (u32, u32)| {
            x.abs_diff(cx).pow(2) + y.abs_diff(cy).pow(2)
        };
        for conduits in 4..=7 {
            let places = places(&design, conduits, data::HEART_FABRICATORS);
            assert!(d2(places.core, (mid, mid)) <= 2, "{:?}", places.core);
            assert_eq!(places.fabricators.len(), 2);
            assert!(places.fabricators.iter().all(|&f| d2(f, places.core) <= 25));
            assert_eq!(places.conduits.len(), conduits as usize);
            for (i, &a) in places.conduits.iter().enumerate() {
                let out = d2(a, places.core);
                assert!((15 * 15..=23 * 23).contains(&out), "{a:?} off the ring");
                for &b in &places.conduits[i + 1..] {
                    assert!(d2(a, b) > 8 * 8, "{a:?} beside {b:?}");
                }
            }
        }
    }

    /// It is the same fortress every time, and it stands clear of every
    /// node of its system and of where a jump lands.
    #[test]
    fn the_fortress_stands_clear_and_is_the_same_every_time() {
        let galaxy = Galaxy::new(0x1234_5678, GalaxyType::Round);
        for star in [0u32, 17, 160, 239] {
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
