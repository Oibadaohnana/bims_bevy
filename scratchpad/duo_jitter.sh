#!/usr/bin/env bash
# The guest's playout buffer (task 148), looked at with two windows: a
# relay on a scratch port, a host and a guest (`BIMS_AUTO`), the guest's
# line made shaky with `BIMS_NET_JITTER` (every arrival held a random
# nought to that many ms, in order). Run it with the buffer on and off:
#
#   scratchpad/duo_jitter.sh            the buffer at 0.40 s (the default)
#   BUFFER=0 scratchpad/duo_jitter.sh   the buffer off, as before task 148
#   JITTER=250 scratchpad/duo_jitter.sh a shakier line (default 150 ms)
#
# The guest prints `playout: target … gaps … frames [none, one, two, 3+]`
# with every checksum: how many of its frames played no step while the
# world ran, one, two or more. A smooth guest is nearly all ones. The
# buffer's cap is the keys file's `network-buffer` line, so the guest is
# given a keys file of its own (XDG_CONFIG_HOME). Everything lands under
# target/duo/jitter-<buffer>/. Build first: `cargo build -p app -p server`
# inside the shell.
set -uo pipefail
cd "$(dirname "$0")/.."

buffer=${BUFFER:-0.40}
jitter=${JITTER:-150}
port=${PORT:-18793}
out=target/duo/jitter-$buffer
rm -rf "$out"
mkdir -p "$out/config/bims"
out=$(realpath "$out")
printf 'network-buffer=%s\n' "$buffer" > "$out/config/bims/keys"

PORT=$port target/debug/bims-server > "$out/server.out" 2>&1 &
server=$!
trap 'kill $server 2>/dev/null' EXIT
sleep 1

common=(BIMS_SERVER=ws://127.0.0.1:$port BIMS_AUTO_PLAYERS=2 BIMS_READY=0)

env "${common[@]}" BIMS_AUTO=create BIMS_NAME=Host BIMS_BIM_NAME=Ada \
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

env "${common[@]}" XDG_CONFIG_HOME="$out/config" BIMS_NET_JITTER=$jitter \
  BIMS_AUTO=join:$code BIMS_NAME=Guest BIMS_BIM_NAME=Bob \
  BIMS_SMOKE_FRAMES=${GUEST_FRAMES:-1400} BIMS_SCREENSHOT="$out/guest.png" \
  ./hidden target/debug/bims game > "$out/guest.out" 2> "$out/guest.err" &
guest=$!

wait $host; host_status=$?
wait $guest; guest_status=$?
echo "host exited $host_status, guest exited $guest_status (buffer $buffer s, jitter $jitter ms)"

fail=0
echo "desyncs on the guest: $(grep -c '^desync:' "$out/guest.out")"
grep -q '^desync:' "$out/guest.out" && fail=1
echo "guest's last playout: $(grep '^playout:' "$out/guest.out" | tail -1)"
last_guest=$(grep '^checksum:' "$out/guest.out" | tail -1)
if [ -n "$last_guest" ] && grep -qF "$last_guest" "$out/host.out"; then
  echo "  ok   the guest's last checksum is one of the host's: $last_guest"
else
  echo "  MISSING  the guest's last checksum among the host's ($last_guest)"
  fail=1
fi
echo "pictures: $out/host.png $out/guest.png"
exit $fail
