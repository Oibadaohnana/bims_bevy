//! Going without, and what it costs: food on one side, sleep on the other.
//!
//! An empty stomach is not itself harmful — the Bim can be hungry for a while
//! and simply be hungry. What does the damage is staying that way, so the
//! measure here is *time spent with nothing in it* rather than the hunger
//! level, and the three stages of malnutrition are thresholds on that clock.
//!
//! Only the last stage costs health. The first two are a warning: a Bim that
//! is merely slow and tiring easily can still be fed and will come right.
//!
//! # The body is three parts, and the blood is a fourth number
//!
//! Health is not one bar but three — the head, the body and the legs, each
//! with a base of its own ([`Part::max`]: 5, 75 and 20, a hundred all told)
//! — and what the panel calls health is the three added up. A shot lands
//! on one part ([`Part::HIT_ODDS`]: one in twenty the head, three in four
//! the body, one in five the legs) and takes the weapon's damage off that
//! part alone. Starvation and mending run over all three in proportion,
//! so the total behaves exactly as the one bar did.
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
//! wounds, [`HEAVY_BLEED`] or [`SLOW_BLEED`] an hour. Under half, the Bim
//! walks at half its pace; under [`OUT_AT`], it is out cold where it
//! stands; at nothing it is dead. A bandage ([`Health::bandage`]) closes
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
/// below the second it is out cold.
pub const SLOWED_AT: f32 = 0.5;
pub const OUT_AT: f32 = 0.4;

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

/// What a treated trauma leaves behind for a while: the trauma's
/// [`Trauma::after`] pace and effort, for `left` more game minutes.
#[derive(Clone, Copy, PartialEq, Debug)]
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

/// Hunger at or below this counts as an empty stomach, and rest at or below it
/// as running on nothing.
const EMPTY: f32 = 0.02;

/// Game minutes of no sleep before each stage of drowsiness sets in. Faster
/// than starvation, because it is: a night missed tells before a meal does.
///
/// The gaps widen — six hours, then eight — so the first stage arrives as a
/// warning with time to act on it, and the last takes a full night of being
/// kept up to reach.
const SLEEPY_AT: f32 = 4.0 * HOUR;
const DEPRIVED_AT: f32 = 10.0 * HOUR;
const WRECKED_AT: f32 = 18.0 * HOUR;

/// Rest above this clears the whole thing. Unlike hunger, which is wound back
/// a little for every mouthful, sleeplessness only lifts when the Bim has
/// properly slept — which is why the fifteen-minute nods-off at the worst
/// stage, worth a few per cent of rest each, never lift it.
const SLEPT_AT: f32 = 0.80;

/// Game minutes on an empty stomach before each stage sets in. A day without
/// food to reach the worst of it; feeding at any point walks it back.
const MILD_AT: f32 = 8.0 * HOUR;
const MODERATE_AT: f32 = 16.0 * HOUR;
const EXTREME_AT: f32 = 24.0 * HOUR;

/// Starvation is undone faster than it sets in, so one meal is visibly worth
/// something rather than being lost in a day-long ledger.
const MEND_RATE: f32 = 3.0;

/// Health goes from full to nothing in a day of extreme malnutrition, and
/// takes two days of eating properly to come back.
const HEALTH_DRAIN: f32 = MAX_HEALTH / DAY;
const HEALTH_RECOVER: f32 = MAX_HEALTH / (2.0 * DAY);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Malnutrition {
    None,
    Mild,
    Moderate,
    Extreme,
}

impl Malnutrition {
    /// 0 for a well-fed Bim, then 1, 2, 3. The host names them.
    pub fn stage(self) -> u32 {
        match self {
            Malnutrition::None => 0,
            Malnutrition::Mild => 1,
            Malnutrition::Moderate => 2,
            Malnutrition::Extreme => 3,
        }
    }

    /// How fast it walks, as a fraction of its usual pace.
    pub fn pace(self) -> f32 {
        match self {
            Malnutrition::None => 1.0,
            Malnutrition::Mild => 0.85,
            Malnutrition::Moderate => 0.70,
            Malnutrition::Extreme => 0.55,
        }
    }

