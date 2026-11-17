#!/bin/sh
# Builds the Apple Intelligence helper (TrackAIHelper.app) next to the app:
#   scripts/build-ai-helper.sh            -> target/debug/TrackAIHelper.app
#   scripts/build-ai-helper.sh release    -> target/release/TrackAIHelper.app
# A packaged Track.app carries it in Contents/Resources/. The app looks beside its binary
# first, then in the bundle (crates/track-core/src/ai/apple.rs).
# Needs an Apple Silicon Mac with the macOS 26 SDK (Foundation Models).
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
SRC="$ROOT/helpers/apple-intelligence"
PROFILE=${1:-debug}
case "$PROFILE" in debug|release) ;; *) echo "usage: $0 [debug|release]" >&2; exit 1 ;; esac
APP="$ROOT/target/$PROFILE/TrackAIHelper.app"
CACHE="$ROOT/target/swift-module-cache"

[ "$(uname -s)" = Darwin ] && [ "$(uname -m)" = arm64 ] || { echo "Apple Intelligence ヘルパーは Apple Silicon Mac でのみビルドできます" >&2; exit 1; }
SDK=$(xcrun --sdk macosx --show-sdk-path)
[ -d "$SDK/System/Library/Frameworks/FoundationModels.framework" ] || { echo "Foundation Models を含む macOS 26 SDK が見つかりません" >&2; exit 1; }

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$CACHE"
cp "$SRC/Info.plist" "$APP/Contents/Info.plist"
CLANG_MODULE_CACHE_PATH="$CACHE" xcrun --sdk macosx swiftc -parse-as-library -target arm64-apple-macosx26.0 \
  -sdk "$SDK" -module-cache-path "$CACHE" "$SRC/TrackAIHelper.swift" -o "$APP/Contents/MacOS/TrackAIHelper"
codesign --force --deep --sign - "$APP"
echo "Apple Intelligence helper: $APP"
