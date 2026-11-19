#!/bin/sh
# Draws the app icon and renders it: packaging/icon.svg → packaging/icon.icns (the .app's icon)
# and packaging/icon-256.png (shown in 「Track について」). Run after changing scripts/icon.py.
set -eu
cd "$(dirname "$0")/.."
work="target/icon"
rm -rf "$work"
mkdir -p "$work/icon.iconset"
python3 scripts/icon.py packaging/icon.svg
swiftc -O scripts/svg2png.swift -o "$work/svg2png"
render() { "$work/svg2png" packaging/icon.svg "$1" "$2"; }
for s in 16 32 128 256 512; do
  render "$work/icon.iconset/icon_${s}x${s}.png" "$s"
  render "$work/icon.iconset/icon_${s}x${s}@2x.png" "$((s * 2))"
done
iconutil -c icns "$work/icon.iconset" -o packaging/icon.icns
render packaging/icon-256.png 256
echo "packaging/icon.icns, packaging/icon-256.png"
