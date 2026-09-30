//! The tuning files, read again while the game runs.
//!
//! - `scaling.ron` (or the file `BIMS_SCALING` names) holds a
//!   [`world::droid::WaveScaling`]: the base machines a wave, how many a
//!   player and a time step add, how many days a time step is, the waves a
//!   held station has at each tier and the first mission's ease.
//! - `rewards.ron` (or `BIMS_REWARDS`) holds a [`world::rewards::Rewards`]:
//!   the experience and the money an enemy down is worth, what a defence
//!   pays of it, whether the money waits for the clear, and what the
//!   buyback, a relic, a combine and the trader's shelf cost.
//!
//! Each file is looked at twice a second; when it changes it is read again
//! and the world is handed the new dials, which the next wave laid, the
//! next enemy down and the next price asked take. A field left out is the
//! constant's; a file that does not parse keeps the last good dials and
//! says why on stderr; no file at all is the constants.
//!
//! The dials are neither saved nor hashed: they are handed over again every
//! frame the world's differ, so a restart or a load takes them too. In a
//! two-player run each game reads its own files, and the two have to agree.
//!
//! The game setup's difficulty (`world::droid::Difficulty`: the base, the
//! per player and the scaling) stands over this file's three for the run
//! it starts — the world keeps it and saves it, and the host deals it to
//! every guest — so those three in the file move a run only when the
//! setup left them as the file had them.

use bevy::prelude::*;
use std::path::PathBuf;
use std::time::SystemTime;
use world::droid::WaveScaling;
use world::rewards::Rewards;

use crate::screens::designer::ShipSession;

/// How often a file's time stamp is looked at, in seconds.
const LOOK_EVERY: f32 = 0.5;

pub struct WaveConfigPlugin;

impl Plugin for WaveConfigPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Watched::<WaveScaling>::new())
            .insert_resource(Watched::<Rewards>::new())
            .add_systems(
                Update,
                (
                    (reload::<WaveScaling>, apply::<WaveScaling>).chain(),
                    (reload::<Rewards>, apply::<Rewards>).chain(),
                ),
            );
    }
}

/// A set of dials one file holds, and where the world keeps them.
pub(crate) trait Dials:
    Copy + PartialEq + serde::de::DeserializeOwned + Send + Sync + 'static
{
    /// The file at the root of the tree.
    const FILE: &'static str;
    /// The variable that names another.
    const ENV: &'static str;
    /// The constants.
    const UNTUNED: Self;
    /// One line of what the dials say.
    fn describe(&self) -> String;
    fn of(world: &world::World) -> Self;
    fn hand(self, world: &mut world::World);
}

impl Dials for WaveScaling {
    const FILE: &'static str = "scaling.ron";
    const ENV: &'static str = "BIMS_SCALING";
    const UNTUNED: Self = WaveScaling::DEFAULT;
    fn describe(&self) -> String {
        format!(
            "{} + {}/player + {}/step of {} days, waves {:?} by tier, first mission -{}",
            self.base,
            self.per_player,
            self.per_step,
            self.step_days.max(1),
            self.tier_waves,
            self.first_mission_ease
        )
    }
    fn of(world: &world::World) -> Self {
        world.wave_scaling()
    }
    fn hand(self, world: &mut world::World) {
        world.set_wave_scaling(self);
    }
}

impl Dials for Rewards {
    const FILE: &'static str = "rewards.ron";
    const ENV: &'static str = "BIMS_REWARDS";
    const UNTUNED: Self = Rewards::DEFAULT;
    fn describe(&self) -> String {
        format!(
            "{} xp and €{:?} a down (defence {}%, {}), buyback €{}, relics €{:?}, combine €{}, shelf {}%",
            self.xp_per_down,
            self.bounty,
            self.defense_bounty_percent,
            if self.bounty_waits_for_clear {
                "paid at the clear"
            } else {
                "paid at once"
            },
            self.buyback,
            self.relic_price,
            self.combine_fee,
            self.shelf_price_percent
        )
    }
    fn of(world: &world::World) -> Self {
        world.rewards()
    }
    fn hand(self, world: &mut world::World) {
        world.set_rewards(self);
    }
}

