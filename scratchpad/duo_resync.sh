#!/usr/bin/env bash
# The host's world is the world (feature 67), looked at with two windows:
# a relay on a scratch port, a host and a guest (`BIMS_AUTO`, see
# CLAUDE.md "A game with company"), each on its own headless compositor
# through ./hidden. Two scenarios, one per argument:
#
#   scratchpad/duo_resync.sh desync   a guest parts from the host on purpose
#                                     (BIMS_DESYNC_AT) and is handed the
#                                     host's world back
#   scratchpad/duo_resync.sh load     the host saves, then loads the save;
#                                     the guest gets the loaded world
#
# Everything lands under target/duo/<scenario>/: host.out, guest.out (the
# `checksum:` / `desync:` / `resync:` lines), host.png, guest.png, and the
# saves directory. The script says which of the expected lines it found,
# and exits non-zero if one is missing. Build first: `cargo build -p app
# -p server` inside the shell.
set -uo pipefail
cd "$(dirname "$0")/.."

scenario=${1:-desync}
port=${PORT:-18792}
out=target/duo/$scenario
rm -rf "$out"
mkdir -p "$out/saves"
out=$(realpath "$out")

# The world opens about frame 100 (lobby, Start, a second in the yard,
# Accept) and runs a step a frame at 1x; the guest is given more frames
# than the host, or the host's picture says "has left".
case $scenario in
  desync)
    host_frames=${HOST_FRAMES:-1300}
    guest_frames=${GUEST_FRAMES:-1400}
    ;;
  load)
    host_frames=${HOST_FRAMES:-1300}
    guest_frames=${GUEST_FRAMES:-1400}
    ;;
  *) echo "unknown scenario: $scenario" >&2; exit 2 ;;
esac

PORT=$port target/debug/bims-server > "$out/server.out" 2>&1 &
server=$!
trap 'kill $server 2>/dev/null' EXIT
sleep 1

common=(BIMS_SERVER=ws://127.0.0.1:$port BIMS_SAVES_DIR="$out/saves" BIMS_AUTO_PLAYERS=2)

host_env=()
guest_env=()
case $scenario in
  desync)
    # The guest applies an order of its own at step 300: the next checksum
    # says `desync:`, the host's world comes back, and the checksums
    # match again from there.
    guest_env=(BIMS_DESYNC_AT=300)
    ;;
  load)
    # The host, once the world is open: at frame 400 (step ~300) Esc, Save, the
    # page's Save button; Esc; then at 480 Esc, Load, the one row, the page's
    # Load button (frame 540, step ~440, so the loaded world is ~100 steps
    # behind the one it replaces). Points are the sheet anchored in the middle of a
    # 1400x900 window (settings.rs, save.rs), read off screenshots of the
    # pages. Move a frame before each click: egui hit-tests against the
    # previous frame's widgets.
    host_env=(
      BIMS_KEYS="${HOST_KEYS:-400:Escape,460:Escape,480:Escape}"
      BIMS_POINTER="${HOST_POINTER:-410:move:560,496;420:click:560,496;430:move:750,427;440:click:750,427;490:move:612,496;500:click:612,496;510:move:620,440;520:click:620,440;530:move:542,471;540:click:542,471}"
    )
    ;;
esac

env "${common[@]}" "${host_env[@]}" BIMS_AUTO=create BIMS_NAME=Host BIMS_BIM_NAME=Ada \
  BIMS_SMOKE_FRAMES=$host_frames BIMS_SCREENSHOT="$out/host.png" \
  ./hidden target/debug/bims game > "$out/host.out" 2> "$out/host.err" &
host=$!

code=
for _ in $(seq 1 100); do
  code=$(sed -n 's/^lobby: //p' "$out/host.out" | head -1)
  [ -n "$code" ] && break
  sleep 0.2
done
if [ -z "$code" ]; then
  echo "the host never printed a lobby code" >&2
  cat "$out/host.err" >&2
  kill $host 2>/dev/null
  exit 1
fi

env "${common[@]}" "${guest_env[@]}" BIMS_AUTO=join:$code BIMS_NAME=Guest BIMS_BIM_NAME=Bob \
  BIMS_SMOKE_FRAMES=$guest_frames BIMS_SCREENSHOT="$out/guest.png" \
  ./hidden target/debug/bims game > "$out/guest.out" 2> "$out/guest.err" &
guest=$!

wait $host; host_status=$?
wait $guest; guest_status=$?
echo "host exited $host_status, guest exited $guest_status"

fail=0
expect() { # file, pattern, words
  if grep -q "$2" "$1"; then
    echo "  ok   $3: $(grep "$2" "$1" | head -1)"
  else
    echo "  MISSING  $3 ($2 in $1)"
    fail=1
  fi
}
echo "host checksums:  $(grep -c '^checksum:' "$out/host.out")"
echo "guest checksums: $(grep -c '^checksum:' "$out/guest.out")"
case $scenario in
  desync)
    expect "$out/guest.out" '^diverged:' "the guest parted on purpose"
    expect "$out/guest.out" '^desync:' "the guest noticed"
    expect "$out/guest.out" '^resync:' "the host's world arrived"
    # After the resync every checksum the guest prints matched the host's
    # — that is what a `checksum:` line is — so what is wanted is that
    # there are some, and that the last agree with the host's own.
    after=$(sed -n '/^resync:/,$p' "$out/guest.out" | grep -c '^checksum:')
    echo "  guest checksums matched after the resync: $after"
    [ "$after" -ge 2 ] || { echo "  MISSING  two matching checksums after the resync"; fail=1; }
    ;;
  load)
    ls -la "$out/saves"
    expect "$out/host.out" '^world sent: [0-9]* loaded at' "the host loaded and sent its world"
    expect "$out/guest.out" '^resync:' "the loaded world arrived on the guest"
    sent=$(sed -n 's/^world sent: \([0-9]*\) loaded.*/\1/p' "$out/host.out" | head -1)
    got=$(sed -n 's/^resync: //p' "$out/guest.out" | head -1)
    if [ -n "$sent" ] && [ "$sent" = "$got" ]; then
      echo "  ok   the world the guest took is the one loaded, at step $sent"
    else
      echo "  MISSING  the guest's world ($got) is not the loaded one ($sent)"; fail=1
    fi
    # The load put the clock back: the host says what step it was at.
    before=$(sed -n 's/^world sent: [0-9]* loaded at \([0-9]*\)/\1/p' "$out/host.out" | head -1)
    if [ -n "$before" ] && [ "$before" -gt "$sent" ]; then
      echo "  ok   the clock went back: $before before the load, $sent after"
    else
      echo "  MISSING  a load that put the clock back ($before -> $sent)"; fail=1
    fi
    after=$(sed -n '/^resync:/,$p' "$out/guest.out" | grep -c '^checksum:')
    echo "  guest checksums matched after the load: $after"
    [ "$after" -ge 2 ] || { echo "  MISSING  two matching checksums after the load"; fail=1; }
    ;;
esac
last_host=$(grep '^checksum:' "$out/host.out" | tail -1)
last_guest=$(grep '^checksum:' "$out/guest.out" | tail -1)
echo "  host's last:  $last_host"
echo "  guest's last: $last_guest"
if grep -qF "$last_guest" "$out/host.out"; then
  echo "  ok   the guest's last checksum is one of the host's"
else
  echo "  MISSING  the guest's last checksum among the host's"
  fail=1
fi
echo "pictures: $out/host.png $out/guest.png"
exit $fail
