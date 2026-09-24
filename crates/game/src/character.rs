//! The Bim: a top-down character that decides where to go on its own, unless
//! a task is telling it what to do.

use crate::combat::{ArmourKind, WeaponKind};
use crate::draw::{Brush, Color, DrawList};
use crate::math::{PI, Rect, TAU, Vec2, angle_lerp, approach, clamp, lerp, vec2, wrap_angle};
use crate::rng::Rng;
use crate::room::{
    BROOM_HEAD, BROOM_POLE, Dish, GRIP, STEEL, draw_knife, draw_plate, draw_spoon, draw_stew_tub,
};

// --- behaviour tuning ---------------------------------------------------

/// How sharply the body swings towards the heading it wants.
const TURN_RATE: f32 = 5.5;
/// Pixels per second squared, used both to speed up and to slow down.
const ACCEL: f32 = 220.0;
/// Odds that a finished walk is followed by a pause rather than another walk.
const PAUSE_CHANCE: f32 = 0.28;
/// Odds that a new walk picks a wholly new direction instead of a gentle turn.
const REVERSAL_CHANCE: f32 = 0.15;
/// Speed when marching to a spot a task or the player picked.
const MARCH_SPEED: f32 = 96.0;
/// How close counts as arrived at the final waypoint.
const ARRIVE_RADIUS: f32 = 5.0;
/// How close counts as having rounded an intermediate corner. Looser than
/// arrival so a multi-leg route flows instead of stuttering at every turn.
const WAYPOINT_RADIUS: f32 = 11.0;
/// Distance over which the Bim eases down on its approach.
const SLOWDOWN_RADIUS: f32 = 70.0;
/// How much of its pace a body backing away runs at (feature 84): a
/// body walking backwards with its gun up is slower than one running.
const BACKSTEP_PACE: f32 = 0.65;
/// And how much of it a body running for the ship with nothing to shoot
/// at runs at — the head-down sprint, a little faster than a walk.
const SPRINT_PACE: f32 = 1.25;
/// How long it stands still on arrival before wandering off again.
const ARRIVE_SETTLE: f32 = 0.9;
/// How far short of where it is bound a leg of a walk on a window may end
/// and count as arrived: a tile. A route on a window ends on a tile's
/// middle, so a spot on the deck reached from the plain is reached
/// exactly, and a click on the ground is reached to within the tile.
const FAR_LEG: f32 = crate::filth::TILE;
/// How much bigger the Bim is drawn than the original sprite. Everything about
/// the body — parts, arm reach, where held items sit — goes through this, so the
/// proportions against the pot and the table stay as designed.
pub const BODY_SCALE: f32 = 1.45;
/// How much smaller than that a body lying on the deck is drawn — dead or
/// out cold. At the standing scale the sprawl ran a little over two tiles
/// from boots to flung hand and lay across most of two more; at half it
/// is about a tile long and fits inside two whichever way it lies.
const FLAT_SCALE: f32 = 0.5;
/// How much bigger than a dead one a body that is only out cold is drawn.
/// A living body on the deck is slack rather than gone: a shade broader and
/// longer than the corpse beside it, on top of the live colours and the
/// breath, so the two tell each other apart at a glance from above.
const OUT_COLD_SCALE: f32 = 1.1;
/// How far the body centre is kept clear of walls and furniture.
pub const BODY_MARGIN: f32 = 23.0;
/// How close a click or marquee has to come to count as touching the Bim.
pub const PICK_RADIUS: f32 = 26.0;
/// How fast the walls talk the Bim out of a plan that points at them.
const INTENT_RATE: f32 = 3.0;
/// How far from a wall the Bim starts turning back, as a fraction of the
/// smaller room dimension so a narrow room is not entirely "edge".
const EDGE_MARGIN_FRAC: f32 = 0.16;
const EDGE_MARGIN_MIN: f32 = 40.0;
const EDGE_MARGIN_MAX: f32 = 120.0;
/// How far from a piece of furniture the Bim starts going round it.
const AVOID_RANGE: f32 = 54.0;

/// One up-and-down of the knife, and one trip of the fork to the mouth. Tasks
/// count their steps in these units so the animation and the state agree.
pub const CHOP_PERIOD: f32 = 0.34;
pub const SCOOP_PERIOD: f32 = 0.62;
pub const BITE_PERIOD: f32 = 0.95;

// --- look ---------------------------------------------------------------

/// The ship's coverall: what every one of the crew wears.
const SHIRT: Color = Color::rgb(0.33, 0.58, 0.85);
const SLEEVE: Color = Color::rgb(0.27, 0.49, 0.74);
/// The station's, worn by everybody who lives there. Far enough from the
/// ship's that who is crew and who is not never has to be told from where
/// they happen to be standing — see [`Uniform`].
const SHIRT_STATION: Color = Color::rgb(0.84, 0.52, 0.24);
const SLEEVE_STATION: Color = Color::rgb(0.70, 0.41, 0.18);
/// A mercenary's: dark olive, so one living on a station is told from the
/// people who live there at a glance, and keeps its colours once hired —
/// hired hands are not crew.
const SHIRT_MERCENARY: Color = Color::rgb(0.40, 0.47, 0.30);
const SLEEVE_MERCENARY: Color = Color::rgb(0.31, 0.37, 0.23);
/// The pressure suit, and the visor over the head while it is worn.
const SHIRT_SUIT: Color = Color::rgb(0.86, 0.88, 0.92);
const SLEEVE_SUIT: Color = Color::rgb(0.70, 0.73, 0.79);
const VISOR: Color = Color::rgb(0.55, 0.78, 0.95);
/// The yoke across the shoulders, in the wearer's own colour: the one thing
/// that tells two people in the same coverall apart from directly above.
const TRIM: Color = Color::rgb(0.86, 0.93, 0.96);
const TRIM_B: Color = Color::rgb(0.74, 0.42, 0.62);
const SKIN: Color = Color::rgb(0.91, 0.73, 0.55);
const NOSE: Color = Color::rgb(0.82, 0.62, 0.45);
/// The hair, in [`Shade`]'s order: the first two are the classic pair's
/// — the first crew member's dark crop, the second's brown fall.
const HAIR: Color = Color::rgb(0.23, 0.17, 0.12);
const HAIR_B: Color = Color::rgb(0.42, 0.26, 0.13);
const HAIR_BLACK: Color = Color::rgb(0.12, 0.10, 0.10);
const HAIR_BLOND: Color = Color::rgb(0.82, 0.68, 0.38);
const HAIR_RED: Color = Color::rgb(0.64, 0.27, 0.12);
const HAIR_GREY: Color = Color::rgb(0.70, 0.70, 0.68);
/// The band round a bun or a ponytail.
const HAIR_BAND: Color = Color::rgb(0.55, 0.20, 0.25);
const BOOT: Color = Color::rgb(0.24, 0.18, 0.13);
const OUTLINE: Color = Color::rgba(0.05, 0.08, 0.07, 0.55);
const SHADOW: Color = Color::rgba(0.0, 0.0, 0.0, 0.20);
const VEG: Color = Color::rgb(0.44, 0.68, 0.24);
const VEG_DARK: Color = Color::rgb(0.30, 0.50, 0.16);
const TOFU: Color = Color::rgb(0.93, 0.91, 0.82);
const TOFU_EDGE: Color = Color::rgb(0.78, 0.76, 0.66);
/// A crate of materials on the way to a construction site, and the strap
/// round it.
const CRATE: Color = Color::rgb(0.55, 0.47, 0.32);
const CRATE_STRAP: Color = Color::rgb(0.32, 0.27, 0.19);
/// A medkit's white box, and the cross on it.
const MEDKIT: Color = Color::rgb(0.92, 0.93, 0.92);
const MEDKIT_CROSS: Color = Color::rgb(0.80, 0.16, 0.16);
/// What a Bim that has had an accident is covered in. The same colour as the
/// mess on the deck, so the two read as the same substance.
const GRIME: Color = Color::rgb(0.24, 0.18, 0.09);
/// The Zs that float off a sleeping Bim.
const SLEEP_Z: Color = Color::rgb(0.80, 0.90, 0.95);
/// The same body, with everything warm taken out of it.
const GONE_SHIRT: Color = Color::rgb(0.30, 0.36, 0.42);
const GONE_SLEEVE: Color = Color::rgb(0.25, 0.30, 0.36);
const GONE_SKIN: Color = Color::rgb(0.55, 0.53, 0.50);
const GONE_HAIR: Color = Color::rgb(0.18, 0.17, 0.16);
/// Shared with the trail and the order marker, so everything the player is
/// steering reads as one colour.
pub const ACCENT: Color = Color::rgb(0.50, 0.82, 0.66);
/// A Bim under direct orders. Its own colour, so being recruited reads at a
/// glance and does not have to be told apart from being merely selected.
const COMMAND: Color = Color::rgb(1.0, 0.82, 0.35);
/// The brackets round a braced soldier (feature 75): the command colour
/// darkened, since it is a stance held rather than an order given.
const BRACED: Color = Color::rgb(0.85, 0.62, 0.20);
/// The **braced stance** (feature 91): what a soldier holding its ground
/// does with its feet and how far it settles onto them. The toes are
/// turned out a good third of a right angle, the heels a little back
/// under the body, and the whole figure is drawn at [`BRACE_CROUCH`] of
/// its size against an unchanged shadow — which from directly above is
/// the only way a picture can say *lower*. Drawing only, off
/// `Character::braced`, which `Game::tick_combat` sets.
///
/// [`BRACE_FEET_APART`] is the load-bearing number, and it is wide for a
/// reason: the boots are drawn **before** the torso, which is 33 units
/// across, so a foot inside 16 of the middle is a foot nobody ever sees —
/// standing at ease they are hidden entirely and a stride is what brings
/// one out. A braced foot has to be set past that edge or the stance is
/// a change to a picture the torso covers.
const BRACE_FEET_APART: f32 = 19.0;
const BRACE_TOE_OUT: f32 = 0.55;
const BRACE_SET_BACK: f32 = 2.5;
const BRACE_CROUCH: f32 = 0.93;
/// The halo round a body under a medic's surge (feature 76): a pale
/// healing green, wide and breathing, so a body that cannot be hurt
/// reads as such.
pub const SURGE: Color = Color::rgb(0.55, 0.95, 0.75);
/// An enemy: the ring under one, in the colour its shots are.
const ENEMY: Color = crate::combat::HOSTILE_BOLT;
/// Blood: the blotch on a part with an open wound, and the drops on the
/// deck. Dark, so it reads as blood and not as the enemy's red.
pub const BLOOD: Color = Color::rgb(0.55, 0.05, 0.05);
/// A bandage: off-white gauze, and the shadowed edge of a turn of it.
const BANDAGE: Color = Color::rgb(0.93, 0.91, 0.84);
const BANDAGE_EDGE: Color = Color::rgb(0.72, 0.70, 0.62);
/// The guns, drawn: the dark body of each, its lighter edge, and the
/// emitter at the muzzle in the colour a friendly bolt is; the wooden
/// furniture of a shotgun or a sniper's stock, the grey of a scope and
/// a rail, and the pale steel of a muzzle brake and a bipod.
const GUN: Color = Color::rgb(0.15, 0.17, 0.20);
const GUN_EDGE: Color = Color::rgb(0.42, 0.47, 0.53);
const GUN_LIT: Color = crate::combat::FRIENDLY_BOLT;
const STOCK: Color = Color::rgb(0.45, 0.30, 0.16);
const SCOPE: Color = Color::rgb(0.30, 0.34, 0.40);
/// The glass in a scope: the objective lens, catching the same light the
/// emitter gives off.
const LENS: Color = Color::rgb(0.55, 0.78, 0.92);
/// How far out to the side each arm hangs off the body. With the hands
/// free that is where the arm is *drawn*, one blob a side; with a weapon
/// in them it is where the arm **starts**, and [`Character::draw_weapon`]
/// runs a sleeve from there out to whichever hand of that gun is on that
/// side (feature 82). An arm drawn as a limb rather than as a blob is
/// what reads as *holding* something when the only view is from above.
const SHOULDER_ACROSS: f32 = 13.5;
/// How wide a sleeve is drawn, and the dark rim round it — the same two
/// widths the blob at a free hand has, so an arm reaching for a gun is
/// the same arm.
const SLEEVE_WIDE: f32 = 8.0;
const SLEEVE_RIM: f32 = 10.0;
/// How far a hand is from the shoulder with the arm straight, and how
/// far out the elbow swings when it is not. A reaching arm drawn as one
/// straight capsule reads as a pole; the bend is what says *arm*, and
/// how much of it there is is how far short of straight the reach is —
/// so a hand held in close doubles the elbow right out and one at arm's
/// length does not bend it at all.
const ARM_REACH: f32 = 26.0;
const ARM_BEND: f32 = 4.5;
/// The carry: how far across the body's own line a gun is angled, in
/// radians, and where its grip sits — forward of the chest and a little
/// over towards the firing shoulder. A gun held straight down the
/// centreline reads as a pole growing out of the face; angled across it
/// reads as held. This is the **carry** and nothing else: a Bim with
/// nothing to shoot at. Aiming, the gun is swung onto the target
/// instead ([`Character::gun_rot`]), because the shot now leaves the
/// muzzle ([`Character::muzzle`], `Game::tick_combat`) and a barrel
/// pointing anywhere but at what it is shooting reads as broken.
const CARRY: f32 = 0.30;
const GRIP_AHEAD: f32 = 3.0;
const GRIP_ACROSS: f32 = 4.5;
/// How far off the body's own facing the gun may be swung onto a target
/// (feature 84), in radians: enough for a Bim shooting as it walks
/// somewhere else to keep the barrel on what it is firing at, and not so
/// far that the arms wrap round the back. Within it the barrel runs
/// from the grip through the target and the shot leaves straight down
/// it; past it — a target behind a walking Bim — the gun holds at the
/// limit and the shot leaves the muzzle for the target all the same,
/// which is the one place the picture and the line disagree.
const AIM_ACROSS: f32 = 1.15;
/// How big a gun is drawn beside the body. [`draw_gun`] describes each
/// one at its own full size and every drawing of it — in the hands, on
/// the deck — is laid at this share of the body's scale, so the whole
/// rack comes down at one knob rather than every block in it being
/// re-numbered. At full size a rifle is longer than the figure is wide
/// and reads as a lance.
const GUN_SCALE: f32 = 0.72;
/// The schword: a hilt, and a blade with a white core and a cyan laser
/// edge — two strokes, one wide and faint, one thin and bright. The swing
/// is the same blade swept through an arc in front of the body. In a
/// hand the core is lit (feature 98): pulled `BLADE_TINT` of the way
/// towards the edge's colour — the crew's cyan or the enemy's red — and
/// `BLADE_HEAT` times as bright, past white, so the bloom makes the glow
/// the two strokes were standing in for. A little cooler than a bolt's
/// core, since a blade is lit the whole time it is held.
const HILT: Color = Color::rgb(0.22, 0.22, 0.26);
const BLADE_CORE: Color = Color::rgb(0.98, 1.0, 1.0);
pub const BLADE_EDGE: Color = Color::rgb(0.45, 0.95, 1.0);
const BLADE_TINT: f32 = 0.35;
const BLADE_HEAT: f32 = 1.9;
/// How long a swing or a punch takes, in seconds — a number of the
/// fight, kept in `crate::balance` since the blow lands when the animation
/// ends (`crate::combat::Blow`) — and how wide a swing sweeps: a hundred
/// degrees in front of the body.
pub use crate::balance::SWING_TIME;
const SWING_ARC: f32 = 100.0 * (PI / 180.0);
/// How far a peeking body leans out towards the eye it aims from, as a
/// share of the way there.
const LEAN: f32 = 0.55;
/// The armour, worn: a steel-blue cap over the hair, a dark plate over the
/// torso with the yoke still showing at the collar, and darker boots with
/// a shin band. A broken piece is drawn cracked — a lighter diagonal
/// stroke across it — in `CRACK`.
const HELM: Color = Color::rgb(0.55, 0.62, 0.72);
const HELM_RIM: Color = Color::rgb(0.42, 0.48, 0.57);
const KEVLAR: Color = Color::rgb(0.22, 0.24, 0.28);
const KEVLAR_STRAP: Color = Color::rgb(0.32, 0.34, 0.38);
const GUARD: Color = Color::rgb(0.16, 0.12, 0.09);
const GUARD_BAND: Color = Color::rgb(0.40, 0.42, 0.46);
const CRACK: Color = Color::rgba(0.85, 0.88, 0.92, 0.75);
/// What a class wears (feature 81). Seen from directly above there is not
/// much of a body to look at, so a class says itself three times over:
/// the coverall dyed its own shade, something on the head and something
/// over the chest and shoulders. The dye is a *mix* rather than a
/// replacement so the coverall underneath still says crew or station —
/// an engineer ashore is an ochre-ish orange, an engineer of the crew an
/// ochre-ish blue.
const KIT_ENGINEER: Color = Color::rgb(0.88, 0.66, 0.18);
const KIT_SOLDIER: Color = Color::rgb(0.34, 0.41, 0.23);
const KIT_MEDIC: Color = Color::rgb(0.93, 0.95, 0.97);
const KIT_TANK: Color = Color::rgb(0.44, 0.49, 0.56);
const KIT_COMMANDER: Color = Color::rgb(0.21, 0.27, 0.49);
/// Webbing, leather and the dark side of a plate.
const KIT_DARK: Color = Color::rgb(0.15, 0.13, 0.12);
/// Brass and braid.
const KIT_GOLD: Color = Color::rgb(0.92, 0.78, 0.32);
/// The soldier's sunglasses: near-black lenses in a pale frame — the
/// frame because the crown behind them is dark on most heads, and a dark
/// lens on dark hair is nothing at all — with a glint along one lens.
const SHADES: Color = Color::rgb(0.07, 0.08, 0.10);
const SHADES_RIM: Color = Color::rgb(0.58, 0.62, 0.68);
const SHADES_GLINT: Color = Color::rgba(0.78, 0.90, 1.0, 0.80);
/// How far the coverall is dyed towards the class's colour.
const DYE: f32 = 0.45;

