//! Relics (feature 106): passive items a player's Bim wins and keeps for
//! the rest of the run, in the manner of Slay the Spire, and the unlocks
//! that put more of them in the pool between runs.
//!
//! # What a relic is
//!
//! A [`Relic`] is held by **one player's Bim** — never a bot's — and is
//! never moved, dropped or sold: it is on the Bim that was given it until
//! the run ends, through a death and a buyback like a level. A relic is
//! held once a run at most: the run's pool ([`Relics::pool`]) loses one
//! the moment a Bim **gets** it — a reward or a cache chosen, a trader's
//! bought — and an offer takes nothing out of it (task 117). What is on
//! offer, pending or on a trader's table is kept out of every other draw
//! while it is ([`Relics::in_play`], `World::draw_relics`), and back in the
//! running the moment it is not.
//!
//! A relic's **tier** (one, two, three) is data the player never sees:
//! it decides the odds a draw picks it at ([`tier_odds`]) and its price at
//! a trader, and nothing else.
//!
//! # What a relic does
//!
//! Every relic is a row of [`RELICS`]: an id, a tier, whether it is in a
//! new profile's pool, and an [`Effect`] made of three kinds of hook, so a
//! new relic is a row and at most a few lines where its hook is read:
//!
//! - a **stat modifier** ([`Effect::Stat`]): a percentage on a [`Stat`] —
//!   weapon damage, accuracy, move speed, armour, class cooldowns,
//!   bounty — under a [`When`] (always, or while another player's Bim is
//!   down). `World::skill_of`, the cooldowns and the bounty read
//!   [`stat_percent`];
//! - a **trigger** ([`Effect::On`], a [`Hook`]): on a kill, a hit taken,
//!   going down, a mission's start or an ability used, an [`Action`] —
//!   getting up again, cooldowns taken off, a spell untouchable — under
//!   **conditions**: once a mission, and below a share of health;
//! - and the one that fits neither, [`Effect::EveryNthShot`], which the
//!   room counts (`bims::combat::Skill::overcharge`).
//!
//! No number is written here: every one is a constant in [`crate::data`].
//! No word either: the names and the descriptions are the app's
//! (`names::relic_name`).
//!
//! # Where relics come from
//!
//! A site cleared **with machines in it** offers [`data::RELIC_OFFER`]
//! relics, a held site may hide a **cache**
//! ([`data::RELIC_CACHE_CHANCE`]) that gives one, and a trader sells one.
//! All three draw by one roll ([`offer`]): a tier by the day's odds, a
//! relic of it, and another tier by the same odds when that one has none
//! left. The site's enemy tier has nothing to do with it. Either way the crew
//! choose together ([`RelicChoice`]): a player proposes a relic and the
//! player's Bim to have it, every connected player accepts, and a new
//! proposal clears every acceptance — the world map's vote over again.
//! A cache's relic is **pending** until the site is cleared, and lost if
//! the crew leave first.
//!
//! # Unlocks
//!
//! A [`Profile`] — the app keeps it on disk, beside the saves — is which
//! relics and classes a player has unlocked and how many runs they have
//! won. A won run unlocks [`data::RELICS_UNLOCKED_PER_WIN`] relics, the
//! first still locked in [`Relic::ALL`]'s order ([`Profile::record_run`]).
//! The host's profile is the run's pool, fixed at the start. Every relic
//! starts unlocked now, so a win unlocks nothing; the machinery stays for
//! a relic marked locked again in [`RELICS`].

use crate::class::Class;
use crate::data;

/// A relic, by its place in [`RELICS`]. The code is what crosses the seam
/// and goes into the checksum; a relic added later goes on the end.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Relic {
    FocusingLens = 0,
    ServoBraces = 1,
    FieldPlating = 2,
    CoolantLoop = 3,
    SteadyGrip = 4,
    TraumaKit = 5,
    SecondWind = 6,
    SalvageBeacon = 7,
    OverchargeCell = 8,
    LastStand = 9,
    KillRelay = 10,
    PhaseHarness = 11,
    // Task 118: five patches of five, in the order a win unlocks them.
    MarksmansHabit = 12,
    ServoCutter = 13,
    CripplersMark = 14,
    PressureSeal = 15,
    QuickWrap = 16,
    ClotBooster = 17,
    BlindSpot = 18,
    SprintCoil = 19,
    SignalScrambler = 20,
    FieldRadio = 21,
    Spotter = 22,
    SquadMorale = 23,
    HazardPay = 24,
    TradeLicense = 25,
    RestockCodes = 26,
    PartsBroker = 27,
    TetherField = 28,
    WideAngleOptics = 29,
    CoverFormation = 30,
    StrongWill = 31,
    TotalTeardown = 32,
    Lifeline = 33,
    Crossfire = 34,
    RallyPoint = 35,
    WarChest = 36,
}

impl Relic {
    /// Every relic, in list order: the order a win unlocks them in.
    pub const ALL: [Relic; 37] = [
        Relic::FocusingLens,
        Relic::ServoBraces,
        Relic::FieldPlating,
        Relic::CoolantLoop,
        Relic::SteadyGrip,
        Relic::TraumaKit,
        Relic::SecondWind,
        Relic::SalvageBeacon,
        Relic::OverchargeCell,
        Relic::LastStand,
        Relic::KillRelay,
        Relic::PhaseHarness,
        Relic::MarksmansHabit,
        Relic::ServoCutter,
        Relic::CripplersMark,
        Relic::PressureSeal,
        Relic::QuickWrap,
        Relic::ClotBooster,
        Relic::BlindSpot,
        Relic::SprintCoil,
        Relic::SignalScrambler,
        Relic::FieldRadio,
        Relic::Spotter,
        Relic::SquadMorale,
        Relic::HazardPay,
        Relic::TradeLicense,
        Relic::RestockCodes,
        Relic::PartsBroker,
        Relic::TetherField,
        Relic::WideAngleOptics,
        Relic::CoverFormation,
        Relic::StrongWill,
        Relic::TotalTeardown,
        Relic::Lifeline,
        Relic::Crossfire,
        Relic::RallyPoint,
        Relic::WarChest,
    ];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<Relic> {
        Relic::ALL.get(code as usize).copied()
    }

    /// Its row of [`RELICS`].
    pub fn def(self) -> &'static RelicDef {
        &RELICS[self as usize]
    }

    /// One, two or three: what the odds of drawing it ([`tier_odds`]) and
    /// its price at a trader are read off (task 117). Never shown.
    pub fn tier(self) -> u8 {
        self.def().tier
    }

    /// Whether a new profile has it in its pool.
    pub fn starts_unlocked(self) -> bool {
        self.def().first
    }

    /// What it does.
    pub fn effect(self) -> Effect {
        self.def().effect
    }
}

/// A number a relic moves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stat {
    /// What every bolt and blow of its weapon does.
    Damage,
    /// Its odds of hitting.
    Accuracy,
    /// Its pace, at all times.
    MoveSpeed,
    /// The protection of what it wears.
    Armour,
    /// How long its **class's** cooldowns are — the engineer's kits, the
    /// soldier's grenades, the taunt and the rally. A minus is shorter.
    Cooldowns,
    /// What the Republic pays for a machine it destroyed.
    Bounty,
    /// What its bolts and blows do **to a machine**, read where the hit
    /// lands on one (task 118) — under a [`When`] about that hit, where
    /// [`Stat::Damage`] is the weapon's own and holds everywhere.
    MachineDamage,
    /// What it takes of every hit that lands on it. A minus is less.
    DamageTaken,
    /// What a trader asks of the crew. A minus is cheaper.
    TraderPrices,
}

