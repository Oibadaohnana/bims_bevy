#!/usr/bin/env bash
# The recordings in `Sounds/` (at the repository root, left exactly as they
# were recorded) cut and filtered into the clips the app plays, one `.ogg`
# beside this script per clip. `sound.rs` embeds these with `include_bytes!`,
# so a re-run of this script is a rebuild of the app, and nothing is read
# from disk at run time.
#
#     crates/app/sounds/prepare.sh        # wants ffmpeg with libvorbis
#
# What is done to each and why is beside it below. The rule throughout: a
# one-shot is cut to the event with a few milliseconds either side and its
# peak brought to a known level, so that every gain the game applies is in
# `sound.rs` and not spread between there and here; a loop is cut to the
# steady part and its own tail cross-faded into its head, so the seam is
# not a click; and the two ambiences are brought down well below the
# recordings, which were made close up, to sit under everything else.
#
# A run re-encodes every clip, and an Ogg stream carries a random serial
# number, so the clips that were not meant to change come out different
# bytes of the same length: put those back from the tree (`git show
# HEAD:crates/app/sounds/x.ogg > x.ogg`) so the diff is the clips that did.
#
# Silence at the start of every recording was measured with
# `silencedetect`, the onsets of the one-shots with a 5 ms RMS envelope,
# and the numbers below are those. Redo them if the recordings change.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
src="$here/../../../Sounds"
out="$here"
ff=(ffmpeg -hide_banner -loglevel error -y)
enc=(-ar 48000 -c:a libvorbis -q:a 5)
# Every clip is mono — the game has no left and right — and the mix-down
# is the first thing in each chain, so the peak measured is the one encoded.
mono="aformat=channel_layouts=mono"

# Gain that brings a stream's sample peak to `target` dBFS. Measured in
# floating point by `astats`, not by `volumedetect`, which counts 16-bit
# samples and so reports a peak above full scale — an mp3 decodes to one
# now and then — as exactly 0 dB, and a clip would come out clipped.
peak_gain() {
    local filter=$1 target=$2 file=$3
    local peak
    peak=$(ffmpeg -hide_banner -i "$file" \
        -af "$filter,astats=measure_perchannel=none:measure_overall=Peak_level" -f null - 2>&1 \
        | sed -n 's/.*Peak level dB: \([-0-9.]*\).*/\1/p')
    awk -v p="$peak" -v t="$target" 'BEGIN { printf "%.2f", t - p }'
}

# A one-shot: `file`, cut from `from` for `length` seconds, through
# `filter`, peak at -1 dBFS, a short fade either end so a cut through a
# waveform is not a click. The filter runs over a quarter-second run-up
# before the cut rather than from the cut: a filter starts from silence
# and takes a few milliseconds to settle, which is a fade on a transient
# if the cut is where it starts. `stretch` is how many seconds of the
# recording make one of the clip, for a filter that slows or hurries it.
shot() {
    local name=$1 file=$2 from=$3 length=$4 filter=$5 fade_out=${6:-0.03} stretch=${7:-1}
    local margin=0.25
    local start
    start=$(awk -v f="$from" -v m="$margin" -v s="$stretch" 'BEGIN { printf "%.4f", f - m * s }')
    local pre="$mono,atrim=start=$start,asetpts=N/SR/TB,$filter,atrim=start=$margin:duration=$length,asetpts=N/SR/TB"
    local gain
    gain=$(peak_gain "$pre" -1 "$src/$file")
    "${ff[@]}" -i "$src/$file" \
        -af "$pre,volume=${gain}dB,afade=t=in:d=0.004,afade=t=out:st=$(awk -v l="$length" -v f="$fade_out" 'BEGIN { print l - f }'):d=$fade_out" \
        "${enc[@]}" "$out/$name.ogg"
    echo "$name.ogg  (${length}s, gain ${gain} dB)"
}