/// Whose coverall a Bim is wearing: the ship's or the station's.
///
/// The crew wear one and the people living on a station wear the other, so
/// that with the two rooms joined at the airlock — see `world::docking` — a
/// glance at the deck says who belongs to the ship and who is going ashore
/// when it casts off. Nothing but `draw` reads it; a resident is simulated
/// exactly as a crew member is.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Uniform {
    Crew,
    Station,
    /// A mercenary's, at a station or hired aboard: the world says who
    /// is one (`world::mercenary`), the room only draws it.
    Mercenary,
    /// A pressure suit, worn for a walk outside and drawn with a visor over
    /// the head. Put on by [`Character::go_outside`] over whichever of the
    /// other two the body wears, and taken off again by
    /// [`Character::come_inside`].
    Suit,
}

impl Uniform {
    fn shirt(self) -> Color {
        match self {
            Uniform::Crew => SHIRT,
            Uniform::Station => SHIRT_STATION,
            Uniform::Mercenary => SHIRT_MERCENARY,
            Uniform::Suit => SHIRT_SUIT,
        }
    }

    fn sleeve(self) -> Color {
        match self {
            Uniform::Crew => SLEEVE,
            Uniform::Station => SLEEVE_STATION,
            Uniform::Mercenary => SLEEVE_MERCENARY,
            Uniform::Suit => SLEEVE_SUIT,
        }
    }
}

/// What a crew member's class shows on the body (feature 81): the kit it
/// works in, drawn over the [`Uniform`] rather than instead of it.
///
/// Nothing but `draw` reads it, and nothing in the room decides it: the
/// world hands it over every step off its own `classes`
/// (`Class::outfit`), which is why it is left out of a save — it is a
/// function of what is saved, like the draw buffer. A body nobody steers
/// — a hire, a resident, a garrison — is [`Outfit::Plain`] and looks the
/// way every Bim used to.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Outfit {
    #[default]
    Plain,
    /// A hard hat with a peak, a tool belt and pouches, in ochre.
    Engineer,
    /// Sunglasses, crossed webbing with three brass magazines on it and a
    /// belt, in olive drab: kitted out and looking as though it has done
    /// this before.
    Soldier,
    /// A white coat, a capped cross on the head and another on the chest,
    /// with a bag on the hip.
    Medic,
    /// A heavy helm with cheek guards, pauldrons capping both shoulders
    /// and a gorget across the chest, in steel — and the biggest body of
    /// the six.
    Tank,
    /// An officer's peaked cap banded in gold, gold shoulder boards and a
    /// sash across the chest, in navy.
    Commander,
}

impl Outfit {
    /// Every one, in the order `world::Class::ALL` lists the classes, so
    /// a class's place is its outfit's.
    pub const ALL: [Outfit; 6] = [
        Outfit::Plain,
        Outfit::Engineer,
        Outfit::Soldier,
        Outfit::Medic,
        Outfit::Tank,
        Outfit::Commander,
    ];

    /// The class's own colour, or `None` for a body in no kit at all.
    fn colour(self) -> Option<Color> {
        match self {
            Outfit::Plain => None,
            Outfit::Engineer => Some(KIT_ENGINEER),
            Outfit::Soldier => Some(KIT_SOLDIER),
            Outfit::Medic => Some(KIT_MEDIC),
            Outfit::Tank => Some(KIT_TANK),
            Outfit::Commander => Some(KIT_COMMANDER),
        }
    }

    /// A cloth of the coverall's, dyed towards the class's colour. The
    /// suit is not dyed: a pressure suit is a pressure suit.
    fn dye(self, cloth: Color, uniform: Uniform) -> Color {
        match self.colour() {
            Some(c) if uniform != Uniform::Suit => cloth.mix(c, DYE),
            _ => cloth,
        }
    }

    /// What the body's scale is multiplied by: the tank fills more of a
    /// tile than the medic does. Drawing only, like [`Look::scale`] — the
    /// reach and where a click lands are the same for all.
    pub fn scale(self) -> f32 {
        match self {
            Outfit::Tank => 1.08,
            Outfit::Soldier => 1.02,
            Outfit::Medic => 0.96,
            _ => 1.0,
        }
    }
}

/// Which of the crew this is, as far as the drawing is concerned: the
/// colour of the yoke on the coverall, how the hair is worn and what
/// colour it is, and how big the body is.
///
/// Nothing but `draw` reads it. Two Bims behave identically — that is the
/// point of the second one — so the only thing that distinguishes them in
/// the simulation is the index, and the only thing that distinguishes them
/// on the deck is this. The coverall itself is the [`Uniform`]'s. The
/// index deals a look ([`Look::of`]); a player picks the hair for their
/// own at the start ([`Look::with_hair`], feature 62), and the room is
/// told through `Game::set_look`.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Look {
    pub yoke: Yoke,
    pub hair: Hair,
    pub shade: Shade,
    /// How big the body is drawn, `0..BUILDS`: [`Look::scale`] turns it
    /// into a factor on the body's scale. Drawing only — the reach, the
    /// collision radius and where a click lands are the same for all.
    pub build: u8,
}

/// How many builds there are, from the slightest to the sturdiest, and
/// how far apart: `BUILDS / 2` is the middling one, drawn at exactly the
/// body scale, and each step either side is `BUILD_STEP` of it — enough
/// to tell two apart standing together, not enough to look like another
/// kind of thing.
pub const BUILDS: u8 = 5;
pub const BUILD_STEP: f32 = 0.06;

/// The colour of the yoke across the shoulders: the one thing that told
/// the classic pair apart from directly above, kept alternating down the
/// crew.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Yoke {
    Pale,
    Mauve,
}

/// How the hair is worn, seen from above. Every arm is a shape or two on
/// the crown in [`Character::draw`] and its lying-down twin in
/// `draw_flat`; the name of each is the app's (`names::HAIR_NAMES`,
/// pinned to [`Hair::ALL`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Hair {
    /// Short all over: the crown and nothing past the head.
    Cropped,
    /// Down past the collar: a fall back over the shoulders.
    Long,
    /// None.
    Bald,
    /// Cut level at the chin: the crown wider than the head all round.
    Bob,
    /// Tied up in a knot at the back.
    Bun,
    /// A strip down the middle, the sides shaved.
    Mohawk,
    /// Tied back and hanging behind in one tail.
    Ponytail,
    /// A round mass standing out from the head all round.
    Curly,
}

impl Hair {
    /// Every style, in the order a chooser lists them. The classic pair's
    /// first: the first crew member's crop, the second's fall.
    pub const ALL: [Hair; 8] = [
        Hair::Cropped,
        Hair::Long,
        Hair::Bald,
        Hair::Bob,
        Hair::Bun,
        Hair::Mohawk,
        Hair::Ponytail,
        Hair::Curly,
    ];

    /// Its place in [`Hair::ALL`].
    pub fn code(self) -> u8 {
        Hair::ALL.iter().position(|&h| h == self).unwrap_or(0) as u8
    }
}

/// What colour the hair is. Like [`Hair`], listed for a chooser and named
/// by the app (`names::SHADE_NAMES`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Shade {
    Dark,
    Brown,
    Black,
    Blond,
    Red,
    Grey,
}

impl Shade {
    pub const ALL: [Shade; 6] = [
        Shade::Dark,
        Shade::Brown,
        Shade::Black,
        Shade::Blond,
        Shade::Red,
        Shade::Grey,
    ];

    /// Its place in [`Shade::ALL`].
    pub fn code(self) -> u8 {
        Shade::ALL.iter().position(|&s| s == self).unwrap_or(0) as u8
    }

    /// The colour, for the swatch a chooser shows as much as for the
    /// crown: red, green and blue, nought to one.
    pub fn rgb(self) -> (f32, f32, f32) {
        let c = self.colour();
        (c.r, c.g, c.b)
    }

    fn colour(self) -> Color {
        match self {
            Shade::Dark => HAIR,
            Shade::Brown => HAIR_B,
            Shade::Black => HAIR_BLACK,
            Shade::Blond => HAIR_BLOND,
            Shade::Red => HAIR_RED,
            Shade::Grey => HAIR_GREY,
        }
    }
}

/// The colour a **player's own** Bim is ringed in (feature 84): one a
/// player, chosen in the setup before the game opens, so that on a deck
/// of fourteen the ones somebody is steering are the ones with a
/// coloured circle under them. A bot has none — the only ring it ever
/// wears is the pale one a selection puts there.
///
/// Listed for a chooser and named by the app (`names::TINT_NAMES`),
/// like [`Hair`] and [`Shade`]; there are as many as there are player
/// slots, so no two players need share one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Tint {
    Teal,
    Amber,
    Rose,
    Violet,
    Sky,
    Lime,
}

impl Tint {
    /// Every colour, in the order a chooser lists them. Slot *i* starts
    /// on the *i*th, so a lobby that nobody touches still has one
    /// colour a player.
    pub const ALL: [Tint; 6] = [
        Tint::Teal,
        Tint::Amber,
        Tint::Rose,
        Tint::Violet,
        Tint::Sky,
        Tint::Lime,
    ];

    /// Its place in [`Tint::ALL`].
    pub fn code(self) -> u8 {
        Tint::ALL.iter().position(|&t| t == self).unwrap_or(0) as u8
    }

    /// The one at that place, for a code that crossed the seam.
    pub fn from_code(code: u8) -> Tint {
        Tint::ALL
            .get(code as usize)
            .copied()
            .unwrap_or(Tint::ALL[0])
    }

    /// The colour, for the swatch a chooser shows as much as for the
    /// ring: red, green and blue, nought to one.
    pub fn rgb(self) -> (f32, f32, f32) {
        let c = self.colour();
        (c.r, c.g, c.b)
    }

    pub(crate) fn colour(self) -> Color {
        match self {
            // The first is the accent every other thing the player
            // steers is already drawn in, so the default look does not
            // change at all.
            Tint::Teal => ACCENT,
            Tint::Amber => Color::rgb(1.0, 0.75, 0.28),
            Tint::Rose => Color::rgb(0.95, 0.48, 0.58),
            Tint::Violet => Color::rgb(0.70, 0.58, 0.95),
            Tint::Sky => Color::rgb(0.45, 0.72, 0.98),
            Tint::Lime => Color::rgb(0.70, 0.90, 0.38),
        }
    }
}

impl Look {
    /// The classic pair: pale yoke and a dark crop, mauve yoke and brown
    /// hair down past the collar, both of middling build. What the first
    /// two of any crew look like, so the room's two Bims are the two they
    /// always were.
    pub const CLASSIC: [Look; 2] = [
        Look {
            yoke: Yoke::Pale,
            hair: Hair::Cropped,
            shade: Shade::Dark,
            build: BUILDS / 2,
        },
        Look {
            yoke: Yoke::Mauve,
            hair: Hair::Long,
            shade: Shade::Brown,
            build: BUILDS / 2,
        },
    ];

    /// The index decides it: the classic pair for the first two, and for
    /// everybody after a hair, a shade and a build dealt off the index
    /// with the yoke still alternating — so a crew of five, a station's
    /// residents and a garrison are all told apart at a glance, and the
    /// same index is the same look on every machine. Deliberately not
    /// off the RNG: a draw here would move every roll after it, and the
    /// probes pin their seeds.
    pub fn of(who: usize) -> Look {
        if let Some(&classic) = Look::CLASSIC.get(who) {
            return classic;
        }
        // A multiplicative hash, so neighbouring indices do not come out
        // as a sequence.
        let k = (who as u32).wrapping_mul(0x9E37_79B9);
        let pick = |shift: u32, of: usize| ((k >> shift) % of as u32) as usize;
        Look {
            yoke: if who % 2 == 0 {
                Yoke::Pale
            } else {
                Yoke::Mauve
            },
            hair: Hair::ALL[pick(8, Hair::ALL.len())],
            shade: Shade::ALL[pick(16, Shade::ALL.len())],
            build: pick(24, BUILDS as usize) as u8,
        }
    }

    /// The same look with the hair the player chose.
    pub fn with_hair(self, hair: Hair, shade: Shade) -> Look {
        Look {
            hair,
            shade,
            ..self
        }
    }

    /// What the body's scale is multiplied by for this build: one for the
    /// middling one, a step either way for each build from it.
    pub fn scale(self) -> f32 {
        let build = self.build.min(BUILDS - 1) as f32;
        1.0 + (build - (BUILDS / 2) as f32) * BUILD_STEP
    }

    fn trim(self) -> Color {
        match self.yoke {
            Yoke::Pale => TRIM,
            Yoke::Mauve => TRIM_B,
        }
    }

    fn hair(self) -> Color {
        self.shade.colour()
    }
}

#[derive(Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
enum Activity {
    Walking,
    Pausing,
    /// Heading for a spot the player or a task picked, rather than one it chose.
    Marching,
}

/// How a body walks while it is falling back to the ship (feature 84).
///
/// A fall back is a walk like any other — the route is the nav grid's —
/// and this is only how the body carries itself along it: **backwards**
/// with the gun kept on the enemy, which is slower, or a **sprint** with
/// its back turned, which is faster. Which of the two is the fight's
/// answer and not the walk's: a body with something to shoot at backs
/// away from it, and one with nothing to shoot at runs.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FallBack {
    /// Backing away, facing that angle — where the enemy is — while the
    /// feet carry it along the route the other way.
    Backwards(f32),
    /// Running for the ship, facing the way it is going.
    Sprint,
}

impl FallBack {
    /// How much of the body's own pace this walk runs at.
    fn pace(self) -> f32 {
        match self {
            FallBack::Backwards(_) => BACKSTEP_PACE,
            FallBack::Sprint => SPRINT_PACE,
        }
    }
}

/// Something in the Bim's hands.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Held {
    Nothing,
    Vegetable,
    /// A block of tofu, which is a vegetable as far as the chain is concerned
    /// and a different shape as far as the eye is.
    Tofu,
    /// What came off the board: rounds of vegetable and cubes of tofu, in
    /// two hands on the way to the pot.
    Chopped {
        rounds: u32,
        cubes: u32,
    },
    /// A plain fork, for eating at the table.
    Fork,
    Knife,
    Spoon,
    /// The broom, out of its locker.
    Broom,
    /// A pick, for a rock outside. In the tool hand, and swung the way the
    /// knife is.
    Pick,
    /// A plate or a bowl, carrying how full it is and which it is.
    Plate(f32, Dish),
    /// A pot of stew in a tub, on its way to the cold store or back from it.
    Stew,
    /// A crate of materials off a shelf, on its way to a construction site.
    /// The room never knows what is in it: the count is the world's.
    Crate,
    /// A medkit off a cabinet, on its way to a crewmate dying.
    Medkit,
}

/// What the hands are busy doing. Each one drives its own arm animation.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Action {
    None,
    /// Arms out in front: opening a door, setting something down, reaching in.
    Reach,
    /// The knife going up and down on the board.
    Chop,
    /// The spoon swinging from the pot across to the plate.
    Serve,
    /// Cutlery going from the plate to the mouth and back.
    Eat,
    /// Out cold, arms tucked in, breathing slowly.
    Sleep,
    /// Both hands together under the tap, turning over one another.
    Wash,
    /// Hopping on the spot: what a Bim that needs the heads does while it
    /// waits, and the only warning the player gets before an accident.
    Fidget,
    /// Both hands on a broom, working it across the deck.
    Sweep,
    /// Doubled over, being sick on the deck.
    Retch,
    /// Talking to the other one: a hand comes up and drops again, the way one
    /// does. The *words* are the host's — no strings cross this boundary — so
    /// all the simulation ever draws is the gesture.
    Talk,
    /// A swing of the schword: the blade sweeps an arc in front of the
    /// body over [`SWING_TIME`].
    Swing,
    /// A jab of the right fist, forward and back, in a melee with no blade.
    Punch,
    /// Winding a bandage: both hands close together in front, going round
    /// one another, the roll in the right and the strip paying out of it.
    Bandage,
}

/// One turn of the hands under the tap.
const SCRUB_PERIOD: f32 = 0.55;
/// One stroke of the broom across the deck and back.
const SWEEP_PERIOD: f32 = 0.9;

/// One hop on the spot, and one heave.
const HOP_PERIOD: f32 = 0.42;
const HEAVE_PERIOD: f32 = 0.75;
/// One rise and fall of the hand while talking.
const TALK_PERIOD: f32 = 1.1;
/// One turn of the hands round each other while a bandage is wound.
const WRAP_PERIOD: f32 = 0.7;

