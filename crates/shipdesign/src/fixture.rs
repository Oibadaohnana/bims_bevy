//! The reference design: one ship, built the same way every time.
//!
//! It is in the library rather than in the tests because two targets have to
//! agree about it. [`design_hash`](crate::design_hash) is what an Accept is
//! recorded against, so it has to come out identical on the native server that
//! will one day be authoritative and in the wasm the players are running — and
//! the only way to find out is to compute it on both and compare each against
//! the same written-down number.
//!
//! The native end is `tests.rs`; the wasm end is `ship_self_check` in
//! `crates/ship`, which the node harness reads. Both compare against
//! [`REFERENCE_HASH`] below. If one target's arithmetic ever drifts from the
//! other's, exactly one of those two fails.

use economy::Money;
use physics::ResourceId;

use crate::budget::Budget;
use crate::design::{Edit, ShipDesign, apply, wall_light_rotation};
use crate::parts::{Layer, PartKind, Rotation};

/// Tiles a side. Small enough to read, big enough to hold a working ship.
pub const AREA: u32 = 20;

/// The crew sizes the reference is built for, in the order
/// [`REFERENCE_HASH`] and [`REFERENCE_PARTS`] are indexed.
pub const CREWS: [u32; 2] = [1, 4];

/// What the reference is built with. Deliberately far more than it needs:
/// this fixture is about the hash and the rules, and a reference ship that
/// ran out of money halfway would be a reference ship whose parts count moved
/// every time a price was tuned.
pub const REFERENCE_POOL: Money = 10_000_000;

/// What [`reference`] hashes to, for each of [`CREWS`].
///
/// Pinned rather than computed: these are the numbers that catch a target
/// hashing differently, and a test that compares two computed values would
/// pass happily while both were wrong. Update them only when the reference
/// design itself is meant to change — or the cargo it hashes grows: two
/// empty slots for the minigun and the rail lance (task 115, `CARGO_SLOTS`
/// 20) moved both from `0x0617_9497_2e16_65ac` and `0x5b4f_9597_b2c6_baf7`,
/// and two more for the arc greaves and the Reflective plate (task 116,
/// `CARGO_SLOTS` 22) from `0xb4b9_08bd_0dda_d32c` and
/// `0xa7a4_60c0_b042_7fd7`, and the drug lab gone (task 120: every part's
/// code after it one lower) from `0x4b3b_e276_e853_f0ac` and
/// `0xe89b_f26c_b13f_a0b7`, and the electricity gone (October 2026: the
/// reactor and its fifty tiles of conduit off the ship, every part code
/// after the helm three lower and after the research desk four) from
/// `0x6254_985d_4f75_5c03` and `0x67b6_c899_2733_a7d8`.
pub const REFERENCE_HASH: [u64; 2] = [0xf78d_b964_dda6_07dd, 0x8d2f_f62b_d15f_1146];

/// What [`reference`] is carrying, whatever the crew size: a few days of
/// vegetables and tofu, bought through [`apply`] like everything else.
///
/// Pinned for the same reason the hash is — the cargo is *in* the hash, so a
/// target that added up a manifest differently would show up here rather
/// than as an unexplained number.
pub const REFERENCE_CARGO: [(ResourceId, u32); 2] =
    [(ResourceId::Vegetable, 40), (ResourceId::Tofu, 20)];

/// How many parts [`reference`] ends up with, for each of [`CREWS`].
///
/// [`reference`] skips an edit that does not take rather than panicking — a
/// panic in a cdylib is an abort and tells nobody anything. This is what
/// notices the skip instead. 717 and 723 until the reactor and its
/// conduit went with the electricity.
pub const REFERENCE_PARTS: [u32; 2] = [666, 672];

/// The two columns of the stern row the reference's engine stands in, its
/// bell in the skin. Two, because the engine is two across.
const REFERENCE_ENGINE_STERN: [u32; 2] = [7, 8];

/// Where the reference's wall lights hang: against the side walls, two a
/// side, each turned to its wall (`wall_light_rotation`); and where its
/// standing light stands, amidships.
const REFERENCE_LIGHTS: [(u32, u32); 4] = [(2, 5), (17, 5), (2, 14), (17, 14)];
const REFERENCE_LAMP: (u32, u32) = (10, 9);

