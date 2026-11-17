#!/bin/sh
# Builds the Apple Intelligence helper (TrackAIHelper.app) into target/ai-helper/.
# Needs an Apple Silicon Mac with the macOS 26 SDK (Foundation Models).
# The app looks for it inside Track.app (crates/track-core/src/ai/apple.rs).
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
SRC="$ROOT/helpers/apple-intelligence"
OUT="$ROOT/target/ai-helper"
APP="$OUT/TrackAIHelper.app"
CACHE="$OUT/.swift-module-cache"

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