/// How long one breath takes while asleep, in seconds.
const BREATH_PERIOD: f32 = 5.4;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Character {
    pub pos: Vec2,
    /// Which of the crew this is to look at. Read by `draw` and nothing else.
    look: Look,
    /// The direction the body actually faces, in radians.
    pub heading: f32,
    pub speed: f32,

    /// Where the Bim *intends* to go, before steering is applied.
    intent: f32,
    target_speed: f32,

    activity: Activity,
    /// Seconds left before the current activity is reconsidered.
    timer: f32,

    /// Advances with distance travelled, so the walk cycle never slides.
    stride: f32,
    /// Advances with time; drives idle breathing and looking around.
    idle: f32,

    /// Waypoints left to walk, from the pathfinder. Overrides the wander until
    /// the last one is reached.
    path: Vec<Vec2>,
    /// Who has this Bim selected: one bit a player, bit `p` for the
    /// player in slot `p`. A mask rather than a flag because two players
    /// do not share a selection — see `crate::order`. Read through
    /// [`Character::is_selected_by`], set through [`Character::select_for`].
    pub selected: u32,
    /// Animates the selection ring so a picked Bim reads at a glance.
    select_pulse: f32,
    /// The colour of the player whose own Bim this is (feature 84), or
    /// `None` for a bot, a hire, a resident — anybody nobody steers.
    /// Drawn as a steady ring on the deck under the body, so a player
    /// picks their own out of a crowd without selecting anything. Set
    /// through [`Character::set_tint`]; `crate::game::Game::set_tints`
    /// is what the world says it with.
    tint: Option<Tint>,

    /// While scripted, the Bim does nothing of its own accord — a task is
    /// driving it, and it stands still between instructions.
    scripted: bool,
    /// Standing a while at a stop of its round (feature 102,
    /// `crate::routine`): it holds still there rather than wander off,
    /// the way a posted body does. Set and cleared by the round alone.
    lingering: bool,
    /// A direction to turn to on the spot, used between scripted steps.
    face_target: Option<f32>,
    seated: bool,

    main: Held,
    tool: Held,
    action: Action,
    action_phase: f32,

    /// How fast it walks, as a fraction of its usual pace. Hunger slows it.
    pace: f32,
    /// Once dead it does nothing at all, and is drawn where it fell.
    dead: bool,
    /// Dropped off standing up. Holds still, keeps whatever it was carrying
    /// and whatever route it was on, and picks both up again on waking.
    napping: bool,
    /// Under direct orders: it stands where it is put rather than pottering
    /// about, and nothing starts of its own accord.
    recruited: bool,
    /// Where it has been told to stand and stay — the helm, the far side of
    /// an airlock. It holds still there rather than pottering about, goes
    /// off on its errands as usual, and walks back afterwards; only a fresh
    /// order or the room letting it go takes the post away. See
    /// `Game::send_to`.
    post: Option<Vec2>,
    /// Whose coverall it wears. Drawing only.
    uniform: Uniform,
    /// What its class wears over that (feature 81). Drawing only, and
    /// left out of a save: the world hands it over every step off its own
    /// classes, so a loaded game has it back before the first frame.
    #[cfg_attr(feature = "serde", serde(skip))]
    outfit: Outfit,
    /// Outside the hull, in a suit. Mechanically this is sitting — put
    /// somewhere by a chain and held there — at a spot beyond the skin,
    /// and `worn` is the coverall to go back into. Read by the world to
    /// dose the body; see `Game::is_outside`.
    outside: bool,
    worn: Uniform,
    /// Out on a planet's plain, past the deck's grids: walking a window of
    /// its own (`Game::refresh_afield`), and told so every step from where
    /// it stands. Nothing to do with the suit.
    afield: bool,
    /// Where a walk beyond the window is really going: the route on it
    /// ends at the window's edge, and the game plans the next leg from
    /// there when it does (`Game::continue_far_walk`). Not arrived until
    /// this is `None`.
    far: Option<Vec2>,
    /// How filthy the Bim itself is, 0 clean to 1 covered. Kept here rather
    /// than with the deck's own mess because this is the share that walks
    /// away with it.
    filth: f32,
    /// Seconds left of a hop or a heave. Both are short and both end by
    /// themselves, so nothing else has to remember to stop them.
    antic: f32,
    /// Weapon drawn: in combat mode, and which, for the picture in the
    /// hands. Drawing only — `Game::tick_combat` sets it every step from
    /// the gear and the orders.
    armed: Option<WeaponKind>,
    /// Where it leans out to, aiming from a peek beside a wall: the eye,
    /// and the body is drawn part of the way there ([`LEAN`]) and turned
    /// to look from it. Drawing only; `Game::tick_combat` sets it.
    lean: Option<Vec2>,
    /// What the weapon is pointed at, in room units (feature 84): the
    /// gun is swung onto it off the facing ([`AIM_ACROSS`]) instead of
    /// being carried across the front, so the barrel lies along the
    /// shot. `None` with nothing to shoot at, which is the carry.
    /// Drawing only — but the shot leaves [`Character::muzzle`], which
    /// is the end of that barrel, so `Game::tick_combat` sets this
    /// **before** it fires. Left out of a save the way `outfit` is: the
    /// fight sets it every step, so a loaded game has it back before the
    /// first shot.
    #[cfg_attr(feature = "serde", serde(skip))]
    aim: Option<Vec2>,
    /// Falling back to the ship (feature 84): how the body carries
    /// itself along the walk — backing away with the gun up, or running
    /// with its back turned — and `None` for every other walk there is.
    /// Movement and drawing only, set every step by `Game::tick_combat`
    /// off the player's standing order, and so left out of a save the
    /// way `aim` is.
    #[cfg_attr(feature = "serde", serde(skip))]
    falling_back: Option<FallBack>,
    /// An enemy, to whoever is looking: ringed in red under the body.
    /// Drawing only; the world says who is.
    hostile: bool,
    /// Braced (feature 75): a soldier holding its ground, marked with
    /// four heavy brackets round it. Drawing only; `Game::tick_combat`
    /// sets it off the Bim's own flag.
    braced: bool,
    /// Under a medic's surge (feature 76): a halo round the body while
    /// nothing can hurt it. Drawing only; `Game::tick_combat` sets it
    /// off the Bim's own timer.
    surging: bool,
    /// Out cold for want of blood: lying where it dropped, alive, doing
    /// nothing until it comes round. Set by `Game::tick_bim` off the
    /// health, the way napping is set off drowsiness; unlike a nap it
    /// drops the route and stands the body up out of whatever it sat in.
    unconscious: bool,
    /// Which parts have an open wound — head, body, legs — for the blotch
    /// drawn on each. Drawing only; `Game::wound` and the bandage set it.
    wounds: [bool; 3],
    /// What is worn on each part — head, body, legs — and whether it is
    /// broken, for the picture of it. Drawing only; the gear itself is the
    /// Bim's (`crate::combat::Gear`) and `Game` refreshes this whenever it
    /// changes.
    armour: [Option<Worn>; 3],
}

/// A piece of armour as the picture needs it: what it is, and whether it
/// is drawn cracked.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Worn {
    pub kind: ArmourKind,
    pub broken: bool,
}

impl Character {
    pub fn new(pos: Vec2, look: Look, rng: &mut Rng) -> Character {
        let heading = rng.range(0.0, TAU);
        let mut c = Character {
            pos,
            look,
            heading,
            speed: 0.0,
            intent: heading,
            target_speed: 0.0,
            activity: Activity::Pausing,
            timer: 0.0,
            stride: 0.0,
            idle: rng.range(0.0, 100.0),
            path: Vec::new(),
            selected: 0,
            select_pulse: 0.0,
            tint: None,
            scripted: false,
            lingering: false,
            face_target: None,
            seated: false,
            main: Held::Nothing,
            tool: Held::Nothing,
            action: Action::None,
            action_phase: 0.0,
            pace: 1.0,
            dead: false,
            napping: false,
            recruited: false,
            post: None,
            uniform: Uniform::Crew,
            outfit: Outfit::Plain,
            outside: false,
            worn: Uniform::Crew,
            afield: false,
            far: None,
            filth: 0.0,
            antic: 0.0,
            armed: None,
            lean: None,
            aim: None,
            falling_back: None,
            hostile: false,
            braced: false,
            surging: false,
            unconscious: false,
            wounds: [false; 3],
            armour: [None; 3],
        };
        c.begin_walk(rng);
        c
    }

    pub fn is_walking(&self) -> bool {
        self.activity != Activity::Pausing && self.speed > 4.0
    }

    /// Radius used for picking and marquee selection.
    pub fn pick_radius(&self) -> f32 {
        PICK_RADIUS
    }

    /// Whether a click at `p` lands on the figure. Standing, within
    /// [`PICK_RADIUS`] of where it is; down, the figure is stretched along
    /// its heading (`draw_flat`), so the test is the same radius about the
    /// line from the boots to the head — a click on either end is a click
    /// on the body, not on the deck under it.
    pub fn picked_at(&self, p: Vec2) -> bool {
        if !(self.dead || self.unconscious) {
            return (self.pos - p).len() <= PICK_RADIUS;
        }
        let along = Vec2::from_angle(self.heading);
        let (feet, head) = (
            -24.0 * BODY_SCALE * FLAT_SCALE,
            32.0 * BODY_SCALE * FLAT_SCALE,
        );
        let t = clamp((p - self.pos).dot(along), feet, head);
        (self.pos + along * t - p).len() <= PICK_RADIUS
    }

    /// True once an ordered walk has finished, so a task can move on.
    pub fn arrived(&self) -> bool {
        self.path.is_empty() && self.far.is_none()
    }

    /// Whether the route in hand has been walked, whatever `far` says.
    pub fn path_done(&self) -> bool {
        self.path.is_empty()
    }

    /// Move the route the Bim is on, waypoint by waypoint, for a body
    /// carried into another room whose origin is `shift` away — see
    /// `Game::adopt`. A route left in the old room's coordinates is a walk
    /// to somewhere that is not there.
    pub fn shift_route(&mut self, shift: Vec2) {
        for p in &mut self.path {
            *p += shift;
        }
        if let Some(far) = self.far.as_mut() {
            *far += shift;
        }
    }

    /// Where the route the Bim is on ends, or `None` if it is not on one.
    ///
    /// A route is planned once and not replanned, so anything that would put
    /// something in the Bim's way — a door shutting itself, say — has to know
    /// where it was going before it does.
    /// The route as it stands, for the probes.
    #[allow(dead_code)]
    pub fn path_for_probe(&self) -> &[Vec2] {
        &self.path
    }

    pub fn destination(&self) -> Option<Vec2> {
        self.path.last().copied()
    }

    // --- orders and scripting -------------------------------------------

    /// Walk the given route, then go back to whatever it was doing before.
    ///
    /// An empty route means the pathfinder found nowhere to go, so the Bim
    /// deliberately does not enter the marching state — a task waiting on
    /// `arrived` would otherwise wait forever.
    pub fn follow_path(&mut self, route: Vec<Vec2>) {
        self.path = route;
        self.far = None;
        self.timer = 0.0;
        self.face_target = None;
        if !self.path.is_empty() {
            self.activity = Activity::Marching;
        }
    }

    /// Hand control to a task, or give it back.
    pub fn set_scripted(&mut self, on: bool) {
        self.scripted = on;
        if on {
            self.path.clear();
            self.far = None;
            self.activity = Activity::Pausing;
            self.timer = 0.0;
        } else {
            self.face_target = None;
            self.seated = false;
            self.main = Held::Nothing;
            self.tool = Held::Nothing;
            self.action = Action::None;
            self.timer = 0.0;
        }
    }

    pub fn is_scripted(&self) -> bool {
        self.scripted
    }

    /// Stand still at a stop of the round rather than wander (feature
    /// 102): see [`crate::routine`].
    pub fn set_lingering(&mut self, on: bool) {
        self.lingering = on;
    }

    pub fn is_lingering(&self) -> bool {
        self.lingering
    }

    /// Turn on the spot to face `angle`.
    pub fn face(&mut self, angle: f32) {
        self.face_target = Some(angle);
    }

    /// True once a `face` has very nearly finished.
    pub fn facing_settled(&self) -> bool {
        match self.face_target {
            None => true,
            Some(a) => wrap_angle(a - self.heading).abs() < 0.12,
        }
    }

    pub fn sit(&mut self, at: Vec2, facing: f32) {
        self.pos = at;
        self.seated = true;
        self.speed = 0.0;
        self.target_speed = 0.0;
        self.face_target = Some(facing);
    }

    /// Lie down on a bed. Mechanically this is sitting — the Bim is put where
    /// the furniture says and holds still until told otherwise — but saying so
    /// at the call site is worth the three lines.
    pub fn lie(&mut self, at: Vec2, facing: f32) {
        self.sit(at, facing);
    }

    pub fn stand(&mut self) {
        self.seated = false;
    }

    /// Stop where it is: the walk dropped, the feet still. What a patient
    /// does while a crewmate walks over to it.
    pub fn halt(&mut self) {
        self.path.clear();
        self.far = None;
        self.speed = 0.0;
        self.target_speed = 0.0;
        if self.activity == Activity::Marching {
            self.activity = Activity::Pausing;
        }
    }

    /// Out through the airlock: standing at `at`, beyond the hull, in the
    /// suit, and from there walking the outside — `Game` hands the body the
    /// outside's grid and solids while it is out, so nothing shoves it back
    /// onto the deck; see `Game::tick_bim`. `pick` is whether it takes the
    /// pick out with it: a walk to mine does, a walk to build does not.
    pub fn go_outside(&mut self, at: Vec2, facing: f32, pick: bool) {
        if !self.outside {
            self.worn = self.uniform;
        }
        self.outside = true;
        self.uniform = Uniform::Suit;
        if pick {
            self.tool = Held::Pick;
        }
        self.stand_at(at);
        self.face(facing);
        self.set_action(Action::Reach);
    }

    /// Back in, standing at `at` — the deck inside the port — in the
    /// coverall that was under the suit. Safe to call on a body that is
    /// not outside; it then does nothing.
    pub fn come_inside(&mut self, at: Vec2) {
        if !self.outside {
            return;
        }
        self.outside = false;
        self.uniform = self.worn;
        // The pick goes with the suit.
        if self.tool == Held::Pick {
            self.tool = Held::Nothing;
        }
        self.stand_at(at);
        self.set_action(Action::None);
    }

    pub fn is_outside(&self) -> bool {
        self.outside
    }

    /// Out on the plain. See `afield`.
    pub fn is_afield(&self) -> bool {
        self.afield
    }

    pub fn set_afield(&mut self, afield: bool) {
        self.afield = afield;
    }

    /// Off the deck's grids either way: outside the hull, or out on the
    /// plain. What picks a body's grid.
    pub fn off_deck(&self) -> bool {
        self.outside || self.afield
    }

    /// Where a walk on a window is really bound, if beyond it. See `far`.
    pub fn far(&self) -> Option<Vec2> {
        self.far
    }

    /// Walk to `to` on `nav`: the route to the free cell nearest it, or
    /// nearest where the grid clamps it to. On a `window` — a grid carried
    /// about the body, which `to` may lie beyond — a route that ends more
    /// than a tile short of `to` is one leg of a longer walk, and `far`
    /// keeps the rest. True when there is a route at all; an empty one
    /// leaves the body arrived, as `follow_path` does.
    pub fn walk_to(&mut self, nav: &crate::nav::Nav, to: Vec2, window: bool) -> bool {
        let goal = nav.nearest_free(to);
        let route = nav.path(self.pos, goal);
        let ok = !route.is_empty();
        self.follow_path(route);
        // A leg that ends where the body stands is the walk over: the
        // nearest the ground lets it get to where it was bound.
        if window && ok && (goal - to).len() > FAR_LEG && (goal - self.pos).len() > FAR_LEG {
            self.far = Some(to);
        }
        ok
    }

    /// Get up and end up standing at `at`, which is how you leave a bed: the
    /// Bim was lying in the middle of it, and the floor beside it is the only
    /// place it can actually stand.
    pub fn stand_at(&mut self, at: Vec2) {
        self.pos = at;
        self.stand();
    }

    pub fn hold_main(&mut self, item: Held) {
        self.main = item;
    }

    pub fn hold_tool(&mut self, item: Held) {
        self.tool = item;
    }

    pub fn main_held(&self) -> Held {
        self.main
    }

    pub fn tool_held(&self) -> Held {
        self.tool
    }

    /// Where the Bim is sitting or lying and which way it faces, if it is on
    /// anything at all. Saved when a task is put down, because standing up is
    /// part of letting go of one.
    pub fn seat(&self) -> Option<(Vec2, f32)> {
        if self.seated {
            Some((self.pos, self.face_target.unwrap_or(self.heading)))
        } else {
            None
        }
    }

    /// How fast it walks, as a fraction of its usual pace.
    pub fn pace(&self) -> f32 {
        self.pace
    }

    pub fn set_pace(&mut self, pace: f32) {
        self.pace = pace;
    }

    /// Drop off on the spot, or come round again. Unlike dying or being sent
    /// to bed this keeps the path and the hands exactly as they were, so the
    /// Bim carries on with whatever it was doing.
    pub fn nod_off(&mut self, napping: bool) {
        self.napping = napping;
        self.speed = 0.0;
        self.target_speed = 0.0;
        self.set_action(if napping { Action::Sleep } else { Action::None });
    }

    pub fn is_napping(&self) -> bool {
        self.napping
    }

    pub fn set_recruited(&mut self, recruited: bool) {
        self.recruited = recruited;
    }

    pub fn is_recruited(&self) -> bool {
        self.recruited
    }

    pub fn set_post(&mut self, post: Option<Vec2>) {
        self.post = post;
    }

    pub fn post(&self) -> Option<Vec2> {
        self.post
    }

    pub fn set_uniform(&mut self, uniform: Uniform) {
        self.uniform = uniform;
    }

    pub fn uniform(&self) -> Uniform {
        self.uniform
    }

    /// What its class wears (feature 81). Drawing only.
    pub fn set_outfit(&mut self, outfit: Outfit) {
        self.outfit = outfit;
    }

    pub fn outfit(&self) -> Outfit {
        self.outfit
    }

    /// Weapon drawn, and which — `None` holstered.
    pub fn set_armed(&mut self, armed: Option<WeaponKind>) {
        self.armed = armed;
    }

    pub fn is_armed(&self) -> bool {
        self.armed.is_some()
    }

    /// Leaning out to a peek eye to aim from it, or standing square.
    pub fn set_lean(&mut self, eye: Option<Vec2>) {
        self.lean = eye;
    }

    /// What the weapon is pointed at, or nothing — the carry (feature
    /// 84). The gun swings onto it within [`AIM_ACROSS`] of the facing,
    /// and [`Character::muzzle`] follows it, so where a shot starts and
    /// where the barrel points are one answer.
    pub fn set_aim(&mut self, at: Option<Vec2>) {
        self.aim = at;
    }

    /// How this body is walking while it falls back to the ship
    /// (feature 84), or `None` for every other walk. Set every step;
    /// see [`FallBack`].
    pub fn set_falling_back(&mut self, how: Option<FallBack>) {
        self.falling_back = how;
    }

