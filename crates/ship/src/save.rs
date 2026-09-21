//! The save file: a game written out, and read back.
//!
//! A game is its [`World`] — the clock, the ship, the room aboard, the
//! stations and everything the crew own — and the four numbers the session
//! round it was opened with: how many players, which this one is, the
//! galaxy's type and the spawn the lobby gave. That is all that goes in.
//! Nothing the app keeps is saved — which panel is open, where the camera
//! is, what is being aimed at — because none of it is the game, and a
//! load stands those up afresh the way an open does ([`Game::resume`]).
//!
//! The text is RON, one value: [`encode`] writes it and [`decode`] reads
//! it, and every type in the world derives serde under the `serde`
//! feature the rules crates carry for this. Where the file goes and what
//! it is called are the app's (`crates/app/src/save.rs`), like every other
//! path. A file from another build is refused by its `version` rather than
//! read wrong: bump [`SAVE_VERSION`] whenever a saved type changes shape.

use world::World;

use crate::Session;
use crate::editor::Editor;
use crate::game::Game;

/// Bumped whenever a saved type changes shape, so an old file is told
/// apart from a broken one. 2: feature 54 grew `Surface` (its biome and
/// population), `Station` (its population), the room's bay (pace and
/// outdoor), its layout (the fields) and its sight (the daylight). 4:
/// the room grew whose bunk is whose (`sleeps_in`) and a Bim the deck's
/// clocks (`ground_left`, `ground_window`, `sore`, `bed`). 5: the
/// workbench grew its slots (`World::bench` for `upgrade`) and the room
/// the carries to it (`ferries` and their three reports).
pub const SAVE_VERSION: u32 = 5;

/// What the file holds, read back.
#[derive(serde::Deserialize)]
pub struct Save {
    pub version: u32,
    pub players: u32,
    pub local: u32,
    /// The galaxy type's code, as the lobby gave it — `Session::galaxy`.
    pub galaxy: u32,
    /// Where the game began — `Session::spawn`.
    pub spawn: Option<(u32, u32)>,
    pub world: World,
}

/// The same, borrowed for writing: a world is not `Clone`.
#[derive(serde::Serialize)]
struct Written<'a> {
    version: u32,
    players: u32,
    local: u32,
    galaxy: u32,
    spawn: Option<(u32, u32)>,
    world: &'a World,
}

/// The version alone, off the front of the text: `encode` writes it
/// first, as `(version:N,`, and it is read by hand rather than through a
/// struct that ignores the rest — RON tells a struct from a tuple by
/// scanning ahead to the matching bracket, and ignoring a world that way
/// is quadratic in its size: minutes for a save. `None` for text that
/// does not start so, which the full read then refuses in its own words.
fn version_of(text: &str) -> Option<u32> {
    let rest = text.strip_prefix("(version:")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Why a file could not be read.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LoadError {
    /// Saved by a build with a different [`SAVE_VERSION`].
    Version(u32),
    /// Not a save at all, or one cut short: what the parser said.
    Syntax(String),
}

/// The game as text. `None` in the design phase: there is no world yet.
pub fn encode(session: &Session) -> Option<String> {
    let game = session.game.as_ref()?;
    let written = Written {
        version: SAVE_VERSION,
        players: session.editor.players,
        local: session.editor.local,
        galaxy: session.galaxy,
        spawn: session.spawn,
        world: &game.world,
    };
    ron::to_string(&written).ok()
}

/// The text read back.
pub fn decode(text: &str) -> Result<Save, LoadError> {
    if let Some(version) = version_of(text)
        && version != SAVE_VERSION
    {
        return Err(LoadError::Version(version));
    }
    ron::from_str(text).map_err(|e| LoadError::Syntax(e.to_string()))
}

impl Session {
    /// The game, written out — [`encode`].
    pub fn save(&self) -> Option<String> {
        encode(self)
    }

    /// A session stood up round a saved game: the world as it was, the
    /// editor settled on its ship the way `simulate_on` settles it, and
    /// the game round it as at an open ([`Game::resume`]).
    pub fn restore(text: &str, width: f32, height: f32) -> Result<Session, LoadError> {
        let Save {
            players,
            local,
            galaxy,
            spawn,
            world,
            ..
        } = decode(text)?;
        let seed = world.galaxy_seed;
        let editor = Editor::settled(world.ship.design.clone(), players, local, width, height);
        let game = Game::resume(world, local, width, height);
        Ok(Session::resumed(editor, game, seed, galaxy, spawn))
    }
}
