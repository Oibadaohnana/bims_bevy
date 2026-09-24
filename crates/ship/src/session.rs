//! One sitting at the ship: the design phase, and the game the last Accept
//! turns it into.
//!
//! A [`Session`] is what `crates/app` owns for the whole of `ship`'s life —
//! an [`Editor`] from the first frame, a [`Game`] from the moment everybody
//! has accepted. Almost every question about the ship is one question
//! whichever half it is in — "how much metal is aboard" is asked of the
//! design being laid out and of the ship that is flying — which is why the
//! two-armed reads below live here rather than in the app: the app should
//! not have to know which phase it is in to ask.
//!
//! Nothing here decides anything. It asks `shipdesign` and `world`.

use flight::Target;
use physics::{Facing, ResourceId};
use shipdesign::market::{Bias, Market, Quote};
use shipdesign::parts::{Layer, PartKind, footprint};
use shipdesign::{Money, ShipDesign, TILE, storage};
use worldgen::{GalaxyType, Node};

use crate::draw::{Color, DrawList};
use crate::editor::{Editor, Phase};
use crate::game::Game;
use crate::{paint, world_paint};
use bims::character::{Hair, Look, Shade, Tint};
use world::Class;

/// "The lobby did not say." What a star or a station id is when there is
/// none — `u32::MAX`, the same value the lobby uses for nothing, because
/// star 0 and station 0 both exist.
pub const NONE: u32 = u32::MAX;

/// What a design phase opens on. The playtest ship unless asked for an
/// empty grid, because a player who wanted an empty grid can clear one and
/// a player who wanted a ship cannot conjure one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Preset {
    Empty,
    Playtest,
}

/// World units to a tile side, for the app's readouts. The geometry is all
/// done in here.
pub const TILE_UNITS: u32 = TILE;

pub fn galaxy_type(code: u32) -> GalaxyType {
    GalaxyType::ALL
        .get(code as usize)
        .copied()
        .unwrap_or(GalaxyType::SpiralTwoArm)
}

/// The spawn the lobby gave, or none. Both halves have to be there — a star
/// with no station is no more a spawn than nothing at all.
pub fn spawn_from(star: u32, station: u32) -> Option<(u32, u32)> {
    (star != NONE && station != NONE).then_some((star, station))
}

/// A dock somebody lives on, anywhere in the galaxy the seed and type name:
/// the `roll`-th of them, wrapping. For the `test` command — the simulation
/// somewhere else each time — where the app rolls the number and which dock
/// the roll lands on is this crate's, so the same roll on the same seed is
/// the same place on every machine.
pub fn pick_dock(seed: u64, galaxy: u32, roll: u64) -> Option<(u32, u32)> {
    world::spawn_anywhere(&worldgen::Galaxy::new(seed, galaxy_type(galaxy)), roll)
}

/// [`pick_dock`] in a system with a planet to set down on whose people are
/// not enemies — `world::spawn_with_ground`. For the `test_planet` command,
/// which is `test` landed on that planet (`Session::land_for_probe`).
pub fn pick_ground(seed: u64, galaxy: u32, roll: u64) -> Option<(u32, u32)> {
    world::spawn_with_ground(&worldgen::Galaxy::new(seed, galaxy_type(galaxy)), roll)
}

/// How many hyperlane hops the `crisis` command puts between the crew's
/// star and the machines' origin (feature 92) — [`Session::crisis_for_probe`].
///
/// Two, where the roll's own floor is eight: at eight, the first star to
/// turn is forty days of the clock from the crew and the chart shows one
/// red speck on the far rim. At two, the crew's own system falls ten days
/// after the first, which is a run somebody can sit through.
pub const CRISIS_HOPS: u16 = 2;

/// How many **hired field medics** the `combat` command's crew carries
/// (feature 86) — [`Session::combat`], and so every fight built on it:
/// `tier2_test`, `tier3_test`, `droids`, and the `combat_<class>` and
/// `combat_droids_<class>` runs.
///
/// Four, and the last four of the sixteen. One would be a rescue that
/// stops the moment it is the one shot; two was a pair that fetch for
/// each other, which is the state the carry was written for; and the
/// user asked for two more on top of those — the crew grew by the two
/// rather than trading two of its guns for them — so a fight with
/// people going down on both flanks has somebody free for each.
pub const COMBAT_MEDICS: usize = 4;

/// A planet with a town the ship can set down at, as the map writes it:
/// which body, whose the town is, and where the icon is drawn.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct LandingSite {
    pub node: Node,
    /// Its people are enemies.
    pub hostile: bool,
    /// The machines are one hop away and it is next (feature 94):
    /// `World::town_threatened`.
    pub threatened: bool,
    /// The crew defended it and held it: it stays friendly for good.
    pub held: bool,
    /// Where the map draws it, in the camera's units about the ship.
    pub at: (f32, f32),
}

pub struct Session {
    pub editor: Editor,
    /// The game, once there is one. `None` for the whole of the design
    /// phase.
    pub game: Option<Game>,
    /// What the lobby asked for, kept from the first frame until Accept
    /// hands it to the world. A galaxy is a seed and a type; where in it the
    /// game starts is a star and a station, and `None` when the lobby did
    /// not say — which is an error screen, never a different dock.
    pub seed: u64,
    pub galaxy: u32,
    pub spawn: Option<(u32, u32)>,
    /// What the players called their crew, in slot order — the one
    /// string a session carries, and the app's to spell (feature 60).
    /// Empty, or empty at a slot, is the app's own name for that slot.
    /// Saved with the world, since a crew renamed and read back under
    /// the old names would be somebody else's crew.
    pub crew_names: Vec<String>,
    /// The hair each player chose for their crew member, in slot order
    /// (feature 62): put onto the crew's looks when the world opens and
    /// whenever it changes ([`Session::dress_crew`]). Not saved — the
    /// look is on the character, and the save carries that.
    pub crew_hair: Vec<(Hair, Shade)>,
    /// The colour each player chose to have their own Bim ringed in, in
    /// slot order (feature 84, `bims::character::Tint`): put onto the
    /// crew beside the hair ([`Session::dress_crew`]). Not saved — the
    /// tint is on the character, and the save carries that.
    pub crew_tints: Vec<Tint>,
    /// The class each player chose for their crew member, in slot order
    /// (feature 74, `world::class`): put onto the world when it opens
    /// (`Session::start_game`, through `World::set_class`) and, in the
    /// design phase, what the pool is worked out from
    /// ([`Session::set_class`]). Nothing past the players; a slot not
    /// said is `Class::None`. Playing, the world's `classes` are the
    /// truth and a change is `Command::SetClass`.
    pub crew_classes: Vec<Class>,
    list: DrawList,
}

impl Session {
    /// Open a design phase.
    ///
    /// `build_area` is tiles a side and `money_per_bim` is what each of the
    /// crew brings, in whole euros. The pool is `economy::starting_pool` of
    /// that and `players`, worked out in [`Editor::new`] so that this and a
    /// native server arrive at it the same way.
    ///
    /// `seed` and `galaxy` are the world the game will open in, and `spawn`
    /// is where in it. The spawn deliberately has **no default**: `None` is
    /// a screen that says so and a way back to the lobby, never a game
    /// somewhere else. [`Session::spawn_ok`] is how the app finds out.
    #[allow(clippy::too_many_arguments)]
    pub fn design(
        build_area: u32,
        money_per_bim: Money,
        players: u32,
        local_slot: u32,
        seed: u64,
        galaxy: u32,
        spawn: Option<(u32, u32)>,
        preset: Preset,
        width: f32,
        height: f32,
    ) -> Session {
        let mut editor = Editor::new(
            build_area,
            money_per_bim,
            players,
            local_slot,
            width,
            height,
        );
        // The spawn's shelf and its desk, before the gift: the gift is
        // valued at the desk. The desk is the kind's with **no local
        // lean** — the start station's roll is forced to nothing, here and
        // in `World::start`, so an opening pool buys the same at a kind of
        // station whatever the seed rolled. The spawn is laid out as a hub
        // there, which is what `market_kind` is asked with.
        let docked = spawn.and_then(|(star, station)| {
            worldgen::Galaxy::new(seed, galaxy_type(galaxy))
                .system(star)
                .and_then(|system| system.station(station).map(|s| (s.stock, s.kind)))
        });
        if let Some((stock, kind)) = docked {
            let desk = world::station::market_kind(kind, world::station::Plan::Hub)
                .map(|kind| Market::new(kind, Bias::NONE))
                .unwrap_or(Market::PLAIN);
            editor.dock_at(stock, desk);
        }
        // The gift: the playtest ship for one, and for a crew of more the
        // combat ship — the same hull with bunks and chairs for five — since
        // the playtest ship sleeps and seats one, and a lobby's yard that
        // opened with a fault before anybody laid a tile would open on
        // "too few bunks" every time.
        let given = if players > 1 {
            shipdesign::fixture::combat_ship_on(build_area)
        } else {
            shipdesign::fixture::playtest_ship_on(build_area)
        };
        if preset == Preset::Playtest
            && let Some(given) = given
        {
            editor.give(given);
        }
        Session {
            editor,
            game: None,
            seed,
            galaxy,
            spawn,
            crew_names: Vec::new(),
            crew_hair: Vec::new(),
            crew_tints: Vec::new(),
            crew_classes: Vec::new(),
            list: DrawList::new(),
        }
    }

