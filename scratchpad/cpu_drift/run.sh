#!/usr/bin/env bash
# CPU drift (2026-10-07): does a machine with another CPU compute the
# simulation differently? Runs the same things twice on this machine —
# once as it is, once with glibc told the CPU has no FMA/AVX2 (the libm
# path a pre-Haswell Intel, a Pentium/Celeron or a pre-Zen AMD takes) —
# and compares bit for bit. `./run.sh arm` also runs the libm probe for
# aarch64 under qemu (fetches zig, qemu-user and the pinned aarch64 glibc).
#
#   libm_probe.rs  4M inputs through every libm function the sim calls,
#                  dumped raw; `libm_probe cmp a b` counts differing bits.
#   libm_probe.c   the same in C, so zig can cross-compile it for ARM.
#   sim/           a crate on the rules crates: `arena` (2 players, 16
#                  crew, tier-two machines, 20 000 steps), `runs a b`
#                  (seeds a..b: a mission at the spawn, back aboard, a
#                  trip, a mission), `gen n` (n seeds × 3 galaxy shapes),
#                  default (the survivors test's station and town). Each
#                  prints the rounded world_checksum and an exact hash of
#                  every body's position bits.
set -euo pipefail
cd "$(dirname "$0")"
OFF='glibc.cpu.hwcaps=-AVX2,-FMA'
OUT=${OUT:-/tmp/cpu_drift}
mkdir -p "$OUT"
rustc -O --edition 2024 libm_probe.rs -o "$OUT/libm_probe"
"$OUT/libm_probe" "$OUT/fma"
GLIBC_TUNABLES=$OFF "$OUT/libm_probe" "$OUT/nofma"
echo "== libm, FMA path vs no-FMA path"; "$OUT/libm_probe" cmp "$OUT/fma" "$OUT/nofma"
CARGO_TARGET_DIR="$OUT/target" cargo build --release -j 4 --manifest-path sim/Cargo.toml
SIM="$OUT/target/release/drift"
for mode in "" arena "gen 1000" "runs 0 100"; do
  "$SIM" $mode > "$OUT/a.txt" 2>/dev/null
  GLIBC_TUNABLES=$OFF "$SIM" $mode > "$OUT/b.txt" 2>/dev/null
  echo "== sim ${mode:-survivors}: $(diff "$OUT/a.txt" "$OUT/b.txt" | grep -c '^<' || true) of $(wc -l < "$OUT/a.txt") lines differ"
done
if [ "${1:-}" = arm ]; then
  root=$(git rev-parse --show-toplevel)
  zig=$(nix build --no-link --print-out-paths nixpkgs#zig)/bin/zig
  qemu=$(nix build --no-link --print-out-paths nixpkgs#qemu-user)/bin/qemu-aarch64
  gla=$(nix build --no-link --print-out-paths --inputs-from "$root" 'nixpkgs#legacyPackages.aarch64-linux.glibc^out')/lib
  ZIG_GLOBAL_CACHE_DIR="$OUT/zig" "$zig" cc -O2 -ffp-contract=off -target aarch64-linux-gnu.2.38 libm_probe.c -o "$OUT/probe_arm" -lm
  "$qemu" "$gla/ld-linux-aarch64.so.1" --library-path "$gla" "$OUT/probe_arm" "$OUT/arm"
  echo "== libm, x86 FMA path vs aarch64"; "$OUT/libm_probe" cmp "$OUT/fma" "$OUT/arm"
fi
