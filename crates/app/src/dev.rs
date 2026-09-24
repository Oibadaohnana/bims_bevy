//! Running the game with nobody at the keyboard.
//!
//! `BIMS_SMOKE_FRAMES=n` runs `n` frames and exits, which is how a build is
//! checked to open a window and draw without anybody watching it; add
//! `BIMS_SCREENSHOT=file.png` and the frame thirty before the end is saved
//! there, which is how a change to a screen is *looked at* from a terminal.
//! `BIMS_WINDOW=1400x900` asks for a window size and `BIMS_FULLSCREEN=1`
//! for the whole screen; a tiling desktop ignores the first, so the second
//! is how a screenshot comes out at the size a player would have.
//!
//! `BIMS_POINTER="40:move:600,250;60:click:600,250;90:right:300,400"` moves
//! and clicks the pointer at those frames, in logical pixels from the
//! window's top left — the way a harness drives a screen. Move at least a
//! frame before clicking: egui hit-tests a click against the widgets laid
//! out on the previous frame. `press`, `move`, `release` (and `rpress`,
//! `rrelease` for the right button) across several frames are a drag.
//! `wheel` and `wheelup` at a point are a notch of the wheel, which zooms.
//! `BIMS_KEYS="60:Escape,90:M"` presses keys. `+Shift` at a frame holds
//! Shift down and `-Shift` lets it go — the two frames apart, since
//! bevy_egui reads the modifier a frame after it is written — so
//! `BIMS_KEYS="58:+Shift,66:-Shift"` round a `right` at 62 is a
//! Shift-right-click: an order that waits its turn (feature 69).
//!
//! `BIMS_LANDED=1` opens it landed on the
//! spawn system's first planet with ground, the settlement beside the pad.
//! `BIMS_LANDING=1` opens it over that planet with the landing just begun,
//! `=0.7` seven tenths of the way down.
//! `BIMS_FIGHT=1` opens it with a fight staged
//! at the dock: the station hostile, the crew member recruited inside its
//! door and one of its people down the corridor. `BIMS_WEAPON=schword` (or
//! `pistol`, `shotgun`, `rifle`, `sniper`) puts that in the crew member's
//! hand instead of the pistol, and `BIMS_ENEMY_WEAPON=…` the same in every
//! resident's — how a swing, a burst or a long shot is looked at.
//! `BIMS_GRAVES=n` leaves `n` of the station's people dead where they
//! stand and builds its room again over the bodies (feature 85), which
//! is how the dead lying on a station's deck are looked at without
//! fighting, flying away and coming back.
//! `BIMS_TRADE=1` opens the simulation with the station's trade window
//! up, which is how the cart is looked at. `BIMS_ARMOURY=1` opens it with
//! the armoury window up, which is how the lockers' grid is looked at;
//! `=storage` and `=fridge` the shelves' and the cold store's.
//!
//! `BIMS_SOUND_LOG=1` prints every clip as it is played and every bed as
//! it starts or stops — how a sound is *heard* from a terminal, where a
//! hidden window has no speaker anybody is listening to.
//!
//! `BIMS_PERF=1` says where the frame went ([`crate::perf`]): the parts of
//! it are timed and printed as a table beside the "ms a frame" line when
//! the run exits. It wants `BIMS_SMOKE_FREE=1` with it, a paced frame
//! being a sixtieth however heavy it is.
//!
//! `BIMS_AUTO=create` and `BIMS_AUTO=join:<code>` play the lobby and the
//! yard with nobody at the keyboard, against the relay `BIMS_SERVER` names
//! — how a game with company is looked at from a terminal ([`auto`]).
//! `BIMS_BIM_NAME` fills the setup's name field for such a run ([`bim_name`])
//! and `BIMS_BIM_HAIR` its hair chooser ([`bim_hair`]). `BIMS_DESYNC_AT=<steps>`
//! parts a guest's world from the host's on purpose at that step, for
//! looking at the resync ([`desync_at`]); `scratchpad/duo_resync.sh` is
//! the pair of runs that does, and the host's load with company.