    /// Skip the design phase and open the world on the playtest ship: what
    /// the `simulation` command is.
    ///
    /// One player, slot 0, [`shipdesign::fixture::playtest_ship`] already
    /// settled, [`world::data::SIMULATION_MONEY`] in hand. The spawn is the
    /// one given when there is one and the simulation's default when there
    /// is not — the lowest star with a station, which is the one place
    /// [`world::spawn`] is still used. Every other number is fixed on
    /// purpose: a playtest that opened somewhere different each time would
    /// be a playtest of nothing.
    pub fn simulate(
        seed: u64,
        galaxy: u32,
        spawn: Option<(u32, u32)>,
        width: f32,
        height: f32,
    ) -> Session {
        let design = shipdesign::fixture::playtest_ship();
        Session::simulate_on(design, 1, seed, galaxy, spawn, width, height)
    }

    /// [`Session::simulate`] on `design` with `crew` aboard, one of them the
    /// player: the `test` command opens on the combat ship with one crew
    /// member this way, so its spare bunks can take a mercenary hired at
    /// the dock.
    pub fn simulate_on(
        design: ShipDesign,
        crew: u32,
        seed: u64,
        galaxy: u32,
        spawn: Option<(u32, u32)>,
        width: f32,
        height: f32,
    ) -> Session {
        let spawn =
            spawn.or_else(|| world::spawn(&worldgen::Galaxy::new(seed, galaxy_type(galaxy))));
        let editor = Editor::settled(design.clone(), 1, 0, width, height);
        let game = spawn.and_then(|(star, station)| {
            Game::start_with_crew(
                design,
                world::data::SIMULATION_MONEY,
                1,
                crew,
                0,
                seed,
                galaxy_type(galaxy),
                star,
                station,
                width,
                height,
            )
        });
        let mut session = Session {
            editor,
            game,
            seed,
            galaxy,
            spawn,
            crew_names: Vec::new(),
            crew_hair: Vec::new(),
            crew_tints: Vec::new(),
            crew_classes: Vec::new(),
            list: DrawList::new(),
        };
        // Nobody went through the setup on this one, so the player's own
        // Bim has the colour its slot deals it (feature 84); the hair is
        // the look the index dealt, which is what it always was.
        session.dress_crew();
        session
    }

