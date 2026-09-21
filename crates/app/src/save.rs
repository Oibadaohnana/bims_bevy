//! Saving and loading: where the files are, and the two pages that write
//! and read them.
//!
//! What a save *is* is `ship::save` — the world as text — and this is the
//! rest: the directory, the names, and the pages. The Esc sheet has both
//! pages (`settings.rs`, `Sheet::Save` and `Sheet::Load`) and the menu at
//! the start has the Load page in a window of its own, so a game is picked
//! up where it was left without walking through the setup. Neither page
//! touches the session: each hands back a [`Request`], and the screen that
//! laid it out does the writing or the reading, because only the screen
//! knows what a loaded game replaces.
//!
//! The saves live in `bims/saves` under the data directory
//! (`$XDG_DATA_HOME`, or `~/.local/share`), one `<name>.ron` each, or
//! wherever `BIMS_SAVES_DIR` says — a smoke run is pointed at a scratch
//! directory so it cannot touch anybody's game. A name is letters, digits,
//! `-` and `_`, the same rule as a station sketch's, so it cannot name a
//! path.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use bevy_egui::egui;

use crate::screens::station::valid_name;
use crate::theme;

/// What a save is called when nothing has been typed.
pub const DEFAULT_NAME: &str = "game";

/// One file on disk.
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    /// How long ago it was written, in words.
    pub age: String,
}

/// What a page asked for. The screen does it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Request {
    /// Write the game as `name`.
    Save(String),
    /// Read the game at `path` and play it.
    Load(PathBuf),
}

/// The pages' state: the name being typed, the files found the last time
/// a page opened, and what happened last.
#[derive(Default)]
pub struct Saves {
    pub name: String,
    pub listed: Vec<Entry>,
    /// The last outcome, and whether it went wrong.
    pub note: Option<(String, bool)>,
    /// Which file the Load page has picked, by index into `listed`.
    pub picked: Option<usize>,
}

impl Saves {
    /// Read the directory again: what a page does as it opens, so a file
    /// written by another run shows up.
    pub fn refresh(&mut self) {
        self.listed = list();
        self.picked = self.picked.filter(|&i| i < self.listed.len());
        if self.name.is_empty() {
            self.name = self
                .listed
                .first()
                .map(|e| e.name.clone())
                .unwrap_or_else(|| DEFAULT_NAME.to_string());
        }
    }

    pub fn saved(&mut self, name: &str) {
        self.note = Some((format!("Saved as {name}."), false));
        self.refresh();
    }

    pub fn failed(&mut self, what: String) {
        self.note = Some((what, true));
    }
}

/// Where the saves live.
pub fn dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("BIMS_SAVES_DIR")
        && !dir.is_empty()
    {
        return dir.into();
    }
    let base = match std::env::var_os("XDG_DATA_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => match std::env::var_os("HOME") {
            Some(home) => PathBuf::from(home).join(".local").join("share"),
            None => PathBuf::from("."),
        },
    };
    base.join("bims").join("saves")
}

/// A save's file, by name.
pub fn path_of(name: &str) -> PathBuf {
    dir().join(format!("{name}.ron"))
}

/// Every save in the directory, newest first.
pub fn list() -> Vec<Entry> {
    let Ok(entries) = std::fs::read_dir(dir()) else {
        return Vec::new();
    };
    let now = SystemTime::now();
    let mut found: Vec<(SystemTime, Entry)> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_stem()?.to_str()?.to_string();
            if path.extension()?.to_str()? != "ron" || !valid_name(&name) {
                return None;
            }
            let written = entry.metadata().ok()?.modified().ok()?;
            let age = age_of(now.duration_since(written).ok()?.as_secs());
            Some((written, Entry { name, path, age }))
        })
        .collect();
    found.sort_by_key(|found| std::cmp::Reverse(found.0));
    found.into_iter().map(|(_, e)| e).collect()
}

/// Write `text` as `name`, making the directory if it is not there.
pub fn write(name: &str, text: &str) -> Result<PathBuf, String> {
    if !valid_name(name) {
        return Err("A name is letters, digits, '-' and '_'.".to_string());
    }
    let dir = dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not make {}: {e}", dir.display()))?;
    let path = path_of(name);
    std::fs::write(&path, text).map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    Ok(path)
}

pub fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("Could not read {}: {e}", path.display()))
}

/// How a load went wrong, in words.
pub fn load_error(err: ship::save::LoadError) -> String {
    match err {
        ship::save::LoadError::Version(v) => format!(
            "Saved by another version of the game ({v}; this one writes {}).",
            ship::save::SAVE_VERSION
        ),
        ship::save::LoadError::Syntax(what) => format!("Not a save: {what}"),
    }
}

/// Seconds ago, in words.
fn age_of(seconds: u64) -> String {
    let (n, unit) = match seconds {
        s if s < 60 => return "just now".to_string(),
        s if s < 3600 => (s / 60, "minute"),
        s if s < 86_400 => (s / 3600, "hour"),
        s => (s / 86_400, "day"),
    };
    if n == 1 {
        format!("{n} {unit} ago")
    } else {
        format!("{n} {unit}s ago")
    }
}

