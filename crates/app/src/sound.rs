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
//! and despawns itself; a **bed** — the ship's hum, a station's, the
//! engines, a planet's air — is a loop that runs the whole time at
//! whatever level the screen asks for, faded rather than switched. A screen asks every
//! frame, and a bed nobody asks for fades out, so a screen that closes
//! takes its sound with it without having to say so.
//!
//! Cues arrive a step's worth at a time, and at the world's top speed a
//! frame is many steps: every chop of a meal in one frame. Each kind of
//! cue has a cool-down in real seconds, **per place**, and what falls
//! inside it is dropped — the same knife twenty-four times in a frame is
//! one stroke, but two guns firing in one frame are two shots, and two
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
    Chop1,
    Chop2,
    Chop3,
    DoorOpen,
    DoorClose,
    DoorForce,
    EngineStart,
    Engine,
    Ship,
    Station,
    Draw,
    Holster,
    Temperate,
    Desert,
    Arctic,
}

/// The bytes of each clip, indexed by [`Clip`]. Ogg Vorbis, mono, 48 kHz,
/// peaks at -1 dBFS for the one-shots and -22 or -30 LUFS for the loops —
/// see `prepare.sh` — so every level below is relative to that.
const CLIPS: [&[u8]; 27] = [
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
    include_bytes!("../sounds/chop_1.ogg"),
    include_bytes!("../sounds/chop_2.ogg"),
    include_bytes!("../sounds/chop_3.ogg"),
    include_bytes!("../sounds/door_open.ogg"),
    include_bytes!("../sounds/door_close.ogg"),
    include_bytes!("../sounds/door_force.ogg"),
    include_bytes!("../sounds/engine_start.ogg"),
    include_bytes!("../sounds/engine.ogg"),
    include_bytes!("../sounds/ship.ogg"),
    include_bytes!("../sounds/station.ogg"),
    include_bytes!("../sounds/draw.ogg"),
    include_bytes!("../sounds/holster.ogg"),
    include_bytes!("../sounds/temperate.ogg"),
    include_bytes!("../sounds/desert.ogg"),
    include_bytes!("../sounds/arctic.ogg"),
];

/// The loops that run the whole time. The order is the order of the
/// `beds` array on [`Sounds`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
pub enum Bed {
    /// The ship's own hum: the deck, anywhere but tied up at a station.
    Ship,
    /// A station's, while docked with the rooms joined.
    Station,
    /// The engines burning.
    Engine,
    /// A planet's air, set down at its settlement with the ground on the
    /// joined deck: one a biome — birds and leaves, a dry wind, a cold one.
    Temperate,
    Desert,
    Arctic,
}