    /// A run (feature 102): what the `game` command's setup or lobby
    /// opens once Start is pressed — **no design phase**. The world opens
    /// straight away, docked at the station the lobby picked, on the
    /// **default ship** — the playtest ship the `simulation` command
    /// flies ([`shipdesign::fixture::playtest_ship`]) — with the crew's
    /// shared pool at `money_per_bim` a player's Bim
    /// ([`world::data::START_MONEY_PER_BIM`] unless the lobby said
    /// otherwise), nothing added for a crew of one. `classes` are the
    /// players' choices, slot by slot, put on as the world opens.
    ///
    /// Every machine of a lobby stands the same session up from the same
    /// numbers, which is what made the design phase's last Accept open
    /// one world on all of them; with no Accept the numbers alone do.
    /// With no spawn — or one the galaxy has not got — there is no game,
    /// and [`Session::spawn_ok`] says so for the screen that says so.
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        money_per_bim: Money,
        players: u32,
        local_slot: u32,
        seed: u64,
        galaxy: u32,
        spawn: Option<(u32, u32)>,
        classes: &[Class],
        width: f32,
        height: f32,
    ) -> Session {
        let players = players.max(1);
        let local_slot = local_slot.min(players - 1);
        let design = shipdesign::fixture::playtest_ship();
        let editor = Editor::settled(design.clone(), players, local_slot, width, height);
        let money = money_per_bim.saturating_mul(Money::from(players));
        let game = spawn.and_then(|(star, station)| {
            Game::start(
                design,
                money,
                players,
                local_slot,
                seed,
                galaxy_type(galaxy),
                star,
                station,
                width,
                height,
            )
        });
        let mut session = Session {
            editor,
            game,
            seed,
            galaxy,
            spawn,
            crew_names: Vec::new(),
            crew_hair: Vec::new(),
            crew_tints: Vec::new(),
            crew_classes: classes.iter().copied().take(players as usize).collect(),
            list: DrawList::new(),
        };
        session.class_crew();
        session.dress_crew();
        session
    }

    /// The fight's ship, crew and arena, before the machines have the
    /// arena: what [`Session::droids`] is built on. It was the `combat`
    /// command's whole session — the arena's own people turned against
    /// the crew — until every enemy was a machine (feature 102) and the
    /// command went. The simulation's spawn for `seed`,
    /// on the combat ship (`shipdesign::fixture::combat_ship`) with a crew
    /// of `COMBAT_CREW` (sixteen: five at the bunks, eleven on the deck) —
    /// the first the player, the rest crew nobody steers — a gun in every
    /// hand, `WeaponKind::ALL`'s order (pistol, shotgun, auto rifle, sniper
    /// rifle, schword) dealt down the crew and round again, docked at the
    /// spawn rebuilt as the arena (`World::arena_dock_for_probe`) and that
    /// made hostile: its people enemies, `ARENA_GARRISON` (sixteen) of
    /// them. Nothing is recruited: whom to send in is the player's.
    pub fn combat(seed: u64, width: f32, height: f32) -> Session {
        use bims::combat::WeaponKind;
        use shipdesign::fixture::{COMBAT_CREW, combat_ship};
        let galaxy = 0;
        let spawn = world::spawn(&worldgen::Galaxy::new(seed, galaxy_type(galaxy)));
        let design = combat_ship();
        let editor = Editor::settled(design.clone(), 1, 0, width, height);
        let game = spawn.and_then(|(star, station)| {
            let mut game = Game::start_with_crew(
                design,
                world::data::SIMULATION_MONEY,
                1,
                COMBAT_CREW,
                0,
                seed,
                galaxy_type(galaxy),
                star,
                station,
                width,
                height,
            )?;
            game.world.arena_dock_for_probe();
            let room = &mut game.world.aboard.room;
            let kinds = WeaponKind::ALL.iter().copied().cycle();
            for (who, kind) in kinds.take(room.crew_count() as usize).enumerate() {
                let gear = room.gear(who);
                room.issue(
                    who,
                    bims::combat::Gear {
                        weapon: Some(kind.basic()),
                        ..gear
                    },
                );
            }
            Some(game)
        });
        let mut session = Session {
            editor,
            game,
            seed,
            galaxy,
            spawn,
            crew_names: Vec::new(),
            crew_hair: Vec::new(),
            crew_tints: Vec::new(),
            crew_classes: Vec::new(),
            list: DrawList::new(),
        };
        session.make_dock_hostile();
        // And **four hired field medics** at the back of the crew
        // (feature 86, `COMBAT_MEDICS`): the last four of the sixteen,
        // on a contract that costs nothing, with a medic's charges of
        // medicine. The fight is where a body goes down, and without a
        // medic in it nobody ever carries one off the deck — so the test
        // fight has the medics that make that half of the game happen at
        // all. They are the *last* four because slot 0 is the player's
        // own and a rescue is a bot's branch (`Game::bot_stand`);
        // `BIMS_FIELD_MEDIC` asks for the same crew members, so a run
        // that sets it over these finds them hired already and changes
        // nothing.
        session.field_medics_for_probe(COMBAT_MEDICS);
        session.dress_crew();
        session
    }

    /// The `tier2_test` and `tier3_test` commands: [`Session::combat`]
    /// with everybody's kit at `tier` — every crew member's gun at it and
    /// a full set of armour at it on, and the garrison's the same
    /// (`World::outfit_for_probe`, after the dock is hostile so the
    /// garrison is the crowd that is dressed). The fight with nothing at
    /// tier one on either side.
    pub fn combat_at_tier(seed: u64, tier: bims::combat::Tier, width: f32, height: f32) -> Session {
        let mut session = Session::combat(seed, width, height);
        if let Some(game) = session.game.as_mut() {
            game.world.outfit_for_probe(tier);
        }
        session
    }

    /// The `tier2_test` and `tier3_test` commands since every enemy is a
    /// machine (feature 102): [`Session::droids`] with the machines at
    /// `tier` and every crew member's gun and a full set of armour at it
    /// too (`World::outfit_for_probe`, which finds no people in a held
    /// station to dress). The machines' fight with nothing at tier one on
    /// either side.
    #[allow(clippy::too_many_arguments)]
    pub fn droids_at_tier(
        seed: u64,
        tier: bims::combat::Tier,
        reinforce: f64,
        wave_max: Option<u32>,
        waves: u32,
        width: f32,
        height: f32,
    ) -> Session {
        let mut session =
            Session::droids(seed, Some(tier), reinforce, wave_max, waves, width, height);
        if let Some(game) = session.game.as_mut() {
            game.world.outfit_for_probe(tier);
        }
        session
    }

    /// The `droids` command (feature 83): [`Session::combat`] with the
    /// arena **droid-held** instead of garrisoned. The ship, the crew and
    /// the guns are `combat`'s own; the arena's people are gone and a
    /// wave of machines stands about it instead, at `tier` — `None`
    /// leaving it to how far the system is from the machines' origin
    /// (feature 93, `World::droid_tier`) — with the reinforcement clock
    /// shortened to `reinforce` minutes so the next wave can be watched
    /// arriving rather than waited two hours for.
    ///
    /// Turning the dock hostile is `combat`'s doing and is left as it is:
    /// a held station is hostile whatever the list says
    /// (`World::stance`), and the garrison it opened with is replaced by
    /// the machines the moment `World::people_of` reads nought for it.
    pub fn droids(
        seed: u64,
        tier: Option<bims::combat::Tier>,
        reinforce: f64,
        wave_max: Option<u32>,
        waves: u32,
        width: f32,
        height: f32,
    ) -> Session {
        let mut session = Session::combat(seed, width, height);
        if let (Some(n), Some(game)) = (wave_max, session.game.as_mut()) {
            game.world.set_droid_wave_for_probe(n);
        }
        session.infest_the_dock_for_probe(tier, reinforce, waves);
        session
    }

    /// The `droids_planet` command: the `test_planet` run — a random
    /// galaxy, a system with friendly ground, the ship set down at the
    /// settlement — with **the town** droid-held. Built by the screen,
    /// which does the landing; this is the infesting half.
    /// Every state a machine can be drawn in, laid out on the deck for
    /// one picture — see `World::stage_droids_for_probe`. What
    /// `BIMS_DROIDS=1` does.
    pub fn stage_droids_for_probe(&mut self) -> bool {
        self.game
            .as_mut()
            .is_some_and(|g| g.world.stage_droids_for_probe())
    }

    /// The `crisis` command (feature 92): this world put a day short of
    /// the crisis's first spread, with the origin forced [`CRISIS_HOPS`]
    /// hyperlane hops from the crew's own star.
    ///
    /// Both halves are the point. The origin is rolled at least
    /// `DROID_ORIGIN_MIN_HOPS` (eight) away, which is forty days of the
    /// clock before the crisis is anywhere near the crew; two hops is ten,
    /// and the chart shows the whole of it spreading rather than one red
    /// star on the far rim. The origin is the machines' from `first_day`
    /// — nought in every run, since feature 102 the crisis is there from
    /// the start — and the clock opens on the day *before* the first
    /// ring round it turns, `first_day` and `DROID_SPREAD_DAYS` on, so
    /// the origin is red on the chart and the stars next to it turn
    /// within a day of the clock — a real minute at 24× — rather than
    /// five. The crew's own system follows five days after that.
    ///
    /// `false` in the design phase, or where the lanes are too short for
    /// the hop count, which no real galaxy is.
    pub fn crisis_for_probe(&mut self, first_day: u32) -> bool {
        let Some(game) = self.game.as_mut() else {
            return false;
        };
        let world = &mut game.world;
        let hops = world.start_star_hops_for_probe();
        // Exactly that many hops off, else the furthest short of it: a
        // galaxy that cannot manage two is not one this will be run on,
        // but it must open rather than panic.
        let pick = hops.iter().position(|&h| h == CRISIS_HOPS).or_else(|| {
            hops.iter()
                .enumerate()
                .filter(|&(_, &h)| h != u16::MAX && h > 0)
                .max_by_key(|&(_, &h)| h)
                .map(|(i, _)| i)
        });
        let Some(origin) = pick else {
            return false;
        };
        world.set_crisis_first_day_for_probe(first_day);
        world.set_droid_origin_for_probe(origin as u32);
        // A day short of the first ring past the origin (feature 102: the
        // origin itself is theirs from the day it turns, which in a run is
        // the day the world opens).
        let ring = first_day.saturating_add(world::data::DROID_SPREAD_DAYS);
        world.set_day_for_probe(ring.saturating_sub(1));
        true
    }

    /// The `jammer` command (feature 93): the crew **in** an infested
    /// system [`CRISIS_HOPS`] hops from the machines' origin, its jammer
    /// standing and one wave of machines aboard the station they are tied
    /// up at.
    ///
    /// It is `crisis_for_probe`'s origin with the clock wound past the
    /// day this system falls rather than a day short of the first — so
    /// the chart is red here and the lanes inward are shut — and then
    /// every station of the system into the machines' hands at once
    /// (`World::infest_here_for_probe`), since the crisis's own flip
    /// waits for the crew to be off the berth and the probe wants them on
    /// one. Two hops is also inside
    /// [`world::data::DROID_TIER_THREE_HOPS`], so the wave comes at tier
    /// three unless `tier` says otherwise.
    ///
    /// `false` in the design phase, or where the lanes are too short for
    /// the hop count.
    pub fn jammer_for_probe(
        &mut self,
        tier: Option<bims::combat::Tier>,
        reinforce: f64,
        waves: u32,
    ) -> bool {
        // The crisis from day nought, as in every run (feature 102).
        if !self.crisis_for_probe(0) {
            return false;
        }
        let Some(game) = self.game.as_mut() else {
            return false;
        };
        let world = &mut game.world;
        // The day this system's own flip is due, and one more so it is
        // certainly past: `infested` is "the day has come".
        let due = world.infested_on(world.star_id);
        world.set_day_for_probe(due.saturating_add(1));
        world.set_droid_tier_for_probe(tier);
        world.set_droid_reinforce_minutes_for_probe(reinforce);
        // Before `infest_here_for_probe`, since a station's wave count is
        // fixed at the crew's first dock and never worked out again.
        world.set_droid_waves_for_probe(waves);
        world.infest_here_for_probe();
        true
    }

    /// The `defense` command (feature 94): the `test_planet` run — a
    /// random galaxy, a system with friendly ground, the ship set down at
    /// the town — with **the town threatened**, so the machines come for
    /// it while the crew are standing in it.
    ///
    /// Threatened is `World::front(star) == Some(1)`: the machines hold
    /// the star next door and this one is next. So the crisis's first
    /// day is wound to nought and the origin forced **one** hyperlane hop
    /// off — where the roll's own floor is eight, and where `crisis`'s
    /// own two would be a system merely near the front rather than on it.
    /// The system's own flip is then `DROID_SPREAD_DAYS` (five days of
    /// the clock) away, which is plenty of room for a fight.
    ///
    /// Both clocks are the caller's, and the command cuts both to a
    /// minute: the first wave lands a minute after the landing rather
    /// than an hour, and the next a minute after the last of one is
    /// destroyed.
    ///
    /// Called **after** `land_for_probe`, since what starts an attack is
    /// the crew being on the pad at a threatened town.
    pub fn defense_for_probe(&mut self, delay: f64, reinforce: f64, waves: u32) -> bool {
        let Some(game) = self.game.as_mut() else {
            return false;
        };
        let world = &mut game.world;
        // A star exactly one hop off, else the nearest there is.
        let hops = world.start_star_hops_for_probe();
        let pick = hops.iter().position(|&h| h == 1).or_else(|| {
            hops.iter()
                .enumerate()
                .filter(|&(_, &h)| h != u16::MAX && h > 0)
                .min_by_key(|&(_, &h)| h)
                .map(|(i, _)| i)
        });
        let Some(origin) = pick else {
            return false;
        };
        world.set_crisis_first_day_for_probe(0);
        world.set_droid_origin_for_probe(origin as u32);
        world.set_defense_delay_for_probe(delay);
        world.set_droid_reinforce_minutes_for_probe(reinforce);
        world.set_droid_waves_for_probe(waves);
        true
    }

    pub fn infest_the_dock_for_probe(
        &mut self,
        tier: Option<bims::combat::Tier>,
        reinforce: f64,
        waves: u32,
    ) -> bool {
        let Some(game) = self.game.as_mut() else {
            return false;
        };
        let Some(id) = game.world.ship.state.station() else {
            return false;
        };
        game.world.set_droid_tier_for_probe(tier);
        game.world.set_droid_reinforce_minutes_for_probe(reinforce);
        // Before `infest`, and before the first step: the count is fixed
        // at the crew's first dock and never worked out again.
        game.world.set_droid_waves_for_probe(waves);
        game.world.infest(id);
        // The room the crew are docked at was opened with the station's
        // own people: it is reopened with the machines the next step,
        // since `people_of` is nought for a held station now. Stepping
        // once here would move the world before the first frame, so the
        // wave is laid by the first step the screen takes.
        true
    }

    /// A session round a game read back from a save — see `crate::save`.
    pub(crate) fn resumed(
        editor: Editor,
        game: Game,
        seed: u64,
        galaxy: u32,
        spawn: Option<(u32, u32)>,
    ) -> Session {
        // No `dress_crew` here, deliberately: the hair and the colour
        // (feature 84) are on the character and the save carries both,
        // so dealing them again would throw away what was chosen.
        Session {
            editor,
            game: Some(game),
            seed,
            galaxy,
            spawn,
            crew_names: Vec::new(),
            crew_hair: Vec::new(),
            crew_tints: Vec::new(),
            crew_classes: Vec::new(),
            list: DrawList::new(),
        }
    }

    /// Whether the spawn the session was given is a station this galaxy
    /// has: both halves present, the star in the galaxy, the station in its
    /// system. Asked once, before a design phase is shown — a player who
    /// laid out a ship for an hour and then learnt at Accept that there was
    /// nowhere to put it would be right to be cross.
    pub fn spawn_ok(&self) -> bool {
        let Some((star, station)) = self.spawn else {
            return false;
        };
        worldgen::Galaxy::new(self.seed, galaxy_type(self.galaxy))
            .system(star)
            .is_some_and(|system| system.station(station).is_some())
    }

    /// The ship set down on the system's first planet with ground, without
    /// the descent — see `World::land_for_probe`. What `BIMS_LANDED=1` does.
    pub fn land_for_probe(&mut self) -> bool {
        self.game.as_mut().is_some_and(|g| g.world.land_for_probe())
    }

    /// The ship over the system's first planet with ground, its landing
    /// just begun — see `World::landing_for_probe`. What `BIMS_LANDING=1` does.
    pub fn landing_for_probe(&mut self, done: f64) -> bool {
        self.game
            .as_mut()
            .is_some_and(|g| g.world.landing_for_probe(done))
    }

    /// A mercenary for hire at the dock whatever the roll said — see
    /// `World::mercenary_for_probe`. What the `test` command does.
    pub fn mercenary_for_probe(&mut self) -> bool {
        self.game
            .as_mut()
            .is_some_and(|g| g.world.mercenary_for_probe())
    }

    /// Landed, walk the crew member out onto the plain thirty tiles west
    /// of the ship and give it a minute to get there, with its errands
    /// off so nothing calls it back. What `BIMS_AFIELD=1` does; false
    /// with no world, or no plain under it.
    pub fn walk_afield_for_probe(&mut self) -> bool {
        let Some(game) = self.game.as_mut() else {
            return false;
        };
        if game.world.aboard.room.plane().is_none() {
            return false;
        }
        let ship = &game.world.ship.design;
        let west = ship
            .parts
            .iter()
            .flat_map(|p| p.tiles())
            .map(|(x, _)| x)
            .min()
            .unwrap_or(0) as f32;
        let mid = ship.build_area as f32 / 2.0;
        let t = TILE as f32;
        let offset = game.world.aboard.offset;
        let to = bims::math::vec2(
            (west - 30.0) * t + offset.x as f32,
            (mid - 20.0) * t + offset.y as f32,
        );
        game.world.aboard.room.set_autonomous(false);
        if !game.world.aboard.room.walk_to(0, to) {
            return false;
        }
        for _ in 0..3600 {
            game.world.step(&[]);
        }
        true
    }

    /// Stage a fight at the dock — see `World::stage_fight_for_probe`.
    pub fn stage_fight_for_probe(&mut self) -> bool {
        self.game
            .as_mut()
            .is_some_and(|g| g.world.stage_fight_for_probe())
    }

    /// `n` of the station alongside dead where they stand, the room
    /// built again over the bodies — see `World::lay_graves_for_probe`.
    pub fn lay_graves_for_probe(&mut self, n: u32) -> bool {
        self.game
            .as_mut()
            .is_some_and(|g| g.world.lay_graves_for_probe(n))
    }

    /// A raid: contact made, and with `dock` the raider tied to the ship
    /// and its boarders on their way — see `World::raid_for_probe`; what
    /// it said goes into the log.
    pub fn raid_for_probe(&mut self, dock: bool) -> bool {
        let Some(game) = self.game.as_mut() else {
            return false;
        };
        match game.world.raid_for_probe(dock) {
            Some(events) => {
                game.events.extend(events);
                true
            }
            None => false,
        }
    }

    /// A raid on its way: the ship off its berth holding in open space
    /// and the next raid due `minutes` of the clock from now — see
    /// `World::raid_coming_for_probe`. The `raid` command.
    pub fn raid_coming_for_probe(&mut self, minutes: u64) -> bool {
        self.game
            .as_mut()
            .is_some_and(|g| g.world.raid_coming_for_probe(minutes))
    }

    /// Everybody of the crew shot where they stand — the end of the run,
    /// for looking at the screen that says so (`BIMS_LOST=1`).
    pub fn lose_for_probe(&mut self) {
        if let Some(game) = self.game.as_mut() {
            for who in 0..game.world.aboard.crew_count() as usize {
                game.world.aboard.room.kill_for_probe(who);
            }
        }
    }

    /// The first `n` of the crew put into a **dying state**: a part of
    /// each taken to nothing, so its trauma is rolled and untreated, and
    /// a couple of wounds opened besides — the body a red cross stands
    /// over on the deck and the peril block on the panel counts down
    /// (`BIMS_DYING=n`). Slot 0 is left alone unless `n` reaches the
    /// whole crew: the player's own Bim walking about is what the
    /// picture is taken from.
    ///
    /// The part is taken in turn — the legs, the body, the head — so a
    /// crew of three shows three different traumas rather than three of
    /// one, and the damage is far past anything worn, since armour takes
    /// a hit before the body does.
    pub fn maim_for_probe(&mut self, n: usize) {
        let Some(game) = self.game.as_mut() else {
            return;
        };
        let crew = game.world.aboard.crew_count() as usize;
        let parts = [
            bims::health::Part::Legs,
            bims::health::Part::Body,
            bims::health::Part::Head,
        ];
        for i in 0..n.min(crew) {
            let who = if n >= crew { i } else { (i + 1).min(crew - 1) };
            let part = parts[i % parts.len()];
            game.world.aboard.room.wound(who, part, 1000.0);
            game.world.aboard.room.wound(who, part, 1000.0);
        }
    }

    /// The last `n` of the crew made **field medics** (feature 86), for
    /// `BIMS_FIELD_MEDIC=n`: the contract and the two medkits, and
    /// nothing else — the last of the crew rather than the first, since
    /// slot 0 is the player's own and a field medic is a bot's trade.
    /// See `World::field_medic_for_probe`.
    pub fn field_medics_for_probe(&mut self, n: usize) {
        let Some(game) = self.game.as_mut() else {
            return;
        };
        let crew = game.world.aboard.crew_count();
        for i in 0..(n as u32).min(crew.saturating_sub(1)) {
            game.world.field_medic_for_probe(crew - 1 - i);
        }
    }

    /// Exactly `n` dressings in **every** crew member's pack, for
    /// `BIMS_BANDAGES=n`, with the bandage cooldown started afresh — a
    /// dressing is everybody's charge, so `BIMS_BANDAGES=0` is the whole
    /// wait ahead and the sweep over the bandage box at the foot of the
    /// canvas, and `BIMS_BANDAGES=2` a part box with the next on its way.
    /// A box holds five, so `BIMS_BANDAGES=7` is a full box beside a
    /// part one (the cooldown brings nothing over the charges).
    pub fn bandages_for_probe(&mut self, n: u32) {
        if let Some(game) = self.game.as_mut() {
            game.world.set_charges_for_probe(world::Charge::Bandage, n);
        }
    }

    /// Exactly `n` medkits in every crew member's pack, for
    /// `BIMS_MEDKITS=n`, the medkit cooldown started afresh the same way.
    pub fn medkits_for_probe(&mut self, n: u32) {
        if let Some(game) = self.game.as_mut() {
            game.world.set_charges_for_probe(world::Charge::Medkit, n);
        }
    }

    /// Exactly `n` of each of the engineer's two kits in every pack
    /// (feature 88), for `BIMS_KITS=n`, with the cooldowns started afresh
    /// — `BIMS_KITS=0` is the one state a scripted run cannot walk itself
    /// into: no charge in hand and the whole wait ahead, which is what
    /// puts the seconds in the corner of the two boxes.
    pub fn kits_for_probe(&mut self, n: u32) {
        if let Some(game) = self.game.as_mut() {
            game.world.set_kits_for_probe(n);
        }
    }

    /// Exactly `n` grenade charges in every pack (feature 90), for
    /// `BIMS_GRENADES=n`, the cooldown started afresh the same way.
    pub fn grenades_for_probe(&mut self, n: u32) {
        if let Some(game) = self.game.as_mut() {
            game.world.set_grenades_for_probe(n);
        }
    }

    /// Crew member 1 taken out cold and carried by the field medic —
    /// `BIMS_CARRY=1` — for looking at a body in somebody's arms. The
    /// carrier is the last of the crew, which is where
    /// `field_medics_for_probe` puts one.
    pub fn carry_for_probe(&mut self) -> bool {
        let Some(game) = self.game.as_mut() else {
            return false;
        };
        let crew = game.world.aboard.crew_count();
        if crew < 2 {
            return false;
        }
        let carrier = (0..crew)
            .find(|&who| game.world.can_lift(who))
            .unwrap_or(crew - 1);
        let patient = if carrier == 1 { 0 } else { 1 };
        game.world.carry_for_probe(carrier, patient)
    }

    /// Shoot the `n` lamps nearest the crew member out, and leave the
    /// next failing — see `World::shoot_lamps_for_probe`.
    pub fn shoot_lamps_for_probe(&mut self, n: usize) {
        if let Some(game) = self.game.as_mut() {
            game.world.shoot_lamps_for_probe(n);
        }
    }

    /// Make the station the ship is tied to an enemy's: what the `combat`
    /// command does to the simulation's dock. False with no world, or
    /// away from a berth.
    pub fn make_dock_hostile(&mut self) -> bool {
        let Some(game) = &mut self.game else {
            return false;
        };
        let Some(station) = game.world.ship.state.station() else {
            return false;
        };
        game.world.set_hostile(station, true);
        true
    }

    /// The simulation's spawn for the seed and type the session opened
    /// with: the lowest star with a station, and the lowest station in it.
    pub fn simulation_spawn(&self) -> Option<(u32, u32)> {
        world::spawn(&worldgen::Galaxy::new(self.seed, galaxy_type(self.galaxy)))
    }

    /// Open the world with the accepted design. Called the moment the last
    /// Accept lands and at no other time.
    ///
    /// With no spawn there is no world: the app checked [`Session::spawn_ok`]
    /// at the start and showed the error screen instead of a design phase,
    /// so this is only reached without one if something went round that
    /// check — and then the honest outcome is still no game rather than a
    /// game somewhere else.
    fn start_game(&mut self) {
        if self.game.is_some() {
            return;
        }
        let Some((star, station)) = self.spawn else {
            return;
        };
        let Some(design) = self.editor.finish_design().cloned() else {
            return;
        };
        let money = self.editor.budget.remaining(&self.editor.design);
        self.game = Game::start(
            design,
            money,
            self.editor.players,
            self.editor.local,
            self.seed,
            galaxy_type(self.galaxy),
            star,
            station,
            self.editor.view.width,
            self.editor.view.height,
        );
        self.dress_crew();
        self.class_crew();
    }

    /// Put the classes the players chose onto the world as it opens
    /// (`crew_classes`): slot *i* gets what was said for it, through the
    /// same `World::set_class` a `Command::SetClass` goes through, so an
    /// engineer's kits are in its pack from the first step. Nothing
    /// before the world opens.
    fn class_crew(&mut self) {
        let Some(game) = &mut self.game else {
            return;
        };
        for (slot, &class) in self.crew_classes.iter().enumerate() {
            let _ = game.world.set_class(slot as u32, class);
        }
    }

    /// A player's class, chosen in the design phase: kept for the world to
    /// open with. The pool is untouched — every Bim brings the same money
    /// whatever its class (feature 75: a class owns abilities, never
    /// money). Playing, a change is `Command::SetClass` instead, and this
    /// does nothing. Whether anything changed.
    pub fn set_class(&mut self, slot: u32, class: Class) -> bool {
        if self.game.is_some() || slot >= self.editor.players {
            return false;
        }
        let slot = slot as usize;
        if self.crew_classes.len() <= slot {
            self.crew_classes.resize(slot + 1, Class::None);
        }
        if self.crew_classes[slot] == class {
            return false;
        }
        self.crew_classes[slot] = class;
        true
    }

    /// A player's class: the world's while playing, else what was chosen.
    pub fn class_of(&self, slot: u32) -> Class {
        match &self.game {
            Some(game) => game.world.class_of(slot),
            None => self
                .crew_classes
                .get(slot as usize)
                .copied()
                .unwrap_or_default(),
        }
    }

    /// Put the hair the players chose onto their crew members
    /// (`crew_hair`, feature 62): slot *i*'s Bim gets its dealt look
    /// (`Look::of`) with the hair said for slot *i*, for as many slots as
    /// have said and are players — a bot, a hire, a resident keeps what
    /// the index dealt it. Nothing before the world opens; called at
    /// `start_game` and by the app whenever a choice arrives late.
    pub fn dress_crew(&mut self) {
        let players = self.editor.players as usize;
        let Some(game) = &mut self.game else {
            return;
        };
        let room = &mut game.world.aboard.room;
        let crew = room.crew_count() as usize;
        for (slot, &(hair, shade)) in self.crew_hair.iter().enumerate().take(players.min(crew)) {
            room.set_look(slot, Look::of(slot).with_hair(hair, shade));
        }
        // And the ring under each player's own (feature 84): slot *i*'s
        // colour, or the *i*th of `Tint::ALL` for a slot that has not
        // said, so every player's Bim is marked and no two share one
        // whatever the lobby did. Everybody past the players — the bots,
        // the hires — is left with none, which is what draws no circle.
        let tints: Vec<Tint> = (0..players)
            .map(|slot| {
                self.crew_tints
                    .get(slot)
                    .copied()
                    .unwrap_or(Tint::ALL[slot % Tint::ALL.len()])
            })
            .collect();
        room.set_tints(&tints);
    }

    /// The live ship: the game's if there is one, the design being laid out
    /// if there is not.
    pub fn design_ref(&self) -> &ShipDesign {
        match &self.game {
            Some(game) => &game.world.ship.design,
            None => &self.editor.design,
        }
    }

    pub fn resize(&mut self, width: f32, height: f32) {
        self.editor.view.resize(width, height);
        if let Some(game) = &mut self.game {
            game.resize(width, height);
        }
    }

    /// Resize, and start the views again from that size — the whole build
    /// area, the whole hull, everything found. What a host does the first
    /// time it knows how big its canvas really is.
    pub fn fit(&mut self, width: f32, height: f32) {
        self.editor.view.fit(width, height);
        if let Some(game) = &mut self.game {
            game.fit(width, height);
        }
    }

    /// Age the fight's passing lights — the muzzles, the flashes, the
    /// scorches, the beams, the cuts, a machine bursting (feature 98,
    /// `bims::fx`) — by `real` seconds of the window's own frame, and
    /// switch them on, in the room aboard and the station's alike. Nought
    /// while the game is paused, so a paused frame is a still picture.
    /// Drawing only: nothing the world reads, and nothing a guest has to
    /// agree with the host about.
    pub fn age_effects(&mut self, real: f32) {
        if let Some(game) = &mut self.game {
            game.world.aboard.room.fade(real);
            if let Some(residents) = &mut game.world.residents {
                residents.aboard.room.fade(real);
            }
        }
    }

    /// Rebuild the shape buffer and hand it back.
    ///
    /// A redraw and **never a step**: the world is advanced by
    /// [`Session::world_step`], which the app calls however many times a
    /// frame is worth. Folding the two together would tie the simulation to
    /// the display's refresh rate, which is the one thing a fixed step
    /// exists to avoid.
    pub fn render(&mut self) -> &[f32] {
        match &mut self.game {
            Some(game) => {
                // The room aboard draws itself once a frame, here, and not
                // once a step: at 24x that is one picture rather than
                // twenty-four. Through this player's eyes: the selection
                // ring is theirs (`bims::order`).
                game.world.aboard.room.set_viewer(game.local);
                game.world.aboard.render();
                // And the station's room, whose people are always in it now:
                // drawn once a frame the same way, or they stand in the
                // picture at their bunks while their names walk about.
                if let Some(residents) = &mut game.world.residents {
                    residents.aboard.render();
                }
                game.frame = game.frame.wrapping_add(1);
                game.tick_airlock();
                game.stream_sky();
                game.follow_player();
                game.hold_view_to_the_ground();
                game.picture_the_plain();
                // Whether the blueprint in hand would go where the pointer
                // is, asked before the painter colours it.
                game.ghost_check();
                world_paint::paint(game, &mut self.list);
            }
            None => paint::paint(&self.editor, &mut self.list),
        }
        self.list.shapes()
    }

    /// The buffer [`Session::render`] last built, cut where the host's
    /// smooth fog goes: what the fog lies over, and what is drawn over it
    /// — the shots, which are always seen, the rings and the overlays. The
    /// second half is empty for a picture with no fog in it: the yard, the
    /// map.
    pub fn fog_split(&self) -> (&[f32], &[f32]) {
        self.list.fog_split()
    }

    // --- the camera -------------------------------------------------------
    //
    // One transform for both halves, so the app has one paint loop rather
    // than two. What changes is what the origin *is*: the corner of the
    // build area during the design phase, and the ship itself once the game
    // has started — see `camera.rs`.

    pub fn view_scale(&self) -> f32 {
        match &self.game {
            Some(game) => game.camera().scale(),
            None => self.editor.view.scale(),
        }
    }

    pub fn view_offset(&self) -> (f32, f32) {
        match &self.game {
            Some(game) => (game.camera().offset_x(), game.camera().offset_y()),
            None => (self.editor.view.offset_x(), self.editor.view.offset_y()),
        }
    }

    /// Shove the view by a screen-pixel delta. Middle-drag and WASD both
    /// come through here.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        match &mut self.game {
            Some(game) => game.pan(dx, dy),
            None => self.editor.view.pan(dx, dy),
        }
    }

    /// Zoom about a point on the canvas by a multiplier.
    pub fn zoom(&mut self, at_x: f32, at_y: f32, factor: f32) {
        match &mut self.game {
            Some(game) => game.zoom(at_x, at_y, factor),
            None => self.editor.view.zoom(at_x, at_y, factor),
        }
    }

    // --- the palette ------------------------------------------------------

    /// Tiles across and down, at the ghost's current rotation — so a
    /// palette row can show what it is about to put down rather than what
    /// it would be upright.
    pub fn part_size(&self, kind: PartKind) -> (u32, u32) {
        footprint(kind, self.editor.ghost)
    }

    /// The colour the part is drawn in. The palette swatches are painted
    /// with these, so a button cannot end up a different colour from the
    /// thing it places.
    pub fn part_color(kind: PartKind) -> Color {
        paint::PART_COLORS[kind as usize]
    }

    /// Whether the pointer is over the build area at all.
    pub fn hover_inside(&self) -> bool {
        self.editor
            .hover
            .is_some_and(|t| self.editor.design.holds(t))
    }

    /// The kind of a part by id, off the **live** ship — the design being
    /// laid out, or the one flying — so the game's readout can name what
    /// the pointer is over the same way the designer's does.
    pub fn part_kind(&self, part_id: u32) -> Option<PartKind> {
        self.design_ref().part(part_id).map(|p| p.kind)
    }

    // --- the money --------------------------------------------------------

    /// What is left of the pool.
    ///
    /// Never negative; `apply` refuses anything that would take it there,
    /// and during the design phase it is **derived from the design** every
    /// time rather than decremented as parts go down.
    ///
    /// Once the game has started it is the world's figure instead — the
    /// same money, carried across at Accept and not converted into
    /// anything, and now spent and earned at stations rather than derived
    /// from a ship.
    pub fn remaining(&self) -> Money {
        match &self.game {
            Some(game) => game.world.money,
            None => self.editor.budget.remaining(&self.editor.design),
        }
    }

    // --- the station's goods, and the hold --------------------------------

    /// Whether the station the ship is at sells `resource`: the design
    /// phase's spawn station, or the one the ship is docked at — and false
    /// anywhere else, since there is nobody to buy from. Playing, and only
    /// what a run may buy at all (`World::buyable`, feature 102): gear.
    pub fn sold_here(&self, resource: ResourceId) -> bool {
        match &self.game {
            Some(g) => match g.world.ship.state {
                world::ShipState::Docked { station } => g
                    .world
                    .station(station)
                    .is_some_and(|s| s.stock.sells(resource) && g.world.buyable(resource)),
                _ => false,
            },
            None => self.editor.sells(resource),
        }
    }

    /// What the desk here quotes for one unit of `resource` — what one
    /// costs bought and what one fetches sold — at the design phase's
    /// spawn station, or the station the ship is docked at. `None`
    /// anywhere else, and at a derelict, which keeps no desk: the panels
    /// show a dash. The rule is `World::quote` — the one place a price
    /// is worked out, front premium and all (feature 94); this only asks.
    pub fn quote(&self, resource: ResourceId) -> Option<Quote> {
        match &self.game {
            Some(g) => match g.world.ship.state {
                world::ShipState::Docked { station } => g.world.quote(station, resource),
                _ => None,
            },
            None => self
                .editor
                .market
                .map(|_| self.editor.budget.market.quote(resource)),
        }
    }

    /// The same at a **tier** (feature 95): the book quote times
    /// `economy::TIER_PRICE` — one, four, sixteen — on both the ask and
    /// the bid, for a gun or a piece of armour; the plain quote for
    /// everything else, which comes at no tier. `World::quote_at` is the
    /// rule while the ship is docked, and the design phase's desk is
    /// asked the same way, so the yard and the world agree.
    pub fn quote_at(&self, resource: ResourceId, tier: u32) -> Option<Quote> {
        match &self.game {
            Some(g) => match g.world.ship.state {
                world::ShipState::Docked { station } => g.world.quote_at(station, resource, tier),
                _ => None,
            },
            None => self.quote(resource).map(|q| {
                if economy::tiered(resource) {
                    q.at_tier(tier)
                } else {
                    q
                }
            }),
        }
    }

    /// Units of it aboard the **live** ship.
    pub fn cargo(&self, resource: ResourceId) -> u32 {
        self.design_ref().carrying(resource)
    }

    pub fn storage_of(resource: ResourceId) -> shipdesign::Storage {
        storage(resource)
    }

    pub fn storage_capacity(&self, class: shipdesign::Storage) -> u32 {
        self.design_ref().capacity(class)
    }

    pub fn storage_used(&self, class: shipdesign::Storage) -> u32 {
        self.design_ref().stored(class)
    }

    // --- accepting --------------------------------------------------------

    /// Record a player's Accept against a hash. `true` if it was taken.
    ///
    /// Refused when the hash is not the design's — an Accept in flight when
    /// somebody else placed a wall is an Accept for a ship that no longer
    /// exists — and refused while there are errors. The last Accept is what
    /// opens the world. There is no separate "start" call: the design phase
    /// ending and the game beginning are one event, and two ways to do it
    /// would be two things that can disagree about which ship got handed
    /// over.
    pub fn accept(&mut self, slot: u32, hash: u64) -> bool {
        let took = self.editor.accept(slot, hash);
        if self.editor.phase == Phase::Game {
            self.start_game();
        }
        took
    }

    /// Whether the ship can still be changed.
    pub fn designing(&self) -> bool {
        self.editor.phase == Phase::Design
    }

    /// Whether the world is open.
    pub fn playing(&self) -> bool {
        !self.designing() && self.game.is_some()
    }

    // --- what the ship is, in either phase --------------------------------

    /// What the ship weighs, crew and cargo included. `0.0` for a design too
    /// light to be a ship — `physics` refuses one rather than quoting an
    /// infinite acceleration.
    pub fn mass(&self) -> f64 {
        match &self.game {
            Some(game) => game.world.ship.dynamics.mass.get(),
            None => paint::debug_mass(&self.editor.design, self.editor.players),
        }
    }

    /// Acceleration along one of the ship's own axes, in world units per
    /// game minute squared.
    pub fn acceleration(&self, axis: Facing) -> f64 {
        let crew = match &self.game {
            Some(game) => game.world.ship.crew_count,
            None => self.editor.players,
        };
        shipdesign::acceleration(self.design_ref(), crew, axis).unwrap_or(0.0)
    }

    // --- the game ---------------------------------------------------------

    /// Advance the world by exactly one step.
    ///
    /// **The app decides how many.** It keeps an accumulator, works out what
    /// a frame is worth at the effective speed, and calls this that many
    /// times. Putting the accumulator in here would give the simulation an
    /// opinion about real time, which it has no way to measure.
    pub fn world_step(&mut self) {
        if let Some(game) = &mut self.game {
            game.step();
        }
    }

    /// How many steps a second of real time is worth at 1x. A fact about
    /// the world rather than about the window, so it comes from here.
    pub fn steps_per_second(&self) -> f64 {
        self.game
            .as_ref()
            .map(|g| g.world.steps_per_second())
            .unwrap_or(time::MINUTES_PER_SECOND / world::data::STEP_MINUTES)
    }

    /// Where a Bim is, in the ship view's camera units about the ship —
    /// what the view offset and scale turn into a canvas pixel, the same way
    /// the shapes are. `None` for a Bim that is not there.
    pub fn crew_on_screen(&self, who: u32) -> Option<(f32, f32)> {
        let game = self.game.as_ref()?;
        if who >= game.world.aboard.crew_count() {
            return None;
        }
        Some(world_paint::crew_on_screen(game, who))
    }

    /// The room's light map and where its corners land in the camera's
    /// units — `world_paint::light_map_on_screen` — for the host to draw
    /// the smooth fog with. `None` before there is a world.
    pub fn light_map(&self) -> Option<(&bims::sight::LightMap, [(f32, f32); 4])> {
        let game = self.game.as_ref()?;
        let corners = world_paint::light_map_on_screen(game)?;
        Some((game.world.aboard.room.light_map()?, corners))
    }

    /// The plain's fog beyond the box, on a planet: a picture a chunk
    /// and the pieces of each to draw, in the camera's units —
    /// `world_paint::plain_fog_on_screen`. Empty anywhere else.
    pub fn plain_fog(
        &self,
    ) -> Vec<(
        (i32, i32),
        &bims::sight::LightMap,
        Vec<world_paint::FogPiece>,
    )> {
        match &self.game {
            Some(game) => world_paint::plain_fog_on_screen(game),
            None => Vec::new(),
        }
    }

    /// The numbers the electricity view puts over the drainers, in the
    /// camera's units — `world_paint::power_labels`. Empty before there is
    /// a world.
    pub fn power_labels(&self) -> Vec<world_paint::PowerLabel> {
        match &self.game {
            Some(game) => world_paint::power_labels(game),
            None => Vec::new(),
        }
    }

    /// Every bunk of the ship's, where its middle lands in the camera's
    /// units and whose it is — `world_paint::bunk_labels`, for the name
    /// the host writes on each. Empty before there is a world.
    pub fn bunk_labels(&self) -> Vec<world_paint::BunkLabel> {
        match &self.game {
            Some(game) => world_paint::bunk_labels(game),
            None => Vec::new(),
        }
    }

    /// How many residents are being simulated. Nought away from any
    /// station, and nought at a derelict. Docked or not, they are in the
    /// station's own room.
    pub fn resident_count(&self) -> u32 {
        let Some(game) = &self.game else {
            return 0;
        };
        game.world
            .residents
            .as_ref()
            .map(|r| r.aboard.count())
            .unwrap_or(0)
    }

    /// What a month of that resident costs if it is a mercenary for hire —
    /// `World::mercenary_fee` — for the `?` over its head.
    pub fn mercenary_fee(&self, who: u32) -> Option<Money> {
        self.game.as_ref()?.world.mercenary_fee(who)
    }

    /// Which station they live on, or `None` for nobody.
    pub fn resident_station(&self) -> Option<u32> {
        let game = self.game.as_ref()?;
        game.world.residents.as_ref().map(|r| r.station)
    }

    /// Which kind of machine that body of a station's room is
    /// (`bims::droid::DroidKind::code`), or `None` for one of its
    /// people: what the app writes over its head instead of a name,
    /// since a droid is a machine and has none (feature 83).
    pub fn resident_droid(&self, who: u32) -> Option<u32> {
        let game = self.game.as_ref()?;
        let room = &game.world.residents.as_ref()?.aboard.room;
        let i = (who as usize).checked_sub(room.crew_count() as usize)?;
        room.droid(i).map(|d| d.kind.code())
    }

    /// Where a resident is, in the ship view's camera units about the ship.
    pub fn resident_on_screen(&self, who: u32) -> Option<(f32, f32)> {
        let game = self.game.as_ref()?;
        if who >= self.resident_count() {
            return None;
        }
        // A name over nobody: a resident the crew cannot see is not drawn,
        // and a name walking about on its own would give them away.
        let drawn = game
            .world
            .residents
            .as_ref()
            .is_some_and(|r| r.aboard.room.body_seen(who as usize));
        if !drawn {
            return None;
        }
        Some(world_paint::resident_on_screen(game, who))
    }

    /// The station the ship is tied to, or `None` for nowhere.
    pub fn docked_at(&self) -> Option<u32> {
        match self.game.as_ref().map(|g| &g.world.ship.state) {
            Some(world::ShipState::Docked { station }) => Some(*station),
            _ => None,
        }
    }

    /// Whether that player's crew member is at the station's trading desk —
    /// `World::at_the_desk`, what a buy or a sell wants beside the berth.
    pub fn at_the_desk(&self, slot: u32) -> bool {
        self.game
            .as_ref()
            .is_some_and(|g| g.world.at_the_desk(slot))
    }

    /// How near the front the desk the ship is tied up at is, in hops —
    /// `None` away from a berth and at a desk too far out for it to
    /// matter (feature 94). What the trade window says a **front
    /// premium** off: the guns, the armour and the medicine here are
    /// dearer than the book, and the player should be told which it is
    /// before they wonder at the price.
    pub fn front_premium(&self) -> Option<u16> {
        let g = self.game.as_ref()?;
        let world::ShipState::Docked { station } = g.world.ship.state else {
            return None;
        };
        g.world.front_at(station)
    }

    // --- the map ----------------------------------------------------------
    //
    // Only what the crew have found. Undiscovered things are not in the list
    // at all rather than being in it and hidden.

    pub fn map_node(&self, i: usize) -> Option<Node> {
        self.game.as_ref()?.world.discovered.get(i).copied()
    }

    /// Where the `i`th thing on the map is, or `None` when the crew have not
    /// found anything with that index.
    pub fn map_position(&self, i: usize) -> Option<worldgen::math::DVec2> {
        let game = self.game.as_ref()?;
        game.world.system.absolute_position(self.map_node(i)?)
    }

    /// What sort of thing the `i`th discovered thing is: a
    /// `worldgen::BodyKind` or `StationKind` discriminant, read against the
    /// app's name table for the node's kind.
    pub fn map_type(&self, node: Node) -> u32 {
        let Some(game) = &self.game else { return 0 };
        match node {
            Node::Body(id) => game
                .world
                .system
                .body(id)
                .map(|b| b.kind as u32)
                .unwrap_or(0),
            Node::Station(id) => game
                .world
                .system
                .station(id)
                .map(|s| s.kind as u32)
                .unwrap_or(0),
        }
    }

    /// Every discovered planet the ship can land on, with whether its
    /// settlement is an enemy's and where the map draws it (the camera's
    /// units about the ship, `Game::map_spot`) — what the app writes a
    /// name and *land* over, so a landable planet is told from the rest
    /// of the map in words as well as by its pad. The rule for which
    /// planets is the world's (`World::surface`); the stance is
    /// `World::stance` of the settlement.
    pub fn landing_sites(&self) -> Vec<LandingSite> {
        let Some(game) = &self.game else {
            return Vec::new();
        };
        game.world
            .discovered
            .iter()
            .filter_map(|&node| {
                let Node::Body(body) = node else { return None };
                let surface = game.world.surface(body)?;
                Some(LandingSite {
                    node,
                    hostile: game.world.stance(surface.id) == bims::sight::Stance::Hostile,
                    threatened: game.world.town_threatened(surface.id),
                    held: game.world.town_held(surface.id),
                    at: game.map_spot(node)?,
                })
            })
            .collect()
    }

    /// Whether the crew are standing in a town the machines are
    /// attacking (feature 94), for the warning along the top.
    pub fn defending_a_town(&self) -> bool {
        self.game
            .as_ref()
            .is_some_and(|g| g.world.defense_here().is_some())
    }

    /// Where a node sits in the discovered list, if it is there.
    pub fn map_index_of(&self, node: Node) -> Option<usize> {
        self.game
            .as_ref()?
            .world
            .discovered
            .iter()
            .position(|&n| n == node)
    }

    /// Quote a trip to a node or a point. Worked out for the **local player
    /// only** and never a command: two players hovering over different
    /// planets must not be an argument about where the ship is going.
    pub fn preview(&mut self, target: Target) {
        if let Some(game) = &mut self.game {
            game.preview(target);
        }
    }

    pub fn clear_preview(&mut self) {
        if let Some(game) = &mut self.game {
            game.clear_preview();
        }
    }

    // --- the two views ----------------------------------------------------

    /// The part under the pointer in the ship view, or `None` — the top of
    /// the tile, the way the designer's `hovered_part` answers: what is
    /// standing there, else the conduit through it, else the deck, else the
    /// frame.
    pub fn game_hovered_part(&self) -> Option<u32> {
        let game = self.game.as_ref()?;
        let tile = game.hover?;
        let grid = game.world.ship.design.grid();
        [
            Layer::Object,
            Layer::Utility,
            Layer::Floor,
            Layer::Structure,
        ]
        .into_iter()
        .map(|layer| grid.get(layer, tile))
        .find(|&id| id != 0)
    }

    /// Whether the pointer is over the hull at all.
    pub fn game_tile_inside(&self) -> bool {
        match self.game.as_ref().and_then(|g| g.hover.map(|t| (g, t))) {
            Some((game, tile)) => game.world.ship.design.holds(tile),
            None => false,
        }
    }

    // --- the room aboard --------------------------------------------------

    /// The room the world is stepping, for the room's panels to act on.
    pub fn room(&mut self) -> Option<&mut bims::game::Game> {
        self.game.as_mut().map(|g| &mut g.world.aboard.room)
    }

    pub fn room_ref(&self) -> Option<&bims::game::Game> {
        self.game.as_ref().map(|g| &g.world.aboard.room)
    }

    /// A canvas point, in the room's coordinates aboard. Only meaningful
    /// once there is a world; nought before.
    ///
    /// Every room coordinate aboard is a design world unit, and this is the
    /// pointer read back through the ship's camera and heading — the same
    /// arithmetic `Game::tile_at` floors.
    pub fn room_point(&self, x: f32, y: f32) -> (f32, f32) {
        match &self.game {
            Some(g) => {
                let p = g.design_point_at(x, y);
                (
                    (p.x + g.world.aboard.offset.x) as f32,
                    (p.y + g.world.aboard.offset.y) as f32,
                )
            }
            None => (0.0, 0.0),
        }
    }

    /// A canvas point as a design point — a point on the ship's own grid,
    /// in design units, whichever phase the session is in: read back
    /// through the yard's view before the world opens, and through the
    /// ship's camera and heading after (`Game::design_point_at`). What
    /// one player's pointer is sent to the others as (feature 60): the
    /// same tile on every machine, whatever each has zoomed and turned.
    pub fn design_point(&self, x: f32, y: f32) -> (f32, f32) {
        match &self.game {
            Some(g) => {
                let p = g.design_point_at(x, y);
                (p.x as f32, p.y as f32)
            }
            None => self.editor.view.to_world(x, y),
        }
    }

    /// [`Self::design_point`] read forwards: where a design point lands
    /// in the view's units — what the view offset and scale turn into a
    /// canvas pixel, the way the shapes and the crew's names are placed.
    /// Nought and the yard's grid agree, so before the world opens a
    /// design point *is* the view's; after, it goes through the ship's
    /// turn like a Bim (`world_paint::crew_on_screen`).
    pub fn design_point_on_screen(&self, x: f32, y: f32) -> (f32, f32) {
        match &self.game {
            Some(g) => world_paint::design_on_screen(g, x, y),
            None => (x, y),
        }
    }
}

