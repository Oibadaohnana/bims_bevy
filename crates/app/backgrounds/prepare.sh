#!/usr/bin/env bash
# The menus' backdrops, from the pictures in `Background/` at the root
# (`crate::backdrop` includes what this writes):
#
#   start.png     — the start menu, Start_screen_Background.png recompressed;
#   setup_N.png   — the game setup's animation, one palette PNG per distinct
#                   frame of Game_Setup_Background_animated.gif. The gif
#                   plays its frames there and back (0 1 2 3 4 3 2 1 0, then
#                   0 9 10 11 12 11 10 9), so only nine are different; the
#                   order and the delays are `backdrop::SETUP_FRAMES`.
#
# Needs imagemagick (`magick`). Run from anywhere; a changed gif wants its
# frame order checked again (the hashes this prints) and the table redone.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
src="$here/../../../Background"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

magick "$src/Start_screen_Background.png" -strip -define png:compression-level=9 "$tmp/start.png"
[ -s "$tmp/start.png" ] && cp "$tmp/start.png" "$here/start.png"

magick "$src/Game_Setup_Background_animated.gif" -coalesce "$tmp/g%02d.png"
for f in "$tmp"/g*.png; do
    echo "$(basename "$f") $(magick "$f" -format '%#' info:)"
done
magick identify -format '%T ' "$src/Game_Setup_Background_animated.gif"; echo
n=0
for i in 00 01 02 03 04 09 10 11 12; do
    magick "$tmp/g$i.png" -strip -define png:compression-level=9 "PNG8:$tmp/setup_$n.png"
    [ -s "$tmp/setup_$n.png" ] && cp "$tmp/setup_$n.png" "$here/setup_$n.png"
    n=$((n + 1))
done
