//! Sound: what the room's cues and the world's events are played as.
//!
//! The room says what happened as a [`Cue`] — a door starting to slide, a
//! knife on the board, a shot, a bolt landing — and the world as a
//! `WorldEvent`; neither has a recording in it, the way neither has a word.
//! This module is where the recordings are, the way `names.rs` is where
//! the words are: one clip per thing, embedded in the binary from
//! `Sounds/game/` (which `crates/app/sounds/prepare.sh` cuts from the
//! recordings in `Sounds/` and `abilities.py` beside it synthesises, and
//! the player may since have edited by hand), and a table that says which
//! clip a cue is, and how loud.
//!
//! Two kinds of thing are played. A **one-shot** is spawned, plays once
//! and despawns itself; a **bed** — the ship's hum, a station's, a
//! planet's air — is a loop that runs the whole time at
//! whatever level the screen asks for, faded rather than switched. A screen asks every
//! frame, and a bed nobody asks for fades out, so a screen that closes
//! takes its sound with it without having to say so.
//!
//! Cues arrive a step's worth at a time, and at the world's top speed a
//! frame is many steps: a door's whole open-and-shut in one frame. Each
//! kind of cue has a cool-down in real seconds, **per place**, and what
//! falls inside it is dropped — the same door twenty-four times in a frame
//! is one slide, but two guns firing in one frame are two shots, and two
//! doors opening at once are two doors. The room says everything, and
//! this is where it is thinned. A frame has a ceiling on one-shots
//! besides, for a fight at top speed.

use bevy::audio::{
    AudioPlayer, AudioSink, AudioSinkPlayback, AudioSource, PlaybackSettings, Volume,
};
use bevy::prelude::*;
use bims::combat::WeaponKind;
use bims::cue::{Cue, Cued};

/// Every recording the app has, by name. The order is the order of
/// [`CLIPS`], and nothing else depends on it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
pub enum Clip {
    Laser1,
    Laser2,
    Laser3,
    Laser4,
    Laser5,
    Laser6,
    Laser7,
    Laser8,
    Laser9,
    Shotgun,
    Rifle,
    Sniper,
    LaserHit,
    LaserWall,
    Schword,
    Punch,
    Ouch,
    DoorOpen,
    DoorClose,
    DoorForce,
    Ship,
    Station,
    Draw,
    Holster,
    Temperate,
    Desert,
    Arctic,
    Bought,
    Sold,
    // The classes' abilities, one clip an ability (built by
    // `sounds/abilities.py`, not cut from a recording).
    GrenadeThrow,
    GrenadeBurst,
    Brace,
    Rampage,
    EmpThrow,
    EmpBurst,
    HealingSentry,
    Sandbags,
    Sentry,
    /// The old Nanite Burst's hiss: a Heal Drone lifting off (task 153)
    /// and a *Field Mender*.
    NaniteBurst,
    BeamOn,
    BeamOff,
    /// The old cloak's shimmer: a Healing Circle coming up (task 153)
    /// and a *Blink Drive*.
    Cloak,
    Taunt,
    BulwarkOn,
    BulwarkOff,
    Juggernaut,
    BattleCry,
    Rally,
    Reinforcements,
    // An enemy down and what it was worth: a soft two-note chime,
    // synthesised (`prepare.sh`'s last line).
    Reward,
    // A crew member downed: a cry, the body on the deck and the suit's
    // vitals alarm, built by `sounds/abilities.py` (`downed`).
    Downed,
    // A crew member brought round: the defibrillator, the heart and
    // the vitals rising, built by `sounds/abilities.py` (`revived`).
    Revived,
    // A magazine reloaded (October 2026): a gun's, which every gun but the
    // shotgun plays, and the shotgun's shells put in one by one.
    Reload,
    ShotgunReload,
    // The laser laid over each (`sounds/abilities.py`, `reload_laser` and
    // `shotgun_reload_laser`), so a reload is a laser cell's and not only
    // a magazine's clicks: played with it, at once.
    ReloadLaser,
    ShotgunReloadLaser,
    // A level gained (October 2026): a run of bells up into a ringing
    // chord, after Dota 2's (`sounds/abilities.py`, `level_up`).
    LevelUp,
    // The tier-two machines' telegraphs (task 157, `sounds/abilities.py`):
    // a bomb rolling and spinning up, a Lancer's rail charging and locking,
    // a Conductor's mark acquiring, and its blink.
    BombArmed,
    RailCharge,
    Marked,
    Blink,
    // The Smoke Launcher (October 2026, the player's recordings): the
    // canister going off, the cloud's steady hiss (a loop, the smoke bed)
    // and its going out.
    SmokeBang,
    SmokeRun,
    SmokeOut,
}

