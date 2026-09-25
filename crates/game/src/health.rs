//! A body: its three parts, its blood, its wounds and its traumas, and how
//! each of them mends or does not.
//!
//! # The body is three parts, and the blood is a fourth number
//!
//! Health is not one bar but three — the head, the body and the legs, each
//! with a base of its own ([`Part::max`]: 5, 75 and 20, a hundred all told)
//! — and what the panel calls health is the three added up. A shot lands
//! on one part ([`Part::HIT_ODDS`]: one in twenty the head, three in four
//! the body, one in five the legs) and takes the weapon's damage off that
//! part alone. Mending runs over all three in proportion, so the total
//! behaves exactly as the one bar did.
//!
//! # A part at nothing is a dying state, not a death
//!
//! A part reaching nothing rolls a [`Trauma`] for it — three a part, four
//! for the legs, [`Trauma::roll`] — and the Bim is **dying**
//! ([`Health::dying`]): the part stays at nothing and does not mend, the
//! trauma bleeds it ([`Trauma::bleed`]) or slows it ([`Trauma::pace`],
//! [`Trauma::works_at`]) until **another Bim treats it with a medkit**
//! ([`Health::treat`]), which puts the part back to [`TREATED_TO`] of its
//! base and leaves whatever the trauma leaves behind — a [`Lasting`]
//! penalty for a day or two ([`Trauma::after`]), or a **leg lost**
//! ([`Trauma::loses_leg`]: the crushed legs, one in twenty of a leg's
//! rolls each), which is for ever and costs [`LEG_LOST_PACE`] of the walk
//! each. A hit on a part already at nothing opens a wound and nothing
//! more: it is as dying as it gets. What kills a Bim now is its **blood**.
//!
//! Beside the three, **blood**: a hundred points, and every hit opens a
//! wound that bleeds [`BLEED_PER_WOUND`] of it an hour until it is dressed
//! — so ten open wounds bleed a Bim out in an hour. A wound is counted
//! in **units**: a shot opens one, a cut ([`CUT_WOUND`]) three, so a
//! blade bleeds three times what a bolt does and a bandage still closes
//! the lot on a part at once. An untreated trauma bleeds beside the
//! wounds, [`HEAVY_BLEED`] or [`SLOW_BLEED`] an hour. Under
//! [`SLOWED_AT`] — three quarters — the Bim walks at half its pace;
//! under [`OUT_AT`], which is **half its blood**, it is out cold where
//! it stands and nothing aims at it any more; at nothing it is dead. A bandage ([`Health::bandage`]) closes
//! every wound on one part, and blood comes back on its own once nothing
//! is open and no trauma bleeds. Armour stands in front of all of this —
//! a worn piece takes a hit before the part does, and only what gets
//! through comes here (`Game::wound`, `crate::combat`); nothing in this
//! file knows about it.

use crate::clock::{DAY, HOUR};

pub const MAX_HEALTH: f32 = 100.0;

/// The blood a body has, full.
pub const MAX_BLOOD: f32 = 100.0;

/// Blood lost an hour by each open wound.
pub const BLEED_PER_WOUND: f32 = 10.0;

/// The wound units a cut opens, against a shot's one.
pub const CUT_WOUND: u32 = 3;

/// Below this share of its blood the Bim walks at half its pace, and
/// below the second it is out cold. **Half its blood is where it goes
/// out** (feature 89) — a body that far gone is out of the fight, and
/// nothing aims at it while it lies there (`world::crew::Aboard::crew_ashore`)
/// — so the slowed band sits above that, from three quarters down.
pub const SLOWED_AT: f32 = 0.75;
pub const OUT_AT: f32 = 0.5;

/// How fast blood comes back once nothing is bleeding: from nothing to
/// full in two days, the same as health.
const BLOOD_RECOVER: f32 = MAX_BLOOD / (2.0 * DAY);

/// What a lost leg costs the walk, for ever: a fifth each.
pub const LEG_LOST_PACE: f32 = 0.8;

/// What an untreated trauma bleeds an hour: ten blood a quarter hour, or
/// five.
pub const HEAVY_BLEED: f32 = 40.0;
pub const SLOW_BLEED: f32 = 20.0;

/// Where a treated part starts again from: half its base, so the next
/// hit on it is a hit and not another trauma at once.
pub const TREATED_TO: f32 = 0.5;

/// The odds a leg at nothing is crushed — the right, and the left — and
/// lost for ever; the rest of the rolls split evenly between the other
/// two.
pub const CRUSHED_ODDS: f32 = 0.05;