use bevy::diagnostic::{DiagnosticsStore, FrameCount, FrameTimeDiagnosticsPlugin};
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::input::mouse::{MouseButton, MouseButtonInput, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::{CursorMoved, WindowEvent, WindowResolution};

/// `BIMS_LANDED=1` opens the simulation set down on the spawn system's
/// first planet with ground, tied up at its settlement — how the ground,
/// the pad and the two buildings are looked at without the descent.
pub fn landed() -> bool {
    std::env::var("BIMS_LANDED").as_deref() == Ok("1")
}

/// `BIMS_LANDING=1` opens the simulation holding over that planet with the
/// landing just begun, and `BIMS_LANDING=0.7` with it run seven tenths of
/// the way down — how the descent is looked at at any point of it: the
/// planet growing under the ship, and the black at the end.
pub fn landing() -> Option<f64> {
    let value = std::env::var("BIMS_LANDING").ok()?;
    if value == "1" {
        return Some(0.0);
    }
    value.parse::<f64>().ok().map(|done| done.clamp(0.0, 0.999))
}

/// `BIMS_ZOOM=0.3` zooms the game view by that factor about the middle of
/// the canvas once it has been fitted: under one is out, over one in.
/// How a whole town, or the plain round it, is looked at from a
/// terminal, since a scripted wheel does not reach the game view.
pub fn zoom() -> Option<f32> {
    std::env::var("BIMS_ZOOM")
        .ok()?
        .parse::<f32>()
        .ok()
        .filter(|z| *z > 0.0 && z.is_finite())
}

/// `BIMS_AFIELD=1` opens the simulation landed (as `BIMS_LANDED=1`) with
/// the crew member walked out onto the ground west of the ship and a
/// minute gone by, so the plain and the fog lifting from it are looked
/// at — with `BIMS_ZOOM` out, the whole of what is seen.
pub fn afield() -> bool {
    std::env::var("BIMS_AFIELD").as_deref() == Ok("1")
}

/// `BIMS_FIGHT=1` opens the simulation with the dock made hostile, the
/// crew member recruited just inside the station's door and one of the
/// station's people a few tiles down the corridor — how a fight is looked
/// at without walking the station for one.
pub fn fight() -> bool {
    std::env::var("BIMS_FIGHT").as_deref() == Ok("1")
}

/// `BIMS_GRAVES=n` opens the simulation with `n` of the station
/// alongside dead where they stand and its room built again over the
/// bodies (feature 85) — how the dead lying on a station's deck are
/// looked at without a fight, a flight away and a flight back.
pub fn graves() -> Option<u32> {
    std::env::var("BIMS_GRAVES").ok()?.parse().ok()
}

/// `BIMS_RAID=1` opens the simulation off its berth with a raider tied to
/// the ship and its boarders on their way through the airlock — how a
/// raid is looked at without holding a day for one; `BIMS_RAID=contact`
/// opens it with the raider on the radar and closing, for the map.
/// Whether one was asked for, and whether it is to dock.
pub fn raid() -> Option<bool> {
    match std::env::var("BIMS_RAID").as_deref() {
        Ok("1") => Some(true),
        Ok("contact") => Some(false),
        _ => None,
    }
}

/// `BIMS_LOST=1` opens the simulation with every crew member shot where
/// they stand: the run over on the first step, and the screen that says
/// so on the next frame.
pub fn lost() -> bool {
    std::env::var("BIMS_LOST").as_deref() == Ok("1")
}

/// `BIMS_DYING=n` puts `n` of the crew into a dying state — a part at
/// nothing with its trauma untreated and wounds open on it — for looking
/// at the red cross over a body on the deck and at the peril block under
/// the health bar (`Session::maim_for_probe`).
pub fn dying() -> Option<usize> {
    std::env::var("BIMS_DYING").ok()?.trim().parse().ok()
}

/// `BIMS_FIELD_MEDIC=n` makes the last `n` of the crew **hired field
/// medics** (feature 86): the contract, the two medkits, and no money
/// taken — for looking at what one does in a fight without flying to a
/// station and hiring one (`Session::field_medics_for_probe`).
pub fn field_medics() -> Option<usize> {
    std::env::var("BIMS_FIELD_MEDIC").ok()?.trim().parse().ok()
}

/// `BIMS_CARRY=1` takes a crew member out cold and puts it in a medic's
/// arms — for looking at a body being carried off the deck without
/// waiting for a fight to put one there (`Session::carry_for_probe`).
/// Wants somebody who may carry: a medic of the class (`BIMS_CLASS=medic`)
/// or a hired one (`BIMS_FIELD_MEDIC=1`).
pub fn carry() -> bool {
    std::env::var("BIMS_CARRY").as_deref() == Ok("1")
}

/// `BIMS_BANDAGES=n` puts `n` dressings in **every** crew member's pack
/// and sets the Management tab's carry target to `n` (feature 87) — for
/// looking at a box of them in the inventory, and at what a crew that
/// binds its own wounds does in a fight, without waiting for the restock
/// to fill the packs a step at a time. Five go in one box, so
/// `BIMS_BANDAGES=7` is a full box beside a part one
/// (`Session::bandages_for_probe`).
pub fn bandages() -> Option<u32> {
    std::env::var("BIMS_BANDAGES").ok()?.trim().parse().ok()
}

/// `BIMS_KITS=n` puts exactly `n` of **each** of the engineer's two kits
/// in every crew member's pack and starts both cooldowns afresh (feature
/// 88). `BIMS_KITS=0` is the state a scripted run cannot walk itself
/// into — no charge in hand and the whole wait ahead — which is how the
/// seconds in the corner of the two boxes are looked at
/// (`Session::kits_for_probe`).
pub fn kits() -> Option<u32> {
    std::env::var("BIMS_KITS").ok()?.trim().parse().ok()
}

/// `BIMS_GRENADES=n` is the same for the soldier's grenade charges
/// (feature 90): exactly `n` in every crew member's pack with the
/// cooldown started afresh, so `BIMS_GRENADES=0 bims combat_soldier` is
/// the `29s` in the corner of the Q box rather than a pointer hunting a
/// tile to throw at (`Session::grenades_for_probe`).
pub fn grenades() -> Option<u32> {
    std::env::var("BIMS_GRENADES").ok()?.trim().parse().ok()
}

/// `BIMS_LAMPS_OUT=n` shoots the `n` lamps nearest the crew member out at
/// open and leaves the next one failing — how a lamp out, the dark round
/// it and a failing lamp's flicker are looked at without a fight that
/// happens to hit one.
pub fn lamps_out() -> Option<usize> {
    std::env::var("BIMS_LAMPS_OUT").ok()?.parse().ok()
}

/// `BIMS_TRADE=1` opens the simulation with the station's trade window up
/// — how the cart is looked at without finding the Station button.
pub fn trade() -> bool {
    std::env::var("BIMS_TRADE").as_deref() == Ok("1")
}

/// `BIMS_ARMOURY=1` opens the simulation with the armoury window up — how
/// the lockers' grid is looked at without finding the armoury on deck;
/// `BIMS_ARMOURY=storage` the first shelf's window, `=fridge` the first
/// cold store's, `=workbench` the workbench's slots, `=plunder` the
/// enemy's shelf (with `BIMS_RAID=1`, the raider's). What to open, if
/// anything.
pub fn armoury() -> Option<String> {
    std::env::var("BIMS_ARMOURY").ok().filter(|s| !s.is_empty())
}

/// `BIMS_WEAPON=schword` puts a schword in the crew member's hand for the
/// run, and `BIMS_ENEMY_WEAPON=…` one in every resident's: a schword's
/// swing, a rifle's burst and a sniper's long shot are each looked at
/// this way rather than by waiting for a station to have issued one. A
/// word the table does not have is nobody's weapon changed. A digit on
/// the end is the tier — `pistol3` is a gold pistol — which is how the
/// tint on a cell and a slot is looked at without a day at the bench.
pub fn weapon() -> Option<bims::combat::Weapon> {
    weapon_named(std::env::var("BIMS_WEAPON").ok()?)
}

pub fn enemy_weapon() -> Option<bims::combat::Weapon> {
    weapon_named(std::env::var("BIMS_ENEMY_WEAPON").ok()?)
}

/// `BIMS_ARMOURED=1` puts a fresh helm, kevlar and leg guards on the crew
/// member for the run — pieces the hold has never seen, so the world's
/// mirror of the pieces ignores them — which is how a crew member lives
/// through a schword's first cut long enough for the melee lock to be
/// looked at.
pub fn armoured() -> bool {
    std::env::var("BIMS_ARMOURED").as_deref() == Ok("1")
}

/// `BIMS_SOUND_LOG=1` prints each sound as it is played: what a smoke run
/// sounded like, read off the log the way a screenshot is read.
pub fn sound_log() -> bool {
    std::env::var("BIMS_SOUND_LOG").as_deref() == Ok("1")
}

fn weapon_named(word: String) -> Option<bims::combat::Weapon> {
    use bims::combat::{Tier, WeaponKind};
    let (name, tier) = match word.strip_suffix(['2', '3']) {
        Some(name) => (name, Tier::from_code(word[name.len()..].parse().ok()?)?),
        None => (word.as_str(), Tier::One),
    };
    let kind = match name {
        "pistol" => WeaponKind::LaserPistol,
        "shotgun" => WeaponKind::Shotgun,
        "rifle" => WeaponKind::AutoRifle,
        "sniper" => WeaponKind::SniperRifle,
        "schword" => WeaponKind::Schword,
        _ => return None,
    };
    Some(kind.at(tier))
}

pub fn smoke_frames() -> Option<u32> {
    std::env::var("BIMS_SMOKE_FRAMES")
        .ok()
        .and_then(|v| v.parse().ok())
}

/// `BIMS_AUTO` plays the lobby and the yard with nobody at the keyboard,
/// which is how a game with company is looked at from a terminal — two
/// windows against a relay, `BIMS_SERVER` naming it. `BIMS_AUTO=create`
/// opens a lobby at the menu and prints `lobby: <code>` when the relay
/// deals one; `BIMS_AUTO=join:<code>` walks into it. Then the host, once
/// `BIMS_AUTO_PLAYERS` (default 2) are in the lobby, picks a random start
/// and presses Start, and every window presses Accept a second into the
/// yard, so the world opens on all of them. With it on, the game prints
/// `checksum: <steps> <hash>` at every checksum a guest matched, and
/// `desync` if one did not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Auto {
    Create,
    Join(String),
}

