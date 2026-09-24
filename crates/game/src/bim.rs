//! One crew member: everything that belongs to a Bim rather than to the ship.
//!
//! Two of them live aboard and they are not distinguishable in code. Each has
//! its own needs, its own health, its own errand and its own queue of errands
//! put down; each has a berth and a seat at the table that the other never
//! uses. What they share is the room — one galley, one pot, one set of heads —
//! and `game.rs` is where the sharing is arbitrated.
//!
//! The only asymmetry is the player. [`PLAYER`] is the one the mouse steers;
//! the other lives exactly the same day entirely on its own account, which is
//! the whole point of it being there.

use crate::character::{Character, Look};
use crate::clock;
use crate::combat::{Blow, Gear, Trigger};
use crate::filth::{self, Filth, Mess, Ordeal};
use crate::health::Health;
use crate::math::{Vec2, vec2};
use crate::memory::Memory;
use crate::needs::Needs;
use crate::rng::Rng;
use crate::social::Solitude;
use crate::task::{Saved, Task};

/// Who is aboard. The index is the whole identity: it picks the berth, the
/// seat, the colour and the name the host prints, and it never changes.
pub const CREW: usize = 2;

/// The one the player steers. Orders, selection and recruiting all mean this
/// one; the other takes no instruction from anybody.
pub const PLAYER: usize = 0;

/// How long a footprint lingers, and how far apart they are laid. Per Bim, so
/// two wanders read as two.
pub const TRAIL_LIFE: f32 = 2.2;
pub const TRAIL_INTERVAL: f32 = 0.08;

/// How many finished errands a Bim keeps to make conversation out of. A few
/// hours' worth: what they talk about should be the afternoon they have just
/// had, not something from the week before last.
pub const TALKS_ABOUT: usize = 8;

/// How often a bleeding Bim leaves a drop of blood on the deck, in
/// seconds, with one open wound — with more it is that many times as
/// often. Real seconds at 1x: the world's speed leaves a longer trail the
/// way it leaves more of everything. A drop is filth on the tile it lands
/// on — `Mess::Blood`, [`filth::BLOOD_COST`] — and stays until it is swept.
pub const DRIP_EVERY: f32 = 1.2;
/// How far from the body's middle a drop lands, in room units, either way.
const DRIP_SCATTER: f32 = 10.0;

/// How long a Bim with no bunk may sleep on the deck, in game minutes,
/// out of every [`GROUND_WINDOW`]: three hours in six. The window opens
/// the minute a lie-down on the deck begins and the allowance comes back
/// whole when it closes, so an interrupted lie-down keeps what it did
/// not use. See `Game::tick_bim` and `Game::can_sleep_on_ground`.
pub const GROUND_SLEEP: f32 = 3.0 * clock::HOUR;
pub const GROUND_WINDOW: f32 = 6.0 * clock::HOUR;
/// How long a Bim is **sore** from a lie-down on the deck, in game minutes
/// from the moment it gets up: half a day of rest draining
/// [`crate::needs::SORE_TIRING`] times as fast. Re-armed every minute of
/// the lie-down, so it runs from the end of it.
pub const SORE_LASTS: f32 = 12.0 * clock::HOUR;

