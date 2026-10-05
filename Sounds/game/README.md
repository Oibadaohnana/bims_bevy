# The game's sounds

Every sound the game plays is one file here, exactly as the game plays
it. Edit one, save it over itself under the same name, rebuild
(`cargo run -- droids`, say), and the game plays your version. The files
are built into the program, so a game already running keeps the old
sound until it is rebuilt.

The recordings one folder up (`Sounds/*.mp3`) are the raw material, cut
down into these. Editing a recording there changes nothing in the game
on its own; editing the file here does.

## Saving one

- **Ogg Vorbis, same name**: `laser_1.ogg` stays `laser_1.ogg`. In
  Audacity: File → Export Audio → *Ogg Vorbis*, quality 5 or more. The
  game reads no MP3. MP3 couldn't carry the loops anyway: an MP3 encoder
  pads the start and end with a little silence, which would be a gap at
  every turn of a loop.
- They are made mono, 48 kHz. Stereo or another rate plays too.
- **Level.** The one-shots peak at -1 dB (the remastered ones are
  at your own levels). The loops are levelled by
  loudness: the five ambiences to -30 LUFS, `smoke_run` to -24. Keep
  about that and the mix stays where it was. To make a sound louder or
  quieter in the game, change its number in `audio.ron` at the root
  instead. The game re-reads that file while it runs.
- **Loops** (marked *loop* below) play end into start over and over, so
  the last sample has to run into the first without a click.
- **Timed** ones (marked *timed*) are lined up with something in the
  game or with another clip. Keep their length and where things happen
  in them.

## Which is which

The remastered ones were exported as MP3 into `Sounds/remastered/`.
They were made Ogg here (mono, 48 kHz, quality 5) at your levels. The
silence an MP3 encoder puts at the start (50–66 ms) was cut, so each
one starts where the old clip did.

*Made from* is the recording it was cut from. *Synthesised* means there
was no recording, so it was built from scratch (layered over a recording
where one is named).

### Guns

| file | heard | made from | length |
| --- | --- | --- | --- |
| `laser_1` … `laser_4` | the laser pistol, nine takes (with the five below) picked at random, never one twice running | `Laser_shot.mp3`, your remaster (`Sounds/remastered/`) | 0.5 s (`laser_2` 0.58) |
| `laser_5`, `laser_6` | the same, `laser_1` and `laser_2` a semitone up | the remaster, sped up ×1.059 | 0.47, 0.54 s |
| `laser_7`, `laser_8` | `laser_3` and `laser_4` a semitone and a half up | the remaster, sped up ×1.091 | 0.46 s |
| `laser_9` | `laser_1` two semitones up | the remaster, sped up ×1.122 | 0.44 s |
| `shotgun` | the shotgun | `Laser_shot.mp3`, slowed to 70% | 0.8 s |
| `rifle` | the auto rifle; the minigun, quieter | `Laser_shot.mp3`, sped up | 0.32 s |
| `sniper` | the sniper rifle; the rail lance, a Lancer's rail, the Unmaker | `Laser_Sniper_shot.mp3` | 1.3 s |
| `reload` | a magazine reloaded, every gun but the shotgun | `Gun_Reload.mp3` | 1.1 s, *timed* |
| `shotgun_reload` | the shotgun's shells pushed in | `Shotgun_reloading.mp3` | 3.55 s, *timed* |
| `reload_laser` | a laser cell's charge, played over `reload` | synthesised | 1.1 s, *timed* to `reload`'s clicks |
| `shotgun_reload_laser` | the same over `shotgun_reload`, a blip a shell | synthesised | 3.55 s, *timed* to its clicks |

### Hits

| file | heard | made from | length |
| --- | --- | --- | --- |
| `laser_hit` | a bolt landing on a body | `Laser_gun_hit.mp3`, your remaster (`Sounds/remastered/`) | 0.17 s |
| `laser_wall` | a bolt off a wall, or off a Guardian's shield | `Laser_gun_hit.mp3`, muffled; your remaster (`Sounds/remastered/`) | 0.17 s |
| `ouch` | one of the crew hit | `own_bim_getting_hit.mp3` | 0.35 s |
| `schword` | a blade's cut | `Schword_hit.mp3` | 0.55 s |
| `punch` | a blow that is not a cut | `Schword_hit.mp3`, dulled | 0.4 s |
| `grenade_burst` | a blast: Frag Grenade, Stun Shot, Satchel Charge, mine | synthesised over `big_droid_explosion.mp3` | 1.9 s |
| `emp_burst` | a blast that stuns rather than burns | synthesised | 1.3 s |

### Doors, hands, the trader, the crew

