#!/bin/sh
# Regenerates res/desktuk.ico (requires ImageMagick). The generated icon is committed.
set -e
cd "$(dirname "$0")/.."
convert -size 256x256 xc:none \
  -fill '#2D7DD2' -draw 'roundrectangle 16,16 240,240 44,44' \
  -fill '#FFFFFF' -draw 'roundrectangle 60,70 196,96 8,8' \
  -fill '#FFFFFF' -draw 'roundrectangle 60,116 112,168 8,8' \
  -fill '#FFFFFF' -draw 'roundrectangle 144,116 196,168 8,8' \
  -define icon:auto-resize=48,32,16 res/desktuk.ico
