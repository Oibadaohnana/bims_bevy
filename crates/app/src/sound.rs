//! Sound: what the room's cues and the world's events are played as.
//!
//! The room says what happened as a [`Cue`] — a door starting to slide, a
//! knife on the board, a shot, a bolt landing — and the world as a
//! `WorldEvent`; neither has a recording in it, the way neither has a word.
//! This module is where the recordings are, the way `names.rs` is where
//! the words are: one clip per thing, embedded in the binary from
//! `crates/app/sounds/` (which `prepare.sh` there cuts from the recordings
//! in `Sounds/`), and a table that says which clip a cue is, and how loud.
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
    NaniteBurst,
    BeamOn,
    BeamOff,
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
}

/// The bytes of each clip, indexed by [`Clip`]. Ogg Vorbis, mono, 48 kHz,
/// peaks at -1 dBFS for the one-shots and -22 or -30 LUFS for the loops —
/// see `prepare.sh` — so every level below is relative to that.
const CLIPS: [&[u8]; 45] = [
    include_bytes!("../sounds/laser_1.ogg"),
    include_bytes!("../sounds/laser_2.ogg"),
    include_bytes!("../sounds/laser_3.ogg"),
    include_bytes!("../sounds/laser_4.ogg"),
    include_bytes!("../sounds/shotgun.ogg"),
    include_bytes!("../sounds/rifle.ogg"),
    include_bytes!("../sounds/sniper.ogg"),
    include_bytes!("../sounds/laser_hit.ogg"),
    include_bytes!("../sounds/laser_wall.ogg"),
    include_bytes!("../sounds/schword.ogg"),
    include_bytes!("../sounds/punch.ogg"),
    include_bytes!("../sounds/ouch.ogg"),
    include_bytes!("../sounds/door_open.ogg"),
    include_bytes!("../sounds/door_close.ogg"),
    include_bytes!("../sounds/door_force.ogg"),
    include_bytes!("../sounds/ship.ogg"),
    include_bytes!("../sounds/station.ogg"),
    include_bytes!("../sounds/draw.ogg"),
    include_bytes!("../sounds/holster.ogg"),
    include_bytes!("../sounds/temperate.ogg"),
    include_bytes!("../sounds/desert.ogg"),
    include_bytes!("../sounds/arctic.ogg"),
    include_bytes!("../sounds/bought.ogg"),
    include_bytes!("../sounds/grenade_throw.ogg"),
    include_bytes!("../sounds/grenade_burst.ogg"),
    include_bytes!("../sounds/brace.ogg"),
    include_bytes!("../sounds/rampage.ogg"),
    include_bytes!("../sounds/emp_throw.ogg"),
    include_bytes!("../sounds/emp_burst.ogg"),
    include_bytes!("../sounds/healing_sentry.ogg"),
    include_bytes!("../sounds/sandbags.ogg"),
    include_bytes!("../sounds/sentry.ogg"),
    include_bytes!("../sounds/nanite_burst.ogg"),
    include_bytes!("../sounds/beam_on.ogg"),
    include_bytes!("../sounds/beam_off.ogg"),
    include_bytes!("../sounds/cloak.ogg"),
    include_bytes!("../sounds/taunt.ogg"),
    include_bytes!("../sounds/bulwark_on.ogg"),
    include_bytes!("../sounds/bulwark_off.ogg"),
    include_bytes!("../sounds/juggernaut.ogg"),
    include_bytes!("../sounds/battle_cry.ogg"),
    include_bytes!("../sounds/rally.ogg"),
    include_bytes!("../sounds/reinforcements.ogg"),
    include_bytes!("../sounds/reward.ogg"),
    include_bytes!("../sounds/downed.ogg"),
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
}

