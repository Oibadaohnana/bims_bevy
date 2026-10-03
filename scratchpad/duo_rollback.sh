#!/usr/bin/env bash
# A guest's own orders played at once and rolled back (task 156), looked
# at with two windows: a relay on a scratch port, a host and a guest
# (`BIMS_AUTO`), the guest walking about on WASD (`BIMS_KEYS`) over a line
# made slow with `BIMS_NET_JITTER` (every arrival held a random nought to
# that many ms, in order). Run it with the rollback on and off:
#
#   scratchpad/duo_rollback.sh              guessing (the default)
#   ROLLBACK=0 scratchpad/duo_rollback.sh   the guest waits on the host, as before
#   JITTER=200 scratchpad/duo_rollback.sh   a slower line (default 120 ms)
#
# The guest prints `rollback: lead … pending … rollbacks … replayed … on
# time … late …` with every checksum, and `checksum:`/`desync:` as ever.
# Everything lands under target/duo/rollback-<on>/. Build first:
# `cargo build -p app -p server` inside the shell.
set -uo pipefail
cd "$(dirname "$0")/.."

rollback=${ROLLBACK:-1}
jitter=${JITTER:-120}
port=${PORT:-18794}
out=target/duo/rollback-$rollback
rm -rf "$out"
mkdir -p "$out"
out=$(realpath "$out")

PORT=$port target/debug/bims-server > "$out/server.out" 2>&1 &
server=$!
trap 'kill $server 2>/dev/null' EXIT
sleep 1

common=(BIMS_SERVER=ws://127.0.0.1:$port BIMS_AUTO_PLAYERS=2 BIMS_READY=0 BIMS_ROLLBACK=$rollback)

env "${common[@]}" BIMS_AUTO=create BIMS_NAME=Host BIMS_BIM_NAME=Ada \
  BIMS_KEYS="${HOST_KEYS:-500:+W,560:-W,700:+S,760:-S}" \
  BIMS_POINTER="${HOST_POINTER:-300:move:900,300}" \
  BIMS_SMOKE_FRAMES=${HOST_FRAMES:-1300} BIMS_SCREENSHOT="$out/host.png" \
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

env "${common[@]}" BIMS_NET_JITTER=$jitter \
  BIMS_AUTO=join:$code BIMS_NAME=Guest BIMS_BIM_NAME=Bob \
  BIMS_KEYS="${GUEST_KEYS:-400:+D,470:-D,520:+S,580:-S,640:+A,700:-A,760:+W,800:+D,840:-W,880:-D,1000:+A,1060:-A}" \
  BIMS_POINTER="${GUEST_POINTER:-300:move:1000,450}" \
  BIMS_SMOKE_FRAMES=${GUEST_FRAMES:-1400} BIMS_SCREENSHOT="$out/guest.png" \
  ./hidden target/debug/bims game > "$out/guest.out" 2> "$out/guest.err" &
guest=$!

wait $host; host_status=$?
wait $guest; guest_status=$?
echo "host exited $host_status, guest exited $guest_status (rollback $rollback, jitter $jitter ms)"

fail=0
echo "desyncs on the guest: $(grep -c '^desync:' "$out/guest.out")"
grep -q '^desync:' "$out/guest.out" && fail=1
echo "guest's last rollback: $(grep '^rollback:' "$out/guest.out" | tail -1)"
last_guest=$(grep '^checksum:' "$out/guest.out" | tail -1)
if [ -n "$last_guest" ] && grep -qF "$last_guest" "$out/host.out"; then
  echo "  ok   the guest's last checksum is one of the host's: $last_guest"
else
  echo "  MISSING  the guest's last checksum among the host's ($last_guest)"
  fail=1
fi
echo "pictures: $out/host.png $out/guest.png"
exit $fail
