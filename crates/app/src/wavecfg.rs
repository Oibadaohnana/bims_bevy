//! The tuning files, read again while the game runs.
//!
//! - `scaling.ron` (or the file `BIMS_SCALING` names) holds a
//!   [`world::droid::WaveScaling`] (task 147; areas since October 2026):
//!   the enemies a player and a bot bring, and the four areas — area 0 and
//!   tiers one to three — each so many days with its own growth, waves,
//!   extras, elite and defenders: the whole of how the enemies scale.
//! - `rewards.ron` (or `BIMS_REWARDS`) holds a [`world::rewards::Rewards`]:
//!   the experience and the money an enemy down is worth, what a defence
//!   pays of it, whether the money waits for the clear, and what the
//!   buyback and the trader's shelf cost.
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
//! The game setup's difficulty (`world::droid::Difficulty`, every dial of
//! the formula) stands in place of this file for the run it starts — the
//! world keeps it and saves it, and the host deals it to every guest — so
//! the file moves a run only when the setup left it as the file had it.

use bevy::prelude::*;
use bims::balance::WeaponDamage;
use bims::combat::WeaponKind;
use std::path::PathBuf;
use std::time::SystemTime;
use world::droid::WaveScaling;
use world::rewards::Rewards;

use crate::net::Online;
use crate::screens::run::ShipSession;
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
    /// What the host dealt of these for a run with company, if it did.
    fn dealt(_: &Dealt) -> Option<Self> {
        None
    }
}

/// The host's tuning for a run with company: dealt at a Start
/// (`builder::Settings::rewards`, `weapons`, `difficulty`) and sent with
/// every world the host sends whole (`Packet::World` — a load from the
/// lobby or in the game, a restart, a retry, a resync). While a run with
/// company is on, these and not this machine's files are what the world
/// is handed — on the host too — so an edit to a file mid-run moves no
/// machine's world away from the others'. Cleared at a Start of one.
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Dealt {
    pub scaling: WaveScaling,
    pub rewards: Rewards,
    pub weapons: WeaponDamage,
}

/// What is dealt now, for the whole process: the weapons are a process
/// global already (`WeaponDamage::arm`), and the screens that send and
/// take a world are several.
static DEALT: std::sync::Mutex<Option<Dealt>> = std::sync::Mutex::new(None);

impl Dealt {
    /// This machine's own files, as they say now.
    pub fn of_files() -> Dealt {
        Dealt {
            scaling: file_dials(),
            rewards: file_dials(),
            weapons: file_dials(),
        }
    }

    /// The world given these dials — before it steps, so nothing between
    /// a world arriving and the watcher's next look runs on others.
    pub fn onto(&self, world: &mut world::World) {
        if world.wave_scaling() != self.scaling {
            world.set_wave_scaling(self.scaling);
        }
        if world.rewards() != self.rewards {
            world.set_rewards(self.rewards);
        }
    }
}

/// Deal these for the run (`None`: this machine's files again), the
/// weapons armed at once.
pub(crate) fn deal(dealt: Option<Dealt>) {
    if let Some(d) = dealt {
        d.weapons.arm();
    }
    if let Ok(mut now) = DEALT.lock() {
        *now = dealt;
    }
}

/// What a host sends with a world: what it plays — what was dealt, else
/// its own files (a load from the lobby, which has no Start).
pub(crate) fn dealing() -> Dealt {
    DEALT
        .lock()
        .ok()
        .and_then(|d| *d)
        .unwrap_or_else(Dealt::of_files)
}

/// The dealt tuning, while the run it was dealt for still has company:
/// with the host gone the game is this machine's and its files count.
fn dealt_now(online: &Option<Res<Online>>) -> Option<Dealt> {
    if !online.as_ref().is_some_and(|o| o.is_online()) {
        return None;
    }
    DEALT.lock().ok().and_then(|d| *d)
}