/// When a stat modifier holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum When {
    Always,
    /// While another player's Bim is down — downed.
    OtherPlayerDown,
    /// A hit on a machine's arms or legs while it still has them.
    OnLimb,
    /// A machine missing its arms or its legs — a hit on it, or a kill.
    Crippled,
    /// A hit on a machine from the side or behind: outside the front arc
    /// a Guardian's shield covers, for every kind.
    Flanked,
}

/// What a stat modifier's [`When`] is asked against: the state of the
/// Bim's crew, and — for a hit or a kill on a machine — of that hit.
/// Everything false is the plain case, what a hit on nothing reads.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Situation {
    pub other_down: bool,
    pub on_limb: bool,
    pub crippled: bool,
    pub flanked: bool,
}

/// What sets a relic's action off.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Trigger {
    /// A mission begins.
    MissionStart,
    /// A machine it hit last is destroyed.
    Kill,
    /// A hit lands on it.
    HitTaken,
    /// It goes down — downed.
    Downed,
    /// It uses one of its class's two keys.
    AbilityUse,
    /// A machine destroyed, by anybody (task 142; it was one it or a bot
    /// hit last).
    CrewKill,
    /// A machine it hit last destroyed by that hit from the side or
    /// behind.
    FlankKill,
    /// It revives a downed crewmate (task 120; it was a dressing).
    Revived,
    /// A player's Bim — another's, or its own — falls under
    /// `data::LIFELINE_BELOW_PERCENT` of its health (task 142). A bot never
    /// says it.
    PlayerLow,
}

/// What a relic's trigger does.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Action {
    /// Up again `after` seconds of the mission clock, at `health_percent`
    /// of its health, if it is still down and alive by then.
    GetUp { after: f64, health_percent: u32 },
    /// Every class cooldown running on it that many seconds shorter.
    CooldownsLess { seconds: f64 },
    /// No hit takes anything from it for that many seconds.
    Untouchable { seconds: f32 },
    /// Its pace raised by `percent` for `seconds`.
    Sprint { percent: i32, seconds: f64 },
    /// No machine aims at it for `seconds`.
    Unseen { seconds: f64 },
    /// The crewmate it revived takes `percent` less of every hit for
    /// `seconds`.
    Tether { percent: i32, seconds: f64 },
    /// It and the player's Bim that fell low within `tiles` of it — or it
    /// alone, when that was itself — each given a shield of `hp` hit points
    /// for `seconds` (task 142).
    Shield { tiles: f32, hp: f32, seconds: f32 },
    /// Every crewmate down within `tiles` of it up again at
    /// `health_percent` of its health.
    RallyUp { tiles: f32, health_percent: u32 },
    /// `points` of health put back into the body it revived.
    Heal { points: f32 },
}

/// A trigger, its conditions and its action.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Hook {
    pub trigger: Trigger,
    /// Fires once a mission at most.
    pub once_per_mission: bool,
    /// Fires only with its health under this share, in per cent.
    pub below_health: Option<u32>,
    /// Fires again only this many seconds of the mission clock after it
    /// last did.
    pub cooldown: Option<f64>,
    pub action: Action,
}

/// A hook with no condition: every time its trigger comes.
const fn on(trigger: Trigger, action: Action) -> Hook {
    Hook {
        trigger,
        once_per_mission: false,
        below_health: None,
        cooldown: None,
        action,
    }
}

/// A hook that fires once a mission at most.
const fn once(trigger: Trigger, action: Action) -> Hook {
    Hook {
        once_per_mission: true,
        ..on(trigger, action)
    }
}

/// A rule of a relic's own that no stat or trigger says, read where it
/// applies (task 118): each is one relic's.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Rule {
    /// Its first hit on each machine lands on a limb: *Marksman's Habit*.
    FirstHitOnLimb,
    /// A hit on a limb already gone tears into the chassis at
    /// `damage_percent` more: *Total Teardown*.
    TearIntoChassis { damage_percent: i32 },
    /// `hp_per_second` of health put back all the time it is alive:
    /// *Pressure Seal*.
    Regen { hp_per_second: f32 },
    /// `hp_per_second` of health put back for the first `seconds` after it
    /// goes down, while it is up again — a downed body is healed by
    /// nothing but a revive (task 120): *Clot Booster*.
    MendWhileDown { hp_per_second: f32, seconds: f64 },
    /// `tiles` added to its weapon's range while it stands still: *Wide
    /// Angle Optics* (task 142; it was a machine's front narrowed).
    StillRange { tiles: f32 },
    /// It and a crewmate further apart round a machine than `apart_cos`
    /// (a cosine), both within `tiles` of it, both do `damage_percent`
    /// more to it: *Crossfire*.
    Crossfire {
        damage_percent: i32,
        apart_cos: f32,
        tiles: f32,
    },
    /// The machine it hit last takes `damage_percent` more from every
    /// crewmate for `seconds`: *Spotter*.
    Spotter { damage_percent: i32, seconds: f64 },
    /// Money to the crew every site cleared: *Hazard Pay*.
    SitePay { money: economy::Money },
    /// Its abilities' effects last `percent` longer: *Strong Will* (task
    /// 142, where *Scrap Collector* paid for every machine it destroyed).
    LongerAbilities { percent: i32 },
    /// The trader's shelf rolled again, once a visit: *Restock Codes*.
    Restock,
    /// Its revives of a crewmate `seconds` quicker, never under
    /// `data::REVIVE_FLOOR_SECONDS`: *Trauma Kit* (task 120).
    QuickRevive { seconds: f32 },
    /// Its damage up `percent_per_thousand` for every thousand in the pool
    /// a player, to `cap`: *War Chest*.
    WarChest { percent_per_thousand: i32, cap: i32 },
}

/// What a relic does: one of the three kinds of hook.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Effect {
    Stat {
        stat: Stat,
        percent: i32,
        when: When,
    },
    /// Every `every`th shot of its weapon does `damage_percent` more.
    EveryNthShot {
        every: u32,
        damage_percent: i32,
    },
    On(Hook),
    /// Several hooks, each on its own trigger.
    OnEach(&'static [Hook]),
    /// A stat of every crewmate within `tiles` of it moved by `percent` —
    /// its own too with `own` (task 142; it was a flag for the bots alone,
    /// which *Cover Formation* was until then).
    Aura {
        stat: Stat,
        percent: i32,
        tiles: f32,
        own: bool,
    },
    /// A rule of its own ([`Rule`]).
    Rule(Rule),
}

/// A relic's row: the data the whole of it is.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RelicDef {
    pub relic: Relic,
    pub tier: u8,
    /// In a new profile's pool, rather than unlocked by a win.
    pub first: bool,
    pub effect: Effect,
}

const fn row(relic: Relic, tier: u8, first: bool, effect: Effect) -> RelicDef {
    RelicDef {
        relic,
        tier,
        first,
        effect,
    }
}

const fn stat(relic: Relic, tier: u8, first: bool, stat: Stat, percent: i32) -> RelicDef {
    when(relic, tier, first, stat, percent, When::Always)
}

const fn when(
    relic: Relic,
    tier: u8,
    first: bool,
    stat: Stat,
    percent: i32,
    when: When,
) -> RelicDef {
    row(
        relic,
        tier,
        first,
        Effect::Stat {
            stat,
            percent,
            when,
        },
    )
}

const fn rule(relic: Relic, tier: u8, first: bool, rule: Rule) -> RelicDef {
    row(relic, tier, first, Effect::Rule(rule))
}

