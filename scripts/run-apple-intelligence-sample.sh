#!/bin/zsh

set -euo pipefail

readonly SCRIPT_DIR="${0:A:h}"
readonly PROJECT_DIR="${SCRIPT_DIR:h}"
readonly SAMPLE_DIR="${PROJECT_DIR}/samples/apple-intelligence"
readonly BUILD_DIR="${SAMPLE_DIR}/.build"
readonly MODULE_CACHE_DIR="${BUILD_DIR}/module-cache"
readonly SAMPLE_APP="${BUILD_DIR}/TrackAISample.app"
readonly SAMPLE_CONTENTS="${SAMPLE_APP}/Contents"
readonly SAMPLE_BINARY="${SAMPLE_CONTENTS}/MacOS/TrackAISample"
readonly SAMPLE_STDOUT="${BUILD_DIR}/sample.stdout"
readonly SAMPLE_STDERR="${BUILD_DIR}/sample.stderr"
readonly CODESIGN_STDERR="${BUILD_DIR}/codesign.stderr"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "このサンプルはmacOSでのみ実行できます。" >&2
  exit 1
fi

readonly OS_MAJOR="$(sw_vers -productVersion | cut -d. -f1)"
if (( OS_MAJOR < 26 )); then
  echo "このサンプルにはmacOS 26以降が必要です。" >&2
  exit 1
fi

readonly SDK_PATH="$(xcrun --sdk macosx --show-sdk-path)"
if [[ ! -d "${SDK_PATH}/System/Library/Frameworks/FoundationModels.framework" ]]; then
  echo "Foundation Modelsを含むmacOS 26 SDKが見つかりません。" >&2
  echo "対応するXcodeまたはCommand Line Toolsをインストールしてください。" >&2
  exit 1
fi

mkdir -p "${SAMPLE_CONTENTS}/MacOS" "${MODULE_CACHE_DIR}"
cp "${SAMPLE_DIR}/Info.plist" "${SAMPLE_CONTENTS}/Info.plist"

CLANG_MODULE_CACHE_PATH="${MODULE_CACHE_DIR}" \
  xcrun --sdk macosx swiftc \
    -parse-as-library \
    -target arm64-apple-macosx26.0 \
    -sdk "${SDK_PATH}" \
    -module-cache-path "${MODULE_CACHE_DIR}" \
    "${SAMPLE_DIR}/TrackAISample.swift" \
    -o "${SAMPLE_BINARY}"

if ! codesign --force --deep --sign - "${SAMPLE_APP}" 2> "${CODESIGN_STDERR}"; then
  cat "${CODESIGN_STDERR}" >&2
  exit 1
fi

# Foundation Modelsのモデル管理サービスは、アプリをLaunchServices経由で起動した
# ときに接続できる。バイナリを直接実行するとavailabilityの確認は通っても、
# 最初の生成時にModelManagerError 1008となる。
: > "${SAMPLE_STDOUT}"
: > "${SAMPLE_STDERR}"
open -n -W \
  --stdout "${SAMPLE_STDOUT}" \
  --stderr "${SAMPLE_STDERR}" \
  "${SAMPLE_APP}" \
  --args "$@"

if [[ -s "${SAMPLE_STDERR}" ]]; then
  cat "${SAMPLE_STDERR}" >&2
fi
if [[ -s "${SAMPLE_STDOUT}" ]]; then
  cat "${SAMPLE_STDOUT}"
fi

if grep -q "生成できませんでした" "${SAMPLE_STDERR}" 2>/dev/null; then
  exit 1
fi