impl Tuning for WaveScaling {
    const FILE: &'static str = "scaling.ron";
    const ENV: &'static str = "BIMS_SCALING";
    const UNTUNED: Self = WaveScaling::DEFAULT;
    fn describe(&self) -> String {
        let area = |a: &world::droid::Area| {
            format!(
                "{} days +{}/day {} waves {}b {}l {}g {}e {}d",
                a.days,
                a.growth_per_day,
                a.waves,
                a.bombers,
                a.lancers,
                a.guardians,
                a.elites,
                a.defenders
            )
        };
        format!(
            "{}/player, {}/bot; area 0 {}; tier 1 {}; tier 2 {}; tier 3 {}",
            self.enemies_per_player,
            self.enemies_per_bot,
            area(&self.area_0),
            area(&self.tier_1_area),
            area(&self.tier_2_area),
            area(&self.tier_3_area)
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
    fn dealt(dealt: &Dealt) -> Option<Self> {
        Some(dealt.scaling)
    }
}

impl Tuning for Rewards {
    const FILE: &'static str = "rewards.ron";
    const ENV: &'static str = "BIMS_REWARDS";
    const UNTUNED: Self = Rewards::DEFAULT;
    fn describe(&self) -> String {
        format!(
            "a site {} xp (+{}% a day), an enemy €{} (+{}% a day) (defence {}%, a player's {}%, a bot's {}%, {}), buyback €{}, shelf {}%",
            self.site_xp,
            self.site_xp_growth_percent,
            self.enemy_money,
            self.enemy_money_growth_percent,
            self.defense_bounty_percent,
            self.player_bounty_percent,
            self.bot_bounty_percent,
            if self.bounty_waits_for_clear {
                "paid at the clear"
            } else {
                "paid at once"
            },
            self.buyback,
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
    fn dealt(dealt: &Dealt) -> Option<Self> {
        Some(dealt.rewards)
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
        // A game started outside the tree's root finds none of the files
        // and plays the constants, and an edit to the file there does
        // nothing: said once, at the start, with where it looked.
        if watched.stamp.is_none() {
            let at = std::path::absolute(&watched.path).unwrap_or_else(|_| watched.path.clone());
            eprintln!(
                "tuning: no {} (start the game in the folder that holds it, or set {}), the constants",
                at.display(),
                T::ENV
            );
        }
        watched
    }

    /// The dials the file says now: the game setup's difficulty shows the
    /// wave file's five until the host picks others.
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

/// The dials the file says right now, read once — the constants where
/// there is none or it does not parse. For a command that wants the
/// file's numbers before the watcher has handed them over.
pub(crate) fn file_dials<T: Tuning>() -> T {
    std::fs::read_to_string(path_of::<T>())
        .ok()
        .and_then(|text| parse::<T>(&text).ok())
        .unwrap_or(T::UNTUNED)
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

/// Write the game setup's difficulty into the wave file — the setup's
/// Save as default. Only the dials' numbers change: the comments and any
/// other line stay as they were written, a field the file left out is put
/// in, and no file at all becomes one holding the dials. The text is read
/// back before it replaces the file, so a file this could not have edited
/// is refused rather than broken; the watcher reads it again like any
/// other save.
pub(crate) fn save_difficulty(difficulty: WaveScaling) -> Result<(), String> {
    let path = path_of::<WaveScaling>();
    let old = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "(\n)\n".to_string(),
        Err(e) => return Err(e.to_string()),
    };
    parse::<WaveScaling>(&old)?;
    let new = with_difficulty(&old, difficulty).ok_or("no closing bracket to write before")?;
    if parse::<WaveScaling>(&new) != Ok(difficulty) {
        return Err("the file is not laid out one dial a line".into());
    }
    // Beside it and then over it, so a reader never sees half a file.
    let mut part = path.clone().into_os_string();
    part.push(".part");
    std::fs::write(&part, new).map_err(|e| e.to_string())?;
    std::fs::rename(&part, &path).map_err(|e| e.to_string())
}

/// `text` with every dial's number put in: each on the line that sets it,
/// or on a line of its own before the closing bracket when none does.
/// `None` when there is no closing bracket.
fn with_difficulty(text: &str, d: WaveScaling) -> Option<String> {
    // `{:?}` keeps the point, so the file reads a float back as a float.
    let area = |a: &world::droid::Area| {
        format!(
            "(days: {}, growth_per_day: {:?}, waves: {}, bombers: {}, lancers: {}, guardians: {}, elites: {}, defenders: {})",
            a.days,
            a.growth_per_day,
            a.waves,
            a.bombers,
            a.lancers,
            a.guardians,
            a.elites,
            a.defenders
        )
    };
    let fields = [
        ("enemies_per_player", d.enemies_per_player.to_string()),
        ("enemies_per_bot", format!("{:?}", d.enemies_per_bot)),
        ("area_0", area(&d.area_0)),
        ("tier_1_area", area(&d.tier_1_area)),
        ("tier_2_area", area(&d.tier_2_area)),
        ("tier_3_area", area(&d.tier_3_area)),
    ];
    let mut done = [false; 6];
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
            // Past the number — or an area's bracketed row, on the one
            // line — to whatever follows it: the comma, a comment,
            // nothing.
            let number = after.trim_start();
            let rest = if number.starts_with('(') {
                let Some(close) = number.find(')') else {
                    continue;
                };
                &number[close + 1..]
            } else {
                number.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.')
            };
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

/// Arm the file's damage for every room whenever the armed differ — or
/// the host's, dealt for a run with company ([`Dealt`]).
fn arm(watched: Res<Watched<WeaponDamage>>, online: Option<Res<Online>>) {
    let want = dealt_now(&online).map_or(watched.dials, |d| d.weapons);
    if WeaponDamage::armed() != want {
        want.arm();
    }
}

/// Hand the session's world the file's dials whenever its own differ — or
/// the host's, dealt for a run with company ([`Dealt`]).
fn apply<T: Dials>(
    watched: Res<Watched<T>>,
    session: Option<ResMut<ShipSession>>,
    online: Option<Res<Online>>,
) {
    let Some(mut session) = session else {
        return;
    };
    let want = dealt_now(&online)
        .as_ref()
        .and_then(T::dealt)
        .unwrap_or(watched.dials);
    let differs = session
        .0
        .game
        .as_ref()
        .is_some_and(|game| T::of(&game.world) != want);
    if differs && let Some(game) = session.0.game.as_mut() {
        want.hand(&mut game.world);
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

    /// Save as default puts the dials' numbers in and leaves every other
    /// line of the file as it was written.
    #[test]
    fn saving_the_difficulty_changes_its_numbers_and_nothing_else() {
        let area = |days, growth_per_day, waves| world::droid::Area {
            days,
            growth_per_day,
            waves,
            bombers: 2,
            lancers: 1,
            guardians: 3,
            elites: 1,
            defenders: 4,
        };
        let d = WaveScaling {
            enemies_per_player: 4,
            enemies_per_bot: 1.5,
            area_0: area(3, 0.25, 1),
            tier_1_area: area(12, 0.5, 2),
            tier_2_area: area(7, 1.0, 3),
            tier_3_area: area(2, 0.75, 4),
        };
        let text = include_str!("../../../scaling.ron");
        let new = with_difficulty(text, d).unwrap();
        assert_eq!(parse(&new), Ok(d));
        let changed = text
            .lines()
            .zip(new.lines())
            .filter(|(a, b)| a != b)
            .count();
        assert!(changed <= 6, "{changed} lines changed");
        assert_eq!(text.lines().count(), new.lines().count());
        // A comment after the number stays, and a field left out is put in.
        let new = with_difficulty("(\n    enemies_per_bot: 1.0, // one\n)\n", d).unwrap();
        assert!(
            new.starts_with("(\n    enemies_per_bot: 1.5, // one\n"),
            "{new}"
        );
        assert_eq!(parse(&new), Ok(d));
        // A word in a comment is not a field, and an area's row is
        // written whole over the one it had.
        let new = with_difficulty(
            "// area_0: (days: 9)\n(\n    tier_1_area: (days: 1, waves: 9), // tier one\n)\n",
            d,
        )
        .unwrap();
        assert!(new.starts_with("// area_0: (days: 9)\n"));
        assert!(new.contains("), // tier one\n"), "{new}");
        assert_eq!(parse(&new), Ok(d));
    }

    /// A field left out is the constant's.
    #[test]
    fn a_field_left_out_is_the_constant_s() {
        let s: WaveScaling = parse("(enemies_per_player: 7)").unwrap();
        assert_eq!(s.enemies_per_player, 7);
        assert_eq!(s.area_0, WaveScaling::DEFAULT.area_0);
        assert!(parse::<WaveScaling>("(enemies_per_player: -1)").is_err());
        let s: WaveScaling = parse("(enemies_per_bot: 1.5)").unwrap();
        assert_eq!(s.enemies_per_bot, 1.5);
        // The name it had before still reads.
        let s: WaveScaling = parse("(enemies_per_defender: 2.5)").unwrap();
        assert_eq!(s.enemies_per_bot, 2.5);
        assert_eq!(s.tier_2_area, WaveScaling::DEFAULT.tier_2_area);
        // A dial left out of an area's row is nought (a wave, one): the
        // row is whole or the area is the built-in one.
        let s: WaveScaling = parse("(tier_1_area: (days: 3))").unwrap();
        assert_eq!(s.tier_1_area.days, 3);
        assert_eq!(s.tier_1_area.waves, 1);
        assert_eq!(s.tier_1_area.guardians, 0);
        let r: Rewards = parse("(buyback: 3)").unwrap();
        assert_eq!(r.buyback, 3);
        assert_eq!(r.enemy_money, Rewards::DEFAULT.enemy_money);
        let w: WeaponDamage = parse("(auto_rifle: (9.0, 8.0))").unwrap();
        assert_eq!(w.of(WeaponKind::AutoRifle), (9.0, 8.0));
        assert_eq!(w.laser_pistol, WeaponDamage::DEFAULT.laser_pistol);
    }
}