/// Every relic there is, in [`Relic::ALL`]'s order.
pub const RELICS: [RelicDef; 37] = [
    stat(
        Relic::FocusingLens,
        1,
        true,
        Stat::Damage,
        data::FOCUSING_LENS_DAMAGE_PERCENT,
    ),
    stat(
        Relic::ServoBraces,
        1,
        true,
        Stat::MoveSpeed,
        data::SERVO_BRACES_SPEED_PERCENT,
    ),
    stat(
        Relic::FieldPlating,
        1,
        true,
        Stat::Armour,
        data::FIELD_PLATING_ARMOUR_PERCENT,
    ),
    stat(
        Relic::CoolantLoop,
        1,
        true,
        Stat::Cooldowns,
        -data::COOLANT_LOOP_COOLDOWN_PERCENT,
    ),
    stat(
        Relic::SteadyGrip,
        1,
        true,
        Stat::Accuracy,
        data::STEADY_GRIP_ACCURACY_PERCENT,
    ),
    rule(
        Relic::TraumaKit,
        1,
        true,
        Rule::QuickRevive {
            seconds: data::TRAUMA_KIT_REVIVE_SECONDS,
        },
    ),
    row(
        Relic::SecondWind,
        2,
        true,
        Effect::On(once(
            Trigger::Downed,
            Action::GetUp {
                after: data::SECOND_WIND_SECONDS,
                health_percent: data::SECOND_WIND_HEALTH_PERCENT,
            },
        )),
    ),
    stat(
        Relic::SalvageBeacon,
        2,
        true,
        Stat::Bounty,
        data::SALVAGE_BEACON_BOUNTY_PERCENT,
    ),
    row(
        Relic::OverchargeCell,
        2,
        true,
        Effect::EveryNthShot {
            every: data::OVERCHARGE_CELL_EVERY,
            damage_percent: data::OVERCHARGE_CELL_DAMAGE_PERCENT,
        },
    ),
    when(
        Relic::LastStand,
        3,
        true,
        Stat::Damage,
        data::LAST_STAND_DAMAGE_PERCENT,
        When::OtherPlayerDown,
    ),
    row(
        Relic::KillRelay,
        3,
        true,
        Effect::On(on(
            Trigger::Kill,
            Action::CooldownsLess {
                seconds: data::KILL_RELAY_SECONDS,
            },
        )),
    ),
    row(
        Relic::PhaseHarness,
        3,
        true,
        Effect::On(Hook {
            below_health: Some(data::PHASE_HARNESS_BELOW_PERCENT),
            ..once(
                Trigger::HitTaken,
                Action::Untouchable {
                    seconds: data::PHASE_HARNESS_SECONDS,
                },
            )
        }),
    ),
    // --- task 118 --------------------------------------------------------
    // Dismantler, Lifeline, Flanker, Command Net, Supply Line: their tier
    // ones and twos first, then the rest a win used to unlock. Every relic
    // is in a new profile's pool now; a `false` here locks one again.
    rule(Relic::MarksmansHabit, 1, true, Rule::FirstHitOnLimb),
    when(
        Relic::ServoCutter,
        1,
        true,
        Stat::MachineDamage,
        data::SERVO_CUTTER_DAMAGE_PERCENT,
        When::OnLimb,
    ),
    when(
        Relic::CripplersMark,
        2,
        true,
        Stat::MachineDamage,
        data::CRIPPLERS_MARK_DAMAGE_PERCENT,
        When::Crippled,
    ),
    // Lifeline heals hit points and never touches the blood, which a
    // health system without blood will not have.
    rule(
        Relic::PressureSeal,
        1,
        true,
        Rule::Regen {
            hp_per_second: data::PRESSURE_SEAL_HP_PER_SECOND,
        },
    ),
    row(
        Relic::QuickWrap,
        1,
        true,
        Effect::On(on(
            Trigger::Revived,
            Action::Heal {
                points: data::QUICK_WRAP_HEAL,
            },
        )),
    ),
    rule(
        Relic::ClotBooster,
        2,
        true,
        Rule::MendWhileDown {
            hp_per_second: data::CLOT_BOOSTER_HP_PER_SECOND,
            seconds: data::CLOT_BOOSTER_SECONDS,
        },
    ),
    when(
        Relic::BlindSpot,
        1,
        true,
        Stat::MachineDamage,
        data::BLIND_SPOT_DAMAGE_PERCENT,
        When::Flanked,
    ),
    row(Relic::SprintCoil, 1, true, Effect::OnEach(&SPRINT_COIL)),
    row(
        Relic::SignalScrambler,
        2,
        true,
        Effect::On(Hook {
            cooldown: Some(data::SIGNAL_SCRAMBLER_COOLDOWN),
            ..on(
                Trigger::FlankKill,
                Action::Unseen {
                    seconds: data::SIGNAL_SCRAMBLER_SECONDS,
                },
            )
        }),
    ),
    row(
        Relic::FieldRadio,
        1,
        true,
        Effect::Aura {
            stat: Stat::Accuracy,
            percent: data::FIELD_RADIO_ACCURACY_PERCENT,
            tiles: data::FIELD_RADIO_TILES,
            own: false,
        },
    ),
    rule(
        Relic::Spotter,
        1,
        true,
        Rule::Spotter {
            damage_percent: data::SPOTTER_DAMAGE_PERCENT,
            seconds: data::SPOTTER_SECONDS,
        },
    ),
    row(
        Relic::SquadMorale,
        2,
        true,
        Effect::On(on(
            Trigger::CrewKill,
            Action::CooldownsLess {
                seconds: data::SQUAD_MORALE_SECONDS,
            },
        )),
    ),
    rule(
        Relic::HazardPay,
        1,
        true,
        Rule::SitePay {
            money: data::HAZARD_PAY,
        },
    ),
    stat(
        Relic::TradeLicense,
        1,
        true,
        Stat::TraderPrices,
        -data::TRADE_LICENSE_PERCENT,
    ),
    rule(Relic::RestockCodes, 2, true, Rule::Restock),
    when(
        Relic::PartsBroker,
        2,
        true,
        Stat::Bounty,
        data::PARTS_BROKER_BOUNTY_PERCENT,
        When::Crippled,
    ),
    row(
        Relic::TetherField,
        2,
        true,
        Effect::On(on(
            Trigger::Revived,
            Action::Tether {
                percent: data::TETHER_FIELD_PERCENT,
                seconds: data::TETHER_FIELD_SECONDS,
            },
        )),
    ),
    rule(
        Relic::WideAngleOptics,
        2,
        true,
        Rule::StillRange {
            tiles: data::WIDE_ANGLE_OPTICS_TILES,
        },
    ),
    row(
        Relic::CoverFormation,
        2,
        true,
        Effect::Aura {
            stat: Stat::DamageTaken,
            percent: -data::COVER_FORMATION_PERCENT,
            tiles: data::COVER_FORMATION_TILES,
            own: true,
        },
    ),
    rule(
        Relic::StrongWill,
        2,
        true,
        Rule::LongerAbilities {
            percent: data::STRONG_WILL_PERCENT,
        },
    ),
    rule(
        Relic::TotalTeardown,
        3,
        true,
        Rule::TearIntoChassis {
            damage_percent: data::TOTAL_TEARDOWN_DAMAGE_PERCENT,
        },
    ),
    row(
        Relic::Lifeline,
        3,
        true,
        Effect::On(once(
            Trigger::PlayerLow,
            Action::Shield {
                tiles: data::LIFELINE_TILES,
                hp: data::LIFELINE_SHIELD_HP,
                seconds: data::LIFELINE_SECONDS,
            },
        )),
    ),
    rule(
        Relic::Crossfire,
        3,
        true,
        Rule::Crossfire {
            damage_percent: data::CROSSFIRE_DAMAGE_PERCENT,
            apart_cos: data::CROSSFIRE_APART_COS,
            tiles: data::CROSSFIRE_TILES,
        },
    ),
    row(
        Relic::RallyPoint,
        3,
        true,
        Effect::On(once(
            Trigger::AbilityUse,
            Action::RallyUp {
                tiles: data::RALLY_POINT_TILES,
                health_percent: data::RALLY_POINT_HEALTH_PERCENT,
            },
        )),
    ),
    rule(
        Relic::WarChest,
        3,
        true,
        Rule::WarChest {
            percent_per_thousand: data::WAR_CHEST_PERCENT_PER_THOUSAND,
            cap: data::WAR_CHEST_CAP_PERCENT,
        },
    ),
];