    /// Whether it is falling back at all — asked by the fight, which
    /// turns a sprint into a backing walk the step it finds something
    /// to shoot at.
    pub fn is_falling_back(&self) -> bool {
        self.falling_back.is_some()
    }

    /// Whether it is walking backwards, which is what the legs and the
    /// gun are drawn from.
    pub fn is_backing(&self) -> bool {
        matches!(self.falling_back, Some(FallBack::Backwards(_)))
    }

    /// Where the body is drawn: leant out towards the peek while it aims
    /// from one, else where it stands. What a shot at it is aimed at is
    /// the game's business (`Game::exposed_at`); this is the picture.
    pub fn drawn_at(&self) -> Vec2 {
        match self.lean {
            Some(eye) => self.pos + (eye - self.pos) * LEAN,
            None => self.pos,
        }
    }

    pub fn set_hostile(&mut self, hostile: bool) {
        self.hostile = hostile;
    }
    /// Braced, or not: the brackets round the body. Drawing only.
    pub fn set_braced(&mut self, braced: bool) {
        self.braced = braced;
    }
    /// Under a surge, or not: the halo round the body. Drawing only.
    pub fn set_surging(&mut self, surging: bool) {
        self.surging = surging;
    }

    /// Out cold, or come round. Going out drops the walk — whatever it
    /// was doing with its hands is the chain's to put down, and `Game`
    /// interrupts the errand first — and it lies where it stood: not
    /// seated, so nothing holds it in a bunk it has fallen out of.
    pub fn knock_out(&mut self, out: bool) {
        self.unconscious = out;
        if out {
            self.path.clear();
            self.activity = Activity::Pausing;
            self.speed = 0.0;
            self.target_speed = 0.0;
            self.seated = false;
            self.antic = 0.0;
            self.set_action(Action::None);
        }
    }

    pub fn is_unconscious(&self) -> bool {
        self.unconscious
    }

    /// Which parts bleed — head, body, legs — for the blotch drawn on each.
    pub fn set_wounds(&mut self, wounds: [bool; 3]) {
        self.wounds = wounds;
    }

    /// What is worn on each part — head, body, legs — for the picture of
    /// it. Drawing only: the gear is the Bim's, and `Game` calls this
    /// whenever it changes.
    pub fn set_worn(&mut self, worn: [Option<Worn>; 3]) {
        self.armour = worn;
    }

    /// It stops where it stands, and stays there.
    pub fn die(&mut self) {
        self.dead = true;
        self.path.clear();
        self.speed = 0.0;
        self.target_speed = 0.0;
        self.seated = false;
        self.main = Held::Nothing;
        self.tool = Held::Nothing;
        self.action = Action::None;
    }

    pub fn is_dead(&self) -> bool {
        self.dead
    }

    /// Up again where it lay, awake and standing (feature 103: a dead
    /// player's Bim bought back). Nothing in its hands.
    pub fn revive(&mut self) {
        self.dead = false;
        self.unconscious = false;
        self.path.clear();
        self.speed = 0.0;
        self.target_speed = 0.0;
        self.seated = false;
        self.action = Action::None;
    }

    /// Sitting or lying: put there by a chain, and not to be shoved about by
    /// anything outside it.
    pub fn is_seated(&self) -> bool {
        self.seated
    }

    pub fn set_action(&mut self, action: Action) {
        if self.action != action {
            self.action = action;
            self.action_phase = 0.0;
        }
    }

    // --- mess -------------------------------------------------------------

    /// How filthy the Bim itself is, 0 to 1.
    pub fn filth(&self) -> f32 {
        self.filth
    }

    /// Cover it in something. Takes the worse of what it has on it already and
    /// what it has just picked up: an accident cannot make a Bim cleaner.
    pub fn soil(&mut self, amount: f32) {
        self.filth = self.filth.max(amount).clamp(0.0, 1.0);
    }

    /// Get some of it off again. Washing at the basin is the only thing that
    /// does this so far, and it is worth what a basin is worth.
    pub fn wash(&mut self, amount: f32) {
        self.filth = (self.filth - amount).max(0.0);
    }

    /// Hop about on the spot for a moment, or be sick. Neither disturbs what
    /// the Bim was doing — they are things that happen *to* it — so both are
    /// refused outright while it is in the middle of a scripted step, where
    /// the pose belongs to the chain.
    pub fn antic(&mut self, action: Action, seconds: f32) {
        if self.scripted || self.seated || self.dead || self.napping || self.unconscious {
            return;
        }
        self.set_action(action);
        self.antic = seconds;
    }

    /// Whether one is running, so the game does not start another over it.
    pub fn in_antic(&self) -> bool {
        self.antic > 0.0
    }

    /// What the hands are doing this instant, for the probes.
    #[allow(dead_code)]
    pub fn action(&self) -> Action {
        self.action
    }

    // --- behaviour ------------------------------------------------------

    fn begin_walk(&mut self, rng: &mut Rng) {
        self.activity = Activity::Walking;
        self.timer = rng.range(1.0, 3.4);
        // Most course changes are small; now and then the Bim changes its mind
        // completely. That mix is what reads as "wandering" rather than "jitter".
        let turn = if rng.chance(REVERSAL_CHANCE) {
            rng.signed() * PI
        } else {
            rng.gaussian() * 0.8
        };
        self.intent = wrap_angle(self.intent + turn);
        self.target_speed = self.pace * rng.range(34.0, 78.0);
    }

    fn begin_pause(&mut self, rng: &mut Rng) {
        self.activity = Activity::Pausing;
        self.timer = rng.range(0.4, 1.8);
        self.target_speed = 0.0;
    }

    /// How hard the room is pushing the Bim around — walls it is too close to
    /// and furniture it is about to walk into — and which way. `None` once it
    /// is out in open floor.
    fn avoid_push(&self, interior: Rect, solids: &[Rect]) -> Option<(f32, f32)> {
        let margin = clamp(
            interior.width().min(interior.height()) * EDGE_MARGIN_FRAC,
            EDGE_MARGIN_MIN,
            EDGE_MARGIN_MAX,
        );
        let mut push = Vec2::ZERO;
        if self.pos.x < interior.min.x + margin {
            push.x += (interior.min.x + margin - self.pos.x) / margin;
        }
        if self.pos.x > interior.max.x - margin {
            push.x -= (self.pos.x - (interior.max.x - margin)) / margin;
        }
        if self.pos.y < interior.min.y + margin {
            push.y += (interior.min.y + margin - self.pos.y) / margin;
        }
        if self.pos.y > interior.max.y - margin {
            push.y -= (self.pos.y - (interior.max.y - margin)) / margin;
        }

        // Furniture pushes too, so the Bim walks around the table rather than
        // bumping along it.
        for solid in solids {
            let away = self.pos - solid.nearest(self.pos);
            let distance = away.len();
            if distance < AVOID_RANGE {
                let strength = 1.0 - distance / AVOID_RANGE;
                let dir = if distance > 0.01 {
                    away * (1.0 / distance)
                } else {
                    vec2(0.0, 1.0)
                };
                push += dir * (strength * 1.6);
            }
        }

        let inward = push.normalize_or_zero();
        if inward == Vec2::ZERO {
            return None;
        }
        // Ease in, so the turn begins as a suggestion and ends as a decision.
        // A straight linear blend leaves the Bim skimming along the wall.
        let t = clamp(push.len(), 0.0, 1.0);
        Some((t * t * (3.0 - 2.0 * t), inward.angle()))
    }

    /// The Bim's own plans: pick a new leg when the current one runs out, and
    /// turn away from walls and furniture. Returns the heading it wants.
    fn wander(&mut self, dt: f32, interior: Rect, solids: &[Rect], rng: &mut Rng) -> f32 {
        self.timer -= dt;
        if self.timer <= 0.0 {
            match self.activity {
                Activity::Pausing => self.begin_walk(rng),
                Activity::Walking => {
                    if rng.chance(PAUSE_CHANCE) {
                        self.begin_pause(rng)
                    } else {
                        self.begin_walk(rng)
                    }
                }
                Activity::Marching => {}
            }
        }

        match self.avoid_push(interior, solids) {
            None => self.intent,
            Some((strength, inward)) => {
                // Talk the Bim out of its plan as well as its heading. Steering
                // the heading alone would send it straight back at the wall the
                // moment it came clear, and it would hug the edge for ages.
                self.intent = angle_lerp(self.intent, inward, approach(INTENT_RATE * strength, dt));
                angle_lerp(self.intent, inward, strength)
            }
        }
    }

    /// Walk the planned route, waypoint by waypoint. Obstacle steering is
    /// deliberately skipped here: the path was planned around the furniture
    /// already, so steering could only argue with it.
    fn follow_order(&mut self) -> f32 {
        let Some(&target) = self.path.first() else {
            self.activity = Activity::Pausing;
            self.timer = 0.0;
            return self.intent;
        };

        let last_leg = self.path.len() == 1;
        let to = target - self.pos;
        let distance = to.len();

        // Corners are rounded rather than stopped at, so a long route flows
        // instead of stuttering at every turn.
        let reached = if last_leg {
            ARRIVE_RADIUS
        } else {
            WAYPOINT_RADIUS
        };
        if distance <= reached {
            self.path.remove(0);
            if self.path.is_empty() {
                // Arrived. Stand for a beat, then go back to wandering.
                self.activity = Activity::Pausing;
                self.timer = if self.scripted { 0.0 } else { ARRIVE_SETTLE };
                self.target_speed = 0.0;
            }
            return self.intent;
        }

        self.intent = to.angle();
        // Ease off on the final approach so it settles on the spot instead of
        // overshooting and circling back. Intermediate corners keep full speed.
        // A fall back runs at its own share of the pace on top of that
        // (feature 84): backing away is slower than walking, a sprint
        // for the ship is faster.
        self.target_speed = self.pace
            * self.falling_back.map_or(1.0, FallBack::pace)
            * if last_leg {
                MARCH_SPEED * clamp(distance / SLOWDOWN_RADIUS, 0.3, 1.0)
            } else {
                MARCH_SPEED
            };
        self.intent
    }

    /// Stand where you are, turning to any direction a task asked for.
    fn hold_still(&mut self) -> f32 {
        self.target_speed = 0.0;
        self.face_target.unwrap_or(self.heading)
    }

    pub fn update(&mut self, dt: f32, interior: Rect, solids: &[Rect], rng: &mut Rng) {
        if self.dead || self.napping || self.unconscious {
            // Nothing moves, but the clock still runs so the shadow, the
            // breathing and the selection ring do not freeze mid-pulse.
            self.idle += dt;
            self.select_pulse = (self.select_pulse + dt * 2.2) % TAU;
            self.action_phase += dt;
            return;
        }
        // A task outranks a player order, which outranks the Bim's own plans.
        let goal = if self.seated {
            self.hold_still()
        } else if self.activity == Activity::Marching {
            self.follow_order()
        } else if self.scripted || self.recruited || self.post.is_some() || self.lingering {
            // Recruited, or posted somewhere, it waits to be told. The wander
            // is the one thing it does unprompted, so that is the one thing
            // being under orders takes away.
            self.hold_still()
        } else {
            self.wander(dt, interior, solids, rng)
        };
        // Backing away, the facing and the feet part company (feature
        // 84): the body turns to the enemy the fall back named and the
        // feet carry it along the route all the same. Every other walk
        // there is goes where it is looking.
        let backing = match self.falling_back {
            Some(FallBack::Backwards(face)) if self.activity == Activity::Marching => Some(face),
            _ => None,
        };
        self.heading = angle_lerp(
            self.heading,
            backing.unwrap_or(goal),
            approach(TURN_RATE, dt),
        );

        // Ease the speed so starts and stops have weight.
        let step = ACCEL * dt;
        self.speed += clamp(self.target_speed - self.speed, -step, step);

        if !self.seated {
            let along = Vec2::from_angle(backing.map_or(self.heading, |_| self.intent));
            self.pos += along * (self.speed * dt);
            // Keep clear of the walls, then shove out of anything walked into.
            self.pos = interior.expand(-BODY_MARGIN).nearest(self.pos);
            for solid in solids {
                if let Some(out) = solid.push_out(self.pos, BODY_MARGIN) {
                    self.pos += out;
                }
            }
        }

        // The legs run the other way round while it backs off, so the
        // walk cycle reads as stepping backwards rather than forwards.
        let paces = self.speed * dt * 0.10;
        self.stride = (self.stride + if backing.is_some() { -paces } else { paces }) % TAU;
        self.idle += dt;
        self.select_pulse = (self.select_pulse + dt * 2.2) % TAU;
        self.action_phase += dt;

        // A hop or a heave runs itself down and puts the Bim back to standing.
        if self.antic > 0.0 {
            self.antic -= dt;
            if self.antic <= 0.0 {
                self.antic = 0.0;
                if matches!(
                    self.action,
                    Action::Fidget | Action::Retch | Action::Swing | Action::Punch
                ) {
                    self.set_action(Action::None);
                }
            }
        }
    }

    // --- rendering ------------------------------------------------------

    /// Forward reach of each arm, and where a held tool sits, for the current
    /// action. Local frame: `+x` is forward, `+y` is the Bim's right.
    fn pose(&self, swing: f32, moving: f32) -> Pose {
        let p = self.action_phase;
        match self.action {
            // With a weapon drawn and nothing else to do, both hands are
            // on the gun out in front and the arms reach them from the
            // shoulders. What `left` and `right` say here is only where
            // the gun is held: they barely alternate, so what the walk
            // gives is a sway of the muzzle, not a swing of the arms.
            Action::None if self.armed.is_some() => Pose {
                left: 12.0 - swing * 1.5 * moving,
                right: 12.0 + swing * 1.5 * moving,
                tool: vec2(15.0, 12.0),
                tool_rot: 0.0,
                reach: 0.6,
            },
            Action::None => Pose {
                left: -swing * 5.0 * moving,
                right: swing * 5.0 * moving,
                tool: vec2(15.0, 12.0),
                tool_rot: 0.0,
                reach: 0.0,
            },
            Action::Reach => {
                // Ramp the arms out over a moment rather than snapping straight.
                let r = clamp(p / 0.25, 0.0, 1.0);
                Pose {
                    left: 7.0 * r,
                    right: 7.0 * r,
                    tool: vec2(14.0 + 12.0 * r, 10.0),
                    tool_rot: 0.0,
                    reach: r,
                }
            }
            Action::Chop => {
                // One stroke per CHOP_PERIOD: down fast, back up.
                let t = (p / CHOP_PERIOD) % 1.0;
                let drop = (t * TAU).sin();
                Pose {
                    left: 6.0,
                    right: 12.0 + drop * 5.0,
                    // Offset to the working hand so the blade clears the head.
                    tool: vec2(26.0 + drop * 5.0, 10.0),
                    tool_rot: -0.5 + drop * 0.5,
                    reach: 1.0,
                }
            }
            Action::Serve => {
                // Sweep across from the pot to the plate and back.
                let t = (p / SCOOP_PERIOD) % 1.0;
                let sweep = (t * TAU).sin();
                Pose {
                    left: 4.0,
                    right: 12.0,
                    // Wide enough to visibly travel from the pot to the plate.
                    tool: vec2(29.0, sweep * 19.0),
                    tool_rot: sweep * 0.4,
                    reach: 1.0,
                }
            }
            Action::Wash => {
                // Hands held together out in front, working over each other:
                // one arm forward as the other comes back, in a small circle.
                let turn = p * TAU / SCRUB_PERIOD;
                Pose {
                    left: 9.0 + turn.sin() * 2.5,
                    right: 9.0 - turn.sin() * 2.5,
                    tool: vec2(20.0, 0.0),
                    tool_rot: 0.0,
                    reach: 1.0,
                }
            }
            Action::Sweep => {
                // Both hands on the pole, the head of the broom travelling
                // side to side across the deck in front. Seen from above that
                // is the arms swinging together rather than alternately, which
                // is what tells it apart from a walk.
                let swing = (p * TAU / SWEEP_PERIOD).sin();
                Pose {
                    left: 7.0 + swing * 2.0,
                    right: 7.0 - swing * 2.0,
                    tool: vec2(26.0, swing * 16.0),
                    tool_rot: swing * 0.5,
                    reach: 1.0,
                }
            }
            Action::Sleep => {
                // Arms in at the sides, lifting a little with each breath.
                let breath = (p * TAU / BREATH_PERIOD).sin();
                Pose {
                    left: -3.0 + breath,
                    right: -3.0 + breath,
                    tool: vec2(15.0, 12.0),
                    tool_rot: 0.0,
                    reach: 0.0,
                }
            }
            Action::Eat => {
                // Out to the plate, back to the mouth.
                let t = (p / BITE_PERIOD) % 1.0;
                let near = ((t * TAU).cos() * 0.5 + 0.5).powf(1.4);
                Pose {
                    left: 5.0,
                    right: 8.0 + near * 4.0,
                    tool: vec2(lerp(26.0, 11.0, near), 5.0),
                    tool_rot: near * 0.6,
                    reach: 0.6,
                }
            }
            Action::Fidget => {
                // Arms tucked in and swapping, the way you do.
                let turn = (p * TAU / HOP_PERIOD).sin();
                Pose {
                    left: 2.0 + turn * 3.0,
                    right: 2.0 - turn * 3.0,
                    tool: vec2(15.0, 12.0),
                    tool_rot: 0.0,
                    reach: 0.0,
                }
            }
            Action::Retch => {
                // Both arms forward and down, hands on the knees.
                let heave = (p * TAU / HEAVE_PERIOD).sin().abs();
                Pose {
                    left: 6.0 + heave * 3.0,
                    right: 6.0 + heave * 3.0,
                    tool: vec2(18.0, 8.0),
                    tool_rot: 0.0,
                    reach: 0.8,
                }
            }
            Action::Talk => {
                // One hand up and down, the other still. Small: it is a
                // conversation, not a semaphore.
                let wave = (p * TAU / TALK_PERIOD).sin();
                Pose {
                    left: 3.0 + wave * 2.5,
                    right: 1.0,
                    tool: vec2(15.0, 12.0),
                    tool_rot: 0.0,
                    reach: 0.25 + 0.15 * wave.max(0.0),
                }
            }
            Action::Swing => {
                // The blade hand comes across the body and sweeps out in
                // front: the right arm follows the blade, the left braces.
                let t = clamp(p / SWING_TIME, 0.0, 1.0);
                let sweep = (t * PI).sin();
                Pose {
                    left: 4.0,
                    right: 8.0 + sweep * 6.0,
                    tool: vec2(15.0, 12.0),
                    tool_rot: 0.0,
                    reach: 0.5 + 0.5 * sweep,
                }
            }
            Action::Punch => {
                // Out and back with the right, the left up as a guard.
                let t = clamp(p / SWING_TIME, 0.0, 1.0);
                let jab = (t * PI).sin();
                Pose {
                    left: 6.0,
                    right: 4.0 + jab * 14.0,
                    tool: vec2(15.0, 12.0),
                    tool_rot: 0.0,
                    reach: 0.4,
                }
            }
            Action::Bandage => {
                // The hands go round one another in front of the body, one
                // forward as the other comes back — a wash's circle, held
                // a little further out so the wrap has room to show between
                // them. `tool_rot` carries the turn for the roll to follow.
                let turn = p * TAU / WRAP_PERIOD;
                Pose {
                    left: 11.0 + turn.cos() * 4.0,
                    right: 11.0 - turn.cos() * 4.0,
                    tool: vec2(22.0, 0.0),
                    tool_rot: turn,
                    reach: 1.0,
                }
            }
        }
    }

