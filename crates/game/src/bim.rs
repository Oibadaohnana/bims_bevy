//! One body on a deck: everything that belongs to a Bim rather than to the
//! room — its figure, its health, its errand and the queue of errands put
//! down, its gear — whether it is one of the crew or one of a
//! station's people. They are not distinguishable in code; `game.rs` is where
//! what they share is arbitrated.
//!
//! The only asymmetry is the player. [`PLAYER`] is the one the mouse steers
//! in a room with one player in it.

use crate::blood::Blood;
use crate::character::{Character, Look};
use crate::clock;
use crate::combat::{Blow, Gear, Trigger};
use crate::health::Health;
use crate::math::{Vec2, vec2};
use crate::rng::Rng;
use crate::task::{Saved, Task};

/// How many a bare room is stood up with (`Game::bare`). The index is the
/// whole identity: it picks the colour and the name the host prints, and it
/// never changes.
pub const CREW: usize = 2;

/// The one the player steers. Orders, selection and recruiting all mean this
/// one; the other takes no instruction from anybody.
pub const PLAYER: usize = 0;

/// How long a footprint lingers, and how far apart they are laid. Per Bim, so
/// two wanders read as two.
pub const TRAIL_LIFE: f32 = 2.2;
pub const TRAIL_INTERVAL: f32 = 0.08;

/// How often a bleeding Bim — one under
/// [`crate::health::BLEEDS_UNDER`] hit points, downed or not — leaves a
/// drop of blood on the deck, in seconds (task 120; it was the rate of one
/// open wound). A drop is blood on the tile it lands on —
/// [`crate::blood::BLOOD_COST`] off it — and stays, so a body on the move
/// leaves a trail.
pub const DRIP_EVERY: f32 = 1.2;
/// How far from the body's middle a drop lands, in room units, either way.
const DRIP_SCATTER: f32 = 10.0;

/// A medic's surge running on a body (feature 76): the seconds of the
/// room's clock it has left.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Surge {
    pub left: f32,
}

/// A shield on a body (task 142, a relic's *Lifeline*): the hit points it
/// still takes before a hit reaches the armour, the seconds of the room's
/// clock it has left, and what it began with, for the picture.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Shield {
    pub hp: f32,
    pub left: f32,
    pub full: f32,
}