/// *Sprint Coil*'s two hooks: a mission's start and every ability used.
const SPRINT_COIL: [Hook; 2] = [
    on(
        Trigger::MissionStart,
        Action::Sprint {
            percent: data::SPRINT_COIL_SPEED_PERCENT,
            seconds: data::SPRINT_COIL_SECONDS,
        },
    ),
    on(
        Trigger::AbilityUse,
        Action::Sprint {
            percent: data::SPRINT_COIL_SPEED_PERCENT,
            seconds: data::SPRINT_COIL_SECONDS,
        },
    ),
];

/// The percentage a Bim holding `held` has on `stat`, every relic's
/// modifier summed — nought with none — in the `situation` each
/// modifier's [`When`] is asked against.
pub fn stat_percent(held: &[Relic], stat: Stat, situation: Situation) -> i32 {
    held.iter()
        .map(|r| match r.effect() {
            Effect::Stat {
                stat: s,
                percent,
                when,
            } if s == stat => {
                let holds = match when {
                    When::Always => true,
                    When::OtherPlayerDown => situation.other_down,
                    When::OnLimb => situation.on_limb,
                    When::Crippled => situation.crippled,
                    When::Flanked => situation.flanked,
                };
                if holds { percent } else { 0 }
            }
            _ => 0,
        })
        .sum()
}

/// The rule a Bim holding `held` has that `pick` answers for, if any —
/// `rule_of(held, |r| match r { Rule::Restock => Some(()), _ => None })`.
pub fn rule_of<T>(held: &[Relic], pick: impl Fn(Rule) -> Option<T>) -> Option<T> {
    held.iter().find_map(|r| match r.effect() {
        Effect::Rule(rule) => pick(rule),
        _ => None,
    })
}

/// The auras a Bim holding `held` casts on `stat`: the percentage, the
/// tiles and whether bots alone are lifted.
pub fn auras(held: &[Relic], stat: Stat) -> impl Iterator<Item = (i32, f32, bool)> + '_ {
    held.iter().filter_map(move |r| match r.effect() {
        Effect::Aura {
            stat: s,
            percent,
            tiles,
            own,
        } if s == stat => Some((percent, tiles, own)),
        _ => None,
    })
}

/// A percentage as a factor: ten is 1.1, minus ten 0.9 — never under
/// nought.
pub fn factor(percent: i32) -> f64 {
    (1.0 + f64::from(percent) / 100.0).max(0.0)
}

/// Every hook a Bim holding `held` has on `trigger`, with the relic.
pub fn hooks(held: &[Relic], trigger: Trigger) -> impl Iterator<Item = (Relic, Hook)> + '_ {
    held.iter().flat_map(move |&r| {
        let list: &'static [Hook] = match &r.def().effect {
            Effect::On(hook) => core::slice::from_ref(hook),
            Effect::OnEach(list) => list,
            _ => &[],
        };
        list.iter()
            .filter(move |h| h.trigger == trigger)
            .map(move |&h| (r, h))
    })
}

/// The overcharge a Bim holding `held` shoots with: every how many shots,
/// and what that shot's damage is multiplied by — `(0, 1.0)` with none.
pub fn overcharge(held: &[Relic]) -> (u32, f32) {
    held.iter()
        .find_map(|r| match r.effect() {
            Effect::EveryNthShot {
                every,
                damage_percent,
            } => Some((every, factor(damage_percent) as f32)),
            _ => None,
        })
        .unwrap_or((0, 1.0))
}

/// What a new profile's pool is: every relic marked for it, in list order.
pub fn starting_pool() -> Vec<Relic> {
    Relic::ALL
        .into_iter()
        .filter(|r| r.starts_unlocked())
        .collect()
}

/// A set of relics as bits, one a code — how a pool crosses the lobby.
pub fn mask_of(relics: &[Relic]) -> u64 {
    relics.iter().fold(0, |m, r| m | 1 << r.code())
}

/// The relics of a mask, in list order.
pub fn relics_of_mask(mask: u64) -> Vec<Relic> {
    Relic::ALL
        .into_iter()
        .filter(|r| mask & (1 << r.code()) != 0)
        .collect()
}

/// The odds of each tier, one to three, on `day` of the world clock, as
/// weights in per cent: [`data::RELIC_ODDS_START`] on day nought, moving
/// in a straight line to [`data::RELIC_ODDS_END`] on
/// [`data::RELIC_ODDS_FULL_DAY`], and staying there. Integers, rounded
/// down, so a day's three can add up to a little under a hundred; a roll
/// is taken against what they add up to.
pub fn tier_odds(day: u32) -> [u32; 3] {
    let full = data::RELIC_ODDS_FULL_DAY;
    if full == 0 || day >= full {
        return data::RELIC_ODDS_END;
    }
    let (a, b) = (data::RELIC_ODDS_START, data::RELIC_ODDS_END);
    [0, 1, 2].map(|t| (a[t] * (full - day) + b[t] * day) / full)
}

/// A tier, one to three, rolled off `roll` by `weights` (one a tier);
/// `None` when they add up to nought.
fn roll_tier(weights: [u32; 3], roll: u64) -> Option<u8> {
    let sum: u64 = weights.iter().map(|&w| u64::from(w)).sum();
    if sum == 0 {
        return None;
    }
    let mut at = roll % sum;
    for (t, &w) in weights.iter().enumerate() {
        if at < u64::from(w) {
            return Some(t as u8 + 1);
        }
        at -= u64::from(w);
    }
    None
}

/// `n` relics drawn from `pool` on `day` of the world clock, each draw off
/// `seed` (task 117). A draw rolls a tier by [`tier_odds`], and takes a
/// relic of that tier out of what is left; when that tier has none left
/// it rolls again, by the same odds, among the tiers that still have some;
/// and it draws nothing once nothing is left. None twice, and in the order
/// drawn. The pool itself is the caller's and is not touched.
pub fn offer(pool: &[Relic], day: u32, n: usize, seed: u64) -> Vec<Relic> {
    let odds = tier_odds(day);
    let mut left: Vec<Relic> = pool.to_vec();
    let mut drawn = Vec::new();
    for i in 0..n {
        if left.is_empty() {
            break;
        }
        let draw = seed ^ (i as u64).wrapping_mul(0x_9E37_79B9_7F4A_7C15);
        let roll = |salt: u64| worldgen::rng::mix(draw ^ salt);
        let has = |t: u8| left.iter().any(|r| r.tier() == t);
        let mut tier = roll_tier(odds, roll(0x_5449_4552)).filter(|&t| has(t));
        if tier.is_none() {
            let mut open = odds;
            for (t, w) in open.iter_mut().enumerate() {
                if !has(t as u8 + 1) {
                    *w = 0;
                }
            }
            tier = roll_tier(open, roll(0x_4147_4149_4E));
        }
        // Odds of nought on every tier that has any left: whatever is
        // left, lowest tier first. Never met with the odds as written.
        let tier = tier.unwrap_or_else(|| left.iter().map(|r| r.tier()).min().unwrap_or(1));
        let of_tier: Vec<Relic> = left.iter().copied().filter(|r| r.tier() == tier).collect();
        let pick = of_tier[(roll(0x_5049_434B) % of_tier.len() as u64) as usize];
        left.retain(|&r| r != pick);
        drawn.push(pick);
    }
    drawn
}