/// A file, when it was last read, and the dials it said.
#[derive(Resource)]
pub(crate) struct Watched<T: Dials> {
    path: PathBuf,
    /// The modification time last read; `None` for no file.
    stamp: Option<SystemTime>,
    since_look: f32,
    dials: T,
}

impl<T: Dials> Watched<T> {
    fn new() -> Watched<T> {
        let path = std::env::var(T::ENV).unwrap_or_else(|_| T::FILE.to_string());
        let mut watched = Watched {
            path: PathBuf::from(path),
            stamp: None,
            // Look at once, on the first frame.
            since_look: LOOK_EVERY,
            dials: T::UNTUNED,
        };
        watched.look();
        watched
    }

    /// The dials the file says now: the game setup's difficulty shows the
    /// wave file's three until the host picks others.
    pub(crate) fn dials(&self) -> T {
        self.dials
    }

    /// Read the file again if its time stamp moved.
    fn look(&mut self) {
        let stamp = std::fs::metadata(&self.path)
            .and_then(|m| m.modified())
            .ok();
        if stamp == self.stamp {
            return;
        }
        self.stamp = stamp;
        if stamp.is_none() {
            if self.dials != T::UNTUNED {
                eprintln!("tuning: {} gone, the constants again", self.path.display());
            }
            self.dials = T::UNTUNED;
            return;
        }
        match std::fs::read_to_string(&self.path)
            .map_err(|e| e.to_string())
            .and_then(|text| parse::<T>(&text))
        {
            Ok(dials) => {
                self.dials = dials;
                eprintln!("tuning: {} read: {}", self.path.display(), dials.describe());
            }
            Err(why) => eprintln!(
                "tuning: {} not read, the last good dials kept: {why}",
                self.path.display()
            ),
        }
    }
}

/// The dials a file says.
fn parse<T: Dials>(text: &str) -> Result<T, String> {
    ron::from_str::<T>(text).map_err(|e| e.to_string())
}

fn reload<T: Dials>(time: Res<Time>, mut watched: ResMut<Watched<T>>) {
    watched.since_look += time.delta_secs();
    if watched.since_look < LOOK_EVERY {
        return;
    }
    watched.since_look = 0.0;
    watched.look();
}

/// Hand the world the dials whenever its own differ: a new file, a new
/// game, a restart or a load.
fn apply<T: Dials>(watched: Res<Watched<T>>, session: Option<ResMut<ShipSession>>) {
    let Some(mut session) = session else {
        return;
    };
    let differs = session
        .0
        .game
        .as_ref()
        .is_some_and(|game| T::of(&game.world) != watched.dials);
    if differs && let Some(game) = session.0.game.as_mut() {
        watched.dials.hand(&mut game.world);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The files at the root of the tree are the constants, so a game run
    /// from there plays as it would untuned — and they parse.
    #[test]
    fn the_shipped_files_are_the_constants() {
        let text = include_str!("../../../scaling.ron");
        assert_eq!(parse(text), Ok(WaveScaling::DEFAULT));
        let text = include_str!("../../../rewards.ron");
        assert_eq!(parse(text), Ok(Rewards::DEFAULT));
    }

    /// A field left out is the constant's.
    #[test]
    fn a_field_left_out_is_the_constant_s() {
        let s: WaveScaling = parse("(base: 7)").unwrap();
        assert_eq!(s.base, 7);
        assert_eq!(s.step_days, WaveScaling::DEFAULT.step_days);
        assert!(parse::<WaveScaling>("(base: -1)").is_err());
        let s: WaveScaling = parse("(tier_waves: (2, 3, 6))").unwrap();
        assert_eq!(s.tier_waves, [2, 3, 6]);
        assert_eq!(s.base, WaveScaling::DEFAULT.base);
        let r: Rewards = parse("(buyback: 3)").unwrap();
        assert_eq!(r.buyback, 3);
        assert_eq!(r.bounty, Rewards::DEFAULT.bounty);
    }
}