/// A dying state: what a part reaching nothing turned into, one of three
/// or four for the part ([`Trauma::roll`]). The codes are the app's, for
/// the name and the line under it. Each is what it does **untreated** —
/// bleeding, or a slower walk and slower work — and what it leaves
/// **after** a medkit, for a day or two or for ever.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Trauma {
    /// A quarter slower walking and working, the two days after treatment
    /// as well.
    HeavyConcussion = 0,
    /// Bleeds ten a quarter hour until treated; nothing after.
    SkullFracture = 1,
    /// Half as fast walking and working, and bleeding five a quarter hour
    /// until treated; the day after treatment still half as fast.
    CranialTrauma = 2,
    /// Bleeds ten a quarter hour, and nothing shows: a medkit is the only
    /// answer.
    InternalBleeding = 3,
    /// A quarter slower walking and working, the two days after treatment
    /// as well.
    BrokenRibs = 4,
    /// Bleeds five a quarter hour and walks at half pace until treated;
    /// the day after, still at half pace.
    ChestTrauma = 5,
    /// Bleeds ten a quarter hour until treated; nothing after.
    FracturedFemur = 6,
    /// Can barely move — a quarter of its pace — until treated; the two
    /// days after, a quarter slower.
    ShatteredKnee = 7,
    /// The leg is gone, for ever, and it bleeds ten a quarter hour until
    /// the stump is treated. One roll in twenty.
    CrushedRightLeg = 8,
    /// The other leg, the same.
    CrushedLeftLeg = 9,
}

/// A body held by a medic's heal beam (feature 76, `world::class`):
/// what the beam does to it this step. Nothing on it bleeds — the open
/// wounds and the untreated traumas stay, and lose no blood — and the
/// blood comes back at `blood_an_hour` (the body's own rate instead
/// once nothing is open, if that is more), never above [`MAX_BLOOD`];
/// the parts above nothing mend at `mend` times [`HEALTH_RECOVER`]
/// (the medic's *mender*), one for the ordinary rate. The world works
/// it out from the medic's talents and hands it to the room every step
/// (`Game::set_held`); the room applies it and knows nothing else.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Beamed {
    pub blood_an_hour: f32,
    pub mend: f32,
    /// What the bleeding is multiplied by: nought for a beam, which
    /// stops it dead, and a share of it for a commander's *steady ranks*
    /// (feature 78), which only slows it.
    pub bleed: f32,
}

impl Beamed {
    /// A hold that stops the bleeding and does nothing else: what a
    /// beam is without *mender*, and what *self-care*, *hold fast* and
    /// the like give.
    pub const HELD: Beamed = Beamed {
        blood_an_hour: 0.0,
        mend: 1.0,
        bleed: 0.0,
    };
}

/// What a Bim's doctoring runs at (feature 76): the world's word, by
/// index, off the medic's talents — `bandage` and `treat` are effort
/// factors on the working step of each errand (two is half the time);
/// `bare` is whether it may treat a trauma with no medkit to hand, and
/// at what effort (the medic's *field surgeon*); `clean_hands` whether
/// a trauma it treats leaves nothing lasting; and `treated_to` where a
/// part it treats starts again from, a share of the part's base
/// ([`TREATED_TO`] for anybody else). [`Doctoring::NONE`] for everybody
/// the world does not name.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Doctoring {
    pub bandage: f32,
    pub treat: f32,
    pub bare: Option<f32>,
    pub clean_hands: bool,
    pub treated_to: f32,
}

impl Doctoring {
    pub const NONE: Doctoring = Doctoring {
        bandage: 1.0,
        treat: 1.0,
        bare: None,
        clean_hands: false,
        treated_to: TREATED_TO,
    };
}

/// What a treated trauma leaves behind for a while: the trauma's
/// [`Trauma::after`] pace and effort, for `left` more game minutes.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Lasting {
    pub trauma: Trauma,
    pub left: f32,
}

impl Trauma {
    pub const ALL: [Trauma; 10] = [
        Trauma::HeavyConcussion,
        Trauma::SkullFracture,
        Trauma::CranialTrauma,
        Trauma::InternalBleeding,
        Trauma::BrokenRibs,
        Trauma::ChestTrauma,
        Trauma::FracturedFemur,
        Trauma::ShatteredKnee,
        Trauma::CrushedRightLeg,
        Trauma::CrushedLeftLeg,
    ];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<Trauma> {
        Trauma::ALL.get(code as usize).copied()
    }

