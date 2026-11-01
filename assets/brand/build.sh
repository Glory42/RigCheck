#!/usr/bin/env bash
# Regenerate every icon PNG from the drawing scripts in src/.
# Needs python3 and ImageMagick 7 (`magick`).
set -euo pipefail
cd "$(dirname "$0")"

python3 -I src/draw_icon_16.py icon-16.png
python3 -I src/draw_icon_32.py icon-32.png

# The art sits on a 32x32 grid, so whole-number nearest-neighbor scaling keeps it sharp.
for s in 64 128 256 512 1024; do
  magick icon-32.png -filter point -resize "${s}x${s}" "PNG32:icon-$s.png"
done