/// The bytes of each clip, indexed by [`Clip`]. Ogg Vorbis, mono, 48 kHz,
/// peaks at -1 dBFS for the one-shots and -22 or -30 LUFS for the loops —
/// see `prepare.sh` — so every level below is relative to that. They live
/// in `Sounds/game/` at the root, beside the recordings, where the player
/// edits them by hand (its `README.md`).
const CLIPS: [&[u8]; 64] = [
    include_bytes!("../../../Sounds/game/laser_1.ogg"),
    include_bytes!("../../../Sounds/game/laser_2.ogg"),
    include_bytes!("../../../Sounds/game/laser_3.ogg"),
    include_bytes!("../../../Sounds/game/laser_4.ogg"),
    include_bytes!("../../../Sounds/game/laser_5.ogg"),
    include_bytes!("../../../Sounds/game/laser_6.ogg"),
    include_bytes!("../../../Sounds/game/laser_7.ogg"),
    include_bytes!("../../../Sounds/game/laser_8.ogg"),
    include_bytes!("../../../Sounds/game/laser_9.ogg"),
    include_bytes!("../../../Sounds/game/shotgun.ogg"),
    include_bytes!("../../../Sounds/game/rifle.ogg"),
    include_bytes!("../../../Sounds/game/sniper.ogg"),
    include_bytes!("../../../Sounds/game/laser_hit.ogg"),
    include_bytes!("../../../Sounds/game/laser_wall.ogg"),
    include_bytes!("../../../Sounds/game/schword.ogg"),
    include_bytes!("../../../Sounds/game/punch.ogg"),
    include_bytes!("../../../Sounds/game/ouch.ogg"),
    include_bytes!("../../../Sounds/game/door_open.ogg"),
    include_bytes!("../../../Sounds/game/door_close.ogg"),
    include_bytes!("../../../Sounds/game/door_force.ogg"),
    include_bytes!("../../../Sounds/game/ship.ogg"),
    include_bytes!("../../../Sounds/game/station.ogg"),
    include_bytes!("../../../Sounds/game/draw.ogg"),
    include_bytes!("../../../Sounds/game/holster.ogg"),
    include_bytes!("../../../Sounds/game/temperate.ogg"),
    include_bytes!("../../../Sounds/game/desert.ogg"),
    include_bytes!("../../../Sounds/game/arctic.ogg"),
    include_bytes!("../../../Sounds/game/bought.ogg"),
    include_bytes!("../../../Sounds/game/sold.ogg"),
    include_bytes!("../../../Sounds/game/grenade_throw.ogg"),
    include_bytes!("../../../Sounds/game/grenade_burst.ogg"),
    include_bytes!("../../../Sounds/game/brace.ogg"),
    include_bytes!("../../../Sounds/game/rampage.ogg"),
    include_bytes!("../../../Sounds/game/emp_throw.ogg"),
    include_bytes!("../../../Sounds/game/emp_burst.ogg"),
    include_bytes!("../../../Sounds/game/healing_sentry.ogg"),
    include_bytes!("../../../Sounds/game/sandbags.ogg"),
    include_bytes!("../../../Sounds/game/sentry.ogg"),
    include_bytes!("../../../Sounds/game/nanite_burst.ogg"),
    include_bytes!("../../../Sounds/game/beam_on.ogg"),
    include_bytes!("../../../Sounds/game/beam_off.ogg"),
    include_bytes!("../../../Sounds/game/cloak.ogg"),
    include_bytes!("../../../Sounds/game/taunt.ogg"),
    include_bytes!("../../../Sounds/game/bulwark_on.ogg"),
    include_bytes!("../../../Sounds/game/bulwark_off.ogg"),
    include_bytes!("../../../Sounds/game/juggernaut.ogg"),
    include_bytes!("../../../Sounds/game/battle_cry.ogg"),
    include_bytes!("../../../Sounds/game/rally.ogg"),
    include_bytes!("../../../Sounds/game/reinforcements.ogg"),
    include_bytes!("../../../Sounds/game/reward.ogg"),
    include_bytes!("../../../Sounds/game/downed.ogg"),
    include_bytes!("../../../Sounds/game/revived.ogg"),
    include_bytes!("../../../Sounds/game/reload.ogg"),
    include_bytes!("../../../Sounds/game/shotgun_reload.ogg"),
    include_bytes!("../../../Sounds/game/reload_laser.ogg"),
    include_bytes!("../../../Sounds/game/shotgun_reload_laser.ogg"),
    include_bytes!("../../../Sounds/game/level_up.ogg"),
    include_bytes!("../../../Sounds/game/bomb_armed.ogg"),
    include_bytes!("../../../Sounds/game/rail_charge.ogg"),
    include_bytes!("../../../Sounds/game/marked.ogg"),
    include_bytes!("../../../Sounds/game/blink.ogg"),
    include_bytes!("../../../Sounds/game/smoke_bang.ogg"),
    include_bytes!("../../../Sounds/game/smoke_run.ogg"),
    include_bytes!("../../../Sounds/game/smoke_out.ogg"),
];

/// The player's own volume for each sound, from `audio.ron` at the root
/// (`BIMS_AUDIO` names another; `wavecfg.rs` reads it again whenever it is
/// saved). Each is a multiple of the level the tables here play it at — 1
/// is as built, 0 is never heard, 2 twice as loud — one a clip, named as
/// its `.ogg` is, and one for each weapon that borrows another's report,
/// so a minigun can be turned down without the rifle. A sound left out of
/// the file is 1.
macro_rules! volumes {
    ($($clip:ident => $key:ident,)* ; $($borrower:ident,)*) => {
        #[derive(Clone, Copy, PartialEq, Debug, serde::Deserialize)]
        #[serde(default, deny_unknown_fields)]
        pub struct Volumes {
            $(pub $key: f32,)*
            $(pub $borrower: f32,)*
        }

        impl Volumes {
            pub const AS_BUILT: Volumes = Volumes {
                $($key: 1.0,)*
                $($borrower: 1.0,)*
            };

            /// Every sound's name in the file.
            pub const NAMES: &[&str] = &[$(stringify!($key),)* $(stringify!($borrower),)*];

            /// The sounds the player turned from 1, by name.
            pub fn turned(&self) -> impl Iterator<Item = (&'static str, f32)> {
                Volumes::NAMES
                    .iter()
                    .copied()
                    .zip([$(self.$key,)* $(self.$borrower,)*])
                    .filter(|&(_, v)| v != 1.0)
            }

            /// The player's multiple for a clip.
            fn of(&self, clip: Clip) -> f32 {
                match clip {
                    $(Clip::$clip => self.$key,)*
                }
            }
        }
    };
}

