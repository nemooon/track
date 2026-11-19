#!/bin/bash
# Builds Track.app (scripts/package.sh), zips it as Track-<version>.zip and publishes the GitHub
# Release v<version> with the zip and the DMG. Publishing triggers .github/workflows/bump-cask.yml,
# which points the Homebrew tap's cask (nemooon/tap/track) at the new zip.
#   scripts/release.sh [release-notes.md]
set -euo pipefail

cd "$(dirname "$0")/.."

VERSION=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
TAG="v${VERSION}"
NOTES_FILE="${1:-}"
OUT="target/package"
ZIP="$OUT/Track-${VERSION}.zip"
DMG="$OUT/Track_${VERSION}.dmg"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "エラー: 未コミットの変更があります。コミットしてからリリースしてください。" >&2
  exit 1
fi
if [[ -n "$NOTES_FILE" && ! -f "$NOTES_FILE" ]]; then
  echo "エラー: リリースノートが見つかりません: ${NOTES_FILE}" >&2
  exit 1
fi
if gh release view "$TAG" >/dev/null 2>&1; then
  echo "エラー: GitHub Release ${TAG} は既に存在します。" >&2
  exit 1
fi
if [[ "$(git rev-parse HEAD)" != "$(git rev-parse '@{u}' 2>/dev/null)" ]]; then
  echo "エラー: push していないコミットがあります（または追跡ブランチがありません）。" >&2
  exit 1
fi

scripts/package.sh
rm -f "$ZIP"
ditto -c -k --sequesterRsrc --keepParent "$OUT/Track.app" "$ZIP"

# Every release ends with where to read how to install and update.
FOOTER="インストールとアップデートの方法は [README](https://github.com/nemooon/track#インストール) にあります。"
NOTES=(--generate-notes --notes "$FOOTER")
if [[ -n "$NOTES_FILE" ]]; then
  BODY=$(mktemp)
  trap 'rm -f "$BODY"' EXIT
  printf '%s\n\n---\n\n%s\n' "$(cat "$NOTES_FILE")" "$FOOTER" > "$BODY"
  NOTES=(--notes-file "$BODY")
fi
gh release create "$TAG" "$ZIP" "$DMG" --target "$(git rev-parse HEAD)" --title "Track ${VERSION}" "${NOTES[@]}"
echo "公開しました: ${TAG}"