/// The reference ship: a framed, floored, hull-plated compartment with a
/// galley along the top, a table and chairs, bunks down the right, a helm, an
/// engine, a bay and a locker, and a few days' food in the cold store. Valid
/// for `crew`, with no errors, no missing fixture and **no radiation getting
/// in** — the hull is outside wall the whole way round.
///
/// Built through [`apply`] like anything else, so it is a ship the rules
/// admit rather than a hand-assembled `Vec` that might not be. That goes for
/// the cargo as well: the food is bought, not written into the array.
pub fn reference(crew: u32) -> ShipDesign {
    let budget = Budget::new(REFERENCE_POOL);
    let mut design = ShipDesign::new(AREA);

    let put = |design: &mut ShipDesign, kind: PartKind, origin, rotation| {
        if let Ok(next) = apply(
            design,
            &budget,
            Edit::Place {
                kind,
                origin,
                rotation,
            },
        ) {
            *design = next;
        }
    };

    // The frame first, and it has to reach everywhere anything else goes:
    // under the deck, and under the hull ring one tile outside it.
    for y in 1..19 {
        for x in 1..19 {
            put(&mut design, PartKind::Structure, (x, y), Rotation::R0);
        }
    }

    // Then the deck, inside the ring.
    for y in 2..18 {
        for x in 2..18 {
            put(&mut design, PartKind::Floor, (x, y), Rotation::R0);
        }
    }

    // Hull round the outside of it. Outside wall rather than plain wall:
    // plain wall does not shield, and a ship skinned in it is a ship whose
    // crew are being cooked. The corners are placed twice and the second
    // attempt is refused, which is the loop being simple rather than a bug.
    for i in 1..19 {
        put(&mut design, PartKind::OutsideWall, (i, 1), Rotation::R0);
        // The stern is hull too, bar where the engine goes: an engine fires
        // aft and has to fire into space (`IssueCode::ExhaustBlocked`), so
        // it stands in the skin with its bell over the edge — decked, since
        // an engine stands on deck — and seals it, since an engine shields.
        if !REFERENCE_ENGINE_STERN.contains(&i) {
            put(&mut design, PartKind::OutsideWall, (i, 18), Rotation::R0);
        } else {
            put(&mut design, PartKind::Floor, (i, 18), Rotation::R0);
        }
        put(&mut design, PartKind::OutsideWall, (1, i), Rotation::R0);
        put(&mut design, PartKind::OutsideWall, (18, i), Rotation::R0);
    }

    // The galley and the heads along row 3, everything facing down the room,
    // so every use spot is the open row below them.
    put(&mut design, PartKind::ColdStore, (3, 3), Rotation::R0);
    put(&mut design, PartKind::Worktop, (5, 3), Rotation::R0);
    put(&mut design, PartKind::Hob, (8, 3), Rotation::R0);
    put(&mut design, PartKind::Dishwasher, (10, 3), Rotation::R0);
    put(&mut design, PartKind::Toilet, (12, 3), Rotation::R0);
    put(&mut design, PartKind::Basin, (14, 3), Rotation::R0);
    put(&mut design, PartKind::BroomLocker, (16, 3), Rotation::R0);

    put(&mut design, PartKind::Table, (4, 6), Rotation::R0);
    put(&mut design, PartKind::Helm, (7, 6), Rotation::R0);
    // Six trays along row 10 to port, worked from the row above them.
    put(&mut design, PartKind::HydroBay, (3, 10), Rotation::R0);
    put(
        &mut design,
        PartKind::Engine,
        (REFERENCE_ENGINE_STERN[0], 16),
        Rotation::R0,
    );

    // Light: a wall light on each side wall, fore and aft, and a standing
    // light amidships. Without them the deck is dark and the crew see
    // ten tiles — `bims::sight`.
    for tile in REFERENCE_LIGHTS {
        let hung = wall_light_rotation(&design, tile).expect("a wall beside it");
        put(&mut design, PartKind::WallLight, tile, hung);
    }
    put(
        &mut design,
        PartKind::StandingLight,
        REFERENCE_LAMP,
        Rotation::R0,
    );

    // A bed and a seat each. The chairs go on the table's own use spots where
    // there are two of them and beside it after that — a chair is walked onto
    // rather than round, so sitting one in a doorway of a use spot is fine.
    for i in 0..crew {
        put(&mut design, PartKind::Bunk, (14, 8 + 2 * i), Rotation::R0);
        put(
            &mut design,
            PartKind::Chair,
            (4 + i % 2, 7 + 2 * (i / 2)),
            Rotation::R0,
        );
    }

    // And something to eat. Bought through `apply` like everything else, so
    // the cold store's capacity is what bounds it — and so the cargo half of
    // `design_hash` is pinned by the same cross-target check the parts are.
    for (resource, units) in REFERENCE_CARGO {
        if let Ok(next) = apply(&design, &budget, Edit::Buy { resource, units }) {
            design = next;
        }
    }

    design
}