volumes! {
    Laser1 => laser_1,
    Laser2 => laser_2,
    Laser3 => laser_3,
    Laser4 => laser_4,
    Laser5 => laser_5,
    Laser6 => laser_6,
    Laser7 => laser_7,
    Laser8 => laser_8,
    Laser9 => laser_9,
    Shotgun => shotgun,
    Rifle => rifle,
    Sniper => sniper,
    LaserHit => laser_hit,
    LaserWall => laser_wall,
    Schword => schword,
    Punch => punch,
    Ouch => ouch,
    DoorOpen => door_open,
    DoorClose => door_close,
    DoorForce => door_force,
    Ship => ship,
    Station => station,
    Draw => draw,
    Holster => holster,
    Temperate => temperate,
    Desert => desert,
    Arctic => arctic,
    Bought => bought,
    Sold => sold,
    GrenadeThrow => grenade_throw,
    GrenadeBurst => grenade_burst,
    Brace => brace,
    Rampage => rampage,
    EmpThrow => emp_throw,
    EmpBurst => emp_burst,
    HealingSentry => healing_sentry,
    Sandbags => sandbags,
    Sentry => sentry,
    NaniteBurst => nanite_burst,
    BeamOn => beam_on,
    BeamOff => beam_off,
    Cloak => cloak,
    Taunt => taunt,
    BulwarkOn => bulwark_on,
    BulwarkOff => bulwark_off,
    Juggernaut => juggernaut,
    BattleCry => battle_cry,
    Rally => rally,
    Reinforcements => reinforcements,
    Reward => reward,
    Downed => downed,
    Revived => revived,
    Reload => reload,
    ShotgunReload => shotgun_reload,
    ReloadLaser => reload_laser,
    ShotgunReloadLaser => shotgun_reload_laser,
    LevelUp => level_up,
    BombArmed => bomb_armed,
    RailCharge => rail_charge,
    Marked => marked,
    Blink => blink,
    SmokeBang => smoke_bang,
    SmokeRun => smoke_run,
    SmokeOut => smoke_out,
    ;
    minigun,
    rail_lance,
    unmaker,
}

impl Default for Volumes {
    fn default() -> Self {
        Volumes::AS_BUILT
    }
}

/// How long a smoke cloud's going out lasts, in seconds: `smoke_out.ogg`,
/// started that long before the cloud is gone.
pub const SMOKE_OUT_SECONDS: f64 = 2.0;

/// The loops that run the whole time. The order is the order of the
/// `beds` array on [`Sounds`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
pub enum Bed {
    /// The ship's own hum: the deck, anywhere but tied up at a station.
    Ship,
    /// A station's, while docked with the rooms joined.
    Station,
    /// A planet's air, set down at its settlement with the ground on the
    /// joined deck: one a biome — birds and leaves, a dry wind, a cold one.
    Temperate,
    Desert,
    Arctic,
    /// A Smoke Launcher's cloud hissing while any hangs (October 2026):
    /// an effect, under the effects volume, not the ambience's.
    Smoke,
}

impl Bed {
    const ALL: [Bed; 6] = [
        Bed::Ship,
        Bed::Station,
        Bed::Temperate,
        Bed::Desert,
        Bed::Arctic,
        Bed::Smoke,
    ];

    /// The bed a planet's ground is heard as.
    pub fn of_biome(biome: world::Biome) -> Bed {
        match biome {
            world::Biome::Temperate => Bed::Temperate,
            world::Biome::Desert => Bed::Desert,
            world::Biome::Arctic => Bed::Arctic,
        }
    }

    fn clip(self) -> Clip {
        match self {
            Bed::Ship => Clip::Ship,
            Bed::Station => Clip::Station,
            Bed::Temperate => Clip::Temperate,
            Bed::Desert => Clip::Desert,
            Bed::Arctic => Clip::Arctic,
            Bed::Smoke => Clip::SmokeRun,
        }
    }

    /// The level the bed plays at when fully up. The ambiences are
    /// already fifteen dB under the recordings in the file; this is on
    /// top, so they sit under a door two rooms away. A planet's air is
    /// levelled the same in the file and played at a quarter of the hum's,
    /// since it is heard the whole time the crew are ashore.
    fn level(self) -> f32 {
        match self {
            Bed::Ship | Bed::Station => 0.7,
            Bed::Temperate | Bed::Desert | Bed::Arctic => 0.175,
            Bed::Smoke => 0.55,
        }
    }

    /// How fast it fades, in fractions of full a second. A station's hum
    /// comes up over a couple of seconds as the airlocks mate; a planet's
    /// air comes in with the ground, at the station's pace.
    fn rate(self) -> f32 {
        match self {
            Bed::Ship | Bed::Station => 0.5,
            Bed::Temperate | Bed::Desert | Bed::Arctic => 0.5,
            // The hiss comes in under the bang and gives way to the going
            // out in a quarter of a second.
            Bed::Smoke => 4.0,
        }
    }
}

/// The kinds of cue, for the cool-downs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Door,
    /// A body heaving at a locked door, and the door giving.
    Smash,
    Shot,
    /// A minigun's bolt (task 115): heard every second one, see
    /// [`Kind::cool_down`].
    Minigun,
    Impact,
    Ricochet,
    Blow,
    /// A weapon out of its holster, or back into it.
    Holster,
    /// A grenade going off (feature 75).
    Burst,
    /// Something bought off the trader, by anyone in the crew.
    Bought,
    /// Something sold to the trader, by anyone in the crew.
    Sold,
    /// A class's ability used, told apart by clip and by who used it.
    Ability,
    /// An enemy down and its pay floating up over it.
    Reward,
    /// A crew member downed, told apart by who.
    Downed,
    /// A crew member brought round, told apart by who.
    Revived,
    /// A magazine being reloaded (October 2026).
    Reload,
    /// A level gained, told apart by who.
    LevelUp,
}

/// How many one-shots a frame may start, whatever the room says. Enough
/// for a fight; a fight at top speed is a fight heard at real speed.
const SHOTS_PER_FRAME: u32 = 12;