/// A medic's surge running on a body (feature 76): the seconds of the
/// room's clock it has left, and whether every open wound is closed as
/// it ends (the medic's *closing surge*).
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Surge {
    pub left: f32,
    pub closing: bool,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Footprint {
    pub pos: Vec2,
    pub age: f32,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bim {
    pub character: Character,
    /// The errand running now, and everything put down to make way for it.
    pub task: Option<Task>,
    pub queue: Vec<Saved>,
    pub needs: Needs,
    pub health: Health,
    /// How long this Bim has been holding on, and how long it has been
    /// standing in the mess. Its own clocks, not the deck's — see [`Ordeal`].
    pub ordeal: Ordeal,
    /// Game minutes left of having dropped off standing up.
    pub nap_left: f32,
    /// How long this Bim has been without anybody to talk to, and what that
    /// is costing it. Its own clock, not the ship's — see [`Solitude`].
    pub solitude: Solitude,
    /// Game minutes left of sitting on the deck having given up for a bit.
    /// Works exactly like `nap_left`: the frame stops for this Bim while it
    /// runs, and whatever it was in the middle of is still there afterwards.
    pub sad_left: f32,
    /// What it is talking about this instant, as a topic code, or 0. Set when
    /// a chat is arranged and cleared when it ends; the host turns it into a
    /// sentence in a bubble, because no strings cross the boundary.
    pub chat_topic: u32,
    /// The last few errands it finished, as `job_code`s, newest last.
    ///
    /// Small talk and nothing else. A Bim used to have something to say by
    /// reading its own diary back — but the diary now keeps only the things
    /// that went wrong, and a crew whose week has gone well would have had
    /// nothing to say to each other at all. So what it has been *doing* is
    /// kept here instead, where nothing but the conversation reads it, and
    /// forgotten again as fast as it arrives.
    pub lately: Vec<u32>,
    /// Seconds until it next looks for somewhere cleaner to stand.
    pub flee_wait: f32,
    /// Holding a line (feature 75): a soldier braced where it stands —
    /// no errands, no running, steadier shooting. Toggled by the world
    /// (`Game::set_braced`); off again on any order that moves it
    /// (`interrupt_for_order`) and when it goes down. Saved with the
    /// room and in `world_checksum`.
    pub braced: bool,
    /// *Rampage* stacks (feature 75): each enemy this soldier downs is
    /// one, up to `world::class::RAMPAGE_STACKS`, until the fight ends —
    /// the world counts them and clears them. In `world_checksum`.
    pub rampage: u32,
    /// Holding a heal beam on somebody (feature 76): a medic linked to a
    /// patient, as the world last said (`Game::set_beaming`). Off again
    /// on any order to an errand (`Game::order`) and when the medic goes
    /// down, which the world reads back and breaks the link by. In
    /// `world_checksum` through the world's own list.
    pub beaming: bool,
    /// A medic's *surge* on this body (feature 76): while it runs, a hit
    /// takes nothing — no wound, no armour drained, no trauma
    /// (`Game::strike`). Set by the world (`Game::set_surge`), counted
    /// down here; `closing` closes every open wound as it ends. Saved
    /// with the room and in `world_checksum`.
    pub surge: Option<Surge>,
    /// Standing as a wall (feature 77): a tank with Bulwark on — half
    /// pace, and the crew close behind him are in cover against a shot
    /// that comes through him. Toggled by the world
    /// (`Game::set_bulwark`), off again when he goes down. Saved with
    /// the room and in `world_checksum`.
    pub bulwark: bool,
    /// How many enemy hits have landed on this body since the last point
    /// of experience they made (feature 77): a tank turns every
    /// `world::class::TANK_HITS_PER_XP` of them into one and starts the
    /// count again; for anybody else it only ever climbs and is read by
    /// nobody. Saved with the room and in `world_checksum`.
    pub hits_taken: u32,
    /// The crewmate this body carries in its arms (feature 86): a medic
    /// — the class, or a hired field medic — that has picked up somebody
    /// unconscious or hurt to take them out of the fire. Set by
    /// [`crate::game::Game::take_up`], put down by `set_down`, and
    /// dropped the moment either of the two goes down. While it runs the
    /// carried body is stood where the carrier stands and walks nowhere
    /// of its own, and the carrier holds its fire and walks at
    /// [`crate::game::CARRY_PACE`]: both its arms are full. Saved with
    /// the room and in `world_checksum`.
    pub carrying: Option<usize>,
    /// Whether this body is a **field medic** (feature 86): a mercenary
    /// hired for the job, with none of the medic class's talents, whose
    /// business under arms is to fetch the fallen out of the fire and
    /// treat them where it is quiet, and who otherwise keeps to the far
    /// end of its weapon's reach. Set by the world every step
    /// (`Game::set_field_medic`) off `world::mercenary::Hired::medic`,
    /// the way the squad's orders are, and so neither saved here nor
    /// hashed.
    pub field_medic: bool,
    /// Seconds this body has been dying with an enemy about (feature
    /// 78): nought until it is, counted up here and read by
    /// `Game::is_fleeing`, which lets it run once the count passes the
    /// hold its skill gives it — nought for everybody, and the
    /// commander's aura's `NERVE_HOLD` for a Bim in one.
    /// Saved with the room and in `world_checksum`.
    pub fear: f32,
    /// Where the player sent it, held back until a door has been opened. Only
    /// ever set on [`PLAYER`]: nobody sends the other one anywhere.
    pub pending_move: Option<Vec2>,
    pub trail: Vec<Footprint>,
    pub trail_timer: f32,
    /// Where it was last frame and how long it has been marching without
    /// getting anywhere, for `Game::unstick`.
    pub last_pos: Vec2,
    pub stuck: f32,

    /// When it was born: a year in [`BORN_FROM`]..=[`BORN_TO`], and a day of
    /// that year. Rolled at the start and never changing, which is the whole
    /// of what a birthday is.
    pub born_year: u32,
    pub born_day: u32,
    /// What it remembers of its days. See `memory.rs`.
    pub memory: Memory,
    /// The worst each of these has been so far, so that going a stage further
    /// can be noticed once rather than every frame it lasts.
    pub worst_hunger: u32,
    pub worst_weariness: u32,
    /// Game minutes of food poisoning left, nothing when well. Its own clock,
    /// like the ordeal's: it is this body that is ill. See `Game::poison`.
    pub poisoned_for: f32,
    /// Sleeping on the deck, for a Bim with no bunk of its own: how many
    /// game minutes of it are left in the window that is open, and how
    /// many minutes the window has left — nought for no window open, when
    /// the allowance is whole. See [`GROUND_SLEEP`].
    pub ground_left: f32,
    pub ground_window: f32,
    /// Game minutes left of being sore from the deck, nothing when not.
    /// See [`SORE_LASTS`].
    pub sore: f32,
    /// Its bunk, carried between rooms and read nowhere else: `Game::take_crew`
    /// writes the room's [`crate::room::Room::sleeps_in`] entry here and
    /// `Game::adopt` reads it back, since the crew leave one room for
    /// another with the deck. While the Bim is in a room the room's table
    /// is the truth and this is stale.
    pub bed: Option<usize>,

    /// What it wears and what it shoots with. See `crate::combat`.
    pub gear: Gear,
    /// Its trigger: the reload, and the burst a pull started. One rule
    /// for a Bim and a sentry alike — see `crate::combat::Trigger` and
    /// `Game::tick_combat`.
    pub trigger: Trigger,
    /// The enemy — an index into the targets — it is in a melee with:
    /// locked, unable to fire, trading blows every `MELEE_PERIOD`
    /// seconds on `melee_timer`. Nothing but the distance between them
    /// keeps it. See `crate::combat`.
    pub locked: Option<usize>,
    pub melee_timer: f32,
    /// The blow it is in the middle of, if any: started with the swing,
    /// landing when the animation ends. See `Game::tick_combat`.
    pub blow: Option<Blow>,
    /// Where it leans out to while it aims from a peek beside a wall:
    /// the peek eye's tile middle, and where a shot at it is aimed and
    /// lands. `None` standing in the open.
    pub peek: Option<Vec2>,
    /// Seconds left of the flash a hit puts on the body.
    pub hit_flash: f32,
    /// How long until the next drop of blood on the deck. See
    /// [`Bim::tick_drips`].
    pub drip_timer: f32,
    /// Seconds until an enemy at war next chooses where to stand. Its
    /// own clock, so a room of enemies does not all replan on one frame.
    pub plan_wait: f32,
    /// Seconds until an enemy with nobody it can reach next looks for a
    /// locked door to go through. See `Game::breach`.
    pub breach_wait: f32,
    /// The door it is heaving at, while it is.
    pub smashing: Option<usize>,
    /// Whether a hostile gunner has hunted this war: it went for where it
    /// last saw its quarry, and from then until the war ends it holds or
    /// closes — never gives ground. See `Game::plan_stand`.
    pub hunting: bool,
    /// A door its run takes it through, and which side of it the run
    /// began on: locked behind it once it is through. See `Game::flee`.
    pub seal: Option<(usize, f32)>,
    /// The door it locked behind itself, while that lock stands: sealed
    /// in, it binds its wounds. See `Game::flee`.
    pub sealed_in: Option<usize>,
    /// Seconds towards the next wound bound while sealed in.
    pub bind_timer: f32,
    /// A station's person's peacetime round (feature 102): its role and
    /// the stops it walks, dealt by the world when the site's room opens
    /// (`Game::set_role`). `None` for the crew and for anybody the world
    /// dealt none. Walked only with the needs off
    /// (`Game::keep_to_routine`), and carried with the body through
    /// `take_crew` and `adopt` like its post.
    pub routine: Option<crate::routine::Routine>,
}

/// The years the crew were born in. Everyone aboard is somewhere between
/// twenty and fifty when the game opens in [`clock::START_YEAR`].
pub const BORN_FROM: u32 = 2350;
pub const BORN_TO: u32 = 2380;

impl Bim {
    pub fn new(who: usize, at: Vec2, rng: &mut Rng) -> Bim {
        Bim {
            character: Character::new(at, Look::of(who), rng),
            task: None,
            queue: Vec::new(),
            needs: Needs::new(),
            health: Health::new(),
            ordeal: Ordeal::new(),
            solitude: Solitude::new(),
            nap_left: 0.0,
            sad_left: 0.0,
            chat_topic: 0,
            lately: Vec::new(),
            flee_wait: 0.0,
            braced: false,
            rampage: 0,
            beaming: false,
            surge: None,
            bulwark: false,
            hits_taken: 0,
            carrying: None,
            field_medic: false,
            fear: 0.0,
            pending_move: None,
            trail: Vec::new(),
            trail_timer: 0.0,
            last_pos: at,
            stuck: 0.0,
            born_year: BORN_FROM + rng.below(BORN_TO - BORN_FROM + 1),
            born_day: rng.below(clock::DAYS_IN_YEAR),
            memory: Memory::new(),
            worst_hunger: 0,
            worst_weariness: 0,
            poisoned_for: 0.0,
            ground_left: GROUND_SLEEP,
            ground_window: 0.0,
            sore: 0.0,
            bed: None,
            gear: Gear::issued(),
            trigger: Trigger::default(),
            locked: None,
            melee_timer: 0.0,
            blow: None,
            peek: None,
            hit_flash: 0.0,
            drip_timer: 0.0,
            plan_wait: 0.0,
            breach_wait: 0.0,
            smashing: None,
            hunting: false,
            seal: None,
            sealed_in: None,
            bind_timer: 0.0,
            routine: None,
        }
    }

    pub fn is_poisoned(&self) -> bool {
        self.poisoned_for > 0.0
    }

    /// Whether it is sore from sleeping on the deck. See [`SORE_LASTS`].
    pub fn is_sore(&self) -> bool {
        self.sore > 0.0
    }

    /// How old it is on the given date, in whole years. A birthday that has
    /// not come round yet this year has not been had.
    pub fn age(&self, year: u32, day_of_year: u32) -> u32 {
        let had_it = day_of_year >= self.born_day;
        year.saturating_sub(self.born_year)
            .saturating_sub(if had_it { 0 } else { 1 })
    }

    pub fn is_alive(&self) -> bool {
        !self.character.is_dead()
    }

    /// Lay and age the footprints behind it. A task is not tracked: during one
    /// the Bim walks where it is told and the prints are only clutter.
    pub fn tick_trail(&mut self, dt: f32) {
        self.trail_timer -= dt;
        if self.trail_timer <= 0.0 && self.character.is_walking() && !self.character.is_scripted() {
            self.trail_timer = TRAIL_INTERVAL;
            self.trail.push(Footprint {
                pos: self.character.pos,
                age: 0.0,
            });
        }
        for f in &mut self.trail {
            f.age += dt;
        }
        self.trail.retain(|f| f.age < TRAIL_LIFE);
    }

    /// Drip blood on the deck. A drop every [`DRIP_EVERY`] seconds over the
    /// open wounds while it bleeds and lives — a dead Bim has stopped —
    /// scattered a little about the body so a Bim standing still leaves a
    /// pool rather than a dot, and each drop is **filth**: the tile it
    /// lands on is soiled with `Mess::Blood`, which the broom takes up like
    /// any stain and the surroundings need follows like any other. There is
    /// no picture of a drop of its own any more; the deck draws the tile.
    /// The scatter is rolled off the room's stream: a Bim only bleeds after
    /// a fight, and no seed-pinned probe has one, so nothing they pin is
    /// re-rolled.
    pub fn tick_drips(&mut self, dt: f32, rng: &mut Rng, deck: &mut Filth) {
        let wounds = self.health.bleeding();
        if wounds > 0 && self.is_alive() {
            self.drip_timer -= dt;
            if self.drip_timer <= 0.0 {
                self.drip_timer = DRIP_EVERY / wounds as f32;
                let at = self.character.pos
                    + vec2(rng.signed() * DRIP_SCATTER, rng.signed() * DRIP_SCATTER);
                deck.soil(at, filth::BLOOD_COST, Mess::Blood);
            }
        } else {
            self.drip_timer = 0.0;
        }
    }
}