# A loop: the steady stretch from `from` to `to`, its last `xfade` seconds
# cross-faded into the `xfade` seconds before `from`, so the file ends on
# the sample it begins with. `filter` runs over the whole recording
# before either cut, for the reason above — filtered afterwards, the
# first sample of the loop would be the filter waking up, and the seam a
# click. Then EBU R128 loudness normalisation to `lufs` integrated: a
# loop plays for minutes, so it is levelled by loudness rather than peak.
loop() {
    local name=$1 file=$2 from=$3 to=$4 xfade=$5 filter=$6 lufs=$7
    local head
    head=$(awk -v f="$from" -v x="$xfade" 'BEGIN { print f - x }')
    local graph="[0:a]$mono,$filter,asplit[x][y];
                 [x]atrim=start=$from:end=$to,asetpts=N/SR/TB[body];
                 [y]atrim=start=$head:end=$from,asetpts=N/SR/TB[tail];
                 [body][tail]acrossfade=d=$xfade:c1=tri:c2=tri"
    # Two passes: loudnorm measures first, then applies linearly — a
    # single dynamic pass would pump on a loop of even background.
    local stats
    stats=$(ffmpeg -hide_banner -i "$src/$file" \
        -filter_complex "$graph,loudnorm=I=$lufs:TP=-2:LRA=11:print_format=json" \
        -f null - 2>&1 | sed -n '/^{/,/^}/p')
    local measured
    measured=$(echo "$stats" | awk -F'"' '
        /input_i/ { i = $4 } /input_tp/ { tp = $4 } /input_lra/ { lra = $4 }
        /input_thresh/ { th = $4 } /target_offset/ { off = $4 }
        END { printf "measured_I=%s:measured_TP=%s:measured_LRA=%s:measured_thresh=%s:offset=%s", i, tp, lra, th, off }')
    "${ff[@]}" -i "$src/$file" \
        -filter_complex "$graph,loudnorm=I=$lufs:TP=-2:LRA=11:$measured:linear=true" \
        "${enc[@]}" "$out/$name.ogg"
    echo "$name.ogg  (loop, $lufs LUFS)"
}

cd "$out"

# --- the fight ------------------------------------------------------------

# Four takes of the same shot, 0.55 s each, a fifth of a second of the
# decay's tail dropped: a bolt is gone long before that. All four are kept
# so a burst is not the same sample eight times over.
for i in 1 2 3 4; do
    case $i in
        1) at=1.425 ;; 2) at=2.620 ;; 3) at=3.980 ;; 4) at=5.062 ;;
    esac
    shot "laser_$i" Laser_shot.mp3 "$at" 0.55 "highpass=f=80"
done
# The shotgun is the loudest take slowed to 70%, which drops it about five
# semitones and stretches the crack: heavier, and clearly not the pistol.
shot shotgun Laser_shot.mp3 2.620 0.80 "asetrate=48000*0.7,aresample=48000,highpass=f=60,bass=g=4:f=120" 0.03 0.7
# The rifle's eight light shots: the shortest take sped up to 115%, cut to
# a third of a second so the burst is eight distinct cracks.
shot rifle Laser_shot.mp3 1.425 0.32 "asetrate=48000*1.15,aresample=48000,highpass=f=120" 0.03 1.15
# The sniper recording is a long charge-up whine, the shot, then the
# reload's clicks. The whine precedes the bolt by most of a second, and a
# cue is the bolt leaving, so the cut starts a tenth before the shot and
# keeps the reload — the weapon fires once in four seconds.
shot sniper Laser_Sniper_shot.mp3 1.50 1.30 "highpass=f=60" 0.15
# A bolt on a body: a sixth of a second of impact.
shot laser_hit Laser_gun_hit.mp3 2.235 0.22 "highpass=f=80"
# The same into a bulkhead: muffled, and the game plays it quieter still.
shot laser_wall Laser_gun_hit.mp3 2.235 0.22 "highpass=f=80,lowpass=f=2500"
# The blade landing. The recording peaks at -7 dB and is lifted like the rest.
shot schword Schword_hit.mp3 1.235 0.55 "highpass=f=60" 0.12
# There is no recording of a fist, so the blade's hit is dulled into a
# thud: everything above 400 Hz gone and slowed a little.
shot punch Schword_hit.mp3 1.235 0.40 "asetrate=48000*0.85,aresample=48000,lowpass=f=400" 0.12 0.85
# A crew member hit. The recording clips (peak above 0 dBFS), so a
# limiter tames the transient before the peak is set.
shot ouch own_bim_getting_hit.mp3 2.545 0.35 "highpass=f=80,alimiter=limit=0.7:level=false" 0.10