impl Bed {
    const ALL: [Bed; 5] = [
        Bed::Ship,
        Bed::Station,
        Bed::Temperate,
        Bed::Desert,
        Bed::Arctic,
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
        }
    }

    /// How fast it fades, in fractions of full a second. A station's hum
    /// comes up over a couple of seconds as the airlocks mate; a planet's
    /// air comes in with the ground, at the station's pace.
    fn rate(self) -> f32 {
        match self {
            Bed::Ship | Bed::Station => 0.5,
            Bed::Temperate | Bed::Desert | Bed::Arctic => 0.5,
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
    /// A class's ability used, told apart by clip and by who used it.
    Ability,
    /// An enemy down and its pay floating up over it.
    Reward,
    /// A crew member downed, told apart by who.
    Downed,
}

/// How many one-shots a frame may start, whatever the room says. Enough
/// for a fight; a fight at top speed is a fight heard at real speed.
const SHOTS_PER_FRAME: u32 = 12;

/// Places are told apart this coarsely, in room units — two tiles — so a
/// gunner walking between the steps of one frame is one place.
const PLACE: f32 = 2.0 * bims::room::TILE;

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
            // One body's same ability twice in a quarter-second is the
            // world saying one use over several steps of a fast frame.
            Kind::Ability => 0.25,
            // A whole wave falling in one frame is one chime.
            Kind::Reward => 0.12,
            // A body goes down once; the room says it the step it does.
            Kind::Downed => 1.0,
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
    /// Which take of a clip with several is next, so a burst is not the
    /// same sample eight times over.
    turn: u32,
    /// `BIMS_SOUND_LOG=1`: say what is played.
    log: bool,
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
        let level = level * volume.max(0.0);
        if self.log {
            println!("sound: {clip:?} at {level:.2}");
        }
        let level = level * self.mix.effects();
        if level <= 0.0 {
            return;
        }
        commands.spawn((
            AudioPlayer::new(self.clips[clip as usize].clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(level)),
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

    /// The next of `n` takes.
    fn take(&mut self, n: u32) -> u32 {
        self.turn = self.turn.wrapping_add(1);
        self.turn % n
    }

    /// A room's cue, played — or dropped, inside its kind's cool-down.
    pub fn play(&mut self, commands: &mut Commands, cued: Cued) {
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
            Cue::Shot { weapon, hostile } => {
                // An enemy's shot a shade quieter: it is the crew's fight
                // the player is listening to.
                let theirs = if hostile { 0.8 } else { 1.0 };
                let v = self.volumes;
                let (clip, level, volume) = match weapon {
                    WeaponKind::LaserPistol => {
                        let takes = [Clip::Laser1, Clip::Laser2, Clip::Laser3, Clip::Laser4];
                        let clip = takes[self.take(4) as usize];
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
                self.one_shot_as(commands, clip, level * theirs, volume);
            }
            Cue::Impact { on_crew } => {
                if on_crew {
                    self.one_shot(commands, Clip::LaserHit, 0.4);
                    self.one_shot(commands, Clip::Ouch, 0.6);
                } else {
                    self.one_shot(commands, Clip::LaserHit, 0.5);
                }
            }
            // A bolt on a Guardian's shield has no recording of its own yet
            // (sounds are not in feature 100): it borrows the wall's.
            Cue::Ricochet | Cue::Shielded => self.one_shot(commands, Clip::LaserWall, 0.18),
            // The throw is heard from the world's `Thrown` and
            // `EmpThrown` (see [`Sounds::ability`]), which tell a grenade
            // from an EMP; the room says both as one `Throw`.
            Cue::Throw => {}
            Cue::Burst => self.one_shot(commands, Clip::GrenadeBurst, 0.9),
            Cue::EmpBurst => self.one_shot(commands, Clip::EmpBurst, 0.6),
            Cue::Blow { cut, on_crew } => {
                if cut {
                    self.one_shot(commands, Clip::Schword, 0.6);
                } else {
                    self.one_shot(commands, Clip::Punch, 0.5);
                }
                if on_crew {
                    self.one_shot(commands, Clip::Ouch, 0.6);
                }
            }
        }
    }

    /// A purchase at the trader — a thing off the shelf or the relic —
    /// by anyone in the crew, the player or another: the world's
    /// `ShelfBought` or `RelicBought`, which every window hears. Not a
    /// place in the room, so all purchases share one cool-down.
    pub fn bought(&mut self, commands: &mut Commands) {
        if self.admit(Kind::Bought, bims::math::Vec2::ZERO) {
            self.one_shot(commands, Clip::Bought, 0.45);
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

    /// A crew member downed — the player or anyone else, bot or human:
    /// the world's `CrewDowned`, which every window hears, so the whole
    /// crew knows someone wants reviving. Loud: it is the one sound in a
    /// fight that asks the player to act. Two bodies down at once are
    /// two.
    pub fn downed(&mut self, commands: &mut Commands, who: u32) {
        if self.admit_in(Kind::Downed, (0, who as i32)) {
            self.one_shot(commands, Clip::Downed, 0.75);
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
            E::Braced { who, on: true } => (who, Clip::Brace, 0.55),
            E::Braced { who, on: false } => (who, Clip::Holster, 0.35),
            E::Rampaged { who } => (who, Clip::Rampage, 0.7),
            E::EmpThrown { who } => (who, Clip::EmpThrow, 0.35),
            E::Deployed { who, kind } => match DeployKind::from_code(kind) {
                Some(DeployKind::Sandbags) => (who, Clip::Sandbags, 0.6),
                Some(DeployKind::HealingSentry) => (who, Clip::HealingSentry, 0.5),
                Some(DeployKind::Sentry) => (who, Clip::Sentry, 0.6),
                None => return,
            },
            E::NaniteBurst { who, .. } => (who, Clip::NaniteBurst, 0.5),
            E::Beamed {
                who,
                patient: Some(_),
            } => (who, Clip::BeamOn, 0.3),
            E::Beamed { who, patient: None } => (who, Clip::BeamOff, 0.3),
            E::Cloaked { who, .. } => (who, Clip::Cloak, 0.5),
            E::Taunted { who } => (who, Clip::Taunt, 0.6),
            E::Bulwarked { who, on: true } => (who, Clip::BulwarkOn, 0.7),
            E::Bulwarked { who, on: false } => (who, Clip::BulwarkOff, 0.45),
            E::Juggernaut { who } => (who, Clip::Juggernaut, 0.75),
            E::BattleCried { who } => (who, Clip::BattleCry, 0.35),
            E::Rallied { who } => (who, Clip::Rally, 0.3),
            E::Reinforced { who, .. } => (who, Clip::Reinforcements, 0.5),
            E::Medivac { who, .. } => (who, Clip::Reinforcements, 0.4),
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
            sink.set_volume(Volume::Linear(
                state.level
                    * bed.level()
                    * sounds.volumes.of(bed.clip()).max(0.0)
                    * sounds.mix.ambience(),
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
        assert_eq!(CLIPS.len(), Clip::Downed as usize + 1);
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