    /// The part it is a trauma of.
    pub fn part(self) -> Part {
        match self {
            Trauma::HeavyConcussion | Trauma::SkullFracture | Trauma::CranialTrauma => Part::Head,
            Trauma::InternalBleeding | Trauma::BrokenRibs | Trauma::ChestTrauma => Part::Body,
            Trauma::FracturedFemur
            | Trauma::ShatteredKnee
            | Trauma::CrushedRightLeg
            | Trauma::CrushedLeftLeg => Part::Legs,
        }
    }

    /// Which trauma a roll of `unit` (0 to 1) lands on for `part`: the
    /// head's and the body's three evenly, the legs' crushed ones
    /// [`CRUSHED_ODDS`] each and the femur and the knee the rest, half
    /// and half.
    pub fn roll(part: Part, unit: f32) -> Trauma {
        let unit = unit.clamp(0.0, 0.999_999);
        match part {
            Part::Head => match (unit * 3.0) as u32 {
                0 => Trauma::HeavyConcussion,
                1 => Trauma::SkullFracture,
                _ => Trauma::CranialTrauma,
            },
            Part::Body => match (unit * 3.0) as u32 {
                0 => Trauma::InternalBleeding,
                1 => Trauma::BrokenRibs,
                _ => Trauma::ChestTrauma,
            },
            Part::Legs => {
                let rest = 1.0 - 2.0 * CRUSHED_ODDS;
                if unit < rest * 0.5 {
                    Trauma::FracturedFemur
                } else if unit < rest {
                    Trauma::ShatteredKnee
                } else if unit < rest + CRUSHED_ODDS {
                    Trauma::CrushedRightLeg
                } else {
                    Trauma::CrushedLeftLeg
                }
            }
        }
    }

    /// Blood lost an hour while it is untreated.
    pub fn bleed(self) -> f32 {
        match self {
            Trauma::SkullFracture
            | Trauma::InternalBleeding
            | Trauma::FracturedFemur
            | Trauma::CrushedRightLeg
            | Trauma::CrushedLeftLeg => HEAVY_BLEED,
            Trauma::CranialTrauma | Trauma::ChestTrauma => SLOW_BLEED,
            Trauma::HeavyConcussion | Trauma::BrokenRibs | Trauma::ShatteredKnee => 0.0,
        }
    }

    /// How fast it walks while untreated, as a fraction of its pace.
    pub fn pace(self) -> f32 {
        match self {
            Trauma::HeavyConcussion | Trauma::BrokenRibs => 0.75,
            Trauma::CranialTrauma | Trauma::ChestTrauma => 0.5,
            Trauma::ShatteredKnee => 0.25,
            _ => 1.0,
        }
    }

    /// How fast it works while untreated, as a fraction of its effort.
    pub fn works_at(self) -> f32 {
        match self {
            Trauma::HeavyConcussion | Trauma::BrokenRibs => 0.75,
            Trauma::CranialTrauma => 0.5,
            _ => 1.0,
        }
    }

    /// What it leaves after treatment — a pace, an effort, and for how
    /// many game minutes — or nothing.
    pub fn after(self) -> Option<(f32, f32, f32)> {
        match self {
            Trauma::HeavyConcussion | Trauma::BrokenRibs => Some((0.75, 0.75, 2.0 * DAY)),
            Trauma::CranialTrauma => Some((0.5, 0.5, DAY)),
            Trauma::ChestTrauma => Some((0.5, 1.0, DAY)),
            Trauma::ShatteredKnee => Some((0.75, 1.0, 2.0 * DAY)),
            _ => None,
        }
    }

    /// Whether the leg is gone for good the moment it is rolled.
    pub fn loses_leg(self) -> bool {
        matches!(self, Trauma::CrushedRightLeg | Trauma::CrushedLeftLeg)
    }
}

/// Where a shot lands. The codes are the app's: the three armour slots
/// are in the same order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Part {
    Head = 0,
    Body = 1,
    Legs = 2,
}

impl Part {
    pub const ALL: [Part; 3] = [Part::Head, Part::Body, Part::Legs];

    /// The odds a shot lands on each, in `ALL` order. They add to one.
    pub const HIT_ODDS: [f32; 3] = [0.05, 0.75, 0.20];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<Part> {
        Part::ALL.get(code as usize).copied()
    }

    /// The part's whole health. The three add to [`MAX_HEALTH`].
    pub fn max(self) -> f32 {
        match self {
            Part::Head => 5.0,
            Part::Body => 75.0,
            Part::Legs => 20.0,
        }
    }