    /// Whether player `slot` has this Bim selected.
    pub fn is_selected_by(&self, slot: u32) -> bool {
        slot < 32 && self.selected & (1 << slot) != 0
    }

    /// Select this Bim for player `slot`, or drop it from their selection.
    pub fn select_for(&mut self, slot: u32, on: bool) {
        if slot >= 32 {
            return;
        }
        if on {
            self.selected |= 1 << slot;
        } else {
            self.selected &= !(1 << slot);
        }
    }

    /// The colour a player's own Bim is ringed in (feature 84), or
    /// `None` for anybody nobody steers.
    pub fn tint(&self) -> Option<Tint> {
        self.tint
    }

    /// Say it, or take it away.
    pub fn set_tint(&mut self, tint: Option<Tint>) {
        self.tint = tint;
    }

    /// What this Bim looks like.
    pub fn look(&self) -> Look {
        self.look
    }

    /// Give it another look: the hair a player chose, or a whole look
    /// dealt afresh. Drawing only.
    pub fn set_look(&mut self, look: Look) {
        self.look = look;
    }

    /// The scale the standing figure is drawn at: [`BODY_SCALE`] by the
    /// look's build, so a sturdy Bim's hands, held things and weapon are
    /// as much bigger as its body. Every draw method goes through this;
    /// the reach and the hit test do not.
    fn body_scale(&self) -> f32 {
        BODY_SCALE * self.look.scale() * self.outfit.scale()
    }

    /// Whether a weapon is up in both hands, which is when the arms are
    /// drawn as limbs reaching for it rather than as a blob a side.
    fn arms_on_a_weapon(&self) -> bool {
        self.armed.is_some() && !self.dead
    }

    /// One arm, from the shoulder on `side` out to a hand at `to` — both
    /// in the body's own frame, since that is where a shoulder is. Two
    /// segments with the elbow swung out away from the body between them
    /// ([`ARM_BEND`]). Every rim goes down before any of the fills, or
    /// the shoulder's own rim would cut the sleeve in half.
    fn draw_arm(&self, b: &mut Brush, side: f32, to: Vec2) {
        let shoulder = vec2(-1.0, SHOULDER_ACROSS * side);
        let span = to - shoulder;
        let bend = ARM_BEND * (1.0 - clamp(span.len() / ARM_REACH, 0.0, 1.0));
        let out = vec2(-span.y, span.x).normalize_or_zero() * (bend * side);
        let elbow = shoulder + span * 0.5 + out;
        let sleeve = self.outfit.dye(self.uniform.sleeve(), self.uniform);
        for (wide, c) in [(SLEEVE_RIM, OUTLINE), (SLEEVE_WIDE, sleeve)] {
            for (from, at) in [(shoulder, elbow), (elbow, to)] {
                let d = at - from;
                b.rect(from + d * 0.5, vec2(d.len(), wide), d.angle(), 0.0, c);
            }
            for at in [shoulder, elbow, to] {
                b.ellipse(at, vec2(wide, wide), 0.0, c);
            }
        }
    }

    /// Where the two hands are on the weapon in hand, in the body's own
    /// frame: the **rear** hand first — the one the right shoulder's arm
    /// reaches — then the forward one. [`Character::draw`] runs the arms
    /// out to these before the head goes on and
    /// [`Character::draw_weapon`] lays the weapon over the lot, so what
    /// is in front of a Bim's face is its gun and not its own elbow.
    fn weapon_hands(&self, pose: Pose, weapon: WeaponKind) -> [Vec2; 2] {
        let grip = self.weapon_grip(pose);
        if weapon == WeaponKind::Schword {
            if self.action == Action::Swing {
                // Along the blade's own line, which is swept round; the
                // offsets come back into the body's frame turned by it.
                let angle = SWING_ARC * (self.swing_through() - 0.5);
                return [vec2(4.0, 3.0).rotate(angle), vec2(11.0, -3.0).rotate(angle)];
            }
            let hilt = grip + vec2(2.0, 0.0);
            return [hilt + vec2(-1.0, 4.2), hilt + vec2(5.5, -4.2)];
        }
        [
            self.on_gun(grip, vec2(0.0, 4.6)),
            self.on_gun(grip, vec2(fore_hand(weapon), -4.6)),
        ]
    }

    /// The grip, in the body's own frame: as far forward as the two arms
    /// average and over towards the firing shoulder. What is left of the
    /// arms' alternation is a sway of the muzzle to one side and back,
    /// not a swing.
    ///
    /// Far enough forward that what is behind the grip — a stock, a butt
    /// — stops in front of the face rather than over it: the head is
    /// drawn before the weapon and a gun laid across it hides the hair,
    /// the nose and whatever the class wears on it (feature 81's
    /// sunglasses and helm bands).
    fn weapon_grip(&self, pose: Pose) -> Vec2 {
        let hold = (pose.left + pose.right) * 0.5;
        let sway = (pose.right - pose.left) * 0.25;
        vec2(hold + GRIP_AHEAD, GRIP_ACROSS + sway)
    }

    /// A point on the gun, in the body's own frame: the gun's grip at
    /// `grip` with its barrel along `+x` from there, laid at
    /// [`Character::gun_rot`] off the facing and at [`GUN_SCALE`] of the
    /// body's own size.
    fn on_gun(&self, grip: Vec2, local: Vec2) -> Vec2 {
        grip + (local * GUN_SCALE).rotate(self.gun_rot(grip))
    }

    /// How far off the facing the gun lies, in the body's own frame:
    /// with nothing aimed at, the carry ([`CARRY`], angled across the
    /// front); aiming, **the line from the grip to what is aimed at**,
    /// held within [`AIM_ACROSS`] of the facing. Since the barrel runs
    /// from the grip through the target, a shot from the muzzle
    /// ([`Character::muzzle`]) leaves along the barrel and lands where
    /// the aim said — the picture and the shot are the same line
    /// (feature 84).
    fn gun_rot(&self, grip: Vec2) -> f32 {
        let Some(aim) = self.aim else {
            return -CARRY;
        };
        let at = self.drawn_at() + (grip * self.body_scale()).rotate(self.heading);
        let to = aim - at;
        if to.len() < 1e-3 {
            return -CARRY;
        }
        clamp(
            wrap_angle(to.angle() - self.heading),
            -AIM_ACROSS,
            AIM_ACROSS,
        )
    }

    /// The pose as it stands this instant — what [`Character::draw`]
    /// lays the body out from, asked again by anything outside the
    /// drawing that needs to know where a hand is
    /// ([`Character::muzzle`]).
    fn current_pose(&self) -> Pose {
        self.pose(self.stride.sin(), clamp(self.speed / 60.0, 0.0, 1.0))
    }

    /// Where the emitter of the gun in its hands is, in room units: the
    /// point a shot leaves (feature 84). `None` with no gun up — bare
    /// hands, a blade, a body down — and the shooter's own position is
    /// what the fight falls back on then.
    pub fn muzzle(&self) -> Option<Vec2> {
        if self.dead || self.unconscious {
            return None;
        }
        let weapon = self.armed.filter(|w| muzzle_ahead(*w) > 0.0)?;
        let grip = self.weapon_grip(self.current_pose());
        let at = self.on_gun(grip, vec2(muzzle_ahead(weapon), 0.0));
        Some(self.drawn_at() + (at * self.body_scale()).rotate(self.heading))
    }

    /// Where a hit on `part` shows and how big (feature 98's flash on the
    /// part struck), in room units: the head, the middle of the coverall
    /// behind it, and the legs trailing under the body — the standing
    /// figure's own places, turned with it and at its scale. Drawing only.
    pub fn part_mark(&self, part: crate::health::Part) -> (Vec2, f32) {
        use crate::health::Part;
        let (local, radius) = match part {
            Part::Head => (vec2(2.5, 0.0), 6.5),
            Part::Body => (vec2(-4.0, 0.0), 9.0),
            Part::Legs => (vec2(-12.0, 0.0), 6.5),
        };
        let scale = self.body_scale();
        (
            self.drawn_at() + (local * scale).rotate(self.heading),
            radius * scale,
        )
    }

    /// How far through its sweep a swing is, nought to one.
    fn swing_through(&self) -> f32 {
        clamp(self.action_phase / SWING_TIME, 0.0, 1.0)
    }

    /// Draw the body, with the rings player `viewer` sees under it: the
    /// selection ring is theirs alone, since a selection is one player's.
    pub fn draw(&self, list: &mut DrawList, viewer: u32) {
        if self.dead {
            self.draw_fallen(list);
            return;
        }
        self.draw_rings(list, viewer);
        if self.unconscious {
            self.draw_lying(list);
            return;
        }
        let swing = self.stride.sin();
        let moving = clamp(self.speed / 60.0, 0.0, 1.0);
        // Breathing while still, a light bounce while walking.
        let bob = lerp(
            (self.idle * 1.8).sin() * 0.4,
            (self.stride * 2.0).cos() * 0.8,
            moving,
        );
        // Idle glancing about; suppressed once the Bim is busy or going somewhere.
        let look = if self.action == Action::None {
            (self.idle * 0.9).sin() * 0.30 * (1.0 - moving)
        } else {
            0.0
        };
        let pose = self.current_pose();

        // Hopping on the spot, or doubled over. Seen from above a jump is the
        // figure growing and its shadow shrinking away underneath it, and a
        // heave is the reverse: hunched down and small.
        let (hop, heave) = match self.action {
            Action::Fidget => (
                (self.action_phase * TAU / HOP_PERIOD).sin().max(0.0),
                0.0f32,
            ),
            Action::Retch => (0.0, (self.action_phase * TAU / HEAVE_PERIOD).sin().abs()),
            _ => (0.0, 0.0),
        };
        // And braced (feature 91): settled down onto the feet, which with
        // the shadow underneath unchanged is a body drawn *lower* — the
        // one thing a picture seen from directly above can say about a
        // stance held.
        let lift = (1.0 + 0.16 * hop - 0.10 * heave) * if self.braced { BRACE_CROUCH } else { 1.0 };
        let scale = self.body_scale() * lift;
        // Leant out to the peek while aiming from one; the body itself
        // has not moved, and nothing but the picture knows.
        let pos = self.drawn_at();

        // Cast under the body and turned with it, so the halo always fits,
        // and soft at its edge (feature 98). It pulls in and darkens as the
        // Bim leaves the deck.
        list.soft_ellipse(
            pos + vec2(0.0, (4.5 + 7.0 * hop) * self.body_scale()),
            vec2(28.0, 36.0) * self.body_scale() * (1.0 - 0.22 * hop),
            self.heading,
            SHADOW,
        );

        let mut b = list.brush(pos, self.heading, scale);

        // Boots, under the body: one strides forward as the other trails. A
        // seated Bim tucks them in. **Braced** (feature 91) they are
        // planted instead — set wide, turned out and a little back under
        // the body, with no stride left in them whatever the legs were
        // doing the step before — so a soldier holding its ground reads as
        // a stance and not only as the brackets drawn round it.
        if !self.seated {
            let planted = self.braced;
            for side in [-1.0f32, 1.0] {
                let (at, splay) = if planted {
                    (
                        vec2(-BRACE_SET_BACK, BRACE_FEET_APART * side),
                        BRACE_TOE_OUT * side,
                    )
                } else {
                    (vec2(swing * 8.0 * side * moving, 7.0 * side), 0.0)
                };
                b.ellipse(at, vec2(13.5, 9.0), splay, BOOT);
                // Leg guards: the boot darker, with a band across the shin.
                if let Some(guard) = self.armour[2] {
                    b.ellipse(at, vec2(13.5, 9.0), splay, GUARD);
                    b.rect(at - vec2(2.5, 0.0), vec2(3.0, 9.0), splay, 0.0, GUARD_BAND);
                    if guard.broken {
                        b.rect(at, vec2(10.0, 1.3), 0.7 + splay, 0.0, CRACK);
                    }
                }
                // A wounded leg bleeds onto the boot.
                if self.wounds[2] {
                    b.ellipse(at - vec2(2.0, 0.0), vec2(8.0, 6.0), splay, BLOOD);
                }
            }
        }

        // Arms with a weapon up: limbs off the shoulders, each reaching
        // for its own hand on the gun (feature 82), and drawn **under**
        // the torso — an arm seen from directly above is mostly shoulder,
        // and laid over the coverall the two of them hide the body they
        // belong to. What shows is the forearm past the chest, which is
        // what there is to see. Only the gun goes on top, at the end.
        if let Some(weapon) = self.armed.filter(|_| !self.dead) {
            let [rear, fore] = self.weapon_hands(pose, weapon);
            self.draw_arm(&mut b, 1.0, rear);
            self.draw_arm(&mut b, -1.0, fore);
        }

        // Torso: broad across the shoulders, shallow front to back, with a dark
        // rim behind it so the silhouette holds up against any floor colour.
        // Asleep the chest rises and falls; it is the only thing moving, so
        // without it the Bim reads as switched off rather than resting.
        let breath = match self.action {
            Action::Sleep => 1.0 + 0.035 * (self.action_phase * TAU / BREATH_PERIOD).sin(),
            _ => 1.0,
        };
        b.ellipse(Vec2::ZERO, vec2(25.0, 33.0) * breath, 0.0, OUTLINE);
        b.ellipse(
            Vec2::ZERO,
            vec2(22.0, 30.0) * breath,
            0.0,
            self.outfit.dye(self.uniform.shirt(), self.uniform),
        );
        // The yoke across the shoulders, in the wearer's own colour — the
        // coverall is the ship's or the station's and says nothing about
        // who is in it.
        b.ellipse(
            vec2(-6.5, 0.0),
            vec2(7.0, 24.0) * breath,
            0.0,
            self.look.trim(),
        );
        // The vest: a dark plate over the torso, set forward so the yoke
        // still shows at the collar behind it, strapped on at the sides.
        if let Some(vest) = self.armour[1] {
            b.ellipse(vec2(3.0, 0.0), vec2(16.0, 24.0) * breath, 0.0, KEVLAR);
            for side in [-1.0f32, 1.0] {
                b.rect(
                    vec2(-3.0, 8.5 * side),
                    vec2(5.0, 2.5),
                    0.0,
                    0.0,
                    KEVLAR_STRAP,
                );
            }
            if vest.broken {
                b.rect(vec2(3.0, 0.0), vec2(20.0, 1.4), 0.9, 0.0, CRACK);
            }
        }

        // A wound on the body: a blotch in the middle of the coverall.
        if self.wounds[1] {
            b.ellipse(vec2(1.0, 0.0), vec2(11.0, 9.0), 0.3, BLOOD);
        }

        // What it has got on itself. Down the front and around the legs, where
        // it would be, and in the same colour as the mess on the deck so the
        // two read as the same substance.
        if self.filth > 0.001 {
            let deep = self.filth.clamp(0.0, 1.0);
            for (i, local) in [
                vec2(-4.0, 5.0),
                vec2(2.0, -6.5),
                vec2(-8.0, -3.0),
                vec2(6.0, 4.0),
            ]
            .into_iter()
            .enumerate()
            {
                // The worse it is, the more of the four show.
                if (i as f32 + 1.0) / 4.0 > deep + 0.25 {
                    continue;
                }
                let size = 7.0 + 4.0 * deep;
                b.ellipse(
                    local,
                    vec2(size, size * 0.85),
                    0.0,
                    GRIME.alpha(0.55 + 0.4 * deep),
                );
            }
        }

        // Arms: swinging while walking, reaching or working otherwise —
        // a blob a side, which is all an arm needs to be when there is
        // nothing at the end of it. With a weapon up they went down
        // before the torso instead, above.
        if !self.arms_on_a_weapon() {
            for (side, forward) in [(-1.0f32, pose.left), (1.0f32, pose.right)] {
                let at = vec2(forward - 1.0, SHOULDER_ACROSS * side);
                b.ellipse(at, vec2(SLEEVE_RIM, SLEEVE_RIM), 0.0, OUTLINE);
                b.ellipse(
                    at,
                    vec2(SLEEVE_WIDE, SLEEVE_WIDE),
                    0.0,
                    self.outfit.dye(self.uniform.sleeve(), self.uniform),
                );
            }
        }

        // The class's kit over the chest and shoulders (feature 81), after
        // the arms so a pauldron caps the shoulder it is strapped to and a
        // strap crosses whatever is under it — the vest included.
        if self.uniform != Uniform::Suit {
            draw_class_rig(&mut b, self.outfit, breath);
        }

        // Head assembly, pivoting about the neck. Seen from above it is mostly
        // hair, with the face and nose showing at the leading edge.
        let pivot = vec2(2.5 + bob * 0.3, 0.0);
        let at = |local: Vec2| pivot + local.rotate(look);

        b.ellipse(at(Vec2::ZERO), vec2(15.5, 15.5), 0.0, OUTLINE);
        b.ellipse(at(Vec2::ZERO), vec2(13.5, 13.5), 0.0, SKIN);
        draw_hair_standing(&mut b, self.look, at, look);
        // What the class wears on its head, over the hair and behind the
        // face — the nose goes on after, so it still shows under a peak.
        // A helm is worn over the lot, and the kit is left off under it;
        // the class says itself on the helm's own band instead.
        let helmed = self.armour[0].is_some();
        if self.uniform != Uniform::Suit && !helmed {
            draw_class_head(&mut b, self.outfit, &at, look);
        }
        b.ellipse(at(vec2(5.6, 0.0)), vec2(4.0, 3.2), look, NOSE);
        // The helm: a cap over the hair, rimmed, leaving the face clear.
        if let Some(helm) = self.armour[0] {
            b.ellipse(at(vec2(-2.5, 0.0)), vec2(12.5, 14.5), look, HELM_RIM);
            b.ellipse(at(vec2(-2.5, 0.0)), vec2(10.5, 12.5), look, HELM);
            // The class's band across it, so a helmeted crew is still
            // read a class at a time.
            if let Some(c) = self.outfit.colour() {
                b.rect(at(vec2(-5.0, 0.0)), vec2(2.6, 11.0), look, 1.0, c);
            }
            if helm.broken {
                b.rect(at(vec2(-2.5, 0.0)), vec2(11.0, 1.2), look + 0.8, 0.0, CRACK);
            }
        }
        // The soldier's sunglasses, over the eyes and over a helm: the one
        // piece of kit nothing is worn on top of.
        // A lens either side of a bridge, each a little wider than the
        // head, with a glint along the left one.
        if self.outfit == Outfit::Soldier && self.uniform != Uniform::Suit {
            b.rect(at(vec2(3.9, 0.0)), vec2(2.6, 13.5), look, 1.2, SHADES_RIM);
            for side in [-1.0f32, 1.0] {
                b.ellipse(at(vec2(3.9, 3.6 * side)), vec2(5.4, 6.6), look, SHADES_RIM);
                b.ellipse(at(vec2(3.9, 3.6 * side)), vec2(4.2, 5.2), look, SHADES);
            }
            b.rect(at(vec2(4.6, -4.2)), vec2(1.3, 2.8), look, 0.5, SHADES_GLINT);
        }
        // A wound on the head: a blotch over the crown — on the helm, if
        // one is worn, since the shot went through it.
        if self.wounds[0] {
            b.ellipse(at(vec2(-1.0, 2.0)), vec2(7.0, 6.0), look, BLOOD);
        }
        // The visor over all of that, in the suit: a helmet from above is
        // a bigger circle than the head, and the face shows through it.
        if self.uniform == Uniform::Suit {
            b.ellipse(at(Vec2::ZERO), vec2(18.0, 18.0), 0.0, OUTLINE);
            b.ellipse(at(Vec2::ZERO), vec2(16.5, 16.5), 0.0, VISOR.alpha(0.55));
        }

        self.draw_held(list, pose);

        if self.action == Action::Sleep {
            self.draw_zs(list);
        }
    }

