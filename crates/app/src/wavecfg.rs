//! The wave formula's dials, tuned while the game runs.
//!
//! `waves.ron` (or the file `BIMS_WAVES` names) holds a
//! [`world::droid::WaveScaling`]: the base machines a wave, how many a
//! player and a time step add, how long a time step is, the waves a held
//! station has and the first mission's ease. The file is looked at twice a
//! second; when it changes it is read again and the world is handed the new
//! dials, which the next wave laid and the next wave count settled take.
//! A wave already standing keeps its size. A field left out is the
//! constant's; a file that does not parse keeps the last good dials and
//! says why on stderr; no file at all is the constants.
//!
//! The dials are neither saved nor hashed: they are handed over again every
//! frame the world's differ, so a restart or a load takes them too. In a
//! two-player run each game reads its own file, and the two have to agree.

use bevy::prelude::*;
use std::path::PathBuf;
use std::time::SystemTime;
use world::droid::WaveScaling;

use crate::screens::designer::ShipSession;

/// How often the file's time stamp is looked at, in seconds.
const LOOK_EVERY: f32 = 0.5;

pub struct WaveConfigPlugin;

impl Plugin for WaveConfigPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WaveConfig::new())
            .add_systems(Update, (reload, apply).chain());
    }
}

/// The file, when it was last read, and the dials it said.
#[derive(Resource)]
struct WaveConfig {
    path: PathBuf,
    /// The modification time last read; `None` for no file.
    stamp: Option<SystemTime>,
    since_look: f32,
    scaling: WaveScaling,
}

impl WaveConfig {
    fn new() -> WaveConfig {
        let path = std::env::var("BIMS_WAVES").unwrap_or_else(|_| "waves.ron".to_string());
        let mut config = WaveConfig {
            path: PathBuf::from(path),
            stamp: None,
            // Look at once, on the first frame.
            since_look: LOOK_EVERY,
            scaling: WaveScaling::DEFAULT,
        };
        config.look();
        config
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
            if self.scaling != WaveScaling::DEFAULT {
                eprintln!("waves: {} gone, the constants again", self.path.display());
            }
            self.scaling = WaveScaling::DEFAULT;
            return;
        }
        match std::fs::read_to_string(&self.path)
            .map_err(|e| e.to_string())
            .and_then(|text| parse(&text))
        {
            Ok(scaling) => {
                self.scaling = scaling;
                eprintln!(
                    "waves: {} read: {}",
                    self.path.display(),
                    describe(&scaling)
                );
            }
            Err(why) => eprintln!(
                "waves: {} not read, the last good dials kept: {why}",
                self.path.display()
            ),
        }
    }
}

/// The dials a file says.
fn parse(text: &str) -> Result<WaveScaling, String> {
    ron::from_str::<WaveScaling>(text).map_err(|e| e.to_string())
}

/// One line of what the dials make of a wave.
fn describe(s: &WaveScaling) -> String {
    format!(
        "{} + {}/player + {}/step of {} h, {} waves + 1 every {} steps, first mission -{}",
        s.base,
        s.per_player,
        s.per_step,
        s.step_hours.max(1),
        s.waves_base,
        s.steps_per_wave,
        s.first_mission_ease
    )
}

fn reload(time: Res<Time>, mut config: ResMut<WaveConfig>) {
    config.since_look += time.delta_secs();
    if config.since_look < LOOK_EVERY {
        return;
    }
    config.since_look = 0.0;
    config.look();
}

/// Hand the world the dials whenever its own differ: a new file, a new
/// game, a restart or a load.
fn apply(config: Res<WaveConfig>, session: Option<ResMut<ShipSession>>) {
    let Some(mut session) = session else {
        return;
    };
    let differs = session
        .0
        .game
        .as_ref()
        .is_some_and(|game| game.world.wave_scaling() != config.scaling);
    if differs && let Some(game) = session.0.game.as_mut() {
        game.world.set_wave_scaling(config.scaling);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The file at the root of the tree is the constants, so a game run
    /// from there plays as it would untuned — and it parses.
    #[test]
    fn the_shipped_file_is_the_constants() {
        let text = include_str!("../../../waves.ron");
        assert_eq!(parse(text), Ok(WaveScaling::DEFAULT));
    }

    /// A field left out is the constant's.
    #[test]
    fn a_field_left_out_is_the_constant_s() {
        let s = parse("(base: 7)").unwrap();
        assert_eq!(s.base, 7);
        assert_eq!(s.step_hours, WaveScaling::DEFAULT.step_hours);
        assert!(parse("(base: -1)").is_err());
    }
}