/// What a shot's level is multiplied by when it is anybody's but the
/// player's own Bim's — a bot's, another player's, a sentry's, an
/// enemy's: thirty per cent quieter, so the player's own gun stands out.
const OTHERS_SHOTS: f32 = 0.7;

/// Places are told apart this coarsely, in room units — two tiles — so a
/// gunner walking between the steps of one frame is one place.
const PLACE: f32 = 2.0 * bims::room::TILE;

/// How far a shot's pitch strays from its recording's, either way: each
/// shot is played this much faster or slower at most (a speed of 0.95 to
/// 1.05, under a semitone), so a burst of one gun is not the one sample
/// eight times over.
const SHOT_PITCH: f32 = 0.05;

impl Kind {
    fn of(cue: Cue) -> Kind {
        match cue {
            Cue::DoorOpens | Cue::DoorShuts => Kind::Door,
            Cue::DoorSmash | Cue::DoorForced => Kind::Smash,
            Cue::Shot {
                weapon: WeaponKind::Minigun,
                ..
            } => Kind::Minigun,
            Cue::Shot { .. } => Kind::Shot,
            Cue::Impact { .. } => Kind::Impact,
            Cue::Ricochet | Cue::Shielded => Kind::Ricochet,
            Cue::Blow { .. } => Kind::Blow,
            Cue::Holster { .. } => Kind::Holster,
            Cue::Throw => Kind::Blow,
            Cue::Burst | Cue::EmpBurst => Kind::Burst,
            Cue::Reload { .. } => Kind::Reload,
            // The tier-two machines' telegraphs (task 157).
            Cue::BombArmed | Cue::RailCharge | Cue::Marked | Cue::Blink => Kind::Ability,
        }
    }

    /// How long after one of these before another is played, in real
    /// seconds. Shorter than the clip for the things that overlap in
    /// earnest — a burst is eight shots a quarter-second apart — and about
    /// the clip's length for the rest, so that twenty-four steps of one
    /// door sliding in one frame is one slide.
    fn cool_down(self) -> f64 {
        match self {
            Kind::Door => 0.2,
            // A heave every two seconds a door; a frame at 24x holds
            // several, which is one.
            Kind::Smash => 0.5,
            Kind::Shot => 0.04,
            // Ten a second from one gun, and the rifle's 0.32-second report
            // under each: every bolt played stacks to 4.4 dB over a single
            // report where the rifle's own burst is 0.9, and every second
            // one to 1.8 (task 115, measured by mixing the clip). So one
            // in two is heard — longer than the tenth between two bolts,
            // shorter than the fifth between every other. The picture and
            // the hits are every bolt's.
            Kind::Minigun => 0.15,
            Kind::Impact => 0.06,
            Kind::Ricochet => 0.1,
            Kind::Blow => 0.1,
            // A hand changes once a step at most; the clip is half a second.
            Kind::Holster => 0.3,
            // A grenade bursts once; two in a frame are two.
            Kind::Burst => 0.05,
            // Two things bought in one frame (a click and a guest's
            // order landing together) are one till ringing, two a moment
            // apart are two.
            Kind::Bought => 0.3,
            // And the same for a sale.
            Kind::Sold => 0.3,
            // One body's same ability twice in a quarter-second is the
            // world saying one use over several steps of a fast frame.
            Kind::Ability => 0.25,
            // A whole wave falling in one frame is one chime.
            Kind::Reward => 0.12,
            // A body goes down once; the room says it the step it does.
            Kind::Downed => 1.0,
            // And comes round once.
            Kind::Revived => 1.0,
            // A reload begins once a magazine; two guns at one place in a
            // frame are one clip.
            Kind::Reload => 0.3,
            // A level comes once; two at once (a big kill's experience) are
            // one ring.
            Kind::LevelUp => 1.0,
        }
    }
}

/// A bed's entity and where its level is going.
struct Loop {
    entity: Entity,
    /// What was asked for this frame: nought unless a screen said.
    wanted: f32,
    /// Where the fade has got to.
    level: f32,
}

#[derive(Resource)]
pub struct Sounds {
    clips: Vec<Handle<AudioSource>>,
    beds: Vec<Loop>,
    /// Real seconds since the app opened, advanced once a frame.
    clock: f64,
    /// What was lately played, where, and when it may be played there
    /// again: a kind and a place (in [`PLACE`] cells), and the clock
    /// reading its cool-down ends at. Entries past that are dropped as
    /// they are met, so the list is as long as the last frame was loud.
    recent: Vec<(Kind, (i32, i32), f64)>,
    /// One-shots started this frame, against [`SHOTS_PER_FRAME`].
    started: u32,
    /// Which take of a clip with several was played last, so the next
    /// is another.
    turn: u32,
    /// A plain generator for the shots' takes and pitch — the app's
    /// alone, nothing of the world's.
    wobble: u32,
    /// `BIMS_SOUND_LOG=1`: say what is played.
    log: bool,
    /// The smoke clouds whose going out has been played, by the key the
    /// screen gives each (`Sounds::smoke`).
    smoke_out: Vec<u64>,
    /// What the player set on the Esc sheet's audio page.
    pub mix: Mix,
    /// And in `audio.ron`, a sound at a time.
    pub volumes: Volumes,
}

/// The player's volumes, nought to one each, from the Esc sheet: one over
/// everything, one over the one-shots and one over the beds, and a mute
/// that keeps the three where they were. Everything else about a level is
/// the tables above; this is the only part the player turns.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Mix {
    pub master: f32,
    pub effects: f32,
    pub ambience: f32,
    pub muted: bool,
}

impl Default for Mix {
    fn default() -> Self {
        Mix {
            master: 0.8,
            effects: 1.0,
            ambience: 1.0,
            muted: false,
        }
    }
}