# --- the doors ------------------------------------------------------------

# Both door recordings are very quiet (peaks near -18 dBFS) with a heavy
# low band that is handling noise, not the door, and they run a second and
# a half where the leaves take four tenths to slide: the low band goes, the
# level comes up, and they are played at one and a half times the speed.
# The click at the end of each is the leaves landing, and is kept.
shot door_open Opening_Door.mp3 1.95 2.02 "highpass=f=140,atempo=1.5,acompressor=threshold=-18dB:ratio=3:attack=5:release=80" 0.06 1.5
shot door_close Closing_Door.mp3 0.80 1.80 "highpass=f=140,atempo=1.5,acompressor=threshold=-18dB:ratio=3:attack=5:release=80" 0.06 1.5
# A door being forced: the recording is one heave, from its onset at a
# fifth of a second to where it falls silent at a second and a tenth,
# and the game plays it once a heave while a body is at a locked door,
# and once more, louder, the moment the lock gives.
shot door_force Force_opening_Door_and_Arilock.mp3 0.19 0.95 "highpass=f=80" 0.10

# --- the ambiences --------------------------------------------------------

# Both recorded close and loud: -17 and -14 LUFS. They are background and
# are brought to -30, a dozen dB and more below where they were, and the
# game has a gain of its own on top. The station's is all bass; a shelf at
# 40 Hz keeps the sub-rumble from being the only thing a small speaker
# hears of it. Seamed over two seconds each.
loop ship Spaceship_ambience.mp3 3.5 75.8 2.0 "highpass=f=40" -30
loop station Spacestation_ambience.mp3 3.6 68.8 2.0 "highpass=f=40,lowpass=f=9000" -30

# The three planets' air, one a biome, played on the ground at a
# settlement in place of the station's hum (`Bed::of_biome`). All three
# recordings start with a moment of silence and end on a fade, and are
# looped over the steady middle. They were recorded at very different
# levels — the temperate one at -25 LUFS, the arctic wind close and loud
# at -15, the desert almost nothing at -48 — and all three are brought
# to the ambiences' -30, so the desert is lifted a good eighteen dB and
# the wind brought down as far. The desert's low band is the recorder's
# rumble more than the wind, and goes with the same shelf as the rest.
loop temperate Temperate_climate_ambiance.mp3 3.5 68.5 2.0 "highpass=f=40" -30
loop desert Desert_world_ambiance.mp3 3.7 47.5 2.0 "highpass=f=40" -30
loop arctic Arctic_world_ambiance.mp3 2.7 56.5 2.0 "highpass=f=40,lowpass=f=9000" -30

# --- the holster ----------------------------------------------------------

# One recording of a weapon coming out of its holster, half a second long
# from its onset at sixty milliseconds; the tail past half a second is
# room tone and is cut. It is the draw as recorded, and the holstering is
# the same slowed to 85%, a shade lower and longer, so the two are told
# apart by ear: out is quick, back is unhurried.
shot draw Unholster_and_holstering.mp3 0.06 0.46 "highpass=f=100" 0.08
shot holster Unholster_and_holstering.mp3 0.06 0.54 "asetrate=48000*0.85,aresample=48000,highpass=f=100" 0.10 0.85