/// A number off the galaxy's seed for one site and one purpose: the same
/// on every machine, and drawn from no stream a fight draws from, so a
/// roll here moves nothing else.
fn site_roll(galaxy_seed: u64, star: u32, station: u32, salt: u64) -> u32 {
    let seed = worldgen::rng::mix(galaxy_seed ^ salt)
        ^ worldgen::rng::mix(u64::from(star) << 32 | u64::from(station));
    worldgen::rng::Rng::new(seed).below(100)
}

/// Whether a site the machines take hides a relic cache, at
/// [`data::RELIC_CACHE_CHANCE`] in a hundred: rolled once a site, off the
/// galaxy's seed.
pub fn cache_rolled(galaxy_seed: u64, star: u32, station: u32) -> bool {
    site_roll(galaxy_seed, star, station, 0x_5245_4C49_4343) < data::RELIC_CACHE_CHANCE
}

/// Whether a site's machines come at tier two, `hops` from the crew's own
/// star, once the world clock is past [`data::ENEMY_TIER2_HOURS`]: odds
/// of `hops` in [`data::ENEMY_TIER2_SURE_HOPS`], and always from there on.
/// Rolled once a site, off the galaxy's seed, so the map's quote and the
/// wave on arrival agree.
pub fn tier_two_rolled(galaxy_seed: u64, star: u32, station: u32, hops: u16) -> bool {
    let sure = data::ENEMY_TIER2_SURE_HOPS.max(1);
    if hops >= sure {
        return true;
    }
    let odds = u32::from(hops) * 100 / u32::from(sure);
    site_roll(galaxy_seed, star, station, 0x_5449_4552_3254) < odds
}

/// Where a relic choice came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Source {
    /// A site cleared with machines in it: chosen on the reward screen,
    /// after the departure check and before the map.
    Reward,
    /// A cache opened on a held site's deck: chosen in the mission with
    /// the game running, and pending until the site is cleared.
    Cache,
}

impl Source {
    pub fn code(self) -> u32 {
        match self {
            Source::Reward => 0,
            Source::Cache => 1,
        }
    }
}

/// A relic, or none, put to the crew for one player's Bim, and who has
/// said yes: the proposer counts as having, and a new proposal starts the
/// count again.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RelicProposal {
    /// The relic, or `None` for taking none.
    pub relic: Option<Relic>,
    /// The player slot whose Bim would have it.
    pub to: u32,
    /// The player slot that put it.
    pub by: u32,
    /// One a player slot.
    pub accepted: Vec<bool>,
}

impl RelicProposal {
    /// Whether every player still at the keyboard has said yes; a slot
    /// past `connected` counts as connected.
    pub fn carried(&self, connected: &[bool]) -> bool {
        self.accepted
            .iter()
            .enumerate()
            .all(|(slot, &yes)| yes || !connected.get(slot).copied().unwrap_or(true))
    }
}

/// The relics on offer and the proposal on the table — or, on the reward
/// screen, every player's own pick (task 146).
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RelicChoice {
    pub source: Source,
    pub options: Vec<Relic>,
    /// A cache's vote; the reward has none.
    pub proposal: Option<RelicProposal>,
    /// The reward's picks (task 146): one a player slot, the relic that
    /// player has picked this round for its own Bim, `None` until it has.
    /// Cleared when a round is settled.
    #[cfg_attr(feature = "serde", serde(default))]
    pub picks: Vec<Option<Relic>>,
    /// One a player slot: the relic it has won off the reward, once it
    /// has — given at once, and out of what the next round picks from.
    #[cfg_attr(feature = "serde", serde(default))]
    pub won: Vec<Option<Relic>>,
    /// How many rounds of picks have been settled: what the dice of the
    /// next are seeded with, beside the offer.
    #[cfg_attr(feature = "serde", serde(default))]
    pub round: u32,
}

impl RelicChoice {
    /// A choice of `options` off `source`, nobody's pick in yet.
    pub fn new(source: Source, options: Vec<Relic>) -> RelicChoice {
        RelicChoice {
            source,
            options,
            proposal: None,
            picks: Vec::new(),
            won: Vec::new(),
            round: 0,
        }
    }

    /// What is still to be won off the reward: the options nobody has
    /// won, in the offer's order.
    pub fn left(&self) -> Vec<Relic> {
        self.options
            .iter()
            .copied()
            .filter(|r| !self.won.contains(&Some(*r)))
            .collect()
    }

    /// The relic player `slot` has picked this round, if it has.
    pub fn pick_of(&self, slot: u32) -> Option<Relic> {
        self.picks.get(slot as usize).copied().flatten()
    }

    /// The relic player `slot` has won off the reward, if it has.
    pub fn won_by(&self, slot: u32) -> Option<Relic> {
        self.won.get(slot as usize).copied().flatten()
    }
}

/// Two dice, thrown by player `slot` for the relic it and another picked
/// (task 146): `a` and `b` from one to six each, the higher sum winning.
/// [`dice`] throws them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Throw {
    pub slot: u32,
    pub relic: Relic,
    pub a: u32,
    pub b: u32,
}

/// Who wins a relic more than one player picked (task 146): each of
/// `contenders` throws two dice in turn, in the order given; the highest
/// sum wins, and those tied for it throw again, after the rest, until one
/// stands alone. The winner and every throw in the order thrown. Off a
/// stream of its own — the galaxy, the offer and the round — so every
/// client throws the same.
pub fn dice(contenders: &[u32], relic: Relic, seed: u64) -> (u32, Vec<Throw>) {
    let mut rng = worldgen::rng::Rng::new(seed);
    let mut throws = Vec::new();
    let mut left = contenders.to_vec();
    loop {
        let mut best = 0;
        let mut top = Vec::new();
        for &slot in &left {
            let a = rng.below(6) + 1;
            let b = rng.below(6) + 1;
            throws.push(Throw { slot, relic, a, b });
            if a + b > best {
                best = a + b;
                top.clear();
            }
            if a + b == best {
                top.push(slot);
            }
        }
        if top.len() <= 1 {
            return (top.first().copied().unwrap_or(u32::MAX), throws);
        }
        left = top;
    }
}

/// Everything the run keeps about relics. Saved and in `world_checksum`
/// whole, as part of [`crate::Run`].
#[derive(Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Relics {
    /// What may still be drawn, in list order: the host's unlocked relics
    /// at the start, one less every relic a Bim **gets** — given, pending
    /// off a cache, or bought — and one back every pending relic lost
    /// (task 117). An offer takes nothing out of it: what is on offer is
    /// kept out of other draws by [`Relics::in_play`] while it is.
    pub pool: Vec<Relic>,
    /// One a player slot: the relics that player's Bim holds, in the order
    /// it was given them.
    pub held: Vec<Vec<Relic>>,
    /// Relics given out of a cache on a site not cleared yet, with the
    /// player slot each is for: kept the step the site is cleared, lost if
    /// the crew leave before.
    pub pending: Vec<(u32, Relic)>,
    /// The choice being made, if one is.
    pub choice: Option<RelicChoice>,
    /// One a player slot: the once-a-mission relics that have fired this
    /// mission.
    pub fired: Vec<Vec<Relic>>,
    /// One a player slot: the mission minute its Bim went down, while it
    /// is down and a relic is waiting to get it up.
    pub down_since: Vec<Option<f64>>,
    /// How many offers have been drawn this run: what the next is seeded
    /// with, beside the site.
    pub offers: u32,
    /// The timed effects of task 118's relics running, each on the crew
    /// member it is on until a mission minute: *Sprint Coil*'s pace,
    /// *Tether Field*'s shelter, *Signal Scrambler*'s cloak. A mission's
    /// start clears them.
    #[cfg_attr(feature = "serde", serde(default))]
    pub buffs: Vec<Buff>,
    /// When a relic with a cooldown may fire again: the player slot, the
    /// relic and the mission minute.
    #[cfg_attr(feature = "serde", serde(default))]
    pub ready_at: Vec<(u32, Relic, f64)>,
    /// The machine each *Spotter* marked: the player slot, the machine's
    /// body index in the residents' room, and the mission minute the mark
    /// lasts until.
    #[cfg_attr(feature = "serde", serde(default))]
    pub spotted: Vec<(u32, u32, f64)>,
    /// The machines each *Marksman's Habit* has already put a hit on: the
    /// player slot and the body index.
    #[cfg_attr(feature = "serde", serde(default))]
    pub limb_aimed: Vec<(u32, u32)>,
    /// The machines, by body index, whose last hit was a player's from the
    /// side or behind — what a kill is told to *Signal Scrambler* with.
    #[cfg_attr(feature = "serde", serde(default))]
    pub flanked: Vec<u32>,
    /// One a player slot: the mission minute its Bim last went down, kept
    /// after it is revived — what *Clot Booster* counts from.
    #[cfg_attr(feature = "serde", serde(default))]
    pub downed_at: Vec<Option<f64>>,
    /// Whether the trader the crew are at has been restocked this visit
    /// (*Restock Codes*); an arrival at a trader clears it.
    #[cfg_attr(feature = "serde", serde(default))]
    pub restocked: bool,
}