| file | heard | made from | length |
| --- | --- | --- | --- |
| `door_open` | a door sliding open | `Opening_Door.mp3`, 1.5× speed | 2.0 s |
| `door_close` | a door sliding shut | `Closing_Door.mp3`, 1.5× speed | 1.8 s |
| `door_force` | a body heaving at a locked door, and it giving | `Force_opening_Door_and_Arilock.mp3` | 0.95 s |
| `draw` | a weapon out of its holster | `Unholster_and_holstering.mp3` | 0.46 s |
| `holster` | and back in | `Unholster_and_holstering.mp3`, slowed | 0.44 s |
| `bought` | anything bought from a trader | `baught.mp3` | 1.8 s |
| `sold` | anything sold to a trader | `Sell_Sound.mp3` | 1.0 s |
| `reward` | the chime of an enemy down and its pay | synthesised | 0.35 s |
| `downed` | a player downed | synthesised over `own_bim_getting_hit.mp3` | 1.5 s |
| `revived` | a body brought round | synthesised | 1.45 s |
| `level_up` | your own level gained | synthesised; your remaster (`Sounds/remastered/`) | 1.95 s |

### The classes' abilities, and the items that borrow them

The file names are older than some of the abilities; this is what each
plays for now.

| file | heard | made from | length |
| --- | --- | --- | --- |
| `grenade_throw` | Soldier: Frag Grenade thrown | synthesised | 0.62 s |
| `brace` | Soldier: Stun Shot charging | synthesised over `Unholster_and_holstering.mp3` | 0.62 s |
| `rampage` | Soldier: Rampage; the Reset Capacitor and Adrenal Injector items | synthesised | 0.95 s |
| `sandbags` | Engineer: a Mine laid | synthesised | 0.85 s |
| `healing_sentry` | Engineer: a Healing Sentry set down | synthesised | 1.15 s |
| `emp_throw` | Engineer: a Satchel Charge thrown | synthesised | 0.85 s |
| `sentry` | Engineer: a Sentry set down | synthesised | 1.4 s |
| `nanite_burst` | Medic: a Heal Drone lifting off; the Field Mender item | synthesised | 1.0 s |
| `beam_on` | Medic: the Heal Beam on a patient; the Tether Link item | synthesised | 0.7 s |
| `beam_off` | Medic: the beam let go, a Healing Circle going down | synthesised | 0.4 s |
| `cloak` | Medic: a Healing Circle coming up; the Blink Drive and Decoy Projector items | synthesised | 1.05 s |
| `bulwark_on` | Tank: the Riot Shield raised; the Ablative Shell item | synthesised over `Force_opening_Door_and_Arilock.mp3` | 1.2 s |
| `bulwark_off` | Tank: the Riot Shield lowered, or broken (louder) | synthesised | 0.45 s |
| `taunt` | Tank: the Reflect Barrier | synthesised | 0.95 s |
| `juggernaut` | Tank: the Bastion | synthesised | 1.45 s |
| `battle_cry` | Commander: Battle Cry | synthesised | 0.9 s |
| `rally` | Commander: Rally | synthesised | 0.72 s |
| `reinforcements` | Commander: Reinforcements, and the Medivac | synthesised over `running_engine.mp3` | 2.2 s |
| `smoke_bang` | the Smoke Launcher's canister going off | `Smoke_Bang.mp3` | 1.3 s |
| `smoke_run` | its cloud hissing while it hangs | `Smoke_running_and_going_out.mp3` | 2.0 s, *loop* |
| `smoke_out` | the cloud going out | `Smoke_running_and_going_out.mp3` | 2.05 s, *timed* to the cloud's last 2 s |

### The machines' warnings

| file | heard | made from | length |
| --- | --- | --- | --- |
| `bomb_armed` | a Bomber's bomb rolling and spinning up | synthesised | 1.5 s, *timed* to the fuse |
| `rail_charge` | a Lancer charging its rail | synthesised | 1.2 s, *timed* to the charge |
| `marked` | a Conductor's mark on somebody | synthesised | 1.0 s, *timed* to the warning |
| `blink` | a Conductor blinking away | synthesised | 0.55 s |

### The ambiences (loops)

| file | heard | made from | length |
| --- | --- | --- | --- |
| `ship` | the ship's hum | `Spaceship_ambience.mp3` | 72 s, *loop* |
| `station` | a station's, docked | `Spacestation_ambience.mp3` | 65 s, *loop* |
| `temperate` | a temperate planet's air | `Temperate_climate_ambiance.mp3` | 65 s, *loop* |
| `desert` | a desert planet's | `Desert_world_ambiance.mp3` | 44 s, *loop* |
| `arctic` | an arctic planet's | `Arctic_world_ambiance.mp3` | 54 s, *loop* |

## Making them again

`crates/app/sounds/prepare.sh` cuts the recorded ones from `Sounds/`.
`crates/app/sounds/abilities.py` builds the synthesised ones. Both
take the names of the clips to make (`prepare.sh smoke_bang`), and
re-making a clip **overwrites your edit of it**. Run with no names they
re-make every clip. Git keeps every earlier version of each file, so an
edit you regret can be put back.
