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
//! - `audio.ron` (or `BIMS_AUDIO`) holds a [`crate::sound::Volumes`]: the
//!   player's volume for each sound, handed to the sound player rather
//!   than the world.
//! - `weapons.ron` (or `BIMS_WEAPONS`) holds a
//!   [`bims::balance::WeaponDamage`]: every weapon's damage near and far,
//!   armed for every room in the process rather than handed to the world,
//!   and taken from the next shot.
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
use bims::balance::WeaponDamage;
use bims::combat::WeaponKind;
use std::path::PathBuf;
use std::time::SystemTime;
use world::droid::{Difficulty, WaveScaling};
use world::rewards::Rewards;

use crate::screens::designer::ShipSession;
use crate::sound::{Sounds, Volumes};

/// How often a file's time stamp is looked at, in seconds.
const LOOK_EVERY: f32 = 0.5;

pub struct WaveConfigPlugin;

impl Plugin for WaveConfigPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Watched::<WaveScaling>::new())
            .insert_resource(Watched::<Rewards>::new())
            .insert_resource(Watched::<Volumes>::new())
            .insert_resource(Watched::<WeaponDamage>::new())
            .add_systems(
                Update,
                (
                    (reload::<WaveScaling>, apply::<WaveScaling>).chain(),
                    (reload::<Rewards>, apply::<Rewards>).chain(),
                    (reload::<Volumes>, hear).chain(),
                    (reload::<WeaponDamage>, arm).chain(),
                ),
            );
    }
}

/// A set of dials one file holds.
pub(crate) trait Tuning:
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
}

/// Dials the world keeps.
pub(crate) trait Dials: Tuning {
    fn of(world: &world::World) -> Self;
    fn hand(self, world: &mut world::World);
}

impl Tuning for WaveScaling {
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
}

impl Dials for WaveScaling {
    fn of(world: &world::World) -> Self {
        world.wave_scaling()
    }
    fn hand(self, world: &mut world::World) {
        world.set_wave_scaling(self);
    }
}

impl Tuning for Rewards {
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
}

impl Dials for Rewards {
    fn of(world: &world::World) -> Self {
        world.rewards()
    }
    fn hand(self, world: &mut world::World) {
        world.set_rewards(self);
    }
}

impl Tuning for Volumes {
    const FILE: &'static str = "audio.ron";
    const ENV: &'static str = "BIMS_AUDIO";
    const UNTUNED: Self = Volumes::AS_BUILT;
    fn describe(&self) -> String {
        let turned: Vec<String> = self
            .turned()
            .map(|(name, v)| format!("{name} {v}"))
            .collect();
        if turned.is_empty() {
            "every sound as built".into()
        } else {
            turned.join(", ")
        }
    }
}

impl Tuning for WeaponDamage {
    const FILE: &'static str = "weapons.ron";
    const ENV: &'static str = "BIMS_WEAPONS";
    const UNTUNED: Self = WeaponDamage::DEFAULT;
    fn describe(&self) -> String {
        let turned: Vec<String> = WeaponKind::EVERY
            .iter()
            .filter(|&&kind| self.of(kind) != WeaponDamage::DEFAULT.of(kind))
            .map(|&kind| {
                let (near, far) = self.of(kind);
                format!("{kind:?} {near}/{far}")
            })
            .collect();
        if turned.is_empty() {
            "every weapon's damage as built".into()
        } else {
            turned.join(", ")
        }
    }
}

/// A file, when it was last read, and the dials it said.
#[derive(Resource)]
pub(crate) struct Watched<T: Tuning> {
    path: PathBuf,
    /// The modification time last read; `None` for no file.
    stamp: Option<SystemTime>,
    since_look: f32,
    dials: T,
}