/// The last outcome, under a page.
fn note(ui: &mut egui::Ui, saves: &Saves) {
    if let Some((text, warn)) = &saves.note {
        let color = if *warn { theme::WARN } else { theme::ACCENT };
        ui.label(egui::RichText::new(text).small().color(color));
    }
}

/// The saves as rows, one picked: the name, and how long ago. `None`
/// when there is nothing on disk yet. Clicking a row picks it, and a
/// double click is the button under the list.
fn rows(ui: &mut egui::Ui, saves: &mut Saves) -> bool {
    let mut chosen = false;
    if saves.listed.is_empty() {
        ui.label(egui::RichText::new("No saves yet.").color(theme::MUTED));
        return false;
    }
    egui::ScrollArea::vertical()
        .max_height(240.0)
        .show(ui, |ui| {
            for (i, entry) in saves.listed.iter().enumerate() {
                let picked = saves.picked == Some(i);
                let row = ui.horizontal(|ui| {
                    let label =
                        egui::Button::selectable(picked, egui::RichText::new(&entry.name).strong());
                    let response = ui.add_sized(egui::vec2(200.0, 0.0), label);
                    ui.label(egui::RichText::new(&entry.age).small().color(theme::MUTED));
                    response
                });
                let response = row.inner;
                if response.clicked() {
                    saves.picked = Some(i);
                    saves.name = entry.name.clone();
                }
                if response.double_clicked() {
                    saves.picked = Some(i);
                    saves.name = entry.name.clone();
                    chosen = true;
                }
            }
        });
    chosen
}

/// The Save page: a name, the saves already there to write over, and the
/// button. `can_save` is whether there is a game to write — the design
/// phase has none — and the page says so instead.
pub fn save_page(ui: &mut egui::Ui, saves: &mut Saves, can_save: bool) -> Option<Request> {
    let mut request = None;
    ui.set_min_width(360.0);
    if !can_save {
        ui.label(
            egui::RichText::new("Nothing to save yet: the game starts when the ship is accepted.")
                .color(theme::MUTED),
        );
        return None;
    }
    theme::heading(ui, "Name");
    ui.horizontal(|ui| {
        let field = ui.add(
            egui::TextEdit::singleline(&mut saves.name)
                .char_limit(40)
                .desired_width(200.0),
        );
        let ok = valid_name(&saves.name);
        let submitted = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let button = ui.add_enabled(ok, egui::Button::new("Save"));
        if (button.clicked() || (submitted && ok)) && ok {
            request = Some(Request::Save(saves.name.clone()));
        }
    });
    ui.label(
        egui::RichText::new("Letters, digits, '-' and '_'. A name already used is written over.")
            .small()
            .color(theme::MUTED),
    );
    ui.add_space(6.0);
    theme::heading(ui, "Saved");
    if rows(ui, saves) {
        request = Some(Request::Save(saves.name.clone()));
    }
    ui.add_space(4.0);
    note(ui, saves);
    request
}

/// The Load page: the saves, one picked, and the button.
pub fn load_page(ui: &mut egui::Ui, saves: &mut Saves) -> Option<Request> {
    let mut request = None;
    ui.set_min_width(360.0);
    theme::heading(ui, "Saved");
    let chosen = rows(ui, saves);
    ui.add_space(6.0);
    let picked = saves.picked.and_then(|i| saves.listed.get(i));
    let button = ui.add_enabled(picked.is_some(), egui::Button::new("Load"));
    if let Some(entry) = picked
        && (button.clicked() || chosen)
    {
        request = Some(Request::Load(entry.path.clone()));
    }
    ui.label(
        egui::RichText::new(format!("From {}", dir().display()))
            .small()
            .color(theme::MUTED),
    );
    ui.add_space(4.0);
    note(ui, saves);
    request
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A save written under a scratch directory is listed, newest first,
    /// and read back as it was; a name that could be a path is refused.
    #[test]
    fn a_save_is_listed_with_its_age_and_read_back_from_where_it_was_written() {
        // --- an_age_is_said_in_the_largest_unit_that_fits ---
        {
            assert_eq!(age_of(5), "just now");
            assert_eq!(age_of(60), "1 minute ago");
            assert_eq!(age_of(150), "2 minutes ago");
            assert_eq!(age_of(3600 * 5), "5 hours ago");
            assert_eq!(age_of(86_400 * 3), "3 days ago");
        }

        // --- a_save_is_listed_and_read_back_from_the_directory_it_was_written_to ---
        {
            let dir = std::env::temp_dir().join(format!("bims-saves-test-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            // SAFETY: the tests in this crate that read the environment run
            // on one thread — `cargo test` runs a crate's tests in parallel,
            // but nothing else in `app` reads `BIMS_SAVES_DIR`.
            unsafe { std::env::set_var("BIMS_SAVES_DIR", &dir) };
            assert!(list().is_empty());
            assert!(write("../escape", "x").is_err());
            let path = write("first", "(one)").unwrap();
            assert_eq!(read(&path).unwrap(), "(one)");
            let listed = list();
            assert_eq!(listed.len(), 1);
            assert_eq!(listed[0].name, "first");
            assert_eq!(listed[0].age, "just now");
            let _ = std::fs::remove_dir_all(&dir);
            unsafe { std::env::remove_var("BIMS_SAVES_DIR") };
        }
    }
}
