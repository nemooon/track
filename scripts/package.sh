#!/bin/sh
# Builds Track.app and a DMG into target/package/ (ad-hoc signed: for this Mac).
#   Track.app/Contents/MacOS/track-gpui        the app (built with track-core/real-data: opens ~/.track)
#   Track.app/Contents/MacOS/track-cli         the CLI the AI skills call (runtime.json points here)
#   Track.app/Contents/Resources/TrackAIHelper.app   Apple Intelligence helper (needs the macOS 26 SDK)
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
VERSION=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
OUT="$ROOT/target/package"
APP="$OUT/Track.app"

cargo build --release --workspace --features track-core/real-data
scripts/build-ai-helper.sh release

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
sed "s/@VERSION@/$VERSION/g" packaging/Info.plist > "$APP/Contents/Info.plist"
cp target/release/track-gpui target/release/track-cli "$APP/Contents/MacOS/"
cp packaging/icon.icns "$APP/Contents/Resources/icon.icns"
cp -R target/release/TrackAIHelper.app "$APP/Contents/Resources/"
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict "$APP"

DMG="$OUT/Track_$VERSION.dmg"
STAGE="$OUT/dmg"
rm -rf "$STAGE" "$DMG"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -quiet -volname "Track $VERSION" -srcfolder "$STAGE" -ov -format UDZO "$DMG"
rm -rf "$STAGE"
echo "$APP"
echo "$DMG"