    /// Zs drifting up off a sleeping Bim, each one rising and fading as the
    /// next sets off. Placed in the room rather than on the body, so they go
    /// the same way whichever way the Bim is lying.
    fn draw_zs(&self, list: &mut DrawList) {
        const COUNT: usize = 3;
        let from = self.pos + vec2(52.0, -18.0);
        for i in 0..COUNT {
            let t = (self.action_phase / BREATH_PERIOD + i as f32 / COUNT as f32) % 1.0;
            let at = from + vec2(20.0 * t, -38.0 * t);
            // In and out again, so none of them pops.
            let fade = (t * PI).sin();
            draw_z(list, at, 11.0 + 9.0 * t, SLEEP_Z.alpha(0.85 * fade));
        }
    }

    /// The rings on the deck under a living Bim: selected, an enemy, under
    /// orders. Drawn before the body, standing or lying.
    fn draw_rings(&self, list: &mut DrawList, viewer: u32) {
        let pos = self.drawn_at();
        if let Some(tint) = self.tint {
            // A player's own Bim, in that player's colour (feature 84):
            // a steady filled circle on the deck under it — nothing
            // about it breathes, since it says whose the Bim is rather
            // than what is happening to it. A bot has none.
            let c = tint.colour();
            let span = 44.0 * self.body_scale();
            list.circle(pos, span, c.alpha(0.16));
            list.ring(pos, span, 2.5, c.alpha(0.9));
        }

        if self.is_selected_by(viewer) {
            // And a selection is a pale ring and nothing else: on a bot
            // it is the only ring there is, so it must not read as an
            // order given.
            let pulse = (1.0 + self.select_pulse.sin() * 0.04) * self.body_scale();
            list.circle(pos, 40.0 * pulse, ACCENT.alpha(0.08));
            list.ring(pos, 40.0 * pulse, 1.5, ACCENT.alpha(0.55));
        }

        if self.hostile {
            // An enemy is ringed in the colour its shots are, thin and
            // steady: a warning, not a selection.
            list.ring(pos, 46.0 * self.body_scale(), 2.0, ENEMY.alpha(0.75));
        }

        if self.braced {
            // Braced (feature 75): four heavy brackets square round the
            // body, steady — nothing about it turns or breathes — a shade
            // darker than the recruit ring they sit inside.
            let span = 44.0 * self.body_scale();
            for i in 0..4 {
                let a = i as f32 * (TAU / 4.0) + PI * 0.25;
                let at = pos + Vec2::from_angle(a) * (span * 0.5);
                list.rect(at, vec2(5.0, 16.0), a, 1.0, BRACED.alpha(0.95));
            }
        }

        if self.surging {
            // A medic's surge (feature 76): a wide pale halo breathing
            // slowly round the body, outside every other ring, in the
            // beam's own green.
            let pulse = (1.0 + (self.select_pulse * 0.7).sin() * 0.06) * self.body_scale();
            list.circle(pos, 58.0 * pulse, SURGE.alpha(0.16));
            list.ring(pos, 58.0 * pulse, 3.0, SURGE.alpha(0.9));
        }

        if self.recruited && self.tint.is_some() {
            // A wider ring outside the selection one, broken into four arcs so
            // the two never read as the same thing. Shown whether or not the
            // Bim is selected: being under orders outlasts a click elsewhere.
            //
            // **A player's own only** (feature 84): under the alarm every
            // bot aboard is recruited, and fourteen of these was a deck
            // of rings with nothing to read off it. A bot's state is the
            // banner it is walking to and the gun in its hand.
            let pulse = (1.0 + self.select_pulse.sin() * 0.05) * self.body_scale();
            let span = 52.0 * pulse;
            list.ring(pos, span, 1.5, COMMAND.alpha(0.35));
            for i in 0..4 {
                let a = i as f32 * (TAU / 4.0) + self.select_pulse * 0.25;
                let at = pos + Vec2::from_angle(a) * (span * 0.5);
                list.rect(at, vec2(9.0, 3.0), a + PI * 0.5, 1.5, COMMAND.alpha(0.9));
            }
        }
    }

    /// Face down where it dropped. Drawn cold and flat — no bob, no breath,
    /// no glancing about — because every other state has one of those, and
    /// the absence is what reads as dead.
    fn draw_fallen(&self, list: &mut DrawList) {
        self.draw_flat(
            list,
            self.body_scale(),
            (GONE_SHIRT, GONE_SLEEVE, GONE_SKIN, GONE_HAIR),
        );
    }

    /// Out cold: the same figure as a fallen one, in its own colours, a
    /// tenth bigger ([`OUT_COLD_SCALE`]), and breathing — a slow swell of
    /// the whole body, since from above a chest rising is the outline
    /// growing. No Zs: this is not sleep. The size, the colour and the
    /// breath are what say alive; the blotches say why it is down.
    fn draw_lying(&self, list: &mut DrawList) {
        let breath = 1.0 + 0.03 * (self.idle * TAU / BREATH_PERIOD).sin();
        self.draw_flat(
            list,
            self.body_scale() * OUT_COLD_SCALE * breath,
            (
                self.outfit.dye(self.uniform.shirt(), self.uniform),
                self.outfit.dye(self.uniform.sleeve(), self.uniform),
                SKIN,
                self.look.hair(),
            ),
        );
    }

    /// The figure stretched out on the deck, in the given shirt, sleeve,
    /// skin and hair, with the blood on whichever parts bleed. Seen from
    /// above a body lying down is long rather than round: the legs trail
    /// out behind the hips to the boots, the torso is longer than it is
    /// wide, one arm is flung out past the head and the other lies along
    /// the side, and the head at the far end is turned onto its cheek.
    /// The limbs are drawn **short**: from directly above, an arm and a
    /// leg are mostly foreshortened away, and at full reach the sprawl
    /// read as a figure seen standing rather than one lying on the deck.
    /// Head forward, the way it was facing when it went down, so it reads
    /// as having fallen where it stood. Drawn at [`FLAT_SCALE`] of the
    /// standing figure: about a tile long, and it is the shape that says
    /// "down" at a glance, not the size.
    fn draw_flat(&self, list: &mut DrawList, scale: f32, colours: (Color, Color, Color, Color)) {
        let (shirt, sleeve, skin, hair) = colours;
        let scale = scale * FLAT_SCALE;
        list.soft_ellipse(
            self.pos + vec2(3.0, 5.0) * FLAT_SCALE,
            vec2(68.0, 36.0) * self.body_scale() * FLAT_SCALE,
            self.heading,
            SHADOW,
        );
        let mut b = list.brush(self.pos, self.heading, scale);

        // Legs, under the torso: from the hips back to the boots, a little
        // apart, each with a boot at its end and the guard over the shin.
        for side in [-1.0f32, 1.0] {
            let splay = 0.09 * side;
            b.ellipse(vec2(-17.0, 6.5 * side), vec2(26.0, 10.5), splay, OUTLINE);
            b.ellipse(vec2(-17.0, 6.5 * side), vec2(24.0, 8.5), splay, shirt);
            b.ellipse(vec2(-28.0, 8.0 * side), vec2(10.0, 8.5), splay, BOOT);
            if let Some(guard) = self.armour[2] {
                b.ellipse(vec2(-23.0, 7.5 * side), vec2(12.0, 8.0), splay, GUARD);
                b.rect(
                    vec2(-20.0, 7.0 * side),
                    vec2(2.5, 8.0),
                    splay,
                    0.0,
                    GUARD_BAND,
                );
                if guard.broken {
                    b.rect(vec2(-23.0, 7.5 * side), vec2(8.0, 1.2), 0.7, 0.0, CRACK);
                }
            }
            // A wounded leg bleeds onto the thigh.
            if self.wounds[2] {
                b.ellipse(vec2(-14.0, 6.5 * side), vec2(9.0, 6.5), splay, BLOOD);
            }
        }

        // The torso, shoulders forward, with the yoke across them.
        b.ellipse(vec2(-1.0, 0.0), vec2(38.0, 27.0), 0.0, OUTLINE);
        b.ellipse(vec2(-1.0, 0.0), vec2(34.0, 23.0), 0.0, shirt);
        b.ellipse(vec2(11.0, 0.0), vec2(8.0, 20.0), 0.0, self.look.trim());
        // The armour stays on a body that is down, the vest over the chest.
        if let Some(vest) = self.armour[1] {
            b.ellipse(vec2(2.0, 0.0), vec2(24.0, 17.0), 0.0, KEVLAR);
            if vest.broken {
                b.rect(vec2(2.0, 0.0), vec2(20.0, 1.4), 0.5, 0.0, CRACK);
            }
        }
        if self.wounds[1] {
            b.ellipse(vec2(0.0, -1.0), vec2(11.0, 9.0), 0.3, BLOOD);
        }

        // The arms: the left thrown up beside the head, the right along
        // the side with the hand by the hip — a sprawl, not a pose, and
        // both short, since an arm on the deck is seen down its length.
        let arm = |b: &mut Brush, from: Vec2, to: Vec2| {
            let mid = (from + to) * 0.5;
            let along = to - from;
            let rot = along.y.atan2(along.x);
            b.rect(mid, vec2(along.len(), 10.0), rot, 5.0, OUTLINE);
            b.rect(mid, vec2(along.len(), 8.0), rot, 4.0, sleeve);
            b.ellipse(to, vec2(11.0, 11.0), 0.0, OUTLINE);
            b.ellipse(to, vec2(9.0, 9.0), 0.0, sleeve);
        };
        arm(&mut b, vec2(10.0, -11.0), vec2(23.0, -19.0));
        arm(&mut b, vec2(8.0, 12.0), vec2(-5.0, 15.0));

        // The head, out past the shoulders, turned onto its right cheek:
        // the hair over the crown and the near side, the face showing to
        // the right. Long hair fans out on the deck behind it.
        let head = vec2(25.0, 1.0);
        draw_hair_lying(&mut b, self.look.hair, head, hair, true);
        b.ellipse(head, vec2(15.5, 15.5), 0.0, OUTLINE);
        b.ellipse(head, vec2(13.0, 13.0), 0.0, skin);
        draw_hair_lying(&mut b, self.look.hair, head, hair, false);
        b.ellipse(head + vec2(3.0, 5.5), vec2(4.0, 3.0), 1.2, NOSE);
        if let Some(helm) = self.armour[0] {
            b.ellipse(head + vec2(-1.5, -3.0), vec2(13.5, 11.5), 0.35, HELM_RIM);
            b.ellipse(head + vec2(-1.5, -3.0), vec2(11.5, 9.5), 0.35, HELM);
            if helm.broken {
                b.rect(head + vec2(-1.5, -3.0), vec2(10.0, 1.2), 1.2, 0.0, CRACK);
            }
        }
        if self.wounds[0] {
            b.ellipse(head + vec2(-1.0, -2.0), vec2(7.0, 6.0), 0.4, BLOOD);
        }
        // The visor, in the suit: the helmet is a bigger circle than the
        // head, lying where the head does.
        if self.uniform == Uniform::Suit {
            b.ellipse(head, vec2(18.0, 18.0), 0.0, OUTLINE);
            b.ellipse(head, vec2(16.5, 16.5), 0.0, VISOR.alpha(0.55));
        }
    }
    /// Whatever is in the hands, placed in front of the body.
    fn draw_held(&self, list: &mut DrawList, pose: Pose) {
        let pos = self.drawn_at();
        let to_world = |local: Vec2| pos + (local * self.body_scale()).rotate(self.heading);

        if let Some(weapon) = self.armed.filter(|_| !self.dead) {
            self.draw_weapon(list, pose, weapon);
        }

        // The bandage being wound: the roll in the right hand, the strip
        // paying out of it across to the left, and the turns already laid
        // as a ring between the hands that fills as the roll goes round —
        // so the dressing reads as progress and not as the wash's circle.
        if self.action == Action::Bandage {
            let turn = pose.tool_rot;
            let roll = to_world(vec2(pose.right + 9.0, 11.0));
            let hand = to_world(vec2(pose.left + 9.0, -11.0));
            let wrap = to_world(pose.tool);
            list.rect(
                roll,
                vec2(8.0, 11.0),
                self.heading + turn * 0.5,
                2.0,
                BANDAGE,
            );
            list.stroke_rect(
                roll,
                vec2(8.0, 11.0),
                self.heading + turn * 0.5,
                2.0,
                1.0,
                BANDAGE_EDGE,
            );
            list.line(roll, wrap, 3.0, BANDAGE);
            list.line(wrap, hand, 3.0, BANDAGE.alpha(0.85));
            // The turns laid so far: a ring of short strokes round the wrap
            // point, one more every full turn of the hands, wrapping back
            // round to the first after a few so the ring never fills solid.
            let laid = ((turn / TAU) as i32 % 6 + 1).max(1);
            for i in 0..laid {
                let a = self.heading + i as f32 * (TAU / 6.0) + turn * 0.15;
                let at = wrap + Vec2::from_angle(a) * (6.0 * self.body_scale());
                list.rect(at, vec2(7.0, 2.5), a + PI * 0.5, 1.0, BANDAGE_EDGE);
            }
        }

        match self.main {
            // The tools are drawn from the tool hand below; the main hand
            // never holds one.
            Held::Nothing | Held::Fork | Held::Pick => {}
            Held::Vegetable => {
                let at = to_world(vec2(18.0 + pose.reach * 13.0, -6.0));
                list.ellipse(at, vec2(30.0, 17.0), self.heading, VEG);
                list.ellipse(
                    at - Vec2::from_angle(self.heading) * 13.0,
                    vec2(8.0, 12.0),
                    self.heading,
                    VEG_DARK,
                );
            }
            Held::Chopped { rounds, cubes } => {
                // A handful of what came off the board, cupped in both hands:
                // rounds and cubes in turn, as many as will show.
                let mut left = (rounds, cubes);
                for i in 0..(rounds + cubes).min(6) {
                    let cube = match left {
                        (0, _) => true,
                        (_, 0) => false,
                        _ => i % 2 == 1,
                    };
                    if cube {
                        left.1 -= 1;
                    } else {
                        left.0 -= 1;
                    }
                    let at = to_world(vec2(
                        16.0 + pose.reach * 13.0 + (i % 2) as f32 * 7.0,
                        -9.0 + i as f32 * 3.6,
                    ));
                    if cube {
                        list.rect(at, vec2(8.0, 8.0), self.heading, 2.0, TOFU);
                        list.stroke_rect(at, vec2(8.0, 8.0), self.heading, 2.0, 1.0, TOFU_EDGE);
                    } else {
                        list.circle(at, 9.0, VEG);
                        list.circle(at, 4.0, VEG_DARK);
                    }
                }
            }
            Held::Tofu => {
                let at = to_world(vec2(18.0 + pose.reach * 13.0, -6.0));
                list.rect(at, vec2(28.0, 20.0), self.heading, 3.0, TOFU);
                list.stroke_rect(at, vec2(28.0, 20.0), self.heading, 3.0, 1.5, TOFU_EDGE);
            }
            Held::Plate(fill, dish) => {
                draw_plate(
                    list,
                    to_world(vec2(23.0 + pose.reach * 5.0, 0.0)),
                    fill,
                    1.0,
                    dish,
                );
            }
            Held::Knife => draw_knife(list, to_world(vec2(20.0, -6.0)), self.heading),
            Held::Spoon => draw_spoon(list, to_world(vec2(20.0, -6.0)), self.heading),
            Held::Stew => draw_stew_tub(
                list,
                to_world(vec2(20.0 + pose.reach * 8.0, 0.0)),
                self.heading,
            ),
            // A crate in both arms, square to the body, with a strap
            // across it so it reads as a box and not a plate.
            Held::Crate => {
                let at = to_world(vec2(21.0 + pose.reach * 8.0, 0.0));
                list.rect(at, vec2(26.0, 30.0), self.heading, 2.0, CRATE);
                list.stroke_rect(at, vec2(26.0, 30.0), self.heading, 2.0, 1.5, CRATE_STRAP);
                list.rect(at, vec2(26.0, 5.0), self.heading, 0.0, CRATE_STRAP);
            }
            // A medkit in one hand: a small white box with a red cross on
            // its lid.
            Held::Medkit => {
                let at = to_world(vec2(18.0 + pose.reach * 6.0, -4.0));
                list.rect(at, vec2(16.0, 12.0), self.heading, 2.0, MEDKIT);
                list.rect(at, vec2(8.0, 2.5), self.heading, 0.0, MEDKIT_CROSS);
                list.rect(at, vec2(2.5, 8.0), self.heading, 0.0, MEDKIT_CROSS);
            }
            Held::Broom => {
                // Held out in front and across, the way anyone carries one:
                // the pole running away from the body and the head on the
                // deck at the far end of it, swinging with the pose.
                let across = pose.tool.y * 0.5;
                let grip = to_world(vec2(13.0, across * 0.3));
                let head = to_world(vec2(36.0, across));
                list.line(grip, head, 4.0, BROOM_POLE);
                list.rect(
                    head,
                    vec2(9.0, 30.0),
                    self.heading + pose.tool_rot,
                    2.0,
                    BROOM_HEAD,
                );
                // Bristles, splayed the way the stroke is going.
                list.rect(
                    head + Vec2::from_angle(self.heading) * 5.0,
                    vec2(4.0, 26.0),
                    self.heading + pose.tool_rot,
                    1.5,
                    BROOM_POLE.alpha(0.55),
                );
            }
        }

        let tool_at = to_world(pose.tool);
        let tool_rot = self.heading + pose.tool_rot;
        match self.tool {
            Held::Knife => draw_knife(list, tool_at, tool_rot),
            Held::Spoon => draw_spoon(list, tool_at, tool_rot),
            Held::Pick => draw_pick(list, tool_at, tool_rot),
            Held::Fork => {
                list.rect(tool_at, vec2(22.0, 4.0), tool_rot, 2.0, STEEL);
                list.rect(
                    tool_at + Vec2::from_angle(tool_rot) * -10.0,
                    vec2(8.0, 6.0),
                    tool_rot,
                    2.0,
                    GRIP,
                );
            }
            _ => {}
        }
    }

