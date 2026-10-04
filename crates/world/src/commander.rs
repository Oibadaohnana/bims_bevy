//! The commander's state (feature 78, reworked by task 129 into a ranked
//! kit, `crate::class`): what a crew member that is a commander holds
//! between steps, and the reinforcements he brought to the mission.
//! His squad orders — attack, fall back, stand ground — were removed.
//!
//! Two things are kept here and nothing else, because everything else a
//! commander is is read afresh each step off his ranks and where he
//! stands:
//!
//! * one [`Commander`] a crew member, by index, on `World::commanders` —
//!   when his last **Battle Cry** and his last **Rally** began, as
//!   mission minutes, and whom each reached: the one timestamp answers
//!   both how long it has to run and how long until the next, and **the
//!   reach is fixed at the call**, so a Bim that walks out keeps what it
//!   was given and one that walks in is given nothing;
//! * one [`Reinforcement`] a Bim he brought, on `World::reinforcements`:
//!   a crew member marked with the commander who brought it, **for one
//!   mission** — called in with his R (`Command::Reinforce`, on
//!   `Commander::last_reinforcement`'s cooldown), gone from the deck the
//!   moment it dies and off the crew at the mission's end, alive or not.
//!
//! His C, the **Medivac**, calls one medic of the Republic's in, kept as a
//! [`Reinforcement`] marked `medic` on the same list, on
//! `Commander::last_medivac`'s cooldown. (It was a damage aura, the
//! Command Aura, until the medivac replaced it.)
//!
//! **Who each reaches.** The cry and the rally lift *every
//! friendly Bim* in range — a player's own steered Bim, the crew's bots
//! and the reinforcements alike, the commander himself
//! among them, and never a sentry.
//!
//! Both are saved and in `world_checksum` — the cries, the rallies'
//! reach and the reinforcements only where there are any.

/// One crew member's commander state.
#[derive(Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Commander {
    /// The mission minute his last rally began at; `None` until he has
    /// rallied this mission — every mission starts with it ready. The
    /// rally runs for `World::rally_seconds` from it and the next one
    /// waits `World::rally_cooldown` from it, so the one number answers
    /// both. A relic taking seconds off the class cooldowns moves it back.
    pub last_rally: Option<f64>,
    /// Whom the last rally reached, by crew index, lowest first: every
    /// friendly Bim within [`crate::class::RALLY_TILES`] of him when he
    /// called it, himself included.
    #[cfg_attr(feature = "serde", serde(default))]
    pub rallied: Vec<u32>,
    /// The mission minute his last Battle Cry began at (task 129), kept
    /// the way the rally is.
    #[cfg_attr(feature = "serde", serde(default))]
    pub last_battle_cry: Option<f64>,
    /// Whom the last Battle Cry reached, by crew index, lowest first.
    #[cfg_attr(feature = "serde", serde(default))]
    pub cried: Vec<u32>,
    /// The mission minute he last called reinforcements in (his R); `None`
    /// until he has this mission — every mission starts with it ready.
    /// The next call waits `World::reinforcement_cooldown` from it.
    #[cfg_attr(feature = "serde", serde(default))]
    pub last_reinforcement: Option<f64>,
    /// The mission minute he last called a medic in (his C, the
    /// Medivac); `None` until he has this mission. The next call waits
    /// `World::medivac_cooldown` from it.
    #[cfg_attr(feature = "serde", serde(default))]
    pub last_medivac: Option<f64>,
}

impl Commander {
    /// A crew index gone from the crew (`World::drop_crew_member`): out
    /// of both reaches, and every index past it one lower.
    pub fn forget(&mut self, gone: u32) {
        for list in [&mut self.rallied, &mut self.cried] {
            list.retain(|&who| who != gone);
            for who in list.iter_mut() {
                if *who > gone {
                    *who -= 1;
                }
            }
        }
    }
}

/// A Bim a commander brought to the mission (task 129, his
/// Reinforcements, or his Medivac's medic): a crew member marked with
/// the commander who brought it. It fights and is revived like any bot;
/// it earns nothing, costs nothing, drops nothing and keeps no run going.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Reinforcement {
    /// The crew member it is.
    pub who: u32,
    /// The player slot of the commander who brought it.
    pub by: u32,
    /// Whether it is the medic his Medivac (C) called in rather than one
    /// of his R's soldiers: it runs to a player downed and revives him.
    #[cfg_attr(feature = "serde", serde(default))]
    pub medic: bool,
}