/// Where the flyer's four thrusters and its sensor array go, as hull tiles
/// they **replace**.
///
/// Mid-edge rather than at the corners, and all four of them, because what a
/// thruster is worth is its distance from the centre of mass and a corner is
/// no further out than the middle of a side on a square hull. Replacing the
/// plating rather than standing beside it is the only option there is: a
/// thruster is hull, it stands on the frame, and a tile holds one object.
const THRUSTER_TILES: [(u32, u32); 4] = [(9, 1), (9, 18), (1, 9), (18, 9)];
const SENSOR_TILE: (u32, u32) = (5, 1);

/// The two hull tiles the flyer's airlock stands in: the starboard skin,
/// below the thruster there. In the skin rather than on the deck, because a
/// port is an airlock with space on one side of it — `dock::port` — and an
/// airlock in the middle of the deck is a door to nowhere the ship could not
/// dock by. The plating comes off, deck goes on, the airlock stands on that.
const AIRLOCK_TILES: [(u32, u32); 2] = [(18, 11), (18, 12)];

/// The reference ship again, with everything a trip actually needs.
///
/// [`reference`] is a ship you can **live** on and it is deliberately not one
/// you can fly: it has an engine and nothing else, so it raises every one of
/// the flight warnings and is exactly the fixture those warnings are tested
/// against. This is the other one — four thrusters to turn with, an airlock
/// to dock through, a sensor array to see with — and it is what `flight` and
/// `world` measure their scenarios against.
///
/// Its hash is deliberately **not** pinned. [`REFERENCE_HASH`] is about two
/// targets agreeing; this one is about a trip being flyable, and pinning a
/// second number would only mean a second thing to update whenever a placement
/// here moved.
pub fn flyer(crew: u32) -> ShipDesign {
    let budget = Budget::new(REFERENCE_POOL);
    let mut design = reference(crew);

    // The hull comes off first. Both replacements shield, so the skin is
    // still closed when they go back on — which `exposure` is asked about in
    // `a_flyer_is_still_sealed`.
    let swap = |design: &mut ShipDesign, tile: (u32, u32), kind: PartKind| {
        let standing = design
            .grid()
            .get(Layer::Object, (tile.0 as i32, tile.1 as i32));
        if standing != 0
            && let Ok(next) = apply(design, &budget, Edit::Remove { part_id: standing })
        {
            *design = next;
        }
        if let Ok(next) = apply(
            design,
            &budget,
            Edit::Place {
                kind,
                origin: tile,
                rotation: Rotation::R0,
            },
        ) {
            *design = next;
        }
    };

    for tile in THRUSTER_TILES {
        swap(&mut design, tile, PartKind::Thruster);
    }
    swap(&mut design, SENSOR_TILE, PartKind::SensorArray);

    for tile in AIRLOCK_TILES {
        swap(&mut design, tile, PartKind::Floor);
    }
    if let Ok(next) = apply(
        &design,
        &budget,
        Edit::Place {
            kind: PartKind::Airlock,
            origin: AIRLOCK_TILES[0],
            rotation: Rotation::R0,
        },
    ) {
        design = next;
    }

    design
}

// --- the playtest ship ------------------------------------------------------