    /// Which part a roll of `unit` (0 to 1) lands on, by [`Part::HIT_ODDS`].
    pub fn hit_by(unit: f32) -> Part {
        let mut edge = 0.0;
        for (i, odds) in Part::HIT_ODDS.iter().enumerate() {
            edge += odds;
            if unit < edge {
                return Part::ALL[i];
            }
        }
        Part::Legs
    }
}

/// Health comes back over two days, from nothing to whole, on a body with
/// nothing open on it.
pub const HEALTH_RECOVER: f32 = MAX_HEALTH / (2.0 * DAY);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Health {
    /// The head, the body and the legs, in [`Part::ALL`] order.
    parts: [f32; 3],
    /// How many legs are gone: none, one, or both.
    legs_lost: u32,
    blood: f32,
    /// Open wound units on each part, bleeding until dressed.
    wounds: [u32; 3],
    /// The untreated trauma on each part, while the part is at nothing.
    traumas: [Option<Trauma>; 3],
    /// What treated traumas have left behind, each for a while yet.
    lasting: Vec<Lasting>,
}

impl Health {
    pub fn new() -> Health {
        Health {
            parts: [Part::Head.max(), Part::Body.max(), Part::Legs.max()],
            legs_lost: 0,
            blood: MAX_BLOOD,
            wounds: [0; 3],
            traumas: [None; 3],
            lasting: Vec::new(),
        }
    }

    /// The three parts added up: what the panel's one bar shows.
    pub fn points(&self) -> f32 {
        self.parts.iter().sum()
    }

    /// One part's health.
    pub fn part(&self, part: Part) -> f32 {
        self.parts[part as usize]
    }

    pub fn blood(&self) -> f32 {
        self.blood
    }

    /// Set the blood to a share of [`MAX_BLOOD`], for a probe that wants a
    /// body out cold without a wound on it: under [`OUT_AT`] it lies
    /// where it is until the blood comes back, which with nothing open
    /// takes days.
    #[allow(dead_code)]
    pub fn set_blood_for_probe(&mut self, share: f32) {
        self.blood = MAX_BLOOD * share;
    }

    /// Open wound units on one part.
    pub fn wounds(&self, part: Part) -> u32 {
        self.wounds[part as usize]
    }

    /// Open wound units all told.
    pub fn bleeding(&self) -> u32 {
        self.wounds.iter().sum()
    }

    pub fn legs_lost(&self) -> u32 {
        self.legs_lost
    }

    /// The untreated trauma on one part, while the part is at nothing.
    pub fn trauma(&self, part: Part) -> Option<Trauma> {
        self.traumas[part as usize]
    }

    /// Whether any part is at nothing with its trauma untreated: the
    /// state a Bim runs from a fight in, and needs a medkit out of.
    pub fn dying(&self) -> bool {
        self.traumas.iter().any(|t| t.is_some())
    }

    /// Hurt at all: an open wound on it, its blood under [`SLOWED_AT`],
    /// or dying. Not what a Bim runs from a fight in — that is
    /// [`Health::dying`] alone (`Game::is_fleeing`); a body merely
    /// bleeding fights on and is dressed when the room is calm.
    pub fn is_hurt(&self) -> bool {
        self.dying() || self.bleeding() > 0 || self.blood < MAX_BLOOD * SLOWED_AT
    }

    /// What treated traumas have left behind, each with the game minutes
    /// it has left to run.
    pub fn lasting(&self) -> &[Lasting] {
        &self.lasting
    }

    /// Dead: bled out, or the head and the body both at nothing with no
    /// trauma on either — given up. A part at
    /// nothing with its trauma untreated is dying, not dead: the blood
    /// decides.
    pub fn is_dead(&self) -> bool {
        self.blood <= 0.0
            || (self.parts[Part::Head as usize] <= 0.0
                && self.parts[Part::Body as usize] <= 0.0
                && self.traumas[Part::Head as usize].is_none()
                && self.traumas[Part::Body as usize].is_none())
    }

    /// Out cold for want of blood. Not dead — that is [`Health::is_dead`].
    pub fn unconscious(&self) -> bool {
        !self.is_dead() && self.blood < MAX_BLOOD * OUT_AT
    }

    /// How fast it walks for what the fight has done to it, as a fraction
    /// of its usual pace: the legs it has left, the blood, and what every
    /// trauma on it — untreated, or treated and lasting — costs.
    pub fn pace(&self) -> f32 {
        self.pace_at(true)
    }

