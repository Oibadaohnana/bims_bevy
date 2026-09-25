//! The player's profile between runs (feature 106): which relics and
//! classes are unlocked and how many runs were won —
//! `world::relic::Profile`, the rules' — kept as `profile.ron` in the
//! data directory beside the saves (`$XDG_DATA_HOME/bims`, or
//! `~/.local/share/bims`), or wherever `BIMS_PROFILE_DIR` says.
//!
//! Every machine keeps its own and writes it itself: a won run unlocks
//! relics in the profile of every player who took part, each on their own
//! disk. The **host's** decides a run's pool and its classes, which cross
//! the lobby with the rest of the settings (`net::SettingsWire`) and are
//! kept for the run as [`RunUnlocks`].

use std::path::PathBuf;

use bevy::prelude::Resource;
use world::{Class, Profile, Relic};

/// The directory the profile lives in.
pub fn dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("BIMS_PROFILE_DIR")
        && !dir.is_empty()
    {
        return dir.into();
    }
    // The saves' own directory's parent: `bims/`, beside `bims/saves/`.
    // A `BIMS_SAVES_DIR` moves the saves and leaves the profile where it
    // is — a smoke run points it elsewhere through `BIMS_PROFILE_DIR`.
    let base = match std::env::var_os("XDG_DATA_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => match std::env::var_os("HOME") {
            Some(home) => PathBuf::from(home).join(".local").join("share"),
            None => PathBuf::from("."),
        },
    };
    base.join("bims")
}

/// The profile's file.
pub fn path() -> PathBuf {
    dir().join("profile.ron")
}

/// This machine's profile: what is on disk, or a new one where there is
/// nothing there or nothing readable — which is never written over until
/// a run is won.
pub fn load() -> Profile {
    std::fs::read_to_string(path())
        .ok()
        .and_then(|text| ship::save::read_profile(&text))
        .unwrap_or_default()
}

/// The profile written back.
pub fn save(profile: &Profile) -> Result<PathBuf, String> {
    let path = path();
    let text = ship::save::profile_text(profile).ok_or("the profile would not write")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(path)
}

/// A run won, on this machine: the relics it unlocks — the first still
/// locked, in list order — written to this player's profile, and said
/// back for the victory screen. A profile that cannot be written is said
/// on the terminal and the unlocks are still shown.
pub fn record_win() -> Vec<Relic> {
    let mut profile = load();
    let fresh = profile.record_run(true);
    if let Err(why) = save(&profile) {
        eprintln!("profile: not written: {why}");
    }
    fresh
}

/// What the host's profile opened for this run: its relic pool and the
/// classes that may be picked, as bits — a relic's code, a class's code.
/// Fixed at the start and the same on every machine of a lobby.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct RunUnlocks {
    pub relics: u64,
    pub classes: u32,
}

impl RunUnlocks {
    /// This machine's own profile's: what a run with no host but itself
    /// opens with, and what a host sends.
    pub fn of(profile: &Profile) -> RunUnlocks {
        RunUnlocks {
            relics: world::relic::mask_of(&profile.pool()),
            classes: Class::ALL
                .into_iter()
                .filter(|&c| profile.class_unlocked(c))
                .fold(0, |m, c| m | 1 << c.code()),
        }
    }

    /// The run's pool.
    pub fn pool(&self) -> Vec<Relic> {
        world::relic::relics_of_mask(self.relics)
    }

    /// Whether a class may be picked this run.
    pub fn class_open(&self, class: Class) -> bool {
        !class.unlockable() || self.classes & (1 << class.code()) != 0
    }
}

/// A run won and written into this machine's profile: what it unlocked,
/// for the victory screen. Present once the end screen has recorded the
/// win, which is what keeps it to once a win; taken away when a run
/// opens.
#[derive(Resource, Clone, Debug, Default)]
pub struct Victory {
    pub unlocked: Vec<Relic>,
}

impl Default for RunUnlocks {
    fn default() -> RunUnlocks {
        RunUnlocks::of(&Profile::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_profile_opens_the_starting_pool_and_every_class() {
        let unlocks = RunUnlocks::default();
        assert_eq!(unlocks.pool(), world::relic::starting_pool());
        for class in Class::ALL {
            assert!(unlocks.class_open(class));
        }
    }

    #[test]
    fn a_profile_reads_back_as_it_was_written() {
        let mut profile = Profile::new();
        profile.record_run(true);
        let text = ship::save::profile_text(&profile).unwrap();
        assert_eq!(ship::save::read_profile(&text), Some(profile));
        assert_eq!(ship::save::read_profile("not a profile"), None);
    }
}