/// What [`playtest_ship`] hashes to. Pinned for the reason [`REFERENCE_HASH`]
/// is: `ship_self_check` computes it in wasm and `tests.rs` natively, and a
/// target that hashed the simulation's ship differently would start a
/// different simulation. Update it only when the ship below is meant to
/// change — it moved from `0xd9cb_319c_6859_03ec` for two more empty cargo
/// slots (task 115), the ship itself untouched, from
/// `0x9647_17e1_0fef_b96c` for two more (task 116), and from
/// `0xa5d6_d976_ad54_1eec` for the drug lab taken out and the bandages and
/// medkits out of the cargo (task 120), and from `0x8046_76c8_2953_29f7`
/// for the hydroponic bay and its tile of conduit taken out (the bots
/// wedged themselves beside it), and from `0xad31_5e8e_0e0d_0049` for
/// the helm, the kevlar and the leg guards in the cargo become one armour
/// (October 2026, one armour slot), and from `0xa708_bdd4_e7b8_4f49` for
/// the electricity gone (October 2026: both reactors, the battery and the
/// sixty-nine tiles of conduit off the ship).
pub const PLAYTEST_HASH: u64 = 0x92a7_b1c3_5e26_ef7f;

/// How many parts [`playtest_ship`] ends up with. What notices a placement
/// that was quietly refused — the builder skips rather than panics, for the
/// reason [`REFERENCE_PARTS`] gives. 666 until the bay and its conduit
/// went, 664 until the reactors, the battery and the conduit did.
pub const PLAYTEST_PARTS: u32 = 592;

/// The playtest hull, as columns of the grid: the west skin and the east,
/// the bow row and the stern row. Sixteen tiles across and eighteen long,
/// which is what a twenty-tile grid leaves room for with a tile of space
/// round it.
const PLAYTEST_WEST: u32 = 2;
const PLAYTEST_EAST: u32 = 17;
const PLAYTEST_BOW: u32 = 1;
const PLAYTEST_STERN: u32 = 18;

/// How many tiles each bow corner is cut back by. The cut runs at
/// forty-five degrees in [`PartKind::DiagonalOutsideWall`] pieces from the
/// skin to the bow row, and everything outside the cut is not part of the
/// ship: no frame, no deck.
const PLAYTEST_CHAMFER: u32 = 4;

/// Where the playtest ship's thrusters go: hull tiles in the two skins,
/// forward and aft, which they **replace**, as the flyer's do. In the
/// straight skin rather than in the chamfer, because a thruster is square.
const PLAYTEST_THRUSTERS: [(u32, u32); 4] = [(2, 7), (17, 7), (2, 16), (17, 16)];

/// The hull tile the sensor array stands in: the bow, just off the
/// centreline.
const PLAYTEST_SENSOR: (u32, u32) = (9, 1);

/// The hull tiles the airlock stands in: one column of the starboard skin,
/// two tall, amidships. They get deck first, because an airlock stands on
/// deck, and the airlock shields, so the hull is as closed as it was.
/// Starboard, because every station's port is in its west skin, and a ship
/// docking starboard-to does so at heading nought.
const PLAYTEST_AIRLOCK: [(u32, u32); 2] = [(17, 11), (17, 12)];

/// The two stern tiles the main engine stands in, in place of hull: the
/// engine shields, so it is the stern there, and its bell is flush with
/// the skin rather than buried a tile inside it.
const PLAYTEST_ENGINE_TILES: [(u32, u32); 2] = [(9, 18), (10, 18)];

/// Where the playtest ship's wall lights hang — the bridge's against the
/// chamfer, the main deck's and engineering's against the hull — and
/// where its standing light stands, on the main deck.
const PLAYTEST_LIGHTS: [(u32, u32); 6] = [(5, 3), (14, 3), (3, 8), (3, 11), (3, 15), (16, 15)];
const PLAYTEST_LAMP: (u32, u32) = (7, 9);

/// Where the playtest ship's comforts go: the picture on the bridge
/// bulkhead beside the doorway, and the small plant at the foot of the
/// bunk.
const PLAYTEST_PICTURE: (u32, u32) = (8, 7);
const PLAYTEST_PLANT: (u32, u32) = (16, 10);