impl<T: Tuning> Watched<T> {
    fn new() -> Watched<T> {
        let mut watched = Watched {
            path: path_of::<T>(),
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
fn parse<T: Tuning>(text: &str) -> Result<T, String> {
    ron::from_str::<T>(text).map_err(|e| e.to_string())
}

/// Where a set of dials is read from: the file its variable names, or
/// the one at the root of the tree.
fn path_of<T: Tuning>() -> PathBuf {
    PathBuf::from(std::env::var(T::ENV).unwrap_or_else(|_| T::FILE.to_string()))
}

/// Write the game setup's difficulty into the wave file as its base, per
/// player and per step — the setup's Save as default. Only those three
/// numbers change: the comments and the other dials stay as they were
/// written, a field the file left out is put in, and no file at all
/// becomes one holding the three. The text is read back before it
/// replaces the file, so a file this could not have edited is refused
/// rather than broken; the watcher reads it again like any other save.
pub(crate) fn save_difficulty(difficulty: Difficulty) -> Result<(), String> {
    let path = path_of::<WaveScaling>();
    let old = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "(\n)\n".to_string(),
        Err(e) => return Err(e.to_string()),
    };
    let before = parse::<WaveScaling>(&old)?;
    let new = with_difficulty(&old, difficulty).ok_or("no closing bracket to write before")?;
    if parse::<WaveScaling>(&new) != Ok(difficulty.over(before)) {
        return Err("the file is not laid out one dial a line".into());
    }
    // Beside it and then over it, so a reader never sees half a file.
    let mut part = path.clone().into_os_string();
    part.push(".part");
    std::fs::write(&part, new).map_err(|e| e.to_string())?;
    std::fs::rename(&part, &path).map_err(|e| e.to_string())
}

/// `text` with the three difficulty dials' numbers put in: each on the
/// line that sets it, or on a line of its own before the closing bracket
/// when none does. `None` when there is no closing bracket.
fn with_difficulty(text: &str, d: Difficulty) -> Option<String> {
    let fields = [
        ("base", d.base),
        ("per_player", d.per_player),
        ("per_step", d.per_step),
    ];
    let mut done = [false; 3];
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    for line in &mut lines {
        let code = line.split("//").next().unwrap_or("");
        let at = code.len() - code.trim_start().len();
        let mut put = None;
        for (i, (name, value)) in fields.iter().enumerate() {
            let Some(after) = code[at..].strip_prefix(name) else {
                continue;
            };
            let Some(after) = after.trim_start().strip_prefix(':') else {
                continue;
            };
            // Past the number to whatever follows it: the comma, a
            // comment, nothing.
            let number = after.trim_start();
            let rest = number.trim_start_matches(|c: char| c.is_ascii_digit());
            let tail = &line[code.len() - rest.len()..];
            put = Some(format!("{}{name}: {value}{tail}", &code[..at]));
            done[i] = true;
            break;
        }
        if let Some(put) = put {
            *line = put;
        }
    }
    let close = lines
        .iter()
        .rposition(|l| l.split("//").next().unwrap_or("").contains(')'))?;
    let missing: Vec<String> = fields
        .iter()
        .zip(done)
        .filter(|(_, done)| !done)
        .map(|((name, value), _)| format!("    {name}: {value},"))
        .collect();
    lines.splice(close..close, missing);
    let mut out = lines.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    Some(out)
}

fn reload<T: Tuning>(time: Res<Time>, mut watched: ResMut<Watched<T>>) {
    watched.since_look += time.delta_secs();
    if watched.since_look < LOOK_EVERY {
        return;
    }
    watched.since_look = 0.0;
    watched.look();
}

/// Hand the sound player the file's volumes whenever its own differ.
fn hear(watched: Res<Watched<Volumes>>, sounds: Option<ResMut<Sounds>>) {
    if let Some(mut sounds) = sounds
        && sounds.volumes != watched.dials
    {
        sounds.volumes = watched.dials;
    }
}

/// Arm the file's damage for every room whenever the armed differ.
fn arm(watched: Res<Watched<WeaponDamage>>) {
    if WeaponDamage::armed() != watched.dials {
        watched.dials.arm();
    }
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

    /// The rewards file at the root of the tree is the constants, so a
    /// game run from there pays as it would untuned. The wave file is the
    /// player's — the setup's Save as default writes it — so it only has
    /// to parse.
    #[test]
    fn the_shipped_files_parse_and_the_rewards_are_the_constants() {
        let text = include_str!("../../../scaling.ron");
        assert!(parse::<WaveScaling>(text).is_ok());
        let text = include_str!("../../../rewards.ron");
        assert_eq!(parse(text), Ok(Rewards::DEFAULT));
        let text = include_str!("../../../weapons.ron");
        assert_eq!(parse(text), Ok(WeaponDamage::DEFAULT));
    }

    /// Save as default puts the three numbers in and leaves every other
    /// line of the file as it was written.
    #[test]
    fn saving_the_difficulty_changes_its_three_numbers_and_nothing_else() {
        let d = Difficulty {
            base: 4,
            per_player: 12,
            per_step: 0,
        };
        let text = include_str!("../../../scaling.ron");
        let new = with_difficulty(text, d).unwrap();
        let before: WaveScaling = parse(text).unwrap();
        assert_eq!(parse(&new), Ok(d.over(before)));
        let changed: Vec<(&str, &str)> = text
            .lines()
            .zip(new.lines())
            .filter(|(a, b)| a != b)
            .collect();
        assert_eq!(changed.len(), 3, "{changed:?}");
        assert_eq!(text.lines().count(), new.lines().count());
        // A comment after the number stays, and a field left out is put in.
        let new = with_difficulty("(\n    base: 1, // one\n    step_days: 3,\n)\n", d).unwrap();
        assert_eq!(
            new,
            "(\n    base: 4, // one\n    step_days: 3,\n    per_player: 12,\n    per_step: 0,\n)\n"
        );
        // A word in a comment is not a field.
        let new = with_difficulty("// base: 9\n(\n)\n", d).unwrap();
        assert!(new.starts_with("// base: 9\n"));
        assert_eq!(parse::<WaveScaling>(&new).unwrap().base, 4);
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
        let w: WeaponDamage = parse("(auto_rifle: (9.0, 8.0))").unwrap();
        assert_eq!(w.of(WeaponKind::AutoRifle), (9.0, 8.0));
        assert_eq!(w.laser_pistol, WeaponDamage::DEFAULT.laser_pistol);
    }
}
