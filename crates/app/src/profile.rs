//! The player's profile between runs (feature 106): which classes are
//! unlocked and how many runs were won —
//! `world::relic::Profile`, the rules' — kept as `profile.ron` in the
//! data directory beside the saves (`$XDG_DATA_HOME/bims`, or
//! `~/.local/share/bims`), or wherever `BIMS_PROFILE_DIR` says.
//!
//! Every machine keeps its own and writes it itself: a won run is counted
//! in the profile of every player who took part, each on their own disk.
//! The **host's** decides a run's classes, which cross the lobby with
//! the rest of the settings (`net::SettingsWire`) and are kept for the run
//! as [`RunUnlocks`]. (It decided the run's relic pool too, until every
//! relic was in every run in October 2026.)

use std::path::PathBuf;

use bevy::prelude::Resource;
use world::{Class, Profile};

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

/// A run won at ascension `at`, on this machine: counted in this
/// player's profile, the next ascension opened. The ascension it opened,
/// if it opened one. A profile that cannot be written is said on the
/// terminal.
pub fn record_win(at: u32) -> Option<u32> {
    let mut profile = load();
    let was = profile.ascension;
    profile.record_win(at);
    if let Err(why) = save(&profile) {
        eprintln!("profile: not written: {why}");
    }
    (profile.ascension > was).then_some(profile.ascension)
}

/// The highest ascension the setup lets this machine pick. Every one,
/// for now (the player's word: "let all ascensions be unlocked for
/// now"); `profile.ascension` is still counted up by every win and is
/// what the setup starts on ([`ascension_reached`]).
pub fn ascension_open() -> u32 {
    world::ascension::MOST
}

/// The highest ascension this machine's profile has opened —
/// `BIMS_ASCENSION` (`dev::ascension`) in its place, for a look at one:
/// what the setup starts on.
pub fn ascension_reached() -> u32 {
    crate::dev::ascension()
        .unwrap_or_else(|| load().ascension)
        .min(world::ascension::MOST)
}

/// What the host's profile opened for this run: the classes that may be
/// picked, as bits — a class's code.
/// Fixed at the start and the same on every machine of a lobby.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct RunUnlocks {
    pub classes: u32,
}

impl RunUnlocks {
    /// This machine's own profile's: what a run with no host but itself
    /// opens with, and what a host sends.
    pub fn of(profile: &Profile) -> RunUnlocks {
        RunUnlocks {
            classes: Class::ALL
                .into_iter()
                .filter(|&c| profile.class_unlocked(c))
                .fold(0, |m, c| m | 1 << c.code()),
        }
    }

    /// Whether a class may be picked this run.
    pub fn class_open(&self, class: Class) -> bool {
        !class.unlockable() || self.classes & (1 << class.code()) != 0
    }
}

/// A run won and written into this machine's profile, with the ascension
/// it opened, if any. Present once the end screen has recorded the win,
/// which is what keeps it to once a win; taken away when a run opens.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct Victory {
    pub opened: Option<u32>,
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
    fn a_new_profile_opens_every_class() {
        let unlocks = RunUnlocks::default();
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

    /// A profile written before the relics were rebuilt — with the list of
    /// relics it had unlocked — still reads, its classes and wins kept.
    #[test]
    fn an_old_profile_with_its_relics_still_reads() {
        let old = "(relics: [0, 1, 2], classes: [1, 2], wins: 3)";
        let profile = ship::save::read_profile(old).expect("an old profile reads");
        assert_eq!(profile.classes, vec![1, 2]);
        assert_eq!(profile.wins, 3);
    }
}