/// What the playtest ship carries: a few days of food, one suit in the
/// locker, a few bandages and a couple of medkits, so a wound can be
/// dressed from the first minute and the drug lab tried, one armour,
/// so the armoury's grid has something
/// in it to equip, and one of each weapon after the handgun, so every gun
/// and the schword can be put in a hand without first being bought.
/// Bought through [`apply`], so the cold store and the lockers are what
/// bound it — and the locker class is the suit locker, the drug lab's
/// cabinet, the armoury's and the shelves between them, which is what
/// makes room for the armour and the weapons.
///
/// The materials went with the money rework (feature 95): the metal, the
/// components and the ore for the smelter had nothing left to be spent
/// on, and the fibre nothing left to be rolled into.
pub const PLAYTEST_CARGO: [(ResourceId, u32); 8] = [
    (ResourceId::Vegetable, 40),
    (ResourceId::Tofu, 20),
    (ResourceId::Suit, 1),
    (ResourceId::Armour, 1),
    (ResourceId::Shotgun, 1),
    (ResourceId::AutoRifle, 1),
    (ResourceId::SniperRifle, 1),
    (ResourceId::Schword, 1),
];

/// Whether a tile of the playtest grid is inside the hull's outline —
/// on the frame, that is — and if it is on the cut, which way the corner
/// piece there faces. `None` is outside; `Some(None)` is a plain tile of
/// the ship; `Some(Some(r))` is a chamfer piece turned `r`.
///
/// The bow corners are cut by [`PLAYTEST_CHAMFER`]: a tile whose distance
/// in from the corner, across plus along, is less than the cut is off the
/// ship, one exactly on it is a corner piece, and the rest are the ship.
/// The solid half of each piece faces the middle of the ship — south-east
/// on the port side, south-west to starboard — which is what seals it.
fn playtest_outline(x: u32, y: u32) -> Option<Option<Rotation>> {
    if x < PLAYTEST_WEST || x > PLAYTEST_EAST || y < PLAYTEST_BOW || y > PLAYTEST_STERN {
        return None;
    }
    let from_bow = y - PLAYTEST_BOW;
    let port = (x - PLAYTEST_WEST) + from_bow;
    let starboard = (PLAYTEST_EAST - x) + from_bow;
    if port < PLAYTEST_CHAMFER || starboard < PLAYTEST_CHAMFER {
        None
    } else if port == PLAYTEST_CHAMFER {
        Some(Some(Rotation::R270))
    } else if starboard == PLAYTEST_CHAMFER {
        Some(Some(Rotation::R0))
    } else {
        Some(None)
    }
}

/// Whether a tile is on the straight skin: the two sides, the bow row and
/// the stern row, where the outline has not cut them off.
fn playtest_skin(x: u32, y: u32) -> bool {
    x == PLAYTEST_WEST || x == PLAYTEST_EAST || y == PLAYTEST_BOW || y == PLAYTEST_STERN
}