impl Mix {
    fn master(&self) -> f32 {
        if self.muted { 0.0 } else { self.master }
    }

    /// What a one-shot's level is multiplied by.
    fn effects(&self) -> f32 {
        self.master() * self.effects
    }

    /// And a bed's.
    fn ambience(&self) -> f32 {
        self.master() * self.ambience
    }
}

impl Sounds {
    /// The clips, added to the asset store from the bytes compiled in —
    /// no loader, no path, nothing read at run time — and the beds
    /// spawned silent and looping.
    fn open(mut assets: ResMut<Assets<AudioSource>>, mut commands: Commands) {
        let clips: Vec<Handle<AudioSource>> = CLIPS
            .iter()
            .map(|bytes| {
                assets.add(AudioSource {
                    bytes: (*bytes).into(),
                })
            })
            .collect();
        let beds = Bed::ALL
            .iter()
            .map(|bed| Loop {
                entity: commands
                    .spawn((
                        AudioPlayer::new(clips[bed.clip() as usize].clone()),
                        PlaybackSettings::LOOP.with_volume(Volume::SILENT),
                    ))
                    .id(),
                wanted: 0.0,
                level: 0.0,
            })
            .collect();
        commands.insert_resource(Sounds {
            clips,
            beds,
            clock: 0.0,
            recent: Vec::new(),
            started: 0,
            turn: 0,
            wobble: 0x9e37_79b9,
            smoke_out: Vec::new(),
            log: crate::dev::sound_log(),
            mix: Mix {
                muted: crate::dev::silent(),
                ..Mix::default()
            },
            volumes: Volumes::AS_BUILT,
        });
    }

    /// One clip, once, at `level` of its own under the player's volume
    /// for it and the effects volume — spawned to play and despawn itself
    /// when it has. Nothing is spawned while muted: a one-shot's volume is
    /// set when it starts, and a mute lifted a moment later would not
    /// reach it.
    fn one_shot(&self, commands: &mut Commands, clip: Clip, level: f32) {
        self.one_shot_as(commands, clip, level, self.volumes.of(clip));
    }

    /// The same under a volume of the player's given outright: a weapon
    /// that borrows another's clip is turned up and down by its own.
    fn one_shot_as(&self, commands: &mut Commands, clip: Clip, level: f32, volume: f32) {
        self.one_shot_at(commands, clip, level, volume, 1.0);
    }

    /// The same at `speed` times the recording's, which raises or lowers
    /// its pitch with it.
    fn one_shot_at(
        &self,
        commands: &mut Commands,
        clip: Clip,
        level: f32,
        volume: f32,
        speed: f32,
    ) {
        let level = level * volume.max(0.0);
        if self.log {
            println!("sound: {clip:?} at {level:.2} x{speed:.3}");
        }
        let level = level * self.mix.effects();
        if level <= 0.0 {
            return;
        }
        commands.spawn((
            AudioPlayer::new(self.clips[clip as usize].clone()),
            PlaybackSettings::DESPAWN
                .with_volume(Volume::Linear(level))
                .with_speed(speed),
        ));
    }

    /// Whether a `kind` may be played now at `at`, and if so, not again
    /// there until its cool-down is up.
    fn admit(&mut self, kind: Kind, at: bims::math::Vec2) -> bool {
        let cell = ((at.x / PLACE).floor() as i32, (at.y / PLACE).floor() as i32);
        self.admit_in(kind, cell)
    }

    /// The same by a cell given outright, for what has no place in the
    /// room.
    fn admit_in(&mut self, kind: Kind, cell: (i32, i32)) -> bool {
        if self.started >= SHOTS_PER_FRAME {
            return false;
        }
        let now = self.clock;
        self.recent.retain(|&(_, _, until)| until > now);
        if self.recent.iter().any(|&(k, c, _)| k == kind && c == cell) {
            return false;
        }
        self.recent.push((kind, cell, now + kind.cool_down()));
        self.started += 1;
        true
    }

    /// The next of [`Sounds::wobble`]'s numbers, 24 bits of it.
    fn roll(&mut self) -> u32 {
        // xorshift32: never nought, from a seed that is not.
        let mut x = self.wobble;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.wobble = x;
        x >> 8
    }

    /// A shot's speed: one, give or take [`SHOT_PITCH`], fresh each time.
    fn shot_speed(&mut self) -> f32 {
        let unit = self.roll() as f32 / (1u32 << 24) as f32;
        1.0 + SHOT_PITCH * (2.0 * unit - 1.0)
    }

    /// One of `n` takes at random, never the one played last, so a
    /// burst never plays one sample twice running.
    fn take(&mut self, n: u32) -> u32 {
        if n < 2 {
            return 0;
        }
        // One of the n - 1 others, stepping over the last.
        let pick = self.roll() % (n - 1);
        let pick = if pick >= self.turn { pick + 1 } else { pick };
        self.turn = pick;
        pick
    }

