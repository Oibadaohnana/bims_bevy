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
/// the carries to it (`ferries` and their three reports). 6: the world
/// grew the raids (`World::raids`, `lost`) and a Bim whether it hunts.
/// 7: the world grew the enemies' shelves laid out as loot
/// (`World::plunder`). 8: a Bim's `selected` became a mask, one bit a
/// player, and the room grew how many of the crew are players' own
/// (`players`) — feature 59, the wire. 9: the crew's names, as the players
/// gave them (`Session::crew_names`) — feature 60. 10: a Bim's look
/// became a yoke, a hair, a shade and a build (`character::Look`) —
/// feature 62. 11: the research grew its queue (`Research::queue`) —
/// feature 64. 12: a docked raid grew whether the ship's airlock has
/// given (`Raid::Docked::breached`) — feature 68. 13: a chain on the queue
/// grew whether it is an order waiting its turn (`Saved::ordered`) and a
/// queued walk (`Kind::Walk`) — feature 69. 14: a station's key became a
/// tier (`World::station_keys`, a `u8` a station), the cargo grew the
/// tier-two key and the research tree the upgrades node — tier two. 15:
/// the world grew what every station has lost (`World::losses`) and every
/// system left as it was left (`World::memories`) — feature 71. 16: the
/// world grew the players' classes and the crew's progress through them
/// (`World::classes`, `progress`, `undocked_once`), the deployables
/// (`World::deployables`, `reused_kits`), the residents' experience
/// flags, the workbench a repair, a Bim its trigger and a room its
/// sentries — feature 74. 17: the cargo grew the grenade, a Bim its brace
/// and its *rampage* stacks, a bolt and a hit whose they were, a room
/// its grenades, the world when each crew member last threw
/// (`World::last_throw`) and the residents who last hit each of them —
/// feature 75. 18: the world grew the medics (`World::medics` — each
/// beam's patients, the surge's charge, the field surgery), a Bim its
/// beam flag and its surge, and a treatment whether it was bare —
/// feature 76. 21: the world grew the machines (`World::infested` —
/// which stations they hold, the waves left, when the next is due — and
/// the tier, the reinforcement clock and the wave cap the probes move),
/// a room its `droids` and a `combat::Hit` its roll and its strip —
/// feature 83. 23: the world grew the dead lying on the stations'
/// decks (`World::graves` — whose deck, where on it, what is still on
/// the body and what it looked like) and the nodes the ship has been at
/// (`World::visited`), both of them filed with the system's memory as
/// well, and a station's room which of its bodies were laid out from a
/// grave (`Residents::grave`) — feature 85.
pub const SAVE_VERSION: u32 = 23;

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
    /// The crew's names in slot order — `Session::crew_names`. Empty at a
    /// slot is the app's default; a file from before the names is refused
    /// by its version, so the field is not optional.
    pub crew_names: Vec<String>,
    pub world: World,
}

/// The same, borrowed for writing: a world is not `Clone`. The order of
/// the fields is the order they are written in, and the first two are
/// read off the front by hand (`version_of`, `players_of`): keep
/// `version` first and `players` right behind it.
#[derive(serde::Serialize)]
struct Written<'a> {
    version: u32,
    players: u32,
    local: u32,
    galaxy: u32,
    spawn: Option<(u32, u32)>,
    crew_names: &'a [String],
    world: &'a World,
}

/// The version alone, off the front of the text: `encode` writes it
/// first, as `(version:N,`, and it is read by hand rather than through a
/// struct that ignores the rest — RON tells a struct from a tuple by
/// scanning ahead to the matching bracket, and ignoring a world that way
/// is quadratic in its size: minutes for a save. `None` for text that
/// does not start so, which the full read then refuses in its own words.
fn version_of(text: &str) -> Option<u32> {
    let (version, _) = number_after(text, "(version:")?;
    Some(version)
}

/// How many players the game was saved for, read off the front the same
/// way: `players` is written right after the version, as
/// `(version:N,players:M,`, so a host can tell whether a file fits the
/// room before it reads the world (feature 67). `None` for text that
/// does not start so.
pub fn players_of(text: &str) -> Option<u32> {
    let (_, rest) = number_after(text, "(version:")?;
    let (players, _) = number_after(rest, ",players:")?;
    Some(players)
}

/// The digits after `head` at the front of `text`, and what follows them.
fn number_after<'a>(text: &'a str, head: &str) -> Option<(u32, &'a str)> {
    let rest = text.strip_prefix(head)?;
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    let number = rest[..end].parse().ok()?;
    Some((number, &rest[end..]))
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
        crew_names: &session.crew_names,
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
    /// the game round it as at an open ([`Game::resume`]) — as the player
    /// the file says it was saved by.
    pub fn restore(text: &str, width: f32, height: f32) -> Result<Session, LoadError> {
        let save = decode(text)?;
        let local = save.local;
        Ok(Self::stood_up(save, local, width, height))
    }

    /// [`Session::restore`], but as the player in slot `local` rather than
    /// the one the file was saved by: a guest handed the host's world
    /// over the wire comes up as *its* crew member (feature 67). A slot
    /// the crew has not got is the last one, as `Game::resume` clamps it.
    pub fn restore_as(
        text: &str,
        local: u32,
        width: f32,
        height: f32,
    ) -> Result<Session, LoadError> {
        let save = decode(text)?;
        Ok(Self::stood_up(save, local, width, height))
    }

    fn stood_up(save: Save, local: u32, width: f32, height: f32) -> Session {
        let Save {
            players,
            galaxy,
            spawn,
            world,
            crew_names,
            ..
        } = save;
        let local = local.min(players.saturating_sub(1));
        let seed = world.galaxy_seed;
        let editor = Editor::settled(world.ship.design.clone(), players, local, width, height);
        let game = Game::resume(world, local, width, height);
        let mut session = Session::resumed(editor, game, seed, galaxy, spawn);
        session.crew_names = crew_names;
        session
    }
}