    /// How much faster it tires: double from the second stage, triple from the
    /// third, so a starving Bim needs more sleep as well as moving worse.
    pub fn tiring(self) -> f32 {
        match self {
            Malnutrition::None | Malnutrition::Mild => 1.0,
            Malnutrition::Moderate => 2.0,
            Malnutrition::Extreme => 3.0,
        }
    }
}

/// How far gone for want of sleep.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Drowsiness {
    None,
    Sleepy,
    Deprived,
    Wrecked,
}

impl Drowsiness {
    /// 0 wide awake, then 1, 2, 3. The host names them.
    pub fn stage(self) -> u32 {
        match self {
            Drowsiness::None => 0,
            Drowsiness::Sleepy => 1,
            Drowsiness::Deprived => 2,
            Drowsiness::Wrecked => 3,
        }
    }

    /// How much longer an errand takes, all told.
    pub fn drag(self) -> f32 {
        match self {
            Drowsiness::None => 1.0,
            Drowsiness::Sleepy => 1.25,
            Drowsiness::Deprived => 1.50,
            Drowsiness::Wrecked => 2.00,
        }
    }

    /// The chance that a step, on finishing, has to be done again.
    ///
    /// A step repeated with probability `p` takes `1 / (1 - p)` times as long
    /// on average, so this is the inverse: the fumbling is random, but how
    /// much it costs over a whole errand is not.
    pub fn fumble(self) -> f32 {
        1.0 - 1.0 / self.drag()
    }

    /// Only at the worst of it does the Bim drop off where it stands.
    pub fn nods_off(self) -> bool {
        self == Drowsiness::Wrecked
    }
}

