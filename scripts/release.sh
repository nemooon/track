#!/bin/bash
# 配布用zipを作成し、GitHub Releaseを公開する。
# release publishedイベントを受けてbump-cask.ymlがHomebrew tapを更新する。
set -euo pipefail

cd "$(dirname "$0")/.."

APP_NAME="Track"
VERSION=$(node scripts/check-version.mjs)
TAG="v${VERSION}"
ZIP_PATH="dist/${APP_NAME}-${VERSION}.zip"
NOTES_FILE="${1:-}"

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

scripts/package-release.sh

COMMIT=$(git rev-parse HEAD)
if [[ -n "$NOTES_FILE" ]]; then
  gh release create "$TAG" "$ZIP_PATH" \
    --target "$COMMIT" \
    --title "${APP_NAME} ${VERSION}" \
    --notes-file "$NOTES_FILE"
else
  gh release create "$TAG" "$ZIP_PATH" \
    --target "$COMMIT" \
    --title "${APP_NAME} ${VERSION}" \
    --generate-notes
fi

echo "公開しました: ${TAG}"