    /// A room's cue, played — or dropped, inside its kind's cool-down.
    /// `own` is the player's own Bim in that room, by index, if it is
    /// there: its shots are played louder than anybody else's.
    pub fn play(&mut self, commands: &mut Commands, cued: Cued, own: Option<usize>) {
        let Cued { cue, at } = cued;
        // A bot's hand, or an enemy's, is not heard changing: the room
        // says every one, and only a player's own is played — before the
        // cool-down, so a silent one does not hold a heard one off.
        if let Cue::Holster { player: false, .. } = cue {
            return;
        }
        if !self.admit(Kind::of(cue), at) {
            return;
        }
        match cue {
            Cue::Holster { drawn: true, .. } => self.one_shot(commands, Clip::Draw, 0.5),
            Cue::Holster { drawn: false, .. } => self.one_shot(commands, Clip::Holster, 0.45),
            Cue::DoorOpens => self.one_shot(commands, Clip::DoorOpen, 0.04),
            Cue::DoorShuts => self.one_shot(commands, Clip::DoorClose, 0.04),
            // The forcing: the recording of a door being forced, a heave
            // at a time, and louder the once it gives.
            Cue::DoorSmash => self.one_shot(commands, Clip::DoorForce, 0.55),
            Cue::DoorForced => self.one_shot(commands, Clip::DoorForce, 0.9),
            Cue::Shot {
                weapon,
                hostile,
                by,
            } => {
                // An enemy's shot a shade quieter: it is the crew's fight
                // the player is listening to. And anybody's but the
                // player's own a good deal quieter again, so the gun in
                // the player's hands is the one heard over the rest.
                let theirs = if hostile { 0.8 } else { 1.0 };
                let others = if by.is_some() && by == own {
                    1.0
                } else {
                    OTHERS_SHOTS
                };
                let theirs = theirs * others;
                let v = self.volumes;
                let (clip, level, volume) = match weapon {
                    WeaponKind::LaserPistol => {
                        // Nine takes, picked at random: the four
                        // recorded, and five of them a semitone or two
                        // higher (`laser_5` … `laser_9`).
                        let takes = [
                            Clip::Laser1,
                            Clip::Laser2,
                            Clip::Laser3,
                            Clip::Laser4,
                            Clip::Laser5,
                            Clip::Laser6,
                            Clip::Laser7,
                            Clip::Laser8,
                            Clip::Laser9,
                        ];
                        let clip = takes[self.take(takes.len() as u32) as usize];
                        (clip, 0.5, v.of(clip))
                    }
                    WeaponKind::Shotgun => (Clip::Shotgun, 0.7, v.shotgun),
                    WeaponKind::AutoRifle => (Clip::Rifle, 0.35, v.rifle),
                    WeaponKind::SniperRifle => (Clip::Sniper, 0.6, v.sniper),
                    // No recordings of their own (task 115): the minigun
                    // borrows the rifle's report, quiet, and the lance the
                    // sniper's, loud.
                    WeaponKind::Minigun => (Clip::Rifle, 0.2, v.minigun),
                    WeaponKind::RailLance => (Clip::Sniper, 0.7, v.rail_lance),
                    // The Lancer's rail (task 157) is the lance's, a shade louder.
                    WeaponKind::Rail => (Clip::Sniper, 0.75, v.rail_lance),
                    // A blade is never fired, nor is a claw; the room
                    // does not say either is.
                    // Sounds are not in feature 100: the Guardian's beam is
                    // silent until the droids have their own recordings.
                    WeaponKind::Schword | WeaponKind::Claw | WeaponKind::Sweeper => return,
                    // The Unmaker has no recording of its own yet —
                    // droid sounds are their own step — so it borrows
                    // the sniper's report, which is the nearest thing
                    // to a heavy single shot the box holds.
                    WeaponKind::Unmaker => (Clip::Sniper, 0.6, v.unmaker),
                };
                let speed = self.shot_speed();
                self.one_shot_at(commands, clip, level * theirs, volume, speed);
            }
            // A hit is heard under the shot that made it, never over it:
            // every level of a hit here and on a blow was halved in
            // October 2026, the player finding them far too loud.
            Cue::Impact { on_crew } => {
                if on_crew {
                    self.one_shot(commands, Clip::LaserHit, 0.2);
                    self.one_shot(commands, Clip::Ouch, 0.3);
                } else {
                    self.one_shot(commands, Clip::LaserHit, 0.25);
                }
            }
            // A bolt on a Guardian's shield has no recording of its own yet
            // (sounds are not in feature 100): it borrows the wall's.
            Cue::Ricochet | Cue::Shielded => self.one_shot(commands, Clip::LaserWall, 0.18),
            // The throw is heard from the world's `Thrown` and
            // `SatchelThrown` (see [`Sounds::ability`]), which tell a
            // grenade from a satchel; the room says both as one `Throw`.
            Cue::Throw => {}
            Cue::Burst => self.one_shot(commands, Clip::GrenadeBurst, 0.9),
            Cue::EmpBurst => self.one_shot(commands, Clip::EmpBurst, 0.6),
            // The tier-two machines (task 157): each clip as long as what it
            // warns of — the fuse, the charge, the mark's warning — so it
            // ends on the thing itself.
            Cue::BombArmed => self.one_shot(commands, Clip::BombArmed, 0.5),
            Cue::RailCharge => self.one_shot(commands, Clip::RailCharge, 0.45),
            Cue::Marked => self.one_shot(commands, Clip::Marked, 0.5),
            Cue::Blink => self.one_shot(commands, Clip::Blink, 0.5),
            // A reload, the player's own over the rest (`OTHERS_SHOTS` and a
            // half again): a crew of bots reloading round a fight is a
            // murmur under it.
            // The laser layer goes with it, under the clicks: the shotgun's
            // recording is far the quieter, so its layer is turned lower.
            Cue::Reload { weapon, by } => {
                let (clip, laser, under) = if weapon == WeaponKind::Shotgun {
                    (Clip::ShotgunReload, Clip::ShotgunReloadLaser, 0.2)
                } else {
                    (Clip::Reload, Clip::ReloadLaser, 0.55)
                };
                let level = if by.is_some() && by == own {
                    0.55
                } else {
                    0.55 * OTHERS_SHOTS * 0.5
                };
                self.one_shot(commands, clip, level);
                self.one_shot(commands, laser, level * under);
            }
            Cue::Blow { cut, on_crew } => {
                if cut {
                    self.one_shot(commands, Clip::Schword, 0.3);
                } else {
                    self.one_shot(commands, Clip::Punch, 0.25);
                }
                if on_crew {
                    self.one_shot(commands, Clip::Ouch, 0.3);
                }
            }
        }
    }