pub struct Health {
    /// Game minutes of empty stomach, wound back by eating.
    starved: f32,
    /// Game minutes awake on no rest at all, cleared by a proper sleep.
    sleepless: f32,
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
            starved: 0.0,
            sleepless: 0.0,
            parts: [Part::Head.max(), Part::Body.max(), Part::Legs.max()],
            legs_lost: 0,
            blood: MAX_BLOOD,
            wounds: [0; 3],
            traumas: [None; 3],
            lasting: Vec::new(),
        }
    }

    pub fn stage(&self) -> Malnutrition {
        if self.starved >= EXTREME_AT {
            Malnutrition::Extreme
        } else if self.starved >= MODERATE_AT {
            Malnutrition::Moderate
        } else if self.starved >= MILD_AT {
            Malnutrition::Mild
        } else {
            Malnutrition::None
        }
    }

    pub fn drowsiness(&self) -> Drowsiness {
        if self.sleepless >= WRECKED_AT {
            Drowsiness::Wrecked
        } else if self.sleepless >= DEPRIVED_AT {
            Drowsiness::Deprived
        } else if self.sleepless >= SLEEPY_AT {
            Drowsiness::Sleepy
        } else {
            Drowsiness::None
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

    /// What treated traumas have left behind, each with the game minutes
    /// it has left to run.
    pub fn lasting(&self) -> &[Lasting] {
        &self.lasting
    }

    /// Dead: bled out, or the head and the body both at nothing with no
    /// trauma on either — starved to nothing, or given up. A part at
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
        let legs = LEG_LOST_PACE.powi(self.legs_lost as i32);
        let blood = if self.blood < MAX_BLOOD * SLOWED_AT {
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
        let i = part as usize;
        let trauma = self.traumas[i].take()?;
        self.parts[i] = if part == Part::Legs && self.legs_lost >= 2 {
            0.0
        } else {
            part.max() * TREATED_TO
        };
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

    /// Take a flat amount off the body, never past nothing. Hunger works on
    /// the health bar over hours; this is for the things that happen all at
    /// once — so far, a Bim nobody has spoken to in a week hurting itself.
    pub fn hurt(&mut self, points: f32) {
        let body = &mut self.parts[Part::Body as usize];
        *body = (*body - points).max(0.0);
    }

    /// The end of it, by the Bim's own hand. Kept apart from [`Health::hurt`]
    /// with everything left of the bar taken at once, so that what happened is
    /// legible here rather than being a subtraction that happened to reach
    /// zero.
    pub fn give_up(&mut self) {
        self.parts = [0.0; 3];
        self.traumas = [None; 3];
    }

    /// `minutes` is game minutes elapsed, `food` and `rest` the levels now,
    /// and `resting` whether the Bim is actually asleep this instant.
    ///
    /// Starvation keeps running while the Bim sleeps: you do not stop starving
    /// because you are asleep, and a Bim that tires three times as fast spends
    /// more of its day in bed — which is exactly the spiral the third stage of
    /// malnutrition is meant to be. Sleeplessness plainly does not, so that one
    /// only counts waking minutes.
    pub fn update(&mut self, minutes: f32, food: f32, rest: f32, resting: bool) {
        // Nothing comes back from nothing. Health mends on its own while the
        // Bim is fed, and without this the bar taken to zero by anything
        // *sudden* — a Bim hurting itself, a Bim giving up — is back above
        // zero on the very next frame, before `Game` has looked at it. The
        // death then simply never happens: the run ends with a Bim whose
        // health reads nought and who is still walking about.
        if self.is_dead() {
            return;
        }
        // The blood: out through every open wound and every untreated
        // trauma that bleeds, back on its own once nothing does. Bleeding
        // to nothing is the death, and the check at the top of the next
        // tick is what says so.
        let open = self.bleeding();
        let trauma: f32 = self.traumas.iter().flatten().map(|t| t.bleed()).sum();
        let an_hour = open as f32 * BLEED_PER_WOUND + trauma;
        if an_hour > 0.0 {
            let loss = an_hour / HOUR * minutes;
            self.blood = (self.blood - loss).max(0.0);
        } else {
            self.blood = (self.blood + BLOOD_RECOVER * minutes).min(MAX_BLOOD);
        }
        // What a treated trauma left behind runs out on its own clock.
        for l in &mut self.lasting {
            l.left -= minutes;
        }
        self.lasting.retain(|l| l.left > 0.0);
        if food <= EMPTY {
            self.starved += minutes;
        } else {
            self.starved = (self.starved - minutes * MEND_RATE).max(0.0);
        }

        if rest > SLEPT_AT {
            // Properly slept. Nothing short of this clears it.
            self.sleepless = 0.0;
        } else if rest <= EMPTY && !resting {
            self.sleepless += minutes;
        }

        let change = if self.stage() == Malnutrition::Extreme {
            -HEALTH_DRAIN
        } else if self.starved <= 0.0 {
            HEALTH_RECOVER
        } else {
            // Malnourished but not yet starving outright: no worse, no better.
            0.0
        };
        // Over the three parts in proportion to their size, so the total
        // goes from full to nothing in a day the way the one bar did. Legs
        // that are gone do not grow back, and a part at nothing with its
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
    fn the_parts_add_to_a_hundred_and_the_odds_to_one() {
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
        h.update(HOUR * 0.25, 1.0, 1.0, false);
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
        h.update(HOUR * 1.5, 1.0, 1.0, false);
        assert!(!h.is_dead(), "{}", h.blood());
        h.update(HOUR * 0.5 + 1.0, 1.0, 1.0, false);
        assert!(h.is_dead(), "bled out, {}", h.blood());

        let mut h = Health::new();
        assert_eq!(
            h.shot(Part::Head, 5.0, false, 0.0),
            Some(Trauma::HeavyConcussion)
        );
        assert_eq!(h.pace(), 0.75);
        assert_eq!(h.works_at(), 0.75);
        h.bandage(Part::Head);
        h.update(DAY, 1.0, 1.0, false);
        assert!(h.dying(), "nothing mends it but a medkit");
        assert_eq!(h.part(Part::Head), 0.0);
        assert_eq!(h.treat(Part::Head), Some(Trauma::HeavyConcussion));
        assert_eq!(h.pace(), 0.75, "and the two days after");
        assert_eq!(h.lasting().len(), 1);
        h.update(DAY, 1.0, 1.0, false);
        assert_eq!(h.pace(), 0.75);
        h.update(DAY + 1.0, 1.0, 1.0, false);
        assert_eq!(h.pace(), 1.0);
        assert!(h.lasting().is_empty());
    }

    #[test]
    fn the_rolls_split_as_asked() {
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
        h.update(HOUR * 0.25, 1.0, 1.0, false);
        assert!(h.blood() < MAX_BLOOD - 10.0, "it bleeds until treated");
        assert_eq!(h.treat(Part::Legs), Some(Trauma::CrushedRightLeg));
        assert_eq!(h.part(Part::Legs), Part::Legs.max() * TREATED_TO);
        assert!((h.pace() - LEG_LOST_PACE).abs() < 1e-6, "for ever");
        h.bandage(Part::Legs);
        h.update(DAY * 3.0, 1.0, 1.0, false);
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
        h.update(DAY, 1.0, 1.0, false);
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
    fn ten_wounds_bleed_a_bim_out_in_an_hour_and_a_bandage_stops_it() {
        let mut h = Health::new();
        for _ in 0..10 {
            h.shot(Part::Body, 1.0, false, 0.0);
        }
        assert_eq!(h.bleeding(), 10);
        h.update(HOUR * 0.5, 1.0, 1.0, false);
        assert!((h.blood() - 50.0).abs() < 1e-3, "{}", h.blood());
        assert_eq!(h.pace(), 1.0, "half is not under half");
        h.update(1.0, 1.0, 1.0, false);
        assert_eq!(h.pace(), 0.5);
        assert!(!h.unconscious());
        h.update(HOUR * 0.25, 1.0, 1.0, false);
        assert!(h.unconscious(), "{}", h.blood());
        assert!(!h.is_dead());
        assert!(h.bandage(Part::Body));
        assert_eq!(h.bleeding(), 0);
        assert!(!h.bandage(Part::Body), "nothing left to dress");
        let before = h.blood();
        h.update(HOUR, 1.0, 1.0, false);
        assert!(h.blood() > before, "blood comes back once nothing is open");

        let mut h = Health::new();
        for _ in 0..10 {
            h.shot(Part::Body, 1.0, false, 0.0);
        }
        h.update(HOUR, 1.0, 1.0, false);
        assert!(h.is_dead(), "bled out");
    }

    #[test]
    fn a_cut_bleeds_three_units_and_a_bandage_closes_the_lot() {
        let mut h = Health::new();
        assert_eq!(h.shot(Part::Body, 12.0, true, 0.0), None);
        assert_eq!(h.wounds(Part::Body), CUT_WOUND);
        assert_eq!(h.bleeding(), 3);
        assert_eq!(h.part(Part::Body), 63.0, "the damage is the damage");
        let mut shot = Health::new();
        shot.shot(Part::Body, 12.0, false, 0.0);
        h.update(HOUR * 0.1, 1.0, 1.0, false);
        shot.update(HOUR * 0.1, 1.0, 1.0, false);
        let cut_lost = MAX_BLOOD - h.blood();
        let shot_lost = MAX_BLOOD - shot.blood();
        assert!(
            (cut_lost - 3.0 * shot_lost).abs() < 1e-3,
            "{cut_lost} vs {shot_lost}"
        );
        assert!(h.bandage(Part::Body));
        assert_eq!(h.bleeding(), 0);
    }

    #[test]
    fn starvation_still_takes_a_day_over_the_whole_of_it() {
        let mut h = Health::new();
        // Up to the last stage — the stage is read after the minutes are
        // added, so the step that crosses the line already drains — then a
        // day on nothing.
        h.update(EXTREME_AT - 1.0, 0.0, 1.0, false);
        assert!(!h.is_dead());
        assert_eq!(h.points(), MAX_HEALTH);
        h.update(DAY * 0.5, 0.0, 1.0, false);
        assert!((h.points() - 50.0).abs() < 0.5, "{}", h.points());
        assert!(!h.is_dead());
        h.update(DAY * 0.5 + 1.0, 0.0, 1.0, false);
        assert!(h.is_dead());
    }
}