    /// The weapon, drawn: every gun **in both hands, angled across the
    /// front** of the body (feature 82) — the grip ahead of the chest
    /// and over towards the firing shoulder with the rear hand on it,
    /// the forward hand out along the fore-end past the centreline, and
    /// the gun itself laid at [`CARRY`] off the facing at [`GUN_SCALE`]
    /// of the body’s own size, its emitter lit at the muzzle. Angled and
    /// smaller for the same reason: down the centreline at full size a
    /// rifle is a lance longer than the figure is wide, with its stock
    /// over the face. What each gun *is* is
    /// [`draw_gun`], shared with the one lying on the deck, so a pistol
    /// is a stubby sidearm, a shotgun a heavy pump, the auto rifle a
    /// rifle — handguard, magazine, rail and brake — and the sniper the
    /// long one with the scope, the bipod and the glass. The schword is
    /// a hilt in both hands with the blade forward and a little raised;
    /// mid-swing the blade is swept through its arc instead.
    ///
    /// The arms that reach for it are **not** here: they are limbs off
    /// the shoulders ([`Character::draw_arm`], out to
    /// [`Character::weapon_hands`]) and go down with the body, so the
    /// head lies over an elbow rather than under one. Broken armour and
    /// the wound blotches are drawn before this and show over nothing
    /// here, since the weapon is out in front of the body and not on it.
    fn draw_weapon(&self, list: &mut DrawList, pose: Pose, weapon: WeaponKind) {
        let pos = self.drawn_at();
        let rot = self.heading;
        let scale = self.body_scale();
        let to_world = |local: Vec2| pos + (local * scale).rotate(rot);
        let grip = self.weapon_grip(pose);
        // What a weapon is drawn at: the rack's own numbers brought down
        // to the size one is held at.
        let blade = scale * GUN_SCALE;
        if weapon == WeaponKind::Schword {
            let hilt = grip + vec2(2.0, 0.0);
            // In a hand the edge is lit, in its side's colour: the crew's
            // cyan, the enemy's red (feature 98).
            let edge = if self.hostile { ENEMY } else { BLADE_EDGE };
            if self.action == Action::Swing {
                // Swept across in front of the body, from the left to
                // the right, the blade along the arc's radius.
                let angle = SWING_ARC * (self.swing_through() - 0.5);
                let base = to_world(vec2(6.0, 0.0));
                let dir = Vec2::from_angle(rot + angle);
                draw_blade(list, base + dir * 4.0, dir, 36.0 * blade, edge, true);
                // The arc behind it, fading: where the blade has been.
                let steps = 6;
                for i in 0..steps {
                    let back = (i + 1) as f32 / steps as f32;
                    let a = angle - SWING_ARC * 0.35 * back;
                    if a < -SWING_ARC * 0.5 {
                        break;
                    }
                    let d = Vec2::from_angle(rot + a);
                    let fade = 1.0 - back;
                    list.line(
                        base + d * (14.0 * blade),
                        base + d * (40.0 * blade),
                        5.0,
                        edge.alpha(0.25 * fade),
                    );
                }
            } else {
                // At rest: the hilt in both hands out in front, the blade
                // forward and a little raised — angled in across the front
                // the way every gun is ([`Character::gun_rot`]).
                let dir = Vec2::from_angle(rot + self.gun_rot(grip));
                draw_blade(list, to_world(hilt), dir, 34.0 * blade, edge, true);
            }
            return;
        }
        // The gun in its own frame, over the arms that already reach for
        // it, so its silhouette runs unbroken along its own line and what
        // shows of a hand is the half of it round the far side.
        let mut b = list.brush(to_world(grip), rot + self.gun_rot(grip), blade);
        let lit = if self.hostile { ENEMY } else { GUN_LIT };
        draw_gun(&mut b, Vec2::ZERO, weapon, Some(lit));
    }
}

/// How far ahead of the grip the forward hand sits on each gun: on a
/// pistol it is all but on the grip itself, on a long gun it is out on
/// the pump, the handguard or the barrel.
fn fore_hand(weapon: WeaponKind) -> f32 {
    match weapon {
        WeaponKind::LaserPistol => 3.0,
        WeaponKind::Shotgun => 14.0,
        WeaponKind::AutoRifle => 15.0,
        WeaponKind::SniperRifle => 20.0,
        WeaponKind::Schword => 6.0,
        // A droid's arm is part of the machine and never in a
        // hand: answered so the match is whole, read by nobody.
        WeaponKind::Claw | WeaponKind::Unmaker => 6.0,
    }
}

/// Where the emitter sits on each gun, ahead of the grip in the gun's
/// own frame: the eye [`draw_gun`] lights and, through
/// [`Character::muzzle`], **the point every shot leaves from** (feature
/// 84). Nought for anything that does not fire out of a barrel — a
/// blade, a droid's arm — which is how [`Character::muzzle`] tells them
/// apart.
fn muzzle_ahead(weapon: WeaponKind) -> f32 {
    match weapon {
        WeaponKind::LaserPistol => 12.0,
        WeaponKind::Shotgun => 28.0,
        WeaponKind::AutoRifle => 32.0,
        WeaponKind::SniperRifle => 42.0,
        WeaponKind::Schword | WeaponKind::Claw | WeaponKind::Unmaker => 0.0,
    }
}

/// How far ahead of the grip each gun reaches, muzzle included — what
/// tells the four of them apart at a glance before any detail does: the
/// pistol is a third of the auto rifle and the sniper is half again as
/// long as that.
fn gun_reach(weapon: WeaponKind) -> f32 {
    match weapon {
        WeaponKind::LaserPistol => 13.5,
        WeaponKind::Shotgun => 29.0,
        WeaponKind::AutoRifle => 33.0,
        WeaponKind::SniperRifle => 44.0,
        WeaponKind::Schword => 34.0,
        WeaponKind::Claw | WeaponKind::Unmaker => 0.0,
    }
}

/// One gun, drawn into `b` with its grip at `grip` and its barrel
/// running forward along the frame's `+x`. The hands are not here: this
/// is the gun itself, so the one in a Bim's hands and the one lying on
/// the deck where a body dropped it are the same gun, and there is one
/// place to change what a gun looks like. `lit` is the emitter's glow at
/// the muzzle, in the side's colour — the crew's blue, an enemy's red
/// (feature 98) — and `None` for a gun on the deck, which is cold.
///
/// Seen from above, so every piece is a block along the length and the
/// things that would hang under the gun are canted out to the side
/// instead: the rifle's magazine, the sniper's bipod legs.
fn draw_gun(b: &mut Brush, grip: Vec2, weapon: WeaponKind, lit: Option<Color>) {
    // A piece of the gun: a block with a rim a shade lighter round it,
    // so each piece holds its own edge against any deck colour.
    let part = |b: &mut Brush, at: Vec2, size: Vec2, rot: f32, c: Color| {
        b.rect(grip + at, size + vec2(1.6, 1.6), rot, 1.6, GUN_EDGE);
        b.rect(grip + at, size, rot, 1.2, c);
    };
    // The muzzle: the emitter's eye, and the glow round it when the gun
    // is in a hand rather than on the deck.
    let muzzle = |b: &mut Brush, at: f32, size: f32| {
        let Some(glow) = lit else {
            return;
        };
        b.ellipse(
            grip + vec2(at, 0.0),
            vec2(size * 2.8, size * 2.8),
            0.0,
            glow.alpha(0.14),
        );
        b.ellipse(
            grip + vec2(at, 0.0),
            vec2(size * 1.7, size * 1.7),
            0.0,
            glow.alpha(0.38),
        );
        b.ellipse(grip + vec2(at, 0.0), vec2(size, size), 0.0, glow);
    };
    match weapon {
        WeaponKind::LaserPistol => {
            // A sidearm, and small with it: a stubby slide with the
            // emitter at its nose and a butt behind the hands. Nothing
            // else — everything a pistol has, it has less of.
            part(b, vec2(-2.0, 1.0), vec2(5.5, 8.0), 0.0, GUN);
            part(b, vec2(5.0, 0.0), vec2(13.0, 5.5), 0.0, GUN);
            b.rect(grip + vec2(4.5, 0.0), vec2(8.0, 1.4), 0.0, 0.0, GUN_EDGE);
            muzzle(b, muzzle_ahead(weapon), 3.8);
        }
        WeaponKind::Shotgun => {
            // Heavy and short: a wide bore over a wooden pump, the
            // receiver square in the middle, the butt braced back.
            part(b, vec2(-4.5, 0.0), vec2(8.0, 6.2), 0.0, STOCK);
            part(b, vec2(2.0, 0.0), vec2(10.0, 7.5), 0.0, GUN);
            part(b, vec2(17.5, 0.0), vec2(23.0, 6.0), 0.0, GUN);
            part(b, vec2(13.0, 0.0), vec2(9.0, 8.0), 0.0, STOCK);
            b.ellipse(grip + vec2(28.0, 0.0), vec2(7.0, 7.0), 0.0, GUN_EDGE);
            b.ellipse(grip + vec2(28.0, 0.0), vec2(4.6, 4.6), 0.0, GUN);
            muzzle(b, muzzle_ahead(weapon), 3.6);
        }
        WeaponKind::AutoRifle => {
            // A rifle, and shaped like one: a stock, a deep receiver
            // with the magazine canted out under it, a vented handguard
            // along the barrel, a sight rail down the top and a brake on
            // the end.
            part(b, vec2(-5.0, 0.0), vec2(9.0, 6.8), 0.0, GUN);
            b.rect(grip + vec2(3.5, 6.0), vec2(5.0, 10.0), 0.35, 1.2, GUN_EDGE);
            b.rect(grip + vec2(3.5, 6.0), vec2(3.4, 8.4), 0.35, 1.0, GUN);
            part(b, vec2(2.5, 0.0), vec2(12.0, 8.0), 0.0, GUN);
            part(b, vec2(16.0, 0.0), vec2(14.0, 6.2), 0.0, GUN);
            for i in 0..3 {
                let x = 11.5 + i as f32 * 4.5;
                b.rect(grip + vec2(x, 0.0), vec2(1.6, 5.0), 0.0, 0.0, GUN_EDGE);
            }
            part(b, vec2(26.5, 0.0), vec2(8.0, 3.8), 0.0, GUN);
            b.rect(grip + vec2(6.0, 0.0), vec2(17.0, 1.8), 0.0, 0.0, SCOPE);
            part(b, vec2(31.0, 0.0), vec2(4.5, 6.0), 0.0, GUN_EDGE);
            muzzle(b, muzzle_ahead(weapon), 3.6);
        }
        WeaponKind::SniperRifle => {
            // The long one, and the only one with glass in it: a cheeked
            // stock, a thin barrel running most of a tile out of a
            // bipod, and a scope down the middle with the objective lens
            // forward — which is what says *sniper* before the length
            // does.
            part(b, vec2(-5.5, 0.0), vec2(10.0, 6.8), 0.0, STOCK);
            b.rect(grip + vec2(-6.0, -2.4), vec2(7.5, 2.6), 0.0, 1.0, GUN_EDGE);
            part(b, vec2(3.0, 0.0), vec2(12.0, 7.0), 0.0, GUN);
            part(b, vec2(24.0, 0.0), vec2(30.0, 3.8), 0.0, GUN);
            // The bipod, splayed either side of the barrel near its end.
            for side in [-1.0f32, 1.0] {
                b.rect(
                    grip + vec2(33.0, 5.0 * side),
                    vec2(11.0, 2.0),
                    1.15 * side,
                    1.0,
                    GUN_EDGE,
                );
            }
            part(b, vec2(6.0, 0.0), vec2(19.0, 4.6), 0.0, SCOPE);
            b.ellipse(grip + vec2(-2.0, 0.0), vec2(5.6, 5.6), 0.0, SCOPE);
            b.ellipse(grip + vec2(16.0, 0.0), vec2(7.2, 7.2), 0.0, SCOPE);
            b.ellipse(grip + vec2(16.0, 0.0), vec2(4.4, 4.4), 0.0, LENS);
            part(b, vec2(40.5, 0.0), vec2(5.0, 6.0), 0.0, GUN_EDGE);
            muzzle(b, muzzle_ahead(weapon), 3.4);
        }
        // Nothing to draw: a blade is drawn by its own hand, and a
        // droid's arm is drawn with the droid (`crate::droid`).
        WeaponKind::Schword | WeaponKind::Claw | WeaponKind::Unmaker => {}
    }
}

/// A pick: a handle along `rot` with the head across its end, the point
/// forward.
fn draw_pick(list: &mut DrawList, at: Vec2, rot: f32) {
    let dir = Vec2::from_angle(rot);
    list.rect(at - dir * 4.0, vec2(30.0, 4.0), rot, 2.0, BROOM_POLE);
    list.rect(at + dir * 12.0, vec2(6.0, 22.0), rot, 2.0, STEEL);
    list.rect(at + dir * 16.0, vec2(6.0, 8.0), rot, 1.5, STEEL);
}

/// The schword: a hilt at `hilt`, the blade `length` along `dir` — a
/// white core between two cyan strokes, one wide and faint and one thin
/// and bright, so the edge reads as light rather than paint.
/// A figure of `look` in `outfit` standing still at the origin facing
/// `heading`, in the crew's coverall, drawn into `list` — what a chooser
/// shows beside the hairstyles (feature 62) and the classes (feature 81).
/// The same `draw` as on the deck, so the picture is the Bim and not a
/// drawing of one; nothing rolled, so it stands the same every frame.
pub fn portrait(look: Look, outfit: Outfit, heading: f32, list: &mut DrawList) {
    let mut figure = Character::new(Vec2::ZERO, look, &mut Rng::new(0));
    figure.heading = heading;
    figure.intent = heading;
    figure.idle = 0.0;
    figure.outfit = outfit;
    figure.draw(list, u32::MAX);
}