// --- the self check -------------------------------------------------------

/// Every bit set means this build agrees with the pinned constants.
pub const SELF_CHECK_ALL: u32 = 0b11111111;

/// Does *this build* get the same answers the fixtures pin?
///
/// `design_hash` is what an Accept is recorded against, so it has to be
/// identical on every machine in a game and on the native server that will
/// one day be authoritative. The fixtures in `shipdesign::fixture` and
/// `world::fixture` are the written-down answers; `crates/shipdesign/src/tests.rs`
/// and `crates/world/src/tests.rs` check them one at a time, and this checks
/// them all at once, as a bitmask so a failure says which half.
pub fn self_check() -> u32 {
    use shipdesign::fixture::{CREWS, REFERENCE_HASH, REFERENCE_PARTS, reference};

    let mut bits = 0;
    if shipdesign::parts::defs_are_sound() {
        bits |= 1;
    }
    for (i, &crew) in CREWS.iter().enumerate() {
        let design = reference(crew);
        let right = shipdesign::design_hash(&design) == REFERENCE_HASH[i]
            && design.parts.len() as u32 == REFERENCE_PARTS[i];
        if right {
            bits |= 1 << (i + 1);
        }
    }
    if !shipdesign::has_errors(&shipdesign::validate(&reference(4), 4)) {
        bits |= 1 << 3;
    }
    // The draw format, and the money arithmetic: a lone player's pool is
    // their own money plus the bonus, and a crew's is nothing but their own.
    // It is the one sum two machines have to agree on down to the euro.
    let solo = economy::starting_pool(100_000, 1) == Ok(120_000);
    let crew = economy::starting_pool(100_000, 4) == Ok(400_000);
    let none = economy::starting_pool(100_000, 0).is_err();
    // And a desk's quote: a handgun is worth 1 500 at the book, and a
    // plain orbital's desk asks 1 575 for one and bids 1 425 — the book,
    // half the spread either side, rounded down. The same sum the market
    // tests pin, so a machine whose integer division went its own way
    // says so.
    let quoted =
        economy::market::quote(economy::market::MarketKind::Orbital, 0, ResourceId::Handgun)
            == economy::market::Quote {
                ask: 1_575,
                bid: 1_425,
            };
    if crate::draw::STRIDE == 12 && solo && crew && none && quoted {
        bits |= 1 << 4;
    }
    // The reference is carrying what it is meant to carry, and the sealed
    // hull is sealed.
    let design = reference(4);
    let stowed = shipdesign::fixture::REFERENCE_CARGO
        .iter()
        .all(|&(id, units)| design.carrying(id) == units);
    if stowed && shipdesign::exposure(&design).is_empty() {
        bits |= 1 << 5;
    }
    // And the **world**: a fixed scenario, stepped a fixed number of times,
    // checksummed.
    if world::fixture::reference_run() == world::fixture::REFERENCE_CHECKSUM {
        bits |= 1 << 6;
    }
    // And the simulation's ship, for the same reason as the reference.
    let playtest = shipdesign::fixture::playtest_ship();
    if shipdesign::design_hash(&playtest) == shipdesign::fixture::PLAYTEST_HASH
        && playtest.parts.len() as u32 == shipdesign::fixture::PLAYTEST_PARTS
    {
        bits |= 1 << 7;
    }
    bits
}