/// A relic's timed effect on one crew member (task 118).
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Buff {
    /// The crew member it is on, by index.
    pub who: u32,
    /// The relic that put it there, which says what it does.
    pub relic: Relic,
    /// The mission minute it runs out at.
    pub until: f64,
}

impl Relics {
    /// A run's relics: the pool it opens with, `players` Bims holding none.
    pub fn new(pool: Vec<Relic>, players: u32) -> Relics {
        let players = players as usize;
        Relics {
            pool,
            held: vec![Vec::new(); players],
            pending: Vec::new(),
            choice: None,
            fired: vec![Vec::new(); players],
            down_since: vec![None; players],
            offers: 0,
            buffs: Vec::new(),
            ready_at: Vec::new(),
            spotted: Vec::new(),
            limb_aimed: Vec::new(),
            flanked: Vec::new(),
            downed_at: vec![None; players],
            restocked: false,
        }
    }

    /// What a player's Bim holds.
    pub fn of(&self, slot: u32) -> &[Relic] {
        self.held.get(slot as usize).map_or(&[], |h| h.as_slice())
    }

    /// Whether a relic is in the run at all — held, pending or on offer
    /// now — which is what keeps it out of every draw while it is. A
    /// trader's relic on its table is the world's to add
    /// (`World::draw_relics`), since the traders are kept beside this.
    pub fn in_play(&self, relic: Relic) -> bool {
        self.held.iter().flatten().any(|&r| r == relic)
            || self.pending.iter().any(|&(_, r)| r == relic)
            || self
                .choice
                .as_ref()
                .is_some_and(|c| c.options.contains(&relic))
    }

    /// A relic a Bim got — given, pending or bought — out of the pool.
    pub(crate) fn take_from_pool(&mut self, relic: Relic) {
        self.pool.retain(|&r| r != relic);
    }

    /// A relic back in the pool, in list order — a pending one lost — unless
    /// a Bim holds it or it is pending still.
    pub(crate) fn return_to_pool(&mut self, relic: Relic) {
        let kept = self.held.iter().flatten().any(|&r| r == relic)
            || self.pending.iter().any(|&(_, r)| r == relic);
        if kept {
            return;
        }
        if let Err(at) = self.pool.binary_search(&relic) {
            self.pool.insert(at, relic);
        }
    }

    /// Give a relic to a player's Bim for good.
    pub fn give(&mut self, slot: u32, relic: Relic) {
        let at = slot as usize;
        if self.held.len() <= at {
            self.held.resize(at + 1, Vec::new());
        }
        if !self.held[at].contains(&relic) {
            self.held[at].push(relic);
        }
    }

    /// Whether a once-a-mission relic has fired this mission.
    pub fn has_fired(&self, slot: u32, relic: Relic) -> bool {
        self.fired
            .get(slot as usize)
            .is_some_and(|f| f.contains(&relic))
    }

    pub(crate) fn mark_fired(&mut self, slot: u32, relic: Relic) {
        let at = slot as usize;
        if self.fired.len() <= at {
            self.fired.resize(at + 1, Vec::new());
        }
        if !self.fired[at].contains(&relic) {
            self.fired[at].push(relic);
        }
    }

    /// A mission begins: every once-a-mission relic ready again and
    /// nobody waiting to get up.
    pub(crate) fn new_mission(&mut self, players: u32) {
        let players = players as usize;
        self.fired = vec![Vec::new(); players];
        self.down_since = vec![None; players];
        if self.held.len() < players {
            self.held.resize(players, Vec::new());
        }
        self.buffs.clear();
        self.ready_at.clear();
        self.spotted.clear();
        self.limb_aimed.clear();
        self.flanked.clear();
        self.downed_at = vec![None; players];
    }
}

/// What a player has unlocked between runs, and how many runs they have
/// won — the app keeps it as `bims/profile.ron` beside the saves. A code a
/// relic or class, so a profile written by a build with more of either
/// still reads.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Profile {
    /// The relics unlocked, by code, in list order.
    pub relics: Vec<u32>,
    /// The classes unlocked, by code.
    pub classes: Vec<u32>,
    /// Runs won.
    pub wins: u32,
}

impl Default for Profile {
    fn default() -> Profile {
        Profile::new()
    }
}

impl Profile {
    /// A new profile: the starting pool and every class not marked
    /// unlockable.
    pub fn new() -> Profile {
        Profile {
            relics: starting_pool().into_iter().map(Relic::code).collect(),
            classes: Class::ALL
                .into_iter()
                .filter(|c| !c.unlockable())
                .map(Class::code)
                .collect(),
            wins: 0,
        }
    }

    /// The relics unlocked, in list order: a run's pool, when this is the
    /// host's. Every relic a new profile starts with is in it whatever the
    /// file says, so a profile written before a starting relic was added
    /// has it too (task 117).
    pub fn pool(&self) -> Vec<Relic> {
        Relic::ALL
            .into_iter()
            .filter(|&r| self.unlocked(r))
            .collect()
    }

    /// Whether a relic is in this profile's pool.
    pub fn unlocked(&self, relic: Relic) -> bool {
        relic.starts_unlocked() || self.relics.contains(&relic.code())
    }

    /// Whether a class may be picked.
    pub fn class_unlocked(&self, class: Class) -> bool {
        !class.unlockable() || self.classes.contains(&class.code())
    }