/// The hair on a standing head, seen from above, in the head's own frame:
/// `at` puts a point local to the head — the face towards +x — onto the
/// body, and `turn` is the glance the head is turned by. The crown sits on
/// the back of the head with the face showing ahead of it; what a style
/// adds is drawn before the crown so the crown lies on top rather than a
/// fall on the face. The nose goes on after, so a puff that reaches the
/// face still leaves it.
/// What the class wears on its head (feature 81), in the head's own
/// frame: `at` puts a point local to the head — the face towards +x —
/// onto the body, and `turn` is the glance the head is turned by. Every
/// one of them sits back off the face, the way the armour's helm does,
/// so the nose drawn after still shows under the peak. The soldier's
/// sunglasses are not here: they go on over a helm as well, so `draw`
/// puts them on last.
fn draw_class_head(b: &mut Brush, outfit: Outfit, at: &impl Fn(Vec2) -> Vec2, turn: f32) {
    match outfit {
        Outfit::Plain | Outfit::Soldier => {}
        // A hard hat: a rimmed ochre shell with a ridge down the middle
        // and a peak out over the brow.
        Outfit::Engineer => {
            b.ellipse(
                at(vec2(-1.0, 0.0)),
                vec2(14.0, 15.0),
                turn,
                KIT_ENGINEER.mix(KIT_DARK, 0.25),
            );
            b.ellipse(at(vec2(-1.0, 0.0)), vec2(12.0, 13.0), turn, KIT_ENGINEER);
            b.rect(
                at(vec2(-1.0, 0.0)),
                vec2(12.0, 3.4),
                turn,
                1.2,
                KIT_ENGINEER.mix(KIT_DARK, 0.35),
            );
            b.ellipse(
                at(vec2(4.2, 0.0)),
                vec2(4.6, 12.0),
                turn,
                KIT_ENGINEER.mix(KIT_DARK, 0.15),
            );
        }
        // A white cap with the cross on the crown, where it is seen from
        // furthest off.
        Outfit::Medic => {
            b.ellipse(at(vec2(-1.5, 0.0)), vec2(12.5, 13.5), turn, KIT_MEDIC);
            b.rect(at(vec2(-1.5, 0.0)), vec2(2.6, 8.0), turn, 0.6, MEDKIT_CROSS);
            b.rect(at(vec2(-1.5, 0.0)), vec2(8.0, 2.6), turn, 0.6, MEDKIT_CROSS);
        }
        // A heavy helm, rimmed dark, with a guard down each cheek and the
        // face still showing between them.
        Outfit::Tank => {
            b.ellipse(at(vec2(-1.5, 0.0)), vec2(14.5, 16.0), turn, KIT_DARK);
            b.ellipse(at(vec2(-1.5, 0.0)), vec2(13.0, 15.0), turn, KIT_TANK);
            for side in [-1.0f32, 1.0] {
                b.rect(
                    at(vec2(2.0, 5.4 * side)),
                    vec2(6.0, 3.6),
                    turn,
                    1.0,
                    KIT_TANK.mix(KIT_DARK, 0.45),
                );
            }
        }
        // An officer's cap: navy, banded in gold, with a peak.
        Outfit::Commander => {
            b.ellipse(at(vec2(-1.5, 0.0)), vec2(13.0, 14.5), turn, KIT_GOLD);
            b.ellipse(at(vec2(-1.5, 0.0)), vec2(11.0, 12.0), turn, KIT_COMMANDER);
            b.ellipse(
                at(vec2(3.8, 0.0)),
                vec2(4.4, 11.5),
                turn,
                KIT_COMMANDER.mix(KIT_DARK, 0.40),
            );
        }
    }
}

/// What the class wears over the chest and shoulders (feature 81), in the
/// body's own frame: `breath` is the swell of a sleeping chest, which
/// anything strapped across it swells with.
fn draw_class_rig(b: &mut Brush, outfit: Outfit, breath: f32) {
    match outfit {
        Outfit::Plain => {}
        // A tool belt round the waist with a pouch on each hip, and the
        // strap of it over one shoulder.
        Outfit::Engineer => {
            b.rect(
                vec2(0.0, -4.0),
                vec2(21.0, 3.4) * breath,
                0.22,
                1.2,
                KIT_DARK,
            );
            b.ellipse(vec2(-6.5, 0.0), vec2(5.0, 22.0) * breath, 0.0, KIT_DARK);
            for side in [-1.0f32, 1.0] {
                b.rect(
                    vec2(-6.5, 8.0 * side),
                    vec2(5.4, 6.0),
                    0.0,
                    1.5,
                    KIT_ENGINEER,
                );
            }
        }
        // Webbing crossed over the chest with three magazines down one
        // strap, and a belt at the waist: kitted, and used to it.
        Outfit::Soldier => {
            for turn in [0.5f32, -0.5] {
                b.rect(
                    vec2(1.0, 0.0),
                    vec2(3.6, 26.0) * breath,
                    turn,
                    1.2,
                    KIT_DARK,
                );
            }
            let along = vec2(-(0.5f32).sin(), (0.5f32).cos());
            for step in [-1.0f32, 0.0, 1.0] {
                b.rect(
                    vec2(1.0, 0.0) + along * (8.0 * step),
                    vec2(5.2, 4.2),
                    0.5,
                    1.0,
                    KIT_GOLD.mix(KIT_DARK, 0.35),
                );
            }
            b.rect(
                vec2(-7.0, 0.0),
                vec2(3.4, 21.0) * breath,
                0.0,
                1.0,
                KIT_DARK,
            );
        }
        // The cross on the chest, and the bag on the hip it is carried in.
        Outfit::Medic => {
            b.rect(
                vec2(3.0, 0.0),
                vec2(3.4, 11.0) * breath,
                0.0,
                0.8,
                MEDKIT_CROSS,
            );
            b.rect(
                vec2(3.0, 0.0),
                vec2(11.0, 3.4) * breath,
                0.0,
                0.8,
                MEDKIT_CROSS,
            );
            b.ellipse(
                vec2(-5.0, 10.0),
                vec2(9.0, 7.0),
                0.0,
                KIT_MEDIC.mix(KIT_DARK, 0.30),
            );
            b.rect(vec2(-5.0, 10.0), vec2(4.4, 1.6), 0.0, 0.4, MEDKIT_CROSS);
            b.rect(vec2(-5.0, 10.0), vec2(1.6, 4.4), 0.0, 0.4, MEDKIT_CROSS);
        }
        // A pauldron capping each shoulder and a gorget across the chest:
        // the broadest silhouette of the six, before the body under it is
        // drawn bigger as well.
        Outfit::Tank => {
            for side in [-1.0f32, 1.0] {
                b.ellipse(vec2(-1.0, 12.0 * side), vec2(16.0, 11.5), 0.0, KIT_DARK);
                b.ellipse(vec2(-1.0, 12.0 * side), vec2(13.5, 9.0), 0.0, KIT_TANK);
            }
            b.ellipse(
                vec2(6.0, 0.0),
                vec2(6.5, 16.0) * breath,
                0.0,
                KIT_TANK.mix(KIT_DARK, 0.35),
            );
        }
        // Gold boards on both shoulders and a sash across the chest.
        Outfit::Commander => {
            b.rect(vec2(1.5, 0.0), vec2(5.0, 24.0) * breath, 0.6, 1.5, KIT_GOLD);
            b.rect(
                vec2(1.5, 0.0),
                vec2(1.6, 24.0) * breath,
                0.6,
                0.6,
                KIT_COMMANDER,
            );
            for side in [-1.0f32, 1.0] {
                b.rect(vec2(-1.0, 11.5 * side), vec2(10.0, 6.0), 0.0, 1.5, KIT_GOLD);
                b.rect(
                    vec2(-3.5, 11.5 * side),
                    vec2(2.2, 4.4),
                    0.0,
                    0.6,
                    KIT_COMMANDER,
                );
            }
        }
    }
}

fn draw_hair_standing(b: &mut Brush, look: Look, at: impl Fn(Vec2) -> Vec2, turn: f32) {
    let hair = look.hair();
    let crown = |b: &mut Brush| b.ellipse(at(vec2(-2.0, 0.0)), vec2(11.0, 13.0), turn, hair);
    match look.hair {
        Hair::Cropped => crown(b),
        Hair::Bald => {}
        // A fall back over the shoulders: a second ellipse behind the head.
        Hair::Long => {
            b.ellipse(at(vec2(-7.5, 0.0)), vec2(14.0, 17.5), turn, hair);
            crown(b);
        }
        // Level at the chin: the crown wider than the head all round, so
        // it shows past the skin at the sides and the back.
        Hair::Bob => {
            b.ellipse(at(vec2(-3.0, 0.0)), vec2(14.0, 17.5), turn, OUTLINE);
            b.ellipse(at(vec2(-3.0, 0.0)), vec2(12.5, 16.0), turn, hair);
        }
        // The crown, and a knot behind it with its band.
        Hair::Bun => {
            crown(b);
            b.ellipse(at(vec2(-8.5, 0.0)), vec2(7.5, 7.5), turn, OUTLINE);
            b.ellipse(at(vec2(-8.5, 0.0)), vec2(6.0, 6.0), turn, hair);
            b.rect(at(vec2(-6.0, 0.0)), vec2(1.5, 5.0), turn, 0.0, HAIR_BAND);
        }
        // A strip down the middle, the sides bare.
        Hair::Mohawk => {
            b.rect(at(vec2(-1.5, 0.0)), vec2(13.0, 4.5), turn, 2.0, hair);
        }
        // The crown, and one tail hanging behind from a band.
        Hair::Ponytail => {
            b.ellipse(at(vec2(-13.0, 0.0)), vec2(15.0, 5.0), turn, hair);
            crown(b);
            b.rect(at(vec2(-7.0, 0.0)), vec2(1.8, 5.0), turn, 0.0, HAIR_BAND);
        }
        // A round mass standing out from the head all round, rimmed so
        // its edge holds against the deck; the face shows in front of it.
        Hair::Curly => {
            b.ellipse(at(vec2(-4.0, 0.0)), vec2(18.0, 20.0), turn, OUTLINE);
            b.ellipse(at(vec2(-4.0, 0.0)), vec2(16.5, 18.5), turn, hair);
        }
    }
}

/// The hair on a head lying on its cheek, `head` the head's centre in the
/// body's frame: what a style spreads on the deck goes down before the
/// head (`under`), what sits on it after. The lying head faces +x with
/// its crown to the near (−y) side, the way `draw_flat` lays it.
fn draw_hair_lying(b: &mut Brush, style: Hair, head: Vec2, hair: Color, under: bool) {
    let crown = |b: &mut Brush| b.ellipse(head + vec2(-1.0, -3.0), vec2(12.0, 10.0), 0.35, hair);
    match (style, under) {
        (Hair::Bald, _) => {}
        (Hair::Cropped, false) => crown(b),
        (Hair::Cropped, true) => {}
        // Long hair fans out on the deck behind it.
        (Hair::Long, true) => b.ellipse(head + vec2(-5.0, -6.0), vec2(20.0, 17.0), -0.5, hair),
        (Hair::Long, false) => crown(b),
        (Hair::Bob, true) => b.ellipse(head + vec2(-2.0, -3.0), vec2(16.5, 14.0), 0.35, OUTLINE),
        (Hair::Bob, false) => b.ellipse(head + vec2(-2.0, -3.0), vec2(15.0, 12.5), 0.35, hair),
        (Hair::Bun, true) => {
            b.ellipse(head + vec2(-7.0, -8.0), vec2(7.5, 7.5), 0.0, OUTLINE);
            b.ellipse(head + vec2(-7.0, -8.0), vec2(6.0, 6.0), 0.0, hair);
        }
        (Hair::Bun, false) => {
            crown(b);
            b.rect(
                head + vec2(-5.0, -6.0),
                vec2(1.5, 4.5),
                -0.8,
                0.0,
                HAIR_BAND,
            );
        }
        (Hair::Mohawk, true) => {}
        (Hair::Mohawk, false) => b.rect(head + vec2(-1.0, -3.5), vec2(12.0, 4.0), 0.35, 2.0, hair),
        (Hair::Ponytail, true) => {
            b.ellipse(head + vec2(-11.0, -9.0), vec2(16.0, 5.0), -0.6, hair);
        }
        (Hair::Ponytail, false) => {
            crown(b);
            b.rect(
                head + vec2(-6.0, -6.0),
                vec2(1.8, 4.5),
                -0.6,
                0.0,
                HAIR_BAND,
            );
        }
        (Hair::Curly, true) => b.ellipse(head + vec2(-3.0, -5.0), vec2(19.5, 17.5), 0.35, OUTLINE),
        (Hair::Curly, false) => b.ellipse(head + vec2(-3.0, -5.0), vec2(18.0, 16.0), 0.35, hair),
    }
}

/// A weapon lying on the deck at `at`, dropped by a body knocked out:
/// the same gun the hands draw ([`draw_gun`]), without the hands and
/// with its emitter cold, laid askew the way a thing falls. A shadow
/// under it, as long as the gun is, so it reads as *on* the deck and
/// not painted on it.
pub fn draw_dropped(list: &mut DrawList, at: Vec2, weapon: WeaponKind) {
    const ASKEW: f32 = 0.55;
    // The same size it is in a hand ([`GUN_SCALE`]), so a gun does not
    // grow on the way to the deck.
    let s = BODY_SCALE * GUN_SCALE;
    let reach = gun_reach(weapon);
    list.ellipse(
        at + vec2(2.0, 3.0),
        vec2(reach + 13.0, 12.0) * s,
        ASKEW,
        SHADOW,
    );
    if weapon == WeaponKind::Schword {
        let dir = Vec2::from_angle(ASKEW);
        draw_blade(
            list,
            at - dir * (14.0 * s),
            dir,
            30.0 * s,
            BLADE_EDGE,
            false,
        );
        return;
    }
    let mut b = list.brush(at, ASKEW, s);
    draw_gun(&mut b, vec2(-reach * 0.5, 0.0), weapon, None);
}

/// A schword's blade from its hilt along `dir`, its edge in `edge`: two
/// strokes of it round a white core. `lit` — the blade in a hand — pulls
/// the core towards the edge's colour and past white (feature 98), so it
/// blooms like a shot does; one on the deck is cold.
fn draw_blade(list: &mut DrawList, hilt: Vec2, dir: Vec2, length: f32, edge: Color, lit: bool) {
    let rot = dir.angle();
    let s = BODY_SCALE;
    let tip = hilt + dir * length;
    let start = hilt + dir * (7.0 * s);
    list.line(start, tip, 10.0 * s, edge.alpha(0.22));
    list.line(start, tip, 4.5 * s, edge.alpha(0.85));
    let core = if lit {
        edge.mix(BLADE_CORE, 1.0 - BLADE_TINT).glowing(BLADE_HEAT)
    } else {
        BLADE_CORE
    };
    list.line(start, tip, 2.0 * s, core);
    list.rect(hilt + dir * (2.0 * s), vec2(9.0, 4.0) * s, rot, 1.0, HILT);
    list.rect(
        hilt + dir * (7.0 * s),
        vec2(2.0, 9.0) * s,
        rot,
        0.5,
        GUN_EDGE,
    );
}

/// A letter Z, drawn from the three strokes you would write it with.
fn draw_z(list: &mut DrawList, at: Vec2, size: f32, c: Color) {
    let h = size * 0.5;
    let line = (size * 0.17).max(1.2);
    list.line(at + vec2(-h, -h), at + vec2(h, -h), line, c);
    list.line(at + vec2(h, -h), at + vec2(-h, h), line, c);
    list.line(at + vec2(-h, h), at + vec2(h, h), line, c);
}

/// Arm and tool placement for one frame of an action.
#[derive(Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct Pose {
    /// Forward offset of the left arm, in local pixels.
    left: f32,
    right: f32,
    /// Where a held tool sits, in the local frame.
    tool: Vec2,
    tool_rot: f32,
    /// 0 to 1, how far the arms are extended. Held items ride along with it.
    reach: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A body backing away walks the route it was given while facing
    /// the other way (feature 84): the fall back's facing is the
    /// enemy's, and the feet carry it home all the same. And it is
    /// slower doing it than a body sprinting the same leg is — which is
    /// the whole of the difference between a crew member covering a
    /// retreat and one running for the airlock.
    fn walker(to: Vec2) -> Character {
        let mut rng = Rng::new(7);
        let mut ch = Character::new(vec2(0.0, 0.0), Look::CLASSIC[0], &mut rng);
        ch.set_recruited(true);
        ch.follow_path(vec![to, to * 2.0]);
        ch
    }

    fn run(ch: &mut Character, seconds: f32) {
        let room = Rect::from_corners(vec2(-4000.0, -4000.0), vec2(4000.0, 4000.0));
        let mut rng = Rng::new(11);
        for _ in 0..(seconds * 60.0) as usize {
            ch.update(1.0 / 60.0, room, &[], &mut rng);
        }
    }

    #[test]
    fn backing_away_walks_the_route_while_facing_the_other_way() {
        let east = vec2(600.0, 0.0);
        // Facing west — where the enemy is — and walking east all the
        // same.
        let mut backing = walker(east);
        backing.set_falling_back(Some(FallBack::Backwards(PI)));
        run(&mut backing, 2.0);
        assert!(
            backing.pos.x > 40.0,
            "it made ground along the route: {:?}",
            backing.pos
        );
        assert!(
            wrap_angle(backing.heading - PI).abs() < 0.2,
            "and it is facing the enemy: {}",
            backing.heading
        );

        // The same leg at a sprint: facing the way it goes, and further
        // along it in the same two seconds.
        let mut sprinting = walker(east);
        sprinting.set_falling_back(Some(FallBack::Sprint));
        run(&mut sprinting, 2.0);
        assert!(
            sprinting.heading.abs() < 0.2,
            "a sprint faces where it is going: {}",
            sprinting.heading
        );
        assert!(
            sprinting.pos.x > backing.pos.x,
            "and outruns the one backing off: {} against {}",
            sprinting.pos.x,
            backing.pos.x
        );

        // And an ordinary walk is between the two.
        let mut walking = walker(east);
        run(&mut walking, 2.0);
        assert!(
            walking.pos.x > backing.pos.x && walking.pos.x < sprinting.pos.x,
            "a walk is between them: {}",
            walking.pos.x
        );
    }
}