pub fn auto() -> Option<Auto> {
    let what = std::env::var("BIMS_AUTO").ok()?;
    match what.split_once(':') {
        None if what == "create" => Some(Auto::Create),
        Some(("join", code)) => Some(Auto::Join(code.to_string())),
        _ => None,
    }
}

/// `BIMS_DESYNC_AT=<steps>`: on a guest, once the world has gone that
/// many steps, one order is applied locally without asking the host — a
/// divergence on purpose, so the resync (feature 67, `Packet::Resync`)
/// can be looked at: the guest prints `diverged:`, then `desync:` at the
/// next checksum, and `resync:` when the host's world lands. Nothing on
/// a host or in a game of one.
pub fn desync_at() -> Option<u64> {
    std::env::var("BIMS_DESYNC_AT").ok()?.parse().ok()
}

/// How many the host waits for before it presses Start, under `BIMS_AUTO`.
pub fn auto_players() -> usize {
    std::env::var("BIMS_AUTO_PLAYERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2)
}

/// What the setup's name field starts with — `BIMS_BIM_NAME`, else
/// blank — so a run with nobody at the keyboard can name its Bim and the
/// name be read off the others' screenshots.
pub fn bim_name() -> String {
    std::env::var("BIMS_BIM_NAME").unwrap_or_default()
}

/// What the setup's hair chooser starts on — `BIMS_BIM_HAIR=<style>:<shade>`,
/// each a place in `Hair::ALL` / `Shade::ALL` or its name from `names.rs`
/// (`mohawk:red`), else the first crew member's own crop — so a run with
/// nobody at the keyboard can pick a head and it be read off a
/// screenshot.
pub fn bim_hair() -> (bims::character::Hair, bims::character::Shade) {
    use bims::character::{Hair, Look, Shade};
    let dealt = Look::of(0);
    let Ok(spec) = std::env::var("BIMS_BIM_HAIR") else {
        return (dealt.hair, dealt.shade);
    };
    let mut parts = spec.split(':');
    let pick = |word: Option<&str>, names: &[&str]| -> Option<usize> {
        let word = word?.trim();
        word.parse::<usize>()
            .ok()
            .or_else(|| names.iter().position(|n| n.eq_ignore_ascii_case(word)))
    };
    let hair = pick(parts.next(), &crate::names::HAIR_NAMES)
        .and_then(|i| Hair::ALL.get(i).copied())
        .unwrap_or(dealt.hair);
    let shade = pick(parts.next(), &crate::names::SHADE_NAMES)
        .and_then(|i| Shade::ALL.get(i).copied())
        .unwrap_or(dealt.shade);
    (hair, shade)
}

/// What the setup's colour chooser starts on (feature 84) —
/// `BIMS_BIM_TINT=<colour>`, a place in `Tint::ALL` or its name from
/// `names.rs` (`amber`), else slot nought's own — so a run with nobody
/// at the keyboard can pick the ring under its Bim and it be read off a
/// screenshot.
pub fn bim_tint() -> bims::character::Tint {
    use bims::character::Tint;
    let Ok(spec) = std::env::var("BIMS_BIM_TINT") else {
        return Tint::ALL[0];
    };
    let word = spec.trim();
    word.parse::<usize>()
        .ok()
        .or_else(|| {
            crate::names::TINT_NAMES
                .iter()
                .position(|n| n.eq_ignore_ascii_case(word))
        })
        .and_then(|i| Tint::ALL.get(i).copied())
        .unwrap_or(Tint::ALL[0])
}

/// What the setup's class chooser starts on — `BIMS_CLASS=<class>`, a
/// place in `Class::ALL` or its name from `names.rs` (`engineer`), else
/// none — so a run with nobody at the keyboard opens as an engineer
/// (feature 74). `BIMS_CLASS` reaches the simulation and the fight too:
/// `dev::class_crew` puts it on slot 0 of a session opened without a
/// design phase.
pub fn bim_class() -> world::Class {
    let Ok(spec) = std::env::var("BIMS_CLASS") else {
        return world::Class::None;
    };
    let word = spec.trim();
    word.parse::<usize>()
        .ok()
        .or_else(|| {
            crate::names::CLASS_NAMES
                .iter()
                .position(|n| n.eq_ignore_ascii_case(word))
        })
        .and_then(|i| world::Class::ALL.get(i).copied())
        .unwrap_or_default()
}

/// `BIMS_CLASS` onto slot 0 of a session that skipped the design phase —
/// the simulation, the fight, the tests — through the same
/// `World::set_class` the yard's choice goes through; and `BIMS_LEVEL=n`
/// straight to that level of it (`World::award` off the table), for
/// looking at what a level gives — a soldier's grenades from the third.
///
/// `asked` is the class the command itself named — `combat_medic` and
/// the rest, `Launch::CombatAs` (feature 79), and `combat_droids_medic`
/// and the rest, `Launch::DroidsAs`, which is the same class put on the
/// same slot for the machines' fight — and `Class::None` where it named
/// none. `BIMS_CLASS` wins over it, so a `combat_tank` run can
/// still be opened as somebody else without a different command;
/// `BIMS_LEVEL` applies to whichever of the two it ends up being.
///
/// **A `combat_<class>` run opens at [`COMBAT_CLASS_LEVEL`]** — the top
/// of the tree (feature 80): the fight is what a class is looked at in,
/// and at the first level there is nothing of it to look at but the one
/// key. Every level's pick is still the player's, waiting on the
/// level-up window the moment the screen opens. `BIMS_LEVEL` says
/// otherwise; every other launch starts at the first level as before.
///
/// **The engineer's sentry kit is its class's own**
/// (`world::deploy::SENTRY_CHARGES`): a scripted fight has
/// nobody to stand at a workbench for the minutes one takes, so the
/// class deals the one the Q is, and nothing here has to.
pub fn class_crew(session: &mut ship::Session, asked: world::Class) {
    let class = match bim_class() {
        world::Class::None => asked,
        chosen => chosen,
    };
    let level = bim_level().or((asked != world::Class::None).then_some(COMBAT_CLASS_LEVEL));
    if class != world::Class::None
        && let Some(game) = &mut session.game
    {
        let _ = game.world.set_class(0, class);
        if let Some(level) = level {
            let want = world::class::LEVEL_XP
                .get(level.saturating_sub(1))
                .copied()
                .unwrap_or(0);
            let mut events = Vec::new();
            game.world.award(0, want, &mut events);
        }
    }
}

/// The level a `combat_<class>` command opens its crew member at: the
/// top of the tree.
pub const COMBAT_CLASS_LEVEL: usize = world::class::LEVELS as usize;

/// `BIMS_BEAM=1` links a medic's heal beam to crew member 1, stood a
/// tile away with a wound on it, and `BIMS_BEAM=surge` triggers the
/// surge over that (`World::beam_for_probe`, feature 76) — how the beam
/// and the halo are looked at without hunting a crewmate with the
/// pointer. Nothing without a medic on slot 0 (`BIMS_CLASS=medic`) and
/// two aboard.
pub fn beam_crew(session: &mut ship::Session) {
    let Ok(word) = std::env::var("BIMS_BEAM") else {
        return;
    };
    let word = word.trim().to_ascii_lowercase();
    if word.is_empty() || word == "0" {
        return;
    }
    if let Some(game) = &mut session.game {
        game.world.beam_for_probe(word == "surge");
    }
}

/// `BIMS_LEVEL`: the level slot 0's class starts at, if any.
fn bim_level() -> Option<usize> {
    std::env::var("BIMS_LEVEL").ok()?.trim().parse().ok()
}

/// What tier the machines come at in the `droids` probes (feature 83):
/// `BIMS_DROID_TIER=2`, or a tier's place in `Tier::ALL`. `None` — unset,
/// or a word that is no tier — leaves it to how far the system is from
/// the machines' origin (feature 93, `World::droid_tier`).
pub fn droid_tier() -> Option<bims::combat::Tier> {
    let spec = std::env::var("BIMS_DROID_TIER").ok()?;
    spec.trim()
        .parse::<u32>()
        .ok()
        .and_then(bims::combat::Tier::from_code)
}

/// How big a wave the `droids` probes force: `BIMS_DROID_WAVE=32`, for
/// the measurements the feature asks for. `None` leaves
/// `data::DROID_WAVE_MAX` where it is.
pub fn droid_wave_max() -> Option<u32> {
    std::env::var("BIMS_DROID_WAVE").ok()?.trim().parse().ok()
}

/// How long the `droids` probes wait between waves, in minutes of the
/// world's clock: `BIMS_DROID_REINFORCE=600` over the commands' own
/// `screens::game::DROID_REINFORCE_IN_PROBE` (one). A minute is a real
/// second at 1× and two and a half **frames** at 24×, so the countdown
/// along the top cannot be looked at through a scripted run without
/// lengthening it. Nought or less reads as the default.
pub fn droid_reinforce(default: f64) -> f64 {
    std::env::var("BIMS_DROID_REINFORCE")
        .ok()
        .and_then(|spec| spec.trim().parse::<f64>().ok())
        .filter(|minutes| *minutes > 0.0)
        .unwrap_or(default)
}

/// How long the `defense` command waits between the crew landing at a
/// threatened town and the first wave, in minutes of the world's clock:
/// `BIMS_DEFENSE_DELAY=30` over the command's own
/// `screens::game::DEFENSE_DELAY_IN_PROBE` (one), where the game's own
/// is `world::data::DEFENSE_DELAY_MINUTES` (an hour). Nought or less
/// reads as the default.
pub fn defense_delay(default: f64) -> f64 {
    std::env::var("BIMS_DEFENSE_DELAY")
        .ok()
        .and_then(|spec| spec.trim().parse::<f64>().ok())
        .filter(|minutes| *minutes > 0.0)
        .unwrap_or(default)
}

/// Which day the machines' first star turns on in the `crisis` command
/// (feature 92): `BIMS_CRISIS_DAY=3` over `data::DROID_FIRST_DAY` (ten).
/// The clock opens a day short of it whatever it is
/// (`Session::crisis_for_probe`), so the dial is not about how long to
/// wait — it is about what the *rest* of the galaxy's days come out at,
/// since every other star is five days a hop after this one.
pub fn crisis_day() -> u32 {
    std::env::var("BIMS_CRISIS_DAY")
        .ok()
        .and_then(|spec| spec.trim().parse::<u32>().ok())
        .unwrap_or(world::data::DROID_FIRST_DAY)
}

/// How many waves a held station has all told in the `droids` probes —
/// the one aboard counted: `BIMS_DROID_WAVES=5`, over the commands' own
/// `screens::game::DROID_WAVES_IN_PROBE` (three). Nought reads as one,
/// since a station with no wave at all is nothing to look at.
pub fn droid_waves(default: u32) -> u32 {
    std::env::var("BIMS_DROID_WAVES")
        .ok()
        .and_then(|spec| spec.trim().parse::<u32>().ok())
        .unwrap_or(default)
        .max(1)
}

/// `BIMS_DROIDS=1` lays every state a machine can be drawn in out on
/// the arena's deck — a row a kind, a column a state — for one picture
/// of the lot (feature 83, `World::stage_droids_for_probe`).
pub fn droid_showcase() -> bool {
    std::env::var("BIMS_DROIDS").as_deref() == Ok("1")
}

/// What the window is called to the window system — its Wayland app id and
/// its X11 class: `bims`, and `bims-smoke` for a run with nobody at the
/// keyboard, so a desktop can be told where to put those and leave the
/// game alone. A smoke run is normally not on the desktop at all: `./hidden`
/// opens it on a headless compositor of its own.
pub fn window_name() -> String {
    if smoke_frames().is_some() {
        "bims-smoke".to_string()
    } else {
        "bims".to_string()
    }
}

/// `BIMS_FULLSCREEN=1` asks for the whole screen. A tiling desktop gives a
/// window whatever tile it likes and ignores the size it asked for, so this
/// is how a smoke run gets a picture at the size a player would actually
/// have.
pub fn window_mode() -> bevy::window::WindowMode {
    use bevy::window::{MonitorSelection, VideoModeSelection, WindowMode};
    match std::env::var("BIMS_FULLSCREEN").as_deref() {
        Ok("1") => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
        Ok("2") => WindowMode::Fullscreen(MonitorSelection::Current, VideoModeSelection::Current),
        _ => WindowMode::Windowed,
    }
}

/// A run with nobody at the keyboard does not wait for the screen. It is
/// meant to be looked at on `./hidden`'s headless compositor, whose output
/// has no real vblank — vsync there came out at six frames a second — so
/// the swapchain is asked not to wait, and [`smoke_exit`] paces the frames
/// to [`SMOKE_HZ`] itself, so a run behaves as it did on a real screen.
pub fn present_mode() -> bevy::window::PresentMode {
    use bevy::window::PresentMode;
    if smoke_frames().is_some() {
        PresentMode::AutoNoVsync
    } else {
        PresentMode::AutoVsync
    }
}

/// What a smoke run's frame rate is held to, in place of the screen's.
const SMOKE_HZ: f64 = 60.0;

pub fn window_resolution() -> WindowResolution {
    let default = WindowResolution::new(1400, 900);
    let Ok(spec) = std::env::var("BIMS_WINDOW") else {
        return default;
    };
    let Some((w, h)) = spec.split_once('x') else {
        return default;
    };
    match (w.parse::<u32>(), h.parse::<u32>()) {
        (Ok(w), Ok(h)) if w > 0 && h > 0 => WindowResolution::new(w, h),
        _ => default,
    }
}

pub struct DevPlugin;

impl Plugin for DevPlugin {
    fn build(&self, app: &mut App) {
        if smoke_frames().is_some() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::default())
                .add_systems(Update, smoke_exit);
        }
        if std::env::var("BIMS_POINTER").is_ok() || std::env::var("BIMS_KEYS").is_ok() {
            app.add_systems(Update, scripted_input);
        }
    }
}