/// The ship `nix run .#simulation` opens with: one of everything a crew of
/// one needs to live and to fly, on a twenty-tile grid, with a stocked
/// hold. Valid for one with **no errors and no warnings**.
///
/// It is laid out the way a small ship would be: a pointed bow with the
/// bridge in it, the main deck amidships with the galley to port and the
/// armoury, the bunk and the airlock to starboard, and engineering aft —
/// the heads, a shelf of stores, and the main engine set
/// into the stern so its bell is the stern. Three compartments,
/// and every doorway and every gangway two tiles wide, because the room's
/// navigation cannot walk a one-tile gap (see the crate's module note);
/// a fixture whose use spot can only be reached down a one-tile channel is
/// a Bim frozen in front of it.
///
/// It is not [`flyer`] for one crew, because a playtest ship is meant to
/// be changed as the game grows without moving the reference that two
/// targets are compared on. Built through [`apply`] like everything else.
///
/// The grid, `x` across and `y` down, hull `#`, corner pieces `/` and `\`,
/// bulkheads `=`, doors `+`, deck `.`, and the first letter of everything
/// else (`T` thruster, `S` sensor array, `A` airlock, `E` engine, `H` helm,
/// `L` life support, then `C` cold store, `W` worktop, `H` hob,
/// `D` dishwasher, `B` locker, `S` suit locker, `A` armoury, `T` table,
/// `C` chair, `B` bunk, `S` shelf, `T` toilet, `B` basin, `S` shower,
/// `W` workbench):
///
/// ```text
///  1       \##S###/
///  2      \......../
///  3     \....HH..../
///  4    \..........LL/
///  5   \...........LL./
///  6   #======++======#
///  7   TCWWHD...BS.AA.T
///  8   #.............B#
///  9   #.............B#
/// 10   #.TT...........#
/// 11   #.C............A
/// 12   #..............A
/// 13   #============++#
/// 14   #...SS...TBS...#
/// 15   #..............#
/// 16   T......EE...SS.T
/// 17   #......EEWW.SS.#
/// 18   #######EE#######
/// ```
pub fn playtest_ship() -> ShipDesign {
    let budget = Budget::new(REFERENCE_POOL);
    let mut design = ShipDesign::new(AREA);

    let put = |design: &mut ShipDesign, kind: PartKind, origin, rotation| {
        if let Ok(next) = apply(
            design,
            &budget,
            Edit::Place {
                kind,
                origin,
                rotation,
            },
        ) {
            *design = next;
        }
    };
    let take = |design: &mut ShipDesign, tile: (u32, u32)| {
        let standing = design
            .grid()
            .get(Layer::Object, (tile.0 as i32, tile.1 as i32));
        if standing != 0
            && let Ok(next) = apply(design, &budget, Edit::Remove { part_id: standing })
        {
            *design = next;
        }
    };

    // The frame over the outline, the deck inside the skin, and the skin
    // itself: plating along the straight runs and corner pieces on the cut.
    for y in PLAYTEST_BOW..=PLAYTEST_STERN {
        for x in PLAYTEST_WEST..=PLAYTEST_EAST {
            let Some(cut) = playtest_outline(x, y) else {
                continue;
            };
            put(&mut design, PartKind::Structure, (x, y), Rotation::R0);
            match cut {
                Some(turn) => put(&mut design, PartKind::DiagonalOutsideWall, (x, y), turn),
                None if playtest_skin(x, y) => {
                    put(&mut design, PartKind::OutsideWall, (x, y), Rotation::R0)
                }
                None => put(&mut design, PartKind::Floor, (x, y), Rotation::R0),
            }
        }
    }

    // The hull's working parts, in place of plating: thrusters in the
    // skins, the array at the bow, the airlock to starboard, and the main
    // engine's two stern tiles decked so it can stand flush with the skin.
    for tile in PLAYTEST_THRUSTERS {
        take(&mut design, tile);
        put(&mut design, PartKind::Thruster, tile, Rotation::R0);
    }
    take(&mut design, PLAYTEST_SENSOR);
    put(
        &mut design,
        PartKind::SensorArray,
        PLAYTEST_SENSOR,
        Rotation::R0,
    );
    for tile in PLAYTEST_AIRLOCK.into_iter().chain(PLAYTEST_ENGINE_TILES) {
        take(&mut design, tile);
        put(&mut design, PartKind::Floor, tile, Rotation::R0);
    }
    put(
        &mut design,
        PartKind::Airlock,
        PLAYTEST_AIRLOCK[0],
        Rotation::R0,
    );

    // Two bulkheads across the ship — the bridge forward of row 6, the main
    // deck, engineering aft of row 13 — each with a two-tile doorway: one
    // door, turned to run along the bulkhead.
    for (y, door) in [(6u32, 9u32..=10), (13, 15..=16)] {
        for x in PLAYTEST_WEST + 1..PLAYTEST_EAST {
            if !door.contains(&x) {
                put(&mut design, PartKind::Wall, (x, y), Rotation::R0);
            }
        }
        put(
            &mut design,
            PartKind::Door,
            (*door.start(), y),
            Rotation::R90,
        );
    }

    // The bridge: the helm on the centreline facing the bow, the pilot's
    // spot behind it, life support in the corner the cut leaves.
    put(&mut design, PartKind::Helm, (9, 3), Rotation::R0);
    put(&mut design, PartKind::LifeSupport, (14, 4), Rotation::R0);

    // The main deck: the galley along the bridge bulkhead to port, worked
    // from the row below it, with the locker beyond the door; the table and
    // its chair under the galley; the bunk against the starboard skin,
    // forward of the airlock so the way through it stays clear. The
    // hydroponic bay's six trays stood across the middle of the deck at
    // (9, 10) until the bots kept wedging themselves in the one-tile gap
    // between its east end and the plant at the foot of the bunk; nothing
    // replaced it, and nothing aboard eats since feature 104.
    put(&mut design, PartKind::ColdStore, (3, 7), Rotation::R0);
    put(&mut design, PartKind::Worktop, (4, 7), Rotation::R0);
    put(&mut design, PartKind::Hob, (6, 7), Rotation::R0);
    put(&mut design, PartKind::Dishwasher, (7, 7), Rotation::R0);
    put(&mut design, PartKind::BroomLocker, (11, 7), Rotation::R0);
    put(&mut design, PartKind::SuitLocker, (12, 7), Rotation::R0);
    put(&mut design, PartKind::Table, (4, 10), Rotation::R0);
    put(&mut design, PartKind::Chair, (4, 11), Rotation::R0);
    put(&mut design, PartKind::Bunk, (16, 8), Rotation::R0);
    // The armoury along the bridge bulkhead to starboard, forward of the
    // bunk and worked from the row below it like the galley.
    put(&mut design, PartKind::Armoury, (14, 7), Rotation::R0);

    // Engineering: a shelf of stores; the heads along the bulkhead to
    // starboard; and the engine on the centreline, its bell in the stern.
    put(&mut design, PartKind::Shelf, (6, 14), Rotation::R0);
    put(&mut design, PartKind::Toilet, (11, 14), Rotation::R0);
    put(&mut design, PartKind::Basin, (12, 14), Rotation::R0);
    put(&mut design, PartKind::Shower, (13, 14), Rotation::R0);
    put(&mut design, PartKind::Engine, (9, 16), Rotation::R0);
    // The workbench, to starboard of the engine and turned to face
    // forward, so it is worked from the row above it — the row below is
    // the stern. The smelter stood beside it until the money rework
    // (feature 95) took ore and metal away; nothing replaced it.
    put(&mut design, PartKind::Workbench, (11, 17), Rotation::R180);
    put(&mut design, PartKind::Shelf, (7, 14), Rotation::R0);
    // The drug lab stood in the stern row to port of the engine until the
    // medicine went out of the game (task 120); nothing replaced it.
    // The research desk on the spine in engineering's forward row, between
    // the shelves and the heads, worked from the row below it: the one
    // two-tile spot on the ship with deck on
    // the far side of its use spot — a spot between two solids is one the
    // room's navigation will not walk. Where the AI does its thinking.
    put(&mut design, PartKind::ResearchDesk, (8, 14), Rotation::R0);

    // Light: wall lights against the hull and the chamfer, a deck each,
    // and a standing light on the main deck. See `bims::sight` for what
    // a dark deck costs.
    for tile in PLAYTEST_LIGHTS {
        let hung = wall_light_rotation(&design, tile).expect("a wall beside it");
        put(&mut design, PartKind::WallLight, tile, hung);
    }
    put(
        &mut design,
        PartKind::StandingLight,
        PLAYTEST_LAMP,
        Rotation::R0,
    );

    // Comforts on the main deck: a picture on the bridge bulkhead beside
    // the doorway, hung like a lamp, and a small plant at the foot of the
    // bunk, clear of the gangway inside the port. What they do to the
    // crew's surroundings is `bims::filth`'s.
    let hung = wall_light_rotation(&design, PLAYTEST_PICTURE).expect("a wall beside it");
    put(&mut design, PartKind::Picture, PLAYTEST_PICTURE, hung);
    put(
        &mut design,
        PartKind::SmallPlant,
        PLAYTEST_PLANT,
        Rotation::R0,
    );

    for (resource, units) in PLAYTEST_CARGO {
        if let Ok(next) = apply(&design, &budget, Edit::Buy { resource, units }) {
            design = next;
        }
    }

    design
}

