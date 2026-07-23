#!/bin/bash
# Tauriの.appをビルドし、GitHub Release / Homebrew Cask用のzipへまとめる。
set -euo pipefail

cd "$(dirname "$0")/.."

APP_NAME="Track"
VERSION=$(node scripts/check-version.mjs)
APP_PATH="src-tauri/target/release/bundle/macos/${APP_NAME}.app"
ZIP_PATH="dist/${APP_NAME}-${VERSION}.zip"

if [[ "$(uname -s)" != "Darwin" || "$(uname -m)" != "arm64" ]]; then
  echo "エラー: 現在の配布物はApple Silicon Macでビルドしてください。" >&2
  exit 1
fi

npm run tauri:build

if [[ ! -d "$APP_PATH" ]]; then
  echo "エラー: アプリが生成されませんでした: ${APP_PATH}" >&2
  exit 1
fi

BUNDLE_VERSION=$(/usr/libexec/PlistBuddy \
  -c "Print :CFBundleShortVersionString" \
  "${APP_PATH}/Contents/Info.plist")
if [[ "$BUNDLE_VERSION" != "$VERSION" ]]; then
  echo "エラー: アプリのバージョンが一致しません (${BUNDLE_VERSION} != ${VERSION})。" >&2
  exit 1
fi

codesign --verify --deep --strict --verbose=2 "$APP_PATH"

mkdir -p dist
rm -f "$ZIP_PATH"
# .appの拡張属性と署名を保ったままアーカイブする。
ditto -c -k --keepParent "$APP_PATH" "$ZIP_PATH"

echo "作成しました: ${ZIP_PATH}"
shasum -a 256 "$ZIP_PATH"