    /// A purchase at the trader — a thing off the shelf or an item — by
    /// anyone in the crew, the player or another: the world's
    /// `ShelfBought` or `ItemBought`, which every window hears. Not a
    /// place in the room, so all purchases share one cool-down.
    pub fn bought(&mut self, commands: &mut Commands) {
        if self.admit(Kind::Bought, bims::math::Vec2::ZERO) {
            self.one_shot(commands, Clip::Bought, 0.45);
        }
    }

    /// A sale to the trader by anyone in the crew, the world's `Sold`,
    /// heard in every window like a purchase, with its own cool-down.
    pub fn sold(&mut self, commands: &mut Commands) {
        if self.admit(Kind::Sold, bims::math::Vec2::ZERO) {
            self.one_shot(commands, Clip::Sold, 0.45);
        }
    }

    /// An enemy down, with the money and the experience it was worth
    /// floating up over it: a soft chime, quiet under the fight, heard
    /// in every window like a purchase.
    pub fn reward(&mut self, commands: &mut Commands) {
        if self.admit(Kind::Reward, bims::math::Vec2::ZERO) {
            self.one_shot(commands, Clip::Reward, 0.22);
        }
    }

    /// A player downed, this window's own or another's — never a bot
    /// (the game screen filters the world's `CrewDowned`): every player
    /// knows someone wants reviving. Loud: it is the one sound in a
    /// fight that asks the player to act. Two bodies down at once are
    /// two.
    pub fn downed(&mut self, commands: &mut Commands, who: u32) {
        if self.admit_in(Kind::Downed, (0, who as i32)) {
            self.one_shot(commands, Clip::Downed, 0.75);
        }
    }

    /// A body brought round where a player had a hand in it — a player
    /// up again, or a player's Bim that did the reviving (the game screen
    /// filters the world's `CrewRevived` and `ResidentRevived`): the
    /// answer to [`Sounds::downed`], a little under it. `who` tells two
    /// patients apart; a townsperson's is past the crew's.
    pub fn revived(&mut self, commands: &mut Commands, who: u32) {
        if self.admit_in(Kind::Revived, (1, who as i32)) {
            self.one_shot(commands, Clip::Revived, 0.6);
        }
    }

    /// This window's own player gaining a level (the game screen filters
    /// the world's `LevelUp`): a reward, so it stands over the fight.
    pub fn level_up(&mut self, commands: &mut Commands, who: u32) {
        if self.admit_in(Kind::LevelUp, (2, who as i32)) {
            self.one_shot(commands, Clip::LevelUp, 0.7);
        }
    }

    /// A class's ability used, by anyone in the crew: the world's event
    /// for it, which every window hears — a teammate's taunt as well as
    /// one's own. What the room hears of an ability afterwards (the
    /// grenade's burst, the EMP's) is its cue. The passive ones (Weak
    /// Spot, Healing Aura, Plated) are never used, and are not heard.
    /// The Medivac borrows the reinforcements' clip, a little quieter.
    pub fn ability(&mut self, commands: &mut Commands, event: world::WorldEvent) {
        use world::WorldEvent as E;
        use world::deploy::DeployKind;
        let (who, clip, level) = match event {
            E::Thrown { who } => (who, Clip::GrenadeThrow, 0.5),
            // The Stun Shot (October 2026): the feet planted for the charge
            // is the brace's clip; the shot is its gun's cue and the burst
            // the grenade's.
            E::ShotCharging { who } => (who, Clip::Brace, 0.55),
            E::Rampaged { who } => (who, Clip::Rampage, 0.7),
            // The engineer's (task 154): a satchel thrown borrows the EMP's
            // throw, a mine laid the sandbags' thump, and the trigger's
            // bursts are the grenade's cue, each.
            E::SatchelThrown { who } => (who, Clip::EmpThrow, 0.35),
            E::Deployed { who, kind } => match DeployKind::from_code(kind) {
                Some(DeployKind::Mine) => (who, Clip::Sandbags, 0.45),
                Some(DeployKind::HealingSentry) => (who, Clip::HealingSentry, 0.5),
                Some(DeployKind::Sentry) => (who, Clip::Sentry, 0.6),
                Some(DeployKind::Satchel) | None => return,
            },
            // The medic's drone and circle (task 153) borrow the clips of
            // the burst and the cloak they replaced: the canister's hiss as
            // a drone lifts off, the shimmer as the circle comes up, and the
            // beam's own let-go as it goes down.
            E::DroneLaunched { who } => (who, Clip::NaniteBurst, 0.4),
            E::Circled { who, on: true } => (who, Clip::Cloak, 0.5),
            E::Circled { who, on: false } => (who, Clip::BeamOff, 0.3),
            E::Beamed {
                who,
                patient: Some(_),
            } => (who, Clip::BeamOn, 0.3),
            E::Beamed { who, patient: None } => (who, Clip::BeamOff, 0.3),
            // The tank's (task 155) borrow the clips of the abilities they
            // replaced: the wall's clank as the shield comes up and goes
            // down (louder as it breaks), the taunt's shout for the
            // barrier, the Juggernaut's roar for the Bastion.
            E::ShieldRaised { who, on: true } => (who, Clip::BulwarkOn, 0.6),
            E::ShieldRaised { who, on: false } => (who, Clip::BulwarkOff, 0.4),
            E::ShieldBroken { who } => (who, Clip::BulwarkOff, 0.75),
            E::Reflecting { who } => (who, Clip::Taunt, 0.55),
            E::Bastion { who, .. } => (who, Clip::Juggernaut, 0.75),
            E::BattleCried { who } => (who, Clip::BattleCry, 0.35),
            E::Rallied { who } => (who, Clip::Rally, 0.3),
            E::Reinforced { who, .. } => (who, Clip::Reinforcements, 0.5),
            E::Medivac { who, .. } => (who, Clip::Reinforcements, 0.4),
            // A Blink Drive (October 2026): the old cloak's shimmer, short of
            // a clip of its own.
            E::Blinked { who } => (who, Clip::Cloak, 0.45),
            // The other active items (October 2026), each the sound of the
            // ability it is nearest: a burst of healing, a rampage's
            // charge, a wall going up.
            E::ItemUsed { who, kind } => match bims::module::ModuleKind::from_code(kind) {
                Some(bims::module::ModuleKind::FieldMender) => (who, Clip::NaniteBurst, 0.5),
                Some(bims::module::ModuleKind::ResetCapacitor) => (who, Clip::Rampage, 0.5),
                Some(bims::module::ModuleKind::AblativeShell) => (who, Clip::BulwarkOn, 0.55),
                // Step three (October 2026): a canister lobbed, a link
                // made, a ghost going out, a rush.
                Some(bims::module::ModuleKind::SmokeLauncher) => (who, Clip::SmokeBang, 0.6),
                Some(bims::module::ModuleKind::TetherLink) => (who, Clip::BeamOn, 0.5),
                Some(bims::module::ModuleKind::DecoyProjector) => (who, Clip::Cloak, 0.5),
                Some(bims::module::ModuleKind::AdrenalInjector) => (who, Clip::Rampage, 0.45),
                _ => return,
            },
            _ => return,
        };
        if self.admit_in(Kind::Ability, (clip as i32, who as i32)) {
            self.one_shot(commands, clip, level);
        }
    }