/// How many berths the combat ship has — a bunk and a chair each — one for
/// each kind of weapon, so every gun is in a hand at once. The ship
/// validates clean for a crew of this many, and the `test` command's one
/// crew member has four of them to spare.
pub const COMBAT_BERTHS: u32 = 5;

/// How many crew the `combat` command puts aboard it: the first
/// [`COMBAT_BERTHS`] at their bunks, the other eleven standing on the deck
/// (`bims::aboard::starts`) — a squad rather than a handful, for a fight
/// with a garrison of `world::data::ARENA_GARRISON`, the last four of it
/// hired field medics (`ship::session::COMBAT_MEDICS`). More than the ship
/// sleeps, which the validator would say and the command does not ask it.
pub const COMBAT_CREW: u32 = 16;

/// Where [`combat_ship`] puts its four extra bunks: on the bridge, the one
/// compartment with room for them — the main deck's aft rows are two deep
/// above the bulkhead, and a bunk there seals a pocket of deck. One lies
/// along the bow to port, got into from the row below; two stand against
/// the life support to starboard, got into from the west; one stands
/// in the port corner, got into from the east — each with a
/// two-tile gangway to it, since the room's navigation cannot walk a
/// one-tile gap. The pilot's spot and the doorway stay clear.
pub const COMBAT_BUNKS: [((u32, u32), Rotation); 4] = [
    ((6, 2), Rotation::R270),
    ((13, 2), Rotation::R0),
    ((13, 4), Rotation::R0),
    ((5, 4), Rotation::R180),
];