    /// [`Health::pace`] with the halving for low blood left out: the
    /// tank's *unmovable*, while its kevlar holds (feature 77). The
    /// legs, the traumas and what they leave behind still count.
    pub fn pace_steady(&self) -> f32 {
        self.pace_at(false)
    }

    fn pace_at(&self, blood_counts: bool) -> f32 {
        let legs = LEG_LOST_PACE.powi(self.legs_lost as i32);
        let blood = if blood_counts && self.blood < MAX_BLOOD * SLOWED_AT {
            0.5
        } else {
            1.0
        };
        let traumas: f32 = self.traumas.iter().flatten().map(|t| t.pace()).product();
        let lasting: f32 = self
            .lasting
            .iter()
            .map(|l| l.trauma.after().map_or(1.0, |(pace, _, _)| pace))
            .product();
        legs * blood * traumas * lasting
    }

    /// How fast it works for what the fight has done to it, as a fraction
    /// of its usual effort: what every trauma on it costs a task.
    pub fn works_at(&self) -> f32 {
        let traumas: f32 = self
            .traumas
            .iter()
            .flatten()
            .map(|t| t.works_at())
            .product();
        let lasting: f32 = self
            .lasting
            .iter()
            .map(|l| l.trauma.after().map_or(1.0, |(_, work, _)| work))
            .product();
        traumas * lasting
    }

    /// A shot landing on `part`: the damage off that part, and a wound
    /// opened on it — one unit, or [`CUT_WOUND`] for a `cut`. The part
    /// reaching nothing rolls a [`Trauma`] for it off `roll` (0 to 1) and
    /// hands it back; a crushed leg is lost then and there. A part already
    /// at nothing with its trauma untreated takes the wound and nothing
    /// else, and legs both lost take the wound alone.
    pub fn shot(&mut self, part: Part, damage: f32, cut: bool, roll: f32) -> Option<Trauma> {
        let i = part as usize;
        self.wounds[i] += if cut { CUT_WOUND } else { 1 };
        if self.traumas[i].is_some() || (part == Part::Legs && self.legs_lost >= 2) {
            return None;
        }
        self.parts[i] = (self.parts[i] - damage).max(0.0);
        if self.parts[i] > 0.0 {
            return None;
        }
        let trauma = Trauma::roll(part, roll);
        self.traumas[i] = Some(trauma);
        if trauma.loses_leg() {
            self.legs_lost += 1;
        }
        Some(trauma)
    }

    /// A medkit on one part's trauma: the trauma is over, the part starts
    /// again from [`TREATED_TO`] of its base — nought for legs both gone —
    /// and what the trauma leaves behind ([`Trauma::after`]) starts its
    /// clock. The trauma treated, or `None` with nothing on that part.
    pub fn treat(&mut self, part: Part) -> Option<Trauma> {
        self.treat_as(part, false, TREATED_TO)
    }

    /// [`Health::treat`] by a medic's hands (feature 76): the part starts
    /// again from `to` of its base, and with `clean` nothing lasting is
    /// left behind (*clean hands*).
    pub fn treat_as(&mut self, part: Part, clean: bool, to: f32) -> Option<Trauma> {
        let i = part as usize;
        let trauma = self.traumas[i].take()?;
        self.parts[i] = if part == Part::Legs && self.legs_lost >= 2 {
            0.0
        } else {
            (part.max() * to).min(part.max())
        };
        if clean {
            return Some(trauma);
        }
        if let Some((_, _, minutes)) = trauma.after() {
            self.lasting.push(Lasting {
                trauma,
                left: minutes,
            });
        }
        Some(trauma)
    }

    /// Dress every wound on one part. `true` when there was one to dress.
    pub fn bandage(&mut self, part: Part) -> bool {
        let i = part as usize;
        let had = self.wounds[i] > 0;
        self.wounds[i] = 0;
        had
    }

    /// The end of it, everything left of the bar taken at once and every
    /// trauma with it: dead at the top of the next tick. What finishes a
    /// body the world says is dead (`Game::kill_now`, a grave laid out).
    pub fn give_up(&mut self) {
        self.parts = [0.0; 3];
        self.traumas = [None; 3];
    }

    /// `minutes` of the body's own clock: the blood, what a treated trauma
    /// left behind, and the mending.
    pub fn update(&mut self, minutes: f32) {
        self.update_held(minutes, None);
    }