    /// A run over: won, [`data::RELICS_UNLOCKED_PER_WIN`] relics unlocked —
    /// the first still locked, in list order — and the win counted; lost,
    /// nothing. The relics unlocked now.
    pub fn record_run(&mut self, won: bool) -> Vec<Relic> {
        if !won {
            return Vec::new();
        }
        self.wins = self.wins.saturating_add(1);
        let fresh: Vec<Relic> = Relic::ALL
            .into_iter()
            .filter(|&r| !self.unlocked(r))
            .take(data::RELICS_UNLOCKED_PER_WIN)
            .collect();
        for r in &fresh {
            self.relics.push(r.code());
        }
        self.relics.sort_unstable();
        fresh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The dice** (task 146): two each, one to six, thrown in the order
    /// given; the winner's the highest sum of the last round, and a tie
    /// at the top throws again — only those tied, after the rest. The
    /// same seed, the same throws; over many seeds every contender wins
    /// about as often.
    #[test]
    fn the_dice_go_in_turn_and_a_tie_at_the_top_throws_again() {
        let mut wins = [0u32; 3];
        let mut rethrown = 0;
        for seed in 0..3_000u64 {
            let (winner, throws) = dice(&[2, 0, 1], Relic::KillRelay, seed);
            assert_eq!(
                (winner, throws.clone()),
                dice(&[2, 0, 1], Relic::KillRelay, seed)
            );
            let order: Vec<u32> = throws.iter().take(3).map(|t| t.slot).collect();
            assert_eq!(order, vec![2, 0, 1], "in the order given");
            assert!(
                throws
                    .iter()
                    .all(|t| (1..=6).contains(&t.a) && (1..=6).contains(&t.b))
            );
            assert!(throws.iter().all(|t| t.relic == Relic::KillRelay));
            // Round by round: each round is the last one's top.
            let mut at = 0;
            let mut left = vec![2u32, 0, 1];
            loop {
                let round = &throws[at..at + left.len()];
                let best = round.iter().map(|t| t.a + t.b).max().unwrap();
                let top: Vec<u32> = round
                    .iter()
                    .filter(|t| t.a + t.b == best)
                    .map(|t| t.slot)
                    .collect();
                at += left.len();
                if top.len() == 1 {
                    assert_eq!(top[0], winner);
                    break;
                }
                left = top;
                rethrown += 1;
            }
            assert_eq!(at, throws.len(), "no throw after the winner's round");
            wins[winner as usize] += 1;
        }
        assert!(rethrown > 0, "ties happen");
        for w in wins {
            assert!((900..=1_100).contains(&w), "{wins:?}");
        }
    }

    #[test]
    fn the_list_is_in_code_order_and_every_tier_is_one_to_three() {
        for (i, def) in RELICS.iter().enumerate() {
            assert_eq!(def.relic.code() as usize, i);
            assert!((1..=3).contains(&def.tier));
            assert_eq!(Relic::from_code(i as u32), Some(def.relic));
        }
        assert_eq!(Relic::from_code(RELICS.len() as u32), None);
    }

    #[test]
    fn every_relic_starts_unlocked_and_a_win_unlocks_nothing() {
        // The player asked for every relic in the pool from the start (it
        // was 23, task 117, with fourteen unlocked by wins).
        assert_eq!(starting_pool(), Relic::ALL.to_vec());
        assert!(Relic::ALL.iter().all(|r| r.starts_unlocked()));
        let mut profile = Profile::new();
        assert_eq!(profile.pool(), Relic::ALL.to_vec());
        assert!(profile.record_run(true).is_empty());
        assert_eq!(profile.wins, 1);
        assert_eq!(profile.pool(), Relic::ALL.to_vec());
    }

    #[test]
    fn the_tier_odds_move_in_a_line_to_the_full_day_and_stay() {
        assert_eq!(tier_odds(0), data::RELIC_ODDS_START);
        assert_eq!(tier_odds(data::RELIC_ODDS_FULL_DAY), data::RELIC_ODDS_END);
        assert_eq!(tier_odds(1_000), data::RELIC_ODDS_END);
        assert_eq!(tier_odds(15), [55, 30, 15]);
        let mut last = tier_odds(0);
        for day in 1..=data::RELIC_ODDS_FULL_DAY {
            let odds = tier_odds(day);
            let sum: u32 = odds.iter().sum();
            assert!((97..=100).contains(&sum), "day {day}: {odds:?}");
            // Tier one only ever falls, tier three only ever rises.
            assert!(odds[0] <= last[0] && odds[2] >= last[2], "day {day}");
            last = odds;
        }
    }

    #[test]
    fn an_offer_never_draws_one_twice_and_never_shrinks_the_pool() {
        let pool = Relic::ALL.to_vec();
        for seed in 0..500 {
            for day in [0, 12, 30, 90] {
                let three = offer(&pool, day, 3, seed);
                assert_eq!(three.len(), 3);
                let mut sorted = three.clone();
                sorted.sort();
                sorted.dedup();
                assert_eq!(sorted.len(), 3, "none twice: {three:?}");
                assert!(three.iter().all(|r| pool.contains(r)));
            }
        }
        assert_eq!(pool, Relic::ALL.to_vec(), "the pool is the caller's");
        // The same seed and day, the same draw.
        assert_eq!(offer(&pool, 7, 3, 42), offer(&pool, 7, 3, 42));
        // Short of three: as many as there are, and nothing from nothing.
        assert_eq!(
            offer(&[Relic::FocusingLens], 0, 3, 1),
            vec![Relic::FocusingLens]
        );
        assert!(offer(&[], 0, 3, 1).is_empty());
    }

    #[test]
    fn an_empty_tier_is_rolled_again_among_the_tiers_left() {
        // Tier three alone: every draw is tier three, whatever the day's
        // odds say — a draw is never lost to an empty tier.
        let high: Vec<Relic> = Relic::ALL.into_iter().filter(|r| r.tier() == 3).collect();
        for seed in 0..200 {
            let drawn = offer(&high, 0, 3, seed);
            assert_eq!(drawn.len(), 3.min(high.len()));
            assert!(drawn.iter().all(|r| r.tier() == 3));
        }
        // No tier one: tier two and three come in the odds' own proportion
        // between them — 25 to 5 on day nought.
        let upper: Vec<Relic> = Relic::ALL.into_iter().filter(|r| r.tier() > 1).collect();
        let n = 6_000;
        let two = (0..n)
            .filter(|&seed| offer(&upper, 0, 1, seed)[0].tier() == 2)
            .count();
        let share = two * 100 / n as usize;
        assert!((80..=87).contains(&share), "tier two {share}% of draws");
    }

    #[test]
    fn the_tier_shares_follow_the_day_s_odds_over_many_seeds() {
        let pool = Relic::ALL.to_vec();
        let n = 10_000u64;
        for day in [0, 15, 30] {
            let odds = tier_odds(day);
            let sum: u32 = odds.iter().sum();
            let mut seen = [0u64; 3];
            for seed in 0..n {
                let first = offer(&pool, day, 1, seed * 7919 + u64::from(day));
                seen[(first[0].tier() - 1) as usize] += 1;
            }
            for t in 0..3 {
                let want = f64::from(odds[t]) / f64::from(sum);
                let got = seen[t] as f64 / n as f64;
                assert!(
                    (got - want).abs() < 0.02,
                    "day {day} tier {}: {got:.3} against {want:.3}",
                    t + 1
                );
            }
        }
    }

    #[test]
    fn a_relic_leaves_the_pool_when_got_and_comes_back_when_lost() {
        let mut relics = Relics::new(Relic::ALL.to_vec(), 2);
        relics.take_from_pool(Relic::KillRelay);
        relics.give(0, Relic::KillRelay);
        assert!(!relics.pool.contains(&Relic::KillRelay));
        // Held: never back.
        relics.return_to_pool(Relic::KillRelay);
        assert!(!relics.pool.contains(&Relic::KillRelay));
        // Pending, then lost: back, in list order.
        relics.take_from_pool(Relic::ServoBraces);
        relics.pending.push((1, Relic::ServoBraces));
        relics.return_to_pool(Relic::ServoBraces);
        assert!(!relics.pool.contains(&Relic::ServoBraces), "still pending");
        relics.pending.clear();
        relics.return_to_pool(Relic::ServoBraces);
        let mut sorted = relics.pool.clone();
        sorted.sort();
        assert_eq!(relics.pool, sorted);
        assert!(relics.pool.contains(&Relic::ServoBraces));
        relics.return_to_pool(Relic::ServoBraces);
        assert_eq!(
            relics
                .pool
                .iter()
                .filter(|&&r| r == Relic::ServoBraces)
                .count(),
            1,
            "never twice"
        );
    }

    #[test]
    fn a_profile_written_before_a_starting_relic_was_added_has_it() {
        let old = Profile {
            relics: vec![Relic::FocusingLens.code()],
            classes: Vec::new(),
            wins: 0,
        };
        assert_eq!(old.pool(), starting_pool());
        // And a win unlocks the first locked, not a starting one.
        let mut old = old;
        let fresh = old.record_run(true);
        assert!(fresh.iter().all(|r| !r.starts_unlocked()), "{fresh:?}");
    }

    #[test]
    fn a_loss_unlocks_nothing_and_counts_no_win() {
        let mut profile = Profile::new();
        assert!(profile.record_run(false).is_empty());
        assert_eq!(profile.wins, 0);
        assert_eq!(profile.pool(), starting_pool());
    }

    #[test]
    fn every_class_is_unlocked_in_a_new_profile() {
        let profile = Profile::new();
        for class in Class::ALL {
            assert!(profile.class_unlocked(class));
        }
    }

    #[test]
    fn a_pool_crosses_as_a_mask_and_back() {
        let pool = starting_pool();
        assert_eq!(relics_of_mask(mask_of(&pool)), pool);
        assert_eq!(relics_of_mask(mask_of(&Relic::ALL)), Relic::ALL.to_vec());
    }

    #[test]
    fn stat_modifiers_sum_and_last_stand_waits_on_a_downed_friend() {
        let held = [Relic::FocusingLens, Relic::LastStand];
        let calm = Situation::default();
        let down = Situation {
            other_down: true,
            ..calm
        };
        assert_eq!(
            stat_percent(&held, Stat::Damage, calm),
            data::FOCUSING_LENS_DAMAGE_PERCENT
        );
        assert_eq!(
            stat_percent(&held, Stat::Damage, down),
            data::FOCUSING_LENS_DAMAGE_PERCENT + data::LAST_STAND_DAMAGE_PERCENT
        );
        assert_eq!(stat_percent(&held, Stat::Armour, down), 0);
        assert!(stat_percent(&[Relic::CoolantLoop], Stat::Cooldowns, calm) < 0);
    }

    #[test]
    fn a_hit_on_a_machine_reads_the_limb_the_cripple_and_the_flank() {
        let held = [
            Relic::ServoCutter,
            Relic::CripplersMark,
            Relic::BlindSpot,
            Relic::FocusingLens,
        ];
        let plain = Situation::default();
        // The weapon's own damage is never the machine's: Focusing Lens is
        // in the skill already.
        assert_eq!(stat_percent(&held, Stat::MachineDamage, plain), 0);
        let each = [
            (
                Situation {
                    on_limb: true,
                    ..plain
                },
                data::SERVO_CUTTER_DAMAGE_PERCENT,
            ),
            (
                Situation {
                    crippled: true,
                    ..plain
                },
                data::CRIPPLERS_MARK_DAMAGE_PERCENT,
            ),
            (
                Situation {
                    flanked: true,
                    ..plain
                },
                data::BLIND_SPOT_DAMAGE_PERCENT,
            ),
        ];
        for (situation, percent) in each {
            assert_eq!(stat_percent(&held, Stat::MachineDamage, situation), percent);
        }
        let all = Situation {
            other_down: false,
            on_limb: true,
            crippled: true,
            flanked: true,
        };
        assert_eq!(
            stat_percent(&held, Stat::MachineDamage, all),
            data::SERVO_CUTTER_DAMAGE_PERCENT
                + data::CRIPPLERS_MARK_DAMAGE_PERCENT
                + data::BLIND_SPOT_DAMAGE_PERCENT
        );
        // Parts Broker is the bounty's, on a crippled kill alone.
        let broker = [Relic::PartsBroker, Relic::SalvageBeacon];
        assert_eq!(
            stat_percent(&broker, Stat::Bounty, plain),
            data::SALVAGE_BEACON_BOUNTY_PERCENT
        );
        assert_eq!(
            stat_percent(
                &broker,
                Stat::Bounty,
                Situation {
                    crippled: true,
                    ..plain
                }
            ),
            data::SALVAGE_BEACON_BOUNTY_PERCENT + data::PARTS_BROKER_BOUNTY_PERCENT
        );
    }

    #[test]
    fn task_118_s_hooks_rules_and_auras_are_where_the_world_looks_for_them() {
        // Kill Relay is three seconds now.
        assert_eq!(data::KILL_RELAY_SECONDS, 3.0);
        // Sprint Coil has two hooks, one on each trigger.
        let coil = [Relic::SprintCoil];
        assert_eq!(hooks(&coil, Trigger::MissionStart).count(), 1);
        assert_eq!(hooks(&coil, Trigger::AbilityUse).count(), 1);
        assert_eq!(hooks(&coil, Trigger::Kill).count(), 0);
        // Squad Morale and Kill Relay stack: four seconds off a kill of
        // the holder's own, one of a bot's.
        let both = [Relic::KillRelay, Relic::SquadMorale];
        let off = |trigger| -> f64 {
            hooks(&both, trigger)
                .map(|(_, h)| match h.action {
                    Action::CooldownsLess { seconds } => seconds,
                    _ => 0.0,
                })
                .sum()
        };
        assert_eq!(off(Trigger::Kill) + off(Trigger::CrewKill), 4.0);
        // The once-a-mission ones, and the scrambler's cooldown.
        let (_, lifeline) = hooks(&[Relic::Lifeline], Trigger::PlayerLow)
            .next()
            .expect("Lifeline waits on a player falling low");
        assert!(lifeline.once_per_mission);
        let (_, rally) = hooks(&[Relic::RallyPoint], Trigger::AbilityUse)
            .next()
            .expect("Rally Point waits on an ability");
        assert!(rally.once_per_mission);
        let (_, scrambler) = hooks(&[Relic::SignalScrambler], Trigger::FlankKill)
            .next()
            .expect("Signal Scrambler waits on a kill from behind");
        assert_eq!(scrambler.cooldown, Some(data::SIGNAL_SCRAMBLER_COOLDOWN));
        // The auras: Field Radio lifts the others, Cover Formation its holder
        // too (task 142).
        assert_eq!(
            auras(&[Relic::FieldRadio], Stat::Accuracy).collect::<Vec<_>>(),
            vec![(
                data::FIELD_RADIO_ACCURACY_PERCENT,
                data::FIELD_RADIO_TILES,
                false
            )]
        );
        assert_eq!(
            auras(&[Relic::CoverFormation], Stat::DamageTaken).collect::<Vec<_>>(),
            vec![(
                -data::COVER_FORMATION_PERCENT,
                data::COVER_FORMATION_TILES,
                true
            )]
        );
        // Every rule is one relic's.
        let rules = Relic::ALL
            .into_iter()
            .filter(|r| matches!(r.effect(), Effect::Rule(_)))
            .count();
        assert_eq!(rules, 12, "Trauma Kit's quicker revive among them");
        assert!(
            rule_of(&[Relic::RestockCodes], |r| (r == Rule::Restock)
                .then_some(()))
            .is_some()
        );
        assert!(rule_of(&[Relic::HazardPay], |r| (r == Rule::Restock).then_some(())).is_none());
        // Trade License is a discount.
        assert!(
            stat_percent(
                &[Relic::TradeLicense],
                Stat::TraderPrices,
                Situation::default()
            ) < 0
        );
    }

    #[test]
    fn the_new_relics_are_five_patches_of_two_ones_two_twos_and_a_three() {
        let new = &Relic::ALL[12..];
        assert_eq!(new.len(), 25);
        for tier in 1..=3u8 {
            let n = new.iter().filter(|r| r.tier() == tier).count();
            assert_eq!(n, [10, 10, 5][tier as usize - 1], "tier {tier}");
        }
        // A new profile has every one of them (codes 27 to 36 were
        // unlocked by wins until every relic started unlocked).
        for r in new {
            assert!(r.starts_unlocked(), "{r:?}");
        }
    }
}