/// Where [`combat_ship`] puts its four extra chairs: the table's other
/// seat, and three along the aft bulkhead of the main deck. A chair is a
/// seat and blocks nothing, so the mess is where the seats are.
pub const COMBAT_CHAIRS: [(u32, u32); 4] = [(5, 11), (6, 12), (7, 12), (8, 12)];

/// [`playtest_ship`] with a bunk and a chair for each of [`COMBAT_BERTHS`]:
/// the same ship, the same cargo, four more bunks at [`COMBAT_BUNKS`] and
/// four more chairs at [`COMBAT_CHAIRS`]. It is what the `combat` command
/// opens on — [`COMBAT_CREW`] aboard, a gun in every hand — and the `test`
/// command, and nothing else, so its hash is not pinned: it is the
/// playtest ship with eight parts added, and
/// `the_combat_ship_sleeps_a_crew_of_five` in the tests counts them.
pub fn combat_ship() -> ShipDesign {
    let budget = Budget::new(REFERENCE_POOL);
    let mut design = playtest_ship();
    let bunks = COMBAT_BUNKS
        .iter()
        .map(|&(origin, rotation)| (PartKind::Bunk, origin, rotation));
    let chairs = COMBAT_CHAIRS
        .iter()
        .map(|&origin| (PartKind::Chair, origin, Rotation::R0));
    for (kind, origin, rotation) in bunks.chain(chairs) {
        if let Ok(next) = apply(
            &design,
            &budget,
            Edit::Place {
                kind,
                origin,
                rotation,
            },
        ) {
            design = next;
        }
    }
    design
}

/// [`playtest_ship`] laid out in the middle of a bigger build area — what
/// the design phase opens with, so nobody starts from an empty grid.
///
/// The same parts at the same places, shifted by half the difference, and
/// the same cargo, all put down through [`apply`] again so the result is a
/// ship the rules admit on *that* grid rather than a copy with its numbers
/// changed. `None` for an area the ship does not fit, which is an empty
/// grid for the player rather than half a ship.
///
/// Its hash is not pinned: it is [`PLAYTEST_HASH`]'s ship moved, and the
/// part count says whether every part came across.
pub fn playtest_ship_on(area: u32) -> Option<ShipDesign> {
    ship_on(playtest_ship(), area)
}

/// [`combat_ship`] laid out on a build area of `area` tiles, the way
/// [`playtest_ship_on`] lays the playtest ship out: what a lobby of more
/// than one is given in the yard, since the playtest ship has one bunk
/// and one chair and a crew of two would be an error before anybody laid
/// a tile.
pub fn combat_ship_on(area: u32) -> Option<ShipDesign> {
    ship_on(combat_ship(), area)
}

/// A ship of [`AREA`] tiles laid out again on `area` of them — the rule
/// [`playtest_ship_on`] describes.
fn ship_on(source: ShipDesign, area: u32) -> Option<ShipDesign> {
    if area < AREA {
        return None;
    }
    let shift = (area - AREA) / 2;
    let budget = Budget::new(REFERENCE_POOL);
    let mut design = ShipDesign::new(area);
    // In id order, which is placement order: the frame went down before the
    // deck and the deck before what stands on it, and ids only ever climb.
    for part in &source.parts {
        if let Ok(next) = apply(
            &design,
            &budget,
            Edit::Place {
                kind: part.kind,
                origin: (part.origin.0 + shift, part.origin.1 + shift),
                rotation: part.rotation,
            },
        ) {
            design = next;
        }
    }
    for (resource, units) in PLAYTEST_CARGO {
        if let Ok(next) = apply(&design, &budget, Edit::Buy { resource, units }) {
            design = next;
        }
    }
    Some(design)
}
