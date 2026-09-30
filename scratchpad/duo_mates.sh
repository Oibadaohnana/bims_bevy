#!/usr/bin/env bash
# Teammates on each other's screens, looked at with two windows: a relay
# on a scratch port, a host and a guest (`BIMS_AUTO`), each through
# ./hidden. The guest does things; the host's picture shows them.
#
#   scratchpad/duo_mates.sh trader   both at a trader (BIMS_TRADER=1): the
#                                    guest's pointer rests on a line of the
#                                    trader's form — the host sees the
#                                    pointer and the line outlined in the
#                                    guest's colour — and the guest pings
#                                    the system view (Alt and a left click)
#   scratchpad/duo_mates.sh map      the map up on both: the
#                                    guest pings the galaxy chart and points
#                                    at the system view
#   scratchpad/duo_mates.sh reward   the relic choice (BIMS_REWARD=1): the
#                                    guest picks the first relic, and the
#                                    host sees it outlined and the pointer
#
# Pictures under target/duo/mates-<scenario>/: host.png, guest.png. Build
# first: `cargo build -p app -p server` inside the shell.
set -uo pipefail
cd "$(dirname "$0")/.."

scenario=${1:-trader}
port=${PORT:-18793}
out=target/duo/mates-$scenario
rm -rf "$out"
mkdir -p "$out"
out=$(realpath "$out")
host_frames=${HOST_FRAMES:-420}
guest_frames=${GUEST_FRAMES:-520}

PORT=$port target/debug/bims-server > "$out/server.out" 2>&1 &
server=$!
trap 'kill $server 2>/dev/null' EXIT
sleep 1

common=(BIMS_SERVER=ws://127.0.0.1:$port BIMS_AUTO_PLAYERS=2 BIMS_READY=0)
host_env=()
case $scenario in
  trader)
    common+=(BIMS_TRADER=1)
    guest_env=(
      BIMS_KEYS="${GUEST_KEYS:-300:+Alt,312:-Alt}"
      BIMS_POINTER="${GUEST_POINTER:-302:move:1000,600;304:click:1000,600;330:move:${LINE_X:-200},${LINE_Y:-260}}"
    )
    ;;
  map)
    # The map up on both (its button, bottom left). The guest pings the galaxy chart and rests its pointer on the
    # system view.
    host_env=(BIMS_POINTER="378:move:188,865;380:click:188,865")
    guest_env=(
      BIMS_KEYS="400:+Alt,412:-Alt"
      BIMS_POINTER="378:move:188,865;380:click:188,865;402:move:300,450;404:click:300,450;420:move:1000,500"
    )
    ;;
  reward)
    common+=(BIMS_REWARD=1)
    guest_env=(
      BIMS_POINTER="${GUEST_POINTER:-300:move:${PICK_X:-700},${PICK_Y:-330};302:click:${PICK_X:-700},${PICK_Y:-330};310:move:${REST_X:-760},${REST_Y:-420}}"
    )
    ;;
  *) echo "unknown scenario: $scenario" >&2; exit 2 ;;
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
  kill $host 2>/dev/null
  exit 1
fi

env "${common[@]}" "${guest_env[@]}" BIMS_AUTO=join:$code BIMS_NAME=Guest BIMS_BIM_NAME=Bob \
  BIMS_SMOKE_FRAMES=$guest_frames BIMS_SCREENSHOT="$out/guest.png" \
  ./hidden target/debug/bims game > "$out/guest.out" 2> "$out/guest.err" &
guest=$!
wait $host; wait $guest
grep -h '^trader:\|^desync' "$out/host.out" "$out/guest.out" | head
echo "pictures: $out/host.png $out/guest.png"