fn smoke_exit(
    mut commands: Commands,
    frames: Res<FrameCount>,
    diagnostics: Res<DiagnosticsStore>,
    window: Single<&Window>,
    mut exit: MessageWriter<AppExit>,
    mut shot_taken: Local<bool>,
    mut last_frame: Local<Option<std::time::Instant>>,
) {
    let Some(limit) = smoke_frames() else { return };
    // Without vsync (`present_mode`) a frame is as quick as the machine
    // draws it, and the screens step by real time, so the run is held to
    // a screen's pace here: the rest of this frame's sixtieth is slept.
    // `BIMS_SMOKE_FREE=1` drops the pacing, so the frame time in the
    // line below is what the machine actually took rather than a
    // sixtieth: the one way to measure a heavy frame from a terminal.
    // Everything that steps by real time then runs as fast as it draws,
    // so a picture taken under it is not the picture a paced run gives.
    let paced = std::env::var("BIMS_SMOKE_FREE").is_err();
    let period = std::time::Duration::from_secs_f64(1.0 / SMOKE_HZ);
    if let Some(last) = (*last_frame).filter(|_| paced) {
        let due = last + period;
        let now = std::time::Instant::now();
        if due > now {
            std::thread::sleep(due - now);
        }
    }
    *last_frame = Some(std::time::Instant::now());
    // The first frames are Bevy coming up, the canvas being fitted and
    // the room being laid out: `BIMS_PERF` throws them away and starts
    // its timers again here, so what it prints is a frame of the game
    // running rather than a frame of it opening (feature 96).
    if crate::perf::wanted() && frames.0 == crate::perf::WARMUP {
        crate::perf::begin(frames.0);
    }
    if frames.0 + 30 >= limit
        && !*shot_taken
        && let Ok(path) = std::env::var("BIMS_SCREENSHOT")
    {
        *shot_taken = true;
        use bevy::render::view::screenshot::{Screenshot, save_to_disk};
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
    if frames.0 >= limit {
        // The frame time goes in the line because a screen that draws
        // slowly is a bug no picture shows, and the window size because the
        // desktop decides it and a scripted pointer is relative to it.
        let avg_ms = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
            .and_then(|d| d.average())
            .unwrap_or(f64::NAN);
        println!(
            "smoke: {} frames, {avg_ms:.1} ms a frame, window {}x{}",
            frames.0,
            window.width(),
            window.height()
        );
        for line in crate::perf::report(frames.0) {
            println!("{line}");
        }
        exit.write(AppExit::Success);
    }
}

/// Pointer input goes out as `WindowEvent`s, the stream bevy_egui reads,
/// *and* into `Window::cursor_position`, so a scripted click lands on an
/// egui widget or on the canvas exactly as a real one would.
fn scripted_input(
    frames: Res<FrameCount>,
    mut keyboard: MessageWriter<KeyboardInput>,
    mut window_events: MessageWriter<WindowEvent>,
    mut mouse: MessageWriter<MouseButtonInput>,
    mut wheels: MessageWriter<MouseWheel>,
    mut window: Single<(Entity, &mut Window)>,
) {
    let (window_entity, window) = &mut *window;
    let window_entity = *window_entity;

    let at_this_frame = |item: &str| -> Option<String> {
        let (at, rest) = item.split_once(':')?;
        (at.trim().parse::<u32>() == Ok(frames.0)).then(|| rest.trim().to_string())
    };

    for item in std::env::var("BIMS_KEYS").unwrap_or_default().split(',') {
        let Some(name) = at_this_frame(item) else {
            continue;
        };
        // `+Name` is the press alone, held from here; `-Name` the release.
        let (name, states): (&str, &[ButtonState]) = match name.as_bytes().first() {
            Some(b'+') => (&name[1..], &[ButtonState::Pressed]),
            Some(b'-') => (&name[1..], &[ButtonState::Released]),
            _ => (
                name.as_str(),
                &[ButtonState::Pressed, ButtonState::Released],
            ),
        };
        let (key_code, logical) = match name {
            "Escape" | "Esc" => (KeyCode::Escape, Key::Escape),
            "Shift" => (KeyCode::ShiftLeft, Key::Shift),
            "Enter" => (KeyCode::Enter, Key::Enter),
            "Tab" => (KeyCode::Tab, Key::Tab),
            "Space" => (KeyCode::Space, Key::Space),
            "1" => (KeyCode::Digit1, Key::Character("1".into())),
            "2" => (KeyCode::Digit2, Key::Character("2".into())),
            "3" => (KeyCode::Digit3, Key::Character("3".into())),
            "4" => (KeyCode::Digit4, Key::Character("4".into())),
            "5" => (KeyCode::Digit5, Key::Character("5".into())),
            other if other.len() == 1 => {
                let ch = other.chars().next().unwrap().to_ascii_lowercase();
                let code = match ch {
                    'a' => KeyCode::KeyA,
                    'c' => KeyCode::KeyC,
                    'd' => KeyCode::KeyD,
                    'e' => KeyCode::KeyE,
                    'f' => KeyCode::KeyF,
                    'm' => KeyCode::KeyM,
                    'n' => KeyCode::KeyN,
                    'q' => KeyCode::KeyQ,
                    'r' => KeyCode::KeyR,
                    's' => KeyCode::KeyS,
                    // The two orders every player has (feature 84): T is
                    // the retreat, and V is the camera's Follow that
                    // Attack's F pushed off it.
                    't' => KeyCode::KeyT,
                    'v' => KeyCode::KeyV,
                    'w' => KeyCode::KeyW,
                    // The commander's two squad keys (feature 78).
                    'x' => KeyCode::KeyX,
                    'z' => KeyCode::KeyZ,
                    _ => continue,
                };
                (code, Key::Character(ch.to_string().into()))
            }
            _ => continue,
        };
        for &state in states {
            let input = KeyboardInput {
                key_code,
                logical_key: logical.clone(),
                state,
                text: None,
                repeat: false,
                window: window_entity,
            };
            keyboard.write(input.clone());
            window_events.write(WindowEvent::KeyboardInput(input));
        }
    }

    for item in std::env::var("BIMS_POINTER").unwrap_or_default().split(';') {
        let Some(spec) = at_this_frame(item) else {
            continue;
        };
        let Some((what, xy)) = spec.split_once(':') else {
            continue;
        };
        let Some((x, y)) = xy.split_once(',') else {
            continue;
        };
        let (Ok(x), Ok(y)) = (x.trim().parse::<f32>(), y.trim().parse::<f32>()) else {
            continue;
        };
        let position = Vec2::new(x, y);
        // Wayland refuses to move a cursor it does not hold; the event is
        // what egui reads, so the refusal is nothing to report.
        window.set_cursor_position(Some(position));
        window_events.write(WindowEvent::CursorMoved(CursorMoved {
            window: window_entity,
            position,
            delta: None,
        }));
        // `press`/`release` are the two halves of a click on their own,
        // for a drag: press at one frame, move at the next few, release.
        const CLICK: &[ButtonState] = &[ButtonState::Pressed, ButtonState::Released];
        // `wheel` is one notch of the wheel down at that point, which zooms
        // the canvas out; `wheelup` a notch up.
        if let Some(notch) = match what.trim() {
            "wheel" => Some(-1.0),
            "wheelup" => Some(1.0),
            _ => None,
        } {
            let wheel = MouseWheel {
                unit: MouseScrollUnit::Line,
                x: 0.0,
                y: notch,
                window: window_entity,
                phase: bevy::input::touch::TouchPhase::Moved,
            };
            wheels.write(wheel);
            window_events.write(WindowEvent::MouseWheel(wheel));
            continue;
        }
        let (button, states): (MouseButton, &[ButtonState]) = match what.trim() {
            "move" => continue,
            "click" => (MouseButton::Left, CLICK),
            "right" => (MouseButton::Right, CLICK),
            "middle" => (MouseButton::Middle, CLICK),
            "press" => (MouseButton::Left, &[ButtonState::Pressed]),
            "release" => (MouseButton::Left, &[ButtonState::Released]),
            "rpress" => (MouseButton::Right, &[ButtonState::Pressed]),
            "rrelease" => (MouseButton::Right, &[ButtonState::Released]),
            _ => continue,
        };
        // Both streams: Bevy's own input reads the typed message, and
        // bevy_egui reads the ordered `WindowEvent` batch.
        for &state in states {
            let input = MouseButtonInput {
                button,
                state,
                window: window_entity,
            };
            mouse.write(input);
            window_events.write(WindowEvent::MouseButtonInput(input));
        }
    }
}