impl Shield {
    /// What is left of it, nought to one.
    pub fn share(&self) -> f32 {
        if self.full > 0.0 {
            (self.hp / self.full).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
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
    pub health: Health,
    /// Holding a line (feature 75): a soldier braced where it stands —
    /// no errands, no running, steadier shooting. Toggled by the world
    /// (`Game::set_braced`); off again on any order that moves it
    /// (`interrupt_for_order`) and when it goes down. Saved with the
    /// room and in `world_checksum`.
    pub braced: bool,
    /// Holding a heal beam on somebody (feature 76): a medic linked to a
    /// patient, as the world last said (`Game::set_beaming`). Off again
    /// on any order to an errand (`Game::order`) and when the medic goes
    /// down, which the world reads back and breaks the link by. In
    /// `world_checksum` through the world's own list.
    pub beaming: bool,
    /// A medic's *surge* on this body (feature 76): while it runs, a hit
    /// takes nothing — no hit points, no armour drained
    /// (`Game::strike`). Set by the world (`Game::set_surge`), counted
    /// down here. Saved with the room and in `world_checksum`.
    pub surge: Option<Surge>,
    /// A relic's shield on this body (task 142): a hit takes it down
    /// before it reaches the armour, a surge's hit leaves it alone
    /// (`Game::strike`). Set by the world (`Game::set_shield`), counted
    /// down here. Saved with the room and in `world_checksum` where set.
    #[cfg_attr(feature = "serde", serde(default))]
    pub shield: Option<Shield>,
    /// Standing as a wall (feature 77): a tank with Bulwark on — half
    /// pace, and the crew close behind him are in cover against a shot
    /// that comes through him. Toggled by the world
    /// (`Game::set_bulwark`), off again when he goes down. Saved with
    /// the room and in `world_checksum`.
    pub bulwark: bool,
    /// How many enemy hits have landed on this body (feature 77): a count
    /// that only climbs, read by the relics for a hit taken. It made a
    /// tank's experience until task 119. Saved with the room and in
    /// `world_checksum`.
    pub hits_taken: u32,
    /// How many shots this body has fired (feature 106): what a relic's
    /// *Overcharge Cell* counts to know which shot is the charged one
    /// (`Skill::for_shot`). Only ever climbs. Saved with the room and in
    /// `world_checksum`.
    pub shots: u32,
    /// The crewmate this body carries in its arms (feature 86): a medic
    /// — the class, or a hired field medic — that has picked up somebody
    /// downed to take them out of the fire. Set by
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
    /// revive them where it is quiet, and who otherwise keeps to the far
    /// end of its weapon's reach. Set by the world every step
    /// (`Game::set_field_medic`) off `world::mercenary::Hired::medic`,
    /// the way the squad's orders are, and so neither saved here nor
    /// hashed.
    pub field_medic: bool,
    /// Whether this body is a commander's **Medivac medic** (his C): a
    /// reinforcement of the Republic's who fights as any bot does and
    /// runs to a player downed to revive him, whatever the fight round the
    /// body (`Game::medivac_patient`). Set by the world every step
    /// (`Game::set_medivac`) off `world::Reinforcement::medic`.
    #[cfg_attr(feature = "serde", serde(default))]
    pub medivac: bool,
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
    /// Its bunk, carried between rooms and read nowhere else: `Game::take_crew`
    /// writes the room's [`crate::room::Room::bunk_of`] entry here and
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
    /// Seconds left of being **under fire**: an enemy's hit landed on the
    /// body within [`crate::game::UNDER_FIRE`] seconds. A bot under fire
    /// fights (`Game::bot_stand`) and starts no revive, whether or not it
    /// can see who shot it.
    #[cfg_attr(feature = "serde", serde(default))]
    pub under_fire: f32,
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
    /// A station's person's peacetime round (feature 102): its role and
    /// the stops it walks, dealt by the world when the site's room opens
    /// (`Game::set_role`). `None` for the crew and for anybody the world
    /// dealt none. Walked by `Game::keep_to_routine`, and carried with the
    /// body through `take_crew` and `adopt` like its post.
    pub routine: Option<crate::routine::Routine>,
    /// Whether this body is a **Manufacturer** (feature 109): one of the
    /// faction that built the machines, laid on a site of theirs by the
    /// world (`Game::enlist_manufacturer`). A hostile Bim like any other in
    /// how it walks, fights and bleeds, with two things taken away:
    /// nobody revives or carries it — downed, it dies when its countdown
    /// runs out — and a gun is never let go of onto the deck, so it leaves
    /// nothing lying for anybody to take.
    #[cfg_attr(feature = "serde", serde(default))]
    pub manufacturer: bool,
    /// Seconds until its arc greaves may discharge again (task 116): set
    /// to [`crate::balance::ARC_COOLDOWN`] by a discharge
    /// (`Game::enemy_strike`) and run down in `Game::tick_combat`. Nought
    /// for anybody who wears none or has not been struck lately.
    #[cfg_attr(feature = "serde", serde(default))]
    pub arc_cool: f32,
    /// Where an **attack-move** is bound (the F key, a player's own Bim
    /// alone): the body walks there with its weapon out, stands still to
    /// shoot the moment it has something in its sights, and walks on
    /// when nothing is left to shoot at (`Game::keep_attack_moving`).
    /// `None` for a plain walk and for everybody else; any other order
    /// the player gives calls it off.
    #[cfg_attr(feature = "serde", serde(default))]
    pub attack_move: Option<Vec2>,
    /// The enemy a player **right-clicked** (task 126, Dota's attack
    /// order), by its index in the room's target list: the body shoots
    /// that one and nobody else, walks after it until it has a shot, and
    /// keeps at it until the enemy is down or dead or the player orders
    /// something else (`Game::order_attack`, `Game::chase`). `None` for
    /// everybody else.
    #[cfg_attr(feature = "serde", serde(default))]
    pub focus: Option<usize>,
    /// **Gone from the deck** (task 129): a commander's reinforcement that
    /// died, which is not drawn, picked or seen from the step it falls —
    /// its index kept until the mission's end so nobody else's moves.
    /// False for every other body.
    #[cfg_attr(feature = "serde", serde(default))]
    pub gone: bool,
    /// What is in its hands (task 138): the weapon, or the medkit a
    /// player picks with the quickselect's second key — with that in
    /// hand it holds its fire, and a right-click on a downed crewmate is
    /// the revive. Only a player's own Bim ever changes it
    /// (`Game::order_hand`); everybody else keeps the weapon.
    #[cfg_attr(feature = "serde", serde(default))]
    pub hand: Hand,
}

/// The quickselect (task 138): what a player's own Bim holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Hand {
    /// The weapon, which it fires as it always did.
    #[default]
    Weapon,
    /// The medkit: no shooting, and a right-click on a downed crewmate
    /// revives it.
    Medkit,
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
            health: Health::new(),
            braced: false,
            beaming: false,
            surge: None,
            shield: None,
            bulwark: false,
            hits_taken: 0,
            shots: 0,
            carrying: None,
            field_medic: false,
            medivac: false,
            trail: Vec::new(),
            trail_timer: 0.0,
            last_pos: at,
            stuck: 0.0,
            born_year: BORN_FROM + rng.below(BORN_TO - BORN_FROM + 1),
            born_day: rng.below(clock::DAYS_IN_YEAR),
            bed: None,
            gear: Gear::issued(),
            trigger: Trigger::default(),
            locked: None,
            melee_timer: 0.0,
            blow: None,
            peek: None,
            hit_flash: 0.0,
            under_fire: 0.0,
            drip_timer: 0.0,
            plan_wait: 0.0,
            breach_wait: 0.0,
            smashing: None,
            hunting: false,
            routine: None,
            manufacturer: false,
            arc_cool: 0.0,
            attack_move: None,
            focus: None,
            gone: false,
            hand: Hand::Weapon,
        }
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

    /// Drip blood on the deck. A drop every [`DRIP_EVERY`] seconds while it
    /// is under [`crate::health::BLEEDS_UNDER`] hit points and alive —
    /// downed too; a dead Bim has stopped (task 120) — scattered a little
    /// about the body so a Bim standing still leaves a pool rather than a
    /// dot, and one walking a trail. There is no picture of a drop of its
    /// own; the deck draws the tile. The scatter is two rolls a drop off the
    /// room's stream, which every fight draws from.
    pub fn tick_drips(&mut self, dt: f32, rng: &mut Rng, deck: &mut Blood) {
        if self.health.bleeds() && self.is_alive() {
            self.drip_timer -= dt;
            if self.drip_timer <= 0.0 {
                self.drip_timer = DRIP_EVERY;
                let at = self.character.pos
                    + vec2(rng.signed() * DRIP_SCATTER, rng.signed() * DRIP_SCATTER);
                deck.drop_at(at);
            }
        } else {
            self.drip_timer = 0.0;
        }
    }
}