    /// Ask for a bed this frame: it fades up to its level and stays
    /// while it keeps being asked for, and fades out when it stops. A
    /// screen asks every frame, in its own system.
    pub fn want(&mut self, bed: Bed) {
        self.beds[bed as usize].wanted = 1.0;
    }

    /// The Smoke Launcher's clouds this frame (October 2026), each a key
    /// and its seconds left: the hiss held while any has more than its
    /// going out to run, and the going out played once a cloud, the
    /// moment it has [`SMOKE_OUT_SECONDS`] left — the recording's tail,
    /// so it dies with the picture.
    pub fn smoke(&mut self, commands: &mut Commands, clouds: &[(u64, f64)]) {
        self.smoke_out
            .retain(|key| clouds.iter().any(|&(k, _)| k == *key));
        for &(key, left) in clouds {
            if left > SMOKE_OUT_SECONDS {
                self.want(Bed::Smoke);
            } else if left > 0.0 && !self.smoke_out.contains(&key) {
                self.smoke_out.push(key);
                self.one_shot(commands, Clip::SmokeOut, 0.55);
            }
        }
    }
}

/// Every bed's level a step towards what was asked for this frame, and
/// the asks cleared for the next; the clock on. Runs once a frame in
/// `Update`; the screens ask in the egui pass, and whether that is before
/// or after this in a frame, a frame's lag on a fade is nothing.
fn fade(mut sounds: ResMut<Sounds>, mut sinks: Query<&mut AudioSink>, time: Res<Time>) {
    let dt = time.delta_secs().min(0.1);
    sounds.clock += dt as f64;
    sounds.started = 0;
    let sounds = &mut *sounds;
    for (bed, state) in Bed::ALL.iter().zip(&mut sounds.beds) {
        let step = bed.rate() * dt;
        let to = state.wanted;
        let was = state.level;
        state.level += (to - state.level).clamp(-step, step);
        state.wanted = 0.0;
        if sounds.log && (was == 0.0) != (state.level == 0.0) {
            println!(
                "sound: {bed:?} {}",
                if state.level > 0.0 { "up" } else { "out" }
            );
        }
        // No sink is no audio device: the bed is silent whatever it is
        // asked, and there is nothing to set.
        if let Ok(mut sink) = sinks.get_mut(state.entity) {
            // The smoke's hiss is an effect, turned with the shots.
            let mix = if *bed == Bed::Smoke {
                sounds.mix.effects()
            } else {
                sounds.mix.ambience()
            };
            sink.set_volume(Volume::Linear(
                state.level * bed.level() * sounds.volumes.of(bed.clip()).max(0.0) * mix,
            ));
        }
    }
}

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, Sounds::open)
            .add_systems(Update, fade);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every clip is a real Ogg Vorbis stream: an `include_bytes!` of the
    /// wrong file, or a `prepare.sh` that was not run, would otherwise be
    /// found by the first shot fired.
    #[test]
    fn every_clip_is_ogg() {
        for (i, clip) in CLIPS.iter().enumerate() {
            assert!(clip.starts_with(b"OggS"), "clip {i} is not an Ogg stream");
            assert!(clip.len() > 1_000, "clip {i} is only {} bytes", clip.len());
        }
        assert_eq!(CLIPS.len(), Clip::SmokeOut as usize + 1);
    }

    /// `audio.ron` at the root parses and names every sound, so the player
    /// finds each one there to turn.
    #[test]
    fn the_audio_file_names_every_sound() {
        let text = include_str!("../../../audio.ron");
        assert!(ron::from_str::<Volumes>(text).is_ok());
        for name in Volumes::NAMES {
            assert!(
                text.lines()
                    .any(|l| l.trim_start().starts_with(&format!("{name}:"))),
                "audio.ron has no {name}"
            );
        }
        assert_eq!(Volumes::NAMES.len(), CLIPS.len() + 3);
        // A sound left out is as built; a misspelt one is refused.
        let v: Volumes = ron::from_str("(ouch: 0.5)").unwrap();
        assert_eq!(v.of(Clip::Ouch), 0.5);
        assert_eq!(v.of(Clip::Rifle), 1.0);
        assert!(ron::from_str::<Volumes>("(ouhc: 0.5)").is_err());
    }
}