    /// [`Health::update`] with the body held by a medic's beam, or not —
    /// see [`Beamed`] (feature 76).
    pub fn update_held(&mut self, minutes: f32, held: Option<Beamed>) {
        // Nothing comes back from nothing. Health mends on its own, and
        // without this the bar taken to zero by anything *sudden* — a body
        // given up — is back above zero on the very next frame, before
        // `Game` has looked at it, and the death never happens.
        if self.is_dead() {
            return;
        }
        // The blood: out through every open wound and every untreated
        // trauma that bleeds, back on its own once nothing does. Bleeding
        // to nothing is the death, and the check at the top of the next
        // tick is what says so.
        let open = self.bleeding();
        let trauma: f32 = self.traumas.iter().flatten().map(|t| t.bleed()).sum();
        // What a hold does to the bleeding: a beam stops it outright, a
        // commander's *steady ranks* only slows it, and nothing holding
        // it leaves it whole.
        let an_hour = (open as f32 * BLEED_PER_WOUND + trauma) * held.map_or(1.0, |h| h.bleed);
        // What comes back an hour: the hold's rate — or the body's own
        // once nothing is open, whichever is more.
        let back = held.map_or(0.0, |h| h.blood_an_hour / HOUR);
        let own = if an_hour > 0.0 { 0.0 } else { BLOOD_RECOVER };
        self.blood =
            (self.blood + (back.max(own) - an_hour / HOUR) * minutes).clamp(0.0, MAX_BLOOD);
        // What a treated trauma left behind runs out on its own clock.
        for l in &mut self.lasting {
            l.left -= minutes;
        }
        self.lasting.retain(|l| l.left > 0.0);
        // A beam with *mender* on it mends the parts faster.
        let change = HEALTH_RECOVER * held.map_or(1.0, |h| h.mend);
        // Over the three parts in proportion to their size, so the total
        // mends the way the one bar did. Legs that are gone do not grow back, and a part at nothing with its
        // trauma untreated stays there: only a medkit starts it again.
        for part in Part::ALL {
            let i = part as usize;
            if part == Part::Legs && self.legs_lost >= 2 {
                self.parts[i] = 0.0;
                continue;
            }
            if self.traumas[i].is_some() {
                self.parts[i] = 0.0;
                continue;
            }
            let share = part.max() / MAX_HEALTH;
            self.parts[i] = (self.parts[i] + change * share * minutes).clamp(0.0, part.max());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parts_add_to_a_hundred_and_the_rolls_split_as_asked() {
        // --- the_parts_add_to_a_hundred_and_the_odds_to_one ---
        {
            let total: f32 = Part::ALL.iter().map(|p| p.max()).sum();
            assert_eq!(total, MAX_HEALTH);
            let odds: f32 = Part::HIT_ODDS.iter().sum();
            assert!((odds - 1.0).abs() < 1e-6);
            assert_eq!(Part::hit_by(0.0), Part::Head);
            assert_eq!(Part::hit_by(0.049), Part::Head);
            assert_eq!(Part::hit_by(0.05), Part::Body);
            assert_eq!(Part::hit_by(0.799), Part::Body);
            assert_eq!(Part::hit_by(0.8), Part::Legs);
            assert_eq!(Part::hit_by(0.999), Part::Legs);
        }

        // --- the_rolls_split_as_asked ---
        {
            assert_eq!(Trauma::roll(Part::Head, 0.0), Trauma::HeavyConcussion);
            assert_eq!(Trauma::roll(Part::Head, 0.34), Trauma::SkullFracture);
            assert_eq!(Trauma::roll(Part::Head, 0.99), Trauma::CranialTrauma);
            assert_eq!(Trauma::roll(Part::Body, 0.0), Trauma::InternalBleeding);
            assert_eq!(Trauma::roll(Part::Body, 0.5), Trauma::BrokenRibs);
            assert_eq!(Trauma::roll(Part::Body, 0.7), Trauma::ChestTrauma);
            assert_eq!(Trauma::roll(Part::Legs, 0.0), Trauma::FracturedFemur);
            assert_eq!(Trauma::roll(Part::Legs, 0.449), Trauma::FracturedFemur);
            assert_eq!(Trauma::roll(Part::Legs, 0.45), Trauma::ShatteredKnee);
            assert_eq!(Trauma::roll(Part::Legs, 0.899), Trauma::ShatteredKnee);
            assert_eq!(Trauma::roll(Part::Legs, 0.9), Trauma::CrushedRightLeg);
            assert_eq!(Trauma::roll(Part::Legs, 0.949), Trauma::CrushedRightLeg);
            assert_eq!(Trauma::roll(Part::Legs, 0.95), Trauma::CrushedLeftLeg);
            assert_eq!(Trauma::roll(Part::Legs, 1.0), Trauma::CrushedLeftLeg);
            for t in Trauma::ALL {
                assert_eq!(Trauma::from_code(t.code()), Some(t));
                assert_eq!(Trauma::roll(t.part(), 0.5).part(), t.part());
            }
        }
    }

    #[test]
    fn a_body_shot_wears_it_down_and_a_head_at_nothing_is_dying_not_dead() {
        let mut h = Health::new();
        assert_eq!(h.shot(Part::Body, 12.0, false, 0.0), None);
        assert_eq!(h.part(Part::Body), 63.0);
        assert_eq!(h.points(), 88.0);
        assert!(!h.is_dead());
        assert_eq!(
            h.shot(Part::Head, 12.0, false, 0.5),
            Some(Trauma::SkullFracture)
        );
        assert!(!h.is_dead(), "a part at nothing is a dying state");
        assert!(h.dying());
        assert_eq!(h.trauma(Part::Head), Some(Trauma::SkullFracture));
        assert_eq!(h.part(Part::Head), 0.0);
        // Another hit on the same part is a wound and nothing more.
        assert_eq!(h.shot(Part::Head, 12.0, false, 0.0), None);
        assert_eq!(h.wounds(Part::Head), 2);
        // The part does not mend on its own, and the fracture bleeds it
        // ten a quarter hour besides the wounds.
        h.update(HOUR * 0.25);
        assert_eq!(h.part(Part::Head), 0.0);
        let lost = MAX_BLOOD - h.blood();
        assert!((lost - (10.0 + 3.0 * 2.5)).abs() < 1e-3, "{lost}");
        // Treated: the head starts again from half, nothing lasting.
        assert_eq!(h.treat(Part::Head), Some(Trauma::SkullFracture));
        assert!(!h.dying());
        assert_eq!(h.part(Part::Head), Part::Head.max() * TREATED_TO);
        assert!(h.lasting().is_empty());
        assert_eq!(h.treat(Part::Head), None, "nothing left to treat");
    }

    #[test]
    fn an_untreated_bleed_kills_and_a_concussion_lasts_two_days_after() {
        let mut h = Health::new();
        assert_eq!(
            h.shot(Part::Body, 75.0, false, 0.0),
            Some(Trauma::InternalBleeding)
        );
        // Forty an hour, and the wound itself ten: gone in two hours.
        h.update(HOUR * 1.5);
        assert!(!h.is_dead(), "{}", h.blood());
        h.update(HOUR * 0.5 + 1.0);
        assert!(h.is_dead(), "bled out, {}", h.blood());

        let mut h = Health::new();
        assert_eq!(
            h.shot(Part::Head, 5.0, false, 0.0),
            Some(Trauma::HeavyConcussion)
        );
        assert_eq!(h.pace(), 0.75);
        assert_eq!(h.works_at(), 0.75);
        h.bandage(Part::Head);
        h.update(DAY);
        assert!(h.dying(), "nothing mends it but a medkit");
        assert_eq!(h.part(Part::Head), 0.0);
        assert_eq!(h.treat(Part::Head), Some(Trauma::HeavyConcussion));
        assert_eq!(h.pace(), 0.75, "and the two days after");
        assert_eq!(h.lasting().len(), 1);
        h.update(DAY);
        assert_eq!(h.pace(), 0.75);
        h.update(DAY + 1.0);
        assert_eq!(h.pace(), 1.0);
        assert!(h.lasting().is_empty());
    }

    #[test]
    fn a_crushed_leg_is_lost_for_ever_and_a_knee_barely_moves() {
        let mut h = Health::new();
        assert_eq!(h.pace(), 1.0);
        assert_eq!(h.shot(Part::Legs, 12.0, false, 0.0), None);
        assert_eq!(
            h.shot(Part::Legs, 12.0, false, 0.92),
            Some(Trauma::CrushedRightLeg),
            "the second shot takes the leg"
        );
        assert_eq!(h.legs_lost(), 1);
        assert_eq!(h.part(Part::Legs), 0.0, "and the stump waits for a medkit");
        assert!((h.pace() - LEG_LOST_PACE).abs() < 1e-6);
        assert!(!h.is_dead());
        h.update(HOUR * 0.25);
        assert!(h.blood() < MAX_BLOOD - 10.0, "it bleeds until treated");
        assert_eq!(h.treat(Part::Legs), Some(Trauma::CrushedRightLeg));
        assert_eq!(h.part(Part::Legs), Part::Legs.max() * TREATED_TO);
        assert!((h.pace() - LEG_LOST_PACE).abs() < 1e-6, "for ever");
        h.bandage(Part::Legs);
        h.update(DAY * 3.0);
        assert!((h.pace() - LEG_LOST_PACE).abs() < 1e-6, "for ever");
        // The other one too, and there are none left.
        h.shot(Part::Legs, 20.0, false, 0.96);
        assert_eq!(h.legs_lost(), 2);
        assert_eq!(h.treat(Part::Legs), Some(Trauma::CrushedLeftLeg));
        assert_eq!(h.part(Part::Legs), 0.0);
        assert!((h.pace() - LEG_LOST_PACE * LEG_LOST_PACE).abs() < 1e-6);
        assert!(!h.is_dead(), "no legs is not dead");
        assert_eq!(
            h.shot(Part::Legs, 20.0, false, 0.96),
            None,
            "nothing left to take"
        );
        h.update(DAY);
        assert_eq!(h.part(Part::Legs), 0.0, "and nothing grows back");

        let mut h = Health::new();
        assert_eq!(
            h.shot(Part::Legs, 20.0, false, 0.5),
            Some(Trauma::ShatteredKnee)
        );
        assert_eq!(h.pace(), 0.25);
        assert_eq!(h.works_at(), 1.0);
        h.treat(Part::Legs);
        assert_eq!(h.pace(), 0.75);
        assert_eq!(h.legs_lost(), 0);
    }

    #[test]
    fn wounds_bleed_a_bim_out_in_an_hour_a_cut_three_units_and_a_bandage_closes_the_lot() {
        // --- ten_wounds_bleed_a_bim_out_in_an_hour_and_a_bandage_stops_it ---
        {
            let mut h = Health::new();
            for _ in 0..10 {
                h.shot(Part::Body, 1.0, false, 0.0);
            }
            assert_eq!(h.bleeding(), 10);
            // A quarter of an hour is a quarter of its blood gone: three
            // quarters left, which is not *under* three quarters.
            h.update(HOUR * 0.25);
            assert!((h.blood() - 75.0).abs() < 1e-3, "{}", h.blood());
            assert_eq!(h.pace(), 1.0, "three quarters is not under it");
            h.update(1.0);
            assert_eq!(h.pace(), 0.5);
            assert!(!h.unconscious());
            // Half an hour and half its blood: the line it goes out at
            // (feature 89), and it is not under it yet.
            h.update(HOUR * 0.25 - 1.0);
            assert!((h.blood() - 50.0).abs() < 1e-3, "{}", h.blood());
            assert!(!h.unconscious(), "half is not under half");
            h.update(1.0);
            assert!(h.unconscious(), "{}", h.blood());
            assert!(!h.is_dead());
            assert!(h.bandage(Part::Body));
            assert_eq!(h.bleeding(), 0);
            assert!(!h.bandage(Part::Body), "nothing left to dress");
            let before = h.blood();
            h.update(HOUR);
            assert!(h.blood() > before, "blood comes back once nothing is open");

            let mut h = Health::new();
            for _ in 0..10 {
                h.shot(Part::Body, 1.0, false, 0.0);
            }
            h.update(HOUR);
            assert!(h.is_dead(), "bled out");
        }

        // --- a_cut_bleeds_three_units_and_a_bandage_closes_the_lot ---
        {
            let mut h = Health::new();
            assert_eq!(h.shot(Part::Body, 12.0, true, 0.0), None);
            assert_eq!(h.wounds(Part::Body), CUT_WOUND);
            assert_eq!(h.bleeding(), 3);
            assert_eq!(h.part(Part::Body), 63.0, "the damage is the damage");
            let mut shot = Health::new();
            shot.shot(Part::Body, 12.0, false, 0.0);
            h.update(HOUR * 0.1);
            shot.update(HOUR * 0.1);
            let cut_lost = MAX_BLOOD - h.blood();
            let shot_lost = MAX_BLOOD - shot.blood();
            assert!(
                (cut_lost - 3.0 * shot_lost).abs() < 1e-3,
                "{cut_lost} vs {shot_lost}"
            );
            assert!(h.bandage(Part::Body));
            assert_eq!(h.bleeding(), 0);
        }
    }
}