impl Bed {
    const ALL: [Bed; 6] = [
        Bed::Ship,
        Bed::Station,
        Bed::Engine,
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
            Bed::Engine => Clip::Engine,
            Bed::Temperate => Clip::Temperate,
            Bed::Desert => Clip::Desert,
            Bed::Arctic => Clip::Arctic,
        }
    }

    /// The level the bed plays at when fully up. The ambiences are
    /// already fifteen dB under the recordings in the file; this is on
    /// top, so they sit under a door two rooms away. A planet's air is
    /// levelled the same in the file and played the same.
    fn level(self) -> f32 {
        match self {
            Bed::Ship | Bed::Station => 0.7,
            Bed::Engine => 0.45,
            Bed::Temperate | Bed::Desert | Bed::Arctic => 0.7,
        }
    }

    /// How fast it fades, in fractions of full a second. A station's hum
    /// comes up over a couple of seconds as the airlocks mate; the engines
    /// are quicker, since the burn is; a planet's air comes in with the
    /// ground, at the station's pace.
    fn rate(self) -> f32 {
        match self {
            Bed::Ship | Bed::Station => 0.5,
            Bed::Engine => 1.2,
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
    Chop,
    Shot,
    Impact,
    Ricochet,
    Blow,
    /// A weapon out of its holster, or back into it.
    Holster,
    /// Not a room cue: the engines catching, off the world's events.
    EngineStart,
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
            Cue::Chop => Kind::Chop,
            Cue::Shot { .. } => Kind::Shot,
            Cue::Impact { .. } => Kind::Impact,
            Cue::Ricochet => Kind::Ricochet,
            Cue::Blow { .. } => Kind::Blow,
            Cue::Holster { .. } => Kind::Holster,
        }
    }

    /// How long after one of these before another is played, in real
    /// seconds. Shorter than the clip for the things that overlap in
    /// earnest — a burst is eight shots a quarter-second apart — and about
    /// the clip's length for the rest, so that twenty-four steps of
    /// chopping in one frame is one stroke.
    fn cool_down(self) -> f64 {
        match self {
            Kind::Door => 0.2,
            // A heave every two seconds a door; a frame at 24x holds
            // several, which is one.
            Kind::Smash => 0.5,
            Kind::Chop => 0.12,
            Kind::Shot => 0.04,
            Kind::Impact => 0.06,
            Kind::Ricochet => 0.1,
            Kind::Blow => 0.1,
            // A hand changes once a step at most; the clip is half a second.
            Kind::Holster => 0.3,
            // Undocking is said, and then departing; one ignition for both.
            Kind::EngineStart => 15.0,
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
            mix: Mix::default(),
        });
    }

    /// One clip, once, at `level` of its own under the player's effects
    /// volume — spawned to play and despawn itself when it has. Nothing is
    /// spawned while muted: a one-shot's volume is set when it starts,
    /// and a mute lifted a moment later would not reach it.
    fn one_shot(&self, commands: &mut Commands, clip: Clip, level: f32) {
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
        if self.started >= SHOTS_PER_FRAME {
            return false;
        }
        let now = self.clock;
        self.recent.retain(|&(_, _, until)| until > now);
        let cell = ((at.x / PLACE).floor() as i32, (at.y / PLACE).floor() as i32);
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
            Cue::Chop => {
                let clip = [Clip::Chop1, Clip::Chop2, Clip::Chop3][self.take(3) as usize];
                self.one_shot(commands, clip, 0.35);
            }
            Cue::Shot { weapon, hostile } => {
                // An enemy's shot a shade quieter: it is the crew's fight
                // the player is listening to.
                let theirs = if hostile { 0.8 } else { 1.0 };
                let (clip, level) = match weapon {
                    WeaponKind::LaserPistol => {
                        let takes = [Clip::Laser1, Clip::Laser2, Clip::Laser3, Clip::Laser4];
                        (takes[self.take(4) as usize], 0.5)
                    }
                    WeaponKind::Shotgun => (Clip::Shotgun, 0.7),
                    WeaponKind::AutoRifle => (Clip::Rifle, 0.35),
                    WeaponKind::SniperRifle => (Clip::Sniper, 0.6),
                    // A blade is never fired; the room does not say it is.
                    WeaponKind::Schword => return,
                };
                self.one_shot(commands, clip, level * theirs);
            }
            Cue::Impact { on_crew } => {
                if on_crew {
                    self.one_shot(commands, Clip::LaserHit, 0.4);
                    self.one_shot(commands, Clip::Ouch, 0.6);
                } else {
                    self.one_shot(commands, Clip::LaserHit, 0.5);
                }
            }
            Cue::Ricochet => self.one_shot(commands, Clip::LaserWall, 0.18),
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

    /// The engines catching: the push-off from a berth, or a trip
    /// beginning from a hold. Once, however many ways the world says it.
    pub fn engine_start(&mut self, commands: &mut Commands) {
        if self.admit(Kind::EngineStart, bims::math::Vec2::ZERO) {
            self.one_shot(commands, Clip::EngineStart, 0.5);
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
                state.level * bed.level() * sounds.mix.ambience(),
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
        assert_eq!(CLIPS.len(), Clip::Arctic as usize + 1);
    }
}
